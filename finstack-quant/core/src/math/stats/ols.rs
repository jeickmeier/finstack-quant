//! Ordinary least-squares regression on a complete numeric panel.

use super::mean;
use crate::{Error, Result};
use nalgebra::{DMatrix, DVector, linalg::SVD};
use serde::{Deserialize, Serialize};

/// Fit of a full-rank linear regression with one residual per input row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OlsResult {
    /// Fitted slope per input feature, in column order.
    pub coefficients: Vec<f64>,
    /// Fitted constant term when the regression includes one.
    pub intercept: Option<f64>,
    /// Coefficient of determination, centered when a constant is fitted.
    pub r_squared: f64,
    /// Coefficient of determination adjusted for residual degrees of freedom.
    pub adj_r_squared: f64,
    /// Residual standard error in response units.
    pub residual_std_error: f64,
    /// Observed minus fitted response in input row order.
    pub residuals: Vec<f64>,
    /// Number of observations used in the fit.
    pub n_obs: usize,
    /// Number of slope coefficients, excluding the intercept.
    pub n_features: usize,
}

fn scaled_column(
    values: impl Iterator<Item = f64> + Clone,
    center: bool,
) -> Result<(Vec<f64>, f64, f64)> {
    let scale = values.clone().map(f64::abs).fold(0.0, f64::max);
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let mut scaled: Vec<_> = values.map(|x| x / scale).collect();
    let offset = if center {
        mean(&scaled)?.unwrap_or_default()
    } else {
        0.0
    };
    scaled.iter_mut().for_each(|value| *value -= offset);
    Ok((scaled, scale, offset))
}

/// Fit slopes and an optional intercept by singular value decomposition.
///
/// Both features and response are scaled internally. Output slopes,
/// intercept and residuals use the original input units.
///
/// # Arguments
/// * `x_row_major` - Finite predictors with observations in rows and
///   features in columns, flattened in row-major order.
/// * `n_obs` - Number of rows. Must exceed the total number of fitted coefficients.
/// * `n_features` - Positive number of predictor columns.
/// * `y` - One finite response for each row, in matching order.
/// * `add_intercept` - Fit an intercept and use centered total variation
///   for R². Otherwise fit through zero and use uncentered total variation.
///
/// # Errors
/// Rejects invalid dimensions, non-finite input, a rank-deficient design,
/// zero total response variation, or a fit that cannot be represented.
pub fn ols(
    x_row_major: &[f64],
    n_obs: usize,
    n_features: usize,
    y: &[f64],
    add_intercept: bool,
) -> Result<OlsResult> {
    let terms = n_features.saturating_add(usize::from(add_intercept));
    if n_features == 0
        || n_obs <= terms
        || n_obs != y.len()
        || n_obs.checked_mul(n_features) != Some(x_row_major.len())
    {
        return Err(Error::Validation("OLS requires a matching response and a rectangular design with positive residual degrees of freedom".into()));
    }
    if x_row_major.iter().chain(y).any(|value| !value.is_finite()) {
        return Err(Error::Validation("OLS inputs must be finite".into()));
    }
    let (target, y_scale, y_offset) = scaled_column(y.iter().copied(), add_intercept)?;
    let target = DVector::from_vec(target);
    let total_ss = target.norm_squared();
    if total_ss == 0.0 {
        return Err(Error::Validation(
            "OLS response total variation must be positive".into(),
        ));
    }
    let mut design = DMatrix::zeros(n_obs, n_features);
    let mut scales = Vec::with_capacity(n_features);
    let mut offsets = Vec::with_capacity(n_features);
    for col in 0..n_features {
        let (column, scale, offset) = scaled_column(
            x_row_major.chunks_exact(n_features).map(|row| row[col]),
            add_intercept,
        )?;
        design.set_column(col, &DVector::from_vec(column));
        scales.push(scale);
        offsets.push(offset);
    }

    let svd = SVD::try_new(design.clone(), true, true, f64::EPSILON, 1000).ok_or_else(|| {
        Error::Validation("OLS singular value decomposition did not converge".into())
    })?;
    let tolerance = 8.0 * f64::EPSILON * n_obs.max(n_features) as f64 * svd.singular_values[0];
    if svd.singular_values.iter().any(|value| *value <= tolerance) {
        return Err(Error::Validation("OLS design is rank deficient".into()));
    }
    let scaled_beta = svd
        .solve(&target, tolerance)
        .map_err(|error| Error::Validation(format!("OLS solve failed: {error}")))?;
    let scaled_errors = target - design * &scaled_beta;
    let rss = scaled_errors.norm_squared();
    let residual_dof = (n_obs - terms) as f64;
    let coefficients: Vec<_> = scaled_beta
        .iter()
        .zip(scales)
        .map(|(beta, scale)| (beta * y_scale) / scale)
        .collect();
    let intercept = if add_intercept {
        let contributions = scaled_beta
            .iter()
            .zip(offsets)
            .map(|(slope, offset)| slope * offset);
        Some((y_offset - super::super::summation::neumaier_sum(contributions)?) * y_scale)
    } else {
        None
    };
    let r_squared = 1.0 - rss / total_ss;
    let adj_r_squared =
        1.0 - (rss / total_ss) * ((n_obs - usize::from(add_intercept)) as f64 / residual_dof);
    let residual_std_error = (rss / residual_dof).sqrt() * y_scale;
    let residuals: Vec<_> = scaled_errors.iter().map(|value| value * y_scale).collect();
    if coefficients
        .iter()
        .chain(intercept.iter())
        .chain(residuals.iter())
        .chain([&r_squared, &adj_r_squared, &residual_std_error])
        .any(|value| !value.is_finite())
    {
        return Err(Error::Validation(
            "OLS fit cannot be represented with finite results".into(),
        ));
    }
    Ok(OlsResult {
        coefficients,
        intercept,
        r_squared,
        adj_r_squared,
        residual_std_error,
        residuals,
        n_obs,
        n_features,
    })
}
