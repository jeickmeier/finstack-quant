//! Cross-sectional panel transforms partitioned by timestamp.

use crate::types::{
    f64_param, finite, mean, op_from_str, population_std, quantile_cont, reject_unknown_params,
    scaled_centered, usize_param, validate_lengths, validate_output,
};
use finstack_quant_core::math::standard_normal_inv_cdf;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::str::FromStr;

/// Supported cross-sectional transform operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CrossSectionalOp {
    /// Population z-score within each partition; `0.0` for constant values.
    Zscore,
    /// Closed-interval percentile rank in `[0, 1]`; ties share the lowest rank.
    Rank,
    /// Open-interval percentile rank using average tied positions.
    PercentileRank,
    /// Integer quantile bucket labels from `0` to `buckets - 1`.
    QuantileBucket,
    /// Difference from partition mean.
    Demean,
    /// Robust z-score using median and MAD with normal-consistency scaling.
    RobustZscore,
    /// Scale finite values to the `[0, 1]` range within each partition.
    MinmaxScale,
    /// Clamp values to explicit lower and upper bounds.
    Clip,
    /// Clamp values to `mean ± sigma * population_std`.
    ClipBySigma,
    /// Map open-interval percentile ranks to standard-normal scores.
    NormalScoreTransform,
    /// Demean signal values and normalize by gross absolute exposure.
    LongShortWeights,
    /// Dollar-neutral, unit-gross weights with a final absolute position cap.
    CapWeights,
    /// Fill missing and non-finite values with a constant.
    FillMissing,
    /// Emit 1.0 for finite inputs and 0.0 otherwise.
    IsFinite,
    /// Emit 1.0 for missing or non-finite inputs and 0.0 otherwise.
    NanMask,
    /// Clamp values to partition quantile bounds.
    Winsorize,
}

impl FromStr for CrossSectionalOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        op_from_str(op, "cross-sectional", &Self::names())
    }
}

impl CrossSectionalOp {
    /// Every operation, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Zscore,
        Self::Rank,
        Self::PercentileRank,
        Self::QuantileBucket,
        Self::Demean,
        Self::RobustZscore,
        Self::MinmaxScale,
        Self::Clip,
        Self::ClipBySigma,
        Self::NormalScoreTransform,
        Self::LongShortWeights,
        Self::CapWeights,
        Self::FillMissing,
        Self::IsFinite,
        Self::NanMask,
        Self::Winsorize,
    ];

    /// Canonical snake_case name accepted by [`transform_cross_sectional`].
    #[must_use]
    pub fn name(self) -> String {
        crate::types::op_name(&self)
    }

    /// Canonical names of every operation, in [`Self::ALL`] order.
    #[must_use]
    pub fn names() -> Vec<String> {
        Self::ALL.iter().map(|op| op.name()).collect()
    }

    /// JSON parameter keys this operation reads; any other key is rejected.
    #[must_use]
    pub fn param_keys(self) -> &'static [&'static str] {
        match self {
            Self::QuantileBucket => &["buckets"],
            Self::Clip | Self::Winsorize => &["lower", "upper"],
            Self::ClipBySigma => &["sigma"],
            Self::CapWeights => &["max_abs"],
            Self::FillMissing => &["value"],
            Self::Zscore
            | Self::Rank
            | Self::PercentileRank
            | Self::Demean
            | Self::RobustZscore
            | Self::MinmaxScale
            | Self::NormalScoreTransform
            | Self::LongShortWeights
            | Self::IsFinite
            | Self::NanMask => &[],
        }
    }
}

/// Transform a value column across entities within each time partition.
///
/// `cap_weights` requires `0 < max_abs <= 1` (default 1). It preserves
/// demeaned-signal signs, allocates gross 0.5 to each side, and redistributes
/// capped exposure proportionally on that side. Infeasible side capacity fails;
/// constant signals produce zeros. Signed zeros tie in all rank operations.
///
/// # Arguments
///
/// * `values` - Row-aligned numeric input values; `None` and non-finite values
///   are handled according to the selected operation.
/// * `time_key` - Row-aligned partition labels; each distinct label defines one
///   cross-section transformed independently.
/// * `op` - Canonical snake-case operation name, such as `"zscore"` or
///   `"winsorize"`.
/// * `params` - Optional operation-specific JSON parameters; omitted keys use
///   the operation's documented defaults.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, `op` is unsupported,
/// operation parameters are malformed, a cap is infeasible, or arithmetic
/// produces a non-finite result.
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
/// # Arguments
///
/// * `values` - Row-aligned numeric input values; output preserves this row
///   order after independently transforming each partition.
/// * `time_key` - Row-aligned partition labels; length must equal `values`.
/// * `op` - Typed cross-sectional operation that determines the transform and
///   accepted parameter keys.
/// * `params` - Optional operation-specific JSON parameters; omitted keys use
///   the operation's documented defaults.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, operation parameters
/// are malformed, a cap is infeasible, or arithmetic produces a non-finite result.
pub fn transform_cross_sectional_with_op(
    values: &[Option<f64>],
    time_key: &[String],
    op: CrossSectionalOp,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(values.len(), &[("time_key", time_key.len())])?;
    reject_unknown_params(params, &op.name(), op.param_keys())?;
    validate_params(op, params)?;
    let partitions = crate::index::partition_by_key(time_key);

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        apply_cross_sectional_op(values, indices, op, params, &mut output)?;
    }
    validate_output(&output)?;
    Ok(output)
}

pub(crate) fn validate_params(op: CrossSectionalOp, params: Option<&Value>) -> Result<()> {
    match op {
        CrossSectionalOp::QuantileBucket => {
            usize_param(params, "buckets", 10)?;
        }
        CrossSectionalOp::Clip | CrossSectionalOp::Winsorize => {
            let quantiles = matches!(op, CrossSectionalOp::Winsorize);
            let lower = f64_param(
                params,
                "lower",
                if quantiles { 0.01 } else { f64::NEG_INFINITY },
            )?;
            let upper = f64_param(
                params,
                "upper",
                if quantiles { 0.99 } else { f64::INFINITY },
            )?;
            if lower > upper
                || (quantiles && (!(0.0..=1.0).contains(&lower) || !(0.0..=1.0).contains(&upper)))
            {
                return Err(Error::Validation(
                    "invalid lower/upper bounds for cross-sectional transform".into(),
                ));
            }
        }
        CrossSectionalOp::ClipBySigma => {
            if f64_param(params, "sigma", 3.0)? < 0.0 {
                return Err(Error::Validation(
                    "clip_by_sigma requires sigma >= 0".into(),
                ));
            }
        }
        CrossSectionalOp::CapWeights => {
            let max_abs = f64_param(params, "max_abs", 1.0)?;
            if !(0.0 < max_abs && max_abs <= 1.0) {
                return Err(Error::Validation(
                    "cap_weights requires 0 < max_abs <= 1".into(),
                ));
            }
        }
        CrossSectionalOp::FillMissing => {
            f64_param(params, "value", 0.0)?;
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn apply_cross_sectional_op(
    values: &[Option<f64>],
    indices: &[usize],
    op: CrossSectionalOp,
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    match op {
        CrossSectionalOp::Zscore => zscore(values, indices, output),
        CrossSectionalOp::Rank => rank(values, indices, output),
        CrossSectionalOp::PercentileRank => percentile_rank(values, indices, output),
        CrossSectionalOp::QuantileBucket => quantile_bucket(values, indices, params, output)?,
        CrossSectionalOp::Demean => demean(values, indices, output),
        CrossSectionalOp::RobustZscore => robust_zscore(values, indices, output),
        CrossSectionalOp::MinmaxScale => minmax_scale(values, indices, output),
        CrossSectionalOp::Clip => clip(values, indices, params, output)?,
        CrossSectionalOp::ClipBySigma => clip_by_sigma(values, indices, params, output)?,
        CrossSectionalOp::Winsorize => winsorize(values, indices, params, output)?,
        CrossSectionalOp::NormalScoreTransform => normal_score_transform(values, indices, output),
        CrossSectionalOp::LongShortWeights => long_short_weights(values, indices, output),
        CrossSectionalOp::CapWeights => cap_weights(values, indices, params, output)?,
        CrossSectionalOp::FillMissing => fill_missing(values, indices, params, output)?,
        CrossSectionalOp::IsFinite => is_finite(values, indices, output),
        CrossSectionalOp::NanMask => nan_mask(values, indices, output),
    }
    Ok(())
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
    let (_, centered) = scaled_centered(&sample);
    let Some(std) = population_std(&centered) else {
        return;
    };
    for ((idx, _), value) in finite_values.iter().zip(centered) {
        output[*idx] = if std <= 0.0 {
            Some(0.0)
        } else {
            Some(value / std)
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
    ranked(values, indices, output, RankMode::ClosedPercentile);
}

#[derive(Clone, Copy)]
enum RankMode {
    ClosedPercentile,
    OpenPercentile,
}

fn ranked(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>], mode: RankMode) {
    for (idx, rank) in partition_ranks(values, indices, mode) {
        output[idx] = Some(rank);
    }
}

fn partition_ranks(values: &[Option<f64>], indices: &[usize], mode: RankMode) -> Vec<(usize, f64)> {
    let mut finite_values = finite_partition(values, indices);
    finite_values.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let n = finite_values.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        let rank = match mode {
            RankMode::ClosedPercentile => 0.0,
            RankMode::OpenPercentile => 0.5,
        };
        return vec![(finite_values[0].0, rank)];
    }

    let mut output = Vec::with_capacity(n);
    let mut pos = 0;
    while pos < n {
        let mut end = pos + 1;
        while end < n && finite_values[end].1.total_cmp(&finite_values[pos].1) == Ordering::Equal {
            end += 1;
        }
        let percentile = match mode {
            RankMode::ClosedPercentile => pos as f64 / (n - 1) as f64,
            RankMode::OpenPercentile => {
                let avg_pos = (pos + end - 1) as f64 / 2.0;
                (avg_pos + 1.0) / (n + 1) as f64
            }
        };
        for (idx, _) in &finite_values[pos..end] {
            output.push((*idx, percentile));
        }
        pos = end;
    }
    output
}

fn percentile_rank(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    ranked(values, indices, output, RankMode::OpenPercentile);
}

fn quantile_bucket(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let buckets = usize_param(params, "buckets", 10)?;
    for (idx, rank) in partition_ranks(values, indices, RankMode::ClosedPercentile) {
        let bucket = (rank * buckets as f64).floor().min((buckets - 1) as f64);
        output[idx] = Some(bucket);
    }
    Ok(())
}

fn robust_zscore(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let finite_values = finite_partition(values, indices);
    let mut sample = finite_values
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let scale = sample
        .iter()
        .copied()
        .map(f64::abs)
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    for value in &mut sample {
        *value /= scale;
    }
    sample.sort_by(f64::total_cmp);
    let Some(center) = quantile_cont(&sample, 0.5) else {
        return;
    };
    let mut deviations = sample
        .iter()
        .map(|value| (*value - center).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let Some(mad) = quantile_cont(&deviations, 0.5) else {
        return;
    };
    for (idx, value) in finite_values {
        output[idx] = if mad <= 0.0 {
            Some(0.0)
        } else {
            Some(crate::types::PHI_INV_075 * (value / scale - center) / mad)
        };
    }
}

fn minmax_scale(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let finite_values = finite_partition(values, indices);
    let Some(((_, first), rest)) = finite_values.split_first() else {
        return;
    };
    let scale = finite_values
        .iter()
        .map(|(_, v)| v.abs())
        .fold(0.0, f64::max);
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let mut min_value = *first / scale;
    let mut max_value = *first / scale;
    for (_, value) in rest {
        min_value = min_value.min(*value / scale);
        max_value = max_value.max(*value / scale);
    }
    let range = max_value - min_value;
    for (idx, value) in finite_values {
        output[idx] = if range <= 0.0 {
            Some(0.0)
        } else {
            Some((value / scale - min_value) / range)
        };
    }
}

fn clip(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let lower = f64_param(params, "lower", f64::NEG_INFINITY)?;
    let upper = f64_param(params, "upper", f64::INFINITY)?;
    if lower > upper {
        return Err(Error::Validation(
            "clip requires lower <= upper".to_string(),
        ));
    }
    for (idx, value) in finite_partition(values, indices) {
        output[idx] = Some(value.clamp(lower, upper));
    }
    Ok(())
}

fn clip_by_sigma(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let sigma = f64_param(params, "sigma", 3.0)?;
    if sigma < 0.0 {
        return Err(Error::Validation(
            "clip_by_sigma requires sigma >= 0".to_string(),
        ));
    }
    let finite_values = finite_partition(values, indices);
    let sample = finite_values
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let Some(center) = mean(&sample) else {
        return Ok(());
    };
    let Some(std) = population_std(&sample) else {
        return Ok(());
    };
    let lower = center - sigma * std;
    let upper = center + sigma * std;
    if !lower.is_finite() || !upper.is_finite() {
        return Err(Error::Validation("non-finite clip_by_sigma bounds".into()));
    }
    for (idx, value) in finite_values {
        output[idx] = Some(value.clamp(lower, upper));
    }
    Ok(())
}

fn normal_score_transform(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    for (idx, rank) in partition_ranks(values, indices, RankMode::OpenPercentile) {
        output[idx] = Some(standard_normal_inv_cdf(rank));
    }
}

fn long_short_weights(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    let finite_values = finite_partition(values, indices);
    let sample: Vec<_> = finite_values.iter().map(|(_, value)| *value).collect();
    let (_, centered) = scaled_centered(&sample);
    let gross = centered.iter().map(|value| value.abs()).sum::<f64>();
    for ((idx, _), weight) in finite_values.iter().zip(centered) {
        output[*idx] = Some(if gross > 0.0 { weight / gross } else { 0.0 });
    }
}

fn cap_weights(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let max_abs = f64_param(params, "max_abs", 1.0)?;
    let finite_values = finite_partition(values, indices);
    let sample: Vec<_> = finite_values.iter().map(|(_, value)| *value).collect();
    let (_, centered) = scaled_centered(&sample);
    let mut long = Vec::new();
    let mut short = Vec::new();
    for ((idx, _), value) in finite_values.iter().zip(centered) {
        output[*idx] = Some(0.0);
        if value > 0.0 {
            long.push((*idx, value));
        } else if value < 0.0 {
            short.push((*idx, -value));
        }
    }
    if long.is_empty() && short.is_empty() {
        return Ok(());
    }
    for (side, sign) in [(&mut long, 1.0), (&mut short, -1.0)] {
        if (side.len() as f64) * max_abs < 0.5 {
            return Err(Error::Validation(
                "cap_weights is infeasible: each signal side must support gross exposure 0.5 at max_abs".into(),
            ));
        }
        side.sort_by(|left, right| right.1.total_cmp(&left.1));
        // Suffix sums avoid cancellation when a dominant position is capped.
        let mut suffix = vec![0.0; side.len() + 1];
        for i in (0..side.len()).rev() {
            suffix[i] = suffix[i + 1] + side[i].1;
        }
        let mut remaining = 0.5;
        for (i, &(idx, magnitude)) in side.iter().enumerate() {
            let weight = (remaining * (magnitude / suffix[i])).min(max_abs);
            output[idx] = Some(sign * weight);
            remaining = (remaining - weight).max(0.0);
        }
    }
    Ok(())
}

fn fill_missing(
    values: &[Option<f64>],
    indices: &[usize],
    params: Option<&Value>,
    output: &mut [Option<f64>],
) -> Result<()> {
    let fill_value = f64_param(params, "value", 0.0)?;
    for &idx in indices {
        output[idx] = Some(finite(values[idx]).unwrap_or(fill_value));
    }
    Ok(())
}

fn is_finite(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    for &idx in indices {
        output[idx] = Some(if finite(values[idx]).is_some() {
            1.0
        } else {
            0.0
        });
    }
}

fn nan_mask(values: &[Option<f64>], indices: &[usize], output: &mut [Option<f64>]) {
    for &idx in indices {
        output[idx] = Some(if finite(values[idx]).is_none() {
            1.0
        } else {
            0.0
        });
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
