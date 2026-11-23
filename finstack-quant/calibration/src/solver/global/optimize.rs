//! Multi-start Levenberg-Marquardt global optimization kernel.

use super::super::multi_start::{MultiStartConfig, perturb_initial_guess};
use super::super::traits::GlobalSolveTarget;
use super::diagnostics::*;
use crate::constants::PENALTY;
use crate::solver::traits::TimeGridAndGuesses;
use crate::{CalibrationConfig, CalibrationReport};
use finstack_quant_core::Result;
use finstack_quant_core::explain::{ExplanationTrace, TraceEntry};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, btree_map::Entry};

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
pub(super) fn fill_penalty(
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
        if let Some(lo) = lb
            && let Some(&b) = lo.get(idx)
            && p < b
        {
            dist += b - p;
        }
        if let Some(hi) = ub
            && let Some(&b) = hi.get(idx)
            && p > b
        {
            dist += p - b;
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
/// Execute a simultaneous weighted least-squares fit.
///
/// # Generic Parameters
/// * `T` - The calibration target (e.g., [`DiscountCurveTarget`](crate::targets::discount::DiscountCurveTarget)).
///
/// # Arguments
/// * `target` - The domain-specific implementation of the [`GlobalSolveTarget`] trait.
/// * `quotes` - The list of high-level market quotes to fit.
/// * `config` - Numerical calibration and acceptance settings.
/// * `use_analytical_jacobian` - Use the target derivative when it supports one.
/// * `success_tolerance` - Accept/reject threshold for the unweighted native residual
///   (`max_i |r_i|`). Distinct from the LM solver convergence tolerance in `config`.
///   Callers pass the target-specific validation tolerance (for example
///   `config.forward_curve.validation_tolerance`). There is no implicit
///   discount-curve fallback.
///
/// # Returns
/// A pair containing the calibrated term structure and a diagnostic report.
///
/// # Tolerance Semantics
/// Success is `max_i |r_i| <= success_tolerance` on the unweighted native residual.
/// Residual weights affect only the LM objective (`objective_value` / weighted L2),
/// not the accept/reject gate. The report stores signed residuals.
pub(crate) fn optimize<T>(
    target: &T,
    quotes: &[T::Quote],
    config: &CalibrationConfig,
    success_tolerance: f64,
    use_analytical_jacobian: bool,
) -> Result<(T::Curve, CalibrationReport)>
where
    T: GlobalSolveTarget,
{
    optimize_impl(
        target,
        quotes,
        config,
        success_tolerance,
        None,
        no_restarts::<T>,
        use_analytical_jacobian,
    )
}

/// Execute a simultaneous weighted least-squares fit with optional multi-start.
///
/// When `multi_start` is `Some`, the optimizer runs `num_restarts` additional solves
/// from deterministically-perturbed starting points. On native targets the restarts
/// run in parallel; results are folded in restart order, so the winner is identical
/// to a serial run. The best result (lowest weighted residual norm) is returned.
pub(crate) fn optimize_with_multi_start<T>(
    target: &T,
    quotes: &[T::Quote],
    config: &CalibrationConfig,
    success_tolerance: f64,
    multi_start: Option<&MultiStartConfig>,
    use_analytical_jacobian: bool,
) -> Result<(T::Curve, CalibrationReport)>
where
    T: GlobalSolveTarget + Sync,
    T::Quote: Sync,
{
    optimize_impl(
        target,
        quotes,
        config,
        success_tolerance,
        multi_start,
        parallel_restarts::<T>,
        use_analytical_jacobian,
    )
}

pub(super) fn optimize_impl<T>(
    target: &T,
    quotes: &[T::Quote],
    config: &CalibrationConfig,
    success_tolerance: f64,
    multi_start: Option<&MultiStartConfig>,
    run_restarts: impl Fn(
        &SolveInputs<'_, T>,
        &[f64],
        &MultiStartConfig,
    ) -> Vec<Result<SingleSolveResult>>,
    use_analytical_jacobian: bool,
) -> Result<(T::Curve, CalibrationReport)>
where
    T: GlobalSolveTarget,
{
    // 1. Build grid and guesses
    let TimeGridAndGuesses {
        times,
        initials,
        quotes: active_quotes,
    } = target.build_time_grid_and_guesses(quotes)?;
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
    validate_parameter_bounds(&initials, &lb, &ub)?;

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
    let inputs = SolveInputs {
        target,
        active_quotes: &active_quotes,
        times: &times,
        weight_scales: &weight_scales,
        lb: &lb,
        ub: &ub,
        config,
        use_analytical_jacobian,
    };
    let mut best = match run_single_solve(&inputs, &initials) {
        Ok(v) => Some(v),
        Err(err) => {
            tracing::warn!(
                error = %err,
                "global: primary LM solve failed; will attempt multi-start if configured"
            );
            primary_error = Some(err);
            None
        }
    };

    // Multi-start: run additional solves from perturbed starting points.
    if let Some(ms) = multi_start {
        tracing::debug!(
            "global: running {} multi-start restarts (perturbation_scale={:.3})",
            ms.num_restarts,
            ms.perturbation_scale,
        );

        // Restarts are independent solves; fold them in index order so the
        // first strictly-better restart wins exactly as it would serially.
        let restart_results = run_restarts(&inputs, &initials, ms);
        for (restart_idx, outcome) in restart_results.into_iter().enumerate() {
            match outcome {
                Ok(result) => {
                    let improved = best
                        .as_ref()
                        .is_none_or(|prev| result.weighted_l2 < prev.weighted_l2);
                    if improved {
                        tracing::debug!(
                            "global: restart {} improved weighted L2: {:?} -> {:.4e}",
                            restart_idx,
                            best.as_ref().map(|previous| previous.weighted_l2),
                            result.weighted_l2,
                        );
                        best = Some(result);
                        winning_start = Some(restart_idx);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        restart = restart_idx,
                        error = %err,
                        "global: multi-start restart failed, skipping",
                    );
                }
            }
        }
    }

    // If the primary solve failed and no restart rescued it, propagate the original
    // error rather than feeding a sentinel into the final-curve build.
    let Some(best_result) = best else {
        return Err(
            primary_error.unwrap_or_else(|| finstack_quant_core::Error::Calibration {
                message: "global: every solve attempt failed".to_string(),
                category: "global_solve".to_string(),
            }),
        );
    };
    let SingleSolveResult {
        solved_params,
        stats,
        residual_evals: eval_counter_val,
        diagnostics: eval_diagnostics_val,
        parameters_clamped: final_n_clamped,
        ..
    } = best_result;

    let final_curve = target.build_curve_final_from_params(&times, &solved_params)?;

    let mut residuals_map = BTreeMap::new();
    let mut resid_values = vec![0.0; n_residuals];
    target.calculate_residuals(&final_curve, &active_quotes, &mut resid_values)?;

    for (i, (&val, quote)) in resid_values.iter().zip(active_quotes.iter()).enumerate() {
        let key = target.residual_key(quote, i);
        match residuals_map.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(val);
            }
            Entry::Occupied(entry) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Global calibration produced duplicate residual key '{}' for observation {i}",
                    entry.key()
                )));
            }
        }
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
    let validation_tolerance = success_tolerance;

    // Success is unweighted `max_i |r_i| <= validation_tolerance`. Weights belong
    // in the LM objective only; do not scale the gate by √n or compare weighted L2
    // to this tolerance. `for_type_with_tolerance` already applies that rule.
    let mut report = CalibrationReport::for_type_with_tolerance(
        "global_fit",
        residuals_map,
        stats.iterations,
        validation_tolerance,
    );
    report.objective_value = weighted_l2_norm;
    // Always surface the LM termination reason so callers can distinguish
    // "gradient tolerance met" (clean convergence) from "max iterations
    // reached but residuals within tolerance" (acceptable but suspect).
    report.convergence_reason.push_str(&format!(
        "; LM terminated with {:?}",
        stats.termination_reason
    ));

    if !report.success {
        // Surface the worst-fit quotes without requiring diagnostics.
        let worst = top_k_worst_fits(target, &active_quotes, &resid_values, 3);
        if !worst.is_empty() {
            report.convergence_reason.push_str(". Worst fits: ");
            report.convergence_reason.push_str(&worst);
        }
    }

    // A clamped final point is not a stationary solution.
    if final_n_clamped > 0 {
        report = report
            .with_metadata("final_params_clamped", final_n_clamped.to_string())
            .with_has_penalty_residuals(true);
    }

    report = report
        .with_solver_method(crate::report::SolverMethod::GlobalFitLmWeightedLsq)
        .with_residual_units(target.residual_units())
        .with_metadata("tolerance_definition", "max_abs_residual")
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

    if let Some(step_norm) = stats.last_step_norm {
        report = report.with_metadata("lm_final_step_norm", format!("{step_norm:.2e}"));
    }
    if let Some(lambda) = stats.last_lambda {
        report = report.with_metadata("lm_lambda_final", format!("{lambda:.2e}"));
    }

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
    if config.explain.enabled {
        let mut trace = ExplanationTrace::new("global_fit");
        trace.push(
            TraceEntry::ComputationStep {
                name: "global_solve".to_string(),
                description: format!("{:?}", stats.termination_reason),
                metadata: Some(serde_json::json!({
                    "times": times,
                    "solved_params": solved_params,
                    "iterations": stats.iterations,
                    "residual_evals": stats.residual_evals,
                    "jacobian_evals": stats.jacobian_evals,
                    "weighted_residual_l2_norm": weighted_l2_norm,
                    "max_abs_residual": max_abs_residual,
                })),
            },
            config.explain.max_entries,
        );
        report = report.with_explanation(trace);
    }

    Ok((final_curve, report))
}

// The Halton multi-start helpers (`halton`, `perturb_initial_guess`, and
// `MultiStartConfig`) live in `super::multi_start` so sibling
// calibration targets can reuse the same deterministic perturbation
// strategy. See that module for documentation, references, and unit
// tests.

/// One solve's parameters, diagnostics and restart-ranking objective.
pub(super) struct SingleSolveResult {
    solved_params: Vec<f64>,
    stats: finstack_quant_core::math::solver_multi::LmStats,
    residual_evals: usize,
    diagnostics: EvalDiagnostics,
    parameters_clamped: usize,
    weighted_l2: f64,
}

/// Inputs shared by the primary solve and every multi-start restart of one fit.
#[derive(Clone, Copy)]
pub(super) struct SolveInputs<'a, T: GlobalSolveTarget> {
    target: &'a T,
    active_quotes: &'a [T::Quote],
    times: &'a [f64],
    weight_scales: &'a [f64],
    lb: &'a Option<Vec<f64>>,
    ub: &'a Option<Vec<f64>>,
    config: &'a CalibrationConfig,
    use_analytical_jacobian: bool,
}

/// Restart runner for [`optimize`], which never restarts.
pub(super) fn no_restarts<T: GlobalSolveTarget>(
    _inputs: &SolveInputs<'_, T>,
    _initials: &[f64],
    _multi_start: &MultiStartConfig,
) -> Vec<Result<SingleSolveResult>> {
    Vec::new()
}

/// Run every restart from its Halton-perturbed start. Restarts are independent
/// LM solves, so they run on the rayon pool on native targets; the output order
/// is the restart index regardless of scheduling.
pub(super) fn parallel_restarts<T>(
    inputs: &SolveInputs<'_, T>,
    initials: &[f64],
    multi_start: &MultiStartConfig,
) -> Vec<Result<SingleSolveResult>>
where
    T: GlobalSolveTarget + Sync,
    T::Quote: Sync,
{
    let solve_from = |restart_idx: usize| {
        let perturbed = perturb_initial_guess(
            initials,
            multi_start.perturbation_scale,
            restart_idx,
            inputs.lb.as_deref(),
            inputs.ub.as_deref(),
        );
        run_single_solve(inputs, &perturbed)
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        (0..multi_start.num_restarts)
            .into_par_iter()
            .map(solve_from)
            .collect()
    }

    #[cfg(target_arch = "wasm32")]
    {
        (0..multi_start.num_restarts).map(solve_from).collect()
    }
}

/// Run one LM solve and return parameters, statistics, evaluation diagnostics,
/// clamp count, and the final weighted L2 residual norm.
pub(super) fn run_single_solve<T>(
    inputs: &SolveInputs<'_, T>,
    initials: &[f64],
) -> Result<SingleSolveResult>
where
    T: GlobalSolveTarget,
{
    let SolveInputs {
        target,
        active_quotes,
        times,
        weight_scales,
        lb,
        ub,
        config,
        use_analytical_jacobian,
    } = *inputs;
    let n_residuals = active_quotes.len();

    let use_efficient = use_analytical_jacobian && target.supports_efficient_jacobian();

    let solver = config.create_lm_solver()?;

    let eval_diagnostics: RefCell<EvalDiagnostics> = RefCell::new(EvalDiagnostics::default());
    let eval_counter: Cell<usize> = Cell::new(0);

    // Reuse a local buffer across LM residual evaluations when bounds
    // clamping is active. This `RefCell` is local to one solve: multi-start
    // restarts run in parallel, but each restart owns its own buffer and the
    // LM solver calls the residual closure from a single thread. Do not hoist
    // it out of this function without switching to `Mutex<Vec<f64>>`.
    let clamp_buffer: RefCell<Vec<f64>> = RefCell::new(Vec::with_capacity(initials.len()));

    let residuals_func = |params: &[f64], resid: &mut [f64]| -> finstack_quant_core::Result<()> {
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
            return Ok(());
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

        let curve = match target.build_curve_from_params(times, params_ref) {
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
                return Ok(());
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
            return Ok(());
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
            return Ok(());
        }

        for (r, w) in resid[..n_residuals].iter_mut().zip(weight_scales.iter()) {
            *r *= *w;
        }
        Ok(())
    };

    let solution = if use_efficient {
        let n_params = initials.len();
        let mut jac_clamp_buffer: Vec<f64> = Vec::with_capacity(n_params);
        let mut jac_rows: Vec<Vec<f64>> = vec![vec![0.0; n_params]; n_residuals];
        let jacobian_func = |params: &[f64], jac: &mut [f64]| -> finstack_quant_core::Result<()> {
            let params_ref = if lb.is_some() || ub.is_some() {
                clamp_to_bounds(params, lb, ub, &mut jac_clamp_buffer);
                &jac_clamp_buffer[..]
            } else {
                params
            };

            target.build_curve_from_params(times, params_ref)?;
            for row in &mut jac_rows {
                row.fill(0.0);
            }
            target.jacobian(params_ref, times, active_quotes, &mut jac_rows)?;

            for (i, row) in jac_rows.iter().enumerate() {
                let scale = weight_scales.get(i).copied().unwrap_or(1.0);
                let dst = &mut jac[i * n_params..(i + 1) * n_params];
                for (slot, &val) in dst.iter_mut().zip(row.iter()) {
                    *slot = val * scale;
                }
            }
            Ok(())
        };
        solver.solve_least_squares_with_jacobian(
            residuals_func,
            jacobian_func,
            initials,
            n_residuals,
        )?
    } else {
        solver.solve_least_squares(residuals_func, initials, n_residuals)?
    };
    // The LM core (`solve_system` / `solve_system_with_jacobian`)
    // runs without native box constraints: the residual closure clamps a *local*
    // pricing copy but LM keeps the unclamped iterate. As a result the converged
    // `solution.params` can land outside the target's `lower_bounds()`/`upper_bounds()`.
    // Clamp the returned parameters to the bounds here, BEFORE the residuals are
    // re-evaluated below, so the reported curve and residuals correspond to the
    // same (in-bounds) parameter set rather than a stale unclamped one.
    let (solved_params, final_n_clamped) = if lb.is_some() || ub.is_some() {
        let mut clamped = Vec::with_capacity(solution.params.len());
        let n_clamped = clamp_to_bounds(&solution.params, lb, ub, &mut clamped);
        (clamped, n_clamped)
    } else {
        (solution.params, 0)
    };
    let stats = solution.stats;

    // Compute weighted L2 norm of the final residuals for comparison.
    let final_curve = target.build_curve_from_params(times, &solved_params)?;
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

    Ok(SingleSolveResult {
        solved_params,
        stats,
        residual_evals: eval_count,
        diagnostics,
        parameters_clamped: final_n_clamped,
        weighted_l2,
    })
}
