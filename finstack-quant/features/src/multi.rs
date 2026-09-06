//! Multi-input feature transforms.
//!
//! Grouped cross-sectional ops, pairwise rolling statistics, OLS
//! neutralization / residualization, and signal-to-weight helpers that operate
//! on more than one aligned column.

use crate::cross_sectional::apply_cross_sectional_op;
use crate::index::{sorted_indices, try_for_each_entity, try_for_each_trailing_window};
use crate::types::{
    bool_param, finite, op_from_str, reject_unknown_params, usize_param, validate_lengths,
    ZERO_TOLERANCE,
};
use crate::{transform_cross_sectional, transform_cross_sectional_with_op, CrossSectionalOp};
use finstack_quant_core::math::linalg::{cholesky_decomposition, cholesky_solve};
use finstack_quant_core::math::stats::{covariance, variance};
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

/// Supported pairwise rolling time-series transform operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PairwiseOp {
    /// Rolling sample covariance between `values` and `other`.
    RollingCov,
    /// Rolling Pearson correlation between `values` and `other`.
    RollingCorr,
    /// Rolling beta of `values` to `other`.
    RollingBeta,
}

impl FromStr for PairwiseOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        op_from_str(op, "pairwise time-series", &Self::names())
    }
}

impl PairwiseOp {
    /// Every operation, in declaration order.
    pub const ALL: &'static [Self] = &[Self::RollingCov, Self::RollingCorr, Self::RollingBeta];

    /// Canonical snake_case name accepted by [`transform_timeseries_pairwise`].
    #[must_use]
    pub fn name(self) -> String {
        crate::types::op_name(&self)
    }

    /// Canonical names of every operation, in [`Self::ALL`] order.
    #[must_use]
    pub fn names() -> Vec<String> {
        Self::ALL.iter().map(|op| op.name()).collect()
    }

    /// JSON parameter keys every pairwise operation reads (`window`,
    /// `min_periods`); any other key is rejected.
    #[must_use]
    pub fn param_keys(self) -> &'static [&'static str] {
        &["window", "min_periods"]
    }
}

/// Transform a cross-section within each `(time_key, group)` sub-partition.
///
/// # Arguments
///
/// * `values` - Row-aligned numeric input values; missing or non-finite values
///   are handled by the selected cross-sectional operation.
/// * `time_key` - Row-aligned time partition labels; each time/group pair is
///   transformed independently.
/// * `groups` - Row-aligned group labels that subdivide every time partition.
/// * `op` - Canonical snake-case cross-sectional operation name.
/// * `params` - Optional operation-specific JSON parameters; omitted keys use
///   the operation's documented defaults.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, `op` is unsupported,
/// or operation parameters are malformed.
pub fn transform_cross_sectional_grouped(
    values: &[Option<f64>],
    time_key: &[String],
    groups: &[String],
    op: &str,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    let op = CrossSectionalOp::from_str(op)?;
    transform_cross_sectional_grouped_with_op(values, time_key, groups, op, params)
}

/// Transform a cross-section within each `(time_key, group)` sub-partition.
///
/// # Arguments
///
/// * `values` - Row-aligned numeric input values; output preserves the input
///   row order after transforming each time/group sub-partition.
/// * `time_key` - Row-aligned time partition labels; length must equal
///   `values`.
/// * `groups` - Row-aligned group labels; length must equal `values`.
/// * `op` - Typed cross-sectional operation to apply to each sub-partition.
/// * `params` - Optional operation-specific JSON parameters; omitted keys use
///   the operation's documented defaults.
///
/// # Errors
///
/// Returns a validation error when input lengths differ or operation parameters
/// are malformed.
pub fn transform_cross_sectional_grouped_with_op(
    values: &[Option<f64>],
    time_key: &[String],
    groups: &[String],
    op: CrossSectionalOp,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(
        values.len(),
        &[("time_key", time_key.len()), ("groups", groups.len())],
    )?;
    reject_unknown_params(params, &op.name(), op.param_keys())?;
    let partitions = crate::index::partition_by_pair(time_key, groups);

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        apply_cross_sectional_op(values, indices, op, params, &mut output)?;
    }
    Ok(output)
}

/// Remove cross-sectional exposure effects by OLS residualization per time key.
///
/// `exposures` is a slice of columns, each aligned to `values`. Parameters:
/// `fit_intercept` (default `true`). Equal-weighted OLS; a singular or
/// underdetermined design in any time partition fails the call.
///
/// # Arguments
///
/// * `values` - Row-aligned dependent-variable observations to residualize.
/// * `time_key` - Row-aligned labels defining independent cross-sectional OLS
///   regressions for each time partition.
/// * `exposures` - Explanatory-variable columns, each row-aligned with
///   `values`; incomplete rows yield no residual.
/// * `params` - Optional JSON parameters; `fit_intercept` defaults to `true`.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, exposure shapes are
/// malformed, parameters are malformed, or a time partition has fewer complete
/// rows than columns or a singular `X'X` (the error names that `time_key`).
pub fn neutralize(
    values: &[Option<f64>],
    time_key: &[String],
    exposures: &[Vec<Option<f64>>],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_exposures(values.len(), exposures)?;
    validate_lengths(values.len(), &[("time_key", time_key.len())])?;
    reject_unknown_params(params, "neutralize", &["fit_intercept"])?;
    let fit_intercept = bool_param(params, "fit_intercept", true)?;
    let partitions = crate::index::partition_by_key(time_key);

    let mut output = vec![None; values.len()];
    for (key, indices) in &partitions {
        residualize_partition(values, exposures, key, indices, fit_intercept, &mut output)?;
    }
    Ok(output)
}

/// Transform two value columns per entity with a rolling pairwise operation.
///
/// `window` and `min_periods` count paired finite observations, not calendar
/// days. Missing rows do not expand the window (pandas `skipna`).
///
/// # Arguments
///
/// * `values` - Row-aligned first series, treated as the dependent series for
///   rolling beta.
/// * `other` - Row-aligned second series; paired observations require finite
///   values in both series.
/// * `entity` - Row-aligned entity identifiers; each entity is rolled
///   independently.
/// * `order` - Row-aligned sortable keys that establish order within entities.
///   Time order is lexicographic; use ISO-8601 for calendar chronology.
/// * `op` - Canonical operation name: `"rolling_cov"`, `"rolling_corr"`, or
///   `"rolling_beta"`.
/// * `params` - Optional JSON parameters; `window` defaults to 1 and
///   `min_periods` defaults to `window`. Both count finite paired rows.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, `op` is unsupported,
/// or operation parameters are malformed.
pub fn transform_timeseries_pairwise(
    values: &[Option<f64>],
    other: &[Option<f64>],
    entity: &[String],
    order: &[String],
    op: &str,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    transform_timeseries_pairwise_with_op(
        values,
        other,
        entity,
        order,
        PairwiseOp::from_str(op)?,
        params,
    )
}

/// Transform two value columns per entity with a typed rolling pairwise op.
///
/// `window` counts paired finite observations (pandas `skipna`).
///
/// # Arguments
///
/// * `values` - Row-aligned first series, treated as the dependent series for
///   rolling beta.
/// * `other` - Row-aligned second series; paired observations require finite
///   values in both series.
/// * `entity` - Row-aligned entity identifiers; each entity is rolled
///   independently.
/// * `order` - Row-aligned sortable keys that establish order within entities.
/// * `op` - Typed pairwise rolling statistic to calculate.
/// * `params` - Optional JSON parameters; `window` defaults to 1 and
///   `min_periods` defaults to `window`.
///
/// # Errors
///
/// Returns a validation error when input lengths differ or operation parameters
/// are malformed.
pub fn transform_timeseries_pairwise_with_op(
    values: &[Option<f64>],
    other: &[Option<f64>],
    entity: &[String],
    order: &[String],
    op: PairwiseOp,
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(
        values.len(),
        &[
            ("other", other.len()),
            ("entity", entity.len()),
            ("order", order.len()),
        ],
    )?;
    reject_unknown_params(params, &op.name(), op.param_keys())?;
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    let required = min_periods.max(2);
    let mut output = vec![None; values.len()];
    let indices = sorted_indices(entity, order);
    try_for_each_entity(entity, &indices, |entity_indices| {
        try_for_each_trailing_window(entity_indices, window, |idx, window_indices| {
            let mut left = Vec::new();
            let mut right = Vec::new();
            for &window_idx in window_indices {
                if let (Some(y), Some(x)) = (finite(values[window_idx]), finite(other[window_idx]))
                {
                    left.push(y);
                    right.push(x);
                }
            }
            if left.len() >= required {
                output[idx] = pairwise_value(&left, &right, op);
            }
            Ok(())
        })
    })?;
    Ok(output)
}

/// Return rolling OLS residuals per entity using aligned exposure columns.
///
/// Parameters: `window`, `min_periods` (default `window`), and `fit_intercept`
/// (default `true`). `window` counts complete finite rows (pandas `skipna`).
/// Rank-deficient windows emit `None` for that row; that is intentional and
/// unlike [`neutralize`], which fails the call.
///
/// # Arguments
///
/// * `values` - Row-aligned dependent observations used in each rolling OLS
///   regression.
/// * `exposures` - Explanatory-variable columns, each aligned to `values`.
/// * `entity` - Row-aligned entity identifiers; regressions do not cross entity
///   boundaries.
/// * `order` - Row-aligned sortable keys that establish rolling chronology.
///   Time order is lexicographic; use ISO-8601 for calendar chronology.
/// * `params` - Optional JSON controls for `window`, `min_periods`, and
///   `fit_intercept`. `window` counts complete finite rows.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, exposure shapes are
/// malformed, or parameters are malformed.
pub fn rolling_regression_residual(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    entity: &[String],
    order: &[String],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_exposures(values.len(), exposures)?;
    validate_lengths(
        values.len(),
        &[("entity", entity.len()), ("order", order.len())],
    )?;
    reject_unknown_params(
        params,
        "rolling_regression_residual",
        &["window", "min_periods", "fit_intercept"],
    )?;
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    let fit_intercept = bool_param(params, "fit_intercept", true)?;
    let mut output = vec![None; values.len()];
    let indices = sorted_indices(entity, order);
    try_for_each_entity(entity, &indices, |entity_indices| {
        try_for_each_trailing_window(entity_indices, window, |idx, window_indices| {
            if count_complete_rows(values, exposures, window_indices) < min_periods {
                return Ok(());
            }
            if let Some(beta) = fit_ols(values, exposures, window_indices, fit_intercept) {
                output[idx] = residual_for_idx(values, exposures, idx, fit_intercept, &beta);
            }
            Ok(())
        })
    })?;
    Ok(output)
}

/// Convert a signal to dollar-neutral inverse-risk-scaled weights per time key.
///
/// Finite rows with `|vol| > 1e-12` become `raw = signal / vol`, then
/// `centered = raw - mean(raw)`, then `weight = centered / sum(|centered|)`.
/// If that gross is at or below `1e-12`, finite rows emit `0.0`. Missing
/// signal or volatility stays missing.
///
/// # Arguments
///
/// * `values` - Row-aligned raw signal values to convert into portfolio weights.
/// * `time_key` - Row-aligned labels defining independently normalized
///   cross-sections.
/// * `volatility` - Row-aligned risk estimates; zero, missing, or non-finite
///   values produce missing output weights.
///
/// # Errors
///
/// Returns a validation error when input lengths differ.
pub fn risk_scaled_weights(
    values: &[Option<f64>],
    time_key: &[String],
    volatility: &[Option<f64>],
) -> Result<Vec<Option<f64>>> {
    validate_lengths(
        values.len(),
        &[
            ("time_key", time_key.len()),
            ("volatility", volatility.len()),
        ],
    )?;
    let scaled = values
        .iter()
        .zip(volatility.iter())
        .map(|(signal, vol)| match (finite(*signal), finite(*vol)) {
            (Some(signal), Some(vol)) if vol.abs() > ZERO_TOLERANCE => Some(signal / vol),
            _ => None,
        })
        .collect::<Vec<_>>();
    transform_cross_sectional_with_op(&scaled, time_key, CrossSectionalOp::LongShortWeights, None)
}

/// Convert cross-sectional ranks into gross-normalized long/short weights.
///
/// # Arguments
///
/// * `values` - Row-aligned signal values to rank before demeaning and gross
///   normalization.
/// * `time_key` - Row-aligned labels defining independently normalized
///   cross-sections.
///
/// # Errors
///
/// Returns a validation error when input lengths differ.
pub fn rank_to_weights(values: &[Option<f64>], time_key: &[String]) -> Result<Vec<Option<f64>>> {
    let ranks = transform_cross_sectional(values, time_key, "rank", None)?;
    transform_cross_sectional_with_op(&ranks, time_key, CrossSectionalOp::LongShortWeights, None)
}

/// Neutralize a signal against exposures and z-score the residuals.
///
/// # Arguments
///
/// * `values` - Row-aligned signal observations to neutralize and standardize.
/// * `time_key` - Row-aligned labels defining independent cross-sectional
///   regressions and z-scores.
/// * `exposures` - Explanatory-variable columns, each aligned to `values`.
/// * `params` - Optional neutralization controls; `fit_intercept` defaults to
///   `true`.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, exposure shapes are
/// malformed, or neutralization parameters are malformed.
pub fn neutralize_and_zscore(
    values: &[Option<f64>],
    time_key: &[String],
    exposures: &[Vec<Option<f64>>],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    let residual = neutralize(values, time_key, exposures, params)?;
    transform_cross_sectional(&residual, time_key, "zscore", None)
}

fn validate_exposures(primary_len: usize, exposures: &[Vec<Option<f64>>]) -> Result<()> {
    for (idx, exposure) in exposures.iter().enumerate() {
        if exposure.len() != primary_len {
            return Err(Error::Validation(format!(
                "panel transform length mismatch: values has length {primary_len}, exposure {idx} has length {}",
                exposure.len()
            )));
        }
    }
    Ok(())
}

fn pairwise_value(left: &[f64], right: &[f64], op: PairwiseOp) -> Option<f64> {
    let cov = covariance(left, right);
    if !cov.is_finite() {
        return None;
    }
    match op {
        PairwiseOp::RollingCov => Some(cov),
        PairwiseOp::RollingCorr => {
            let left_var = variance(left);
            let right_var = variance(right);
            let denom = (left_var * right_var).sqrt();
            if denom <= ZERO_TOLERANCE {
                Some(0.0)
            } else {
                Some(cov / denom)
            }
        }
        PairwiseOp::RollingBeta => {
            let right_var = variance(right);
            if right_var <= ZERO_TOLERANCE {
                Some(0.0)
            } else {
                Some(cov / right_var)
            }
        }
    }
}

fn residualize_partition(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    time_key: &str,
    indices: &[usize],
    fit_intercept: bool,
    output: &mut [Option<f64>],
) -> Result<()> {
    let beta = fit_ols(values, exposures, indices, fit_intercept).ok_or_else(|| {
        Error::Validation(format!(
            "neutralize OLS failed for time_key '{time_key}': singular or underdetermined design"
        ))
    })?;
    for &idx in indices {
        output[idx] = residual_for_idx(values, exposures, idx, fit_intercept, &beta);
    }
    Ok(())
}

fn count_complete_rows(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    indices: &[usize],
) -> usize {
    indices
        .iter()
        .filter(|&&idx| {
            finite(values[idx]).is_some()
                && exposures
                    .iter()
                    .all(|exposure| finite(exposure[idx]).is_some())
        })
        .count()
}

fn fit_ols(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    indices: &[usize],
    fit_intercept: bool,
) -> Option<Vec<f64>> {
    let width = exposures.len() + usize::from(fit_intercept);
    if width == 0 {
        return None;
    }
    let complete_rows = count_complete_rows(values, exposures, indices);
    if complete_rows < width {
        return None;
    }

    let mut gram = vec![0.0; width * width];
    let mut rhs = vec![0.0; width];
    for &idx in indices {
        let Some(y) = finite(values[idx]) else {
            continue;
        };
        let mut row = Vec::with_capacity(width);
        if fit_intercept {
            row.push(1.0);
        }
        let mut complete = true;
        for exposure in exposures {
            if let Some(value) = finite(exposure[idx]) {
                row.push(value);
            } else {
                complete = false;
                break;
            }
        }
        if !complete {
            continue;
        }
        for i in 0..width {
            rhs[i] += row[i] * y;
            for j in 0..width {
                gram[i * width + j] += row[i] * row[j];
            }
        }
    }

    let chol = cholesky_decomposition(&gram, width).ok()?;
    if cholesky_factor_is_singular(&chol, width) {
        return None;
    }
    let mut beta = vec![0.0; width];
    cholesky_solve(&chol, &rhs, &mut beta).ok()?;
    Some(beta)
}

fn cholesky_factor_is_singular(chol: &[f64], width: usize) -> bool {
    let max_diag_sq = (0..width)
        .map(|i| {
            let diag = chol[i * width + i];
            diag * diag
        })
        .fold(0.0, f64::max);
    let threshold = ZERO_TOLERANCE * max_diag_sq.max(1.0);
    (0..width).any(|i| {
        let diag = chol[i * width + i];
        diag * diag <= threshold
    })
}

fn residual_for_idx(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    idx: usize,
    fit_intercept: bool,
    beta: &[f64],
) -> Option<f64> {
    let y = finite(values[idx])?;
    let mut fitted = 0.0;
    let mut offset = 0;
    if fit_intercept {
        fitted += beta[0];
        offset = 1;
    }
    for (exposure_idx, exposure) in exposures.iter().enumerate() {
        fitted += beta[offset + exposure_idx] * finite(exposure[idx])?;
    }
    Some(y - fitted)
}
