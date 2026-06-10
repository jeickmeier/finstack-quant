//! Finite-difference SABR derivatives for calibration.
//!
//! Supplies SABR implied volatility together with central finite-difference
//! gradients with respect to the model parameters (alpha, nu, rho), for the
//! Levenberg-Marquardt calibrator.
//!
//! Volatilities are evaluated through [`SABRModel::implied_volatility`] — the
//! production Hagan (2002) implementation — and each parameter gradient is a
//! central finite difference of that same function. Computing the gradient
//! from the identical volatility routine the calibration objective uses keeps
//! the two exactly consistent and avoids the accuracy pitfalls of
//! hand-derived Hagan-expansion gradients.

use super::sabr::{vega_weight, SABRModel, SABRParameters};
use finstack_core::math::solver_multi::AnalyticalDerivatives;
use finstack_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Market data for SABR calibration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SABRMarketData {
    /// Forward price
    pub forward: f64,
    /// Time to expiry
    pub time_to_expiry: f64,
    /// Strike prices
    pub strikes: Vec<f64>,
    /// Market implied volatilities
    pub market_vols: Vec<f64>,
    /// Fixed beta parameter
    pub beta: f64,
    /// Optional shift for handling negative rates in lognormal SABR (beta ≈ 1.0)
    /// Default: 0.02 (200 basis points) if None and rates are negative
    pub shift: Option<f64>,
}

impl SABRMarketData {
    /// Construct market data for SABR calibration with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are inconsistent (e.g. mismatched lengths) or out of range.
    pub fn new(
        forward: f64,
        time_to_expiry: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        beta: f64,
    ) -> Result<Self> {
        if forward <= 0.0 {
            return Err(Error::Validation(format!(
                "SABRMarketData invalid: forward must be positive, got {}",
                forward
            )));
        }
        if time_to_expiry <= 0.0 {
            return Err(Error::Validation(format!(
                "SABRMarketData invalid: time_to_expiry must be positive, got {}",
                time_to_expiry
            )));
        }
        if strikes.is_empty() {
            return Err(Error::Validation(
                "SABRMarketData invalid: strikes cannot be empty".to_string(),
            ));
        }
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABRMarketData invalid: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }
        if !(0.0..=1.0).contains(&beta) {
            return Err(Error::Validation(format!(
                "SABRMarketData invalid: beta must be in [0, 1], got {}",
                beta
            )));
        }

        Ok(Self {
            forward,
            time_to_expiry,
            strikes,
            market_vols,
            beta,
            shift: None,
        })
    }

    /// Same as `new` but allows setting an explicit shift.
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are invalid, or if `shift` is not positive.
    pub fn new_with_shift(
        forward: f64,
        time_to_expiry: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        beta: f64,
        shift: f64,
    ) -> Result<Self> {
        if shift <= 0.0 {
            return Err(Error::Validation(format!(
                "SABRMarketData invalid: shift must be positive, got {}",
                shift
            )));
        }
        let mut md = Self::new(forward, time_to_expiry, strikes, market_vols, beta)?;
        md.shift = Some(shift);
        Ok(md)
    }
}

/// Finite-difference SABR derivatives provider for calibration.
///
/// Supplies SABR implied volatility and its parameter gradient
/// (∂σ/∂α, ∂σ/∂ν, ∂σ/∂ρ) to the Levenberg-Marquardt calibrator. Volatilities
/// come from [`SABRModel::implied_volatility`]; each gradient component is a
/// central finite difference of that same routine, and the gradient carries
/// the same per-strike vega weights as the calibration objective, so the two
/// are exactly consistent.
pub struct SABRCalibrationDerivatives {
    market_data: SABRMarketData,
    /// Per-strike vega weights matching the calibration objective
    /// `Σ w·(σ_model − σ_market)²`. They depend only on the (fixed) market
    /// data, so they are computed once at construction. Any configured shift
    /// is applied to forward/strike, mirroring `sabr_vol_fd`.
    weights: Vec<f64>,
}

impl SABRCalibrationDerivatives {
    /// Create a new SABR derivatives provider.
    pub fn new(market_data: SABRMarketData) -> Self {
        let shift = market_data.shift.unwrap_or(0.0);
        let weights = market_data
            .strikes
            .iter()
            .zip(market_data.market_vols.iter())
            .map(|(&strike, &market_vol)| {
                vega_weight(
                    market_data.forward + shift,
                    strike + shift,
                    market_vol,
                    market_data.time_to_expiry,
                    market_data.beta,
                )
            })
            .collect();
        Self {
            market_data,
            weights,
        }
    }

    /// Compute SABR implied volatility and its parameter derivatives.
    ///
    /// Returns `(vol, ∂vol/∂α, ∂vol/∂ν, ∂vol/∂ρ)`. The volatility is the
    /// [`SABRModel`] Hagan value; the three derivatives are central finite
    /// differences of that volatility with a `1e-6` parameter step, so they
    /// are consistent with the calibration objective.
    fn sabr_vol_and_derivatives(
        &self,
        strike: f64,
        alpha: f64,
        nu: f64,
        rho: f64,
    ) -> (f64, f64, f64, f64) {
        let base_vol = self.sabr_vol_fd(strike, alpha, nu, rho);

        // Central finite differences of the SABRModel volatility.
        let eps = 1e-6;
        let d_vol_d_alpha = (self.sabr_vol_fd(strike, alpha + eps, nu, rho)
            - self.sabr_vol_fd(strike, alpha - eps, nu, rho))
            / (2.0 * eps);
        let d_vol_d_nu = (self.sabr_vol_fd(strike, alpha, nu + eps, rho)
            - self.sabr_vol_fd(strike, alpha, nu - eps, rho))
            / (2.0 * eps);
        let d_vol_d_rho = (self.sabr_vol_fd(strike, alpha, nu, rho + eps)
            - self.sabr_vol_fd(strike, alpha, nu, rho - eps))
            / (2.0 * eps);

        (base_vol, d_vol_d_alpha, d_vol_d_nu, d_vol_d_rho)
    }

    /// Evaluate SABR implied volatility via [`SABRModel`].
    ///
    /// Honors any `shift` configured on the market data, so shifted
    /// (negative-rate) calibrations price with the same effective
    /// forward/strike the model uses. Returns `0.0` for parameter triples
    /// [`SABRModel`] rejects; the least-squares objective then sees a large
    /// residual at that point.
    fn sabr_vol_fd(&self, strike: f64, alpha: f64, nu: f64, rho: f64) -> f64 {
        let beta = self.market_data.beta;
        let params_result = match self.market_data.shift {
            Some(shift) => SABRParameters::new_with_shift(alpha, beta, nu, rho, shift),
            None => SABRParameters::new(alpha, beta, nu, rho),
        };
        let params = match params_result {
            Ok(p) => p,
            Err(_) => return 0.0, // Invalid parameter triple — large residual.
        };

        SABRModel::new(params)
            .implied_volatility(
                self.market_data.forward,
                strike,
                self.market_data.time_to_expiry,
            )
            .unwrap_or(0.0)
    }
}

impl AnalyticalDerivatives for SABRCalibrationDerivatives {
    fn gradient(&self, params: &[f64], gradient: &mut [f64]) {
        // params = [alpha, nu, rho]
        if params.len() != 3 || gradient.len() != 3 {
            return;
        }

        let alpha = params[0];
        let nu = params[1];
        let rho = params[2];

        gradient[0] = 0.0;
        gradient[1] = 0.0;
        gradient[2] = 0.0;

        // Gradient of the vega-weighted least-squares objective
        // Σ w·(model_vol − market_vol)² — the same weighting the
        // calibration objective applies (see `vega_weight`).
        for (i, &strike) in self.market_data.strikes.iter().enumerate() {
            let (model_vol, d_alpha, d_nu, d_rho) =
                self.sabr_vol_and_derivatives(strike, alpha, nu, rho);

            let market_vol = self.market_data.market_vols[i];
            let residual = model_vol - market_vol;
            let w = self.weights[i];

            gradient[0] += 2.0 * w * residual * d_alpha;
            gradient[1] += 2.0 * w * residual * d_nu;
            gradient[2] += 2.0 * w * residual * d_rho;
        }
    }

    fn has_gradient(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sabr_derivatives_atm() {
        let market_data = SABRMarketData {
            forward: 100.0,
            time_to_expiry: 1.0,
            strikes: vec![100.0], // ATM
            market_vols: vec![0.20],
            beta: 0.5,
            shift: None,
        };

        let deriv_provider = SABRCalibrationDerivatives::new(market_data);

        // Test at some reasonable parameter values
        let params = vec![0.15, 0.3, -0.1]; // alpha, nu, rho
        let mut gradient = vec![0.0; 3];

        deriv_provider.gradient(&params, &mut gradient);

        // Gradient should be finite and reasonable
        assert!(gradient[0].is_finite());
        assert!(gradient[1].is_finite());
        assert!(gradient[2].is_finite());
    }

    #[test]
    fn test_gradient_finite_differences() {
        let market_data = SABRMarketData {
            forward: 100.0,
            time_to_expiry: 1.0,
            strikes: vec![90.0, 100.0, 110.0],
            market_vols: vec![0.22, 0.20, 0.21],
            beta: 0.5,
            shift: None,
        };

        let deriv_provider = SABRCalibrationDerivatives::new(market_data.clone());

        // Provider gradient of the vega-weighted least-squares objective.
        let params = vec![0.15, 0.3, -0.1];
        let mut provider_grad = vec![0.0; 3];
        deriv_provider.gradient(&params, &mut provider_grad);

        // Numerical gradient of the same vega-weighted objective, built
        // directly from the SABRModel volatilities.
        let eps = 1e-6;
        let mut numerical_grad = [0.0; 3];

        let objective = |p: &[f64]| -> f64 {
            let alpha = p[0];
            let nu = p[1];
            let rho = p[2];

            let mut sum_sq = 0.0;
            for (i, &strike) in market_data.strikes.iter().enumerate() {
                let model_vol = deriv_provider.sabr_vol_fd(strike, alpha, nu, rho);
                let residual = model_vol - market_data.market_vols[i];
                sum_sq += deriv_provider.weights[i] * residual * residual;
            }
            sum_sq
        };

        for i in 0..3 {
            let mut params_plus = params.clone();
            let mut params_minus = params.clone();
            params_plus[i] += eps;
            params_minus[i] -= eps;

            numerical_grad[i] = (objective(&params_plus) - objective(&params_minus)) / (2.0 * eps);
        }

        for i in 0..3 {
            let abs_diff = (provider_grad[i] - numerical_grad[i]).abs();
            let rel_error = if numerical_grad[i].abs() > 1e-10 {
                abs_diff / numerical_grad[i].abs()
            } else {
                abs_diff
            };

            assert!(
                rel_error < 0.01 || abs_diff < 1e-6,
                "Gradient component {} differs: provider={}, numerical={}, rel_error={}",
                i,
                provider_grad[i],
                numerical_grad[i],
                rel_error
            );
        }
    }

    #[test]
    fn test_gradient_otm_strikes() {
        // Test with out-of-the-money strikes
        let market_data = SABRMarketData {
            forward: 100.0,
            time_to_expiry: 1.0,
            strikes: vec![80.0, 120.0],
            market_vols: vec![0.25, 0.23],
            beta: 0.5,
            shift: None,
        };

        let deriv_provider = SABRCalibrationDerivatives::new(market_data.clone());

        // Provider gradient.
        let params = vec![0.15, 0.3, -0.1];
        let mut provider_grad = vec![0.0; 3];
        deriv_provider.gradient(&params, &mut provider_grad);

        // Numerical gradient of the same vega-weighted objective.
        let eps = 1e-6;
        let mut numerical_grad = [0.0; 3];

        let objective = |p: &[f64]| -> f64 {
            let alpha = p[0];
            let nu = p[1];
            let rho = p[2];

            let mut sum_sq = 0.0;
            for (i, &strike) in market_data.strikes.iter().enumerate() {
                let model_vol = deriv_provider.sabr_vol_fd(strike, alpha, nu, rho);
                let residual = model_vol - market_data.market_vols[i];
                sum_sq += deriv_provider.weights[i] * residual * residual;
            }
            sum_sq
        };

        for i in 0..3 {
            let mut params_plus = params.clone();
            let mut params_minus = params.clone();
            params_plus[i] += eps;
            params_minus[i] -= eps;

            numerical_grad[i] = (objective(&params_plus) - objective(&params_minus)) / (2.0 * eps);
        }

        for i in 0..3 {
            let abs_diff = (provider_grad[i] - numerical_grad[i]).abs();
            let rel_error = if numerical_grad[i].abs() > 1e-10 {
                abs_diff / numerical_grad[i].abs()
            } else {
                abs_diff
            };

            assert!(
                rel_error < 0.01 || abs_diff < 1e-6,
                "OTM gradient component {} differs: provider={}, numerical={}, rel_error={}",
                i,
                provider_grad[i],
                numerical_grad[i],
                rel_error
            );
        }
    }

    /// Validate that the derivatives from `sabr_vol_and_derivatives` agree with
    /// manual central-difference derivatives of `SABRModel::implied_volatility`.
    ///
    /// Uses rates-scale parameters (small forward/strike) to exercise the
    /// regime where hand-derived Hagan-expansion gradients were historically
    /// unreliable; the provider's `SABRModel`-backed FD gradients must match.
    #[test]
    fn test_sabr_fd_vs_manual_fd_single_strike_derivatives() {
        let alpha = 0.04;
        let beta = 0.5;
        let rho = -0.3;
        let nu = 0.4;
        let forward = 0.03;
        let strike = 0.035;
        let t = 1.0;

        let market_data = SABRMarketData {
            forward,
            time_to_expiry: t,
            strikes: vec![strike],
            market_vols: vec![0.20],
            beta,
            shift: None,
        };

        let provider = SABRCalibrationDerivatives::new(market_data);

        let (vol, d_alpha_provider, d_nu_provider, d_rho_provider) =
            provider.sabr_vol_and_derivatives(strike, alpha, nu, rho);

        let h = 1e-5;
        let sabr_vol = |a: f64, n: f64, r: f64| -> f64 {
            let params = SABRParameters::new(a, beta, n, r).expect("valid SABR params");
            SABRModel::new(params)
                .implied_volatility(forward, strike, t)
                .expect("valid vol")
        };

        assert!(vol > 0.0, "Base vol should be positive: {}", vol);

        let d_alpha_fd = (sabr_vol(alpha + h, nu, rho) - sabr_vol(alpha - h, nu, rho)) / (2.0 * h);
        let d_nu_fd = (sabr_vol(alpha, nu + h, rho) - sabr_vol(alpha, nu - h, rho)) / (2.0 * h);
        let d_rho_fd = (sabr_vol(alpha, nu, rho + h) - sabr_vol(alpha, nu, rho - h)) / (2.0 * h);

        let rel_tol = 1e-4;
        let check = |name: &str, provider_val: f64, manual_fd: f64| {
            let denom = manual_fd.abs().max(1e-12);
            let rel_err = (provider_val - manual_fd).abs() / denom;
            assert!(
                rel_err < rel_tol,
                "{}: provider={:.8e}, manual_fd={:.8e}, rel_err={:.4e} exceeds {:.0e}",
                name,
                provider_val,
                manual_fd,
                rel_err,
                rel_tol,
            );
        };

        check("d_sigma/d_alpha", d_alpha_provider, d_alpha_fd);
        check("d_sigma/d_nu", d_nu_provider, d_nu_fd);
        check("d_sigma/d_rho", d_rho_provider, d_rho_fd);
    }

    /// A `SABRMarketData` carrying an explicit `shift` must evaluate volatility
    /// through shifted SABR — i.e. [`SABRParameters::new_with_shift`] — so the
    /// provider's vol matches a directly-constructed shifted [`SABRModel`].
    ///
    /// Regression guard: an earlier implementation built `SABRParameters` with
    /// `new()` (shift = `None`) inside the FD path, silently ignoring the
    /// configured shift and pricing un-shifted SABR for a shifted calibration.
    #[test]
    fn test_shifted_sabr_derivatives_honor_configured_shift() {
        let forward = 0.015;
        let shift = 0.03; // lifts forward/strikes well above zero
        let beta = 0.5;
        let t = 1.0;
        let strikes = vec![0.005, 0.015, 0.025];
        let market_vols = vec![0.22, 0.20, 0.21];

        let market_data = SABRMarketData::new_with_shift(
            forward,
            t,
            strikes.clone(),
            market_vols.clone(),
            beta,
            shift,
        )
        .expect("valid shifted market data");

        let provider = SABRCalibrationDerivatives::new(market_data);

        let alpha = 0.04;
        let nu = 0.3;
        let rho = -0.1;

        for &strike in &strikes {
            let (vol, da, dnu, drho) = provider.sabr_vol_and_derivatives(strike, alpha, nu, rho);

            // Independently price the same point with shifted SABR.
            let shifted_model = SABRModel::new(
                SABRParameters::new_with_shift(alpha, beta, nu, rho, shift)
                    .expect("valid shifted SABR params"),
            );
            let expected = shifted_model
                .implied_volatility(forward, strike, t)
                .expect("shifted SABR vol");

            assert!(
                (vol - expected).abs() < 1e-12,
                "shifted-SABR vol mismatch at K={strike}: provider={vol}, expected={expected}"
            );
            assert!(da.is_finite() && dnu.is_finite() && drho.is_finite());
        }

        // The shift must actually change the answer: an un-shifted provider
        // over the same sub-shift-scale forward/strikes prices a different
        // smile, so dropping the shift would be observable.
        let unshifted = SABRCalibrationDerivatives::new(
            SABRMarketData::new(forward, t, strikes.clone(), market_vols, beta)
                .expect("valid market data"),
        );
        let shifted_atm = provider.sabr_vol_and_derivatives(forward, alpha, nu, rho).0;
        let unshifted_atm = unshifted
            .sabr_vol_and_derivatives(forward, alpha, nu, rho)
            .0;
        assert!(
            (shifted_atm - unshifted_atm).abs() > 1e-6,
            "shift should change the vol: shifted={shifted_atm}, unshifted={unshifted_atm}"
        );
    }
}
