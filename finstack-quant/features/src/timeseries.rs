//! Backward-looking time-series panel transforms.

mod advanced;

use crate::types::{
    finite, required_f64_param, sample_std, usize_param, validate_lengths, ZERO_TOLERANCE,
};
use advanced::{drawdown, exponential_decay_weights, rolling_advanced, AdvancedRollingOp};
use finstack_quant_core::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

/// Supported backward-looking time-series transform operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeSeriesOp {
    /// Simple return `v_t / v_{t-periods} - 1`.
    Returns,
    /// Log return `ln(v_t / v_{t-periods})`.
    LogReturns,
    /// Difference `v_t - v_{t-periods}`.
    Diff,
    /// Value shifted forward by `periods`.
    Lag,
    /// Mean over the trailing window.
    RollingMean,
    /// Sum over the trailing window.
    RollingSum,
    /// Sample standard deviation over the trailing window.
    RollingStd,
    /// Minimum over the trailing window.
    RollingMin,
    /// Maximum over the trailing window.
    RollingMax,
    /// Z-score of the current value against the trailing window.
    RollingZscore,
    /// Percentile rank of the current value against the trailing window.
    RollingRank,
    /// Quantile over the trailing window.
    RollingQuantile,
    /// Skewness over the trailing window.
    RollingSkew,
    /// Excess kurtosis over the trailing window.
    RollingKurtosis,
    /// Linear trend slope over the trailing window.
    RollingSlope,
    /// Mean divided by sample standard deviation over the trailing window.
    RollingSharpe,
    /// Clamp current values to trailing quantile bounds.
    RollingWinsorize,
    /// Peak-to-trough drawdown from the running maximum.
    Drawdown,
    /// Hampel outlier filter over the trailing window.
    HampelFilter,
    /// Current observation's normalized exponential-decay weight.
    ExponentialDecayWeights,
    /// Exponentially weighted mean using `span`.
    EwmaMean,
    /// Exponentially weighted volatility using `span`.
    EwmaVol,
    /// Exponentially weighted z-score using `span`.
    EwmaZscore,
}

impl TimeSeriesOp {
    /// Return the canonical snake_case operation name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Returns => "returns",
            Self::LogReturns => "log_returns",
            Self::Diff => "diff",
            Self::Lag => "lag",
            Self::RollingMean => "rolling_mean",
            Self::RollingSum => "rolling_sum",
            Self::RollingStd => "rolling_std",
            Self::RollingMin => "rolling_min",
            Self::RollingMax => "rolling_max",
            Self::RollingZscore => "rolling_zscore",
            Self::RollingRank => "rolling_rank",
            Self::RollingQuantile => "rolling_quantile",
            Self::RollingSkew => "rolling_skew",
            Self::RollingKurtosis => "rolling_kurtosis",
            Self::RollingSlope => "rolling_slope",
            Self::RollingSharpe => "rolling_sharpe",
            Self::RollingWinsorize => "rolling_winsorize",
            Self::Drawdown => "drawdown",
            Self::HampelFilter => "hampel_filter",
            Self::ExponentialDecayWeights => "exponential_decay_weights",
            Self::EwmaMean => "ewma_mean",
            Self::EwmaVol => "ewma_vol",
            Self::EwmaZscore => "ewma_zscore",
        }
    }
}

impl FromStr for TimeSeriesOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        match op {
            "returns" => Ok(Self::Returns),
            "log_returns" => Ok(Self::LogReturns),
            "diff" => Ok(Self::Diff),
            "lag" => Ok(Self::Lag),
            "rolling_mean" => Ok(Self::RollingMean),
            "rolling_sum" => Ok(Self::RollingSum),
            "rolling_std" => Ok(Self::RollingStd),
            "rolling_min" => Ok(Self::RollingMin),
            "rolling_max" => Ok(Self::RollingMax),
            "rolling_zscore" => Ok(Self::RollingZscore),
            "rolling_rank" => Ok(Self::RollingRank),
            "rolling_quantile" => Ok(Self::RollingQuantile),
            "rolling_skew" => Ok(Self::RollingSkew),
            "rolling_kurtosis" => Ok(Self::RollingKurtosis),
            "rolling_slope" => Ok(Self::RollingSlope),
            "rolling_sharpe" => Ok(Self::RollingSharpe),
            "rolling_winsorize" => Ok(Self::RollingWinsorize),
            "drawdown" => Ok(Self::Drawdown),
            "hampel_filter" => Ok(Self::HampelFilter),
            "exponential_decay_weights" => Ok(Self::ExponentialDecayWeights),
            "ewma_mean" => Ok(Self::EwmaMean),
            "ewma_vol" => Ok(Self::EwmaVol),
            "ewma_zscore" => Ok(Self::EwmaZscore),
            _ => Err(Error::Validation(format!(
                "unsupported time-series transform op '{op}'"
            ))),
        }
    }
}

/// Transform a value column per entity, ordered by a sortable key.
///
/// `order` is compared lexicographically within each entity. Use ISO-8601 date
/// strings or another sortable key format when passing temporal labels.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, `op` is unsupported,
/// or operation parameters are malformed.
pub fn transform_timeseries(
    values: &[Option<f64>],
    entity: &[String],
    order: &[String],
    op: &str,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    transform_timeseries_with_op(values, entity, order, TimeSeriesOp::from_str(op)?, params)
}

/// Transform a value column per entity with a typed operation.
///
/// `order` is compared lexicographically within each entity. Use ISO-8601 date
/// strings or another sortable key format when passing temporal labels.
///
/// # Errors
///
/// Returns a validation error when input lengths differ or operation parameters
/// are malformed.
pub fn transform_timeseries_with_op(
    values: &[Option<f64>],
    entity: &[String],
    order: &[String],
    op: TimeSeriesOp,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(
        values.len(),
        &[("entity", entity.len()), ("order", order.len())],
    )?;
    let mut output = vec![None; values.len()];
    let mut indices = sorted_indices(entity, order);
    let mut start = 0;
    while start < indices.len() {
        let mut end = start + 1;
        while end < indices.len() && entity[indices[end]] == entity[indices[start]] {
            end += 1;
        }
        transform_entity(values, &indices[start..end], op, params, &mut output)?;
        start = end;
    }
    indices.clear();
    Ok(output)
}

fn sorted_indices(entity: &[String], order: &[String]) -> Vec<usize> {
    let mut indices = (0..entity.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        entity[*left]
            .cmp(&entity[*right])
            .then(order[*left].cmp(&order[*right]))
            .then(left.cmp(right))
    });
    indices
}

fn transform_entity(
    values: &[Option<f64>],
    indices: &[usize],
    op: TimeSeriesOp,
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    match op {
        TimeSeriesOp::Returns => shifted_ratio(values, indices, params, output, false),
        TimeSeriesOp::LogReturns => shifted_ratio(values, indices, params, output, true),
        TimeSeriesOp::Diff => diff(values, indices, params, output),
        TimeSeriesOp::Lag => lag(values, indices, params, output),
        TimeSeriesOp::RollingMean => rolling(values, indices, params, output, RollingOp::Mean),
        TimeSeriesOp::RollingSum => rolling(values, indices, params, output, RollingOp::Sum),
        TimeSeriesOp::RollingStd => rolling(values, indices, params, output, RollingOp::Std),
        TimeSeriesOp::RollingMin => rolling(values, indices, params, output, RollingOp::Min),
        TimeSeriesOp::RollingMax => rolling(values, indices, params, output, RollingOp::Max),
        TimeSeriesOp::RollingZscore => rolling(values, indices, params, output, RollingOp::Zscore),
        TimeSeriesOp::RollingRank => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Rank)
        }
        TimeSeriesOp::RollingQuantile => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Quantile)
        }
        TimeSeriesOp::RollingSkew => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Skew)
        }
        TimeSeriesOp::RollingKurtosis => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Kurtosis)
        }
        TimeSeriesOp::RollingSlope => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Slope)
        }
        TimeSeriesOp::RollingSharpe => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Sharpe)
        }
        TimeSeriesOp::RollingWinsorize => rolling_advanced(
            values,
            indices,
            params,
            output,
            AdvancedRollingOp::Winsorize,
        ),
        TimeSeriesOp::Drawdown => drawdown(values, indices, output),
        TimeSeriesOp::HampelFilter => {
            rolling_advanced(values, indices, params, output, AdvancedRollingOp::Hampel)
        }
        TimeSeriesOp::ExponentialDecayWeights => {
            exponential_decay_weights(values, indices, params, output)
        }
        TimeSeriesOp::EwmaMean => ewma_mean(values, indices, params, output),
        TimeSeriesOp::EwmaVol => ewma_vol(values, indices, params, output),
        TimeSeriesOp::EwmaZscore => ewma_zscore(values, indices, params, output),
    }
}

fn shifted_ratio(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
    log_return: bool,
) -> Result<()> {
    let periods = usize_param(params, "periods", 1)?;
    for (pos, &idx) in indices.iter().enumerate() {
        if pos < periods {
            output[idx] = None;
            continue;
        }
        let current = finite(values[idx]);
        let previous = finite(values[indices[pos - periods]]);
        output[idx] = match (current, previous) {
            (Some(current), Some(previous)) if previous.abs() > ZERO_TOLERANCE => {
                let ratio = current / previous;
                if log_return {
                    if ratio > 0.0 {
                        Some(ratio.ln())
                    } else {
                        None
                    }
                } else {
                    Some(ratio - 1.0)
                }
            }
            _ => None,
        };
    }
    Ok(())
}

fn lag(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let periods = usize_param(params, "periods", 1)?;
    for (pos, &idx) in indices.iter().enumerate() {
        output[idx] = if pos < periods {
            None
        } else {
            finite(values[indices[pos - periods]])
        };
    }
    Ok(())
}

fn diff(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let periods = usize_param(params, "periods", 1)?;
    for (pos, &idx) in indices.iter().enumerate() {
        if pos < periods {
            output[idx] = None;
            continue;
        }
        output[idx] = match (finite(values[idx]), finite(values[indices[pos - periods]])) {
            (Some(current), Some(previous)) => Some(current - previous),
            _ => None,
        };
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RollingOp {
    Mean,
    Sum,
    Std,
    Min,
    Max,
    Zscore,
}

fn rolling(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
    op: RollingOp,
) -> Result<()> {
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    for (pos, &idx) in indices.iter().enumerate() {
        let start = pos.saturating_sub(window - 1);
        let finite_values = indices[start..=pos]
            .iter()
            .filter_map(|window_idx| finite(values[*window_idx]))
            .collect::<Vec<_>>();
        let required = match op {
            RollingOp::Std | RollingOp::Zscore => min_periods.max(2),
            _ => min_periods,
        };
        if finite_values.len() < required {
            output[idx] = None;
            continue;
        }
        output[idx] = match op {
            RollingOp::Mean => Some(finite_values.iter().sum::<f64>() / finite_values.len() as f64),
            RollingOp::Sum => Some(finite_values.iter().sum()),
            RollingOp::Std => sample_std(&finite_values),
            RollingOp::Min => finite_values.into_iter().reduce(f64::min),
            RollingOp::Max => finite_values.into_iter().reduce(f64::max),
            RollingOp::Zscore => {
                let current = finite(values[idx]);
                let mean = finite_values.iter().sum::<f64>() / finite_values.len() as f64;
                let std = sample_std(&finite_values);
                match (current, std) {
                    (Some(current), Some(std)) if std > ZERO_TOLERANCE => {
                        Some((current - mean) / std)
                    }
                    (Some(_), Some(_)) => Some(0.0),
                    _ => None,
                }
            }
        };
    }
    Ok(())
}

fn ewma_alpha(params: Option<&Value>) -> Result<f64> {
    let span = required_f64_param(params, "span")?;
    if span <= 0.0 {
        return Err(Error::Validation(
            "panel transform parameter 'span' must be positive".to_string(),
        ));
    }
    Ok(2.0 / (span + 1.0))
}

fn ewma_mean(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let alpha = ewma_alpha(params)?;
    let mut state = None;
    for &idx in indices {
        output[idx] = match finite(values[idx]) {
            Some(value) => {
                let next = match state {
                    Some(prev) => alpha * value + (1.0 - alpha) * prev,
                    None => value,
                };
                state = Some(next);
                Some(next)
            }
            None => None,
        };
    }
    Ok(())
}

fn ewma_vol(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let alpha = ewma_alpha(params)?;
    let mut variance = None;
    for &idx in indices {
        output[idx] = match finite(values[idx]) {
            Some(value) => {
                let next = match variance {
                    Some(prev) => alpha * value * value + (1.0 - alpha) * prev,
                    None => value * value,
                };
                variance = Some(next);
                Some(next.sqrt())
            }
            None => None,
        };
    }
    Ok(())
}

fn ewma_zscore(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let alpha = ewma_alpha(params)?;
    let mut mean_state = None;
    let mut variance_state = None;
    for &idx in indices {
        output[idx] = match finite(values[idx]) {
            Some(value) => {
                let (next_mean, next_variance) = match (mean_state, variance_state) {
                    (Some(prev_mean), Some(prev_variance)) => {
                        let diff = value - prev_mean;
                        let next_mean = prev_mean + alpha * diff;
                        let next_variance = (1.0 - alpha) * (prev_variance + alpha * diff * diff);
                        (next_mean, next_variance)
                    }
                    _ => (value, 0.0),
                };
                mean_state = Some(next_mean);
                variance_state = Some(next_variance);
                if next_variance <= ZERO_TOLERANCE {
                    Some(0.0)
                } else {
                    Some((value - next_mean) / next_variance.sqrt())
                }
            }
            None => None,
        };
    }
    Ok(())
}
