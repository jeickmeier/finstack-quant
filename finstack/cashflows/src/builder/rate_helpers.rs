//! Centralized rate projection for floating rate instruments.
//!
//! Provides a single implementation of floating rate projection logic used across
//! all instruments: bonds, swaps, credit facilities, and structured products.
//!
//! ## Responsibilities
//!
//! - Project forward rates from market curves
//! - Apply floors and caps according to ISDA conventions
//! - Support gearing/leverage on rates
//! - Consistent floor/cap ordering
//!
//! ## API
//!
//! - [`project_floating_rate`]: Primary function taking a resolved forward curve and params
//! - [`project_floating_rate_from_market`]: Convenience wrapper that looks up the curve
//!
//! ## Formulas
//!
//! ### Gearing Includes Spread (Default)
//! `rate = cap( max( all_in_floor, gearing * ( max(index, floor) + spread ) ) )`
//!
//! ### Gearing Excludes Spread (Affine Model)
//! `rate = cap( max( all_in_floor, (gearing * max(index, floor)) + spread ) )`
//!
//! ## References
//!
//! - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - `docs/REFERENCES.md#hull-options-futures`

use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::warn;

/// Parameters for floating rate projection.
#[derive(Debug, Clone)]
pub struct FloatingRateParams {
    /// Spread over index in basis points.
    pub spread_bp: f64,

    /// Gearing multiplier (default: 1.0).
    pub gearing: f64,

    /// Whether gearing includes the spread (default: true).
    /// - `true`: `(Index + Spread) * Gearing`
    /// - `false`: `(Index * Gearing) + Spread`
    pub gearing_includes_spread: bool,

    /// Floor on index rate in basis points (applied to index component).
    pub index_floor_bp: Option<f64>,

    /// Cap on index rate in basis points (applied to index component).
    pub index_cap_bp: Option<f64>,

    /// Floor on all-in rate in basis points (Min Coupon).
    pub all_in_floor_bp: Option<f64>,

    /// Cap on all-in rate in basis points (Max Coupon).
    pub all_in_cap_bp: Option<f64>,
}

/// Runtime-resolved fallback policy for floating-rate projection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedFloatingRateFallback {
    /// Propagate the original error.
    Error,
    /// Use the spread-only fallback implied by the projection params.
    SpreadOnly,
    /// Use a fixed index rate, already converted to `f64`.
    FixedRate(f64),
}

impl ResolvedFloatingRateFallback {
    /// Return the fallback all-in rate when the policy permits it.
    #[must_use]
    pub fn fallback_rate(&self, params: &FloatingRateParams) -> Option<f64> {
        match self {
            Self::Error => None,
            Self::SpreadOnly => Some(project_fallback_rate(params)),
            Self::FixedRate(index_rate) => Some(calculate_floating_rate(*index_rate, params)),
        }
    }
}

/// Validated runtime floating-rate configuration used by coupon emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedFloatingRateSpec {
    /// Projection parameters consumed by the numerical helpers.
    pub params: FloatingRateParams,
    /// Runtime-resolved fallback policy.
    pub fallback: ResolvedFloatingRateFallback,
}

impl Default for FloatingRateParams {
    fn default() -> Self {
        Self {
            spread_bp: 0.0,
            gearing: 1.0,
            gearing_includes_spread: true,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
        }
    }
}

impl FloatingRateParams {
    /// Create params with just spread (most common case).
    ///
    /// # Example
    /// ```rust
    /// use finstack_cashflows::builder::rate_helpers::FloatingRateParams;
    ///
    /// let params = FloatingRateParams::with_spread(200.0); // 200 bps spread
    /// assert_eq!(params.spread_bp, 200.0);
    /// assert_eq!(params.gearing, 1.0);
    /// ```
    pub fn with_spread(spread_bp: f64) -> Self {
        Self {
            spread_bp,
            ..Default::default()
        }
    }

    /// Validate the floating rate parameters.
    ///
    /// Checks that:
    /// - Spread and gearing are finite numbers
    /// - Gearing is positive (non-zero)
    /// - Floor/cap pairs are not contradictory (floor <= cap)
    ///
    /// # Arguments
    ///
    /// * `self` - Floating-rate quote and floor/cap configuration to validate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all parameters are valid, otherwise returns an error
    /// describing the validation failure.
    ///
    /// # Errors
    ///
    /// Returns `InputError::Invalid` when any numeric input is non-finite,
    /// gearing is non-positive, or a floor exceeds its paired cap.
    pub fn validate(&self) -> Result<()> {
        use finstack_core::InputError;

        // Check spread is finite
        if !self.spread_bp.is_finite() {
            return Err(finstack_core::Error::Input(InputError::Invalid));
        }

        // Check gearing is positive and finite
        if !self.gearing.is_finite() || self.gearing <= 0.0 {
            return Err(finstack_core::Error::Input(InputError::Invalid));
        }

        // Check optional floor/cap values are finite if present
        for v in [
            self.index_floor_bp,
            self.index_cap_bp,
            self.all_in_floor_bp,
            self.all_in_cap_bp,
        ]
        .into_iter()
        .flatten()
        {
            if !v.is_finite() {
                return Err(finstack_core::Error::Input(InputError::Invalid));
            }
        }

        // Check index floor <= index cap if both specified
        if let (Some(floor), Some(cap)) = (self.index_floor_bp, self.index_cap_bp) {
            if floor > cap {
                return Err(finstack_core::Error::Input(InputError::Invalid));
            }
        }

        // Check all-in floor <= all-in cap if both specified
        if let (Some(floor), Some(cap)) = (self.all_in_floor_bp, self.all_in_cap_bp) {
            if floor > cap {
                return Err(finstack_core::Error::Input(InputError::Invalid));
            }
        }

        Ok(())
    }
}

/// Convert an optional [`Decimal`] constraint (floor/cap in bp) to `f64`.
///
/// Returns `None` both when the input is `None` and when the decimal fails
/// conversion to `f64` (which requires a pathologically out-of-range value).
/// A `warn!` is emitted for the latter case, matching the pre-existing soft
/// degradation in coupon emission for optional floor/cap fields.
fn optional_decimal_to_f64(value: Option<Decimal>, label: &str) -> Option<f64> {
    value.and_then(|d| {
        let v = d.to_f64();
        if v.is_none() {
            warn!(
                value = %d,
                "{label} Decimal-to-f64 conversion failed; constraint will be ignored"
            );
        }
        v
    })
}

impl TryFrom<&crate::builder::specs::FloatingRateSpec> for FloatingRateParams {
    type Error = finstack_core::Error;

    /// Canonical conversion from the serde-level `FloatingRateSpec` (Decimal) to
    /// the projection-level `FloatingRateParams` (f64).
    ///
    /// Required numeric fields (`spread_bp`, `gearing`) error on `Decimal → f64`
    /// overflow; optional floor/cap fields warn and drop the constraint on
    /// conversion failure, matching legacy coupon-emission semantics.
    fn try_from(spec: &crate::builder::specs::FloatingRateSpec) -> Result<Self> {
        use finstack_core::InputError;

        spec.validate()?;
        let spread_bp = spec
            .spread_bp
            .to_f64()
            .ok_or(finstack_core::Error::Input(InputError::ConversionOverflow))?;
        let gearing = spec
            .gearing
            .to_f64()
            .ok_or(finstack_core::Error::Input(InputError::ConversionOverflow))?;

        let params = FloatingRateParams {
            spread_bp,
            gearing,
            gearing_includes_spread: spec.gearing_includes_spread,
            index_floor_bp: optional_decimal_to_f64(spec.index_floor_bp, "index_floor_bp"),
            index_cap_bp: optional_decimal_to_f64(spec.index_cap_bp, "index_cap_bp"),
            all_in_floor_bp: optional_decimal_to_f64(spec.all_in_floor_bp, "all_in_floor_bp"),
            all_in_cap_bp: optional_decimal_to_f64(spec.all_in_cap_bp, "all_in_cap_bp"),
        };
        params.validate()?;
        Ok(params)
    }
}

impl TryFrom<&crate::builder::specs::FloatingRateSpec> for ResolvedFloatingRateSpec {
    type Error = finstack_core::Error;

    fn try_from(spec: &crate::builder::specs::FloatingRateSpec) -> Result<Self> {
        use crate::builder::specs::FloatingRateFallback;
        use finstack_core::InputError;

        let params = FloatingRateParams::try_from(spec)?;
        let fallback = match &spec.fallback {
            FloatingRateFallback::Error => ResolvedFloatingRateFallback::Error,
            FloatingRateFallback::SpreadOnly => ResolvedFloatingRateFallback::SpreadOnly,
            FloatingRateFallback::FixedRate(rate) => ResolvedFloatingRateFallback::FixedRate(
                rate.to_f64()
                    .ok_or(finstack_core::Error::Input(InputError::ConversionOverflow))?,
            ),
        };

        Ok(Self { params, fallback })
    }
}

/// Calculate the all-in floating rate from an index rate and parameters.
///
/// This is the core rate calculation logic that applies floors, caps, gearing,
/// and spread to an index rate. Used by both market-based projection (with real
/// forward rates) and fallback scenarios (with index=0).
///
/// # Arguments
///
/// * `index_rate` - The underlying index rate (decimal, e.g., 0.03 for 3%)
/// * `params` - Floating rate parameters (spread, gearing, floors, caps)
///
/// # Returns
///
/// The all-in rate as a decimal (e.g., 0.05 for 5%).
///
/// # Examples
///
/// ```rust
/// use finstack_cashflows::builder::rate_helpers::{calculate_floating_rate, FloatingRateParams};
///
/// let params = FloatingRateParams::with_spread(200.0); // 200 bps spread
/// let rate = calculate_floating_rate(0.03, &params); // 3% index + 2% spread = 5%
/// assert!((rate - 0.05).abs() < 0.0001);
/// ```
pub fn calculate_floating_rate(index_rate: f64, params: &FloatingRateParams) -> f64 {
    // Apply index floor/cap
    let mut eff_index = index_rate;
    if let Some(floor) = params.index_floor_bp {
        eff_index = eff_index.max(floor * 1e-4);
    }
    if let Some(cap) = params.index_cap_bp {
        eff_index = eff_index.min(cap * 1e-4);
    }

    // Calculate rate based on gearing style
    let mut rate = if params.gearing_includes_spread {
        // (Index + Spread) * Gearing
        (eff_index + params.spread_bp * 1e-4) * params.gearing
    } else {
        // (Index * Gearing) + Spread
        (eff_index * params.gearing) + params.spread_bp * 1e-4
    };

    // Apply all-in floor/cap
    if let Some(floor) = params.all_in_floor_bp {
        rate = rate.max(floor * 1e-4);
    }
    if let Some(cap) = params.all_in_cap_bp {
        rate = rate.min(cap * 1e-4);
    }

    rate
}

/// Calculate fallback rate assuming index is 0.0 (e.g., when curve is missing).
///
/// This is a convenience wrapper for scenarios where no forward curve is available.
/// The result is typically just the spread (with floor/cap applied).
///
/// # Arguments
///
/// * `params` - Floating rate parameters
///
/// # Returns
///
/// The all-in rate assuming a zero index rate.
///
/// # Examples
///
/// ```rust
/// use finstack_cashflows::builder::rate_helpers::{project_fallback_rate, FloatingRateParams};
///
/// let params = FloatingRateParams {
///     spread_bp: 200.0,
///     index_floor_bp: Some(100.0), // 1% index floor
///     ..Default::default()
/// };
/// let rate = project_fallback_rate(&params);
/// // Index floored to 1%, plus 2% spread = 3%
/// assert!((rate - 0.03).abs() < 0.0001);
/// ```
pub(crate) fn project_fallback_rate(params: &FloatingRateParams) -> f64 {
    calculate_floating_rate(0.0, params)
}

/// Project floating rate using a resolved forward curve and full parameter set.
///
/// This is the primary rate projection function. It computes the all-in floating
/// rate by:
/// 1. Looking up the forward rate from the curve for the accrual period
/// 2. Applying index floor/cap to the forward rate
/// 3. Adding spread and applying gearing
/// 4. Applying all-in floor/cap to the final rate
///
/// # Arguments
///
/// * `reset_date` - The rate fixing/reset date
/// * `reset_period_end` - End of the accrual period
/// * `fwd` - Resolved forward curve
/// * `params` - Floating rate parameters (spread, gearing, floors, caps)
///
/// # Returns
///
/// All-in projected coupon rate as a decimal.
///
/// # Errors
///
/// Returns an error if:
///
/// - `params` fails validation
/// - day-count conversion from the curve base date to the reset or accrual end
///   date fails
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - `docs/REFERENCES.md#hull-options-futures`
///
/// # Examples
///
/// ```rust
/// use finstack_core::dates::{Date, DayCount};
/// use finstack_core::market_data::term_structures::ForwardCurve;
/// use finstack_cashflows::builder::rate_helpers::{project_floating_rate, FloatingRateParams};
/// use time::Month;
///
/// let reset = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
/// let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
///
/// let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
///     .base_date(reset)
///     .day_count(DayCount::Act360)
///     .knots([(0.0, 0.03), (1.0, 0.04)])
///     .build()
///     .expect("curve");
///
/// let params = FloatingRateParams::with_spread(200.0); // SOFR + 200 bps
/// let rate = project_floating_rate(reset, period_end, &fwd, &params)?;
/// # Ok::<(), finstack_core::Error>(())
/// ```
pub fn project_floating_rate(
    reset_date: Date,
    reset_period_end: Date,
    fwd: &ForwardCurve,
    params: &FloatingRateParams,
) -> Result<f64> {
    // Validate parameters before projection
    params.validate()?;

    let fwd_dc = fwd.day_count();
    let fwd_base = fwd.base_date();

    // Compute time points for the accrual period
    //
    // Curves are defined from their base date forward; when pricing instruments whose
    // reset dates fall on/before the curve base date (e.g., seasoned swaps or T+0
    // test fixtures), clamp to t=0 rather than erroring on an inverted date range.
    let t0 = if reset_date <= fwd_base {
        0.0
    } else {
        fwd_dc.year_fraction(fwd_base, reset_date, DayCountContext::default())?
    };
    let t1 = if reset_period_end <= fwd_base {
        0.0
    } else {
        fwd_dc.year_fraction(fwd_base, reset_period_end, DayCountContext::default())?
    };

    // Get period forward rate (robust to zero-length periods).
    let index_rate = if t1 > t0 {
        fwd.rate_period(t0, t1)
    } else {
        fwd.rate(t0)
    };

    // Use shared calculation logic
    Ok(calculate_floating_rate(index_rate, params))
}

/// Project floating rate by looking up the forward curve from market context.
///
/// This is a convenience wrapper around [`project_floating_rate`] that handles
/// the curve lookup from a `MarketContext`.
///
/// # Arguments
///
/// * `reset_date` - The rate fixing/reset date
/// * `reset_period_end` - End of the accrual period
/// * `index_id` - Forward curve identifier (e.g., "USD-SOFR-3M")
/// * `params` - Floating rate parameters
/// * `market` - Market context containing forward curves
///
/// # Returns
///
/// All-in projected coupon rate as a decimal.
///
/// # Errors
///
/// Returns an error if the forward curve cannot be found in `market`, if
/// `params` fails validation, or if the underlying time conversion fails.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
///
/// # Examples
///
/// ```rust
/// use finstack_core::dates::{Date, DayCount};
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::market_data::term_structures::ForwardCurve;
/// use finstack_cashflows::builder::rate_helpers::{
///     project_floating_rate_from_market, FloatingRateParams
/// };
/// use time::Month;
///
/// let reset = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
/// let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
///
/// let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
///     .base_date(reset)
///     .day_count(DayCount::Act360)
///     .knots([(0.0, 0.03), (1.0, 0.04)])
///     .build()
///     .expect("curve");
/// let market = MarketContext::new().insert(fwd);
///
/// let params = FloatingRateParams::with_spread(200.0);
/// let rate = project_floating_rate_from_market(
///     reset, period_end, "USD-SOFR-3M", &params, &market
/// )?;
/// # Ok::<(), finstack_core::Error>(())
/// ```
pub fn project_floating_rate_from_market(
    reset_date: Date,
    reset_period_end: Date,
    index_id: &str,
    params: &FloatingRateParams,
    market: &MarketContext,
) -> Result<f64> {
    let fwd = market.get_forward(index_id)?;
    project_floating_rate(reset_date, reset_period_end, fwd.as_ref(), params)
}

/// Compute a compounded overnight rate for a single accrual period.
///
/// Implements the ISDA 2021 compounded-in-arrears formula:
///
/// ```text
/// Rate = [∏(1 + r_i × d_i / 360) - 1] × 360 / D
/// ```
///
/// # Arguments
///
/// * `daily_rates` — slice of (rate, accrual_days) pairs for each overnight fixing
/// * `total_days` — total calendar days in the accrual period
/// * `day_count_basis` — annual day count basis (typically 360 for USD SOFR)
///
/// # Returns
///
/// The annualized compounded rate in decimal form.
/// Returns `0.0` when inputs do not define a valid compounding period,
/// including non-positive or non-finite day-count bases.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
///
/// # Examples
///
/// ```rust
/// use finstack_cashflows::builder::rate_helpers::compute_compounded_rate;
///
/// let fixings = vec![
///     (0.05, 1u32), (0.05, 1), (0.05, 1), (0.05, 1), (0.05, 3),
/// ];
/// let rate = compute_compounded_rate(&fixings, 7, 360.0);
/// assert!((rate - 0.05).abs() < 0.001);
/// ```
pub(crate) fn compute_compounded_rate(
    daily_rates: &[(f64, u32)],
    total_days: u32,
    day_count_basis: f64,
) -> f64 {
    if daily_rates.is_empty()
        || total_days == 0
        || day_count_basis <= 0.0
        || !day_count_basis.is_finite()
    {
        return 0.0;
    }
    let mut product = 1.0;
    for &(rate, days) in daily_rates {
        product *= 1.0 + rate * (days as f64) / day_count_basis;
    }
    (product - 1.0) * day_count_basis / (total_days as f64)
}

fn compute_compounded_rate_with_lockout(
    daily_rates: &[(f64, u32)],
    total_days: u32,
    day_count_basis: f64,
    lockout_days: u32,
) -> f64 {
    if daily_rates.is_empty()
        || total_days == 0
        || day_count_basis <= 0.0
        || !day_count_basis.is_finite()
    {
        return 0.0;
    }

    let n = daily_rates.len();
    let lockout = lockout_days as usize;
    let lockout_rate = if n > lockout {
        daily_rates[n - lockout - 1].0
    } else {
        daily_rates[0].0
    };
    let lockout_start = n.saturating_sub(lockout);

    let mut product = 1.0;
    for (i, &(rate, days)) in daily_rates.iter().enumerate() {
        let effective_rate = if i >= lockout_start {
            lockout_rate
        } else {
            rate
        };
        product *= 1.0 + effective_rate * (days as f64) / day_count_basis;
    }

    (product - 1.0) * day_count_basis / (total_days as f64)
}

/// Compute a simple average overnight rate for a single accrual period.
///
/// ```text
/// Rate = (Σ r_i × d_i) / D
/// ```
///
/// # Arguments
///
/// * `daily_rates` - Slice of `(rate, accrual_days)` pairs for each fixing.
/// * `total_days` - Total calendar days in the accrual period.
///
/// # Returns
///
/// Annualized simple-average overnight rate in decimal form.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
///
/// # Examples
///
/// ```rust
/// use finstack_cashflows::builder::rate_helpers::compute_simple_average_rate;
///
/// let fixings = vec![(0.05, 1u32), (0.06, 1), (0.04, 3)];
/// let rate = compute_simple_average_rate(&fixings, 5);
/// assert!(rate > 0.0);
/// ```
pub(crate) fn compute_simple_average_rate(daily_rates: &[(f64, u32)], total_days: u32) -> f64 {
    if daily_rates.is_empty() || total_days == 0 {
        return 0.0;
    }
    let weighted_sum: f64 = daily_rates
        .iter()
        .map(|&(rate, days)| rate * (days as f64))
        .sum();
    weighted_sum / (total_days as f64)
}

/// Apply overnight compounding method to compute the period rate from daily fixings.
///
/// Dispatches to the appropriate calculation based on `OvernightCompoundingMethod`.
///
/// # Arguments
///
/// * `method` — the compounding convention to apply
/// * `daily_rates` — slice of (rate, accrual_days) pairs, ordered chronologically
/// * `total_days` — total calendar days in the accrual period
/// * `day_count_basis` — annual day count basis (typically 360 for USD)
///
/// # Returns
///
/// The period rate as a decimal.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
///
/// # Examples
///
/// ```rust
/// use finstack_cashflows::builder::rate_helpers::compute_overnight_rate;
/// use finstack_cashflows::builder::OvernightCompoundingMethod;
///
/// let fixings = vec![(0.05, 1u32), (0.05, 1), (0.05, 3)];
/// let rate = compute_overnight_rate(
///     OvernightCompoundingMethod::CompoundedInArrears,
///     &fixings,
///     5,
///     360.0,
/// );
///
/// assert!(rate > 0.0);
/// ```
pub(crate) fn compute_overnight_rate(
    method: super::specs::OvernightCompoundingMethod,
    daily_rates: &[(f64, u32)],
    total_days: u32,
    day_count_basis: f64,
) -> f64 {
    use super::specs::OvernightCompoundingMethod;

    match method {
        OvernightCompoundingMethod::SimpleAverage => {
            compute_simple_average_rate(daily_rates, total_days)
        }
        OvernightCompoundingMethod::CompoundedInArrears => {
            compute_compounded_rate(daily_rates, total_days, day_count_basis)
        }
        OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: _ } => {
            // ARRC 2020 §2 / ISDA 2021 Supp. 70 §7.1(g)(ii) "Lookback": rate
            // observations are offset earlier by `lookback_days` business
            // days; accrual-period day weights are preserved. The upstream
            // `sample_overnight_rates_with_lookback` in coupons.rs already
            // pairs shifted rates with accrual weights, so the dispatcher
            // reduces to standard in-arrears compounding over the assembled
            // (shifted_rate, accrual_weight) pairs.
            //
            // The shift is applied upstream during sampling rather than
            // by rewriting indices into an accrual-window sample, so
            // rates from before `accrual_start` are available to the
            // first `lookback_days` observations instead of clamping to
            // `daily_rates[0]`.
            compute_compounded_rate(daily_rates, total_days, day_count_basis)
        }
        OvernightCompoundingMethod::CompoundedWithLockout { lockout_days } => {
            compute_compounded_rate_with_lockout(
                daily_rates,
                total_days,
                day_count_basis,
                lockout_days,
            )
        }
        OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days: _ } => {
            // ISDA 2021 Supp. 70 §7.1(g) "Observation Shift" variant: the
            // observation window itself is moved earlier by the configured
            // number of business days, and BOTH the sampled rates AND the
            // per-day weights come from the shifted window. Annualization
            // uses the shifted-window day count.
            //
            // The window shift is performed upstream in
            // `emission::coupons::emit_float_coupons_on` (see
            // `observation_window`). By the time we get here,
            // `daily_rates` and `total_days` already describe the
            // observation period, so the compounding product and
            // annualization reduce to the same formula as
            // `CompoundedInArrears`. Shifting indices *within* an
            // accrual-window sample (the previous approach) could not
            // access pre-accrual rates and produced SOFR/SONIA errors
            // of 2–10 bp in normal regimes (wider at rate-move
            // boundaries).
            compute_compounded_rate(daily_rates, total_days, day_count_basis)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::{Date, DayCount};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::ForwardCurve;
    use time::Month;

    fn create_test_market(base_date: Date) -> MarketContext {
        let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        MarketContext::new().insert(fwd_curve)
    }

    #[test]
    fn test_project_floating_rate_no_floor_no_cap() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");
        let market = create_test_market(reset);

        let params = FloatingRateParams::with_spread(200.0); // 200 bps
        let rate =
            project_floating_rate_from_market(reset, period_end, "USD-SOFR-3M", &params, &market)
                .expect("Rate projection should succeed in test");

        // Should be ~3% index + 2% spread = ~5%
        assert!(rate > 0.04 && rate < 0.06, "Rate should be ~5%: {}", rate);
    }

    #[test]
    fn test_project_floating_rate_with_floor() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        // Create market with very low rates (below floor)
        let fwd_curve = ForwardCurve::builder("USD-LIBOR-3M", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.001), (1.0, 0.001), (5.0, 0.001)]) // 0.1% < 1% floor
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            index_floor_bp: Some(100.0),
            ..Default::default()
        }; // 100 bps spread, 1% floor
        let rate =
            project_floating_rate_from_market(reset, period_end, "USD-LIBOR-3M", &params, &market)
                .expect("Rate projection should succeed in test");

        // Floor lifts index to 1%, plus 1% spread = 2%
        assert!(
            (rate - 0.02).abs() < 0.001,
            "Rate should be ~2% (floor + spread): {}",
            rate
        );
    }

    #[test]
    fn test_project_floating_rate_with_cap() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        // Create market with high rates
        let fwd_curve = ForwardCurve::builder("USD-LIBOR-3M", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.08), (1.0, 0.08), (5.0, 0.08)]) // 8% index
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 200.0,
            all_in_cap_bp: Some(500.0),
            ..Default::default()
        }; // 200 bps spread, 5% cap
        let rate =
            project_floating_rate_from_market(reset, period_end, "USD-LIBOR-3M", &params, &market)
                .expect("Rate projection should succeed in test");

        // 8% index + 2% spread = 10%, capped at 5%
        assert!(
            (rate - 0.05).abs() < 0.001,
            "Rate should be capped at 5%: {}",
            rate
        );
    }

    #[test]
    fn test_floor_applied_before_spread() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        // Use very low rate (0.01% = 1 bp) which is below the floor
        let fwd_curve = ForwardCurve::builder("TEST-INDEX", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.0001), (1.0, 0.0001)]) // 0.01% index (below 1% floor)
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            index_floor_bp: Some(100.0),
            ..Default::default()
        }; // 100 bps spread, 1% floor
        let rate =
            project_floating_rate_from_market(reset, period_end, "TEST-INDEX", &params, &market)
                .expect("Rate projection should succeed in test");

        // Floor lifts index from 0.01% to 1%, then add 1% spread = 2%
        assert!(
            (rate - 0.02).abs() < 0.001,
            "Rate should be 2% (floored index + spread): {}",
            rate
        );
    }

    #[test]
    fn test_cap_applied_after_gearing() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        let fwd_curve = ForwardCurve::builder("TEST-INDEX", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.03)]) // 3% index
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            all_in_cap_bp: Some(600.0),
            ..Default::default()
        }; // 100 bps spread, 2x gearing, 6% cap
        let rate =
            project_floating_rate_from_market(reset, period_end, "TEST-INDEX", &params, &market)
                .expect("Rate projection should succeed in test");

        // (3% index + 1% spread) * 2 = 8%, capped at 6%
        assert!(
            (rate - 0.06).abs() < 0.001,
            "Rate should be capped at 6% after gearing: {}",
            rate
        );
    }

    #[test]
    fn test_gearing_multiplies_all_in_rate() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        let fwd_curve = ForwardCurve::builder("TEST-INDEX", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.02), (1.0, 0.02)]) // 2% index
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 1.5,
            ..Default::default()
        }; // 100 bps spread, 1.5x gearing
        let rate =
            project_floating_rate_from_market(reset, period_end, "TEST-INDEX", &params, &market)
                .expect("Rate projection should succeed in test");

        // (2% + 1%) * 1.5 = 4.5%
        assert!(
            (rate - 0.045).abs() < 0.001,
            "Rate should be 4.5% with gearing: {}",
            rate
        );
    }

    #[test]
    fn test_direct_curve_projection() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        let fwd_curve = ForwardCurve::builder("TEST-INDEX", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");

        let params = FloatingRateParams::with_spread(150.0); // 150 bps
        let rate = project_floating_rate(reset, period_end, &fwd_curve, &params)
            .expect("Rate projection should succeed in test");

        // Should project forward rate + spread
        assert!(
            rate > 0.03 && rate < 0.06,
            "Rate should be reasonable: {}",
            rate
        );
    }

    // =========================================================================
    // Validation tests
    // =========================================================================

    #[test]
    fn test_params_validate_default_succeeds() {
        let params = FloatingRateParams::default();
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_standard_gearing_applies_to_spread() {
        // Standard: (Index + Spread) * Gearing
        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            gearing_includes_spread: true,
            ..Default::default()
        };
        assert!(params.gearing_includes_spread);
        assert_eq!(params.spread_bp, 100.0);
        assert_eq!(params.gearing, 2.0);

        // 3% index + 1% spread = 4%, then * 2 = 8%
        let rate = calculate_floating_rate(0.03, &params);
        assert!(
            (rate - 0.08).abs() < 0.0001,
            "Standard: (3% + 1%) * 2 = 8%, got {}",
            rate
        );
    }

    #[test]
    fn test_affine_gearing_applies_only_to_index() {
        // Affine: (Index * Gearing) + Spread
        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            gearing_includes_spread: false,
            ..Default::default()
        };
        assert!(!params.gearing_includes_spread);
        assert_eq!(params.spread_bp, 100.0);
        assert_eq!(params.gearing, 2.0);

        // (3% * 2) + 1% = 6% + 1% = 7%
        let rate = calculate_floating_rate(0.03, &params);
        assert!(
            (rate - 0.07).abs() < 0.0001,
            "Affine: (3% * 2) + 1% = 7%, got {}",
            rate
        );
    }

    #[test]
    fn test_standard_vs_affine_difference() {
        // The difference between standard and affine is: Spread * (Gearing - 1)
        // With 100 bps spread and 2x gearing: 100 * (2 - 1) = 100 bps = 1%
        let standard = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            gearing_includes_spread: true,
            ..Default::default()
        };
        let affine = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            gearing_includes_spread: false,
            ..Default::default()
        };

        let rate_standard = calculate_floating_rate(0.03, &standard);
        let rate_affine = calculate_floating_rate(0.03, &affine);

        // Standard is higher by exactly Spread * (Gearing - 1) = 1%
        let diff = rate_standard - rate_affine;
        assert!(
            (diff - 0.01).abs() < 0.0001,
            "Difference should be 1%, got {}",
            diff
        );
    }

    #[test]
    fn test_params_validate_valid_floor_cap() {
        let params = FloatingRateParams {
            all_in_floor_bp: Some(100.0),
            all_in_cap_bp: Some(500.0),
            ..Default::default()
        };
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_params_validate_contradictory_all_in_floor_cap() {
        let params = FloatingRateParams {
            all_in_floor_bp: Some(500.0), // 5% floor
            all_in_cap_bp: Some(300.0),   // 3% cap < floor!
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_params_validate_contradictory_index_floor_cap() {
        let params = FloatingRateParams {
            index_floor_bp: Some(200.0),
            index_cap_bp: Some(100.0), // cap < floor!
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_params_validate_nan_spread() {
        let params = FloatingRateParams {
            spread_bp: f64::NAN,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_params_validate_zero_gearing() {
        let params = FloatingRateParams {
            gearing: 0.0,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_params_validate_negative_gearing() {
        let params = FloatingRateParams {
            gearing: -1.0,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_projection_fails_on_invalid_params() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");

        let fwd_curve = ForwardCurve::builder("TEST-INDEX", 0.25)
            .base_date(reset)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("ForwardCurve builder should succeed");

        // Invalid params: cap < floor
        let params = FloatingRateParams {
            all_in_floor_bp: Some(500.0),
            all_in_cap_bp: Some(300.0),
            ..Default::default()
        };

        let result = project_floating_rate(reset, period_end, &fwd_curve, &params);
        assert!(result.is_err(), "Should fail with contradictory floor/cap");
    }

    // =========================================================================
    // Overnight compounding tests
    // =========================================================================

    #[test]
    fn test_compounded_rate_constant_fixings() {
        let fixings = vec![(0.05, 1u32), (0.05, 1), (0.05, 1), (0.05, 1), (0.05, 3)];
        let rate = compute_compounded_rate(&fixings, 7, 360.0);
        assert!(
            (rate - 0.05).abs() < 0.001,
            "Compounded rate with constant fixings should be ~5%: {rate:.6}"
        );
    }

    #[test]
    fn test_compounded_rate_varying_fixings() {
        let fixings = vec![
            (0.0530, 1u32),
            (0.0528, 1),
            (0.0531, 1),
            (0.0529, 1),
            (0.0530, 3),
        ];
        let rate = compute_compounded_rate(&fixings, 7, 360.0);
        assert!(
            rate > 0.052 && rate < 0.054,
            "Rate should be in range: {rate:.6}"
        );
    }

    #[test]
    fn test_compounded_rate_empty() {
        assert_eq!(compute_compounded_rate(&[], 7, 360.0), 0.0);
    }

    #[test]
    fn test_compounded_rate_invalid_basis_returns_zero() {
        let fixings = vec![(0.05, 1u32), (0.05, 1)];

        assert_eq!(compute_compounded_rate(&fixings, 2, 0.0), 0.0);
        assert_eq!(compute_compounded_rate(&fixings, 2, -360.0), 0.0);
        assert_eq!(compute_compounded_rate(&fixings, 2, f64::NAN), 0.0);
        assert_eq!(compute_compounded_rate(&fixings, 2, f64::INFINITY), 0.0);
    }

    #[test]
    fn test_simple_average_rate() {
        let fixings = vec![(0.05, 1u32), (0.06, 1), (0.04, 1), (0.05, 1), (0.05, 3)];
        let rate = compute_simple_average_rate(&fixings, 7);
        let expected = (0.05 + 0.06 + 0.04 + 0.05 + 0.15) / 7.0;
        assert!(
            (rate - expected).abs() < 1e-12,
            "got {rate:.6}, expected {expected:.6}"
        );
    }

    #[test]
    fn test_overnight_compounded_in_arrears() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.05, 1u32), (0.05, 1), (0.05, 1), (0.05, 1), (0.05, 3)];
        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::CompoundedInArrears,
            &fixings,
            7,
            360.0,
        );
        assert!(
            (rate - 0.05).abs() < 0.001,
            "CompoundedInArrears: {rate:.6}"
        );
    }

    #[test]
    fn test_overnight_compounded_invalid_basis_returns_zero() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.05, 1u32), (0.05, 1)];

        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::CompoundedInArrears,
            &fixings,
            2,
            0.0,
        );

        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_overnight_lockout() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.05, 1u32), (0.05, 1), (0.05, 1), (0.06, 1), (0.07, 3)];
        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 2 },
            &fixings,
            7,
            360.0,
        );
        // Lockout freezes last 2 fixings to rate of day 2 (0.05)
        assert!((rate - 0.05).abs() < 0.001, "Lockout rate: {rate:.6}");
    }

    #[test]
    fn test_overnight_lockout_matches_explicit_locked_fixings() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.04, 1u32), (0.05, 1), (0.06, 1), (0.07, 1), (0.08, 3)];
        let explicit_locked = vec![(0.04, 1u32), (0.05, 1), (0.06, 1), (0.06, 1), (0.06, 3)];

        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 2 },
            &fixings,
            7,
            360.0,
        );
        let expected = compute_compounded_rate(&explicit_locked, 7, 360.0);

        assert!(
            (rate - expected).abs() < 1e-12,
            "allocation-free lockout should match explicit locked fixings"
        );
    }

    #[test]
    fn test_overnight_simple_average() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.05, 1u32), (0.05, 1), (0.05, 1), (0.05, 1), (0.05, 3)];
        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::SimpleAverage,
            &fixings,
            7,
            360.0,
        );
        assert!((rate - 0.05).abs() < 1e-12, "Simple average: {rate:.6}");
    }

    #[test]
    fn test_compounded_vs_simple_divergence() {
        let fixings = vec![(0.01, 1u32), (0.10, 1), (0.01, 1), (0.10, 1), (0.01, 3)];
        let compounded = compute_compounded_rate(&fixings, 7, 360.0);
        let simple = compute_simple_average_rate(&fixings, 7);
        assert!(compounded > 0.0);
        assert!(simple > 0.0);
        assert!((compounded - simple).abs() < 0.01);
    }

    #[test]
    fn test_overnight_lookback() {
        use crate::builder::specs::OvernightCompoundingMethod;
        let fixings = vec![(0.04, 1u32), (0.05, 1), (0.06, 1), (0.07, 1), (0.08, 3)];
        let rate = compute_overnight_rate(
            OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: 2 },
            &fixings,
            7,
            360.0,
        );
        assert!(rate > 0.0 && rate.is_finite(), "Lookback rate: {rate:.6}");
    }

    // =========================================================================
    // FloatingRateSpec → FloatingRateParams conversion
    // =========================================================================

    #[test]
    fn try_from_floating_rate_spec_round_trips_all_fields() {
        use crate::builder::specs::{FloatingRateFallback, FloatingRateSpec};
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(200.0),
            gearing: dec!(1.5),
            gearing_includes_spread: false,
            index_floor_bp: Some(dec!(25.0)),
            all_in_floor_bp: Some(dec!(50.0)),
            all_in_cap_bp: Some(dec!(1500.0)),
            index_cap_bp: Some(dec!(1200.0)),
            reset_freq: Tenor::quarterly(),
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        };

        let params = FloatingRateParams::try_from(&spec).expect("conversion should succeed");

        assert!((params.spread_bp - 200.0).abs() < 1e-12);
        assert!((params.gearing - 1.5).abs() < 1e-12);
        assert!(!params.gearing_includes_spread);
        assert_eq!(params.index_floor_bp, Some(25.0));
        assert_eq!(params.all_in_floor_bp, Some(50.0));
        assert_eq!(params.all_in_cap_bp, Some(1500.0));
        assert_eq!(params.index_cap_bp, Some(1200.0));
    }

    #[test]
    fn try_from_floating_rate_spec_maps_none_constraints() {
        use crate::builder::specs::{FloatingRateFallback, FloatingRateSpec};
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(100.0),
            gearing: dec!(1.0),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            reset_freq: Tenor::quarterly(),
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        };

        let params = FloatingRateParams::try_from(&spec).expect("conversion should succeed");
        assert_eq!(params.index_floor_bp, None);
        assert_eq!(params.index_cap_bp, None);
        assert_eq!(params.all_in_floor_bp, None);
        assert_eq!(params.all_in_cap_bp, None);
    }

    #[test]
    fn try_from_floating_rate_spec_rejects_contradictory_caps_and_floors() {
        use crate::builder::specs::{FloatingRateFallback, FloatingRateSpec};
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(100.0),
            gearing: dec!(1.0),
            gearing_includes_spread: true,
            index_floor_bp: Some(dec!(200.0)),
            all_in_floor_bp: Some(dec!(600.0)),
            all_in_cap_bp: Some(dec!(500.0)),
            index_cap_bp: Some(dec!(100.0)),
            reset_freq: Tenor::quarterly(),
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        };

        assert!(
            FloatingRateParams::try_from(&spec).is_err(),
            "runtime conversion should reject contradictory floating-rate constraints"
        );
    }
}
