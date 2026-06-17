//! Backward-looking time-series panel transforms.

use crate::types::{
    finite, required_f64_param, sample_std, usize_param, validate_lengths, ZERO_TOLERANCE,
};
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
    /// Exponentially weighted mean using `span`.
    Ewma,
}

impl TimeSeriesOp {
    /// Return the canonical snake_case operation name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Returns => "returns",
            Self::LogReturns => "log_returns",
            Self::Lag => "lag",
            Self::RollingMean => "rolling_mean",
            Self::RollingSum => "rolling_sum",
            Self::RollingStd => "rolling_std",
            Self::RollingMin => "rolling_min",
            Self::RollingMax => "rolling_max",
            Self::Ewma => "ewma",
        }
    }
}

impl FromStr for TimeSeriesOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        match op {
            "returns" => Ok(Self::Returns),
            "log_returns" => Ok(Self::LogReturns),
            "lag" => Ok(Self::Lag),
            "rolling_mean" => Ok(Self::RollingMean),
            "rolling_sum" => Ok(Self::RollingSum),
            "rolling_std" => Ok(Self::RollingStd),
            "rolling_min" => Ok(Self::RollingMin),
            "rolling_max" => Ok(Self::RollingMax),
            "ewma" => Ok(Self::Ewma),
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
        TimeSeriesOp::Lag => lag(values, indices, params, output),
        TimeSeriesOp::RollingMean => rolling(values, indices, params, output, RollingOp::Mean),
        TimeSeriesOp::RollingSum => rolling(values, indices, params, output, RollingOp::Sum),
        TimeSeriesOp::RollingStd => rolling(values, indices, params, output, RollingOp::Std),
        TimeSeriesOp::RollingMin => rolling(values, indices, params, output, RollingOp::Min),
        TimeSeriesOp::RollingMax => rolling(values, indices, params, output, RollingOp::Max),
        TimeSeriesOp::Ewma => ewma(values, indices, params, output),
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

#[derive(Clone, Copy)]
enum RollingOp {
    Mean,
    Sum,
    Std,
    Min,
    Max,
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
            RollingOp::Std => min_periods.max(2),
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
        };
    }
    Ok(())
}

fn ewma(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let span = required_f64_param(params, "span")?;
    if span <= 0.0 {
        return Err(Error::Validation(
            "panel transform parameter 'span' must be positive".to_string(),
        ));
    }
    let alpha = 2.0 / (span + 1.0);
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
