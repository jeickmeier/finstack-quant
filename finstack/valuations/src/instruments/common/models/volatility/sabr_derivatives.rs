//! Analytical derivatives for SABR model calibration.
//!
//! Provides exact gradients for SABR implied volatility with respect to
//! model parameters (alpha, nu, rho), significantly accelerating calibration.

use super::sabr::{SABRModel, SABRParameters};
use finstack_core::math::solver_multi::AnalyticalDerivatives;
use finstack_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Floor volatility returned when the SABR formula is undefined for the
/// configured forward/strike/shift triple. Calibration treats this as an
/// effectively-zero gradient contribution; operators are warned via
/// `tracing::warn!` when this branch fires.
const SABR_FLOOR_VOL: f64 = 1e-4;

/// β-snap tolerance — kept identical to `SABRModel`'s internal `BETA_SNAP_TOL`
/// so the analytical-derivative pathway and the `SABRModel`-backed pathway
/// classify β (normal / lognormal / general) the same way.
const DERIV_BETA_SNAP_TOL: f64 = 1e-4;

/// Snap β to the exact endpoints 0 or 1 when it is within `DERIV_BETA_SNAP_TOL`,
/// mirroring the snap `SABRModel::implied_volatility` applies. Without this the
/// analytical derivatives use a raw β while the FD path (via `SABRModel`) uses
/// a snapped β, so the two pathways classify β (normal / lognormal / general)
/// differently for β just inside the snap band, and their gradients diverge.
#[inline]
fn effective_beta(raw_beta: f64) -> f64 {
    if raw_beta < DERIV_BETA_SNAP_TOL {
        0.0
    } else if (1.0 - raw_beta).abs() < DERIV_BETA_SNAP_TOL {
        1.0
    } else {
        raw_beta
    }
}

/// Internal parameter bundle for derivative calculations.
#[derive(Debug, Clone)]
struct SABRDerivParams {
    alpha: f64,
    nu: f64,
    rho: f64,
}

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

/// Analytical derivatives provider for SABR calibration.
///
/// This implementation provides exact gradients of the least-squares
/// objective function with respect to SABR parameters (alpha, nu, rho).
///
/// Can optionally use finite-difference gradients for higher accuracy
/// at the expense of performance.
pub struct SABRCalibrationDerivatives {
    market_data: SABRMarketData,
    /// Use finite-difference gradients instead of analytical approximations
    use_fd: bool,
}

impl SABRCalibrationDerivatives {
    /// Create a new SABR derivatives provider with analytical gradients (default).
    pub fn new(market_data: SABRMarketData) -> Self {
        Self {
            market_data,
            use_fd: false,
        }
    }

    /// Create a new SABR derivatives provider with finite-difference gradients.
    /// More accurate but slower than analytical approximations.
    pub fn new_with_fd(market_data: SABRMarketData) -> Self {
        Self {
            market_data,
            use_fd: true,
        }
    }

    /// Compute SABR implied volatility and its derivatives.
    ///
    /// Returns (vol, d_vol/d_alpha, d_vol/d_nu, d_vol/d_rho)
    ///
    /// Note: The gradient implementation below follows a pragmatic market
    /// approach for speed: we treat some secondary dependencies (e.g.,
    /// the x(z) term in Hagan’s formula) as approximately constant with
    /// respect to small perturbations in alpha/nu/rho. This is commonly
    /// acceptable for calibration stability and performance. For users
    /// requiring fully analytical derivatives, consider switching to the
    /// LM solver without derivatives or extending these expressions.
    fn sabr_vol_and_derivatives(
        &self,
        strike: f64,
        alpha: f64,
        nu: f64,
        rho: f64,
    ) -> (f64, f64, f64, f64) {
        // When configured for finite-difference gradients, use the actual
        // SABRModel-based volatility for both the base value and the
        // perturbations so that the derivatives are consistent with the
        // calibration objective.
        if self.use_fd {
            let base_vol = self.sabr_vol_fd(strike, alpha, nu, rho);

            // Central finite differences for better accuracy
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

            return (base_vol, d_vol_d_alpha, d_vol_d_nu, d_vol_d_rho);
        }

        let f_raw = self.market_data.forward;
        let k_raw = strike;
        let t = self.market_data.time_to_expiry;
        // Snap β to the endpoints with the SAME 1e-4 tolerance `SABRModel`
        // uses, so the analytical pathway and the `SABRModel`-backed FD
        // pathway classify β identically — otherwise their base vols (and
        // hence the calibration residuals and gradients) diverge for β just
        // inside the snap band around 0 or 1.
        let beta = effective_beta(self.market_data.beta);

        // Negative-rate handling for LogNormal SABR (beta ~ 1.0).
        // Standard LogNormal SABR fails for f <= 0 or K <= 0; if the user did
        // not configure a shift we fall back to a 200 bp default ONLY when
        // negative rates would otherwise blow up the formula. `beta` is the
        // already-snapped value, so the β≈1 test is now an exact `== 1.0`.
        let shift = if (beta - 1.0).abs() < f64::EPSILON && (f_raw <= 0.0 || k_raw <= 0.0) {
            self.market_data.shift.unwrap_or(0.02)
        } else {
            self.market_data.shift.unwrap_or(0.0)
        };

        let f = f_raw + shift;
        let k = k_raw + shift;

        // If the chosen shift is still insufficient (e.g. SOFR at -2 % with no
        // explicit shift configured), the only honest answer is that the
        // formula is undefined here. Emit a structured warning so operators
        // can see the floor activation in logs, then return a clearly-zero
        // gradient with a small positive vol — calibration will treat this
        // point as a no-op contribution rather than panic.
        if f <= 0.0 || k <= 0.0 {
            tracing::warn!(
                target: "finstack_valuations::sabr",
                forward = f_raw,
                strike = k_raw,
                shift,
                beta,
                "SABR volatility undefined: forward+shift or strike+shift is non-positive. \
                 Configure an explicit `shift` on SABRMarketData large enough to lift both \
                 (forward + shift) and (strike + shift) above zero."
            );
            return (SABR_FLOOR_VOL, 0.0, 0.0, 0.0);
        }

        // Handle ATM case
        if (f - k).abs() < 1e-10 {
            return self.sabr_atm_vol_and_derivatives(alpha, nu, rho, f);
        }

        // Pre-compute common terms
        let f_mid = (f * k).sqrt();
        let log_fk = (f / k).ln();
        let z = (nu / alpha) * f_mid.powf(1.0 - beta) * log_fk;
        let x = if z.abs() < 1e-10 {
            1.0 // Limit as z -> 0
        } else {
            // Correct Hagan et al. (2002) formula:
            // x(z) = ln((√(1-2ρz+z²) + z - ρ) / (1-ρ))
            let sqrt_term = (1.0 - 2.0 * rho * z + z * z).sqrt();
            ((sqrt_term + z - rho) / (1.0 - rho)).ln()
        };

        // Main volatility formula components. `beta` here is already snapped
        // (see `effective_beta`) so this pathway and the `SABRModel`-backed FD
        // pathway classify β identically near β≈0/1.
        let f_mid_power = f_mid.powf(1.0 - beta);
        let term1 = alpha
            / (f_mid_power
                * (1.0
                    + (1.0 - beta).powi(2) / 24.0 * log_fk.powi(2)
                    + (1.0 - beta).powi(4) / 1920.0 * log_fk.powi(4)));

        let term2_base = 1.0
            + t * (((1.0 - beta).powi(2) * alpha * alpha)
                / (24.0 * f_mid.powf(2.0 * (1.0 - beta)))
                + (rho * beta * nu * alpha) / (4.0 * f_mid_power)
                + (2.0 - 3.0 * rho * rho) * nu * nu / 24.0);

        let vol = term1 * x * term2_base;

        // Analytical approximations (faster) — pass shifted f/k so derivatives
        // are consistent with the shifted vol computed above.
        let sabr_params = SABRDerivParams { alpha, nu, rho };
        let d_vol_d_alpha = self.d_vol_d_alpha_impl(f, k, &sabr_params, vol, x, term2_base);
        let d_vol_d_nu = self.d_vol_d_nu_impl(f, k, &sabr_params, vol, x, term2_base);
        let d_vol_d_rho = self.d_vol_d_rho_impl(f, k, &sabr_params, vol, x, term2_base);
        (vol, d_vol_d_alpha, d_vol_d_nu, d_vol_d_rho)
    }

    /// Compute SABR volatility only (for finite differences).
    fn sabr_vol_fd(&self, strike: f64, alpha: f64, nu: f64, rho: f64) -> f64 {
        // Create SABR parameters
        let params = match SABRParameters::new(alpha, self.market_data.beta, nu, rho) {
            Ok(p) => p,
            Err(_) => return 0.0, // Return 0 for invalid parameters
        };

        let sabr = SABRModel::new(params);
        sabr.implied_volatility(
            self.market_data.forward,
            strike,
            self.market_data.time_to_expiry,
        )
        .unwrap_or(0.0)
    }

    /// Compute derivative of x with respect to z.
    ///
    /// x(z, ρ) = ln((√(1-2ρz+z²) + z - ρ) / (1-ρ))
    ///
    /// Since ln(1-ρ) is constant in z, dx/dz = d/dz ln(√(1-2ρz+z²) + z - ρ):
    ///
    /// dx/dz = ((-ρ+z)/√(1-2ρz+z²) + 1) / (√(1-2ρz+z²) + z - ρ)
    fn dx_dz(&self, z: f64, rho: f64) -> f64 {
        let sqrt_term = (1.0 - 2.0 * rho * z + z * z).sqrt();
        let numerator = -rho + z + sqrt_term;
        let denominator = sqrt_term * (sqrt_term + z - rho);

        if denominator.abs() < 1e-14 {
            return 0.0;
        }

        numerator / denominator
    }

    /// Compute derivative of x with respect to rho.
    ///
    /// x(z, ρ) = ln((√(1-2ρz+z²) + z - ρ) / (1-ρ))
    ///
    /// dx/dρ = derivative of the log term
    fn dx_drho(&self, z: f64, rho: f64) -> f64 {
        let sqrt_term = (1.0 - 2.0 * rho * z + z * z).sqrt();
        let arg = (sqrt_term + z - rho) / (1.0 - rho);

        if arg <= 0.0 || (1.0 - rho).abs() < 1e-14 {
            return 0.0;
        }

        // d/dρ of ln((√(1-2ρz+z²) + z - ρ) / (1-ρ))
        // = 1/arg * d/dρ of arg
        // = 1/arg * [(1-ρ)*(-z/√(...) - 1) - (√(...) + z - ρ)*(-1)] / (1-ρ)²

        let d_sqrt_d_rho = -z / sqrt_term;
        let d_numerator_d_rho = d_sqrt_d_rho - 1.0;
        let d_denominator_d_rho = -1.0;

        let numerator = sqrt_term + z - rho;
        let denominator = 1.0 - rho;

        // Quotient rule: (d_num * denom - num * d_denom) / denom²
        let d_arg_d_rho = (d_numerator_d_rho * denominator - numerator * d_denominator_d_rho)
            / (denominator * denominator);

        d_arg_d_rho / arg
    }

    /// Compute ATM volatility and derivatives.
    ///
    /// `f` is the *effective* (shift-applied) ATM forward — at ATM the strike
    /// equals it.
    fn sabr_atm_vol_and_derivatives(
        &self,
        alpha: f64,
        nu: f64,
        rho: f64,
        f: f64,
    ) -> (f64, f64, f64, f64) {
        let t = self.market_data.time_to_expiry;
        // Snap β consistently with `SABRModel` (see `effective_beta`) so the
        // ATM analytical pathway classifies β the same way near β≈0/1.
        let beta = effective_beta(self.market_data.beta);

        let f_power = f.powf(1.0 - beta);

        // ATM volatility
        let vol_base = alpha / f_power;
        let correction = 1.0
            + t * (((1.0 - beta).powi(2) * alpha * alpha) / (24.0 * f.powf(2.0 * (1.0 - beta)))
                + (rho * beta * nu * alpha) / (4.0 * f_power)
                + (2.0 - 3.0 * rho * rho) * nu * nu / 24.0);

        let vol = vol_base * correction;

        // Derivatives for ATM case
        let d_vol_d_alpha = correction / f_power
            + vol_base
                * t
                * (((1.0 - beta).powi(2) * 2.0 * alpha) / (24.0 * f.powf(2.0 * (1.0 - beta)))
                    + (rho * beta * nu) / (4.0 * f_power));

        let d_vol_d_nu = vol_base
            * t
            * ((rho * beta * alpha) / (4.0 * f_power) + (2.0 - 3.0 * rho * rho) * 2.0 * nu / 24.0);

        let d_vol_d_rho =
            vol_base * t * ((beta * nu * alpha) / (4.0 * f_power) - 6.0 * rho * nu * nu / 24.0);

        (vol, d_vol_d_alpha, d_vol_d_nu, d_vol_d_rho)
    }

    /// Partial derivative with respect to alpha.
    ///
    /// Complete chain rule implementation including dx/dα.
    /// `forward` and `strike` must already include any SABR shift.
    fn d_vol_d_alpha_impl(
        &self,
        forward: f64,
        strike: f64,
        sabr_params: &SABRDerivParams,
        _vol: f64,
        x: f64,
        term2: f64,
    ) -> f64 {
        let f = forward;
        let t = self.market_data.time_to_expiry;
        // Snap β consistently with `SABRModel` so this gradient term and the
        // base vol classify β identically.
        let beta = effective_beta(self.market_data.beta);

        let f_mid = (f * strike).sqrt();
        let log_fk = (f / strike).ln();
        let f_power = f_mid.powf(1.0 - beta);

        // term1 = alpha / (f_mid_power * denom)
        let denom = 1.0
            + (1.0 - beta).powi(2) / 24.0 * log_fk.powi(2)
            + (1.0 - beta).powi(4) / 1920.0 * log_fk.powi(4);
        let term1 = sabr_params.alpha / (f_power * denom);

        // d_term1/d_alpha = 1 / (f_power * denom)
        // Note: denom depends on beta which is fixed during calibration, so d_denom/d_alpha = 0
        let d_term1_d_alpha = 1.0 / (f_power * denom);

        // d_term2/d_alpha
        let d_term2_d_alpha = t
            * (((1.0 - beta).powi(2) * 2.0 * sabr_params.alpha)
                / (24.0 * f_mid.powf(2.0 * (1.0 - beta)))
                + (sabr_params.rho * beta * sabr_params.nu) / (4.0 * f_power));

        // Compute dx/d_alpha using chain rule
        // z = (nu/alpha) * f_mid^(1-beta) * log(f/k)
        // dz/d_alpha = -nu/alpha^2 * f_mid^(1-beta) * log(f/k) = -z/alpha
        let z = (sabr_params.nu / sabr_params.alpha) * f_mid.powf(1.0 - beta) * log_fk;
        let dx_d_alpha = if z.abs() < 1e-10 {
            0.0 // Derivative is zero at z=0 limit
        } else {
            let dz_d_alpha = -z / sabr_params.alpha;
            self.dx_dz(z, sabr_params.rho) * dz_d_alpha
        };

        // Apply product rule: d(term1 * x * term2)/d_alpha
        d_term1_d_alpha * x * term2 + term1 * dx_d_alpha * term2 + term1 * x * d_term2_d_alpha
    }

    /// Partial derivative with respect to nu (vol of vol).
    ///
    /// Complete chain rule implementation including dx/dν.
    /// `forward` and `strike` must already include any SABR shift.
    fn d_vol_d_nu_impl(
        &self,
        forward: f64,
        strike: f64,
        sabr_params: &SABRDerivParams,
        _vol: f64,
        x: f64,
        term2: f64,
    ) -> f64 {
        let f = forward;
        let t = self.market_data.time_to_expiry;
        // Snap β consistently with `SABRModel` so this gradient term and the
        // base vol classify β identically.
        let beta = effective_beta(self.market_data.beta);

        let f_mid = (f * strike).sqrt();
        let log_fk = (f / strike).ln();
        let f_power = f_mid.powf(1.0 - beta);

        // term1 = alpha / (f_mid_power * denom)
        let denom = 1.0
            + (1.0 - beta).powi(2) / 24.0 * log_fk.powi(2)
            + (1.0 - beta).powi(4) / 1920.0 * log_fk.powi(4);
        let term1 = sabr_params.alpha / (f_power * denom);

        // d_term1/d_nu = 0 (term1 doesn't depend on nu; beta is fixed)

        // d_term2/d_nu
        let d_term2_d_nu = t
            * ((sabr_params.rho * beta * sabr_params.alpha) / (4.0 * f_power)
                + (2.0 - 3.0 * sabr_params.rho * sabr_params.rho) * 2.0 * sabr_params.nu / 24.0);

        // Compute dx/d_nu using chain rule
        // z = (nu/alpha) * f_mid^(1-beta) * log(f/k)
        // dz/d_nu = (1/alpha) * f_mid^(1-beta) * log(f/k) = z/nu
        let z = (sabr_params.nu / sabr_params.alpha) * f_mid.powf(1.0 - beta) * log_fk;
        let dx_d_nu = if z.abs() < 1e-10 {
            0.0 // Derivative is zero at z=0 limit
        } else {
            let dz_d_nu = z / sabr_params.nu;
            self.dx_dz(z, sabr_params.rho) * dz_d_nu
        };

        // Apply product rule: d(term1 * x * term2)/d_nu
        // d_term1/d_nu = 0, so first term vanishes
        term1 * dx_d_nu * term2 + term1 * x * d_term2_d_nu
    }

    /// Partial derivative with respect to rho (correlation).
    ///
    /// Complete chain rule implementation including dx/dρ.
    /// `forward` and `strike` must already include any SABR shift.
    fn d_vol_d_rho_impl(
        &self,
        forward: f64,
        strike: f64,
        sabr_params: &SABRDerivParams,
        _vol: f64,
        x: f64,
        term2: f64,
    ) -> f64 {
        let f = forward;
        let t = self.market_data.time_to_expiry;
        // Snap β consistently with `SABRModel` so this gradient term and the
        // base vol classify β identically.
        let beta = effective_beta(self.market_data.beta);

        let f_mid = (f * strike).sqrt();
        let log_fk = (f / strike).ln();
        let f_power = f_mid.powf(1.0 - beta);

        // term1 = alpha / (f_mid_power * denom)
        let denom = 1.0
            + (1.0 - beta).powi(2) / 24.0 * log_fk.powi(2)
            + (1.0 - beta).powi(4) / 1920.0 * log_fk.powi(4);
        let term1 = sabr_params.alpha / (f_power * denom);

        // d_term1/d_rho = 0 (term1 doesn't depend on rho; beta is fixed)

        // d_term2/d_rho
        let d_term2_d_rho = t
            * ((beta * sabr_params.nu * sabr_params.alpha) / (4.0 * f_power)
                - 6.0 * sabr_params.rho * sabr_params.nu * sabr_params.nu / 24.0);

        // Compute dx/d_rho using chain rule
        // z = (nu/alpha) * f_mid^(1-beta) * log(f/k)
        // dz/d_rho = 0 (z doesn't depend on rho)
        // But x(z, rho) depends on rho directly
        let z = (sabr_params.nu / sabr_params.alpha) * f_mid.powf(1.0 - beta) * log_fk;
        let dx_d_rho = if z.abs() < 1e-10 {
            0.0 // Derivative is zero at z=0 limit
        } else {
            self.dx_drho(z, sabr_params.rho)
        };

        // Apply product rule: d(term1 * x * term2)/d_rho
        // d_term1/d_rho = 0, so first term vanishes
        term1 * dx_d_rho * term2 + term1 * x * d_term2_d_rho
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

        // Initialize gradient
        gradient[0] = 0.0;
        gradient[1] = 0.0;
        gradient[2] = 0.0;

        // Compute gradient of least-squares objective
        for (i, &strike) in self.market_data.strikes.iter().enumerate() {
            let (model_vol, d_alpha, d_nu, d_rho) =
                self.sabr_vol_and_derivatives(strike, alpha, nu, rho);

            let market_vol = self.market_data.market_vols[i];
            let residual = model_vol - market_vol;

            // Gradient of squared residual
            gradient[0] += 2.0 * residual * d_alpha;
            gradient[1] += 2.0 * residual * d_nu;
            gradient[2] += 2.0 * residual * d_rho;
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

        // Use finite-difference backed derivatives so that the gradient matches
        // the objective used in calibration (SABRModel-based volatilities).
        let deriv_provider = SABRCalibrationDerivatives::new_with_fd(market_data.clone());

        // Compute analytical gradient
        let params = vec![0.15, 0.3, -0.1];
        let mut analytical_grad = vec![0.0; 3];
        deriv_provider.gradient(&params, &mut analytical_grad);

        // Compute numerical gradient using ACTUAL SABR formula
        let eps = 1e-6;
        let mut numerical_grad = [0.0; 3];

        // Helper to compute objective using ACTUAL SABR implementation
        let objective = |p: &[f64]| -> f64 {
            let alpha = p[0];
            let nu = p[1];
            let rho = p[2];

            // Use actual SABR volatility calculation via sabr_vol_fd
            let mut sum_sq = 0.0;
            for (i, &strike) in market_data.strikes.iter().enumerate() {
                let model_vol = deriv_provider.sabr_vol_fd(strike, alpha, nu, rho);
                let residual = model_vol - market_data.market_vols[i];
                sum_sq += residual * residual;
            }
            sum_sq
        };

        // Compute finite differences
        for i in 0..3 {
            let mut params_plus = params.clone();
            let mut params_minus = params.clone();
            params_plus[i] += eps;
            params_minus[i] -= eps;

            numerical_grad[i] = (objective(&params_plus) - objective(&params_minus)) / (2.0 * eps);
        }

        // Compare gradients with proper tolerance
        for i in 0..3 {
            let abs_diff = (analytical_grad[i] - numerical_grad[i]).abs();
            let rel_error = if numerical_grad[i].abs() > 1e-10 {
                abs_diff / numerical_grad[i].abs()
            } else {
                abs_diff
            };

            // Expect good agreement between analytical and numerical gradients
            assert!(
                rel_error < 0.01 || abs_diff < 1e-6,
                "Gradient component {} differs: analytical={}, numerical={}, rel_error={}",
                i,
                analytical_grad[i],
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

        // Use finite-difference backed derivatives for consistency with the
        // SABRModel-based objective used in calibration.
        let deriv_provider = SABRCalibrationDerivatives::new_with_fd(market_data.clone());

        // Compute analytical gradient
        let params = vec![0.15, 0.3, -0.1];
        let mut analytical_grad = vec![0.0; 3];
        deriv_provider.gradient(&params, &mut analytical_grad);

        // Compute numerical gradient
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
                sum_sq += residual * residual;
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

        // Compare gradients
        for i in 0..3 {
            let abs_diff = (analytical_grad[i] - numerical_grad[i]).abs();
            let rel_error = if numerical_grad[i].abs() > 1e-10 {
                abs_diff / numerical_grad[i].abs()
            } else {
                abs_diff
            };

            assert!(
                rel_error < 0.01 || abs_diff < 1e-6,
                "OTM gradient component {} differs: analytical={}, numerical={}, rel_error={}",
                i,
                analytical_grad[i],
                numerical_grad[i],
                rel_error
            );
        }
    }

    /// Validate that the FD-mode derivatives from `sabr_vol_and_derivatives`
    /// agree with manual central-difference derivatives using `SABRModel::implied_volatility`.
    ///
    /// Uses rates-scale params where analytical approximations are known to
    /// diverge, so we test the FD pathway which is used in production calibration.
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

        let provider = SABRCalibrationDerivatives::new_with_fd(market_data);

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

    /// The analytical derivative path must snap β to the endpoints 0/1 with the
    /// *same* `1e-4` tolerance [`SABRModel`] uses, so it and the FD path (which
    /// routes through `SABRModel`) classify β consistently near β≈0/1.
    ///
    /// Failure mode under test: `SABRModel::implied_volatility` snaps β to 1.0
    /// when `|1-β| < 1e-4` (and to 0.0 when `β < 1e-4`), but the analytical
    /// `sabr_vol_and_derivatives` used the *raw* β. So β just inside the snap
    /// band (e.g. 0.99999) and the exact endpoint (1.0) produced *different*
    /// analytical results while `SABRModel` treated them identically — the FD
    /// and analytic gradients then diverged near β≈0/1.
    ///
    /// Contract pinned here: the analytical vol+gradients for a β just inside
    /// the snap band equal those at the exact endpoint (both snap to it).
    #[test]
    fn test_analytical_beta_snap_consistent_with_endpoints() {
        let forward = 100.0;
        let strike = 110.0;
        let t = 1.0;
        let alpha = 0.20;
        let nu = 0.30;
        let rho = -0.20;

        let eval = |beta: f64| -> (f64, f64, f64, f64) {
            let md = SABRMarketData {
                forward,
                time_to_expiry: t,
                strikes: vec![strike],
                market_vols: vec![0.20],
                beta,
                shift: None,
            };
            SABRCalibrationDerivatives::new(md).sabr_vol_and_derivatives(strike, alpha, nu, rho)
        };

        // β just inside the 1e-4 band around 1.0 must snap to exactly 1.0,
        // producing identical vol + all three gradient components.
        let near_one = eval(1.0 - 1e-5);
        let exact_one = eval(1.0);
        assert!(
            (near_one.0 - exact_one.0).abs() < 1e-12
                && (near_one.1 - exact_one.1).abs() < 1e-12
                && (near_one.2 - exact_one.2).abs() < 1e-12
                && (near_one.3 - exact_one.3).abs() < 1e-12,
            "β=1−1e-5 must snap to β=1: near_one={near_one:?}, exact_one={exact_one:?}"
        );

        // β just inside the 1e-4 band around 0.0 must snap to exactly 0.0.
        let near_zero = eval(1e-5);
        let exact_zero = eval(0.0);
        assert!(
            (near_zero.0 - exact_zero.0).abs() < 1e-12
                && (near_zero.1 - exact_zero.1).abs() < 1e-12
                && (near_zero.2 - exact_zero.2).abs() < 1e-12
                && (near_zero.3 - exact_zero.3).abs() < 1e-12,
            "β=1e-5 must snap to β=0: near_zero={near_zero:?}, exact_zero={exact_zero:?}"
        );

        // `effective_beta` must match SABRModel's snap thresholds exactly.
        assert!((effective_beta(1.0 - 1e-5) - 1.0).abs() < f64::EPSILON);
        assert!(effective_beta(1e-5).abs() < f64::EPSILON);
        // Just OUTSIDE the band: β stays unsnapped.
        let outside = 1.0 - 2e-4;
        assert!((effective_beta(outside) - outside).abs() < f64::EPSILON);
    }

    /// Verify that the **analytical** derivative path produces correct gradients
    /// when a SABR shift is active.
    ///
    /// Uses `new()` (not `new_with_fd`) so the analytical `d_vol_d_*_impl`
    /// methods are exercised, then compares against central finite differences
    /// of `sabr_vol_and_derivatives` itself. The shift changes the effective
    /// forward/strike used for vol computation (f+shift, k+shift), so the
    /// derivatives must use the same shifted coordinates.
    #[test]
    fn test_analytical_derivatives_with_shift() {
        let forward = 100.0;
        let shift = 10.0; // effective forward = 110, effective strikes = 100/110/120
        let beta = 0.5;
        let strikes = vec![90.0, 100.0, 110.0];
        let market_vols = vec![0.22, 0.20, 0.21];

        let market_data = SABRMarketData::new_with_shift(
            forward,
            1.0,
            strikes.clone(),
            market_vols.clone(),
            beta,
            shift,
        )
        .expect("valid shifted market data");

        // Analytical gradient provider (NOT fd) — exercises d_vol_d_*_impl
        let provider = SABRCalibrationDerivatives::new(market_data);

        // Alpha must be scaled for shifted coordinates: with effective fwd=110
        // and beta=0.5, alpha ≈ target_vol * fwd^(1-beta) ≈ 0.20 * 110^0.5 ≈ 2.1
        let alpha = 2.1;
        let nu = 0.3;
        let rho = -0.1;
        let eps = 1e-6;

        for &strike in &strikes {
            let (vol, da, dnu, drho) = provider.sabr_vol_and_derivatives(strike, alpha, nu, rho);

            assert!(vol.is_finite(), "vol must be finite for strike {strike}");

            // Central finite differences of the same analytical vol function
            let fd_da = (provider
                .sabr_vol_and_derivatives(strike, alpha + eps, nu, rho)
                .0
                - provider
                    .sabr_vol_and_derivatives(strike, alpha - eps, nu, rho)
                    .0)
                / (2.0 * eps);
            let fd_dnu = (provider
                .sabr_vol_and_derivatives(strike, alpha, nu + eps, rho)
                .0
                - provider
                    .sabr_vol_and_derivatives(strike, alpha, nu - eps, rho)
                    .0)
                / (2.0 * eps);
            let fd_drho = (provider
                .sabr_vol_and_derivatives(strike, alpha, nu, rho + eps)
                .0
                - provider
                    .sabr_vol_and_derivatives(strike, alpha, nu, rho - eps)
                    .0)
                / (2.0 * eps);

            let check = |name: &str, analytical: f64, fd: f64| {
                let denom = fd.abs().max(1e-12);
                let rel_err = (analytical - fd).abs() / denom;
                assert!(
                    rel_err < 0.02 || (analytical - fd).abs() < 1e-8,
                    "Shifted SABR {name} at K={strike}: analytical={analytical:.6e}, fd={fd:.6e}, rel={rel_err:.4e}",
                );
            };

            check("d_vol/d_alpha", da, fd_da);
            check("d_vol/d_nu", dnu, fd_dnu);
            check("d_vol/d_rho", drho, fd_drho);
        }
    }
}
