use super::model::SABRModel;
use super::parameters::SABRParameters;
use finstack_core::{Error, Result};

/// SABR calibration using market prices.
///
/// # Tolerance Considerations
///
/// The default tolerance of 1e-6 provides a balance between speed and accuracy:
///
/// | Tolerance | Use Case | Accuracy | Speed |
/// |-----------|----------|----------|-------|
/// | 1e-4 | Quick screening | ~0.5 vol bp | Fast |
/// | 1e-6 | Standard production | ~0.01 vol bp | Moderate |
/// | 1e-8 | High-precision (BBG VCUB) | ~0.0001 vol bp | Slow |
/// | 1e-10 | Research/validation | Machine precision | Very slow |
///
/// For production vol surfaces where Greeks are computed from the surface,
/// consider using tighter tolerance (1e-8) to ensure smooth Greeks.
///
/// # Gradient Methods
///
/// Two gradient computation methods are available:
///
/// - **Finite differences** (default): More robust, works for all parameter ranges
/// - **Analytical**: Faster but may have numerical issues at parameter boundaries
#[derive(Clone)]
pub struct SABRCalibrator {
    /// Tolerance for calibration convergence.
    ///
    /// Lower values give more accurate calibration but take longer.
    /// See struct-level docs for guidance on choosing tolerance.
    tolerance: f64,
    /// Maximum iterations for the optimizer.
    max_iterations: usize,
    /// Use finite-difference gradients instead of analytical approximations.
    use_fd_gradients: bool,
}

impl SABRCalibrator {
    /// Create new calibrator with production-ready defaults.
    ///
    /// Default settings:
    /// - **Tolerance**: 1e-6 (standard production accuracy)
    /// - **Max iterations**: 100
    /// - **Gradient method**: Finite difference (more robust)
    ///
    /// # Production Usage
    ///
    /// For high-precision applications (e.g., Greeks computation from vol surface),
    /// consider using tighter tolerance:
    ///
    /// ```rust,no_run
    /// use finstack_valuations::instruments::models::volatility::sabr::SABRCalibrator;
    ///
    /// let _calibrator = SABRCalibrator::new();
    ///
    /// let _precise_calibrator = SABRCalibrator::new()
    ///     .with_tolerance(1e-8)
    ///     .with_max_iterations(200);
    /// ```
    ///
    /// By default, uses finite-difference gradients (`use_fd_gradients: true`)
    /// for more accurate calibration at the cost of some performance.
    /// Use `with_fd_gradients(false)` to switch to analytical approximations
    /// for faster but potentially less accurate calibration.
    pub fn new() -> Self {
        Self {
            tolerance: 1e-6,
            max_iterations: 100,
            use_fd_gradients: true, // Default to FD for production accuracy
        }
    }

    /// Create calibrator with high-precision settings.
    ///
    /// Uses Bloomberg VCUB-equivalent tolerance (1e-8) for applications
    /// requiring very accurate vol surface fitting, such as:
    /// - Greeks computation from interpolated surface
    /// - Exotic pricing with vol smile dependence
    /// - Regulatory model validation
    pub fn high_precision() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 200,
            use_fd_gradients: true,
        }
    }

    /// Set tolerance
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Enable finite-difference gradients for higher accuracy (slower).
    pub fn with_fd_gradients(mut self, use_fd: bool) -> Self {
        self.use_fd_gradients = use_fd;
        self
    }

    /// Calibrate SABR parameters with automatic negative rate detection
    pub fn calibrate_auto_shift(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SABRParameters> {
        // Check if we need shift for negative rates
        let min_strike = strikes
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .ok_or_else(|| Error::Validation("Strikes should not be empty".to_string()))?;
        let min_rate = forward.min(*min_strike);

        if min_rate < 0.0 {
            // Use shifted SABR
            let shift = (-min_rate + 0.001).max(0.001); // At least 10bps shift
            self.calibrate_shifted(forward, strikes, market_vols, time_to_expiry, beta, shift)
        } else {
            // Use standard SABR
            self.calibrate(forward, strikes, market_vols, time_to_expiry, beta)
        }
    }

    /// Calibrate SABR parameters with automatic negative rate detection and analytical derivatives
    pub fn calibrate_auto_shift_with_derivatives(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SABRParameters> {
        // Check if we need shift for negative rates
        let min_strike = strikes
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .ok_or_else(|| Error::Validation("Strikes should not be empty".to_string()))?;
        let min_rate = forward.min(*min_strike);

        if min_rate < 0.0 {
            // Use shifted SABR with derivatives
            let shift = (-min_rate + 0.001).max(0.001); // At least 10bps shift
            self.calibrate_shifted_with_derivatives(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
                shift,
            )
        } else {
            // Use standard SABR with derivatives
            self.calibrate_with_derivatives(forward, strikes, market_vols, time_to_expiry, beta)
        }
    }

    /// Calibrate shifted SABR parameters for negative rate environments
    pub fn calibrate_shifted(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        shift: f64,
    ) -> Result<SABRParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        // Apply shift to all rates
        let shifted_forward = forward + shift;
        let shifted_strikes: Vec<f64> = strikes.iter().map(|&s| s + shift).collect();

        // Validate shifted rates are positive
        if shifted_forward <= 0.0 || shifted_strikes.iter().any(|&s| s <= 0.0) {
            let min_shifted_strike = shifted_strikes
                .iter()
                .copied()
                .min_by(|a, b| a.total_cmp(b))
                .unwrap_or(0.0);
            return Err(Error::Validation(format!(
                "Shifted SABR calibration: shift={:.6} is insufficient. \
                 shifted_forward={:.6}, min_shifted_strike={:.6}. Increase shift.",
                shift, shifted_forward, min_shifted_strike
            )));
        }

        // Calibrate using shifted rates
        let base_params = self.calibrate(
            shifted_forward,
            &shifted_strikes,
            market_vols,
            time_to_expiry,
            beta,
        )?;

        // Return parameters with shift
        SABRParameters::new_with_shift(
            base_params.alpha,
            beta,
            base_params.nu,
            base_params.rho,
            shift,
        )
    }

    /// Calibrate SABR parameters to market implied volatilities using multi-dimensional solver
    pub fn calibrate(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64, // Beta is usually fixed
    ) -> Result<SABRParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        // Use Levenberg-Marquardt solver for robust calibration
        use finstack_core::math::solver_multi::LevenbergMarquardtSolver;

        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(self.tolerance)
            .with_max_iterations(self.max_iterations);

        // Define objective function: sum of squared volatility errors
        let strikes_vec = strikes.to_vec();
        let market_vols_vec = market_vols.to_vec();
        let objective = move |params: &[f64]| -> f64 {
            let alpha = params[0];
            let nu = params[1];
            let rho = params[2];

            // Create SABR parameters and model
            if let Ok(sabr_params) = SABRParameters::new(alpha, beta, nu, rho) {
                let model = SABRModel::new(sabr_params);

                // Calculate sum of squared errors
                strikes_vec
                    .iter()
                    .zip(market_vols_vec.iter())
                    .map(|(&strike, &market_vol)| {
                        model
                            .implied_volatility(forward, strike, time_to_expiry)
                            .map(|model_vol| (model_vol - market_vol).powi(2))
                            .unwrap_or(1e6) // Large penalty for invalid parameters
                    })
                    .sum()
            } else {
                1e12 // Very large penalty for invalid parameters
            }
        };

        // Initial guess for parameters
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let initial = vec![
            atm_vol * forward.powf(1.0 - beta), // alpha: ATM vol adjusted for beta
            0.3,                                // nu: typical vol-of-vol
            0.0,                                // rho: start neutral
        ];

        // Parameter bounds for SABR model
        let bounds = vec![
            (0.001, 5.0),  // alpha: positive, reasonable range
            (0.001, 2.0),  // nu: positive vol-of-vol
            (-0.99, 0.99), // rho: correlation bounds
        ];

        // Calibrate using multi-dimensional solver
        let solution = solver.minimize(objective, &initial, Some(&bounds))?;

        // Extract calibrated parameters
        SABRParameters::new(solution[0], beta, solution[1], solution[2])
    }

    /// Calibrate SABR parameters with analytical derivatives for improved performance
    pub fn calibrate_with_derivatives(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SABRParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        // Use analytical derivatives from the parent module
        use crate::instruments::models::volatility::sabr_derivatives::{
            SABRCalibrationDerivatives, SABRMarketData,
        };
        use finstack_core::math::solver_multi::LevenbergMarquardtSolver;

        // Create market data structure
        let market_data = SABRMarketData {
            forward,
            time_to_expiry,
            strikes: strikes.to_vec(),
            market_vols: market_vols.to_vec(),
            beta,
            shift: None,
        };

        // Create derivatives provider (with or without FD gradients)
        let derivatives_provider = if self.use_fd_gradients {
            SABRCalibrationDerivatives::new_with_fd(market_data.clone())
        } else {
            SABRCalibrationDerivatives::new(market_data.clone())
        };

        // Create Levenberg-Marquardt solver
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(self.tolerance)
            .with_max_iterations(self.max_iterations);

        // Define objective function: sum of squared volatility errors
        let objective = move |params: &[f64]| -> f64 {
            let alpha = params[0];
            let nu = params[1];
            let rho = params[2];

            // Create SABR parameters and model
            if let Ok(sabr_params) = SABRParameters::new(alpha, beta, nu, rho) {
                let model = SABRModel::new(sabr_params);

                // Calculate sum of squared errors
                market_data
                    .strikes
                    .iter()
                    .zip(market_data.market_vols.iter())
                    .map(|(&strike, &market_vol)| {
                        model
                            .implied_volatility(forward, strike, time_to_expiry)
                            .map(|model_vol| (model_vol - market_vol).powi(2))
                            .unwrap_or(1e6) // Large penalty for invalid parameters
                    })
                    .sum()
            } else {
                1e12 // Very large penalty for invalid parameters
            }
        };

        // Initial guess for parameters
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let initial = vec![
            atm_vol * forward.powf(1.0 - beta), // alpha
            0.3,                                // nu
            0.0,                                // rho
        ];

        // Parameter bounds
        let bounds = vec![
            (1e-6, 5.0),   // alpha bounds
            (1e-6, 2.0),   // nu bounds
            (-0.99, 0.99), // rho bounds
        ];

        // Solve with analytical derivatives
        let solution = solver.minimize_with_derivatives(
            objective,
            &derivatives_provider,
            &initial,
            Some(&bounds),
        )?;

        // Extract calibrated parameters
        let alpha = solution[0];
        let nu = solution[1];
        let rho = solution[2];

        SABRParameters::new(alpha, beta, nu, rho)
    }

    /// Calibrate shifted SABR with analytical derivatives
    pub fn calibrate_shifted_with_derivatives(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        shift: f64,
    ) -> Result<SABRParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        // Apply shift to all rates
        let shifted_forward = forward + shift;
        let shifted_strikes: Vec<f64> = strikes.iter().map(|&s| s + shift).collect();

        // Validate shifted rates are positive
        if shifted_forward <= 0.0 || shifted_strikes.iter().any(|&s| s <= 0.0) {
            let min_shifted_strike = shifted_strikes
                .iter()
                .copied()
                .min_by(|a, b| a.total_cmp(b))
                .unwrap_or(0.0);
            return Err(Error::Validation(format!(
                "Shifted SABR calibration: shift={:.6} is insufficient. \
                 shifted_forward={:.6}, min_shifted_strike={:.6}. Increase shift.",
                shift, shifted_forward, min_shifted_strike
            )));
        }

        // Calibrate using shifted rates with derivatives
        let base_params = self.calibrate_with_derivatives(
            shifted_forward,
            &shifted_strikes,
            market_vols,
            time_to_expiry,
            beta,
        )?;

        // Return parameters with shift
        SABRParameters::new_with_shift(
            base_params.alpha,
            beta,
            base_params.nu,
            base_params.rho,
            shift,
        )
    }

    /// Find the ATM volatility (volatility at `strike == forward`) from a
    /// discrete market smile.
    ///
    /// The smile rarely carries a quote exactly at the forward, so this
    /// **interpolates** the two bracketing quotes rather than snapping to the
    /// nearest strike. Interpolation is linear in total variance `σ²·T`; since
    /// every quote in a single-expiry slice shares the same `T`, that is
    /// equivalent to interpolating `σ²` linearly in strike. Snapping to the
    /// nearest strike (the previous behaviour) pins the ATM-calibration target
    /// to a genuinely off-ATM quote whenever the grid omits the forward.
    ///
    /// Outside the quoted strike range the nearest endpoint vol is used (flat
    /// extrapolation) — extrapolating an ATM level past the wings is not
    /// meaningful.
    fn find_atm_vol(&self, forward: f64, strikes: &[f64], vols: &[f64]) -> Result<f64> {
        if strikes.is_empty() || vols.is_empty() {
            return Err(Error::Validation(
                "SABR find_atm_vol: empty strikes/vols".to_string(),
            ));
        }
        if strikes.len() != vols.len() {
            return Err(Error::Validation(format!(
                "SABR find_atm_vol: strikes length ({}) must match vols length ({})",
                strikes.len(),
                vols.len()
            )));
        }

        // Pair and sort by strike so bracketing works regardless of input order.
        let mut quotes: Vec<(f64, f64)> = strikes
            .iter()
            .copied()
            .zip(vols.iter().copied())
            .collect();
        quotes.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Single quote: nothing to interpolate.
        let first = quotes
            .first()
            .ok_or_else(|| Error::Validation("SABR find_atm_vol: no quotes".to_string()))?;
        let last = quotes
            .last()
            .ok_or_else(|| Error::Validation("SABR find_atm_vol: no quotes".to_string()))?;
        if quotes.len() == 1 || forward <= first.0 {
            return Ok(first.1);
        }
        if forward >= last.0 {
            return Ok(last.1);
        }

        // Find the bracket [k_lo, k_hi] with k_lo <= forward < k_hi.
        for window in quotes.windows(2) {
            let (k_lo, v_lo) = window[0];
            let (k_hi, v_hi) = window[1];
            if forward >= k_lo && forward <= k_hi {
                let span = k_hi - k_lo;
                if span.abs() < 1e-14 {
                    // Coincident strikes — interpolation weight is undefined;
                    // both endpoints carry the same level, return either.
                    return Ok(v_lo);
                }
                // Linear-in-variance interpolation.
                let w = (forward - k_lo) / span;
                let var_lo = v_lo * v_lo;
                let var_hi = v_hi * v_hi;
                let var_atm = var_lo + (var_hi - var_lo) * w;
                if var_atm < 0.0 {
                    return Err(Error::Validation(format!(
                        "SABR find_atm_vol: interpolated variance {var_atm:.6e} is negative \
                         (k_lo={k_lo}, k_hi={k_hi}, v_lo={v_lo}, v_hi={v_hi})"
                    )));
                }
                return Ok(var_atm.sqrt());
            }
        }

        // Unreachable given the endpoint guards above, but return a defined
        // value rather than panicking if invariants are somehow violated.
        Ok(first.1)
    }

    /// Calibrate SABR with ATM volatility pinning (market-standard approach).
    ///
    /// This method ensures the calibrated model matches the ATM volatility exactly
    /// by solving for alpha analytically, then fitting only nu and rho to the smile.
    /// This is the standard market approach for SABR calibration.
    ///
    /// # Arguments
    /// * `forward` - Forward rate
    /// * `strikes` - Vector of strikes (should include ATM)
    /// * `market_vols` - Market implied volatilities corresponding to strikes
    /// * `time_to_expiry` - Time to expiry in years
    /// * `beta` - SABR beta parameter (typically fixed)
    ///
    /// # Returns
    /// Calibrated SABR parameters with exact ATM match
    pub fn calibrate_with_atm_pinning(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SABRParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        // Find ATM vol from market data
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;

        // Use 2D solver for nu and rho only
        use finstack_core::math::solver_multi::LevenbergMarquardtSolver;

        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(self.tolerance)
            .with_max_iterations(self.max_iterations);

        let strikes_vec = strikes.to_vec();
        let market_vols_vec = market_vols.to_vec();
        let tol = self.tolerance;

        // Objective: fit nu and rho, with alpha solved to match ATM
        let objective = move |params: &[f64]| -> f64 {
            let nu = params[0];
            let rho = params[1];

            // Solve for alpha that matches ATM vol exactly
            let alpha =
                match solve_alpha_for_atm(forward, atm_vol, time_to_expiry, beta, nu, rho, tol) {
                    Ok(a) => a,
                    Err(_) => return 1e12,
                };

            // Create model and compute smile errors (excluding ATM)
            if let Ok(sabr_params) = SABRParameters::new(alpha, beta, nu, rho) {
                let model = SABRModel::new(sabr_params);

                strikes_vec
                    .iter()
                    .zip(market_vols_vec.iter())
                    .map(|(&strike, &market_vol)| {
                        // Skip ATM point (it's matched exactly by construction)
                        let is_atm = (strike - forward).abs() / forward < 0.001;
                        if is_atm {
                            0.0
                        } else {
                            model
                                .implied_volatility(forward, strike, time_to_expiry)
                                .map(|model_vol| (model_vol - market_vol).powi(2))
                                .unwrap_or(1e6)
                        }
                    })
                    .sum()
            } else {
                1e12
            }
        };

        // Initial guess: nu=0.3 (typical), rho=0.0 (neutral)
        let initial = vec![0.3, 0.0];

        // Bounds for nu and rho
        let bounds = vec![
            (0.001, 2.0),  // nu
            (-0.99, 0.99), // rho
        ];

        let solution = solver.minimize(objective, &initial, Some(&bounds))?;

        let nu = solution[0];
        let rho = solution[1];

        // Final alpha solve with calibrated nu/rho
        let alpha = solve_alpha_for_atm(
            forward,
            atm_vol,
            time_to_expiry,
            beta,
            nu,
            rho,
            self.tolerance,
        )?;

        SABRParameters::new(alpha, beta, nu, rho)
    }
}

/// Solve for alpha that matches target ATM volatility given other SABR parameters.
///
/// Uses Newton iteration on the ATM volatility formula:
/// σ_ATM = α/F^(1-β) * [1 + T * corrections(α, ν, ρ)]
pub(super) fn solve_alpha_for_atm(
    forward: f64,
    target_atm_vol: f64,
    time_to_expiry: f64,
    beta: f64,
    nu: f64,
    rho: f64,
    tolerance: f64,
) -> Result<f64> {
    // Initial guess: first-order approximation
    let f_pow = forward.powf(1.0 - beta);
    let mut alpha = target_atm_vol * f_pow;

    // Newton iteration to refine alpha
    for _ in 0..50 {
        // Compute model ATM vol with current alpha
        let params = SABRParameters::new(alpha, beta, nu, rho)?;
        let model = SABRModel::new(params);
        let model_vol = model.atm_volatility(forward, time_to_expiry)?;

        let error = model_vol - target_atm_vol;
        if error.abs() < tolerance {
            return Ok(alpha);
        }

        // Numerical derivative for Newton step
        let bump = alpha * 1e-6;
        let params_bumped = SABRParameters::new(alpha + bump, beta, nu, rho)?;
        let model_bumped = SABRModel::new(params_bumped);
        let vol_bumped = model_bumped.atm_volatility(forward, time_to_expiry)?;

        let d_vol_d_alpha = (vol_bumped - model_vol) / bump;
        if d_vol_d_alpha.abs() < 1e-14 {
            break; // Can't continue Newton iteration
        }

        // Newton step with damping for stability
        let step = -error / d_vol_d_alpha;
        alpha += step.clamp(-alpha * 0.5, alpha * 0.5); // Limit step size

        // Ensure alpha stays positive
        if alpha <= 0.0 {
            alpha = target_atm_vol * f_pow * 0.5;
        }
    }

    // Return best alpha found even if not converged
    Ok(alpha)
}

impl Default for SABRCalibrator {
    fn default() -> Self {
        Self::new()
    }
}
