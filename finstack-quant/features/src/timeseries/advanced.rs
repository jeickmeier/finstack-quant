//! Advanced rolling time-series helpers.

use crate::types::{f64_param, finite, mean, quantile_cont, sample_std, scaled_centered};
use finstack_quant_core::{Error, Result};
use serde_json::Value;

pub(super) fn drawdown(
    values: &[Option<f64>],
    indices: &[usize],
    output: &mut [Option<f64>],
) -> Result<()> {
    let mut peak: Option<f64> = None;
    for &idx in indices {
        output[idx] = match finite(values[idx]) {
            Some(value) if value > 0.0 => {
                let peak_value = peak.map_or(value, |prev| prev.max(value));
                peak = Some(peak_value);
                Some(value / peak_value - 1.0)
            }
            _ => None,
        };
    }
    Ok(())
}

/// Current row's normalized exponential-decay weight over finite observations.
pub(super) fn exponential_decay_weights(
    values: &[Option<f64>],
    indices: &[usize],
    window: usize,
    decay: f64,
    output: &mut [Option<f64>],
) -> Result<()> {
    for (pos, &idx) in indices.iter().enumerate() {
        if finite(values[idx]).is_none() {
            continue;
        }
        let start = pos.saturating_sub(window - 1);
        let finite_count = indices[start..=pos]
            .iter()
            .filter(|window_idx| finite(values[**window_idx]).is_some())
            .count();
        let denominator = (0..finite_count)
            .map(|age| decay.powi(age as i32))
            .sum::<f64>();
        output[idx] = Some(1.0 / denominator);
    }
    Ok(())
}

pub(super) fn probability_param(params: Option<&Value>, key: &str, default: f64) -> Result<f64> {
    let value = f64_param(params, key, default)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(Error::Validation(format!(
            "panel transform parameter '{key}' must satisfy 0 <= {key} <= 1"
        )));
    }
    Ok(value)
}

pub(super) fn rolling_rank_value(current: Option<f64>, sample: &mut [f64]) -> Option<f64> {
    let current = current?;
    sample.sort_by(f64::total_cmp);
    if sample.len() == 1 {
        return Some(0.0);
    }
    let pos = sample
        .iter()
        .position(|value| value.total_cmp(&current) == std::cmp::Ordering::Equal)?;
    Some(pos as f64 / (sample.len() - 1) as f64)
}

pub(super) fn skewness(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 3 {
        return None;
    }
    let (_, centered) = scaled_centered(values);
    let (sum_sq, sum_cubed) = centered
        .iter()
        .fold((0.0, 0.0), |(sum_sq, sum_cubed), value| {
            (sum_sq + value * value, sum_cubed + value * value * value)
        });
    let sample_var = sum_sq / (n - 1) as f64;
    if sample_var <= 0.0 {
        return Some(0.0);
    }
    let sample_std = sample_var.sqrt();
    let n = n as f64;
    Some((n / ((n - 1.0) * (n - 2.0))) * sum_cubed / sample_std.powi(3))
}

pub(super) fn excess_kurtosis(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 4 {
        return None;
    }
    let (_, centered) = scaled_centered(values);
    let (sum_sq, sum_fourth) = centered
        .iter()
        .fold((0.0, 0.0), |(sum_sq, sum_fourth), value| {
            let squared = value * value;
            (sum_sq + squared, sum_fourth + squared * squared)
        });
    let sample_var = sum_sq / (n - 1) as f64;
    if sample_var <= 0.0 {
        return Some(0.0);
    }
    let n = n as f64;
    let g2_leading = n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0));
    let g2_correction = 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0));
    Some(g2_leading * sum_fourth / sample_var.powi(2) - g2_correction)
}

pub(super) fn rolling_slope(values: &[Option<f64>], indices: &[usize]) -> Option<f64> {
    let pairs: Vec<_> = indices
        .iter()
        .enumerate()
        .filter_map(|(pos, &idx)| finite(values[idx]).map(|value| (pos as f64, value)))
        .collect();
    if pairs.len() < 2 {
        return None;
    }
    let x_mean = pairs.iter().map(|(pos, _)| pos).sum::<f64>() / pairs.len() as f64;
    let (scale, centered) =
        scaled_centered(&pairs.iter().map(|(_, value)| *value).collect::<Vec<_>>());
    let (cov, var) =
        pairs
            .iter()
            .zip(centered)
            .fold((0.0, 0.0), |(cov, var), ((pos, _), value)| {
                let x = pos - x_mean;
                (cov + x * value, var + x * x)
            });
    if var <= 0.0 {
        Some(0.0)
    } else {
        Some((cov / var) * scale)
    }
}

pub(super) fn rolling_sharpe(values: &[f64], risk_free: f64) -> Option<f64> {
    let mean = mean(values)?;
    match sample_std(values) {
        Some(std) if std > 0.0 => Some((mean - risk_free) / std),
        Some(_) => Some(0.0),
        None => None,
    }
}

pub(super) fn hampel_value(
    current: Option<f64>,
    sample: &mut [f64],
    threshold: f64,
) -> Option<f64> {
    let current = current?;
    sample.sort_by(f64::total_cmp);
    let median = quantile_cont(sample, 0.5)?;
    let scale = sample.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if scale <= 0.0 {
        return Some(current);
    }
    let mut deviations = sample
        .iter()
        .map(|value| (*value / scale - median / scale).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let mad = quantile_cont(&deviations, 0.5)?;
    let scaled_mad = crate::types::MAD_NORMAL_CONSISTENCY * mad;
    if (current / scale - median / scale).abs() > threshold * scaled_mad {
        Some(median)
    } else {
        Some(current)
    }
}
