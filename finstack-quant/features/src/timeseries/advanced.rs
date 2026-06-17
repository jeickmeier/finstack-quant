//! Advanced rolling time-series helpers.

use crate::types::{
    f64_param, finite, required_f64_param, sample_std, usize_param, ZERO_TOLERANCE,
};
use finstack_quant_core::{Error, Result};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) enum AdvancedRollingOp {
    Rank,
    Quantile,
    Skew,
    Kurtosis,
    Slope,
    Sharpe,
    Winsorize,
    Hampel,
}

pub(super) fn rolling_advanced(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
    op: AdvancedRollingOp,
) -> Result<()> {
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    let required = match op {
        AdvancedRollingOp::Skew | AdvancedRollingOp::Kurtosis => min_periods.max(3),
        AdvancedRollingOp::Slope | AdvancedRollingOp::Sharpe => min_periods.max(2),
        _ => min_periods,
    };
    for (pos, &idx) in indices.iter().enumerate() {
        let start = pos.saturating_sub(window - 1);
        let mut finite_values = indices[start..=pos]
            .iter()
            .filter_map(|window_idx| finite(values[*window_idx]))
            .collect::<Vec<_>>();
        if finite_values.len() < required {
            continue;
        }
        output[idx] = match op {
            AdvancedRollingOp::Rank => rolling_rank_value(finite(values[idx]), &mut finite_values),
            AdvancedRollingOp::Quantile => {
                let q = probability_param(params, "quantile", 0.5)?;
                finite_values.sort_by(f64::total_cmp);
                quantile_cont(&finite_values, q)
            }
            AdvancedRollingOp::Skew => skewness(&finite_values),
            AdvancedRollingOp::Kurtosis => excess_kurtosis(&finite_values),
            AdvancedRollingOp::Slope => rolling_slope(&finite_values),
            AdvancedRollingOp::Sharpe => rolling_sharpe(&finite_values),
            AdvancedRollingOp::Winsorize => {
                let lower = probability_param(params, "lower", 0.01)?;
                let upper = probability_param(params, "upper", 0.99)?;
                if lower > upper {
                    return Err(Error::Validation(
                        "rolling_winsorize requires 0 <= lower <= upper <= 1".to_string(),
                    ));
                }
                let current = finite(values[idx]);
                finite_values.sort_by(f64::total_cmp);
                match (
                    current,
                    quantile_cont(&finite_values, lower),
                    quantile_cont(&finite_values, upper),
                ) {
                    (Some(current), Some(lower_bound), Some(upper_bound)) => {
                        Some(current.clamp(lower_bound, upper_bound))
                    }
                    _ => None,
                }
            }
            AdvancedRollingOp::Hampel => {
                hampel_value(finite(values[idx]), &mut finite_values, params)?
            }
        };
    }
    Ok(())
}

pub(super) fn drawdown(
    values: &[Option<f64>],
    indices: &[usize],
    output: &mut [Option<f64>],
) -> Result<()> {
    let mut peak = None;
    for &idx in indices {
        output[idx] = match finite(values[idx]) {
            Some(value) if value > ZERO_TOLERANCE => {
                peak = Some(match peak {
                    Some(prev_peak) if prev_peak > value => prev_peak,
                    _ => value,
                });
                peak.map(|peak_value| value / peak_value - 1.0)
            }
            _ => None,
        };
    }
    Ok(())
}

pub(super) fn exponential_decay_weights(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let window = usize_param(params, "window", 1)?;
    let half_life = required_f64_param(params, "half_life")?;
    if half_life <= 0.0 {
        return Err(Error::Validation(
            "panel transform parameter 'half_life' must be positive".to_string(),
        ));
    }
    let decay = (-std::f64::consts::LN_2 / half_life).exp();
    for (pos, &idx) in indices.iter().enumerate() {
        if finite(values[idx]).is_none() {
            continue;
        }
        let start = pos.saturating_sub(window - 1);
        let finite_count = indices[start..=pos]
            .iter()
            .filter(|window_idx| finite(values[**window_idx]).is_some())
            .count();
        if finite_count == 0 {
            continue;
        }
        let denominator = (0..finite_count)
            .map(|age| decay.powi(age as i32))
            .sum::<f64>();
        output[idx] = Some(1.0 / denominator);
    }
    Ok(())
}

fn probability_param(params: Option<&Value>, key: &str, default: f64) -> Result<f64> {
    let value = f64_param(params, key, default)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(Error::Validation(format!(
            "panel transform parameter '{key}' must satisfy 0 <= {key} <= 1"
        )));
    }
    Ok(value)
}

fn rolling_rank_value(current: Option<f64>, sample: &mut [f64]) -> Option<f64> {
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

fn skewness(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let (m2, m3) = values.iter().fold((0.0, 0.0), |(m2, m3), value| {
        let centered = *value - mean;
        (
            m2 + centered * centered,
            m3 + centered * centered * centered,
        )
    });
    let m2 = m2 / values.len() as f64;
    if m2 <= ZERO_TOLERANCE {
        return Some(0.0);
    }
    let m3 = m3 / values.len() as f64;
    Some(m3 / m2.powf(1.5))
}

fn excess_kurtosis(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let (m2, m4) = values.iter().fold((0.0, 0.0), |(m2, m4), value| {
        let centered = *value - mean;
        let squared = centered * centered;
        (m2 + squared, m4 + squared * squared)
    });
    let m2 = m2 / values.len() as f64;
    if m2 <= ZERO_TOLERANCE {
        return Some(0.0);
    }
    let m4 = m4 / values.len() as f64;
    Some(m4 / (m2 * m2) - 3.0)
}

fn rolling_slope(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let x_mean = (values.len() - 1) as f64 / 2.0;
    let y_mean = values.iter().sum::<f64>() / values.len() as f64;
    let (cov, var) = values
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(cov, var), (idx, value)| {
            let x = idx as f64 - x_mean;
            (cov + x * (*value - y_mean), var + x * x)
        });
    if var <= ZERO_TOLERANCE {
        Some(0.0)
    } else {
        Some(cov / var)
    }
}

fn rolling_sharpe(values: &[f64]) -> Option<f64> {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    match sample_std(values) {
        Some(std) if std > ZERO_TOLERANCE => Some(mean / std),
        Some(_) => Some(0.0),
        None => None,
    }
}

fn hampel_value(
    current: Option<f64>,
    sample: &mut [f64],
    params: Option<&Value>,
) -> Result<Option<f64>> {
    let Some(current) = current else {
        return Ok(None);
    };
    let threshold = f64_param(params, "threshold", 3.0)?;
    if threshold < 0.0 {
        return Err(Error::Validation(
            "hampel_filter requires threshold >= 0".to_string(),
        ));
    }
    sample.sort_by(f64::total_cmp);
    let Some(median) = quantile_cont(sample, 0.5) else {
        return Ok(None);
    };
    let mut deviations = sample
        .iter()
        .map(|value| (*value - median).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let Some(mad) = quantile_cont(&deviations, 0.5) else {
        return Ok(None);
    };
    if mad <= ZERO_TOLERANCE {
        return Ok(Some(current));
    }
    let scaled_mad = 1.482_602_218_505_602 * mad;
    if (current - median).abs() > threshold * scaled_mad {
        Ok(Some(median))
    } else {
        Ok(Some(current))
    }
}

fn quantile_cont(sorted: &[f64], probability: f64) -> Option<f64> {
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
    Some(sorted[lower_idx] + weight * (sorted[upper_idx] - sorted[lower_idx]))
}
