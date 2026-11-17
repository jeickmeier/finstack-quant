//! Fade-to-target forecast: glide from the base value to a terminal level.
//!
//! The workhorse of financial-company modelling: fade a margin, ratio, or
//! level from its last actual toward a long-run target — NIM normalization,
//! cost-income convergence, provisions reverting to through-the-cycle
//! charge-off rates, ROE fading toward cost of equity, or a level pinned to
//! management guidance (geometric shape = CAGR-to-terminal).

use crate::error::{Error, Result};
use crate::types::{FadeToTargetParams, ForecastFadeShape};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Glide from `base_value` to a target level over the forecast horizon.
///
/// Supported shapes:
///
/// - linear: `v[t] = base + (target - base) * t / N`, where `N` is the
///   number of forecast periods. Reaches the target exactly at the final
///   period.
/// - geometric: `|v[t]| = exp((1 - t/N) ln|base| + (t/N) ln|target|)` with the
///   endpoints' common sign — constant compound growth (CAGR) interpolation
///   on the logarithms of the magnitudes, which reaches the target exactly at
///   the final period. Requires `base_value` and `target` to be non-zero and
///   share the same sign.
/// - exponential: `v[t] = target + (base - target) * 2^(-t / half_life)`
///   — asymptotic half-life decay toward the target; never reaches it
///   exactly. Requires `half_life`.
///
/// # Arguments
///
/// * `base_value` - Starting level (typically the last actual value), in the
///   node's own units; must be finite
/// * `forecast_periods` - Periods that require projected values, in forecast
///   order; an empty slice returns an empty map
/// * `params` - Finite `target` level in the node's own units, the fade
///   `shape`, and `half_life` (positive number of periods for the fade to
///   close half the remaining gap; required for — and only legal with — the
///   exponential shape)
///
/// # Errors
///
/// Returns an error when `target` or `base_value` is non-finite, when
/// `half_life` is missing/non-positive for the exponential shape or supplied
/// for any other shape, or when the geometric shape is requested with a zero
/// or sign-crossing base/target pair.
pub(super) fn fade_to_target(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &FadeToTargetParams,
) -> Result<IndexMap<PeriodId, f64>> {
    let target = params.target;
    if !target.is_finite() {
        return Err(Error::forecast(format!(
            "FadeToTarget 'target' must be finite, got {target}"
        )));
    }
    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "FadeToTarget forecast requires a finite base_value, got {base_value}"
        )));
    }

    let shape = params.shape;
    if shape != ForecastFadeShape::Exponential && params.half_life.is_some() {
        return Err(Error::forecast(format!(
            "FadeToTarget 'half_life' is only valid with shape = \"exponential\", \
             but shape is {shape:?}. Remove 'half_life' or switch the shape."
        )));
    }

    if shape == ForecastFadeShape::Geometric
        && (base_value == 0.0 || target == 0.0 || (base_value < 0.0) != (target < 0.0))
    {
        return Err(Error::forecast(format!(
            "FadeToTarget geometric shape requires base_value and target to be \
             non-zero and share the same sign, got base_value = {base_value} and \
             target = {target}. Use shape = \"linear\" for sign-crossing or \
             zero-anchored fades."
        )));
    }
    let half_life = match (shape, params.half_life) {
        (ForecastFadeShape::Exponential, Some(half_life))
            if half_life.is_finite() && half_life > 0.0 =>
        {
            half_life
        }
        (ForecastFadeShape::Exponential, Some(half_life)) => {
            return Err(Error::forecast(format!(
                "FadeToTarget 'half_life' must be a positive finite number of \
                 periods, got {half_life}"
            )));
        }
        (ForecastFadeShape::Exponential, None) => {
            return Err(Error::forecast(
                "FadeToTarget exponential shape requires a numeric 'half_life' \
                 parameter (number of periods to close half the remaining gap, \
                 e.g. 4.0 for a one-year half-life in a quarterly model).",
            ));
        }
        _ => 0.0,
    };

    let mut results = IndexMap::new();
    if forecast_periods.is_empty() {
        return Ok(results);
    }
    let n = forecast_periods.len() as f64;

    match shape {
        ForecastFadeShape::Linear => {
            let last = forecast_periods.len() - 1;
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let weight = (i + 1) as f64 / n;
                // Convex weights avoid overflowing target - base for finite
                // endpoints with opposite signs.
                let value = if i == last {
                    target
                } else {
                    (1.0 - weight) * base_value + weight * target
                };
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "FadeToTarget linear forecast produced a non-finite value at period {period_id:?}"
                    )));
                }
                results.insert(*period_id, value);
            }
        }
        ForecastFadeShape::Geometric => {
            // Interpolate the logarithms of the magnitudes: every level lies
            // between the endpoints, so it is representable even when the
            // endpoint ratio overflows or underflows.
            let (log_base, log_target) = (base_value.abs().ln(), target.abs().ln());
            let last = forecast_periods.len() - 1;
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let value = if i == last {
                    target
                } else {
                    let weight = (i + 1) as f64 / n;
                    ((1.0 - weight) * log_base + weight * log_target)
                        .exp()
                        .copysign(target)
                };
                if !value.is_finite() || value == 0.0 {
                    return Err(Error::forecast(format!(
                        "FadeToTarget geometric forecast produced an unrepresentable value at \
                         period {period_id:?}"
                    )));
                }
                results.insert(*period_id, value);
            }
        }
        ForecastFadeShape::Exponential => {
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let t = (i + 1) as f64;
                let weight = (-t / half_life).exp2();
                let value = weight * base_value + (1.0 - weight) * target;
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "FadeToTarget exponential forecast produced a non-finite value at period {period_id:?}"
                    )));
                }
                results.insert(*period_id, value);
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    fn quarters(n: u8) -> Vec<PeriodId> {
        (1..=n)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect()
    }

    fn params(target: f64, shape: ForecastFadeShape, half_life: Option<f64>) -> FadeToTargetParams {
        FadeToTargetParams {
            start: None,
            target,
            shape,
            half_life,
            min: None,
            max: None,
        }
    }

    fn linear(target: f64) -> FadeToTargetParams {
        params(target, ForecastFadeShape::Linear, None)
    }

    #[test]
    fn linear_hits_target_at_last_period() {
        let periods = quarters(4);
        let results =
            fade_to_target(100.0, &periods, &linear(200.0)).expect("linear fade should succeed");
        assert_eq!(results.len(), 4);
        assert!((results[&periods[0]] - 125.0).abs() < 1e-12);
        assert!((results[&periods[1]] - 150.0).abs() < 1e-12);
        assert!((results[&periods[2]] - 175.0).abs() < 1e-12);
        assert!((results[&periods[3]] - 200.0).abs() < 1e-12);
    }

    #[test]
    fn linear_supports_downward_fade() {
        let periods = quarters(2);
        let results =
            fade_to_target(0.04, &periods, &linear(0.02)).expect("downward fade should succeed");
        assert!((results[&periods[0]] - 0.03).abs() < 1e-12);
        assert!((results[&periods[1]] - 0.02).abs() < 1e-12);
    }

    #[test]
    fn geometric_is_cagr_interpolation() {
        let periods = quarters(4);
        let p = params(200.0, ForecastFadeShape::Geometric, None);
        let results = fade_to_target(100.0, &periods, &p).expect("geometric fade should succeed");
        let cagr = 2.0_f64.powf(0.25);
        for (i, pid) in periods.iter().enumerate() {
            let expected = 100.0 * cagr.powi(i as i32 + 1);
            assert!((results[pid] - expected).abs() < 1e-9);
        }
        assert!((results[&periods[3]] - 200.0).abs() < 1e-9);
    }

    #[test]
    fn geometric_rejects_sign_crossing_and_zero() {
        let periods = quarters(2);
        for (base, target) in [(100.0, -50.0), (-100.0, 50.0), (0.0, 50.0), (100.0, 0.0)] {
            let p = params(target, ForecastFadeShape::Geometric, None);
            let err = fade_to_target(base, &periods, &p).expect_err("should reject");
            assert!(err.to_string().contains("same sign"), "{err}");
        }
    }

    #[test]
    fn exponential_halves_gap_each_half_life() {
        let periods = quarters(4);
        let p = params(100.0, ForecastFadeShape::Exponential, Some(1.0));
        let results = fade_to_target(200.0, &periods, &p).expect("exponential fade");
        assert!((results[&periods[0]] - 150.0).abs() < 1e-12);
        assert!((results[&periods[1]] - 125.0).abs() < 1e-12);
        assert!((results[&periods[2]] - 112.5).abs() < 1e-12);
        assert!((results[&periods[3]] - 106.25).abs() < 1e-12);
    }

    #[test]
    fn exponential_requires_half_life() {
        let periods = quarters(2);
        let p = params(100.0, ForecastFadeShape::Exponential, None);
        let err = fade_to_target(200.0, &periods, &p).expect_err("missing half_life");
        assert!(err.to_string().contains("half_life"), "{err}");
    }

    #[test]
    fn half_life_rejected_for_other_shapes() {
        let periods = quarters(2);
        let p = params(100.0, ForecastFadeShape::Linear, Some(2.0));
        let err = fade_to_target(200.0, &periods, &p).expect_err("half_life misuse");
        assert!(err.to_string().contains("only valid"), "{err}");
    }

    #[test]
    fn invalid_half_life_rejected() {
        let periods = quarters(2);
        for hl in [0.0, -1.0, f64::NAN] {
            let p = params(100.0, ForecastFadeShape::Exponential, Some(hl));
            assert!(fade_to_target(200.0, &periods, &p).is_err());
        }
    }

    #[test]
    fn non_finite_inputs_rejected() {
        let periods = quarters(1);
        assert!(fade_to_target(f64::NAN, &periods, &linear(100.0)).is_err());
        assert!(fade_to_target(100.0, &periods, &linear(f64::INFINITY)).is_err());
    }

    #[test]
    fn empty_periods_return_empty_map() {
        let results = fade_to_target(50.0, &[], &linear(100.0)).expect("empty periods valid");
        assert!(results.is_empty());
    }

    /// Every level lies between the endpoints, so the path is representable
    /// even though `target / base` overflows or underflows.
    #[test]
    fn geometric_path_survives_an_unrepresentable_endpoint_ratio() {
        let periods = quarters(4);
        for (base, target) in [(1e-200, 1e200), (1e200, 1e-200), (-1e-200, -1e200)] {
            let p = params(target, ForecastFadeShape::Geometric, None);
            let results = fade_to_target(base, &periods, &p).expect("representable path");
            // Independent reference: the geometric mean of the endpoints is
            // 1 in magnitude, reached at the midpoint.
            assert!((results[&periods[1]].abs() - 1.0).abs() < 1e-12);
            assert_eq!(results[&periods[1]].signum(), target.signum());
            assert_eq!(results[&periods[3]].to_bits(), target.to_bits());
            let quarter_point = results[&periods[0]].abs().log10();
            assert!((quarter_point - base.abs().log10() / 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn linear_and_geometric_return_the_exact_target_in_the_final_slot() {
        let periods = quarters(3);
        for shape in [ForecastFadeShape::Linear, ForecastFadeShape::Geometric] {
            let p = params(0.3, shape, None);
            let results = fade_to_target(0.1, &periods, &p).expect("fade");
            assert_eq!(results[&periods[2]].to_bits(), 0.3_f64.to_bits());
        }
    }

    /// Shape checks run even when there is no period to forecast.
    #[test]
    fn shape_checks_run_for_an_empty_horizon() {
        let geometric = params(-50.0, ForecastFadeShape::Geometric, None);
        assert!(fade_to_target(100.0, &[], &geometric).is_err());
        let exponential = params(50.0, ForecastFadeShape::Exponential, Some(0.0));
        assert!(fade_to_target(100.0, &[], &exponential).is_err());
    }
}
