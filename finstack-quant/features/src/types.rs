//! Shared transform parameter helpers.

use finstack_quant_core::{Error, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Parse a snake_case operation name through the enum's serde representation.
pub(crate) fn op_from_str<T: DeserializeOwned>(
    op: &str,
    kind: &str,
    accepted: &[String],
) -> Result<T> {
    serde_json::from_value(Value::String(op.to_owned())).map_err(|_| {
        Error::Validation(format!(
            "unsupported {kind} transform op '{op}'; accepted ops: {}",
            accepted.join(", ")
        ))
    })
}

/// Canonical snake_case wire name of a serde-tagged unit enum variant.
pub(crate) fn op_name<T: serde::Serialize>(op: &T) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Reject `params` that are not a JSON object or carry keys the operation
/// does not read, so a misspelt key (`"windows"`) fails instead of silently
/// falling back to the default.
pub(crate) fn reject_unknown_params(
    params: Option<&Value>,
    op: &str,
    allowed: &[&str],
) -> Result<()> {
    let Some(params) = params else {
        return Ok(());
    };
    let Some(object) = params.as_object() else {
        return Err(Error::Validation(format!(
            "panel transform parameters for '{op}' must be a JSON object"
        )));
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            let accepted = if allowed.is_empty() {
                "none".to_string()
            } else {
                allowed.join(", ")
            };
            return Err(Error::Validation(format!(
                "unknown panel transform parameter '{key}' for op '{op}'; accepted parameters: {accepted}"
            )));
        }
    }
    Ok(())
}

/// Φ⁻¹(0.75). Reciprocal of [`MAD_NORMAL_CONSISTENCY`].
pub(crate) const PHI_INV_075: f64 = 0.674_489_750_196_081_7;

/// 1 / Φ⁻¹(0.75) — MAD-to-σ consistency factor under normality.
pub(crate) const MAD_NORMAL_CONSISTENCY: f64 = 1.482_602_218_505_602;

pub(crate) fn finite(value: Option<f64>) -> Option<f64> {
    // Signed zeros are numerically equal, including for total-ordered ranks.
    value
        .filter(|inner| inner.is_finite())
        .map(|v| if v.abs() <= 0.0 { 0.0 } else { v })
}

pub(crate) fn validate_lengths(primary: usize, others: &[(&str, usize)]) -> Result<()> {
    for (name, len) in others {
        if *len != primary {
            return Err(Error::Validation(format!(
                "panel transform length mismatch: values has length {primary}, {name} has length {len}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn usize_param(params: Option<&Value>, key: &str, default: usize) -> Result<usize> {
    match params.and_then(|value| value.get(key)) {
        Some(value) => {
            let raw = value.as_u64().ok_or_else(|| {
                Error::Validation(format!(
                    "panel transform parameter '{key}' must be an integer"
                ))
            })?;
            if raw == 0 {
                return Err(Error::Validation(format!(
                    "panel transform parameter '{key}' must be positive"
                )));
            }
            usize::try_from(raw).map_err(|_| {
                Error::Validation(format!("panel transform parameter '{key}' is too large"))
            })
        }
        None => Ok(default),
    }
}

pub(crate) fn required_f64_param(params: Option<&Value>, key: &str) -> Result<f64> {
    params
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            Error::Validation(format!("panel transform parameter '{key}' must be finite"))
        })
}

pub(crate) fn f64_param(params: Option<&Value>, key: &str, default: f64) -> Result<f64> {
    match params.and_then(|value| value.get(key)) {
        Some(value) => value
            .as_f64()
            .filter(|inner| inner.is_finite())
            .ok_or_else(|| {
                Error::Validation(format!("panel transform parameter '{key}' must be finite"))
            }),
        None => Ok(default),
    }
}

pub(crate) fn bool_param(params: Option<&Value>, key: &str, default: bool) -> Result<bool> {
    match params.and_then(|value| value.get(key)) {
        Some(value) => value.as_bool().ok_or_else(|| {
            Error::Validation(format!(
                "panel transform parameter '{key}' must be a boolean"
            ))
        }),
        None => Ok(default),
    }
}

/// Row-window length and required finite count, independent of input readiness.
pub(crate) fn window_params(params: Option<&Value>) -> Result<(usize, usize)> {
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    if min_periods > window {
        return Err(Error::Validation(
            "min_periods must not exceed window".into(),
        ));
    }
    Ok((window, min_periods))
}

/// Reject numerical failures instead of letting JSON turn them into missing data.
pub(crate) fn validate_output(values: &[Option<f64>]) -> Result<()> {
    if let Some(idx) = values
        .iter()
        .position(|value| value.is_some_and(|v| !v.is_finite()))
    {
        return Err(Error::Validation(format!(
            "non-finite feature result at row {idx}"
        )));
    }
    Ok(())
}

/// Scale before centering so moments do not depend on the input's units.
pub(crate) fn scaled_centered(values: &[f64]) -> (f64, Vec<f64>) {
    let scale = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if scale <= 0.0 {
        return (0.0, vec![0.0; values.len()]);
    }
    let mut centered: Vec<_> = values.iter().map(|value| value / scale).collect();
    let center = finstack_quant_core::math::summation::kahan_sum(centered.iter().copied())
        / values.len() as f64;
    for value in &mut centered {
        *value -= center;
    }
    (scale, centered)
}

pub(crate) fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let scale = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if scale <= 0.0 {
        return Some(0.0);
    }
    let normalized =
        finstack_quant_core::math::summation::kahan_sum(values.iter().map(|value| value / scale))
            / values.len() as f64;
    Some(normalized.clamp(-1.0, 1.0) * scale)
}

/// Return the Type-7 continuous quantile of an ascending, total-ordered slice.
pub(crate) fn quantile_cont(sorted: &[f64], probability: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let pos = probability * (sorted.len() - 1) as f64;
    let lower_idx = pos.floor() as usize;
    let upper_idx = pos.ceil() as usize;
    let weight = pos - lower_idx as f64;
    let lower = sorted[lower_idx];
    let upper = sorted[upper_idx];
    Some((1.0 - weight) * lower + weight * upper)
}

pub(crate) fn sample_std(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    Some(std(values, values.len() - 1))
}

pub(crate) fn population_std(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(std(values, values.len()))
}

fn std(values: &[f64], denominator: usize) -> f64 {
    let (scale, centered) = scaled_centered(values);
    let sum_sq =
        finstack_quant_core::math::summation::kahan_sum(centered.iter().map(|value| value * value));
    (sum_sq / denominator as f64).sqrt() * scale
}

#[cfg(test)]
mod tests {
    use super::{MAD_NORMAL_CONSISTENCY, PHI_INV_075};

    #[test]
    fn normal_consistency_constants_are_exact_reciprocals() {
        let product = PHI_INV_075 * MAD_NORMAL_CONSISTENCY;
        assert!(
            (product - 1.0).abs() < 1e-15,
            "PHI_INV_075 * MAD_NORMAL_CONSISTENCY must be 1.0, got {product}"
        );
    }

    #[test]
    fn phi_inv_075_matches_the_standard_normal_third_quartile() {
        // scipy.stats.norm.ppf(0.75) / Python statistics.NormalDist().inv_cdf(0.75)
        assert!(
            (PHI_INV_075 - 0.674_489_750_196_081_7).abs() < 1e-16,
            "PHI_INV_075 drifted from the published Φ⁻¹(0.75)"
        );
    }
}
