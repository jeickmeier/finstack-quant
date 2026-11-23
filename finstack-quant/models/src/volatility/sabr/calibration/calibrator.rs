//! SABR smile calibrator and deterministic calibration outcome.

use super::super::parameters::SabrParameters;
use super::fit::{PreparedSabrFit, initial_alpha_guess, normalized_quote_weights};
use super::multi_start::{
    bounded_to_unconstrained, compute_sabr_jacobian, deterministic_sabr_starts,
    run_deterministic_sabr_starts, unconstrained_to_bounded,
};
use super::shift::{SabrShift, standard_shift};
use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;
use finstack_quant_core::{Error, Result, market_data::surfaces::VolQuoteType};

/// SABR calibration with normalized relative quote residuals.
///
/// `tolerance` is the maximum accepted relative error of any final volatility
/// quote (1e-4 means 0.01% of the quoted volatility). Vega weights sum to one,
/// residuals are divided by each quote, and alpha is optimized in log space.
/// These choices preserve calibration behavior when rate/volatility units scale.
/// Final diagnostics contain signed errors in the original quote units.
#[derive(Clone)]
pub struct SabrCalibrator {
    /// Tolerance for calibration convergence.
    ///
    /// Lower values give more accurate calibration but take longer.
    /// See struct-level docs for guidance on choosing tolerance.
    tolerance: f64,
    /// Maximum iterations for the optimizer.
    max_iterations: usize,
    /// Displacement policy applied to the forward and strikes before fitting.
    shift: SabrShift,
    /// Whether alpha is solved analytically for an exact ATM match.
    atm_pinning: bool,
}

/// Calibrated SABR parameters and deterministic solver diagnostics.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SabrCalibrationOutcome {
    /// Calibrated model parameters.
    pub parameters: SabrParameters,
    /// Total iterations consumed across all attempted starts.
    pub total_iterations: usize,
    /// Iterations consumed by the selected start.
    pub winning_iterations: usize,
    /// Actual objective residual invocations across all deterministic starts,
    /// including attempts rejected by the solver or final quote-fit checks.
    pub residual_evaluations: usize,
    /// Initial alpha, nu, and rho values for the selected start.
    pub winning_start: [f64; 3],
    /// Signed model-minus-market errors, in the original quote units: decimal
    /// absolute rate volatility for normal quotes, relative decimals for Black.
    pub quote_errors: Vec<f64>,
    /// Largest absolute quote error divided by its positive market quote.
    pub max_relative_quote_error: f64,
    /// Parameter names whose calibrated values landed on configured bounds.
    pub parameters_at_bounds: Vec<String>,
}

impl SabrCalibrator {
    fn validate_settings(&self) -> Result<()> {
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 || self.max_iterations == 0 {
            return Err(Error::Validation("SABR calibration requires positive finite relative tolerance and nonzero iteration budget".to_owned()));
        }
        Ok(())
    }

    /// Create a calibrator accepting at most 1e-4 relative quote error.
    ///
    /// Uses nine deterministic starts and up to 2000 iterations per start.
    pub fn new() -> Self {
        Self {
            tolerance: 1e-4,
            max_iterations: 2000,
            shift: SabrShift::None,
            atm_pinning: false,
        }
    }

    /// Create a calibrator accepting at most 1e-8 relative quote error.
    pub fn high_precision() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 200,
            shift: SabrShift::None,
            atm_pinning: false,
        }
    }

    /// Set the displacement policy applied before fitting.
    ///
    /// # Arguments
    ///
    /// * `shift` - [`SabrShift::None`] for strictly positive inputs,
    ///   [`SabrShift::Fixed`] for an explicit additive displacement in the
    ///   forward's units, or [`SabrShift::Auto`] to pick a standardized shift
    ///   only when the forward or a strike is negative.
    pub fn with_shift(mut self, shift: SabrShift) -> Self {
        self.shift = shift;
        self
    }

    /// Enable or disable exact ATM pinning.
    ///
    /// # Arguments
    ///
    /// * `atm_pinning` - When `true`, alpha is solved analytically so the
    ///   model reproduces the ATM volatility interpolated from the quotes
    ///   exactly, and only nu and rho are fitted to the smile.
    pub fn with_atm_pinning(mut self, atm_pinning: bool) -> Self {
        self.atm_pinning = atm_pinning;
        self
    }

    /// Set the maximum accepted relative error of every final volatility quote.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Finite positive dimensionless error limit; 1e-4 accepts at most 0.01% of each quoted volatility.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set maximum iterations
    ///
    /// # Arguments
    ///
    /// * `max_iterations` - Positive cap on solver iterations before convergence failure.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Maximum accepted dimensionless relative error of every fitted volatility quote.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Iteration cap before the solver reports non-convergence.
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Displacement policy applied before fitting.
    pub fn shift(&self) -> SabrShift {
        self.shift
    }

    /// Whether alpha is pinned to the interpolated ATM volatility.
    pub fn atm_pinning(&self) -> bool {
        self.atm_pinning
    }

    /// Calibrate SABR and return the parameters with solver diagnostics.
    ///
    /// Applies the configured shift and ATM-pinning policy and runs a
    /// deterministic multi-start fit. The selected quote type sets the units
    /// and the pricing formula for the objective and residual checks.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for invalid settings, fewer than three
    /// aligned positive quotes, invalid shifted rates, or a fit outside the
    /// configured per-quote error tolerance.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward price or decimal rate.
    /// * `strikes` - Unshifted strikes in the same units as `forward`; at
    ///   least three, aligned with `market_vols`.
    /// * `market_vols` - Implied volatilities aligned with `strikes`.
    /// * `time_to_expiry` - Time to expiry in years.
    /// * `beta` - Fixed SABR elasticity parameter in `[0, 1]`.
    /// * `quote_type` - Input annualized Normal or Black quote type. Shifted Black requires an explicit
    ///   displacement policy.
    pub fn calibrate(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        quote_type: VolQuoteType,
    ) -> Result<SabrCalibrationOutcome> {
        if strikes.len() != market_vols.len() || strikes.len() < 3 {
            return Err(Error::Validation(format!(
                "SABR calibration requires at least three aligned strike/vol quotes; strikes={}, vols={}",
                strikes.len(),
                market_vols.len()
            )));
        }
        let fit_quote = if quote_type == VolQuoteType::Normal {
            VolQuoteType::Normal
        } else {
            VolQuoteType::BlackLognormal
        };
        match (quote_type, self.shift) {
            (VolQuoteType::BlackLognormal, SabrShift::None)
            | (VolQuoteType::Normal, SabrShift::None) => {
                return self.fit(
                    forward,
                    strikes,
                    market_vols,
                    time_to_expiry,
                    beta,
                    fit_quote,
                );
            }
            (VolQuoteType::BlackLognormal, _)
            | (VolQuoteType::ShiftedBlackLognormal, SabrShift::None) => {
                return Err(Error::Validation(
                    "SABR calibration quote type disagrees with the displacement policy".into(),
                ));
            }
            _ => {}
        }
        let shift = self.resolve_shift(forward, strikes)?;
        let shifted_forward = forward + shift;
        let shifted_strikes: Vec<f64> = strikes.iter().map(|&strike| strike + shift).collect();
        if shifted_forward <= 0.0 || shifted_strikes.iter().any(|&strike| strike <= 0.0) {
            return Err(Error::Validation(format!(
                "Shifted SABR calibration: shift={shift:.6} is insufficient"
            )));
        }
        let mut outcome = self.fit(
            shifted_forward,
            &shifted_strikes,
            market_vols,
            time_to_expiry,
            beta,
            fit_quote,
        )?;
        outcome.parameters = SabrParameters::new_with_shift(
            outcome.parameters.alpha,
            beta,
            outcome.parameters.nu,
            outcome.parameters.rho,
            shift,
        )?;
        Ok(outcome)
    }

    /// Resolve the configured [`SabrShift`] to a concrete displacement.
    fn resolve_shift(&self, forward: f64, strikes: &[f64]) -> Result<f64> {
        match self.shift {
            SabrShift::None => Ok(0.0),
            SabrShift::Fixed(shift) if shift.is_finite() => Ok(shift),
            SabrShift::Fixed(shift) => Err(Error::Validation(format!(
                "SABR calibration: fixed shift must be finite, got {shift}"
            ))),
            SabrShift::Auto => {
                let min_strike = strikes
                    .iter()
                    .copied()
                    .min_by(f64::total_cmp)
                    .ok_or_else(|| Error::Validation("Strikes should not be empty".to_string()))?;
                let min_rate = forward.min(min_strike);
                if min_rate < 0.0 {
                    standard_shift(min_rate)
                } else {
                    Ok(0.0)
                }
            }
        }
    }

    /// Dispatch to the free or ATM-pinned fit on already-shifted inputs.
    fn fit(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        quote_type: VolQuoteType,
    ) -> Result<SabrCalibrationOutcome> {
        if self.atm_pinning {
            self.fit_atm_pinned(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
                quote_type,
            )
        } else {
            self.fit_free(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
                quote_type,
            )
        }
    }

    fn prepare_sabr_fit<'a>(
        &self,
        forward: f64,
        strikes: &'a [f64],
        market_vols: &'a [f64],
        time_to_expiry: f64,
        beta: f64,
        quote_type: VolQuoteType,
    ) -> Result<PreparedSabrFit<'a>> {
        let weights = normalized_quote_weights(
            forward,
            strikes,
            market_vols,
            time_to_expiry,
            beta,
            quote_type,
        )?;
        self.validate_settings()?;
        let solver =
            LevenbergMarquardtSolver::new(self.tolerance.min(1e-10), 1e-12, self.max_iterations)?;
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let alpha_start = initial_alpha_guess(atm_vol, forward, beta, quote_type);
        let starts = deterministic_sabr_starts(alpha_start);
        let template = SabrParameters::new(1.0, beta, 0.4, 0.0)?;
        let prepared: Vec<_> = strikes
            .iter()
            .map(|&strike| {
                super::super::expansion::PreparedSabrStrike::new(
                    &template,
                    forward,
                    strike,
                    time_to_expiry,
                    quote_type,
                )
            })
            .collect::<Result<_>>()?;
        Ok(PreparedSabrFit {
            forward,
            strikes,
            market_vols,
            time_to_expiry,
            quote_type,
            tolerance: self.tolerance,
            weights,
            atm_vol,
            starts,
            prepared,
            solver,
        })
    }

    /// Fit alpha, nu, and rho freely (no ATM pinning) on positive inputs.
    fn fit_free(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        quote_type: VolQuoteType,
    ) -> Result<SabrCalibrationOutcome> {
        let fit = self.prepare_sabr_fit(
            forward,
            strikes,
            market_vols,
            time_to_expiry,
            beta,
            quote_type,
        )?;
        let outcome = run_deterministic_sabr_starts(
            fit.starts.clone(),
            fit.tolerance,
            "SABR",
            beta,
            |physical_start| {
                let initial = [
                    physical_start[0].ln(),
                    bounded_to_unconstrained(physical_start[1], 0.001, 2.0),
                    bounded_to_unconstrained(physical_start[2], -0.99, 0.99),
                ];
                let calls = std::cell::Cell::new(0);
                let residuals = |unconstrained: &[f64],
                                 output: &mut [f64]|
                 -> finstack_quant_core::Result<()> {
                    calls.set(calls.get() + 1);
                    let alpha = unconstrained[0].exp();
                    let nu = unconstrained_to_bounded(unconstrained[1], 0.001, 2.0);
                    let rho = unconstrained_to_bounded(unconstrained[2], -0.99, 0.99);
                    let parameters = SabrParameters::new(alpha, beta, nu, rho)?;
                    fit.evaluate_residuals(&parameters, output)
                };
                let jacobian = |coordinates: &[f64],
                                output: &mut [f64]|
                 -> finstack_quant_core::Result<()> {
                    compute_sabr_jacobian(&fit.prepared, &fit.weights, beta, coordinates, output)
                };
                let solution = if quote_type == VolQuoteType::Normal && beta > 0.0 {
                    fit.solver
                        .solve_least_squares(residuals, &initial, fit.prepared.len())
                } else {
                    fit.solver.solve_least_squares_with_jacobian(
                        residuals,
                        jacobian,
                        &initial,
                        fit.prepared.len(),
                    )
                };
                (solution, calls.get())
            },
            |solution| {
                let physical = [
                    solution.params[0].exp(),
                    unconstrained_to_bounded(solution.params[1], 0.001, 2.0),
                    unconstrained_to_bounded(solution.params[2], -0.99, 0.99),
                ];
                let parameters =
                    SabrParameters::new(physical[0], beta, physical[1], physical[2]).ok()?;
                fit.candidate_fits(&parameters).then_some(physical)
            },
        )?
        .into_outcome();
        fit.validate_quote_fit(outcome)
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
    pub(super) fn find_atm_vol(&self, forward: f64, strikes: &[f64], vols: &[f64]) -> Result<f64> {
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
        let mut quotes: Vec<(f64, f64)> =
            strikes.iter().copied().zip(vols.iter().copied()).collect();
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

        Ok(first.1)
    }

    /// Fit nu and rho with alpha solved analytically for an exact ATM match.
    ///
    /// The ATM target is the volatility interpolated (linearly in variance)
    /// to the forward from the quoted smile; see [`Self::find_atm_vol`].
    fn fit_atm_pinned(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        quote_type: VolQuoteType,
    ) -> Result<SabrCalibrationOutcome> {
        let fit = self.prepare_sabr_fit(
            forward,
            strikes,
            market_vols,
            time_to_expiry,
            beta,
            quote_type,
        )?;
        let outcome = run_deterministic_sabr_starts(
            fit.starts.clone(),
            fit.tolerance,
            "ATM-pinned SABR",
            beta,
            |physical_start| {
                let initial = [
                    bounded_to_unconstrained(physical_start[1], 0.001, 2.0),
                    bounded_to_unconstrained(physical_start[2], -0.99, 0.99),
                ];
                let calls = std::cell::Cell::new(0);
                let residuals = |unconstrained: &[f64],
                                 output: &mut [f64]|
                 -> finstack_quant_core::Result<()> {
                    calls.set(calls.get() + 1);
                    let nu = unconstrained_to_bounded(unconstrained[0], 0.001, 2.0);
                    let rho = unconstrained_to_bounded(unconstrained[1], -0.99, 0.99);
                    let alpha = solve_alpha_for_atm(
                        forward,
                        fit.atm_vol,
                        time_to_expiry,
                        [beta, nu, rho],
                        self.tolerance.min(1e-10),
                        quote_type,
                    )?;
                    let parameters = SabrParameters::new(alpha, beta, nu, rho)?;
                    fit.evaluate_residuals(&parameters, output)
                };
                let solution =
                    fit.solver
                        .solve_least_squares(residuals, &initial, market_vols.len());
                (solution, calls.get())
            },
            |solution| {
                let nu = unconstrained_to_bounded(solution.params[0], 0.001, 2.0);
                let rho = unconstrained_to_bounded(solution.params[1], -0.99, 0.99);
                solve_alpha_for_atm(
                    forward,
                    fit.atm_vol,
                    time_to_expiry,
                    [beta, nu, rho],
                    self.tolerance.min(1e-10),
                    quote_type,
                )
                .ok()
                .map(|alpha| [alpha, nu, rho])
                .filter(|&physical| {
                    SabrParameters::new(physical[0], beta, physical[1], physical[2])
                        .ok()
                        .is_some_and(|parameters| fit.candidate_fits(&parameters))
                })
            },
        )?
        .into_outcome();
        fit.validate_quote_fit(outcome)
    }
}

/// Solve for alpha that matches target ATM volatility given other SABR parameters.
///
/// Uses Newton iteration on the ATM volatility formula:
/// σ_ATM = α/F^(1-β) * [1 + T * corrections(α, ν, ρ)]
pub(crate) fn solve_alpha_for_atm(
    forward: f64,
    target_atm_vol: f64,
    time_to_expiry: f64,
    [beta, nu, rho]: [f64; 3],
    tolerance: f64,
    quote_type: VolQuoteType,
) -> Result<f64> {
    let alpha_start = initial_alpha_guess(target_atm_vol, forward, beta, quote_type);
    let mut alpha = alpha_start;

    const MAX_ITER: usize = 50;

    let mut last_error = f64::INFINITY;
    for _ in 0..MAX_ITER {
        let params = SabrParameters::new(alpha, beta, nu, rho)?;
        let model_vol = params.implied_vol(forward, forward, time_to_expiry, quote_type)?;

        let error = model_vol - target_atm_vol;
        last_error = error;
        if error.abs() <= tolerance * target_atm_vol {
            return Ok(alpha);
        }

        // Numerical derivative for Newton step
        let bump = alpha * 1e-6;
        let params_bumped = SabrParameters::new(alpha + bump, beta, nu, rho)?;
        let vol_bumped = params_bumped.implied_vol(forward, forward, time_to_expiry, quote_type)?;

        let d_vol_d_alpha = (vol_bumped - model_vol) / bump;
        if d_vol_d_alpha.abs() < 1e-14 {
            break; // Can't continue Newton iteration
        }

        // Newton step with damping for stability
        let step = -error / d_vol_d_alpha;
        alpha += step.clamp(-alpha * 0.5, alpha * 0.5);

        if alpha <= 0.0 {
            alpha = alpha_start * 0.5;
        }
    }

    // Non-convergence is an error: silently returning the last iterate breaks
    // the ATM-pinning contract (the pinning objective excludes the ATM strike,
    // so nothing downstream would catch a mismatched ATM vol).
    Err(Error::Calibration {
        message: format!(
            "solve_alpha_for_atm did not converge within {MAX_ITER} Newton iterations: \
             last alpha {alpha:.6e} leaves ATM vol error {last_error:.3e} \
             (tolerance {tolerance:.1e}) at forward {forward}, T {time_to_expiry}, \
             beta {beta}, nu {nu}, rho {rho}."
        ),
        category: "sabr_atm_alpha".to_string(),
    })
}

impl Default for SabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}
