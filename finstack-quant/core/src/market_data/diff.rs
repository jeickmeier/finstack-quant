//! Market data comparison and shift measurement.
//!
//! Provides utilities for measuring market movements between two `MarketContext`
//! instances. Used primarily for metrics-based P&L attribution, risk reporting,
//! and scenario analysis.
//!
//! # Purpose
//!
//! This module is symmetric to `bumps.rs`:
//! - **bumps.rs**: Apply shifts to create scenarios
//! - **diff.rs**: Measure shifts between markets
//!
//! # Use Cases
//!
//! - **P&L Attribution**: Explain P&L changes via DV01 × Δrates, CS01 × Δspreads
//! - **Risk Reporting**: Measure daily market moves for VaR and stress testing
//! - **Calibration**: Compare calibrated curves vs market inputs
//! - **Market Analysis**: Track curve movements over time
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::diff::{measure_discount_curve_shift, TenorSamplingMethod};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::types::CurveId;
//!
//! # fn example(market_yesterday: MarketContext, market_today: MarketContext) -> finstack_quant_core::Result<()> {
//! // Measure rate shift between two markets
//! let shift_bp = measure_discount_curve_shift(
//!     &CurveId::from("USD-OIS"),
//!     &market_yesterday,
//!     &market_today,
//!     TenorSamplingMethod::Standard,
//! )?;
//!
//! println!("USD-OIS moved {} basis points", shift_bp);
//! # Ok(())
//! # }
//! ```

use super::context::MarketContext;
use crate::currency::Currency;
use crate::dates::{Date, DayCount, DayCountContext};
use crate::Result;

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

/// Standard market tenors for curve sampling (in years).
///
/// Based on liquid swap market points: 3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y.
/// These are the conventional points where curves are most actively traded
/// and quoted.
pub const STANDARD_TENORS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];

/// ATM reference strike multiplier (1.0 = 100% of spot).
pub const ATM_MONEYNESS: f64 = 1.0;

/// Default volatility surface expiry for sampling (1 year).
pub const DEFAULT_VOL_EXPIRY: f64 = 1.0;

// -----------------------------------------------------------------------------
// Tenor Sampling Method
// -----------------------------------------------------------------------------

/// Method for selecting tenor points when measuring curve shifts.
///
/// Different sampling strategies trade off accuracy, performance, and
/// robustness to curve structure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenorSamplingMethod {
    /// Standard swap market tenors (3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y).
    ///
    /// Most robust for typical interest rate curves. Matches market liquidity
    /// points and works well for parallel shift detection.
    #[default]
    Standard,

    /// Use curve's own knot points dynamically.
    ///
    /// Adapts to curve structure but may miss shifts between pillars.
    /// Good for curves with non-standard pillar structure.
    Dynamic,

    /// Custom tenor list specified by caller.
    ///
    /// Allows fine-grained control for specific use cases (e.g., matching
    /// an instrument's cashflow schedule).
    Custom(Vec<f64>),
}

impl TenorSamplingMethod {
    /// Get the tenor points to sample based on the method.
    ///
    /// For `Dynamic`, uses the knot points from the provided curve.
    /// For `Standard`, uses `STANDARD_TENORS`.
    /// For `Custom`, uses the provided tenor list.
    fn tenors<'a>(&'a self, curve_knots: Option<&'a [f64]>) -> &'a [f64] {
        match self {
            Self::Standard => STANDARD_TENORS,
            Self::Dynamic => curve_knots.unwrap_or(STANDARD_TENORS),
            Self::Custom(tenors) => tenors.as_slice(),
        }
    }
}

// -----------------------------------------------------------------------------
// Curve Shift Measurements
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Curve Shift Measurements
// -----------------------------------------------------------------------------

/// Generic internal measurement helper for curve-like objects.
fn measure_average_shift(
    sample_points: &[f64],
    scaling_factor: f64,
    mut value_t0: impl FnMut(f64) -> f64,
    mut value_t1: impl FnMut(f64) -> f64,
) -> f64 {
    let mut total_shift = 0.0;
    let mut count = 0;

    for &t in sample_points {
        if t <= 0.0 {
            continue;
        }
        let v0 = value_t0(t);
        let v1 = value_t1(t);

        let shift = if values_are_effectively_equal(v0, v1) {
            0.0
        } else {
            (v1 - v0) * scaling_factor
        };
        total_shift += shift;
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }
    total_shift / count as f64
}

fn values_are_effectively_equal(a: f64, b: f64) -> bool {
    const RELATIVE_EQUALITY_TOLERANCE: f64 = 1.0e-14;

    if !(a.is_finite() && b.is_finite()) {
        return false;
    }
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= RELATIVE_EQUALITY_TOLERANCE * scale
}

/// Measure average parallel rate shift in discount curve (basis points).
pub fn measure_discount_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_discount(&curve_id)?;
    let curve_t1 = market_t1.get_discount(&curve_id)?;

    let tenors = method.tenors(Some(curve_t0.knots()));
    Ok(measure_average_shift(
        tenors,
        10_000.0,
        |t| curve_t0.zero(t),
        |t| curve_t1.zero(t),
    ))
}

/// Measure average parallel spread shift in hazard curve (basis points).
pub fn measure_hazard_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;

    let t0_knots: Vec<f64> = curve_t0.knot_points().map(|(t, _)| t).collect();
    let tenors = match method {
        TenorSamplingMethod::Dynamic => t0_knots.as_slice(),
        _ => method.tenors(None),
    };

    Ok(measure_average_shift(
        tenors,
        10_000.0,
        |t| curve_t0.hazard_rate(t),
        |t| curve_t1.hazard_rate(t),
    ))
}

/// Measure the average **par CDS spread** shift on a hazard curve (basis points).
///
/// This is the par-spread analogue of [`measure_hazard_curve_shift`]: it samples
/// the par CDS spread (via `HazardCurve::cds_quote_bp`) rather than the raw
/// hazard rate. Credit sensitivities (`Cs01`, `BucketedCs01`) are defined per bp
/// of **par-spread** move, so attribution must pair them with this — pairing a
/// par-spread CS01 with a hazard-rate move overstates credit P&L by a factor of
/// `1 / (1 - recovery)`.
pub fn measure_par_spread_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;

    let t0_knots: Vec<f64> = curve_t0.knot_points().map(|(t, _)| t).collect();
    let tenors = match method {
        TenorSamplingMethod::Dynamic => t0_knots.as_slice(),
        _ => method.tenors(None),
    };

    let interp_t0 = curve_t0.par_interp();
    let interp_t1 = curve_t1.par_interp();
    // `cds_quote_bp` already returns basis points → scaling factor 1.0.
    Ok(measure_average_shift(
        tenors,
        1.0,
        |t| curve_t0.cds_quote_bp(t, interp_t0),
        |t| curve_t1.cds_quote_bp(t, interp_t1),
    ))
}

/// Measure the **per-tenor** par CDS spread shift (basis points) at the given
/// tenors.
///
/// Unlike [`measure_par_spread_shift`] (which averages over a tenor grid), this
/// returns the shift at each requested tenor so callers can pair it with a
/// per-tenor (key-rate) `BucketedCs01`, attributing non-parallel (twisted)
/// credit-curve moves correctly. Returns one entry per input tenor, in order.
pub fn measure_per_tenor_par_spread_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;
    let interp_t0 = curve_t0.par_interp();
    let interp_t1 = curve_t1.par_interp();
    Ok(tenors
        .iter()
        .map(|&t| curve_t1.cds_quote_bp(t, interp_t1) - curve_t0.cds_quote_bp(t, interp_t0))
        .collect())
}

/// Measure the average **credit-curve** shift (basis points), accepting either
/// curve representation a credit-risky instrument may use.
///
/// A credit curve declared in an instrument's `credit_curves` dependency can be
/// modelled two ways:
///
/// - a [`HazardCurve`](crate::market_data::term_structures::HazardCurve) — the
///   move is the **par CDS spread** shift ([`measure_par_spread_shift`]), the
///   basis a hazard-curve `Cs01` is defined on; or
/// - a [`DiscountCurve`](crate::market_data::term_structures::DiscountCurve) —
///   e.g. the Tsiveriotis–Zhang risky discount curve a convertible bond prices
///   off — the move is the **zero-rate** shift
///   ([`measure_discount_curve_shift`]), the basis a discount-style `Cs01` is
///   bumped on.
///
/// The hazard interpretation is tried first, falling back to the discount
/// interpretation, so the returned move is always unit-consistent with the
/// instrument's own CS01. Used by P&L attribution so credit-spread P&L from a
/// convertible's risky discount curve is attributed to the credit factor
/// rather than leaking into the residual.
pub fn measure_credit_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_id = curve_id.as_ref();
    if let Ok(shift) = measure_par_spread_shift(curve_id, market_t0, market_t1, method.clone()) {
        return Ok(shift);
    }
    measure_discount_curve_shift(curve_id, market_t0, market_t1, method)
}

/// Measure the **per-tenor** credit-curve shift (basis points) at the given
/// tenors, accepting either curve representation.
///
/// The per-tenor counterpart of [`measure_credit_curve_shift`] (see that
/// function for the hazard / discount duality): tries the par CDS spread
/// interpretation first ([`measure_per_tenor_par_spread_shift`]); on failure
/// falls back to the per-tenor discount zero-rate shift. Returns one entry per
/// input tenor, in order, so callers can pair it with a per-tenor (key-rate)
/// `BucketedCs01`.
pub fn measure_per_tenor_credit_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    let curve_id = curve_id.as_ref();
    if let Ok(shifts) = measure_per_tenor_par_spread_shift(curve_id, market_t0, market_t1, tenors) {
        return Ok(shifts);
    }
    let curve_t0 = market_t0.get_discount(curve_id)?;
    let curve_t1 = market_t1.get_discount(curve_id)?;
    Ok(tenors
        .iter()
        .map(|&t| (curve_t1.zero(t) - curve_t0.zero(t)) * 10_000.0)
        .collect())
}

/// Measure average inflation rate shift (basis points).
pub fn measure_inflation_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let curve_t0 = market_t0.get_inflation_curve(&curve_id)?;
    let curve_t1 = market_t1.get_inflation_curve(&curve_id)?;
    Ok(measure_average_shift(
        STANDARD_TENORS,
        10_000.0,
        |t| {
            let ratio = curve_t0.cpi(t) / curve_t0.base_cpi();
            if t == 0.0 {
                0.0
            } else {
                ratio.powf(1.0 / t) - 1.0
            }
        },
        |t| {
            let ratio = curve_t1.cpi(t) / curve_t1.base_cpi();
            if t == 0.0 {
                0.0
            } else {
                ratio.powf(1.0 / t) - 1.0
            }
        },
    ))
}

/// Measure the annualized inflation-rate shift represented by two published
/// index snapshots, in basis points.
///
/// The comparison uses a common anchor and the latest date present in either
/// snapshot. `InflationIndex::value_on` deliberately carries the last published
/// value forward, so a newly released print in `market_t1` is measured against
/// the information set that was available in `market_t0`.
pub fn measure_inflation_index_shift(
    index_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let index_id = index_id.as_ref();
    let index_t0 = market_t0.get_inflation_index(index_id)?;
    let index_t1 = market_t1.get_inflation_index(index_id)?;
    let (first_t0, last_t0) = index_t0.date_range()?;
    let (first_t1, last_t1) = index_t1.date_range()?;
    let anchor = first_t0.max(first_t1);
    let end = last_t0.max(last_t1);
    if end <= anchor {
        return Err(crate::InputError::TooFewPoints.into());
    }
    let t = DayCount::Act365F.year_fraction(anchor, end, DayCountContext::default())?;
    if t <= 0.0 {
        return Err(crate::InputError::InvalidDateRange.into());
    }
    let rate = |index: &super::scalars::InflationIndex| -> Result<f64> {
        let base = index.value_on(anchor)?;
        let terminal = index.value_on(end)?;
        if !base.is_finite() || !terminal.is_finite() || base <= 0.0 || terminal <= 0.0 {
            return Err(crate::InputError::Invalid.into());
        }
        Ok((terminal / base).powf(1.0 / t) - 1.0)
    };
    Ok((rate(index_t1.as_ref())? - rate(index_t0.as_ref())?) * 10_000.0)
}

/// Measure a declared inflation source, combining projected-curve and
/// published-index shifts when both are present.
///
/// A hybrid source has two independent information changes: movement in the
/// projected zero-inflation curve and discrete publication of realized CPI.
/// Both are expressed in basis-point-equivalent annualized inflation rates and
/// are additive for first-order attribution.
pub fn measure_inflation_source_shift(
    source_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let source_id = source_id.as_ref();
    let has_curve = market_t0.get_inflation_curve(source_id).is_ok()
        && market_t1.get_inflation_curve(source_id).is_ok();
    let has_index = market_t0.get_inflation_index(source_id).is_ok()
        && market_t1.get_inflation_index(source_id).is_ok();
    match (has_curve, has_index) {
        (true, true) => Ok(
            measure_inflation_curve_shift(source_id, market_t0, market_t1)?
                + measure_inflation_index_shift(source_id, market_t0, market_t1)?,
        ),
        (_, false) => measure_inflation_curve_shift(source_id, market_t0, market_t1),
        (false, true) => measure_inflation_index_shift(source_id, market_t0, market_t1),
    }
}

// -----------------------------------------------------------------------------
// Surface Shift Measurements
// -----------------------------------------------------------------------------

/// Measure volatility surface shift (percentage points).
///
/// Measures the change in implied volatility levels. Can measure at a specific
/// point or sample across the surface for average shift.
///
/// # Arguments
///
/// * `surface_id` - Volatility surface identifier
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `reference_expiry` - Optional expiry to measure (defaults to 1Y ATM)
/// * `reference_strike` - Optional strike to measure (defaults to ATM)
///
/// # Returns
///
/// Average volatility shift in percentage points (e.g., 2.0 = +2% vol).
///
/// # Errors
///
/// Returns error if surface not found in either market.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_core::market_data::diff::measure_vol_surface_shift;
/// # use finstack_quant_core::market_data::context::MarketContext;
/// # use finstack_quant_core::types::CurveId;
/// # fn example(market_t0: MarketContext, market_t1: MarketContext) -> finstack_quant_core::Result<()> {
/// // Measure 1Y ATM vol shift
/// let vol_shift = measure_vol_surface_shift(
///     &CurveId::from("SPX-VOL"),
///     &market_t0,
///     &market_t1,
///     Some(1.0),  // 1Y expiry
///     Some(1.0),  // ATM (100%)
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn measure_vol_surface_shift(
    surface_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    reference_expiry: Option<f64>,
    reference_strike: Option<f64>,
) -> Result<f64> {
    let surface_t0 = market_t0.get_surface(&surface_id)?;
    let surface_t1 = market_t1.get_surface(&surface_id)?;

    // If specific point requested, measure there
    if let (Some(expiry), Some(strike)) = (reference_expiry, reference_strike) {
        let vol_t0 = surface_t0.value_checked(expiry, strike)?;
        let vol_t1 = surface_t1.value_checked(expiry, strike)?;

        // Shift in percentage points
        return Ok((vol_t1 - vol_t0) * 100.0);
    }

    // Otherwise, sample across surface at standard points
    let expiries = surface_t0.expiries();
    let strikes = surface_t0.strikes();

    let mut total_shift = 0.0;
    let mut sample_count = 0;

    // Sample at available expiries and middle strike (approximately ATM)
    for &expiry in expiries {
        if expiry <= 0.0 {
            continue;
        }

        // Use middle strike as ATM approximation
        if strikes.is_empty() {
            continue;
        }
        let mid_idx = strikes.len() / 2;
        let strike = strikes[mid_idx];

        let vol_t0 = surface_t0.value_checked(expiry, strike)?;
        let vol_t1 = surface_t1.value_checked(expiry, strike)?;

        let shift_pct = (vol_t1 - vol_t0) * 100.0;

        total_shift += shift_pct;
        sample_count += 1;
    }

    if sample_count == 0 {
        return Ok(0.0);
    }

    Ok(total_shift / sample_count as f64)
}

// -----------------------------------------------------------------------------
// FX and Scalar Shift Measurements
// -----------------------------------------------------------------------------

/// Measure FX spot rate shift (percentage change).
///
/// Calculates the percentage change in FX rate between two markets.
///
/// # Arguments
///
/// * `base_ccy` - Base currency
/// * `quote_ccy` - Quote currency
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
///
/// # Returns
///
/// Percentage change in FX rate: (rate_t1 / rate_t0 - 1) * 100
///
/// # Errors
///
/// Returns error if FX matrix not available in either market or conversion fails.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_core::market_data::diff::measure_fx_shift;
/// # use finstack_quant_core::market_data::context::MarketContext;
/// # use finstack_quant_core::currency::Currency;
/// # use time::macros::date;
/// # fn example(market_t0: MarketContext, market_t1: MarketContext) -> finstack_quant_core::Result<()> {
/// let date_t0 = date!(2024 - 01 - 01);
/// let date_t1 = date!(2024 - 01 - 02);
/// let fx_shift = measure_fx_shift(
///     Currency::USD,
///     Currency::EUR,
///     &market_t0,
///     &market_t1,
///     date_t0,
///     date_t1,
/// )?;
///
/// println!("USD/EUR moved {}%", fx_shift);
/// # Ok(())
/// # }
/// ```
pub fn measure_fx_shift(
    base_ccy: Currency,
    quote_ccy: Currency,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<f64> {
    use crate::money::fx::FxQuery;

    // Get FX matrices via the shared helper on MarketContext.
    let fx_t0 = market_t0.fx_required()?;
    let fx_t1 = market_t1.fx_required()?;

    // Get rates using FxQuery with the provided valuation dates
    let query_t0 = FxQuery::new(base_ccy, quote_ccy, as_of_t0);
    let query_t1 = FxQuery::new(base_ccy, quote_ccy, as_of_t1);
    let rate_t0 = fx_t0.rate(query_t0)?.rate;
    let rate_t1 = fx_t1.rate(query_t1)?.rate;

    // Guard against division by zero
    if rate_t0 == 0.0 {
        return Err(crate::Error::Validation(format!(
            "Cannot compute FX shift: rate_t0 is zero for {}/{}",
            base_ccy, quote_ccy
        )));
    }

    // Percentage change
    let pct_change = (rate_t1 / rate_t0 - 1.0) * 100.0;

    Ok(pct_change)
}

/// Measure market scalar shift (percentage change).
///
/// Measures the change in market scalar values (equity prices, commodities, etc.).
///
/// # Arguments
///
/// * `scalar_id` - Market scalar identifier
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
///
/// # Returns
///
/// Percentage change in scalar value.
///
/// # Errors
///
/// Returns error if scalar not found in either market.
pub fn measure_scalar_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    use crate::market_data::scalars::MarketScalar;

    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;

    // Extract numeric values from enum
    let value_t0 = match scalar_t0 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    let value_t1 = match scalar_t1 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    // Guard against division by zero
    if value_t0.abs() < 1e-15 {
        return Err(crate::Error::Validation(format!(
            "Cannot compute percentage shift: baseline value is zero for scalar '{}'",
            scalar_id.as_ref()
        )));
    }

    // Percentage change
    let pct_change = (value_t1 / value_t0 - 1.0) * 100.0;

    if !pct_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite percentage shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }

    Ok(pct_change)
}

/// Compute the **absolute** change in a market scalar between two market states.
///
/// Unlike [`measure_scalar_shift`], which returns a percentage change, this
/// returns `value_t1 - value_t0` in the scalar's native units. Use this when
/// applying a sensitivity defined per unit of underlying move — e.g. an option
/// delta `dPV/dS` or a dividend sensitivity `dPV/d(dividend)` — where
/// multiplying by a percentage shift would introduce a `100 / level` scaling
/// error.
///
/// # Errors
///
/// Returns `Err` if either market lacks the scalar or the computed shift is
/// non-finite.
pub fn measure_scalar_absolute_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    use crate::market_data::scalars::MarketScalar;

    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;

    let value_t0 = match scalar_t0 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let value_t1 = match scalar_t1 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    let abs_change = value_t1 - value_t0;
    if !abs_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite absolute shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }
    Ok(abs_change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::Date;
    use crate::market_data::scalars::InflationIndex;
    use crate::market_data::term_structures::{DiscountCurve, HazardCurve, InflationCurve};
    use crate::math::interp::InterpStyle;
    use time::Month;

    fn sample_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
    }

    #[test]
    fn inflation_index_shift_measures_newly_published_print() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let prior_end = Date::from_calendar_date(2025, Month::December, 1).expect("date");
        let new_end = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let index_t0 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0)],
            Currency::USD,
        )
        .expect("t0 index");
        let index_t1 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0), (new_end, 112.0)],
            Currency::USD,
        )
        .expect("t1 index");
        let market_t0 = MarketContext::new().insert_inflation_index("US-CPI", index_t0);
        let market_t1 = MarketContext::new().insert_inflation_index("US-CPI", index_t1);

        let shift = measure_inflation_source_shift("US-CPI", &market_t0, &market_t1)
            .expect("published-print shift");
        assert!(
            shift > 0.0,
            "a higher newly published CPI print must be measured"
        );
    }

    #[test]
    fn hybrid_inflation_source_includes_discrete_print_shift() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let prior_end = Date::from_calendar_date(2025, Month::December, 1).expect("date");
        let new_end = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let curve = || {
            InflationCurve::builder("US-CPI")
                .base_date(start)
                .base_cpi(100.0)
                .knots([(0.0, 100.0), (5.0, 110.0)])
                .build()
                .expect("inflation curve")
        };
        let index_t0 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0)],
            Currency::USD,
        )
        .expect("t0 index");
        let index_t1 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0), (new_end, 112.0)],
            Currency::USD,
        )
        .expect("t1 index");
        let market_t0 = MarketContext::new()
            .insert(curve())
            .insert_inflation_index("US-CPI", index_t0);
        let market_t1 = MarketContext::new()
            .insert(curve())
            .insert_inflation_index("US-CPI", index_t1);

        let index_shift =
            measure_inflation_index_shift("US-CPI", &market_t0, &market_t1).expect("index shift");
        let source_shift =
            measure_inflation_source_shift("US-CPI", &market_t0, &market_t1).expect("hybrid shift");
        assert!((source_shift - index_shift).abs() < 1e-12);
    }

    #[test]
    fn test_par_spread_shift_is_hazard_shift_scaled_by_lgd() {
        let base_date = sample_date();
        let curve_t0 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("hazard curve t0 should build");
        let curve_t1 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.0125), (5.0, 0.0225), (10.0, 0.0275)])
            .build()
            .expect("hazard curve t1 should build");
        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let hazard_shift = measure_hazard_curve_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("hazard shift");
        let par_shift = measure_par_spread_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("par-spread shift");

        // With no stored par quotes, `cds_quote_bp` falls back to λ·(1−R)·1e4,
        // so the par-spread move is the hazard-rate move scaled by LGD = 1−R.
        assert!(
            (hazard_shift - 25.0).abs() < 1.0,
            "hazard shift ~25bp, got {hazard_shift}"
        );
        assert!(
            (par_shift - 15.0).abs() < 1.0,
            "par-spread shift ~15bp (25 × 0.6), got {par_shift}"
        );

        let per_tenor = measure_per_tenor_par_spread_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            &[1.0, 5.0, 10.0],
        )
        .expect("per-tenor par-spread shift");
        assert_eq!(per_tenor.len(), 3, "one entry per requested tenor");
        for s in per_tenor {
            assert!(
                (s - 15.0).abs() < 1.0,
                "per-tenor par-spread move ~15bp, got {s}"
            );
        }
    }

    #[test]
    fn test_credit_curve_shift_falls_back_to_discount_zero_rate() {
        // A credit curve modelled as a `DiscountCurve` (e.g. a convertible's
        // risky discount curve) has no par CDS quote — `measure_credit_curve_shift`
        // must fall back to the zero-rate shift instead of erroring.
        let base_date = sample_date();
        let curve = |rate: f64| {
            DiscountCurve::builder("USD-CREDIT")
                .base_date(base_date)
                .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
                .interp(InterpStyle::LogLinear)
                .build()
                .expect("discount curve should build")
        };
        // +40bp move in the risky discount curve's zero rate.
        let market_t0 = MarketContext::new().insert(curve(0.05));
        let market_t1 = MarketContext::new().insert(curve(0.054));

        let shift = measure_credit_curve_shift(
            "USD-CREDIT",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("credit-curve shift should fall back to discount measurement");
        assert!(
            (shift - 40.0).abs() < 1.0,
            "expected ~40bp zero-rate shift, got {shift}"
        );

        let per_tenor = measure_per_tenor_credit_curve_shift(
            "USD-CREDIT",
            &market_t0,
            &market_t1,
            &[1.0, 5.0, 10.0],
        )
        .expect("per-tenor credit-curve shift should fall back to discount measurement");
        assert_eq!(per_tenor.len(), 3, "one entry per requested tenor");
        for s in per_tenor {
            assert!(
                (s - 40.0).abs() < 1.0,
                "per-tenor zero-rate shift ~40bp, got {s}"
            );
        }
    }

    #[test]
    fn test_credit_curve_shift_uses_par_spread_for_hazard_curve() {
        // For a `HazardCurve`, `measure_credit_curve_shift` must use the par
        // CDS spread move (== hazard shift × LGD), matching `measure_par_spread_shift`.
        let base_date = sample_date();
        let curve_t0 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("hazard curve t0");
        let curve_t1 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.0125), (5.0, 0.0225), (10.0, 0.0275)])
            .build()
            .expect("hazard curve t1");
        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let credit_shift = measure_credit_curve_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("credit shift");
        let par_shift = measure_par_spread_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("par-spread shift");
        assert_eq!(
            credit_shift, par_shift,
            "hazard-curve credit shift must equal the par-spread shift"
        );
    }

    #[test]
    fn average_shift_uses_relative_equality_for_large_magnitudes() {
        let shift = measure_average_shift(&[1.0], 10_000.0, |_| 1.0e9, |_| 1.0e9 + 1.0e-6);

        assert_eq!(shift, 0.0);
    }
}
