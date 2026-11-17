//! Floating rate projection for the cashflow builder and its consumers.
//!
//! Term and overnight projection, fixing cutoffs, and the gearing/spread/bound
//! arithmetic used by the schedule builder: bonds, term loans, structured
//! coupons and the bond LSMC pricer. Compounded overnight swap-style coupons
//! (IRS, basis, cross-currency, futures, cap/floor, TRS, revolver) project
//! through a separate engine, `finstack_quant_valuations_core::instruments::pricing::overnight`;
//! mirror overnight convention changes in both. See [`super::overnight`].
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
//! - [`project_floating_rate`]: all-in coupon rate from a resolved forward curve and params
//! - [`project_index_rate`] / [`project_overnight_rate`]: the raw term or overnight index rate
//! - [`observed_index_fixing`]: the realized term or overnight fixing at a valuation cutoff
//! - [`calculate_floating_rate`]: gearing, spread, floors and caps applied to an index rate
//! - [`apply_bp_bounds`]: basis-point floor and cap on a decimal rate
//!
//! ## Formulas
//!
//! ### Gearing Includes Spread (Default)
//! `rate = cap( max( all_in_floor, gearing * ( max(index, floor) + spread ) ) )`
//!
//! ### Gearing Excludes Spread (Affine Model)
//! `rate = cap( max( all_in_floor, (gearing * max(index, floor)) + spread ) )`

use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::fixings::require_fixing_value_exact;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;

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
    /// Returns `Error::Validation` when any numeric input is non-finite,
    /// gearing is non-positive, or a floor exceeds its paired cap.
    pub fn validate(&self) -> Result<()> {
        if !self.spread_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "floating rate spread_bp must be finite".to_string(),
            ));
        }

        if !self.gearing.is_finite() || self.gearing <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "floating rate gearing must be finite and positive".to_string(),
            ));
        }

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
                return Err(finstack_quant_core::Error::Validation(
                    "floating rate caps and floors must be finite".to_string(),
                ));
            }
        }

        if let (Some(floor), Some(cap)) = (self.index_floor_bp, self.index_cap_bp)
            && floor > cap
        {
            return Err(finstack_quant_core::Error::Validation(
                "index floor must not exceed index cap".to_string(),
            ));
        }

        if let (Some(floor), Some(cap)) = (self.all_in_floor_bp, self.all_in_cap_bp)
            && floor > cap
        {
            return Err(finstack_quant_core::Error::Validation(
                "all-in floor must not exceed all-in cap".to_string(),
            ));
        }

        Ok(())
    }
}

impl TryFrom<&crate::builder::specs::FloatingRateSpec> for FloatingRateParams {
    type Error = finstack_quant_core::Error;

    /// Canonical conversion from the serde-level `FloatingRateSpec` (Decimal) to
    /// the projection-level `FloatingRateParams` (f64).
    ///
    /// All numeric fields — including optional floor/cap constraints — error
    /// on `Decimal → f64` conversion failure; a silently dropped constraint
    /// would change coupon economics without notice.
    fn try_from(spec: &crate::builder::specs::FloatingRateSpec) -> Result<Self> {
        spec.validate()?;
        let spread_bp = spec.spread_bp.as_f64();
        let gearing = spec.gearing.as_f64();

        let params = FloatingRateParams {
            spread_bp,
            gearing,
            gearing_includes_spread: spec.gearing_includes_spread,
            index_floor_bp: spec.index_floor_bp.map(|d| d.as_f64()),
            index_cap_bp: spec.index_cap_bp.map(|d| d.as_f64()),
            all_in_floor_bp: spec.all_in_floor_bp.map(|d| d.as_f64()),
            all_in_cap_bp: spec.all_in_cap_bp.map(|d| d.as_f64()),
        };
        params.validate()?;
        Ok(params)
    }
}

/// All-in floating rate after index/all-in floors and caps, gearing, and spread.
///
/// Used for both curve-projected index rates and fallback scenarios (`index_rate = 0`).
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
/// use finstack_quant_cashflows::builder::{calculate_floating_rate, FloatingRateParams};
///
/// let params = FloatingRateParams {
///     spread_bp: 200.0, // 200 bp spread
///     ..Default::default()
/// };
/// let rate = calculate_floating_rate(0.03, &params); // 3% index + 2% spread = 5%
/// assert!((rate - 0.05).abs() < 0.0001);
/// ```
pub fn calculate_floating_rate(index_rate: f64, params: &FloatingRateParams) -> f64 {
    let eff_index = apply_bp_bounds(index_rate, params.index_floor_bp, params.index_cap_bp);
    let rate = if params.gearing_includes_spread {
        (eff_index + params.spread_bp * 1e-4) * params.gearing
    } else {
        (eff_index * params.gearing) + params.spread_bp * 1e-4
    };
    apply_bp_bounds(rate, params.all_in_floor_bp, params.all_in_cap_bp)
}

/// Clamp a decimal rate to optional floor and cap quoted in basis points.
///
/// The floor applies first, then the cap, so a cap below the floor wins
/// (validated specs reject that combination).
///
/// # Arguments
///
/// * `rate` - Decimal rate (`0.05` = 5%).
/// * `floor_bp` - Optional lower bound in basis points.
/// * `cap_bp` - Optional upper bound in basis points.
#[must_use]
pub fn apply_bp_bounds(rate: f64, floor_bp: Option<f64>, cap_bp: Option<f64>) -> f64 {
    let floored = floor_bp.map_or(rate, |floor| rate.max(floor * 1e-4));
    cap_bp.map_or(floored, |cap| floored.min(cap * 1e-4))
}

/// Project the all-in floating rate from a resolved forward curve and coupon parameters.
///
/// Looks up the curve-basis term-index rate at `reset_date`, converts its
/// annualization to the contractual accrual fraction, then applies
/// [`calculate_floating_rate`]. Spreads and rate bounds use the contractual
/// convention and are not rescaled.
///
/// # Arguments
///
/// * `reset_date` - Rate fixing date used to locate the term-index forward on
///   `fwd`.
/// * `fwd` - Resolved forward curve supplying the underlying index rate at the
///   reset date.
/// * `params` - Floating-rate adjustments: spread and gearing plus index and
///   all-in floors or caps, quoted in the documented parameter units.
/// * `accrual_start` - Start of the full contractual coupon period whose basis
///   annualizes the projected index; may precede the curve base for a future
///   reset inside an already-running coupon.
/// * `accrual_end` - Exclusive end of that coupon period, after `accrual_start`.
/// * `accrual_year_fraction` - Finite positive contractual coupon accrual,
///   including any frequency, reference-period, stub, or termination context.
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
/// - `reset_date` is strictly before the curve base date. A strictly-past
///   observation is a realized historical fixing that the curve cannot
///   supply; this function projects only. The emission layer resolves
///   seasoned resets from a `MarketContext` `ScalarTimeSeries` with id
///   `FIXING:{forward_curve_id}` *before* calling this function, and routes this
///   error through the spec's
///   [`crate::builder::FloatingRateFallback`] policy (see its doc for when
///   that applies). A reset exactly on the curve base date (T+0) is projected
///   from `t = 0`.
/// - the accrual window or fraction is invalid, or a required curve-time
///   conversion fails
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - `docs/REFERENCES.md#hull-options-futures`
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
/// use finstack_quant_core::market_data::term_structures::ForwardCurve;
/// use finstack_quant_core::math::Compounding;
/// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
/// use finstack_quant_cashflows::builder::{project_floating_rate, FloatingRateParams};
/// use time::Month;
///
/// let reset = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
/// let period_end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
///
/// let fwd = ForwardCurve::from_zero_rates(
///     "USD-SOFR-3M",
///     reset,
///     &[(0.0, 0.03), (1.0, 0.04)],
///     Compounding::Continuous,
///     DayCount::Act360,
///     InterpStyle::LogLinear,
///     ExtrapolationPolicy::Extend,
/// )?;
///
/// let params = FloatingRateParams {
///     spread_bp: 200.0, // SOFR + 200 bp
///     ..Default::default()
/// };
/// let accrual = DayCount::Act360.year_fraction(reset, period_end, DayCountContext::default())?;
/// let rate = project_floating_rate(reset, &fwd, &params, reset, period_end, accrual)?;
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn project_floating_rate(
    reset_date: Date,
    fwd: &ForwardCurve,
    params: &FloatingRateParams,
    accrual_start: Date,
    accrual_end: Date,
    accrual_year_fraction: f64,
) -> Result<f64> {
    params.validate()?;
    let index_rate = project_index_rate(
        reset_date,
        fwd,
        accrual_start,
        accrual_end,
        accrual_year_fraction,
    )?;
    Ok(calculate_floating_rate(index_rate, params))
}

/// Recorded term or overnight index fixing at a valuation cutoff.
///
/// A fixing is required before `as_of`. On `as_of`, a published exact-date
/// fixing applies when present; otherwise `None` asks the caller to project.
/// Dates after `as_of` always return `None`, even if the series contains a
/// future observation. The forward curve's base date does not set this cutoff.
///
/// # Arguments
///
/// * `observation_date` - Contractual index publication date (term reset or
///   overnight observation), without carrying earlier observations forward.
/// * `fixings` - Optional recorded `FIXING:{forward_curve_id}` series of decimal
///   annual rates. Missing exact historical observations are errors.
/// * `forward_curve_id` - Index identifier used for canonical-series guidance
///   and error diagnostics; this function does not look up a forward curve.
/// * `as_of` - Valuation date separating realized from projected observations.
///
/// # Errors
///
/// Returns a validation error when an observation before `as_of` has no
/// exact fixing, whether the series is absent or incomplete.
pub fn observed_index_fixing(
    observation_date: Date,
    fixings: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
    as_of: Date,
) -> Result<Option<f64>> {
    if observation_date < as_of {
        return require_fixing_value_exact(fixings, forward_curve_id, observation_date)
            .map(Some)
            .map_err(|err| {
                finstack_quant_core::Error::Validation(format!("{err} (valuation date: {as_of})"))
            });
    }
    if observation_date == as_of {
        return Ok(fixings.and_then(|series| series.get_value_exact(observation_date).ok()));
    }
    Ok(None)
}

/// Project the one-day overnight index rate observed on a date.
///
/// The forward curve's one-day simple rate from `observation_date` is
/// re-annualized from the curve clock to the overnight basis
/// (`1 / day_count_basis` years per day). Observation-day carry weights and
/// coupon constraints are applied by the caller.
///
/// # Arguments
///
/// * `fwd` - Overnight forward curve; its day count supplies the curve clock.
/// * `observation_date` - Fixing date on or after the curve base date.
/// * `day_count_basis` - Overnight annualization basis in days (360 or 365).
///
/// # Errors
///
/// Returns a validation error when the one-day interval leaves the supported
/// date range, the curve or index accrual is not finite and positive, or the
/// projection is non-finite; day-count errors propagate.
pub fn project_overnight_rate(
    fwd: &ForwardCurve,
    observation_date: Date,
    day_count_basis: f64,
) -> Result<f64> {
    if observation_date < fwd.base_date() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "overnight observation date {} is before the '{}' curve base date {}",
            observation_date,
            fwd.id(),
            fwd.base_date(),
        )));
    }
    let observation_end = observation_date
        .checked_add(time::Duration::days(1))
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "overnight projection endpoint exceeds the date range for {observation_date}"
            ))
        })?;
    let index_year_fraction = 1.0 / day_count_basis;
    if !day_count_basis.is_finite()
        || day_count_basis <= 0.0
        || !index_year_fraction.is_finite()
        || index_year_fraction <= 0.0
    {
        return Err(finstack_quant_core::Error::Validation(
            "overnight projection requires finite positive curve and index accrual fractions"
                .into(),
        ));
    }
    fwd.rate_between_dates(observation_date, observation_end, index_year_fraction)
}

/// Project a term-index fixing in the contractual accrual convention from projection factors.
///
/// Evaluates `(P(accrual_start) / P(accrual_end) - 1) / accrual_year_fraction`
/// via [`ForwardCurve::rate_between_dates`], after rejecting historical
/// resets before [`ForwardCurve::base_date`].
///
/// # Arguments
///
/// * `reset_date` - Contractual reset-effective date on or after the forward
///   curve base date; past resets require recorded observations instead.
/// * `fwd` - Term-index forward projection factor curve.
/// * `accrual_start` - Full contractual index period start.
/// * `accrual_end` - Exclusive full index period end after `accrual_start`.
/// * `accrual_year_fraction` - Finite positive accrual year fraction in the
///   contractual index convention.
///
/// # Errors
///
/// Returns a validation error for a past reset (`reset_date < fwd.base_date()`),
/// an empty or reversed period, a non-finite or non-positive accrual, or factor
/// evaluation failure.
pub fn project_index_rate(
    reset_date: Date,
    fwd: &ForwardCurve,
    accrual_start: Date,
    accrual_end: Date,
    accrual_year_fraction: f64,
) -> Result<f64> {
    let fwd_base = fwd.base_date();

    // Strictly-past resets are realized fixings; the curve must not clamp them
    // to today's short end. Emission resolves `FIXING:{forward_curve_id}` first.
    if reset_date < fwd_base {
        return Err(finstack_quant_core::Error::Validation(format!(
            "floating-rate observation date {} is before the '{}' curve base date {}; the \
             realized historical fixings are missing — provide a MarketContext \
             ScalarTimeSeries with id 'FIXING:{}', supply a curve based on/before the \
             observation date, or configure a FloatingRateFallback",
            reset_date,
            fwd.id(),
            fwd_base,
            fwd.id(),
        )));
    }
    fwd.rate_between_dates(accrual_start, accrual_end, accrual_year_fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use time::Month;

    #[test]
    fn index_fixing_policy_requires_history_and_ignores_future_observations() {
        let past = Date::from_calendar_date(2025, Month::January, 3).expect("past");
        let as_of = Date::from_calendar_date(2025, Month::January, 6).expect("valuation");
        let future = Date::from_calendar_date(2025, Month::January, 7).expect("future");
        let complete = ScalarTimeSeries::new(
            "FIXING:TEST",
            vec![(past, 0.041), (as_of, 0.042), (future, 0.99)],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("complete series with a future observation");
        let incomplete = ScalarTimeSeries::new(
            "FIXING:TEST",
            vec![(future, 0.99)],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("no historical or valuation-day observation");
        for history in [None, Some(&incomplete), Some(&complete)] {
            let past_result = observed_index_fixing(past, history, "TEST", as_of);
            if history.is_some_and(|series| series.get_value_exact(past).is_ok()) {
                assert_eq!(past_result.expect("recorded fixing"), Some(0.041));
            } else {
                assert!(matches!(
                    past_result,
                    Err(finstack_quant_core::Error::Validation(_))
                ));
            }
            assert_eq!(
                observed_index_fixing(as_of, history, "TEST", as_of).expect("same-day policy"),
                history.and_then(|series| series.get_value_exact(as_of).ok()),
            );
            assert_eq!(
                observed_index_fixing(future, history, "TEST", as_of).expect("future projection"),
                None,
            );
        }
    }

    fn project_floating_rate_from_market(
        reset_date: Date,
        forward_curve_id: &str,
        params: &FloatingRateParams,
        market: &MarketContext,
    ) -> Result<f64> {
        let fwd = market.get_forward(forward_curve_id)?;
        // Fixtures project a quarterly ACT/360 coupon; parameter constraints
        // are quoted on that contractual basis.
        let period_end = reset_date.add_months(3)?;
        let accrual =
            DayCount::Act360.year_fraction(reset_date, period_end, DayCountContext::default())?;
        project_floating_rate(
            reset_date,
            fwd.as_ref(),
            params,
            reset_date,
            period_end,
            accrual,
        )
    }

    fn create_test_market(base_date: Date) -> MarketContext {
        let fwd_curve = ForwardCurve::from_zero_rates(
            "USD-SOFR-3M",
            base_date,
            &[(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)],
            finstack_quant_core::math::Compounding::Continuous,
            DayCount::Act360,
            finstack_quant_core::math::interp::InterpStyle::LogLinear,
            finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
        )
        .expect("ForwardCurve builder should succeed with valid test data");
        MarketContext::new().insert(fwd_curve)
    }

    #[test]
    fn test_project_floating_rate_no_floor_no_cap() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let market = create_test_market(reset);

        let params = FloatingRateParams {
            spread_bp: 200.0,
            ..Default::default()
        }; // 200 bp
        let rate = project_floating_rate_from_market(reset, "USD-SOFR-3M", &params, &market)
            .expect("Rate projection should succeed in test");

        assert!(rate > 0.04 && rate < 0.06, "Rate should be ~5%: {}", rate);
    }

    #[test]
    fn test_project_floating_rate_with_floor() {
        let reset = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");

        let fwd_curve = ForwardCurve::flat("USD-LIBOR-3M", reset, 0.001, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            index_floor_bp: Some(100.0),
            ..Default::default()
        }; // 100 bp spread, 1% floor
        let rate = project_floating_rate_from_market(reset, "USD-LIBOR-3M", &params, &market)
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

        let fwd_curve = ForwardCurve::flat("USD-LIBOR-3M", reset, 0.08, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 200.0,
            all_in_cap_bp: Some(500.0),
            ..Default::default()
        }; // 200 bp spread, 5% cap
        let rate = project_floating_rate_from_market(reset, "USD-LIBOR-3M", &params, &market)
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

        let fwd_curve = ForwardCurve::flat("TEST-INDEX", reset, 0.0001, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            index_floor_bp: Some(100.0),
            ..Default::default()
        }; // 100 bp spread, 1% floor
        let rate = project_floating_rate_from_market(reset, "TEST-INDEX", &params, &market)
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

        let fwd_curve = ForwardCurve::flat("TEST-INDEX", reset, 0.03, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 2.0,
            all_in_cap_bp: Some(600.0),
            ..Default::default()
        }; // 100 bp spread, 2x gearing, 6% cap
        let rate = project_floating_rate_from_market(reset, "TEST-INDEX", &params, &market)
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

        let fwd_curve = ForwardCurve::flat("TEST-INDEX", reset, 0.02, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");
        let market = MarketContext::new().insert(fwd_curve);

        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 1.5,
            ..Default::default()
        }; // 100 bp spread, 1.5x gearing
        let rate = project_floating_rate_from_market(reset, "TEST-INDEX", &params, &market)
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

        let fwd_curve = ForwardCurve::flat("TEST-INDEX", reset, 0.03, DayCount::Act360)
            .expect("ForwardCurve flat should succeed with valid test data");

        let params = FloatingRateParams {
            spread_bp: 150.0,
            ..Default::default()
        }; // 150 bp
        let period_end = reset.add_months(3).expect("quarter end");
        let accrual = (period_end - reset).whole_days() as f64 / 360.0;
        let rate = project_floating_rate(reset, &fwd_curve, &params, reset, period_end, accrual)
            .expect("Rate projection should succeed in test");

        assert!(
            rate > 0.03 && rate < 0.06,
            "Rate should be reasonable: {}",
            rate
        );
    }

    #[test]
    fn term_projection_projects_factor_growth_over_contractual_period() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let reset = Date::from_calendar_date(2025, Month::April, 1).expect("Valid test date");
        let period_end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid test date");
        let fwd_curve = ForwardCurve::from_zero_rates(
            "TEST-INDEX",
            base,
            &[(0.0, 0.01), (1.0, 0.05)],
            finstack_quant_core::math::Compounding::Continuous,
            DayCount::Act360,
            finstack_quant_core::math::interp::InterpStyle::LogLinear,
            finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
        )
        .expect("ForwardCurve should build with valid test data");
        let params = FloatingRateParams::default();
        let accrual = (period_end - reset).whole_days() as f64 / 360.0;
        let expected = fwd_curve
            .rate_between_dates(reset, period_end, accrual)
            .expect("factor interval rate");

        let projected =
            project_floating_rate(reset, &fwd_curve, &params, reset, period_end, accrual)
                .expect("term projection should succeed");

        assert!((projected - expected).abs() < 1e-14);
    }

    #[test]
    fn term_projection_converts_curve_basis_before_coupon_constraints() {
        let reset = Date::from_calendar_date(2025, Month::April, 2).expect("reset");
        let period_end = Date::from_calendar_date(2025, Month::July, 2).expect("period end");
        let days = (period_end - reset).whole_days() as f64;
        let t_period = days / 365.0;
        let simple_365_cont_equiv = (0.04 * t_period).ln_1p() / t_period;
        let fwd = ForwardCurve::flat("USD-TERM", reset, simple_365_cont_equiv, DayCount::Act365F)
            .expect("curve");
        let accrual = days / 360.0;
        let index = project_index_rate(reset, &fwd, reset, period_end, accrual)
            .expect("contractual index rate");
        assert!((index - 0.04 * 360.0 / 365.0).abs() < 1e-14);

        // The contractual 3.98% floor binds after converting the 4% curve quote.
        let params = FloatingRateParams {
            spread_bp: 100.0,
            gearing: 1.5,
            index_floor_bp: Some(398.0),
            ..Default::default()
        };
        let all_in = project_floating_rate(reset, &fwd, &params, reset, period_end, accrual)
            .expect("floored contractual coupon");
        assert!((all_in - (0.0398 + 0.01) * 1.5).abs() < 1e-14);
    }

    #[test]
    fn term_projection_rejects_invalid_contractual_periods() {
        let reset = Date::from_calendar_date(2025, Month::April, 2).expect("reset");
        let period_end = Date::from_calendar_date(2025, Month::July, 2).expect("period end");
        let fwd = ForwardCurve::flat("USD-TERM", reset, 0.04, DayCount::Act360).expect("curve");
        for accrual in [0.0, -0.25, f64::NAN, f64::INFINITY] {
            assert!(project_index_rate(reset, &fwd, reset, period_end, accrual).is_err());
        }
        assert!(project_index_rate(reset, &fwd, period_end, reset, 0.25).is_err());
        assert!(project_index_rate(reset, &fwd, reset, reset, 0.25).is_err());
    }

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
        // With 100 bp spread and 2x gearing: 100 * (2 - 1) = 100 bp = 1%
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

        let fwd_curve = ForwardCurve::flat("TEST-INDEX", reset, 0.03, DayCount::Act360)
            .expect("ForwardCurve flat should succeed");

        let params = FloatingRateParams {
            all_in_floor_bp: Some(500.0),
            all_in_cap_bp: Some(300.0),
            ..Default::default()
        };

        let period_end = reset.add_months(3).expect("quarter end");
        let accrual = (period_end - reset).whole_days() as f64 / 360.0;
        let result = project_floating_rate(reset, &fwd_curve, &params, reset, period_end, accrual);
        assert!(result.is_err(), "Should fail with contradictory floor/cap");
    }

    #[test]
    fn try_from_floating_rate_spec_round_trips_all_fields() {
        use crate::builder::specs::{
            FloatingRateFallback, FloatingRateSpec, OvernightIndexConstraintApplication,
        };
        use finstack_quant_core::dates::Tenor;
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(200.0),
            gearing: dec!(1.5),
            gearing_includes_spread: false,
            index_floor_bp: Some(dec!(25.0)),
            all_in_floor_bp: Some(dec!(50.0)),
            all_in_cap_bp: Some(dec!(1500.0)),
            index_cap_bp: Some(dec!(1200.0)),
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
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
        use crate::builder::specs::{
            FloatingRateFallback, FloatingRateSpec, OvernightIndexConstraintApplication,
        };
        use finstack_quant_core::dates::Tenor;
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(100.0),
            gearing: dec!(1.0),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
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
        use crate::builder::specs::{
            FloatingRateFallback, FloatingRateSpec, OvernightIndexConstraintApplication,
        };
        use finstack_quant_core::dates::Tenor;
        use rust_decimal_macros::dec;

        let spec = FloatingRateSpec {
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(100.0),
            gearing: dec!(1.0),
            gearing_includes_spread: true,
            index_floor_bp: Some(dec!(200.0)),
            all_in_floor_bp: Some(dec!(600.0)),
            all_in_cap_bp: Some(dec!(500.0)),
            index_cap_bp: Some(dec!(100.0)),
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        };

        assert!(
            FloatingRateParams::try_from(&spec).is_err(),
            "runtime conversion should reject contradictory floating-rate constraints"
        );
    }
}
