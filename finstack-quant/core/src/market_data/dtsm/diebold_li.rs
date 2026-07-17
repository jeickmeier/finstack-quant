//! Diebold-Li (2006) dynamic Nelson-Siegel model.
//!
//! Extends the static Nelson-Siegel framework by treating the three factors
//! (level, slope, curvature) as time-varying and modeling their dynamics
//! via a VAR(1) process. Enables yield curve forecasting and scenario
//! generation.
//!
//! # Model
//!
//! Cross-section (measurement equation):
//! ```text
//! y(t, tau_i) = beta1(t) * 1
//!             + beta2(t) * ((1 - exp(-lambda * tau_i)) / (lambda * tau_i))
//!             + beta3(t) * ((1 - exp(-lambda * tau_i)) / (lambda * tau_i) - exp(-lambda * tau_i))
//!             + epsilon(t, tau_i)
//! ```
//!
//! Dynamics (transition equation):
//! ```text
//! beta(t+1) = mu + Phi * (beta(t) - mu) + eta(t)
//! ```
//!
//! where Phi is a 3x3 VAR(1) coefficient matrix and eta ~ N(0, Q).
//!
//! # References
//!
//! - Diebold, F. X., & Li, C. (2006). "Forecasting the Term Structure of
//!   Government Bond Yields." *Journal of Econometrics*, 130(2), 337-364.
//! - Diebold, F. X., & Rudebusch, G. D. (2013). *Yield Curve Modeling and
//!   Forecasting: The Dynamic Nelson-Siegel Approach*. Princeton UP.

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

use super::types::{FactorTimeSeries, YieldForecast, YieldPanel};

/// Default Diebold-Li decay parameter, expressed for tenors **in years** (the
/// workspace-wide tenor convention; see the `dtsm` module examples).
///
/// Diebold & Li (2006) use `λ = 0.0609` for maturities in **months**, which
/// maximizes the curvature loading at ≈30 months. Because `λ·τ` is
/// dimensionless, the years-equivalent is `12 × 0.0609 = 0.7308`, placing the
/// curvature peak at the same ≈2.45-year (≈30-month) maturity. Using the raw
/// months value with year tenors would mis-place the peak at ≈30 **years** and
/// distort the level/slope/curvature decomposition.
const DEFAULT_LAMBDA: f64 = 0.7308;

// ---------------------------------------------------------------------------
// DieboldLi
// ---------------------------------------------------------------------------

/// Diebold-Li (2006) dynamic Nelson-Siegel model.
///
/// See module-level documentation for model details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawDieboldLi")]
pub struct DieboldLi {
    /// Decay parameter lambda (fixed, not estimated).
    lambda: f64,
    /// Extracted factor time series (populated after `extract_factors`).
    factors: Option<FactorTimeSeries>,
    /// VAR(1) intercept vector mu (3x1), populated after `fit_var`.
    mu: Option<DVector<f64>>,
    /// VAR(1) coefficient matrix Phi (3x3), populated after `fit_var`.
    phi: Option<DMatrix<f64>>,
    /// VAR(1) residual covariance Q (3x3), populated after `fit_var`.
    q_cov: Option<DMatrix<f64>>,
    /// Tenor grid from the input panel.
    tenors: Vec<f64>,
}

/// Raw deserialization state of [`DieboldLi`].
///
/// Mirrors the serialized field layout exactly; conversion re-applies the
/// builder's lambda validation and rejects unknown fields.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDieboldLi {
    /// Decay parameter lambda.
    lambda: f64,
    /// Extracted factor time series.
    factors: Option<FactorTimeSeries>,
    /// VAR(1) intercept vector mu.
    mu: Option<DVector<f64>>,
    /// VAR(1) coefficient matrix Phi.
    phi: Option<DMatrix<f64>>,
    /// VAR(1) residual covariance Q.
    q_cov: Option<DMatrix<f64>>,
    /// Tenor grid from the input panel.
    tenors: Vec<f64>,
}

impl TryFrom<RawDieboldLi> for DieboldLi {
    type Error = crate::Error;

    fn try_from(raw: RawDieboldLi) -> crate::Result<Self> {
        if !raw.lambda.is_finite() || raw.lambda <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Lambda must be positive and finite, got {}",
                raw.lambda
            )));
        }
        Ok(Self {
            lambda: raw.lambda,
            factors: raw.factors,
            mu: raw.mu,
            phi: raw.phi,
            q_cov: raw.q_cov,
            tenors: raw.tenors,
        })
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`DieboldLi`].
pub struct DieboldLiBuilder {
    lambda: f64,
}

impl DieboldLiBuilder {
    /// Set the decay parameter lambda.
    ///
    /// Default: `0.7308` for tenors in **years** (the years-equivalent of
    /// Diebold-Li's canonical `0.0609` months value; curvature loading peaks at
    /// ≈2.45 years ≈ 30 months). Pass a months-scaled lambda only if you also
    /// supply tenors in months.
    #[must_use]
    pub fn lambda(mut self, lambda: f64) -> Self {
        self.lambda = lambda;
        self
    }

    /// Build the model.
    ///
    /// # Errors
    /// - lambda <= 0 or non-finite
    pub fn build(self) -> crate::Result<DieboldLi> {
        if !self.lambda.is_finite() || self.lambda <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Lambda must be positive and finite, got {}",
                self.lambda
            )));
        }
        Ok(DieboldLi {
            lambda: self.lambda,
            factors: None,
            mu: None,
            phi: None,
            q_cov: None,
            tenors: Vec::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl DieboldLi {
    /// Create a builder with default lambda = 0.7308 (years convention).
    #[must_use]
    pub fn builder() -> DieboldLiBuilder {
        DieboldLiBuilder {
            lambda: DEFAULT_LAMBDA,
        }
    }

    /// Extract time-varying Nelson-Siegel factors from a yield panel via OLS.
    ///
    /// For each date t, solves the cross-sectional regression:
    ///   y(t) = X * beta(t) + epsilon(t)
    ///
    /// where X is the N x 3 NS loading matrix (fixed given lambda and tenors).
    ///
    /// # Errors
    /// - Panel has fewer than 3 tenors (underdetermined system)
    /// - OLS system is singular
    pub fn extract_factors(mut self, panel: &YieldPanel) -> crate::Result<Self> {
        let n = panel.num_tenors();
        let t = panel.num_dates();

        if n < 3 {
            return Err(crate::Error::Validation(format!(
                "Need at least 3 tenors for factor extraction, got {n}"
            )));
        }

        self.tenors = panel.tenors.clone();

        // Build N x 3 NS loading matrix
        let x = ns_loading_matrix(self.lambda, &self.tenors);

        // Compute (X'X)^{-1} X' once (pseudoinverse for OLS)
        let xt = x.transpose();
        let xtx = &xt * &x;

        // Solve via Cholesky: (X'X) is 3x3 symmetric positive definite
        let chol = xtx.cholesky().ok_or_else(|| {
            crate::Error::Validation(
                "NS loading matrix is singular -- check lambda and tenor grid".into(),
            )
        })?;

        // Factor and residual storage
        let mut factors = DMatrix::zeros(t, 3);
        let mut residuals = DMatrix::zeros(t, n);
        let mut ss_res = vec![0.0_f64; n];
        let mut ss_tot = vec![0.0_f64; n];

        // Column means for R-squared computation
        let mut col_means = vec![0.0_f64; n];
        for (j, mean) in col_means.iter_mut().enumerate() {
            let mut sum = 0.0;
            for i in 0..t {
                sum += panel.yields[(i, j)];
            }
            *mean = sum / t as f64;
        }

        // Extract factors for each date via OLS: beta(t) = (X'X)^{-1} X' y(t)
        for i in 0..t {
            let y_row = panel.yields.row(i).transpose();
            let rhs = &xt * &y_row;
            let beta = chol.solve(&rhs);

            for k in 0..3 {
                factors[(i, k)] = beta[k];
            }

            // Residuals: e(t) = y(t) - X * beta(t)
            let fitted = &x * &beta;
            for j in 0..n {
                let res = y_row[j] - fitted[j];
                residuals[(i, j)] = res;
                ss_res[j] += res * res;
                let dev = y_row[j] - col_means[j];
                ss_tot[j] += dev * dev;
            }
        }

        // R-squared per tenor
        let r_squared: Vec<f64> = ss_res
            .iter()
            .zip(ss_tot.iter())
            .map(|(res, tot)| {
                if *tot < 1e-30 {
                    1.0 // constant series -> perfect fit
                } else {
                    1.0 - res / tot
                }
            })
            .collect();

        let r_squared_avg = r_squared.iter().sum::<f64>() / r_squared.len() as f64;

        self.factors = Some(FactorTimeSeries {
            factors,
            residuals,
            r_squared,
            r_squared_avg,
        });

        Ok(self)
    }

    /// Fit VAR(1) dynamics to the extracted factors.
    ///
    /// Estimates: beta(t+1) = c + Phi * beta(t) + eta(t)
    ///
    /// Then derives the unconditional mean: mu = (I - Phi)^{-1} c
    ///
    /// Must be called after `extract_factors`.
    ///
    /// # Errors
    /// - Factors not yet extracted
    /// - Fewer than 5 observations (insufficient for VAR estimation)
    pub fn fit_var(mut self) -> crate::Result<Self> {
        let fts = self.factors.as_ref().ok_or_else(|| {
            crate::Error::Validation("Factors not extracted -- call extract_factors first".into())
        })?;

        let t = fts.factors.nrows();
        if t < 5 {
            return Err(crate::Error::Validation(format!(
                "Need at least 5 factor observations for VAR(1), got {t}"
            )));
        }

        // Build Y (dependent, T-1 x 3) and Z (lagged, T-1 x 4 with intercept)
        let n_obs = t - 1;
        let mut y_mat = DMatrix::zeros(n_obs, 3);
        let mut z_mat = DMatrix::zeros(n_obs, 4); // [1, beta1(t), beta2(t), beta3(t)]

        for i in 0..n_obs {
            for k in 0..3 {
                y_mat[(i, k)] = fts.factors[(i + 1, k)];
                z_mat[(i, k + 1)] = fts.factors[(i, k)];
            }
            z_mat[(i, 0)] = 1.0; // intercept
        }

        // OLS: [c, Phi'] = (Z'Z)^{-1} Z'Y  =>  B = (Z'Z)^{-1} Z'Y  (4 x 3)
        let zt = z_mat.transpose();
        let ztz = &zt * &z_mat;
        let chol = ztz
            .cholesky()
            .ok_or_else(|| crate::Error::Validation("VAR(1) design matrix is singular".into()))?;
        let zty = &zt * &y_mat;

        // Solve column-by-column
        let mut b_hat = DMatrix::zeros(4, 3);
        for k in 0..3 {
            let col = zty.column(k).into_owned();
            let sol = chol.solve(&col);
            for j in 0..4 {
                b_hat[(j, k)] = sol[j];
            }
        }

        // Extract intercept c (3x1) and Phi (3x3)
        let c = DVector::from_fn(3, |k, _| b_hat[(0, k)]);
        let phi = DMatrix::from_fn(3, 3, |row, col| b_hat[(col + 1, row)]);

        // Compute unconditional mean: mu = (I - Phi)^{-1} c
        let eye3 = DMatrix::identity(3, 3);
        let i_minus_phi = &eye3 - &phi;
        let mu = i_minus_phi.try_inverse().map(|inv| &inv * &c).unwrap_or(c); // fallback to c if non-invertible (unit root)

        // Residual covariance Q
        let mut residuals = DMatrix::zeros(n_obs, 3);
        for i in 0..n_obs {
            let z_row = z_mat.row(i).transpose();
            for k in 0..3 {
                let fitted: f64 = (0..4).map(|j| b_hat[(j, k)] * z_row[j]).sum();
                residuals[(i, k)] = y_mat[(i, k)] - fitted;
            }
        }
        let rt = residuals.transpose();
        let q_cov = (&rt * &residuals) / (n_obs as f64 - 4.0).max(1.0);

        self.mu = Some(mu);
        self.phi = Some(phi);
        self.q_cov = Some(q_cov);

        Ok(self)
    }

    /// Forecast the yield curve h steps ahead.
    ///
    /// Uses the VAR(1) dynamics to iterate factor forecasts forward,
    /// then converts back to yields using the NS loading matrix.
    ///
    /// Confidence bands are computed from the h-step forecast error
    /// covariance: Sigma_h = sum_{j=0}^{h-1} Phi^j * Q * (Phi^j)'.
    ///
    /// # Errors
    /// - VAR not yet fitted
    /// - horizon == 0
    pub fn forecast(&self, horizon: usize) -> crate::Result<YieldForecast> {
        if horizon == 0 {
            return Err(crate::Error::Validation(
                "Forecast horizon must be >= 1".into(),
            ));
        }

        let mu = self.mu.as_ref().ok_or_else(|| {
            crate::Error::Validation("VAR not fitted -- call fit_var first".into())
        })?;
        let phi = self.phi.as_ref().ok_or_else(|| {
            crate::Error::Validation("VAR not fitted -- call fit_var first".into())
        })?;
        let q = self.q_cov.as_ref().ok_or_else(|| {
            crate::Error::Validation("VAR not fitted -- call fit_var first".into())
        })?;
        let fts = self
            .factors
            .as_ref()
            .ok_or_else(|| crate::Error::Validation("Factors not extracted".into()))?;

        let t = fts.factors.nrows();
        // Last observed factor vector
        let last_beta = DVector::from_fn(3, |k, _| fts.factors[(t - 1, k)]);

        // Iterate forecast: beta_hat(t+h) = mu + Phi^h * (beta(t) - mu)
        let dev = &last_beta - mu;
        let mut phi_power = DMatrix::identity(3, 3);
        for _ in 0..horizon {
            phi_power = phi * &phi_power;
        }
        let forecast_beta = mu + &phi_power * &dev;

        // h-step forecast error covariance: Sigma_h = sum_{j=0}^{h-1} Phi^j Q (Phi^j)'
        let mut sigma_h = DMatrix::zeros(3, 3);
        let mut phi_j = DMatrix::identity(3, 3);
        for _ in 0..horizon {
            sigma_h += &phi_j * q * phi_j.transpose();
            phi_j = phi * &phi_j;
        }

        // Convert factor forecast to yields
        let x = ns_loading_matrix(self.lambda, &self.tenors);
        let n = self.tenors.len();

        let factor_vec = DVector::from_fn(3, |k, _| forecast_beta[k]);
        let yield_vec = &x * &factor_vec;

        let mut yields = vec![0.0; n];
        let mut lower_95 = vec![0.0; n];
        let mut upper_95 = vec![0.0; n];

        for i in 0..n {
            yields[i] = yield_vec[i];

            // Yield variance at tenor i: loading_i' * Sigma_h * loading_i
            let loading = x.row(i).transpose();
            let var_i = (loading.transpose() * &sigma_h * &loading)[(0, 0)];
            let std_i = var_i.max(0.0).sqrt();

            // 95% confidence band (z = 1.96)
            lower_95[i] = yield_vec[i] - 1.96 * std_i;
            upper_95[i] = yield_vec[i] + 1.96 * std_i;
        }

        Ok(YieldForecast {
            horizon,
            yields,
            tenors: self.tenors.clone(),
            factors: [forecast_beta[0], forecast_beta[1], forecast_beta[2]],
            lower_95,
            upper_95,
        })
    }

    /// Access the extracted factor time series.
    #[must_use]
    pub fn factors(&self) -> Option<&FactorTimeSeries> {
        self.factors.as_ref()
    }

    /// Access the VAR(1) coefficient matrix Phi.
    #[must_use]
    pub fn phi(&self) -> Option<&DMatrix<f64>> {
        self.phi.as_ref()
    }

    /// Access the VAR(1) intercept mu.
    #[must_use]
    pub fn mu(&self) -> Option<&DVector<f64>> {
        self.mu.as_ref()
    }

    /// Access the VAR(1) residual covariance Q.
    #[must_use]
    pub fn q_cov(&self) -> Option<&DMatrix<f64>> {
        self.q_cov.as_ref()
    }

    /// Lambda value used for this model.
    #[must_use]
    pub fn lambda(&self) -> f64 {
        self.lambda
    }

    /// Convert forecast yields to a `ParametricCurve` by fitting NS parameters
    /// to the forecast point estimates.
    ///
    /// Since the forecast is already NS-shaped (by construction), this is a
    /// near-exact fit that reuses the forecast factor values directly.
    ///
    /// `forecast.factors` become the Nelson-Siegel level, slope, and curvature
    /// loadings without refitting the point yields. The decay horizon is
    /// `tau = 1 / self.lambda`; the resulting curve uses the parametric
    /// builder's default Act/365F day count unless callers construct a curve
    /// directly with a different convention. `forecast.yields` and confidence
    /// bands are not used by this conversion.
    ///
    /// # Errors
    ///
    /// Returns an error propagated by the parametric-curve builder when the
    /// derived Nelson-Siegel model is invalid, including a non-positive decay
    /// horizon. Callers should fit a positive, finite `lambda` before invoking
    /// this conversion.
    pub fn to_parametric_curve(
        &self,
        id: impl Into<crate::types::CurveId>,
        base_date: crate::dates::Date,
        forecast: &YieldForecast,
    ) -> crate::Result<crate::market_data::term_structures::ParametricCurve> {
        use crate::market_data::term_structures::NelsonSiegelModel;

        // The forecast factors are already NS parameters; tau = 1/lambda
        let model = NelsonSiegelModel::Ns {
            beta0: forecast.factors[0],
            beta1: forecast.factors[1],
            beta2: forecast.factors[2],
            tau: 1.0 / self.lambda,
        };

        crate::market_data::term_structures::ParametricCurve::builder(id)
            .base_date(base_date)
            .model(model)
            .build()
    }

    /// Nelson-Siegel loading matrix for the current lambda and tenor grid.
    ///
    /// Returns an N x 3 matrix where column 0 = level loading (all 1s),
    /// column 1 = slope loading, column 2 = curvature loading.
    #[must_use]
    pub fn loading_matrix(&self) -> DMatrix<f64> {
        ns_loading_matrix(self.lambda, &self.tenors)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the N x 3 Nelson-Siegel loading matrix for a given lambda and tenor grid.
///
/// Column 0: level = 1
/// Column 1: slope = (1 - exp(-lambda * tau)) / (lambda * tau)
/// Column 2: curvature = slope_loading - exp(-lambda * tau)
pub(crate) fn ns_loading_matrix(lambda: f64, tenors: &[f64]) -> DMatrix<f64> {
    let n = tenors.len();
    let mut x = DMatrix::zeros(n, 3);
    for (i, &tau) in tenors.iter().enumerate() {
        let lt = lambda * tau;
        let exp_lt = (-lt).exp();
        let slope = if lt.abs() < 1e-10 {
            1.0 // limit as lt -> 0
        } else {
            (1.0 - exp_lt) / lt
        };
        x[(i, 0)] = 1.0;
        x[(i, 1)] = slope;
        x[(i, 2)] = slope - exp_lt;
    }
    x
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate synthetic yields from known NS parameters.
    fn make_ns_yields(beta0: f64, beta1: f64, beta2: f64, lambda: f64, tenors: &[f64]) -> Vec<f64> {
        tenors
            .iter()
            .map(|&tau| {
                let lt = lambda * tau;
                let exp_lt = (-lt).exp();
                let slope = (1.0 - exp_lt) / lt;
                beta0 + beta1 * slope + beta2 * (slope - exp_lt)
            })
            .collect()
    }

    fn standard_tenors() -> Vec<f64> {
        vec![0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0]
    }

    #[test]
    fn builder_default_lambda() {
        let model = DieboldLi::builder().build().unwrap();
        // Years-convention default (12 × Diebold-Li's 0.0609 months value).
        assert!((model.lambda() - 0.7308).abs() < 1e-10);
    }

    #[test]
    fn builder_custom_lambda() {
        let model = DieboldLi::builder().lambda(0.05).build().unwrap();
        assert!((model.lambda() - 0.05).abs() < 1e-10);
    }

    #[test]
    fn builder_invalid_lambda_rejected() {
        assert!(DieboldLi::builder().lambda(0.0).build().is_err());
        assert!(DieboldLi::builder().lambda(-0.1).build().is_err());
        assert!(DieboldLi::builder().lambda(f64::NAN).build().is_err());
    }

    #[test]
    fn extract_factors_recovers_known_ns() {
        let lambda = 0.0609;
        let tenors = standard_tenors();
        let beta0 = 0.06;
        let beta1 = -0.02;
        let beta2 = 0.01;

        // Generate a panel with constant factors (20 dates, same curve)
        let yields_row = make_ns_yields(beta0, beta1, beta2, lambda, &tenors);
        let n = tenors.len();
        let t = 20;
        let mut data = DMatrix::zeros(t, n);
        for i in 0..t {
            for j in 0..n {
                data[(i, j)] = yields_row[j];
            }
        }

        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let model = DieboldLi::builder()
            .lambda(lambda)
            .build()
            .unwrap()
            .extract_factors(&panel)
            .unwrap();

        let fts = model.factors().unwrap();

        // All extracted betas should match input
        for i in 0..t {
            assert!(
                (fts.factors[(i, 0)] - beta0).abs() < 1e-10,
                "beta0 mismatch at date {i}"
            );
            assert!(
                (fts.factors[(i, 1)] - beta1).abs() < 1e-10,
                "beta1 mismatch at date {i}"
            );
            assert!(
                (fts.factors[(i, 2)] - beta2).abs() < 1e-10,
                "beta2 mismatch at date {i}"
            );
        }

        // Residuals should be effectively zero for clean NS data
        for i in 0..t {
            for j in 0..n {
                assert!(
                    fts.residuals[(i, j)].abs() < 1e-10,
                    "Non-zero residual at ({i}, {j}): {}",
                    fts.residuals[(i, j)]
                );
            }
        }

        // R-squared should be 1.0 (within tolerance) for pure NS data
        assert!(fts.r_squared_avg > 0.999);
    }

    #[test]
    fn extract_factors_too_few_tenors() {
        let data = DMatrix::from_row_slice(3, 2, &[0.01, 0.02, 0.01, 0.02, 0.01, 0.02]);
        let panel = YieldPanel::new(data, vec![1.0, 2.0], None).unwrap();
        let model = DieboldLi::builder().build().unwrap();
        assert!(model.extract_factors(&panel).is_err());
    }

    #[test]
    fn loading_matrix_shape_and_level() {
        let model = DieboldLi::builder().build().unwrap();
        let tenors = standard_tenors();
        let panel = {
            let n = tenors.len();
            let data = DMatrix::from_fn(5, n, |_, j| 0.03 + 0.001 * j as f64);
            YieldPanel::new(data, tenors, None).unwrap()
        };
        let model = model.extract_factors(&panel).unwrap();
        let x = model.loading_matrix();

        // Column 0 should be all 1s (level loading)
        for i in 0..x.nrows() {
            assert!((x[(i, 0)] - 1.0).abs() < 1e-15);
        }

        // Column 1 (slope) should be decreasing
        for i in 1..x.nrows() {
            assert!(x[(i, 1)] <= x[(i - 1, 1)] + 1e-15);
        }
    }

    #[test]
    fn var_fit_and_forecast_basic() {
        let lambda = 0.0609;
        let tenors = standard_tenors();
        let n = tenors.len();
        let t = 50;

        // Generate slowly evolving factors
        let mut data = DMatrix::zeros(t, n);
        for i in 0..t {
            let b0 = 0.06 + 0.001 * (i as f64 / t as f64);
            let b1 = -0.02;
            let b2 = 0.01;
            let row = make_ns_yields(b0, b1, b2, lambda, &tenors);
            for j in 0..n {
                data[(i, j)] = row[j];
            }
        }

        let panel = YieldPanel::new(data, tenors.clone(), None).unwrap();
        let model = DieboldLi::builder()
            .lambda(lambda)
            .build()
            .unwrap()
            .extract_factors(&panel)
            .unwrap()
            .fit_var()
            .unwrap();

        // VAR should have mu, phi, q_cov populated
        assert!(model.mu().is_some());
        assert!(model.phi().is_some());
        assert!(model.q_cov().is_some());

        // Forecast should produce valid yields
        let fc = model.forecast(1).unwrap();
        assert_eq!(fc.yields.len(), tenors.len());
        assert_eq!(fc.tenors.len(), tenors.len());

        // Confidence bands should widen with horizon
        let fc1 = model.forecast(1).unwrap();
        let fc12 = model.forecast(12).unwrap();
        let width1: f64 = fc1
            .upper_95
            .iter()
            .zip(fc1.lower_95.iter())
            .map(|(u, l)| u - l)
            .sum();
        let width12: f64 = fc12
            .upper_95
            .iter()
            .zip(fc12.lower_95.iter())
            .map(|(u, l)| u - l)
            .sum();
        assert!(
            width12 >= width1,
            "12-step band should be wider than 1-step"
        );
    }

    #[test]
    fn var_forecast_horizon_zero_rejected() {
        let lambda = 0.0609;
        let tenors = standard_tenors();
        let n = tenors.len();
        let t = 50;
        let mut data = DMatrix::zeros(t, n);
        for i in 0..t {
            let b0 = 0.06 + 0.002 * ((i as f64) * 0.3).sin();
            let b1 = -0.02 + 0.001 * ((i as f64) * 0.5).cos();
            let b2 = 0.01 + 0.001 * ((i as f64) * 0.7).sin();
            let row = make_ns_yields(b0, b1, b2, lambda, &tenors);
            for j in 0..n {
                data[(i, j)] = row[j];
            }
        }
        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let model = DieboldLi::builder()
            .lambda(lambda)
            .build()
            .unwrap()
            .extract_factors(&panel)
            .unwrap()
            .fit_var()
            .unwrap();
        assert!(model.forecast(0).is_err());
    }

    #[test]
    fn forecast_without_var_rejected() {
        let tenors = standard_tenors();
        let n = tenors.len();
        let data = DMatrix::from_fn(20, n, |_, j| 0.03 + 0.001 * j as f64);
        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let model = DieboldLi::builder()
            .build()
            .unwrap()
            .extract_factors(&panel)
            .unwrap();
        // No fit_var called
        assert!(model.forecast(1).is_err());
    }

    #[test]
    fn to_parametric_curve_roundtrip() {
        let lambda = 0.0609;
        let tenors = standard_tenors();
        let n = tenors.len();
        let t = 50;
        let mut data = DMatrix::zeros(t, n);
        for i in 0..t {
            let b0 = 0.06 + 0.002 * ((i as f64) * 0.3).sin();
            let b1 = -0.02 + 0.001 * ((i as f64) * 0.5).cos();
            let b2 = 0.01 + 0.001 * ((i as f64) * 0.7).sin();
            let row = make_ns_yields(b0, b1, b2, lambda, &tenors);
            for j in 0..n {
                data[(i, j)] = row[j];
            }
        }
        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let model = DieboldLi::builder()
            .lambda(lambda)
            .build()
            .unwrap()
            .extract_factors(&panel)
            .unwrap()
            .fit_var()
            .unwrap();

        let fc = model.forecast(1).unwrap();
        let base_date =
            crate::dates::Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = model
            .to_parametric_curve("USD-FORECAST", base_date, &fc)
            .unwrap();

        // The parametric curve should produce rates close to forecast
        for (i, &tau) in fc.tenors.iter().enumerate() {
            let curve_rate = curve.zero_rate(tau);
            assert!(
                (curve_rate - fc.yields[i]).abs() < 0.005,
                "Mismatch at tenor {tau}: curve={curve_rate}, forecast={}",
                fc.yields[i]
            );
        }
    }
}
