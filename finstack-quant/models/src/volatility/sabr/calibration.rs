//! SABR model, smile, parameter, and calibration support.
//!
use super::model::{SabrModel, BETA_SNAP_TOL};
use super::parameters::SabrParameters;
use crate::closed_form::{bachelier_vega, black_vega};
use finstack_quant_core::math::solver_multi::{
    LevenbergMarquardtSolver, LmSolution, LmTerminationReason,
};
use finstack_quant_core::{Error, Result};

/// Vega weight used by the SABR calibration objectives.
///
/// Standard practitioner choice (Hagan 2002, Bloomberg VCUB): weight each
/// (strike, market_vol) residual by vega. Vega concentrates near ATM
/// and decays into the wings, so an unweighted `Σ(σ_m − σ_*)²` would
/// over-fit the wings (large numbers of low-information quotes) at the
/// expense of the ATM. Vega weighting gives every dollar of premium roughly
/// equal weight, which is what the market actually quotes against.
///
/// The vega convention must match the vol convention of the quotes being
/// fitted, which follows the model's own β classification
/// ([`SabrModel::implied_volatility`] returns normal vols for β≈0):
///
/// - `beta ≈ 0` (within `BETA_SNAP_TOL`): the quotes are *normal*
///   (Bachelier) vols, so weight with Bachelier vega `√T·φ((F−K)/(σ_N√T))`.
///   Feeding a ~1% normal vol to Black vega would collapse all wing weights
///   to the floor and leave the smile uncalibrated.
/// - otherwise: lognormal (Black) quotes, weight with Black-76 vega. Shifted
///   calibrations pass already-shifted forward/strikes, making this the
///   shifted-Black vega.
///
/// Floor at a tiny positive number keeps deep-OTM strikes from getting a
/// strictly-zero weight (which would let the optimizer drift on the wings).
///
/// # Arguments
///
/// * `forward` - Forward price or rate in the same units as `strike`.
/// * `strike` - Option strike in price units or decimal-rate units.
/// * `market_vol` - Market volatility in the convention implied by `beta`.
/// * `time_to_expiry` - Time to expiry in years.
/// * `beta` - SABR elasticity parameter in the closed interval `[0, 1]`.
#[inline]
pub fn vega_weight(
    forward: f64,
    strike: f64,
    market_vol: f64,
    time_to_expiry: f64,
    beta: f64,
) -> f64 {
    const MIN_VEGA: f64 = 1e-10;
    let vega = if beta.abs() < BETA_SNAP_TOL {
        bachelier_vega(forward, strike, market_vol, time_to_expiry)
    } else {
        black_vega(forward, strike, market_vol, time_to_expiry)
    };
    vega.max(MIN_VEGA)
}

/// Initial alpha guess for the LM calibration.
///
/// From Hagan's ATM expansion `σ_ATM ≈ α / F^(1−β)`, so `α₀ = σ_ATM·F^(1−β)`
/// for lognormal-convention quotes. For `β ≈ 0` (within [`BETA_SNAP_TOL`])
/// the quotes and the model output are *normal* vols where `σ_N,ATM ≈ α`
/// directly — scaling by `F` would start the solver orders of magnitude off
/// for rate-like forwards.
#[inline]
fn initial_alpha_guess(atm_vol: f64, forward: f64, beta: f64) -> f64 {
    if beta.abs() < BETA_SNAP_TOL {
        atm_vol
    } else {
        atm_vol * forward.powf(1.0 - beta)
    }
}

/// Standardized shift ladder for shifted-SABR auto-shift selection.
///
/// Market practice quotes shifted-Black smiles against a small set of
/// standardized shifts (e.g. 1% for EUR/CHF swaptions, 2%/3% for deeply
/// negative short rates) rather than an ad-hoc data-dependent value, so the
/// same surface re-calibrated on a slightly different day does not silently
/// change convention. `calibrate_auto_shift` rounds the required minimum
/// shift (`−min_rate + 10bp` headroom) *up* to the next rung. Callers that
/// need an exact per-currency convention should pass an explicit shift to
/// [`SabrCalibrator::calibrate_shifted`].
const STANDARD_SHIFTS: [f64; 5] = [0.005, 0.01, 0.02, 0.03, 0.04];

/// Round the minimum required shift up to the standardized ladder.
///
/// Errors if rates are so negative that even the largest standardized shift
/// (4%) cannot make all shifted rates positive.
fn standard_shift(min_rate: f64) -> Result<f64> {
    let required = (-min_rate + 0.001).max(0.001); // at least 10bp headroom
    STANDARD_SHIFTS
        .iter()
        .copied()
        .find(|&s| s >= required)
        .ok_or_else(|| {
            Error::Validation(format!(
                "SABR auto-shift: minimum rate {min_rate:.6} requires a shift larger than the \
                 maximum standardized shift of 4%; pass an explicit shift via calibrate_shifted"
            ))
        })
}

/// SABR calibration using market prices.
///
/// # Tolerance Considerations
///
/// The tolerance applies to the vega-weighted sum-of-squared-errors (SSE)
/// objective minimized by the Levenberg-Marquardt solver. Since the core
/// `minimize` now errors loudly on non-convergence (instead of silently
/// returning the best iterate), the default must be attainable for typical
/// market smiles: SABR's `rho` is weakly identified on near-symmetric strike
/// sets, producing long shallow valleys the solver traverses slowly. The
/// default of 1e-4 (SSE) with a 2000-iteration budget converges reliably on
/// such inputs while keeping the refit smile within a fraction of a vol point
/// of the market quotes:
///
/// | Tolerance (SSE) | Use Case | Speed |
/// |-----------------|----------|-------|
/// | 1e-4 | Standard production (default) | Moderate |
/// | 1e-6 | Tight fits on well-identified smiles | Slow |
/// | 1e-8 | High-precision (BBG VCUB); needs a large iteration budget | Very slow |
///
/// Tighter tolerances may fail with a solver-convergence error on smiles
/// where `rho` is weakly identified; pair them with a larger
/// [`Self::with_max_iterations`] budget.
///
/// # Gradient Method
///
/// `calibrate_with_derivatives` drives the Levenberg-Marquardt solver with
/// central finite-difference gradients of the SABR implied-vol function. The
/// gradient is therefore exactly consistent with the calibration objective
/// and robust across the full parameter range.
#[derive(Clone)]
pub struct SabrCalibrator {
    /// Tolerance for calibration convergence.
    ///
    /// Lower values give more accurate calibration but take longer.
    /// See struct-level docs for guidance on choosing tolerance.
    tolerance: f64,
    /// Maximum iterations for the optimizer.
    max_iterations: usize,
}

/// Calibrated SABR parameters and deterministic solver diagnostics.
#[derive(Clone, Debug)]
pub struct SabrCalibrationOutcome {
    /// Calibrated model parameters.
    pub parameters: SabrParameters,
    /// Total iterations consumed across all attempted starts.
    pub total_iterations: usize,
    /// Iterations consumed by the selected start.
    pub winning_iterations: usize,
    /// Total objective residual evaluations.
    pub residual_evaluations: usize,
    /// Initial alpha, nu, and rho values for the selected start.
    pub winning_start: [f64; 3],
    /// Parameter names whose calibrated values landed on configured bounds.
    pub parameters_at_bounds: Vec<&'static str>,
}

fn bounded_to_unconstrained(value: f64, lower: f64, upper: f64) -> f64 {
    let unit = ((value - lower) / (upper - lower)).clamp(1e-9, 1.0 - 1e-9);
    (unit / (1.0 - unit)).ln()
}

fn unconstrained_to_bounded(value: f64, lower: f64, upper: f64) -> f64 {
    let unit = if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp_value = value.exp();
        exp_value / (1.0 + exp_value)
    };
    lower + (upper - lower) * unit
}

fn sabr_parameters_at_bounds(alpha: f64, nu: f64, rho: f64) -> Vec<&'static str> {
    const BOUNDS: [(&str, f64, f64); 3] = [
        ("alpha", 0.001, 5.0),
        ("nu", 0.001, 2.0),
        ("rho", -0.99, 0.99),
    ];
    [alpha, nu, rho]
        .into_iter()
        .zip(BOUNDS)
        .filter_map(|(value, (name, lower, upper))| {
            let threshold = 1e-5 * (upper - lower);
            ((value - lower).abs() <= threshold || (upper - value).abs() <= threshold)
                .then_some(name)
        })
        .collect()
}

fn deterministic_sabr_starts(alpha: f64) -> Vec<[f64; 3]> {
    const NU_STARTS: [f64; 3] = [0.15, 0.4, 0.9];
    const RHO_STARTS: [f64; 3] = [-0.6, 0.0, 0.6];
    let mut starts = Vec::with_capacity(NU_STARTS.len() * RHO_STARTS.len());
    for nu in NU_STARTS {
        for rho in RHO_STARTS {
            starts.push([alpha.clamp(0.001_001, 4.999_999), nu, rho]);
        }
    }
    starts
}

fn sabr_termination_is_acceptable(
    reason: &LmTerminationReason,
    final_residual_norm: f64,
    residual_tolerance: f64,
) -> bool {
    match reason {
        LmTerminationReason::ConvergedResidualNorm
        | LmTerminationReason::ConvergedRelativeReduction
        | LmTerminationReason::ConvergedGradient => true,
        LmTerminationReason::StepTooSmall | LmTerminationReason::MaxIterations => {
            final_residual_norm.is_finite() && final_residual_norm <= residual_tolerance
        }
        LmTerminationReason::NumericalFailure => false,
    }
}

#[derive(Default)]
struct RejectedSabrStarts {
    rejected_starts: usize,
    solver_failures: usize,
    best_rejected: Option<(f64, LmTerminationReason, usize)>,
}

impl RejectedSabrStarts {
    fn record_solver_failure(&mut self) {
        self.solver_failures += 1;
    }

    fn record_rejected(&mut self, solution: &LmSolution) {
        self.rejected_starts += 1;
        let score = solution.stats.final_residual_norm;
        if score.is_finite()
            && self
                .best_rejected
                .as_ref()
                .is_none_or(|(best_score, _, _)| score < *best_score)
        {
            self.best_rejected = Some((
                score,
                solution.stats.termination_reason.clone(),
                solution.stats.iterations,
            ));
        }
    }

    fn no_acceptable_error(&self, path: &str) -> Error {
        let best = self.best_rejected.as_ref().map_or_else(
            || "best_rejected_residual=none".to_string(),
            |(score, reason, iterations)| {
                format!(
                    "best_rejected_residual={score:.6e}, best_rejected_reason={reason:?}, \
                     best_rejected_iterations={iterations}"
                )
            },
        );
        Error::Calibration {
            message: format!(
                "no acceptable deterministic {path} start; rejected_starts={}, \
                 solver_failures={}; {best}",
                self.rejected_starts, self.solver_failures
            ),
            category: "sabr_multi_start".to_string(),
        }
    }
}

struct SabrMultiStartResult {
    outcome: SabrCalibrationOutcome,
    rejected: RejectedSabrStarts,
}

impl SabrMultiStartResult {
    fn into_outcome(self) -> SabrCalibrationOutcome {
        let Self {
            outcome,
            rejected: _rejected,
        } = self;
        outcome
    }
}

fn run_deterministic_sabr_starts<Solve, Reconstruct>(
    starts: Vec<[f64; 3]>,
    residual_tolerance: f64,
    path: &str,
    beta: f64,
    mut solve: Solve,
    mut reconstruct: Reconstruct,
) -> Result<SabrMultiStartResult>
where
    Solve: FnMut([f64; 3]) -> Result<LmSolution>,
    Reconstruct: FnMut(&LmSolution) -> Option<[f64; 3]>,
{
    let mut best: Option<(f64, LmSolution, [f64; 3], [f64; 3])> = None;
    let mut rejected = RejectedSabrStarts::default();
    let mut total_iterations = 0;

    for physical_start in starts {
        let Ok(solution) = solve(physical_start) else {
            rejected.record_solver_failure();
            continue;
        };
        total_iterations += solution.stats.iterations;
        let score = solution.stats.final_residual_norm;
        if !score.is_finite()
            || !sabr_termination_is_acceptable(
                &solution.stats.termination_reason,
                score,
                residual_tolerance,
            )
        {
            rejected.record_rejected(&solution);
            continue;
        }
        let Some(physical) = reconstruct(&solution) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _, _, _)| score < *best_score)
        {
            best = Some((score, solution, physical_start, physical));
        }
    }

    let Some((_score, solution, winning_start, physical)) = best else {
        return Err(rejected.no_acceptable_error(path));
    };

    Ok(SabrMultiStartResult {
        outcome: SabrCalibrationOutcome {
            parameters: SabrParameters::new(physical[0], beta, physical[1], physical[2])?,
            total_iterations,
            winning_iterations: solution.stats.iterations,
            residual_evaluations: solution.stats.residual_evals,
            winning_start,
            parameters_at_bounds: sabr_parameters_at_bounds(physical[0], physical[1], physical[2]),
        },
        rejected,
    })
}

impl SabrCalibrator {
    /// Create new calibrator with production-ready defaults.
    ///
    /// Default settings:
    /// - **Tolerance**: 1e-4 on the vega-weighted SSE objective
    /// - **Max iterations**: 2000
    /// - **Gradient method**: Finite difference (more robust)
    ///
    /// These defaults are attainable for typical market smiles under the
    /// strict non-convergence semantics of `core::math::solver_multi::
    /// LevenbergMarquardtSolver::minimize` :
    /// the solver now errors instead of silently returning its best iterate,
    /// so the prior defaults (1e-6 / 100 iterations) failed loudly on smiles
    /// where `rho` is weakly identified.
    ///
    /// # Production Usage
    ///
    /// For high-precision applications (e.g., Greeks computation from vol surface),
    /// consider using tighter tolerance with a larger iteration budget:
    ///
    /// ```
    /// use finstack_quant_models::volatility::sabr::SabrCalibrator;
    ///
    /// let _calibrator = SabrCalibrator::new();
    ///
    /// let _precise_calibrator = SabrCalibrator::new()
    ///     .with_tolerance(1e-8)
    ///     .with_max_iterations(5000);
    /// ```
    pub fn new() -> Self {
        Self {
            tolerance: 1e-4,
            max_iterations: 2000,
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
        }
    }

    /// Set tolerance
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

    /// Convergence tolerance on the vega-weighted SSE objective.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Iteration cap before the solver reports non-convergence.
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Calibrate SABR parameters with automatic negative rate detection
    pub fn calibrate_auto_shift(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SabrParameters> {
        Ok(self
            .calibrate_auto_shift_with_diagnostics(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
            )?
            .parameters)
    }

    /// Calibrate with an automatically selected shift and return solver diagnostics.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward price or decimal rate.
    /// * `strikes` - Unshifted strikes in the same units as `forward`.
    /// * `market_vols` - Implied volatilities aligned with `strikes`.
    /// * `time_to_expiry` - Time to expiry in years.
    /// * `beta` - Fixed SABR elasticity parameter in `[0, 1]`.
    pub fn calibrate_auto_shift_with_diagnostics(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SabrCalibrationOutcome> {
        let min_strike = strikes
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .ok_or_else(|| Error::Validation("Strikes should not be empty".to_string()))?;
        let min_rate = forward.min(*min_strike);
        if min_rate < 0.0 {
            let shift = standard_shift(min_rate)?;
            self.calibrate_shifted_with_diagnostics(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
                shift,
            )
        } else {
            self.calibrate_with_diagnostics(forward, strikes, market_vols, time_to_expiry, beta)
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
    ) -> Result<SabrParameters> {
        Ok(self
            .calibrate_shifted_with_diagnostics(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
                shift,
            )?
            .parameters)
    }

    /// Calibrate shifted SABR and return solver diagnostics.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward price or decimal rate.
    /// * `strikes` - Unshifted strikes in the same units as `forward`.
    /// * `market_vols` - Implied volatilities aligned with `strikes`.
    /// * `time_to_expiry` - Time to expiry in years.
    /// * `beta` - Fixed SABR elasticity parameter in `[0, 1]`.
    /// * `shift` - Additive shift applied to the forward and strikes.
    pub fn calibrate_shifted_with_diagnostics(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
        shift: f64,
    ) -> Result<SabrCalibrationOutcome> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }
        let shifted_forward = forward + shift;
        let shifted_strikes: Vec<f64> = strikes.iter().map(|&strike| strike + shift).collect();
        if shifted_forward <= 0.0 || shifted_strikes.iter().any(|&strike| strike <= 0.0) {
            return Err(Error::Validation(format!(
                "Shifted SABR calibration: shift={shift:.6} is insufficient"
            )));
        }
        let mut outcome = self.calibrate_with_diagnostics(
            shifted_forward,
            &shifted_strikes,
            market_vols,
            time_to_expiry,
            beta,
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

    /// Calibrate SABR parameters to market implied volatilities using multi-dimensional solver.
    ///
    /// # Vol quoting convention
    ///
    /// The objective compares Hagan-expansion vols to `market_vols` directly,
    /// and the expansion's output convention is β-dependent (see
    /// `SabrVolType`): pass **normal (Bachelier)** quotes when calibrating
    /// with β≈0 and **lognormal (Black)** quotes for β>0. Mixing conventions
    /// silently mis-calibrates.
    pub fn calibrate(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64, // Beta is usually fixed
    ) -> Result<SabrParameters> {
        Ok(self
            .calibrate_with_diagnostics(forward, strikes, market_vols, time_to_expiry, beta)?
            .parameters)
    }

    /// Calibrate unshifted SABR and return solver diagnostics.
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward price or decimal rate.
    /// * `strikes` - Strikes in the same units as `forward`.
    /// * `market_vols` - Implied volatilities aligned with `strikes`.
    /// * `time_to_expiry` - Time to expiry in years.
    /// * `beta` - Fixed SABR elasticity parameter in `[0, 1]`.
    pub fn calibrate_with_diagnostics(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SabrCalibrationOutcome> {
        if strikes.len() != market_vols.len() || strikes.len() < 3 {
            return Err(Error::Validation(format!(
                "SABR calibration requires at least three aligned strike/vol quotes; strikes={}, vols={}",
                strikes.len(),
                market_vols.len()
            )));
        }
        let residual_tolerance = self.tolerance.sqrt();
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(residual_tolerance)
            .with_max_iterations(self.max_iterations);
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let alpha_start = initial_alpha_guess(atm_vol, forward, beta);
        let starts = deterministic_sabr_starts(alpha_start);
        run_deterministic_sabr_starts(
            starts,
            residual_tolerance,
            "SABR",
            beta,
            |physical_start| {
                let initial = [
                    bounded_to_unconstrained(physical_start[0], 0.001, 5.0),
                    bounded_to_unconstrained(physical_start[1], 0.001, 2.0),
                    bounded_to_unconstrained(physical_start[2], -0.99, 0.99),
                ];
                let residuals = |unconstrained: &[f64], output: &mut [f64]| {
                    let alpha = unconstrained_to_bounded(unconstrained[0], 0.001, 5.0);
                    let nu = unconstrained_to_bounded(unconstrained[1], 0.001, 2.0);
                    let rho = unconstrained_to_bounded(unconstrained[2], -0.99, 0.99);
                    let Ok(parameters) = SabrParameters::new(alpha, beta, nu, rho) else {
                        output.fill(1e6);
                        return;
                    };
                    let model = SabrModel::new(parameters);
                    for (index, (&strike, &market_vol)) in
                        strikes.iter().zip(market_vols).enumerate()
                    {
                        let weight =
                            vega_weight(forward, strike, market_vol, time_to_expiry, beta).sqrt();
                        output[index] = model
                            .implied_volatility(forward, strike, time_to_expiry)
                            .map_or(1e6, |model_vol| weight * (model_vol - market_vol));
                    }
                };
                solver.solve_system_with_dim_stats(residuals, &initial, market_vols.len())
            },
            |solution| {
                Some([
                    unconstrained_to_bounded(solution.params[0], 0.001, 5.0),
                    unconstrained_to_bounded(solution.params[1], 0.001, 2.0),
                    unconstrained_to_bounded(solution.params[2], -0.99, 0.99),
                ])
            },
        )
        .map(SabrMultiStartResult::into_outcome)
    }

    /// Calibrate SABR parameters with finite-difference parameter gradients.
    pub fn calibrate_with_derivatives(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SabrParameters> {
        if strikes.len() != market_vols.len() {
            return Err(Error::Validation(format!(
                "SABR calibration: strikes length ({}) must match market_vols length ({})",
                strikes.len(),
                market_vols.len()
            )));
        }

        use crate::volatility::sabr_derivatives::{SabrCalibrationDerivatives, SabrMarketData};
        use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;

        let market_data = SabrMarketData {
            forward,
            time_to_expiry,
            strikes: strikes.to_vec(),
            market_vols: market_vols.to_vec(),
            beta,
            shift: None,
        };

        let derivatives_provider = SabrCalibrationDerivatives::new(market_data.clone());

        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(self.tolerance)
            .with_max_iterations(self.max_iterations);

        let objective = move |params: &[f64]| -> f64 {
            let alpha = params[0];
            let nu = params[1];
            let rho = params[2];

            if let Ok(sabr_params) = SabrParameters::new(alpha, beta, nu, rho) {
                let model = SabrModel::new(sabr_params);

                // Vega-weighted sum of squared errors (see `vega_weight`).
                market_data
                    .strikes
                    .iter()
                    .zip(market_data.market_vols.iter())
                    .map(|(&strike, &market_vol)| {
                        let w = vega_weight(forward, strike, market_vol, time_to_expiry, beta);
                        model
                            .implied_volatility(forward, strike, time_to_expiry)
                            .map(|model_vol| w * (model_vol - market_vol).powi(2))
                            .unwrap_or(1e6) // Large penalty for invalid parameters
                    })
                    .sum()
            } else {
                1e12 // Very large penalty for invalid parameters
            }
        };

        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let initial = vec![
            initial_alpha_guess(atm_vol, forward, beta), // alpha
            0.3,                                         // nu
            0.0,                                         // rho
        ];

        let bounds = vec![
            (1e-6, 5.0),   // alpha bounds
            (1e-6, 2.0),   // nu bounds
            (-0.99, 0.99), // rho bounds
        ];

        let solution = solver.minimize_with_derivatives(
            objective,
            &derivatives_provider,
            &initial,
            Some(&bounds),
        )?;

        let alpha = solution[0];
        let nu = solution[1];
        let rho = solution[2];

        SabrParameters::new(alpha, beta, nu, rho)
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

    /// Calibrate SABR with ATM volatility pinning.
    ///
    /// Solves for alpha analytically so the model matches ATM vol exactly, then
    /// fits only nu and rho to the smile.
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
    ) -> Result<SabrParameters> {
        Ok(self
            .calibrate_with_atm_pinning_diagnostics(
                forward,
                strikes,
                market_vols,
                time_to_expiry,
                beta,
            )?
            .parameters)
    }

    /// Calibrate with exact ATM pinning and return solver diagnostics.
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward price or decimal rate.
    /// * `strikes` - Strikes in the same units as `forward`, including ATM.
    /// * `market_vols` - Implied volatilities aligned with `strikes`.
    /// * `time_to_expiry` - Time to expiry in years.
    /// * `beta` - Fixed SABR elasticity parameter in `[0, 1]`.
    pub fn calibrate_with_atm_pinning_diagnostics(
        &self,
        forward: f64,
        strikes: &[f64],
        market_vols: &[f64],
        time_to_expiry: f64,
        beta: f64,
    ) -> Result<SabrCalibrationOutcome> {
        if strikes.len() != market_vols.len() || strikes.len() < 3 {
            return Err(Error::Validation(format!(
                "ATM-pinned SABR calibration requires at least three aligned quotes; strikes={}, vols={}",
                strikes.len(),
                market_vols.len()
            )));
        }
        let atm_vol = self.find_atm_vol(forward, strikes, market_vols)?;
        let alpha_start = initial_alpha_guess(atm_vol, forward, beta);
        let starts = deterministic_sabr_starts(alpha_start);
        let residual_tolerance = self.tolerance.sqrt();
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(residual_tolerance)
            .with_max_iterations(self.max_iterations);
        run_deterministic_sabr_starts(
            starts,
            residual_tolerance,
            "ATM-pinned SABR",
            beta,
            |physical_start| {
                let initial = [
                    bounded_to_unconstrained(physical_start[1], 0.001, 2.0),
                    bounded_to_unconstrained(physical_start[2], -0.99, 0.99),
                ];
                let residuals = |unconstrained: &[f64], output: &mut [f64]| {
                    let nu = unconstrained_to_bounded(unconstrained[0], 0.001, 2.0);
                    let rho = unconstrained_to_bounded(unconstrained[1], -0.99, 0.99);
                    let Ok(alpha) = solve_alpha_for_atm(
                        forward,
                        atm_vol,
                        time_to_expiry,
                        beta,
                        nu,
                        rho,
                        self.tolerance,
                    ) else {
                        output.fill(1e6);
                        return;
                    };
                    let Ok(parameters) = SabrParameters::new(alpha, beta, nu, rho) else {
                        output.fill(1e6);
                        return;
                    };
                    let model = SabrModel::new(parameters);
                    for (index, (&strike, &market_vol)) in
                        strikes.iter().zip(market_vols).enumerate()
                    {
                        let is_atm = (strike - forward).abs() / forward.abs().max(1e-8) < 0.001;
                        output[index] = if is_atm {
                            0.0
                        } else {
                            let weight =
                                vega_weight(forward, strike, market_vol, time_to_expiry, beta)
                                    .sqrt();
                            model
                                .implied_volatility(forward, strike, time_to_expiry)
                                .map_or(1e6, |model_vol| weight * (model_vol - market_vol))
                        };
                    }
                };
                solver.solve_system_with_dim_stats(residuals, &initial, market_vols.len())
            },
            |solution| {
                let nu = unconstrained_to_bounded(solution.params[0], 0.001, 2.0);
                let rho = unconstrained_to_bounded(solution.params[1], -0.99, 0.99);
                solve_alpha_for_atm(
                    forward,
                    atm_vol,
                    time_to_expiry,
                    beta,
                    nu,
                    rho,
                    self.tolerance,
                )
                .ok()
                .map(|alpha| [alpha, nu, rho])
            },
        )
        .map(SabrMultiStartResult::into_outcome)
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
    // For normal SABR, ATM volatility is approximately α.
    let f_pow = if beta < BETA_SNAP_TOL {
        1.0
    } else {
        forward.powf(1.0 - beta)
    };
    let mut alpha = target_atm_vol * f_pow;

    const MAX_ITER: usize = 50;

    let mut last_error = f64::INFINITY;
    for _ in 0..MAX_ITER {
        let params = SabrParameters::new(alpha, beta, nu, rho)?;
        let model = SabrModel::new(params);
        let model_vol = model.atm_volatility(forward, time_to_expiry)?;

        let error = model_vol - target_atm_vol;
        last_error = error;
        if error.abs() < tolerance {
            return Ok(alpha);
        }

        // Numerical derivative for Newton step
        let bump = alpha * 1e-6;
        let params_bumped = SabrParameters::new(alpha + bump, beta, nu, rho)?;
        let model_bumped = SabrModel::new(params_bumped);
        let vol_bumped = model_bumped.atm_volatility(forward, time_to_expiry)?;

        let d_vol_d_alpha = (vol_bumped - model_vol) / bump;
        if d_vol_d_alpha.abs() < 1e-14 {
            break; // Can't continue Newton iteration
        }

        // Newton step with damping for stability
        let step = -error / d_vol_d_alpha;
        alpha += step.clamp(-alpha * 0.5, alpha * 0.5);

        if alpha <= 0.0 {
            alpha = target_atm_vol * f_pow * 0.5;
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

#[cfg(test)]
mod acceptance_tests {
    use super::*;
    use finstack_quant_core::math::solver_multi::{LmStats, LmTerminationReason};

    #[test]
    fn sabr_step_too_small_requires_residual_tolerance() {
        let tolerance = 1.0e-4;
        assert!(!sabr_termination_is_acceptable(
            &LmTerminationReason::StepTooSmall,
            1.0e-3,
            tolerance,
        ));
        assert!(sabr_termination_is_acceptable(
            &LmTerminationReason::StepTooSmall,
            1.0e-5,
            tolerance,
        ));
    }

    #[test]
    fn sabr_max_iterations_requires_residual_tolerance_for_both_paths() {
        let tolerance = 1.0e-4;
        assert!(!sabr_termination_is_acceptable(
            &LmTerminationReason::MaxIterations,
            1.0e-3,
            tolerance,
        ));
        assert!(sabr_termination_is_acceptable(
            &LmTerminationReason::MaxIterations,
            1.0e-5,
            tolerance,
        ));
    }

    #[test]
    fn sabr_genuine_convergence_reasons_remain_acceptable() {
        for reason in [
            LmTerminationReason::ConvergedResidualNorm,
            LmTerminationReason::ConvergedRelativeReduction,
            LmTerminationReason::ConvergedGradient,
        ] {
            assert!(sabr_termination_is_acceptable(&reason, 1.0, 1.0e-4));
        }
    }

    #[test]
    fn sabr_numerical_failure_is_never_acceptable() {
        assert!(!sabr_termination_is_acceptable(
            &LmTerminationReason::NumericalFailure,
            0.0,
            1.0e-4,
        ));
    }

    #[test]
    fn deterministic_runner_filters_before_selection_and_retains_rejections() {
        let starts = vec![[0.02, 0.15, -0.6], [0.02, 0.4, 0.0]];
        let result = run_deterministic_sabr_starts(
            starts.clone(),
            1.0e-4,
            "test SABR",
            0.5,
            |start| {
                let (score, reason, iterations) = if start == starts[0] {
                    (1.0e-12, LmTerminationReason::NumericalFailure, 2)
                } else {
                    (1.0e-3, LmTerminationReason::ConvergedGradient, 3)
                };
                Ok(LmSolution {
                    params: vec![0.02, start[1], start[2]],
                    stats: LmStats {
                        iterations,
                        residual_evals: iterations + 1,
                        jacobian_evals: iterations,
                        termination_reason: reason,
                        final_residual_norm: score,
                        final_step_norm: 0.0,
                        lambda_final: 1.0,
                        lambda_bound_hits: 0,
                    },
                })
            },
            |solution| {
                Some(
                    solution
                        .params
                        .as_slice()
                        .try_into()
                        .expect("three parameters"),
                )
            },
        )
        .expect("higher-residual converged candidate should win");

        assert_eq!(result.outcome.winning_start, starts[1]);
        assert_eq!(result.outcome.parameters.alpha, 0.02);
        assert_eq!(result.outcome.parameters.nu, 0.4);
        assert_eq!(result.outcome.parameters.rho, 0.0);
        assert_eq!(result.rejected.rejected_starts, 1);
        assert_eq!(result.rejected.solver_failures, 0);
        assert!(matches!(
            result.rejected.best_rejected,
            Some((score, LmTerminationReason::NumericalFailure, 2))
                if score == 1.0e-12
        ));
    }

    #[test]
    fn free_and_atm_pinned_multi_start_outputs_are_repeatable_and_fit() {
        let forward = 0.03;
        let strikes = [0.02, 0.025, 0.03, 0.035, 0.04];
        let time_to_expiry = 2.0;
        let beta = 0.5;
        let source_parameters =
            SabrParameters::new(0.02, beta, 0.4, -0.3).expect("source parameters");
        let source = SabrModel::new(source_parameters.clone());
        let market_vols: Vec<f64> = strikes
            .iter()
            .map(|&strike| {
                source
                    .implied_volatility(forward, strike, time_to_expiry)
                    .expect("source volatility")
            })
            .collect();
        let calibrator = SabrCalibrator::new()
            .with_tolerance(1.0e-8)
            .with_max_iterations(2_000);

        let calibrate_free = || {
            calibrator
                .calibrate_with_diagnostics(forward, &strikes, &market_vols, time_to_expiry, beta)
                .expect("free-alpha calibration")
        };
        let calibrate_pinned = || {
            calibrator
                .calibrate_with_atm_pinning_diagnostics(
                    forward,
                    &strikes,
                    &market_vols,
                    time_to_expiry,
                    beta,
                )
                .expect("ATM-pinned calibration")
        };
        let free_first = calibrate_free();
        let free_second = calibrate_free();
        let pinned_first = calibrate_pinned();
        let pinned_second = calibrate_pinned();

        let assert_repeatable = |first: &SabrCalibrationOutcome,
                                 second: &SabrCalibrationOutcome| {
            assert_eq!(first.parameters.alpha, second.parameters.alpha);
            assert_eq!(first.parameters.beta, second.parameters.beta);
            assert_eq!(first.parameters.nu, second.parameters.nu);
            assert_eq!(first.parameters.rho, second.parameters.rho);
            assert_eq!(first.parameters.shift, second.parameters.shift);
            assert_eq!(first.total_iterations, second.total_iterations);
            assert_eq!(first.winning_iterations, second.winning_iterations);
            assert_eq!(first.residual_evaluations, second.residual_evaluations);
            assert_eq!(first.winning_start, second.winning_start);
            assert_eq!(first.parameters_at_bounds, second.parameters_at_bounds);
        };
        assert_repeatable(&free_first, &free_second);
        assert_repeatable(&pinned_first, &pinned_second);

        let residual_norm = |outcome: &SabrCalibrationOutcome, pin_atm: bool| {
            let model = SabrModel::new(outcome.parameters.clone());
            strikes
                .iter()
                .zip(&market_vols)
                .map(|(&strike, &market_vol)| {
                    let is_atm = (strike - forward).abs() / forward.abs().max(1.0e-8) < 0.001;
                    if pin_atm && is_atm {
                        0.0
                    } else {
                        let weight =
                            vega_weight(forward, strike, market_vol, time_to_expiry, beta).sqrt();
                        let model_vol = model
                            .implied_volatility(forward, strike, time_to_expiry)
                            .expect("fitted volatility");
                        weight * (model_vol - market_vol)
                    }
                })
                .map(|residual| residual * residual)
                .sum::<f64>()
                .sqrt()
        };
        let residual_tolerance = calibrator.tolerance.sqrt();
        assert!(
            residual_norm(&free_first, false) <= residual_tolerance,
            "free-alpha winner must satisfy the configured residual tolerance"
        );
        assert!(
            residual_norm(&pinned_first, true) <= residual_tolerance,
            "ATM-pinned winner must satisfy the configured residual tolerance"
        );

        let atm_vol = calibrator
            .find_atm_vol(forward, &strikes, &market_vols)
            .expect("ATM volatility");
        let expected_alpha_start = initial_alpha_guess(atm_vol, forward, beta);
        for outcome in [&free_first, &pinned_first] {
            assert!((outcome.winning_start[0] - expected_alpha_start).abs() <= 1.0e-15);
            assert_eq!(outcome.winning_start[1], 0.4);
            assert_eq!(outcome.winning_start[2], 0.0);
            assert_eq!(outcome.parameters.beta, beta);
            assert_eq!(outcome.parameters.shift, None);
            assert!(outcome.parameters_at_bounds.is_empty());
            assert!((outcome.parameters.alpha - source_parameters.alpha).abs() <= 2.0e-6);
            assert!((outcome.parameters.nu - source_parameters.nu).abs() <= 5.0e-4);
            assert!((outcome.parameters.rho - source_parameters.rho).abs() <= 2.0e-4);
        }

        let pinned_atm = SabrModel::new(pinned_first.parameters)
            .atm_volatility(forward, time_to_expiry)
            .expect("pinned ATM volatility");
        assert!((pinned_atm - atm_vol).abs() <= calibrator.tolerance);
    }

    #[test]
    fn free_alpha_path_reports_rejected_stalled_starts() {
        let calibrator = SabrCalibrator::new()
            .with_tolerance(1.0e-12)
            .with_max_iterations(1);
        let error = calibrator
            .calibrate(
                0.03,
                &[0.02, 0.025, 0.03, 0.035, 0.04],
                &[0.02, 0.015, 0.01, 0.015, 0.02],
                5.0,
                0.0,
            )
            .expect_err("one iteration cannot accept this bounded SABR smile");
        let message = error.to_string();

        assert!(message.contains("no acceptable deterministic SABR start"));
        assert!(message.contains("rejected_starts="));
        assert!(message.contains("best_rejected_residual="));
    }

    #[test]
    fn atm_pinned_path_reports_rejected_stalled_starts() {
        let calibrator = SabrCalibrator::new()
            .with_tolerance(1.0e-12)
            .with_max_iterations(1);
        let error = calibrator
            .calibrate_with_atm_pinning(
                0.03,
                &[0.02, 0.025, 0.03, 0.035, 0.04],
                &[0.02, 0.015, 0.01, 0.015, 0.02],
                5.0,
                0.0,
            )
            .expect_err("one iteration cannot accept this bounded ATM-pinned smile");
        let message = error.to_string();

        assert!(message.contains("no acceptable deterministic ATM-pinned SABR start"));
        assert!(message.contains("rejected_starts="));
        assert!(message.contains("best_rejected_residual="));
    }
}
