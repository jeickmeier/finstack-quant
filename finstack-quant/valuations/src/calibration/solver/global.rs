//! Simultaneous weighted least-squares calibration using Levenberg–Marquardt.
//!
//! Notes:
//! - Optional multi-start support to escape local minima (see [`MultiStartConfig`]).
//! - Residual weighting is supported via per-quote weights (weighted least squares).

use super::multi_start::{perturb_initial_guess, MultiStartConfig};
use super::traits::GlobalSolveTarget;
use crate::calibration::constants::PENALTY;
use crate::calibration::report::{CalibrationDiagnostics, QuoteQuality};
use crate::calibration::{CalibrationConfig, CalibrationReport};
use finstack_quant_core::Result;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// Fill `resid` with a penalty term that gives LM a direction pointing back into the
/// feasible bound box.
///
/// History: an earlier implementation added a `‖p‖²`-dependent term, which makes the LM
/// normal equations minimize `‖p‖` rather than push toward feasibility. A later revision
/// fixed the *direction* but broadcast a single scalar to every residual — that scalar is
/// constant in each parameter, so the finite-difference Jacobian gets a **zero column**
/// for any parameter whose ±h neighbourhood is also infeasible. With a zero column LM has
/// no descent direction for that parameter and freezes it at its seed.
///
/// This implementation makes the penalty **parameter-dependent per residual**: each
/// residual `k` carries a smooth bound-distance penalty for parameter `k % n_params`, so
/// every parameter influences at least one residual and its Jacobian column is non-zero.
/// The penalty is `0` when a parameter is interior and grows smoothly with the signed
/// violation distance, so the FD column points monotonically back toward the feasible
/// region. A constant floor (`PENALTY · FLOOR_FRACTION`) is added on top so the residual
/// vector stays recognisably "penalised" (large `|r|`) for the caller-side validity
/// checks even when no individual parameter is violated (e.g. an infeasible region the
/// bounds do not describe, such as a curve builder rejecting otherwise in-bounds params).
fn fill_penalty(
    resid: &mut [f64],
    n_residuals: usize,
    params: &[f64],
    lb: Option<&[f64]>,
    ub: Option<&[f64]>,
) {
    let n = n_residuals.min(resid.len());
    if n == 0 {
        return;
    }
    let n_params = params.len();

    // Per-parameter signed bound-violation distance (0 if interior). A smooth,
    // strictly-increasing transform keeps the FD step sensible while still giving a
    // non-zero gradient: d/dx [PENALTY·x/(1+x)] > 0 for all x >= 0.
    let param_penalty = |idx: usize| -> f64 {
        if idx >= n_params {
            return 0.0;
        }
        let p = params[idx];
        if !p.is_finite() {
            // Non-finite parameter: emit the full penalty so LM is pushed hard and the
            // residual vector is flagged invalid by downstream checks.
            return PENALTY;
        }
        let mut dist = 0.0_f64;
        if let Some(lo) = lb {
            if let Some(&b) = lo.get(idx) {
                if p < b {
                    dist += b - p;
                }
            }
        }
        if let Some(hi) = ub {
            if let Some(&b) = hi.get(idx) {
                if p > b {
                    dist += p - b;
                }
            }
        }
        if dist > 0.0 {
            PENALTY * (dist / (1.0 + dist))
        } else {
            0.0
        }
    };

    // Constant floor so the residual vector stays large enough that callers treating
    // `|r| >= OBJECTIVE/RESIDUAL` thresholds as "infeasible eval" still trip. Kept well
    // below `PENALTY` so the parameter-dependent term dominates the Jacobian columns.
    const FLOOR_FRACTION: f64 = 0.5;
    let floor = PENALTY * FLOOR_FRACTION;

    if n_params == 0 {
        // No parameters to key on — fall back to a flat penalty (J^T r ≈ 0 from a
        // constant; LM relies on its trust-region step). This branch is unreachable for
        // real targets, which always have at least one knot.
        for r in resid.iter_mut().take(n) {
            *r = PENALTY;
        }
        return;
    }

    for (k, r) in resid.iter_mut().take(n).enumerate() {
        // Round-robin parameter assignment guarantees coverage of every parameter when
        // `n_residuals >= n_params` (the global solver enforces this in
        // `validate_global_inputs`).
        *r = floor + param_penalty(k % n_params);
    }
}

/// Simultaneous weighted least-squares optimizer with optional multi-start.
///
/// Implements a global optimization approach that fits all knots simultaneously
/// by minimizing the sum of weighted squared residuals. This is particularly
/// useful for overdetermined systems (e.g., fitting a smooth curve to many
/// noisy market quotes), multi-curve systems with complex inter-dependencies,
/// or when analytical Jacobians are available for performance.
///
/// Under the hood, it uses the Levenberg–Marquardt algorithm from `finstack-quant-core`,
/// which provides robust convergence by damping the Gauss-Newton step toward
/// gradient descent when in non-linear or ill-conditioned regions.
///
/// When [`MultiStartConfig`] is provided, the optimizer runs additional LM solves
/// from deterministically-perturbed starting points and returns the best result.
pub(crate) struct GlobalFitOptimizer;

impl GlobalFitOptimizer {
    /// Execute a simultaneous weighted least-squares fit.
    ///
    /// # Generic Parameters
    /// * `T` - The calibration target (e.g., [`DiscountCurveTarget`](crate::calibration::targets::discount::DiscountCurveTarget)).
    ///
    /// # Arguments
    /// * `target` - The domain-specific implementation of the [`GlobalSolveTarget`] trait.
    /// * `quotes` - The list of high-level market quotes to fit.
    /// * `config` - Calibration settings specifying tolerances and methods.
    /// * `success_tolerance` - Target-specific validation tolerance for determining calibration success.
    ///   If `None`, falls back to `config.discount_curve.validation_tolerance`.
    ///
    /// # Returns
    /// A pair containing the calibrated term structure and a diagnostic report.
    ///
    /// # Tolerance Semantics
    /// Success is determined by comparing the **weighted L2 norm of the residual vector**
    /// (i.e., `sqrt(sum((r_i * sqrt(w_i))^2))`) against the `success_tolerance`.
    pub(crate) fn optimize<T>(
        target: &T,
        quotes: &[T::Quote],
        config: &CalibrationConfig,
        success_tolerance: Option<f64>,
    ) -> Result<(T::Curve, CalibrationReport)>
    where
        T: GlobalSolveTarget,
    {
        Self::optimize_with_multi_start(target, quotes, config, success_tolerance, None)
    }

    /// Execute a simultaneous weighted least-squares fit with optional multi-start.
    ///
    /// When `multi_start` is `Some`, the optimizer runs `num_restarts` additional solves
    /// from deterministically-perturbed starting points. The best result (lowest weighted
    /// residual norm) is returned.
    pub(crate) fn optimize_with_multi_start<T>(
        target: &T,
        quotes: &[T::Quote],
        config: &CalibrationConfig,
        success_tolerance: Option<f64>,
        multi_start: Option<&MultiStartConfig>,
    ) -> Result<(T::Curve, CalibrationReport)>
    where
        T: GlobalSolveTarget,
    {
        // 1. Build grid and guesses
        let (times, initials, active_quotes) = target.build_time_grid_and_guesses(quotes)?;
        let n_residuals = active_quotes.len();

        if n_residuals == 0 {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        validate_global_inputs(&times, &initials, n_residuals)?;

        let mut weights = vec![1.0_f64; n_residuals];
        target.residual_weights(&active_quotes, &mut weights)?;
        for (idx, w) in weights.iter().enumerate() {
            if !w.is_finite() || *w < 0.0 {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Global solve requires non-negative finite residual weights; got {} at index {}.",
                        w, idx
                    ),
                    category: "global_solve".to_string(),
                });
            }
        }
        if weights.iter().all(|w| *w == 0.0) {
            return Err(finstack_quant_core::Error::Calibration {
                message: "Global solve requires at least one positive residual weight.".to_string(),
                category: "global_solve".to_string(),
            });
        }
        let weight_scales: Vec<f64> = weights.iter().map(|w| w.sqrt()).collect();

        let lb = target.lower_bounds();
        let ub = target.upper_bounds();

        // Run the primary solve from the original initial guess. A hard LM error is
        // logged but does *not* short-circuit multi-start: if perturbed restarts exist
        // they may succeed where the primary failed (e.g. the initial guess landed in a
        // region where the curve builder rejected the params). The original error is
        // returned only if every attempt fails.
        let mut primary_error: Option<finstack_quant_core::Error> = None;
        // Tracks which start produced the best result so far: None = primary solve,
        // Some(i) = multi-start restart i. Recorded in the calibration report so a
        // user can see whether the answer came from the originally-supplied initial
        // guess or required a perturbation to escape a bad region.
        let mut winning_start: Option<usize> = None;
        let mut best: Option<(SingleSolveResult, f64)> = match run_single_solve(
            target,
            &active_quotes,
            &times,
            &initials,
            &weight_scales,
            &lb,
            &ub,
            config,
        ) {
            Ok(v) => Some(v),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "GlobalFitOptimizer: primary LM solve failed; will attempt multi-start if configured"
                );
                primary_error = Some(err);
                None
            }
        };

        // Multi-start: run additional solves from perturbed starting points.
        if let Some(ms) = multi_start {
            if config.verbose {
                tracing::info!(
                    "GlobalFitOptimizer: running {} multi-start restarts (perturbation_scale={:.3})",
                    ms.num_restarts,
                    ms.perturbation_scale,
                );
            }

            for restart_idx in 0..ms.num_restarts {
                let perturbed = perturb_initial_guess(
                    &initials,
                    ms.perturbation_scale,
                    restart_idx,
                    lb.as_deref(),
                    ub.as_deref(),
                );

                match run_single_solve(
                    target,
                    &active_quotes,
                    &times,
                    &perturbed,
                    &weight_scales,
                    &lb,
                    &ub,
                    config,
                ) {
                    Ok((result, wl2)) => {
                        let improved = best.as_ref().is_none_or(|(_, prev)| wl2 < *prev);
                        if improved {
                            if config.verbose {
                                tracing::info!(
                                    "GlobalFitOptimizer: restart {} improved weighted L2: {:?} -> {:.4e}",
                                    restart_idx,
                                    best.as_ref().map(|(_, l)| *l),
                                    wl2,
                                );
                            }
                            best = Some((result, wl2));
                            winning_start = Some(restart_idx);
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            restart = restart_idx,
                            error = %err,
                            "GlobalFitOptimizer: multi-start restart failed, skipping",
                        );
                    }
                }
            }
        }

        // If the primary solve failed and no restart rescued it, propagate the original
        // error rather than feeding a sentinel into the final-curve build.
        let Some((best_result, _best_weighted_l2)) = best else {
            return Err(
                primary_error.unwrap_or_else(|| finstack_quant_core::Error::Calibration {
                    message: "GlobalFitOptimizer: every solve attempt failed".to_string(),
                    category: "global_solve".to_string(),
                }),
            );
        };
        let (solved_params, stats, eval_counter_val, eval_diagnostics_val) = best_result;

        // Build final curve
        let final_curve = target.build_curve_final_from_params(&times, &solved_params)?;

        // Build report
        let mut residuals_map = BTreeMap::new();
        let mut resid_values = vec![0.0; n_residuals];
        target.calculate_residuals(&final_curve, &active_quotes, &mut resid_values)?;

        for (i, (&val, quote)) in resid_values.iter().zip(active_quotes.iter()).enumerate() {
            residuals_map.insert(target.residual_key(quote, i), val.abs());
        }

        let l2_norm: f64 = resid_values.iter().map(|r| r * r).sum::<f64>().sqrt();
        let weighted_l2_norm: f64 = resid_values
            .iter()
            .zip(weight_scales.iter())
            .map(|(r, w)| (r * w).powi(2))
            .sum::<f64>()
            .sqrt();
        let max_abs_residual = resid_values.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
        let weighted_max_abs_residual = resid_values
            .iter()
            .zip(weight_scales.iter())
            .map(|(r, w)| (r * w).abs())
            .fold(0.0_f64, f64::max);

        // `validation_tolerance` is the accept/reject threshold for the final residual,
        // distinct from the LM solver convergence tolerance. See calibration/README.md.
        let validation_tolerance =
            success_tolerance.unwrap_or(config.discount_curve.validation_tolerance);

        // Success requires BOTH the weighted L2 norm AND the max individual residual
        // to be within tolerance. The L2 norm alone can mask outlier instruments when
        // many quotes fit well but one fits poorly.
        //
        // Both sides of the gate must be expressed on the SAME (weighted) residual
        // scale. The L2 term is `sqrt(Σ (r_i·√w_i)^2)`; for a uniform per-quote error
        // `e` on the weighted scale, that norm is `e·√n`, so the per-quote tolerance
        // is `validation_tolerance·√n`. The max term must therefore also use the
        // *weighted* max residual `max_i |r_i·√w_i|` — comparing an unweighted
        // `max_i |r_i|` against a √n-scaled tolerance mixes units and can spuriously
        // pass (or fail) whenever the weights differ from 1.
        let max_residual_tolerance = validation_tolerance * (n_residuals as f64).sqrt();
        let calibration_success = weighted_l2_norm <= validation_tolerance
            && weighted_max_abs_residual <= max_residual_tolerance;

        let mut report = CalibrationReport::for_type_with_tolerance(
            "global_fit",
            residuals_map,
            stats.iterations,
            validation_tolerance,
        );
        // Override success based on weighted L2 + max-residual criteria.
        report.success = calibration_success;
        report.objective_value = weighted_l2_norm;
        // Always surface the LM termination reason so callers can distinguish
        // "gradient tolerance met" (clean convergence) from "max iterations
        // reached but residuals within tolerance" (acceptable but suspect).
        if calibration_success {
            report.convergence_reason = format!(
                "global fit succeeded: LM terminated with {:?}; weighted L2 norm {:.2e} <= tolerance {:.2e}; \
                 weighted max residual {:.2e} <= per-quote tolerance {:.2e}",
                stats.termination_reason,
                weighted_l2_norm,
                validation_tolerance,
                weighted_max_abs_residual,
                max_residual_tolerance,
            );
        } else if weighted_l2_norm > validation_tolerance
            && weighted_max_abs_residual > max_residual_tolerance
        {
            report.convergence_reason = format!(
                "global fit calibration failed: LM terminated with {:?}; weighted L2 norm ({:.2e}) exceeds \
                 tolerance ({:.2e}) and weighted max residual ({:.2e}) exceeds per-quote tolerance ({:.2e})",
                stats.termination_reason,
                weighted_l2_norm, validation_tolerance, weighted_max_abs_residual, max_residual_tolerance,
            );
        } else if weighted_max_abs_residual > max_residual_tolerance {
            report.convergence_reason = format!(
                "global fit calibration failed: LM terminated with {:?}; weighted max residual ({:.2e}) exceeds \
                 per-quote tolerance ({:.2e}), weighted L2 norm ({:.2e}) passed",
                stats.termination_reason,
                weighted_max_abs_residual, max_residual_tolerance, weighted_l2_norm,
            );
        } else {
            report.convergence_reason = format!(
                "global fit calibration failed: LM terminated with {:?}; weighted L2 norm ({:.2e}) exceeds \
                 tolerance ({:.2e}), weighted max residual ({:.2e}) passed per-quote tolerance ({:.2e})",
                stats.termination_reason,
                weighted_l2_norm,
                validation_tolerance,
                weighted_max_abs_residual,
                max_residual_tolerance,
            );
        }

        if !calibration_success {
            // Surface the worst-fit quotes without requiring diagnostics.
            let worst = top_k_worst_fits(target, &active_quotes, &resid_values, 3);
            if !worst.is_empty() {
                report.convergence_reason.push_str(". Worst fits: ");
                report.convergence_reason.push_str(&worst);
            }
        }

        report = report
            .with_metadata("method", "global_fit_lm_weighted_lsq")
            .with_metadata("tolerance_definition", "weighted_l2_norm_and_max_residual")
            .with_metadata(
                "validation_tolerance",
                format!("{:.2e}", validation_tolerance),
            )
            .with_metadata(
                "solver_tolerance",
                format!("{:.2e}", config.solver.tolerance()),
            )
            .with_metadata("residual_evals", stats.residual_evals.to_string())
            .with_metadata("residual_closure_evals", eval_counter_val.to_string())
            .with_metadata(
                "lm_termination_reason",
                format!("{:?}", stats.termination_reason),
            )
            .with_metadata("lm_jacobian_evals", stats.jacobian_evals.to_string())
            .with_metadata(
                "lm_final_weighted_resid_l2_norm",
                format!("{:.2e}", stats.final_residual_norm),
            )
            .with_metadata(
                "lm_final_step_norm",
                format!("{:.2e}", stats.final_step_norm),
            )
            .with_metadata("lm_lambda_final", format!("{:.2e}", stats.lambda_final))
            .with_metadata("final_unweighted_resid_l2_norm", format!("{:.2e}", l2_norm))
            .with_metadata(
                "final_unweighted_max_abs_residual",
                format!("{:.2e}", max_abs_residual),
            )
            .with_metadata(
                "final_weighted_resid_l2_norm",
                format!("{:.2e}", weighted_l2_norm),
            )
            .with_metadata(
                "final_weighted_max_abs_residual",
                format!("{:.2e}", weighted_max_abs_residual),
            );

        if let Some(ms) = multi_start {
            report = report.with_metadata("multi_start_restarts", ms.num_restarts.to_string());
            // Surface which start produced the best result so users can tell whether
            // the primary initial guess won or a perturbation rescued it.
            let winner_label = match winning_start {
                None => "primary".to_string(),
                Some(i) => format!("restart_{}", i),
            };
            report = report.with_metadata("multi_start_winner", winner_label);
        }

        // Attach diagnostics from any infeasible evaluations encountered during solving.
        {
            report.metadata.insert(
                "invalid_eval_count".to_string(),
                eval_diagnostics_val.invalid_eval_count.to_string(),
            );
            if let Some(first) = &eval_diagnostics_val.first_invalid_eval {
                report
                    .metadata
                    .insert("first_invalid_eval".to_string(), first.clone());
            }
        }

        report.update_solver_config(config.solver.clone());

        // Compute optional diagnostics if requested.
        if config.compute_diagnostics {
            let diagnostics = compute_global_diagnostics(
                target,
                &active_quotes,
                &times,
                &solved_params,
                &resid_values,
                &weight_scales,
                config,
            );
            report = report.with_diagnostics(diagnostics);
        }

        Ok((final_curve, report))
    }
}

// The Halton multi-start helpers (`halton`, `perturb_initial_guess`, and
// `MultiStartConfig`) live in `super::multi_start` so sibling
// calibration targets can reuse the same deterministic perturbation
// strategy. See that module for documentation, references, and unit
// tests.

type SingleSolveResult = (
    Vec<f64>,
    finstack_quant_core::math::solver_multi::LmStats,
    usize,
    EvalDiagnostics,
);

/// Run a single LM solve from the given initial guess. Returns (solved_params, stats,
/// eval_count, diagnostics) and the weighted L2 norm of the final residuals.
#[allow(clippy::too_many_arguments)]
fn run_single_solve<T>(
    target: &T,
    active_quotes: &[T::Quote],
    times: &[f64],
    initials: &[f64],
    weight_scales: &[f64],
    lb: &Option<Vec<f64>>,
    ub: &Option<Vec<f64>>,
    config: &CalibrationConfig,
) -> Result<(SingleSolveResult, f64)>
where
    T: GlobalSolveTarget,
{
    let n_residuals = active_quotes.len();

    let use_efficient = match config.calibration_method {
        crate::calibration::config::CalibrationMethod::GlobalSolve {
            use_analytical_jacobian,
        } => use_analytical_jacobian && target.supports_efficient_jacobian(),
        crate::calibration::config::CalibrationMethod::Bootstrap => false,
    };

    let solver = config.create_lm_solver();

    let eval_diagnostics: RefCell<EvalDiagnostics> = RefCell::new(EvalDiagnostics::default());
    let eval_counter: Cell<usize> = Cell::new(0);

    // Reuse a local buffer across LM residual evaluations when bounds
    // clamping is active. SEQUENTIAL-ONLY: this `RefCell` assumes the
    // residual closure is called from a single thread. The framework
    // currently runs LM solves and multi-start restarts serially. If a
    // future change parallelises restarts via `rayon::par_iter()` or moves
    // the closure across threads, switch to `Mutex<Vec<f64>>` (or a
    // per-thread local) — `RefCell` will panic on concurrent borrow_mut.
    let clamp_buffer: RefCell<Vec<f64>> = RefCell::new(Vec::with_capacity(initials.len()));

    let residuals_func = |params: &[f64], resid: &mut [f64]| {
        let eval_idx = eval_counter.get() + 1;
        eval_counter.set(eval_idx);

        if resid.len() < n_residuals {
            record_eval_error(
                &eval_diagnostics,
                eval_idx,
                "residual buffer",
                params,
                &format!(
                    "residual buffer too small: got {}, need {}",
                    resid.len(),
                    n_residuals
                ),
            );
            fill_penalty(resid, resid.len(), params, lb.as_deref(), ub.as_deref());
            return;
        }

        for r in resid.iter_mut().take(n_residuals) {
            *r = 0.0;
        }

        let mut params_to_use = clamp_buffer.borrow_mut();
        let params_ref: &[f64] = if lb.is_some() || ub.is_some() {
            let n_clamped = clamp_to_bounds(params, lb, ub, &mut params_to_use);
            if n_clamped > 0 {
                record_eval_error(
                    &eval_diagnostics,
                    eval_idx,
                    "bound clamping",
                    params,
                    &format!("{n_clamped} param(s) clamped to bounds"),
                );
            }
            &params_to_use[..]
        } else {
            params
        };

        let curve = match target.build_curve_for_solver_from_params(times, params_ref) {
            Ok(c) => c,
            Err(e) => {
                record_eval_error(
                    &eval_diagnostics,
                    eval_idx,
                    "curve construction",
                    params_ref,
                    &format!("{}", e),
                );
                fill_penalty(resid, n_residuals, params_ref, lb.as_deref(), ub.as_deref());
                return;
            }
        };

        if let Err(e) = target.calculate_residuals(&curve, active_quotes, &mut resid[..n_residuals])
        {
            record_eval_error(
                &eval_diagnostics,
                eval_idx,
                "residual evaluation",
                params_ref,
                &format!("while evaluating {} quotes: {}", active_quotes.len(), e),
            );
            fill_penalty(resid, n_residuals, params_ref, lb.as_deref(), ub.as_deref());
            return;
        }

        // Guard against NaN/Inf in residual values. LM consumes residuals as f64 and
        // does not natively reject non-finite entries: a single NaN poisons the entire
        // Jacobian step and can cause the solver to stall or accept a bogus minimum.
        // Treat non-finite as evaluation failure and substitute the penalty pattern.
        if let Some(bad_idx) = resid[..n_residuals].iter().position(|r| !r.is_finite()) {
            record_eval_error(
                &eval_diagnostics,
                eval_idx,
                "residual evaluation",
                params_ref,
                &format!(
                    "non-finite residual at index {} (value {:?}); substituting penalty",
                    bad_idx, resid[bad_idx]
                ),
            );
            fill_penalty(resid, n_residuals, params_ref, lb.as_deref(), ub.as_deref());
            return;
        }

        for (r, w) in resid[..n_residuals].iter_mut().zip(weight_scales.iter()) {
            *r *= *w;
        }
    };

    type OptionalBoundRef<'a> = &'a Option<Vec<f64>>;
    struct TargetDerivatives<'a, T: GlobalSolveTarget> {
        target: &'a T,
        active_quotes: &'a [T::Quote],
        weight_scales: &'a [f64],
        times: &'a [f64],
        lb: OptionalBoundRef<'a>,
        ub: OptionalBoundRef<'a>,
    }

    impl<'a, T: GlobalSolveTarget> finstack_quant_core::math::solver_multi::AnalyticalDerivatives
        for TargetDerivatives<'a, T>
    {
        fn gradient(&self, _params: &[f64], _gradient: &mut [f64]) {}

        fn has_gradient(&self) -> bool {
            false
        }

        fn jacobian(&self, params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
            let mut params_to_use = Vec::new();
            let params_ref = if self.lb.is_some() || self.ub.is_some() {
                clamp_to_bounds(params, self.lb, self.ub, &mut params_to_use);
                &params_to_use
            } else {
                params
            };

            if self
                .target
                .build_curve_for_solver_from_params(self.times, params_ref)
                .is_err()
            {
                return None;
            }

            if self
                .target
                .jacobian(params_ref, self.times, self.active_quotes, jacobian)
                .is_err()
            {
                return None;
            }

            for (i, row) in jacobian.iter_mut().enumerate() {
                let scale = self.weight_scales.get(i).copied().unwrap_or(1.0);
                for val in row.iter_mut() {
                    *val *= scale;
                }
            }
            Some(())
        }

        fn has_jacobian(&self) -> bool {
            true
        }

        fn residual_count(&self) -> Option<usize> {
            Some(self.active_quotes.len())
        }
    }

    let solution = if use_efficient {
        let derivatives = TargetDerivatives {
            target,
            active_quotes,
            weight_scales,
            times,
            lb,
            ub,
        };
        solver.solve_system_with_jacobian_stats(residuals_func, &derivatives, initials)?
    } else {
        solver.solve_system_with_dim_stats(residuals_func, initials, n_residuals)?
    };
    // The LM core (`solve_system_with_dim_stats` / `solve_system_with_jacobian_stats`)
    // runs without native box constraints: the residual closure clamps a *local*
    // pricing copy but LM keeps the unclamped iterate. As a result the converged
    // `solution.params` can land outside the target's `lower_bounds()`/`upper_bounds()`.
    // Clamp the returned parameters to the bounds here, BEFORE the residuals are
    // re-evaluated below, so the reported curve and residuals correspond to the
    // same (in-bounds) parameter set rather than a stale unclamped one.
    let solved_params = if lb.is_some() || ub.is_some() {
        let mut clamped = Vec::with_capacity(solution.params.len());
        clamp_to_bounds(&solution.params, lb, ub, &mut clamped);
        clamped
    } else {
        solution.params
    };
    let stats = solution.stats;

    // Compute weighted L2 norm of the final residuals for comparison.
    let final_curve = target.build_curve_for_solver_from_params(times, &solved_params)?;
    let mut resid_values = vec![0.0; n_residuals];
    target.calculate_residuals(&final_curve, active_quotes, &mut resid_values)?;

    // Reject solves that produced non-finite residuals at the optimum: the solver may
    // have stalled at a degenerate point and any reported L2 norm would be unreliable.
    if let Some(bad_idx) = resid_values.iter().position(|r| !r.is_finite()) {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global solve converged with non-finite residual at index {} (value {:?}); \
                 final point is unusable.",
                bad_idx, resid_values[bad_idx]
            ),
            category: "global_solve".to_string(),
        });
    }

    let weighted_l2: f64 = resid_values
        .iter()
        .zip(weight_scales.iter())
        .map(|(r, w)| (r * w).powi(2))
        .sum::<f64>()
        .sqrt();

    let eval_count = eval_counter.get();
    let diagnostics = eval_diagnostics.into_inner();

    Ok(((solved_params, stats, eval_count, diagnostics), weighted_l2))
}

fn validate_global_inputs(times: &[f64], initials: &[f64], n_residuals: usize) -> Result<()> {
    if times.len() != initials.len() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global solve requires times.len() == initials.len(); got {} vs {}.",
                times.len(),
                initials.len()
            ),
            category: "global_solve".to_string(),
        });
    }

    if initials.is_empty() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global fit requires at least one parameter; got n_params=0 and n_residuals={}.",
                n_residuals
            ),
            category: "global_solve".to_string(),
        });
    }

    if n_residuals < initials.len() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global fit requires n_residuals >= n_params for a stable least-squares solve; got {} vs {}.",
                n_residuals,
                initials.len()
            ),
            category: "global_solve".to_string(),
        });
    }

    for (idx, &t) in times.iter().enumerate() {
        if !t.is_finite() || t < 0.0 {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires non-negative finite times; got {} at index {}.",
                    t, idx
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    for (idx, &init) in initials.iter().enumerate() {
        if !init.is_finite() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires finite initial guesses; got {} at index {}.",
                    init, idx
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    for idx in 1..times.len() {
        let prev = times[idx - 1];
        let next = times[idx];
        if next <= prev {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires strictly increasing times; indices {} -> {} have values {} -> {}.",
                    idx - 1,
                    idx,
                    prev,
                    next
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct EvalDiagnostics {
    invalid_eval_count: usize,
    first_invalid_eval: Option<String>,
}

fn record_eval_error(
    store: &RefCell<EvalDiagnostics>,
    eval_idx: usize,
    stage: &str,
    params: &[f64],
    detail: &str,
) {
    let mut diag = store.borrow_mut();
    diag.invalid_eval_count += 1;
    if diag.first_invalid_eval.is_some() {
        return;
    }
    let (min_param, max_param) = param_range(params);
    diag.first_invalid_eval = Some(format!(
        "Global fit {stage} failed at eval #{eval_idx} (param_range=[{min:.4e}, {max:.4e}]): {detail}",
        stage = stage,
        eval_idx = eval_idx,
        min = min_param,
        max = max_param,
        detail = detail
    ));
}

/// Inward offset applied when clamping solver parameters to bounds.
///
/// Keeps parameters strictly interior to the feasible region so that
/// finite-difference Jacobian perturbations (typically ~1e-7) do not land
/// exactly on the boundary, which would create a flat gradient plateau.
const BOUND_INWARD_EPS: f64 = 1e-8;

/// Clamp `params` to `[lower + eps, upper - eps]` and return the number of
/// parameters that were actually clamped.
fn clamp_to_bounds(
    params: &[f64],
    lb: &Option<Vec<f64>>,
    ub: &Option<Vec<f64>>,
    out: &mut Vec<f64>,
) -> usize {
    out.clear();
    out.reserve(params.len());
    let mut clamped = 0usize;
    for (i, &p) in params.iter().enumerate() {
        let mut v = p;
        if let Some(ref lower) = lb {
            if i < lower.len() {
                let lo = lower[i] + BOUND_INWARD_EPS;
                if v < lo {
                    v = lo;
                    clamped += 1;
                }
            }
        }
        if let Some(ref upper) = ub {
            if i < upper.len() {
                let hi = upper[i] - BOUND_INWARD_EPS;
                if v > hi {
                    v = hi;
                    clamped += 1;
                }
            }
        }
        out.push(v);
    }
    clamped
}

/// Return a comma-separated string naming the `k` quotes with the largest
/// absolute residuals, formatted as `"<key>=±<resid>"`. Used to make
/// calibration failure messages actionable without requiring the user to
/// re-run with `compute_diagnostics=true`.
fn top_k_worst_fits<T>(target: &T, quotes: &[T::Quote], residuals: &[f64], k: usize) -> String
where
    T: GlobalSolveTarget,
{
    let mut indexed: Vec<(usize, f64)> = residuals
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.abs()))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    indexed
        .into_iter()
        .take(k)
        .filter_map(|(i, mag)| {
            quotes
                .get(i)
                .map(|q| format!("{}={:.2e}", target.residual_key(q, i), mag))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn param_range(params: &[f64]) -> (f64, f64) {
    if params.is_empty() {
        return (0.0, 0.0);
    }
    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;
    for &p in params {
        if p.is_finite() {
            if p < min_val {
                min_val = p;
            }
            if p > max_val {
                max_val = p;
            }
        }
    }
    if !min_val.is_finite() || !max_val.is_finite() {
        (0.0, 0.0)
    } else {
        (min_val, max_val)
    }
}

/// Compute calibration diagnostics after a global solve.
///
/// This function builds per-quote quality metrics using finite-difference
/// sensitivities (dResidual/dParam for the most sensitive parameter per quote),
/// computes the Jacobian's normal equations (J^T * J) for condition number
/// estimation, and calculates residual summary statistics.
fn compute_global_diagnostics<T>(
    target: &T,
    active_quotes: &[T::Quote],
    times: &[f64],
    solved_params: &[f64],
    resid_values: &[f64],
    weight_scales: &[f64],
    config: &CalibrationConfig,
) -> CalibrationDiagnostics
where
    T: GlobalSolveTarget,
{
    let n_residuals = resid_values.len();
    let n_params = solved_params.len();

    // 1. Compute per-quote quality with finite-difference sensitivities.
    let bump_h = super::helpers::diagnostics_bump_h(config);
    let mut per_quote = Vec::with_capacity(n_residuals);

    // Build Jacobian via finite differences (n_params bumps, each producing n_residuals).
    // jacobian[i][j] = dResidual_i / dParam_j
    let mut jacobian: Vec<Vec<f64>> = vec![vec![0.0; n_params]; n_residuals];
    let mut jacobian_ok = true;

    let mut bumped = solved_params.to_vec();
    let mut resid_up = vec![0.0; n_residuals];
    let mut resid_dn = vec![0.0; n_residuals];

    for j in 0..n_params {
        let h = bump_h * (1.0 + solved_params[j].abs());

        // Central differences: O(h^2) accuracy
        bumped[j] = solved_params[j] + h;
        let ok_up = target
            .build_curve_for_solver_from_params(times, &bumped)
            .and_then(|c| target.calculate_residuals(&c, active_quotes, &mut resid_up))
            .is_ok();

        bumped[j] = solved_params[j] - h;
        let ok_dn = target
            .build_curve_for_solver_from_params(times, &bumped)
            .and_then(|c| target.calculate_residuals(&c, active_quotes, &mut resid_dn))
            .is_ok();

        bumped[j] = solved_params[j];

        if ok_up && ok_dn {
            for i in 0..n_residuals {
                jacobian[i][j] = (resid_up[i] - resid_dn[i]) / (2.0 * h);
            }
        } else if ok_up {
            for i in 0..n_residuals {
                jacobian[i][j] = (resid_up[i] - resid_values[i]) / h;
            }
        } else {
            jacobian_ok = false;
        }
    }

    // Build per-quote quality metrics using the Jacobian.
    for (i, (&resid, quote)) in resid_values.iter().zip(active_quotes.iter()).enumerate() {
        // Max absolute sensitivity across all parameters for this quote.
        let sensitivity = if jacobian_ok {
            jacobian[i].iter().map(|v| v.abs()).fold(0.0_f64, f64::max)
        } else {
            0.0
        };

        per_quote.push(QuoteQuality {
            quote_label: target.residual_key(quote, i),
            target_value: 0.0, // Target is implicitly zero for residual-based calibration.
            fitted_value: resid, // The residual IS the deviation from zero.
            residual: resid,
            sensitivity,
        });
    }

    // 2. Compute condition number from J^T * J eigenvalues.
    let condition_number = if jacobian_ok && n_params > 0 {
        compute_condition_number(&jacobian, n_params, weight_scales)
    } else {
        None
    };

    // 3. Basic residual stats (shared with bootstrap so both paths report identically).
    let (max_residual, rms_residual) = super::helpers::residual_stats(resid_values);

    CalibrationDiagnostics {
        per_quote,
        condition_number,
        singular_values: None, // Full SVD is expensive; omit for now.
        max_residual,
        rms_residual,
        r_squared: None, // Meaningful only when target values have variance; omit for residual-based.
    }
}

/// Estimate the 2-norm condition number of the weighted normal equations matrix
/// `J^T W J` via power iteration for the largest eigenvalue and Cholesky-based
/// inverse iteration for the smallest.
///
/// Both eigenvalue solves are correct for symmetric positive (semi-)definite matrices;
/// the smallest-eigenvalue path uses `cholesky_solve` for exact back-substitution rather
/// than Gauss-Seidel sweeps. Returns `None` if `J^T W J` is singular (Cholesky factor not
/// PD) or has a non-finite largest eigenvalue — the caller treats `None` as "report
/// `cond=N/A`".
fn compute_condition_number(
    jacobian: &[Vec<f64>],
    n_params: usize,
    weight_scales: &[f64],
) -> Option<f64> {
    use finstack_quant_core::math::linalg::{cholesky_decomposition, cholesky_solve};

    if n_params == 0 {
        return None;
    }

    // Build J^T W J row-major as flat Vec<f64> (n×n), W = diag(weight_scales^2).
    let mut jtj = vec![0.0_f64; n_params * n_params];
    for (i, row) in jacobian.iter().enumerate() {
        let w2 = weight_scales.get(i).copied().unwrap_or(1.0).powi(2);
        for j in 0..n_params {
            let rj = row[j];
            for k in j..n_params {
                let val = w2 * rj * row[k];
                jtj[j * n_params + k] += val;
                if k != j {
                    jtj[k * n_params + j] += val;
                }
            }
        }
    }

    if n_params == 1 {
        return if jtj[0].abs() > 1e-30 {
            Some(1.0)
        } else {
            None
        };
    }

    // λ_max via power iteration on J^T W J.
    let mat_vec_flat = |m: &[f64], x: &[f64], out: &mut [f64]| {
        for (i, oi) in out.iter_mut().enumerate() {
            let row = &m[i * n_params..(i + 1) * n_params];
            *oi = row.iter().zip(x.iter()).map(|(a, b)| a * b).sum();
        }
    };

    let max_iter = 100;
    let tol = 1e-10;
    let mut v = vec![1.0 / (n_params as f64).sqrt(); n_params];
    let mut w = vec![0.0_f64; n_params];
    let mut lambda_max = 0.0_f64;
    for _ in 0..max_iter {
        mat_vec_flat(&jtj, &v, &mut w);
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if !norm.is_finite() || norm < 1e-30 {
            return None;
        }
        let new_lambda = w.iter().zip(v.iter()).map(|(a, b)| a * b).sum::<f64>();
        for (vi, wi) in v.iter_mut().zip(w.iter()) {
            *vi = wi / norm;
        }
        if (new_lambda - lambda_max).abs() < tol * lambda_max.abs().max(1.0) {
            lambda_max = new_lambda;
            break;
        }
        lambda_max = new_lambda;
    }
    if !lambda_max.is_finite() || lambda_max <= 0.0 {
        return None;
    }

    // λ_min via inverse power iteration: x_{k+1} = A^{-1} x_k, Rayleigh = x^T A x / x^T x.
    // A^{-1} application uses Cholesky factorisation of J^T W J. If the matrix is
    // singular, Cholesky fails and we report `cond = N/A` rather than a misleading
    // huge number.
    let Ok(chol) = cholesky_decomposition(&jtj, n_params) else {
        return None;
    };
    let mut v_min = vec![1.0 / (n_params as f64).sqrt(); n_params];
    let mut x_solve = vec![0.0_f64; n_params];
    let mut lambda_min = lambda_max;

    for _ in 0..max_iter {
        if cholesky_solve(&chol, &v_min, &mut x_solve).is_err() {
            return None;
        }
        let norm = x_solve.iter().map(|x| x * x).sum::<f64>().sqrt();
        if !norm.is_finite() || norm < 1e-30 {
            return None;
        }
        // Rayleigh quotient on the normalized x: x^T A x / x^T x = λ_min once x converges
        // to the smallest eigenvector. We compute it directly via mat-vec.
        for (vi, xi) in v_min.iter_mut().zip(x_solve.iter()) {
            *vi = xi / norm;
        }
        mat_vec_flat(&jtj, &v_min, &mut w);
        let rayleigh: f64 = w.iter().zip(v_min.iter()).map(|(a, b)| a * b).sum();
        if (rayleigh - lambda_min).abs() < tol * lambda_min.abs().max(1.0) {
            lambda_min = rayleigh;
            break;
        }
        lambda_min = rayleigh;
    }

    if !lambda_min.is_finite() || lambda_min.abs() < 1e-30 {
        return None;
    }

    Some((lambda_max / lambda_min).abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::CalibrationConfig;
    use finstack_quant_core::Error;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

    #[derive(Debug, Clone)]
    struct DummyCurve(#[allow(dead_code)] Vec<f64>);

    struct TestTarget {
        times: Vec<f64>,
        initials: Vec<f64>,
        residuals: Vec<f64>,
        weights: Option<Vec<f64>>,
        key_prefix: Option<String>,
    }

    impl TestTarget {
        fn new(times: Vec<f64>, initials: Vec<f64>, residuals: Vec<f64>) -> Self {
            Self {
                times,
                initials,
                residuals,
                weights: None,
                key_prefix: None,
            }
        }

        fn from_len(len: usize, residuals: Vec<f64>) -> Self {
            let times: Vec<f64> = (1..=len).map(|i| i as f64).collect();
            let initials = vec![0.0; len];
            Self::new(times, initials, residuals)
        }

        fn with_weights(mut self, weights: Vec<f64>) -> Self {
            self.weights = Some(weights);
            self
        }

        fn with_key_prefix(mut self, prefix: impl Into<String>) -> Self {
            self.key_prefix = Some(prefix.into());
            self
        }
    }

    impl GlobalSolveTarget for TestTarget {
        type Quote = usize;
        type Curve = DummyCurve;

        fn build_time_grid_and_guesses(
            &self,
            quotes: &[Self::Quote],
        ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
            Ok((self.times.clone(), self.initials.clone(), quotes.to_vec()))
        }

        fn build_curve_from_params(&self, _times: &[f64], params: &[f64]) -> Result<Self::Curve> {
            Ok(DummyCurve(params.to_vec()))
        }

        fn calculate_residuals(
            &self,
            _curve: &Self::Curve,
            _quotes: &[Self::Quote],
            residuals: &mut [f64],
        ) -> Result<()> {
            for (i, r) in residuals.iter_mut().enumerate() {
                *r = *self.residuals.get(i).unwrap_or(&0.0);
            }
            Ok(())
        }

        fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
            if let Some(prefix) = &self.key_prefix {
                format!("{}-{}", prefix, quote)
            } else {
                format!("GLOBAL-{:06}", quote)
            }
        }

        fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
            if let Some(custom) = &self.weights {
                assert_eq!(
                    custom.len(),
                    quotes.len(),
                    "weights must align with quotes for TestTarget"
                );
                for (out, value) in weights_out.iter_mut().zip(custom.iter()) {
                    *out = *value;
                }
                Ok(())
            } else {
                if quotes.len() != weights_out.len() {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "weights_out length mismatch ({} vs {})",
                            weights_out.len(),
                            quotes.len()
                        ),
                        category: "global_solve".to_string(),
                    });
                }
                for weight in weights_out.iter_mut() {
                    *weight = 1.0;
                }
                Ok(())
            }
        }
    }

    /// Item 1: the success gate must compare a *weighted* max residual against the
    /// √n-scaled tolerance — both sides on the same (weighted) scale.
    ///
    /// Construct residuals `[0.5, 0.0]` with small weights `w=[0.01,0.01]` (√w=0.1).
    /// Weighted residuals are `[0.05, 0.0]`: weighted L2 = 0.05, weighted max = 0.05.
    /// With `success_tolerance = 0.1`, `max_residual_tolerance = 0.1·√2 ≈ 0.1414`.
    ///   * Pre-fix gate: `weighted_l2(0.05) ≤ 0.1` AND `UNWEIGHTED max(0.5) ≤ 0.1414`
    ///     → false → spurious FAILURE despite the weighted fit being well in tolerance.
    ///   * Post-fix gate: `weighted_l2(0.05) ≤ 0.1` AND `weighted max(0.05) ≤ 0.1414`
    ///     → true → SUCCESS.
    #[test]
    fn success_gate_uses_weighted_max_residual_consistent_units() {
        let target = TestTarget::from_len(2, vec![0.5, 0.0]).with_weights(vec![0.01, 0.01]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1e-12);

        // The 4th arg is the *validation/success* tolerance (distinct from the LM
        // solver tolerance set via `with_tolerance`).
        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, Some(0.1))
            .expect("optimization should complete");

        assert!(
            report.success,
            "weighted L2 ({:.4}) and weighted max residual both within tolerance — \
             the gate must not fail on an UNWEIGHTED max residual (convergence_reason: {})",
            report.objective_value, report.convergence_reason,
        );
    }

    /// Item 1 (converse): a genuinely large *weighted* max residual must still fail the
    /// gate. Residuals `[0.3, 0.0]`, unit weights, `success_tolerance = 0.01`: weighted
    /// L2 = 0.3 and weighted max = 0.3 both far exceed any `tol·√n` per-quote bound.
    /// Guards the fix against degenerating into an L2-only check.
    #[test]
    fn success_gate_still_fails_on_large_weighted_max_residual() {
        let target = TestTarget::from_len(2, vec![0.3, 0.0]).with_weights(vec![1.0, 1.0]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1e-12);
        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, Some(0.01))
            .expect("optimization should complete");
        assert!(
            !report.success,
            "a weighted max residual far above tolerance must fail the gate"
        );
    }

    #[test]
    fn failure_reason_reports_l2_failure_when_weighted_max_passes() {
        let target = TestTarget::from_len(4, vec![0.06; 4]);
        let quotes = vec![0usize, 1usize, 2usize, 3usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);
        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, Some(0.1))
            .expect("optimization should complete");

        assert!(!report.success);
        assert!(
            report.convergence_reason.contains("weighted L2 norm")
                && report.convergence_reason.contains("exceeds")
                && report.convergence_reason.contains("weighted max residual")
                && report.convergence_reason.contains("passed"),
            "failure reason must identify the failing and passing criteria: {}",
            report.convergence_reason
        );
    }

    /// Item 2: `fill_penalty` must produce a residual vector that *depends on each
    /// parameter*, so the finite-difference Jacobian has a non-zero column for every
    /// parameter even when its ±h neighbourhood is also infeasible.
    ///
    /// Pre-fix: every residual was set to one scalar `v(p)`; bumping a single parameter
    /// while it stayed infeasible left `v` unchanged for that bump direction in the
    /// pattern's structure — the per-column FD difference collapsed to zero because the
    /// scalar mixed all parameters together and a one-parameter bump barely moved an
    /// aggregate `‖violation‖`. Post-fix each residual keys on one specific parameter, so
    /// a bump of parameter `j` strictly changes residual `j` (mod n_params).
    #[test]
    fn fill_penalty_yields_nonzero_jacobian_column_for_each_parameter() {
        let n_params = 3usize;
        let n_residuals = 6usize;
        // Lower bounds 0.0; all parameters start *below* the bound (infeasible) and stay
        // infeasible under a small bump — the exact regime that froze parameters before.
        let lb = vec![0.0_f64; n_params];
        let ub = vec![10.0_f64; n_params];

        let base_params = vec![-1.0_f64, -2.0, -0.5];
        let h = 1e-7_f64;

        for j in 0..n_params {
            let mut resid_base = vec![0.0_f64; n_residuals];
            fill_penalty(
                &mut resid_base,
                n_residuals,
                &base_params,
                Some(&lb),
                Some(&ub),
            );

            let mut bumped = base_params.clone();
            bumped[j] += h; // still < 0.0 -> still infeasible
            let mut resid_bumped = vec![0.0_f64; n_residuals];
            fill_penalty(
                &mut resid_bumped,
                n_residuals,
                &bumped,
                Some(&lb),
                Some(&ub),
            );

            // The FD column for parameter j: at least one residual must change.
            let column_norm: f64 = resid_base
                .iter()
                .zip(resid_bumped.iter())
                .map(|(a, b)| (b - a).abs())
                .fold(0.0_f64, f64::max);

            assert!(
                column_norm > 0.0,
                "fill_penalty produced a ZERO Jacobian column for parameter {j}: \
                 LM would have no descent direction and freeze that parameter. \
                 base={resid_base:?} bumped={resid_bumped:?}"
            );
        }
    }

    /// Item 2 (direction): the penalty must *decrease* as a violated parameter moves
    /// back toward its bound, so the LM gradient points into the feasible box.
    #[test]
    fn fill_penalty_decreases_toward_feasible_region() {
        let lb = vec![0.0_f64];
        let ub = vec![10.0_f64];
        let n_residuals = 1usize;

        let mut deep = vec![0.0_f64; n_residuals];
        fill_penalty(&mut deep, n_residuals, &[-5.0], Some(&lb), Some(&ub));

        let mut shallow = vec![0.0_f64; n_residuals];
        fill_penalty(&mut shallow, n_residuals, &[-0.5], Some(&lb), Some(&ub));

        let mut feasible = vec![0.0_f64; n_residuals];
        fill_penalty(&mut feasible, n_residuals, &[5.0], Some(&lb), Some(&ub));

        assert!(
            deep[0] > shallow[0],
            "penalty must shrink as the parameter approaches the bound: deep={} shallow={}",
            deep[0],
            shallow[0],
        );
        assert!(
            shallow[0] > feasible[0],
            "penalty must be smallest when the parameter is interior: shallow={} feasible={}",
            shallow[0],
            feasible[0],
        );
    }

    #[test]
    fn max_abs_residual_metadata_uses_absolute_value() {
        let target = TestTarget::from_len(2, vec![-0.02, -0.01]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, None)
            .expect("optimization succeeds");

        let meta = report
            .metadata
            .get("final_unweighted_max_abs_residual")
            .expect("metadata should contain final_unweighted_max_abs_residual");
        let parsed: f64 = meta.parse().expect("metadata should parse as f64");

        assert!(
            (parsed - 2.0e-2).abs() < 1e-12,
            "final_unweighted_max_abs_residual metadata should use absolute values (got {})",
            parsed
        );
    }

    #[test]
    fn rejects_length_mismatch_between_times_and_initials() {
        let target = TestTarget::new(vec![1.0, 2.0], vec![0.0], vec![0.0, 0.0]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let err =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect_err("should fail");
        match err {
            Error::Calibration { message, .. } => {
                assert!(
                    message.contains("times.len() == initials.len()"),
                    "unexpected message: {}",
                    message
                );
            }
            other => panic!("unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn rejects_non_increasing_times() {
        let target = TestTarget::new(vec![1.0, 0.5], vec![0.0, 0.0], vec![0.0, 0.0]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let err =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect_err("should fail");
        match err {
            Error::Calibration { message, .. } => {
                assert!(
                    message.contains("strictly increasing"),
                    "unexpected message: {}",
                    message
                );
            }
            other => panic!("unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn rejects_non_finite_inputs() {
        let target = TestTarget::new(vec![f64::NAN], vec![f64::INFINITY], vec![0.0]);
        let quotes = vec![0usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let err =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect_err("should fail");
        match err {
            Error::Calibration { message, .. } => {
                assert!(
                    message.contains("non-negative finite times")
                        || message.contains("finite initial guesses"),
                    "unexpected message: {}",
                    message
                );
            }
            other => panic!("unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn uses_custom_residual_keys_and_weights() {
        let target = TestTarget::from_len(2, vec![0.01, 0.02])
            .with_weights(vec![4.0, 1.0])
            .with_key_prefix("TEST");
        let quotes = vec![5usize, 7usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, None)
            .expect("optimization succeeds");

        assert!(report.residuals.contains_key("TEST-5"));
        assert!(report.residuals.contains_key("TEST-7"));

        let weighted_l2 = report
            .metadata
            .get("final_weighted_resid_l2_norm")
            .expect("metadata should include final_weighted_resid_l2_norm");
        let expected = ((0.01_f64 * 2.0).powi(2) + (0.02_f64 * 1.0).powi(2)).sqrt();
        assert_eq!(
            weighted_l2,
            &format!("{:.2e}", expected),
            "weighted_l2_norm should reflect weights"
        );
    }

    #[test]
    fn supports_overdetermined_least_squares() {
        let target = TestTarget::new(vec![1.0, 2.0], vec![0.0, 0.0], vec![0.01, -0.02, 0.03]);
        let quotes = vec![10usize, 11usize, 12usize];
        let config = CalibrationConfig::default().with_tolerance(1e12);

        let (_curve, report) =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("should succeed");

        assert_eq!(report.residuals.len(), 3);
        assert!(report.residuals.contains_key("GLOBAL-000010"));
        assert!(report.residuals.contains_key("GLOBAL-000011"));
        assert!(report.residuals.contains_key("GLOBAL-000012"));
    }

    #[test]
    fn intermediate_eval_errors_are_reported_but_not_fatal_if_final_evaluation_succeeds() {
        struct FlakyResidualTarget {
            inner: TestTarget,
            fail_once: AtomicBool,
        }

        impl GlobalSolveTarget for FlakyResidualTarget {
            type Quote = usize;
            type Curve = DummyCurve;

            fn build_time_grid_and_guesses(
                &self,
                quotes: &[Self::Quote],
            ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
                self.inner.build_time_grid_and_guesses(quotes)
            }

            fn build_curve_from_params(
                &self,
                times: &[f64],
                params: &[f64],
            ) -> Result<Self::Curve> {
                self.inner.build_curve_from_params(times, params)
            }

            fn calculate_residuals(
                &self,
                curve: &Self::Curve,
                quotes: &[Self::Quote],
                residuals: &mut [f64],
            ) -> Result<()> {
                if self
                    .fail_once
                    .compare_exchange(false, true, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
                    .is_ok()
                {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: "intentional transient residual failure".to_string(),
                        category: "test".to_string(),
                    });
                }
                self.inner.calculate_residuals(curve, quotes, residuals)
            }
        }

        let target = FlakyResidualTarget {
            inner: TestTarget::from_len(2, vec![0.01, 0.02]),
            fail_once: AtomicBool::new(false),
        };
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1e12);

        let (_curve, report) =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("should succeed");

        let invalid_count: usize = report
            .metadata
            .get("invalid_eval_count")
            .expect("should include invalid_eval_count")
            .parse()
            .expect("invalid_eval_count should parse");
        assert!(invalid_count >= 1);
        assert!(
            report.metadata.contains_key("first_invalid_eval"),
            "should include first_invalid_eval"
        );
    }

    #[test]
    fn analytical_jacobian_path_handles_more_quotes_than_probe_buffer() {
        struct EfficientTarget {
            times: Vec<f64>,
            targets: Vec<f64>,
        }

        impl GlobalSolveTarget for EfficientTarget {
            type Quote = usize;
            type Curve = DummyCurve;

            fn build_time_grid_and_guesses(
                &self,
                quotes: &[Self::Quote],
            ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
                Ok((self.times.clone(), vec![0.0, 0.0], quotes.to_vec()))
            }

            fn build_curve_from_params(
                &self,
                _times: &[f64],
                params: &[f64],
            ) -> Result<Self::Curve> {
                Ok(DummyCurve(params.to_vec()))
            }

            fn calculate_residuals(
                &self,
                curve: &Self::Curve,
                quotes: &[Self::Quote],
                residuals: &mut [f64],
            ) -> Result<()> {
                let level = curve.0[0] + curve.0[1];
                for (i, quote) in quotes.iter().enumerate() {
                    residuals[i] = level - self.targets[*quote];
                }
                Ok(())
            }

            fn jacobian(
                &self,
                _params: &[f64],
                _times: &[f64],
                quotes: &[Self::Quote],
                jacobian: &mut [Vec<f64>],
            ) -> Result<()> {
                if jacobian.len() != quotes.len() {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "jacobian row count mismatch: {} vs {}",
                            jacobian.len(),
                            quotes.len()
                        ),
                        category: "efficient_jacobian".to_string(),
                    });
                }
                for row in jacobian.iter_mut() {
                    row[0] = 1.0;
                    row[1] = 1.0;
                }
                Ok(())
            }

            fn supports_efficient_jacobian(&self) -> bool {
                true
            }
        }

        let target = EfficientTarget {
            times: vec![1.0, 2.0],
            targets: vec![0.5; 10],
        };
        let quotes: Vec<usize> = (0..10).collect();
        let config = CalibrationConfig::default()
            .with_calibration_method(crate::calibration::CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: true,
            })
            .with_tolerance(1e-12)
            .with_max_iterations(50);

        let (_curve, report) =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("should succeed");

        assert_eq!(report.residuals.len(), quotes.len());
    }

    #[test]
    fn rejects_empty_active_quotes() {
        let target = TestTarget::from_len(1, vec![0.0]);
        let quotes: Vec<usize> = Vec::new();
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let err = GlobalFitOptimizer::optimize(&target, &quotes, &config, None)
            .expect_err("empty active quotes should fail");
        assert!(matches!(
            err,
            Error::Input(finstack_quant_core::InputError::TooFewPoints)
        ));
    }

    #[test]
    fn rejects_underdetermined_least_squares_system() {
        let target = TestTarget::new(vec![1.0, 2.0, 3.0], vec![0.0, 0.0, 0.0], vec![0.01, 0.02]);
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let err = GlobalFitOptimizer::optimize(&target, &quotes, &config, None)
            .expect_err("n_residuals < n_params should fail");
        match err {
            Error::Calibration { message, .. } => {
                assert!(
                    message.contains("n_residuals >= n_params"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("unexpected error type: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_and_zero_residual_weights() {
        let quotes = vec![0usize, 1usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        for weights in [vec![-1.0, 1.0], vec![f64::NAN, 1.0]] {
            let target = TestTarget::from_len(2, vec![0.01, 0.02]).with_weights(weights);
            let err = GlobalFitOptimizer::optimize(&target, &quotes, &config, None)
                .expect_err("invalid weights should fail");
            match err {
                Error::Calibration { message, .. } => {
                    assert!(
                        message.contains("non-negative finite residual weights"),
                        "unexpected message: {message}"
                    );
                }
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        let zero_target = TestTarget::from_len(2, vec![0.01, 0.02]).with_weights(vec![0.0, 0.0]);
        let zero_err = GlobalFitOptimizer::optimize(&zero_target, &quotes, &config, None)
            .expect_err("all-zero weights should fail");
        match zero_err {
            Error::Calibration { message, .. } => {
                assert!(
                    message.contains("at least one positive residual weight"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("unexpected error type: {other:?}"),
        }
    }

    #[test]
    fn success_tolerance_override_controls_report_success() {
        let target = TestTarget::from_len(1, vec![0.15]);
        let quotes = vec![0usize];
        let config = CalibrationConfig::default().with_tolerance(1.0);

        let (_curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, Some(0.1))
            .expect("optimization should still complete");

        assert!(
            !report.success,
            "explicit success tolerance should be enforced"
        );
        assert_eq!(
            report.metadata.get("success_tolerance"),
            Some(&format!("{:.2e}", 0.1))
        );
        assert!(
            (report.objective_value - 0.15).abs() < 1e-12,
            "objective_value should be weighted L2 norm"
        );
    }

    #[test]
    fn compute_diagnostics_populates_report_when_enabled() {
        let target = TestTarget::from_len(1, vec![0.01]);
        let quotes = vec![0usize];
        let config = CalibrationConfig::default()
            .with_tolerance(1.0)
            .with_compute_diagnostics(true);

        let (_curve, report) =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("should succeed");

        let diagnostics = report
            .diagnostics
            .as_ref()
            .expect("diagnostics should be populated");
        assert_eq!(diagnostics.per_quote.len(), 1);
        assert!((diagnostics.max_residual - 0.01).abs() < 1e-12);
        assert!(diagnostics.condition_number.is_none());
    }

    #[test]
    fn perturb_initial_guess_is_deterministic_and_respects_bounds() {
        let initials = vec![1.0, 2.0];
        let lb = vec![0.9, 1.7];
        let ub = vec![1.1, 2.5];

        // restart_idx 2 → halton(3, 2) = 0.75 on dim 0, halton(3, 3) = 1/9
        // on dim 1. Additive perturbation (scale 0.5):
        //   dim 0: 1.0 + 0.5·(2·0.75 − 1) = 1.0 + 0.25 = 1.25 → clamps to 1.1
        //   dim 1: 2.0 + 0.5·(2·(1/9) − 1) ≈ 2.0 − 0.389 = 1.611 → clamps to 1.7
        let first =
            perturb_initial_guess(&initials, 0.5, 2, Some(lb.as_slice()), Some(ub.as_slice()));
        let second =
            perturb_initial_guess(&initials, 0.5, 2, Some(lb.as_slice()), Some(ub.as_slice()));

        assert_eq!(
            first, second,
            "Halton-based perturbations should be deterministic"
        );
        assert!(
            (first[0] - 1.1).abs() < 1e-12,
            "first coordinate should clamp to upper bound, got {}",
            first[0]
        );
        assert!(
            (first[1] - 1.7).abs() < 1e-12,
            "second coordinate should clamp to lower bound, got {}",
            first[1]
        );
    }
}
