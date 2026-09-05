//! Fade-to-target forecast: glide from the base value to a terminal level.
//!
//! The workhorse of financial-company modelling: fade a margin, ratio, or
//! level from its last actual toward a long-run target — NIM normalization,
//! cost-income convergence, provisions reverting to through-the-cycle
//! charge-off rates, ROE fading toward cost of equity, or a level pinned to
//! management guidance (geometric shape = CAGR-to-terminal).

use crate::error::{Error, Result};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Glide from `base_value` to a target level over the forecast horizon.
///
/// Supported shapes (`shape` parameter, default `"linear"`):
///
/// - `"linear"`: `v[t] = base + (target - base) * t / N`, where `N` is the
///   number of forecast periods. Reaches the target exactly at the final
///   period.
/// - `"geometric"`: `v[t] = base * (target / base)^(t / N)` — constant
///   compound growth (CAGR) interpolation. Requires `base_value` and
///   `target` to be non-zero and share the same sign.
/// - `"exponential"`: `v[t] = target + (base - target) * 2^(-t / half_life)`
///   — asymptotic half-life decay toward the target; never reaches it
///   exactly. Requires the `half_life` parameter.
///
/// # Arguments
///
/// * `base_value` - Starting level (typically the last actual value), in the
///   node's own units; must be finite
/// * `forecast_periods` - Periods that require projected values, in forecast
///   order; an empty slice returns an empty map
/// * `params` - JSON parameter map containing `target` (required terminal
///   level in the node's own units), optional `shape` (one of `"linear"`,
///   `"geometric"`, `"exponential"`; default `"linear"`), and `half_life`
///   (positive number of periods for the fade to close half the remaining
///   gap; required for — and only legal with — the exponential shape)
///
/// # Errors
///
/// Returns an error when `target` is missing or non-finite, when
/// `base_value` is non-finite, when `shape` is not one of the supported
/// strings, when `half_life` is missing/non-positive for the exponential
/// shape or supplied for any other shape, or when the geometric shape is
/// requested with a zero or sign-crossing base/target pair.
pub(super) fn fade_to_target(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    let target = params
        .get("target")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'target' parameter for FadeToTarget forecast. \
                 Expected a finite number (the terminal level in the node's own units).",
            )
        })?;
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

    let shape = params
        .get("shape")
        .map(|v| {
            v.as_str().ok_or_else(|| {
                Error::forecast(format!(
                    "FadeToTarget 'shape' must be a string \
                     (\"linear\", \"geometric\", or \"exponential\"), got {v}"
                ))
            })
        })
        .transpose()?
        .unwrap_or("linear");

    let half_life = params.get("half_life");
    if shape != "exponential" && half_life.is_some() {
        return Err(Error::forecast(format!(
            "FadeToTarget 'half_life' is only valid with shape = \"exponential\", \
             but shape is \"{shape}\". Remove 'half_life' or switch the shape."
        )));
    }

    let mut results = IndexMap::new();
    if forecast_periods.is_empty() {
        return Ok(results);
    }
    let n = forecast_periods.len() as f64;

    match shape {
        "linear" => {
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let t = (i + 1) as f64;
                results.insert(*period_id, base_value + (target - base_value) * t / n);
            }
        }
        "geometric" => {
            if base_value == 0.0 || target == 0.0 || (base_value < 0.0) != (target < 0.0) {
                return Err(Error::forecast(format!(
                    "FadeToTarget geometric shape requires base_value and target to be \
                     non-zero and share the same sign, got base_value = {base_value} and \
                     target = {target}. Use shape = \"linear\" for sign-crossing or \
                     zero-anchored fades."
                )));
            }
            let ratio = target / base_value;
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let t = (i + 1) as f64;
                let value = base_value * ratio.powf(t / n);
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "FadeToTarget geometric forecast produced a non-finite value at \
                         period {period_id:?}"
                    )));
                }
                results.insert(*period_id, value);
            }
        }
        "exponential" => {
            let half_life = half_life.and_then(|v| v.as_f64()).ok_or_else(|| {
                Error::forecast(
                    "FadeToTarget exponential shape requires a numeric 'half_life' \
                     parameter (number of periods to close half the remaining gap, \
                     e.g. 4.0 for a one-year half-life in a quarterly model).",
                )
            })?;
            if !half_life.is_finite() || half_life <= 0.0 {
                return Err(Error::forecast(format!(
                    "FadeToTarget 'half_life' must be a positive finite number of \
                     periods, got {half_life}"
                )));
            }
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let t = (i + 1) as f64;
                let value = target + (base_value - target) * (-t / half_life).exp2();
                results.insert(*period_id, value);
            }
        }
        other => {
            return Err(Error::forecast(format!(
                "Unknown FadeToTarget shape '{other}'. \
                 Expected \"linear\", \"geometric\", or \"exponential\"."
            )));
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

    fn params(entries: &[(&str, serde_json::Value)]) -> IndexMap<String, serde_json::Value> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn linear_hits_target_at_last_period() {
        let periods = quarters(4);
        let p = params(&[("target", serde_json::json!(200.0))]);
        let results = fade_to_target(100.0, &periods, &p).expect("linear fade should succeed");
        assert_eq!(results.len(), 4);
        assert!((results[&periods[0]] - 125.0).abs() < 1e-12);
        assert!((results[&periods[1]] - 150.0).abs() < 1e-12);
        assert!((results[&periods[2]] - 175.0).abs() < 1e-12);
        assert!((results[&periods[3]] - 200.0).abs() < 1e-12);
    }

    #[test]
    fn linear_supports_downward_fade() {
        let periods = quarters(2);
        let p = params(&[("target", serde_json::json!(0.02))]);
        let results = fade_to_target(0.04, &periods, &p).expect("downward fade should succeed");
        assert!((results[&periods[0]] - 0.03).abs() < 1e-12);
        assert!((results[&periods[1]] - 0.02).abs() < 1e-12);
    }

    #[test]
    fn geometric_is_cagr_interpolation() {
        let periods = quarters(4);
        let p = params(&[
            ("target", serde_json::json!(200.0)),
            ("shape", serde_json::json!("geometric")),
        ]);
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
            let p = params(&[
                ("target", serde_json::json!(target)),
                ("shape", serde_json::json!("geometric")),
            ]);
            let err = fade_to_target(base, &periods, &p).expect_err("should reject");
            assert!(err.to_string().contains("same sign"), "{err}");
        }
    }

    #[test]
    fn exponential_halves_gap_each_half_life() {
        let periods = quarters(4);
        let p = params(&[
            ("target", serde_json::json!(100.0)),
            ("shape", serde_json::json!("exponential")),
            ("half_life", serde_json::json!(1.0)),
        ]);
        let results = fade_to_target(200.0, &periods, &p).expect("exponential fade");
        assert!((results[&periods[0]] - 150.0).abs() < 1e-12);
        assert!((results[&periods[1]] - 125.0).abs() < 1e-12);
        assert!((results[&periods[2]] - 112.5).abs() < 1e-12);
        assert!((results[&periods[3]] - 106.25).abs() < 1e-12);
    }

    #[test]
    fn exponential_requires_half_life() {
        let periods = quarters(2);
        let p = params(&[
            ("target", serde_json::json!(100.0)),
            ("shape", serde_json::json!("exponential")),
        ]);
        let err = fade_to_target(200.0, &periods, &p).expect_err("missing half_life");
        assert!(err.to_string().contains("half_life"), "{err}");
    }

    #[test]
    fn half_life_rejected_for_other_shapes() {
        let periods = quarters(2);
        let p = params(&[
            ("target", serde_json::json!(100.0)),
            ("half_life", serde_json::json!(2.0)),
        ]);
        let err = fade_to_target(200.0, &periods, &p).expect_err("half_life misuse");
        assert!(err.to_string().contains("only valid"), "{err}");
    }

    #[test]
    fn invalid_half_life_rejected() {
        let periods = quarters(2);
        for hl in [0.0, -1.0, f64::NAN] {
            let p = params(&[
                ("target", serde_json::json!(100.0)),
                ("shape", serde_json::json!("exponential")),
                ("half_life", serde_json::json!(hl)),
            ]);
            assert!(fade_to_target(200.0, &periods, &p).is_err());
        }
    }

    #[test]
    fn unknown_shape_rejected() {
        let periods = quarters(1);
        let p = params(&[
            ("target", serde_json::json!(100.0)),
            ("shape", serde_json::json!("sigmoid")),
        ]);
        let err = fade_to_target(200.0, &periods, &p).expect_err("unknown shape");
        assert!(
            err.to_string().contains("Unknown FadeToTarget shape"),
            "{err}"
        );
    }

    #[test]
    fn missing_target_rejected() {
        let periods = quarters(1);
        let p = params(&[]);
        assert!(fade_to_target(100.0, &periods, &p).is_err());
    }

    #[test]
    fn non_finite_inputs_rejected() {
        let periods = quarters(1);
        let p = params(&[("target", serde_json::json!(100.0))]);
        assert!(fade_to_target(f64::NAN, &periods, &p).is_err());
    }

    #[test]
    fn empty_periods_return_empty_map() {
        let p = params(&[("target", serde_json::json!(100.0))]);
        let results = fade_to_target(50.0, &[], &p).expect("empty periods valid");
        assert!(results.is_empty());
    }
}
