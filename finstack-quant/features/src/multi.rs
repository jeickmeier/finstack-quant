//! Multi-input feature transforms.

use crate::types::{bool_param, finite, usize_param, validate_lengths, ZERO_TOLERANCE};
use crate::{transform_cross_sectional, CrossSectionalOp};
use finstack_quant_core::math::linalg::{cholesky_decomposition, cholesky_solve};
use finstack_quant_core::math::stats::{covariance, variance};
use finstack_quant_core::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Supported pairwise rolling time-series transform operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PairwiseOp {
    /// Rolling sample covariance between `values` and `other`.
    RollingCov,
    /// Rolling Pearson correlation between `values` and `other`.
    RollingCorr,
    /// Rolling beta of `values` to `other`.
    RollingBeta,
}

impl PairwiseOp {
    /// Return the canonical snake_case operation name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RollingCov => "rolling_cov",
            Self::RollingCorr => "rolling_corr",
            Self::RollingBeta => "rolling_beta",
        }
    }
}

impl FromStr for PairwiseOp {
    type Err = Error;

    fn from_str(op: &str) -> Result<Self> {
        match op {
            "rolling_cov" => Ok(Self::RollingCov),
            "rolling_corr" => Ok(Self::RollingCorr),
            "rolling_beta" => Ok(Self::RollingBeta),
            _ => Err(Error::Validation(format!(
                "unsupported pairwise time-series transform op '{op}'"
            ))),
        }
    }
}

/// Transform a cross-section within each `(time_key, group)` sub-partition.
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
    let grouped_key = time_key
        .iter()
        .zip(groups.iter())
        .map(|(time, group)| grouped_partition_key(time, group))
        .collect::<Vec<_>>();
    transform_cross_sectional(values, &grouped_key, op.as_str(), params)
}

fn grouped_partition_key(time: &str, group: &str) -> String {
    format!("{}:{time}{group}", time.len())
}

/// Remove cross-sectional exposure effects by OLS residualization per time key.
///
/// `exposures` is a slice of columns, each aligned to `values`. Parameters:
/// `fit_intercept` (default `true`).
///
/// # Errors
///
/// Returns a validation error when input lengths differ, exposure shapes are
/// malformed, or parameters are malformed.
pub fn neutralize(
    values: &[Option<f64>],
    time_key: &[String],
    exposures: &[Vec<Option<f64>>],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_exposures(values.len(), exposures)?;
    validate_lengths(values.len(), &[("time_key", time_key.len())])?;
    let fit_intercept = bool_param(params, "fit_intercept", true)?;
    let mut partitions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (idx, key) in time_key.iter().enumerate() {
        partitions.entry(key.as_str()).or_default().push(idx);
    }

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        residualize_partition(values, exposures, indices, fit_intercept, &mut output);
    }
    Ok(output)
}

/// Transform two value columns per entity with a rolling pairwise operation.
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
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    let required = min_periods.max(2);
    let mut output = vec![None; values.len()];
    for indices in entity_slices(entity, order) {
        for (pos, &idx) in indices.iter().enumerate() {
            let start = pos.saturating_sub(window - 1);
            let mut left = Vec::new();
            let mut right = Vec::new();
            for &window_idx in &indices[start..=pos] {
                if let (Some(y), Some(x)) = (finite(values[window_idx]), finite(other[window_idx]))
                {
                    left.push(y);
                    right.push(x);
                }
            }
            if left.len() < required {
                continue;
            }
            output[idx] = pairwise_value(&left, &right, op);
        }
    }
    Ok(output)
}

/// Return rolling OLS residuals per entity using aligned exposure columns.
///
/// Parameters: `window`, `min_periods` (default `window`), and `fit_intercept`
/// (default `true`).
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
    let window = usize_param(params, "window", 1)?;
    let min_periods = usize_param(params, "min_periods", window)?;
    let fit_intercept = bool_param(params, "fit_intercept", true)?;
    let mut output = vec![None; values.len()];
    for indices in entity_slices(entity, order) {
        for (pos, &idx) in indices.iter().enumerate() {
            let start = pos.saturating_sub(window - 1);
            let window_indices = &indices[start..=pos];
            if count_complete_rows(values, exposures, window_indices) < min_periods {
                continue;
            }
            let Some(beta) = fit_ols(values, exposures, window_indices, fit_intercept) else {
                continue;
            };
            output[idx] = residual_for_idx(values, exposures, idx, fit_intercept, &beta);
        }
    }
    Ok(output)
}

/// Convert a signal to inverse-risk-scaled weights per time key.
///
/// Finite rows are transformed as `signal / volatility`, then normalized so the
/// sum of absolute weights in each time partition is one.
///
/// # Errors
///
/// Returns a validation error when input lengths differ.
pub fn risk_scaled_weights(
    values: &[Option<f64>],
    time_key: &[String],
    volatility: &[Option<f64>],
    _params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    validate_lengths(
        values.len(),
        &[
            ("time_key", time_key.len()),
            ("volatility", volatility.len()),
        ],
    )?;
    let mut partitions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (idx, key) in time_key.iter().enumerate() {
        partitions.entry(key.as_str()).or_default().push(idx);
    }

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        let mut raw = Vec::new();
        let mut gross = 0.0;
        for &idx in indices {
            let value = match (finite(values[idx]), finite(volatility[idx])) {
                (Some(signal), Some(vol)) if vol.abs() > ZERO_TOLERANCE => Some(signal / vol),
                _ => None,
            };
            if let Some(weight) = value {
                gross += weight.abs();
            }
            raw.push((idx, value));
        }
        if gross <= ZERO_TOLERANCE {
            continue;
        }
        for (idx, value) in raw {
            if let Some(value) = value {
                output[idx] = Some(value / gross);
            }
        }
    }
    Ok(output)
}

/// Apply the default signal cleaning pass: cross-sectional quantile clipping.
///
/// Parameters are forwarded to `clip_by_quantile` (`lower`, `upper`).
///
/// # Errors
///
/// Returns a validation error when input lengths differ or clipping parameters
/// are malformed.
pub fn clean_signal(
    values: &[Option<f64>],
    time_key: &[String],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    transform_cross_sectional(values, time_key, "clip_by_quantile", params)
}

/// Normalize a signal cross-sectionally with a selected method.
///
/// `params.method` defaults to `zscore` and may name any single-column
/// cross-sectional operation.
///
/// # Errors
///
/// Returns a validation error when input lengths differ, the method is
/// unsupported, or operation parameters are malformed.
pub fn normalize_signal(
    values: &[Option<f64>],
    time_key: &[String],
    params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    let method = string_param(params, "method", "zscore")?;
    transform_cross_sectional(values, time_key, method, params)
}

/// Convert cross-sectional ranks into gross-normalized long/short weights.
///
/// # Errors
///
/// Returns a validation error when input lengths differ.
pub fn rank_to_weights(
    values: &[Option<f64>],
    time_key: &[String],
    _params: Option<&Value>,
) -> Result<Vec<Option<f64>>> {
    let ranks = transform_cross_sectional(values, time_key, "rank", None)?;
    demean_and_gross_normalize(&ranks, time_key)
}

/// Neutralize a signal against exposures and z-score the residuals.
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

fn string_param<'a>(params: Option<&'a Value>, key: &str, default: &'a str) -> Result<&'a str> {
    match params.and_then(|value| value.get(key)) {
        Some(value) => value.as_str().ok_or_else(|| {
            Error::Validation(format!(
                "panel transform parameter '{key}' must be a string"
            ))
        }),
        None => Ok(default),
    }
}

fn demean_and_gross_normalize(
    values: &[Option<f64>],
    time_key: &[String],
) -> Result<Vec<Option<f64>>> {
    validate_lengths(values.len(), &[("time_key", time_key.len())])?;
    let mut partitions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (idx, key) in time_key.iter().enumerate() {
        partitions.entry(key.as_str()).or_default().push(idx);
    }

    let mut output = vec![None; values.len()];
    for indices in partitions.values() {
        let finite_rows = indices
            .iter()
            .filter_map(|idx| finite(values[*idx]).map(|value| (*idx, value)))
            .collect::<Vec<_>>();
        if finite_rows.is_empty() {
            continue;
        }
        let mean =
            finite_rows.iter().map(|(_, value)| *value).sum::<f64>() / finite_rows.len() as f64;
        let gross = finite_rows
            .iter()
            .map(|(_, value)| (*value - mean).abs())
            .sum::<f64>();
        if gross <= ZERO_TOLERANCE {
            for (idx, _) in finite_rows {
                output[idx] = Some(0.0);
            }
            continue;
        }
        for (idx, value) in finite_rows {
            output[idx] = Some((value - mean) / gross);
        }
    }
    Ok(output)
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

fn entity_slices(entity: &[String], order: &[String]) -> Vec<Vec<usize>> {
    let mut indices = (0..entity.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        entity[*left]
            .cmp(&entity[*right])
            .then(order[*left].cmp(&order[*right]))
            .then(left.cmp(right))
    });

    let mut groups = Vec::new();
    let mut start = 0;
    while start < indices.len() {
        let mut end = start + 1;
        while end < indices.len() && entity[indices[end]] == entity[indices[start]] {
            end += 1;
        }
        groups.push(indices[start..end].to_vec());
        start = end;
    }
    groups
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
    indices: &[usize],
    fit_intercept: bool,
    output: &mut [Option<f64>],
) {
    let Some(beta) = fit_ols(values, exposures, indices, fit_intercept) else {
        return;
    };
    for &idx in indices {
        output[idx] = residual_for_idx(values, exposures, idx, fit_intercept, &beta);
    }
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
    let mut beta = vec![0.0; width];
    cholesky_solve(&chol, &rhs, &mut beta).ok()?;
    Some(beta)
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
