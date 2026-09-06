//! Multi-input feature transforms.
//!
//! Grouped cross-sectional ops, pairwise rolling statistics, OLS
//! neutralization / residualization, and signal-to-weight helpers that operate
//! on more than one aligned column.

use crate::cross_sectional::apply_cross_sectional_op;
use crate::index::{sorted_indices, try_for_each_entity, try_for_each_trailing_window};
use crate::types::{
    bool_param, finite, mean, op_from_str, reject_unknown_params, scaled_centered,
    validate_lengths, validate_output, window_params,
};
use crate::{transform_cross_sectional, transform_cross_sectional_with_op, CrossSectionalOp};
use finstack_quant_core::math::stats::{covariance, variance};
use finstack_quant_core::{Error, Result};
use nalgebra::{DMatrix, DVector};
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
    crate::cross_sectional::validate_params(op, params)?;
    let partitions = crate::index::partition_by_pair(time_key, groups);

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        apply_cross_sectional_op(values, indices, op, params, &mut output)?;
    }
    validate_output(&output)?;
    Ok(output)
}

/// Remove cross-sectional exposure effects by OLS residualization per time key.
///
/// `exposures` is a slice of columns, each aligned to `values`. Parameters:
/// `fit_intercept` (default `true`). Equal-weighted OLS; a singular or
/// underdetermined design in any time partition fails the call. Scaled SVD
/// avoids dependence on exposure units. Residuals within numerical roundoff
/// of an exact fit are set to zero.
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
/// rows than columns or a rank-deficient scaled design (the error names that `time_key`).
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
    validate_output(&output)?;
    Ok(output)
}

/// Transform two value columns per entity with a rolling pairwise operation.
///
/// `window` spans trailing entity rows, including missing rows; `min_periods`
/// counts complete finite pairs within it. Aggregates may emit at a missing
/// current row when enough pairs remain. Neither parameter counts calendar days.
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
///   `min_periods` defaults to `window`. `window` counts rows; `min_periods`
///   counts finite pairs and cannot exceed `window`.
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
/// `window` spans trailing rows, including missing rows. `min_periods` counts
/// finite pairs within that window and cannot exceed its length.
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
    let (window, min_periods) = window_params(params)?;
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
    validate_output(&output)?;
    Ok(output)
}

/// Return rolling OLS residuals per entity using aligned exposure columns.
///
/// Parameters: `window`, `min_periods` (default `window`), and `fit_intercept`
/// (default `true`). `window` spans rows, including missing rows.
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
///   `fit_intercept`. `window` spans rows; `min_periods` counts complete rows
///   within it and must not exceed `window`.
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
    let (window, min_periods) = window_params(params)?;
    let fit_intercept = bool_param(params, "fit_intercept", true)?;
    let mut output = vec![None; values.len()];
    let indices = sorted_indices(entity, order);
    try_for_each_entity(entity, &indices, |entity_indices| {
        try_for_each_trailing_window(entity_indices, window, |idx, window_indices| {
            if count_complete_rows(values, exposures, window_indices) < min_periods {
                return Ok(());
            }
            if let Some(fit) = fit_ols(values, exposures, window_indices, fit_intercept)? {
                output[idx] = residual_for_idx(values, exposures, idx, &fit);
            }
            Ok(())
        })
    })?;
    validate_output(&output)?;
    Ok(output)
}

/// Convert a signal to dollar-neutral inverse-risk-scaled weights per time key.
///
/// Finite rows with `vol > 0` become `raw = signal / vol`, then
/// `centered = raw - mean(raw)`, then `weight = centered / sum(|centered|)`.
/// If that gross is zero, finite rows emit `0.0`. Negative volatility fails. Missing
/// signal or volatility stays missing.
///
/// # Arguments
///
/// * `values` - Row-aligned raw signal values to convert into portfolio weights.
/// * `time_key` - Row-aligned labels defining independently normalized
///   cross-sections.
/// * `volatility` - Row-aligned nonnegative risk estimates using a common horizon
///   and units; negative values fail, while zero, missing, or non-finite values
///   produce missing output weights.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, volatility is negative,
/// or scaling produces a non-finite result.
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
    if volatility
        .iter()
        .any(|vol| finite(*vol).is_some_and(|v| v < 0.0))
    {
        return Err(Error::Validation("volatility must not be negative".into()));
    }
    let scaled = values
        .iter()
        .zip(volatility.iter())
        .map(|(signal, vol)| match (finite(*signal), finite(*vol)) {
            (Some(signal), Some(vol)) if vol > 0.0 => Some(signal / vol),
            _ => None,
        })
        .collect::<Vec<_>>();
    validate_output(&scaled)?;
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
///   `true` and must remain true to preserve exposure neutrality after demeaning.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, exposure shapes are
/// malformed, a partition is singular, `fit_intercept` is false, or arithmetic fails.
pub fn neutralize_and_zscore(
    values: &[Option<f64>],
    time_key: &[String],
    exposures: &[Vec<Option<f64>>],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    if !bool_param(params, "fit_intercept", true)? {
        return Err(Error::Validation(
            "neutralize_and_zscore requires fit_intercept=true to preserve exposure neutrality"
                .into(),
        ));
    }
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
    let (left_scale, left) = scaled_centered(left);
    let (right_scale, right) = scaled_centered(right);
    let cov = covariance(&left, &right);
    match op {
        PairwiseOp::RollingCov => Some((cov * left_scale) * right_scale),
        PairwiseOp::RollingCorr => {
            let left_std = variance(&left).sqrt();
            let right_std = variance(&right).sqrt();
            Some(if left_std > 0.0 && right_std > 0.0 {
                ((cov / left_std) / right_std).clamp(-1.0, 1.0)
            } else {
                0.0
            })
        }
        PairwiseOp::RollingBeta => {
            let right_var = variance(&right);
            Some(if right_var > 0.0 {
                ((cov / right_var) * left_scale) / right_scale
            } else {
                0.0
            })
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
    let fit = fit_ols(values, exposures, indices, fit_intercept)?.ok_or_else(|| {
        Error::Validation(format!(
            "neutralize OLS failed for time_key '{time_key}': singular or underdetermined design"
        ))
    })?;
    for &idx in indices {
        output[idx] = residual_for_idx(values, exposures, idx, &fit);
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

/// A fitted model evaluated in the scaled coordinates used by the solver.
struct OlsFit {
    beta: Vec<f64>,
    scales: Vec<f64>,
    centers: Vec<f64>,
    spreads: Vec<f64>,
    response_scale: f64,
    fit_intercept: bool,
    roundoff: f64,
}

fn fit_ols(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    indices: &[usize],
    fit_intercept: bool,
) -> Result<Option<OlsFit>> {
    let width = exposures.len() + usize::from(fit_intercept);
    let complete: Vec<_> = indices
        .iter()
        .copied()
        .filter(|&idx| {
            finite(values[idx]).is_some() && exposures.iter().all(|col| finite(col[idx]).is_some())
        })
        .collect();
    if width == 0 || complete.len() < width {
        return Ok(None);
    }
    let mut design = DMatrix::zeros(complete.len(), width);
    if fit_intercept {
        design.column_mut(0).fill(1.0);
    }
    let mut fit = OlsFit {
        beta: Vec::new(),
        scales: Vec::new(),
        centers: Vec::new(),
        spreads: Vec::new(),
        response_scale: complete
            .iter()
            .map(|&idx| values[idx].unwrap_or(0.0).abs())
            .fold(0.0, f64::max),
        fit_intercept,
        roundoff: 8.0 * f64::EPSILON * complete.len().max(width) as f64,
    };
    if fit.response_scale <= 0.0 {
        fit.response_scale = 1.0;
    }
    for (col, exposure) in exposures.iter().enumerate() {
        let scale = complete
            .iter()
            .map(|&idx| exposure[idx].unwrap_or(0.0).abs())
            .fold(0.0, f64::max);
        if scale <= 0.0 {
            return Ok(None);
        }
        let normalized: Vec<_> = complete
            .iter()
            .map(|&idx| exposure[idx].unwrap_or(0.0) / scale)
            .collect();
        let center = if fit_intercept {
            mean(&normalized).unwrap_or(0.0)
        } else {
            0.0
        };
        let spread = normalized
            .iter()
            .map(|value| (value - center).abs())
            .fold(0.0, f64::max);
        if spread <= 0.0 {
            return Ok(None);
        }
        for (row, value) in normalized.iter().enumerate() {
            design[(row, col + usize::from(fit_intercept))] = (value - center) / spread;
        }
        fit.scales.push(scale);
        fit.centers.push(center);
        fit.spreads.push(spread);
    }
    let response = DVector::from_iterator(
        complete.len(),
        complete
            .iter()
            .map(|&idx| values[idx].unwrap_or(0.0) / fit.response_scale),
    );
    // Scale columns before SVD; do not square the design's condition number
    // by forming X'X. Truly rank-deficient windows remain explicitly missing.
    let svd = nalgebra::linalg::SVD::try_new(design, true, true, f64::EPSILON, 1000)
        .ok_or_else(|| Error::Validation("feature OLS SVD did not converge".into()))?;
    let max_singular = svd.singular_values.iter().copied().fold(0.0, f64::max);
    let cutoff = f64::EPSILON * complete.len().max(width) as f64 * max_singular;
    if svd.rank(cutoff) < width {
        return Ok(None);
    }
    let beta = svd
        .solve(&response, cutoff)
        .map_err(|message| Error::Validation(format!("feature OLS solve failed: {message}")))?;
    if beta.iter().any(|value| !value.is_finite()) {
        return Err(Error::Validation(
            "non-finite feature OLS coefficients".into(),
        ));
    }
    fit.beta = beta.as_slice().to_vec();
    Ok(Some(fit))
}

fn residual_for_idx(
    values: &[Option<f64>],
    exposures: &[Vec<Option<f64>>],
    idx: usize,
    fit: &OlsFit,
) -> Option<f64> {
    let y = finite(values[idx])? / fit.response_scale;
    let offset = usize::from(fit.fit_intercept);
    let mut fitted = if fit.fit_intercept { fit.beta[0] } else { 0.0 };
    let mut magnitude = y.abs() + fitted.abs();
    for (col, exposure) in exposures.iter().enumerate() {
        let x = (finite(exposure[idx])? / fit.scales[col] - fit.centers[col]) / fit.spreads[col];
        let term = fit.beta[col + offset] * x;
        fitted += term;
        magnitude += term.abs();
    }
    let residual = y - fitted;
    // Do not amplify solver roundoff into unit-variance signals for an exact fit.
    Some(if residual.abs() <= fit.roundoff * magnitude {
        0.0
    } else {
        residual * fit.response_scale
    })
}
