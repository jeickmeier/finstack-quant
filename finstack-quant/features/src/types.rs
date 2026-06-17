//! Shared transform parameter helpers.

use finstack_quant_core::{Error, Result};
use serde_json::Value;

/// Numerical tolerance used for zero-denominator checks.
pub(crate) const ZERO_TOLERANCE: f64 = 1e-12;

pub(crate) fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|inner| inner.is_finite())
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

pub(crate) fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

pub(crate) fn sample_std(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = mean(values)?;
    let variance = values
        .iter()
        .map(|value| {
            let centered = *value - mean;
            centered * centered
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    Some(variance.sqrt())
}

pub(crate) fn population_std(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mean = mean(values)?;
    let variance = values
        .iter()
        .map(|value| {
            let centered = *value - mean;
            centered * centered
        })
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.sqrt())
}
