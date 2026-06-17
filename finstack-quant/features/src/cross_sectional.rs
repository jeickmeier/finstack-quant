//! Cross-sectional panel transforms partitioned by timestamp.

use crate::types::{f64_param, finite, mean, population_std, validate_lengths, ZERO_TOLERANCE};
use finstack_quant_core::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Supported cross-sectional transform operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CrossSectionalOp {
    /// Z-score within each partition.
    Zscore,
    /// Percentile rank within each partition.
    Rank,
    /// Difference from partition mean.
    Demean,
    /// Clamp values to partition quantile bounds.
    Winsorize,
}

impl CrossSectionalOp {
    /// Return the canonical snake_case operation name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zscore => "zscore",
            Self::Rank => "rank",
            Self::Demean => "demean",
            Self::Winsorize => "winsorize",
        }
    }
}

impl FromStr for CrossSectionalOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        match op {
            "zscore" => Ok(Self::Zscore),
            "rank" => Ok(Self::Rank),
            "demean" => Ok(Self::Demean),
            "winsorize" => Ok(Self::Winsorize),
            _ => Err(Error::Validation(format!(
                "unsupported cross-sectional transform op '{op}'"
            ))),
        }
    }
}

/// Transform a value column across entities within each time partition.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, `op` is unsupported,
/// or operation parameters are malformed.
pub fn transform_cross_sectional(
    values: &[Option<f64>],
    time_key: &[String],
    op: &str,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    transform_cross_sectional_with_op(values, time_key, CrossSectionalOp::from_str(op)?, params)
}

/// Transform a value column across entities within each time partition.
///
/// # Errors
///
/// Returns a validation error when input lengths differ or operation parameters
/// are malformed.
pub fn transform_cross_sectional_with_op(
    values: &[Option<f64>],
    time_key: &[String],
    op: CrossSectionalOp,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(values.len(), &[("time_key", time_key.len())])?;
    let mut partitions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (idx, key) in time_key.iter().enumerate() {
        partitions.entry(key.as_str()).or_default().push(idx);
    }

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        match op {
            CrossSectionalOp::Zscore => zscore(values, indices, &mut output),
            CrossSectionalOp::Rank => rank(values, indices, &mut output),
            CrossSectionalOp::Demean => demean(values, indices, &mut output),
            CrossSectionalOp::Winsorize => winsorize(values, indices, params, &mut output)?,
        }
    }
    Ok(output)
}

fn finite_partition(values: &[Option<f64>], indices: &[usize]) -> Vec<(usize, f64)> {
    indices
        .iter()
        .filter_map(|idx| finite(values[*idx]).map(|value| (*idx, value)))
        .collect()
}

fn zscore(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let finite_values = finite_partition(values, indices);
    let sample = finite_values
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let Some(mean) = mean(&sample) else {
        return;
    };
    let Some(std) = population_std(&sample) else {
        return;
    };
    for (idx, value) in finite_values {
        output[idx] = if std <= ZERO_TOLERANCE {
            Some(0.0)
        } else {
            Some((value - mean) / std)
        };
    }
}

fn demean(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let finite_values = finite_partition(values, indices);
    let sample = finite_values
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let Some(mean) = mean(&sample) else {
        return;
    };
    for (idx, value) in finite_values {
        output[idx] = Some(value - mean);
    }
}

fn rank(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let mut finite_values = finite_partition(values, indices);
    finite_values.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let n = finite_values.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        output[finite_values[0].0] = Some(0.0);
        return;
    }

    let mut pos = 0;
    while pos < n {
        let mut end = pos + 1;
        while end < n && finite_values[end].1.total_cmp(&finite_values[pos].1) == Ordering::Equal {
            end += 1;
        }
        let percentile = pos as f64 / (n - 1) as f64;
        for (idx, _) in &finite_values[pos..end] {
            output[*idx] = Some(percentile);
        }
        pos = end;
    }
}

fn winsorize(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let lower = f64_param(params, "lower", 0.01)?;
    let upper = f64_param(params, "upper", 0.99)?;
    if !(0.0..=1.0).contains(&lower) || !(0.0..=1.0).contains(&upper) || lower > upper {
        return Err(Error::Validation(
            "winsorize requires 0 <= lower <= upper <= 1".to_string(),
        ));
    }

    let finite_values = finite_partition(values, indices);
    let mut sorted = finite_values
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    sorted.sort_by(f64::total_cmp);
    let Some(lower_bound) = quantile_cont(&sorted, lower) else {
        return Ok(());
    };
    let Some(upper_bound) = quantile_cont(&sorted, upper) else {
        return Ok(());
    };
    for (idx, value) in finite_values {
        output[idx] = Some(value.clamp(lower_bound, upper_bound));
    }
    Ok(())
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
    let lower = sorted[lower_idx];
    let upper = sorted[upper_idx];
    Some(lower + weight * (upper - lower))
}
