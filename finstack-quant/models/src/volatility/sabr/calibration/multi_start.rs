//! Multi-start orchestration, parameter bounds transforms, and analytical Jacobian for SABR calibration.

use super::super::parameters::SabrParameters;
use super::calibrator::SabrCalibrationOutcome;
use finstack_quant_core::math::solver_multi::{LmSolution, LmTerminationReason};
use finstack_quant_core::{Error, Result};

pub(super) fn bounded_to_unconstrained(value: f64, lower: f64, upper: f64) -> f64 {
    let unit = ((value - lower) / (upper - lower)).clamp(1e-9, 1.0 - 1e-9);
    (unit / (1.0 - unit)).ln()
}

pub(super) fn unconstrained_to_bounded(value: f64, lower: f64, upper: f64) -> f64 {
    let unit = if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp_value = value.exp();
        exp_value / (1.0 + exp_value)
    };
    lower + (upper - lower) * unit
}

pub(super) fn sabr_parameters_at_bounds(_alpha: f64, nu: f64, rho: f64) -> Vec<String> {
    const BOUNDS: [(&str, f64, f64); 2] = [("nu", 0.001, 2.0), ("rho", -0.99, 0.99)];
    [nu, rho]
        .into_iter()
        .zip(BOUNDS)
        .filter_map(|(value, (name, lower, upper))| {
            let threshold = 1e-5 * (upper - lower);
            ((value - lower).abs() <= threshold || (upper - value).abs() <= threshold)
                .then(|| name.to_owned())
        })
        .collect()
}

pub(super) fn deterministic_sabr_starts(alpha: f64) -> Vec<[f64; 3]> {
    const NU_STARTS: [f64; 3] = [0.15, 0.4, 0.9];
    const RHO_STARTS: [f64; 3] = [-0.6, 0.0, 0.6];
    let mut starts = Vec::with_capacity(NU_STARTS.len() * RHO_STARTS.len());
    for nu in NU_STARTS {
        for rho in RHO_STARTS {
            starts.push([alpha, nu, rho]);
        }
    }
    starts
}

/// A start is acceptable only when LM converged or stopped on a small step
/// with a finite residual norm within `residual_tolerance`.
pub(super) fn sabr_termination_is_acceptable(
    termination_reason: finstack_quant_core::math::solver_multi::LmTerminationReason,
    final_residual_norm: f64,
    residual_tolerance: f64,
) -> bool {
    matches!(
        termination_reason,
        finstack_quant_core::math::solver_multi::LmTerminationReason::ConvergedResidualNorm
            | finstack_quant_core::math::solver_multi::LmTerminationReason::StepTooSmall
    ) && final_residual_norm.is_finite()
        && final_residual_norm <= residual_tolerance
}

#[derive(Default)]
pub(super) struct RejectedSabrStarts {
    pub(super) rejected_starts: usize,
    pub(super) solver_failures: usize,
    pub(super) best_rejected: Option<(f64, LmTerminationReason, usize)>,
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

pub(super) struct SabrMultiStartResult {
    pub(super) outcome: SabrCalibrationOutcome,
    pub(super) rejected: RejectedSabrStarts,
}

impl SabrMultiStartResult {
    pub(super) fn into_outcome(self) -> SabrCalibrationOutcome {
        let Self {
            outcome,
            rejected: _rejected,
        } = self;
        outcome
    }
}

pub(super) fn run_deterministic_sabr_starts<Solve, Reconstruct>(
    starts: Vec<[f64; 3]>,
    residual_tolerance: f64,
    path: &str,
    beta: f64,
    mut solve: Solve,
    mut reconstruct: Reconstruct,
) -> Result<SabrMultiStartResult>
where
    Solve: FnMut([f64; 3]) -> (Result<LmSolution>, usize),
    Reconstruct: FnMut(&LmSolution) -> Option<[f64; 3]>,
{
    let mut best: Option<(f64, LmSolution, [f64; 3], [f64; 3])> = None;
    let mut rejected = RejectedSabrStarts::default();
    let mut total_iterations = 0;
    let mut residual_evaluations = 0;

    for physical_start in starts {
        let (solution, calls) = solve(physical_start);
        residual_evaluations += calls;
        let Ok(solution) = solution else {
            rejected.record_solver_failure();
            continue;
        };
        total_iterations += solution.stats.iterations;
        let score = solution.stats.final_residual_norm;
        if !sabr_termination_is_acceptable(
            solution.stats.termination_reason.clone(),
            score,
            residual_tolerance,
        ) {
            rejected.record_rejected(&solution);
            continue;
        }
        let Some(physical) = reconstruct(&solution) else {
            rejected.record_rejected(&solution);
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
            residual_evaluations,
            winning_start,
            quote_errors: Vec::new(),
            max_relative_quote_error: f64::INFINITY,
            parameters_at_bounds: sabr_parameters_at_bounds(physical[0], physical[1], physical[2]),
        },
        rejected,
    })
}

pub(super) fn compute_sabr_jacobian(
    strikes: &[super::super::expansion::PreparedSabrStrike],
    weights: &[f64],
    beta: f64,
    coordinates: &[f64],
    output: &mut [f64],
) -> finstack_quant_core::Result<()> {
    let alpha = coordinates[0].exp();
    let nu = unconstrained_to_bounded(coordinates[1], 0.001, 2.0);
    let rho = unconstrained_to_bounded(coordinates[2], -0.99, 0.99);
    let parameters = SabrParameters::new(alpha, beta, nu, rho)?;
    let chain = [
        alpha,
        (nu - 0.001) * (2.0 - nu) / (2.0 - 0.001),
        (rho + 0.99) * (0.99 - rho) / 1.98,
    ];
    for (i, (strike, weight)) in strikes.iter().zip(weights).enumerate() {
        let physical = strike.gradient(&parameters)?;
        for j in 0..3 {
            output[i * 3 + j] = weight * physical[j] * chain[j];
        }
    }
    Ok(())
}
