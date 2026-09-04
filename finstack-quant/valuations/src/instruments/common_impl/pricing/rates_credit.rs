//! Date-origin-safe calibration inputs for the shared rates-credit lattice.
//!
//! Discount and hazard curves may have different base dates and day-count
//! conventions. The lattice must therefore receive conditional market targets
//! evaluated on calendar dates, rather than applying one elapsed-time scalar to
//! both curves. This module owns that conversion for every valuation caller.

use finstack_quant_core::dates::{Date, Duration};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::trees::two_factor_rates_credit::RatesCreditCalibrationTargets;

/// Exact discounted default-probability weight for one constant-forward step.
///
/// If `discount = exp(-(r + s) * dt)` and `survival = exp(-lambda * dt)`,
/// this returns
/// `lambda * (1 - exp(-(r + s + lambda) * dt)) / (r + s + lambda)`.
/// Expressing the calculation in terms of interval factors makes the result
/// independent of the curve day-count convention and gives deterministic and
/// sampled hazard pricing one recovery convention.
///
/// # Arguments
///
/// * `discount` - Conditional risk-free-plus-OAS discount factor over the
///   interval, strictly positive and finite.
/// * `survival` - Conditional survival probability over the interval, in
///   `(0, 1]`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for an invalid factor.
pub(crate) fn continuous_frp_weight(discount: f64, survival: f64) -> Result<f64> {
    if !discount.is_finite() || discount <= 0.0 {
        return Err(Error::Validation(format!(
            "FRP interval discount factor must be positive and finite, got {discount}"
        )));
    }
    if !survival.is_finite() || survival <= 0.0 || survival > 1.0 + 1.0e-12 {
        return Err(Error::Validation(format!(
            "FRP interval survival probability must be in (0, 1], got {survival}"
        )));
    }
    let survival = survival.min(1.0);
    let integrated_hazard = -survival.ln();
    if integrated_hazard == 0.0 {
        return Ok(0.0);
    }
    let total_exponent = -(discount * survival).ln();
    let exponential_average = if total_exponent.abs() < 1.0e-8 {
        1.0 - 0.5 * total_exponent + total_exponent * total_exponent / 6.0
    } else {
        -(-total_exponent).exp_m1() / total_exponent
    };
    Ok(integrated_hazard * exponential_average)
}

/// Build conditional targets on the canonical ACT/365F bond exercise grid.
///
/// Every calendar day from `origin` through `horizon_date` is represented. If
/// `minimum_steps` requests a finer grid, one integer number of equal substeps
/// is placed inside every day so that calendar dates remain exact nodes.
///
/// # Arguments
///
/// * `discount` - Risk-free discount curve used to produce conditional
///   `DF(origin, date)` targets.
/// * `hazard` - Issuer hazard curve used to produce conditional survival
///   targets and the recovery-rate assumption.
/// * `origin` - Valuation or settlement date represented by model time zero.
/// * `horizon_date` - Final adjusted bond payment date represented by the last
///   grid node.
/// * `minimum_steps` - Requested minimum grid resolution. Values below one
///   step per calendar day still produce a daily grid.
///
/// # Errors
///
/// Returns [`Error::Validation`] when the date range is empty, the requested
/// step count is zero, the resulting grid overflows, or a curve target is not
/// positive and finite.
pub(crate) fn build_daily_bond_rates_credit_targets(
    discount: &dyn Discounting,
    hazard: &HazardCurve,
    origin: Date,
    horizon_date: Date,
    minimum_steps: usize,
) -> Result<RatesCreditCalibrationTargets> {
    if minimum_steps == 0 {
        return Err(Error::Validation(
            "rates-credit calibration requires at least one requested step".to_string(),
        ));
    }
    let span_days = (horizon_date - origin).whole_days();
    if span_days <= 0 {
        return Err(Error::Validation(format!(
            "rates-credit bond horizon date {horizon_date} must be after origin {origin}"
        )));
    }
    let days = usize::try_from(span_days).map_err(|_| {
        Error::Validation("rates-credit bond horizon exceeds supported grid size".to_string())
    })?;
    let substeps_per_day = minimum_steps.div_ceil(days).max(1);
    let steps = days.checked_mul(substeps_per_day).ok_or_else(|| {
        Error::Validation("rates-credit bond grid step count overflow".to_string())
    })?;
    let horizon = span_days as f64 / 365.0;
    build_rates_credit_targets(discount, hazard, origin, horizon_date, horizon, steps)
}

/// Build conditional discount and survival targets on a uniform model grid.
///
/// Each grid coordinate is mapped to the same fractional calendar position
/// between `origin` and `horizon_date`. Values between whole dates are
/// log-linearly interpolated. This keeps the two curves on one calendar axis
/// while allowing each curve to apply its own base date and day count.
///
/// # Arguments
///
/// * `discount` - Risk-free discount curve used to produce conditional
///   `DF(origin, date)` targets.
/// * `hazard` - Issuer hazard curve used to produce conditional survival
///   targets and the recovery-rate assumption.
/// * `origin` - Valuation or settlement date represented by model time zero.
/// * `horizon_date` - Calendar date represented by the final lattice step.
/// * `horizon` - Positive model year fraction from `origin` to
///   `horizon_date`, on the consuming valuator's time basis.
/// * `steps` - Positive number of uniform lattice intervals.
///
/// # Errors
///
/// Returns [`Error::Validation`] when the grid is empty or reversed, or when
/// either curve produces a non-positive or non-finite target.
pub(crate) fn build_rates_credit_targets(
    discount: &dyn Discounting,
    hazard: &HazardCurve,
    origin: Date,
    horizon_date: Date,
    horizon: f64,
    steps: usize,
) -> Result<RatesCreditCalibrationTargets> {
    if steps == 0 {
        return Err(Error::Validation(
            "rates-credit calibration requires at least one step".to_string(),
        ));
    }
    if !horizon.is_finite() || horizon <= 0.0 {
        return Err(Error::Validation(format!(
            "rates-credit calibration horizon must be positive and finite, got {horizon}"
        )));
    }
    let span_days = (horizon_date - origin).whole_days();
    if span_days <= 0 {
        return Err(Error::Validation(format!(
            "rates-credit calibration horizon date {horizon_date} must be after origin {origin}"
        )));
    }

    let mut dates = Vec::with_capacity(2 * (steps + 1));
    let mut brackets = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let position = step as f64 * span_days as f64 / steps as f64;
        let lower_days = position.floor() as i64;
        let upper_days = position.ceil() as i64;
        let lower = origin + Duration::days(lower_days);
        let upper = origin + Duration::days(upper_days);
        let weight = position - lower_days as f64;
        let lower_index = dates.len();
        dates.push(lower);
        let upper_index = dates.len();
        dates.push(upper);
        brackets.push((lower_index, upper_index, weight));
    }

    let unconditional_survival = hazard.survival_at_dates(&dates)?;
    let origin_survival = hazard.survival_at_dates(&[origin])?[0];
    validate_positive_target("survival probability at origin", origin_survival)?;

    let mut discount_factors = Vec::with_capacity(steps + 1);
    let mut survival_probabilities = Vec::with_capacity(steps + 1);
    for &(lower_index, upper_index, weight) in &brackets {
        let lower_date = dates[lower_index];
        let upper_date = dates[upper_index];
        let lower_df = discount.df_between_dates(origin, lower_date)?;
        let upper_df = discount.df_between_dates(origin, upper_date)?;
        validate_positive_target("discount factor", lower_df)?;
        validate_positive_target("discount factor", upper_df)?;

        let lower_survival = unconditional_survival[lower_index] / origin_survival;
        let upper_survival = unconditional_survival[upper_index] / origin_survival;
        validate_positive_target("conditional survival probability", lower_survival)?;
        validate_positive_target("conditional survival probability", upper_survival)?;

        discount_factors.push(log_interpolate(lower_df, upper_df, weight));
        survival_probabilities.push(log_interpolate(lower_survival, upper_survival, weight));
    }

    // Pin the origin exactly. Besides being the financial definition of a
    // conditional target, this avoids harmless floating-point noise from a
    // curve implementation leaking into the calibration validator.
    discount_factors[0] = 1.0;
    survival_probabilities[0] = 1.0;

    Ok(RatesCreditCalibrationTargets {
        times: (0..=steps)
            .map(|step| step as f64 * horizon / steps as f64)
            .collect(),
        discount_factors,
        survival_probabilities,
        recovery_rate: hazard.recovery_rate(),
    })
}

#[inline]
fn log_interpolate(lower: f64, upper: f64, weight: f64) -> f64 {
    ((1.0 - weight) * lower.ln() + weight * upper.ln()).exp()
}

fn validate_positive_target(label: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "rates-credit {label} must be positive and finite, got {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use time::macros::date;

    #[test]
    fn targets_are_conditional_on_the_requested_origin() {
        let curve_base = date!(2024 - 01 - 01);
        let origin = date!(2025 - 01 - 01);
        let end = date!(2026 - 01 - 01);
        let discount = DiscountCurve::flat("USD-OIS", curve_base, 0.04).expect("discount curve");
        let hazard = HazardCurve::flat("ACME-HZD", curve_base, 0.03, 0.4).expect("hazard curve");

        let targets =
            build_rates_credit_targets(&discount, &hazard, origin, end, 1.0, 4).expect("targets");

        assert_eq!(targets.discount_factors[0], 1.0);
        assert_eq!(targets.survival_probabilities[0], 1.0);
        let expected_df = discount
            .df_between_dates(origin, end)
            .expect("conditional df");
        let survival = hazard
            .survival_at_dates(&[origin, end])
            .expect("survival dates");
        let expected_survival = survival[1] / survival[0];
        assert!((targets.discount_factors[4] - expected_df).abs() < 1.0e-12);
        assert!((targets.survival_probabilities[4] - expected_survival).abs() < 1.0e-12);
        assert_eq!(targets.recovery_rate, 0.4);
    }

    #[test]
    fn bond_grid_contains_every_calendar_day_and_rounds_resolution_up() {
        let origin = date!(2024 - 02 - 28);
        let end = date!(2024 - 03 - 02);
        let discount = DiscountCurve::flat("USD-OIS", origin, 0.04).expect("discount curve");
        let hazard = HazardCurve::flat("ACME-HZD", origin, 0.03, 0.4).expect("hazard curve");

        let daily = build_daily_bond_rates_credit_targets(&discount, &hazard, origin, end, 2)
            .expect("daily targets");
        assert_eq!(daily.times.len(), 4);
        assert!((daily.times[3] - 3.0 / 365.0).abs() < 1.0e-15);

        let subdaily = build_daily_bond_rates_credit_targets(&discount, &hazard, origin, end, 4)
            .expect("subdaily targets");
        // ceil(4 / 3) = two steps per day, hence six intervals.
        assert_eq!(subdaily.times.len(), 7);
        assert!((subdaily.times[2] - 1.0 / 365.0).abs() < 1.0e-15);
    }

    #[test]
    fn continuous_recovery_weight_matches_closed_form_and_stable_limits() {
        let rate: f64 = -0.015;
        let hazard_rate: f64 = 0.015;
        let dt: f64 = 0.25;
        let discount = (-rate * dt).exp();
        let survival = (-hazard_rate * dt).exp();
        let zero_denominator = continuous_frp_weight(discount, survival).expect("weight");
        assert!((zero_denominator - hazard_rate * dt).abs() < 1.0e-12);

        assert_eq!(continuous_frp_weight(0.99, 1.0).expect("zero hazard"), 0.0);
        let positive =
            continuous_frp_weight((-0.04_f64 * dt).exp(), survival).expect("positive weight");
        let expected =
            hazard_rate * (1.0 - (-(0.04 + hazard_rate) * dt).exp()) / (0.04 + hazard_rate);
        assert!((positive - expected).abs() < 1.0e-12);
    }
}
