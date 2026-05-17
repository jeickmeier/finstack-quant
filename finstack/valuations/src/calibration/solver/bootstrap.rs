//! Generic sequential bootstrapping algorithm.

use super::bracket_solve_1d_with_diagnostics;
use super::helpers::BracketDiagnostics;
use super::traits::BootstrapTarget;
use crate::calibration::constants::{OBJECTIVE_VALID_ABS_MAX, RESIDUAL_PENALTY_ABS_MIN};
use crate::calibration::report::{CalibrationDiagnostics, QuoteQuality};
use crate::calibration::{CalibrationConfig, CalibrationReport};
use finstack_core::Result;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// Helper Types
// =============================================================================

/// A quote reference with its computed time, used for sorting.
struct SortedQuote {
    time: f64,
    original_idx: usize,
}

/// Context for resolving what to do when no bracket is found.
struct NoBracketContext<'a, Q> {
    time: f64,
    quote: &'a Q,
    diag: &'a BracketDiagnostics,
    validation_tolerance: f64,
    first_eval_error: Option<String>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Sort quotes by time, validating each quote produces a valid time.
fn sort_quotes_by_time<T: BootstrapTarget>(
    target: &T,
    quotes: &[T::Quote],
) -> Result<Vec<SortedQuote>> {
    let mut quote_times = Vec::with_capacity(quotes.len());
    for (idx, quote) in quotes.iter().enumerate() {
        let time = target
            .quote_time(quote)
            .map_err(|e| finstack_core::Error::Calibration {
                message: format!(
                    "Bootstrap failed to compute quote_time for quote index {idx}: {e}"
                ),
                category: "bootstrapping".to_string(),
            })?;
        validate_quote_time(time, idx)?;
        quote_times.push(SortedQuote {
            time,
            original_idx: idx,
        });
    }
    quote_times.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.original_idx.cmp(&b.original_idx))
    });
    Ok(quote_times)
}

/// Validate that a quote time is finite and positive.
fn validate_quote_time(time: f64, idx: usize) -> Result<()> {
    if !time.is_finite() || time <= 0.0 {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap quote_time must be finite and > 0; got t={time} (quote index {idx})"
            ),
            category: "bootstrapping".to_string(),
        });
    }
    Ok(())
}

/// Validate that quote times are strictly increasing.
fn validate_time_ordering(time: f64, last_time: f64, original_idx: usize) -> Result<()> {
    if time < last_time {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap requires increasing quote times; got t={time:.12} after last_time={last_time:.12} (quote index {original_idx})"
            ),
            category: "bootstrapping".to_string(),
        });
    }
    if (time - last_time).abs() <= crate::calibration::constants::TOLERANCE_DUP_KNOTS {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap rejects duplicate quote times: t={time:.12} appears more than once (quote index {original_idx})"
            ),
            category: "bootstrapping".to_string(),
        });
    }
    Ok(())
}

/// Cap on the geometric scan-grid half-width as a multiple of `max(|center|, 1.0)`.
/// Wider scans waste evaluations and risk NaN in pricers; tighter scans miss distressed
/// regimes (e.g. hazard rates up to ~10000% from a sub-percent guess).
const SCAN_GRID_HALF_WIDTH_CAP: f64 = 100.0;

/// Build the default geometric scan grid around an initial guess.
fn build_default_scan_grid(initial_guess: f64, config: &CalibrationConfig) -> Vec<f64> {
    let center = if initial_guess.is_finite() {
        initial_guess
    } else {
        0.0
    };

    let step0 = (config.discount_curve.scan_grid_step * (1.0 + center.abs())).max(1e-8);
    let grid_size = config.discount_curve.scan_grid_points;
    let max_step = SCAN_GRID_HALF_WIDTH_CAP * (1.0_f64).max(center.abs());
    let mut pts = Vec::with_capacity(2 * grid_size + 1);
    pts.push(center);
    let mut step = step0;
    for _ in 0..grid_size {
        let s = step.min(max_step);
        pts.push(center - s);
        pts.push(center + s);
        step *= 2.0;
    }
    pts
}

/// Normalize and deduplicate scan points.
fn normalize_scan_points(mut points: Vec<f64>, initial_guess: f64, time: f64) -> Result<Vec<f64>> {
    points.retain(|x| x.is_finite());
    if initial_guess.is_finite() {
        points.push(initial_guess);
    }
    points.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    points.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

    if points.is_empty() {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap failed at t={time:.6}: scan_points is empty after filtering non-finite values."
            ),
            category: "bootstrapping".to_string(),
        });
    }
    Ok(points)
}

/// Resolve the solved value when no sign-change bracket was found.
///
/// Returns `(solved_x, is_approximate)` on success, where `is_approximate = true`
/// indicates the knot was accepted from a local `|f|`-minimum rather than a true
/// sign-change bracket.  Callers MUST propagate this flag so the calibration report
/// can distinguish approximate knots from properly-bracketed roots (W-40).
fn resolve_no_bracket<Q: std::fmt::Debug>(ctx: NoBracketContext<'_, Q>) -> Result<(f64, bool)> {
    // Check if best point is within tolerance
    if let (Some(best_x), Some(best_f)) = (ctx.diag.best_point, ctx.diag.best_value) {
        if best_f.is_finite() && best_f.abs() <= ctx.validation_tolerance {
            // Accepted without a sign-change bracket: flag as approximate.
            return Ok((best_x, true));
        }
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap failed at t={:.6} (quote={:?}): no bracket found and best |residual|={:.3e} exceeds tolerance={:.3e} (scan_bounds=[{:.3e}, {:.3e}])",
                ctx.time,
                ctx.quote,
                best_f.abs(),
                ctx.validation_tolerance,
                ctx.diag.scan_bounds.0,
                ctx.diag.scan_bounds.1
            ),
            category: "bootstrapping".to_string(),
        });
    }

    // All evaluations invalid
    if ctx.diag.valid_eval_count == 0 {
        let hint = ctx.first_eval_error.unwrap_or_else(|| {
            "no error recorded (all evaluations penalized or non-finite)".to_string()
        });
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap failed at t={:.6}: all {} objective evaluations returned invalid/penalized values (|f| >= {:.3e}). First error: {}",
                ctx.time, ctx.diag.eval_count, OBJECTIVE_VALID_ABS_MAX, hint
            ),
            category: "bootstrapping".to_string(),
        });
    }

    // Valid evaluations but no bracket
    Err(finstack_core::Error::Calibration {
        message: format!(
            "Bootstrap failed at t={:.6}: no bracket found despite {} valid evaluations (scan_bounds=[{:.3e}, {:.3e}]).",
            ctx.time,
            ctx.diag.valid_eval_count,
            ctx.diag.scan_bounds.0,
            ctx.diag.scan_bounds.1
        ),
        category: "bootstrapping".to_string(),
    })
}

/// Validate the final residual after solving.
fn validate_residual(time: f64, residual: f64, tolerance: f64) -> Result<()> {
    let abs = residual.abs();
    if !residual.is_finite() || abs >= RESIDUAL_PENALTY_ABS_MIN {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap converged to invalid/penalty residual at t={time:.6}: residual={residual} (|.|={abs:.3e})"
            ),
            category: "bootstrapping".to_string(),
        });
    }
    if abs > tolerance {
        return Err(finstack_core::Error::Calibration {
            message: format!(
                "Bootstrap failed to converge at t={time:.6}: residual={residual} (|.|={abs:.3e}) exceeds tolerance={tolerance:.3e}"
            ),
            category: "bootstrapping".to_string(),
        });
    }
    Ok(())
}

/// Validate solved value and commit to knots, returning the residual.
fn validate_and_commit_knot<T: BootstrapTarget>(
    target: &T,
    knots: &mut Vec<(f64, f64)>,
    time: f64,
    solved_value: f64,
    quote: &T::Quote,
    validation_tolerance: f64,
) -> Result<f64> {
    target.validate_knot(time, solved_value)?;

    // PERF: avoid `knots.clone()` by temporarily pushing the candidate knot and popping
    // on error. This keeps the hot loop allocation-free while preserving correctness.
    knots.push((time, solved_value));
    let result = (|| {
        let curve = target.build_curve_for_solver(knots)?;
        let residual = target.calculate_residual(&curve, quote)?;
        validate_residual(time, residual, validation_tolerance)?;
        Ok(residual)
    })();

    if result.is_err() {
        knots.pop();
    }
    result
}

/// Record a calibration iteration in the trace.
fn record_iteration(
    trace: &mut finstack_core::explain::ExplanationTrace,
    sorted_idx: usize,
    time: f64,
    residual: f64,
    validation_tolerance: f64,
    config: &CalibrationConfig,
) {
    use finstack_core::explain::TraceEntry;
    trace.push(
        TraceEntry::CalibrationIteration {
            iteration: sorted_idx,
            residual,
            knots_updated: vec![format!("t={time:.4}")],
            converged: residual.abs() <= validation_tolerance,
        },
        config.explain.max_entries,
    );
}

// =============================================================================
// Main Bootstrapper
// =============================================================================

/// Generic sequential bootstrapper.
///
/// Implements a robust sequential bootstrapping algorithm that iterates through
/// a sorted list of market quotes and solves for each curve/surface knot
/// independently. This is the industry standard for liquid interest rate
/// and credit curves where causality (independence of knots at earlier times)
/// is preserved.
///
/// The algorithm uses a hybrid bracketing-plus-polishing approach:
/// 1. **Scan**: Evaluates the objective on a grid to find a sign-change bracket.
/// 2. **Bracket**: If no bracket is found, fall back to initial guess or best point.
/// 3. **Solve**: Use Brent's method (bracketing) for robustness followed by optional
///    Newton-Raphson polishing for high-precision convergence in f-space.
pub(crate) struct SequentialBootstrapper;

impl SequentialBootstrapper {
    /// Execute the sequential bootstrapping algorithm.
    ///
    /// # Generic Parameters
    /// * `T` - The calibration target (e.g., [`DiscountCurveTarget`](crate::calibration::targets::discount::DiscountCurveTarget)).
    ///
    /// # Arguments
    /// * `target` - The domain-specific implementation of the [`BootstrapTarget`] trait.
    /// * `quotes` - The list of high-level market quotes to fit.
    /// * `initial_knots` - Optional pre-existing knots (e.g., spot or short-end anchors).
    /// * `config` - Calibration settings specifying tolerances and methods.
    /// * `success_tolerance` - Target-specific validation tolerance for determining calibration success.
    ///   If `None`, falls back to `config.solver.tolerance()`.
    /// * `trace` - Optional trace for collecting diagnostics and intermediate steps.
    ///
    /// # Returns
    /// A pair containing the calibrated term structure and a diagnostic report.
    pub(crate) fn bootstrap<T>(
        target: &T,
        quotes: &[T::Quote],
        initial_knots: Vec<(f64, f64)>,
        config: &CalibrationConfig,
        success_tolerance: Option<f64>,
        mut trace: Option<finstack_core::explain::ExplanationTrace>,
    ) -> Result<(T::Curve, CalibrationReport)>
    where
        T: BootstrapTarget,
        T::Quote: std::fmt::Debug,
    {
        let validation_tolerance = success_tolerance.unwrap_or(config.solver.tolerance());
        let sorted_quotes = sort_quotes_by_time(target, quotes)?;

        let mut knots = initial_knots;
        let mut residuals = BTreeMap::new();
        let mut total_iterations = 0;
        let mut last_time = knots.iter().map(|(t, _)| *t).fold(0.0_f64, f64::max);
        // W-40: track knot times accepted without a sign-change bracket so the
        // report can flag them as approximate rather than silently treating them
        // as true bracketed roots.
        let mut approximate_knot_times: Vec<f64> = Vec::new();

        for (sorted_idx, sq) in sorted_quotes.into_iter().enumerate() {
            validate_time_ordering(sq.time, last_time, sq.original_idx)?;
            let quote = &quotes[sq.original_idx];
            let time = sq.time;

            let (solved_value, eval_count, is_approximate) =
                Self::solve_single_knot(target, quote, &knots, time, config, validation_tolerance)?;

            if is_approximate {
                approximate_knot_times.push(time);
            }

            let residual = validate_and_commit_knot(
                target,
                &mut knots,
                time,
                solved_value,
                quote,
                validation_tolerance,
            )?;

            total_iterations += eval_count;
            last_time = time;
            residuals.insert(format!("quote_{sorted_idx:06}"), residual);

            if let Some(t) = &mut trace {
                record_iteration(t, sorted_idx, time, residual, validation_tolerance, config);
            }
        }

        let final_curve = target.build_curve_final(&knots)?;
        let mut report = CalibrationReport::for_type_with_tolerance(
            "generic_bootstrap",
            residuals,
            total_iterations,
            validation_tolerance,
        );
        report = match trace {
            Some(t) => report.with_explanation(t),
            None => report,
        };

        // W-40: flag approximate knots in report metadata so callers can
        // distinguish true bracketed roots from accepted local |f|-minima.
        if !approximate_knot_times.is_empty() {
            let times_str = approximate_knot_times
                .iter()
                .map(|t| format!("{t:.6}"))
                .collect::<Vec<_>>()
                .join(",");
            report = report.with_metadata("approximate_knots", times_str);
        }

        // Compute optional diagnostics if requested.
        if config.compute_diagnostics {
            let resid_vec: Vec<f64> = report.residuals.values().copied().collect();
            let diagnostics =
                compute_bootstrap_diagnostics(target, quotes, &knots, &resid_vec, config);
            report = report.with_diagnostics(diagnostics);
        }

        Ok((final_curve, report))
    }

    /// Solve for a single knot value using bracket + polish.
    ///
    /// Returns `(solved_value, eval_count, is_approximate)`.
    /// `is_approximate = true` means the knot was accepted from a local `|f|`-minimum
    /// without a sign-change bracket (W-40).
    fn solve_single_knot<T>(
        target: &T,
        quote: &T::Quote,
        knots: &[(f64, f64)],
        time: f64,
        config: &CalibrationConfig,
        validation_tolerance: f64,
    ) -> Result<(f64, usize, bool)>
    where
        T: BootstrapTarget,
        T::Quote: std::fmt::Debug,
    {
        let initial_guess = target.initial_guess(quote, knots)?;

        // Track the first evaluation error for diagnostics
        let first_eval_error: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
        let eval_counter = AtomicUsize::new(0);

        // Optimization: reuse buffer to avoid allocation in hot loop
        let reuse_buffer = std::cell::RefCell::new(Vec::with_capacity(knots.len() + 1));

        // Define objective function
        let objective = |value: f64| -> f64 {
            let eval_idx = eval_counter.fetch_add(1, Ordering::Relaxed) + 1;

            let mut temp_knots_guard = reuse_buffer.borrow_mut();
            temp_knots_guard.clear();
            temp_knots_guard.extend_from_slice(knots);
            temp_knots_guard.push((time, value));
            let temp_knots = &*temp_knots_guard;

            let curve = match target.build_curve_for_solver(temp_knots) {
                Ok(c) => c,
                Err(e) => {
                    if first_eval_error.borrow().is_none() {
                        *first_eval_error.borrow_mut() = Some(format!(
                            "eval#{eval_idx} curve construction failed at value={value}: {e}"
                        ));
                    }
                    return f64::NAN;
                }
            };

            match target.calculate_residual(&curve, quote) {
                Ok(r) => r,
                Err(e) => {
                    if first_eval_error.borrow().is_none() {
                        *first_eval_error.borrow_mut() = Some(format!(
                            "eval#{eval_idx} residual evaluation failed at value={value}: {e}"
                        ));
                    }
                    f64::NAN
                }
            }
        };

        // Build scan grid
        let scan_points = {
            let points = target.scan_points(quote, initial_guess)?;
            if points.is_empty() {
                build_default_scan_grid(initial_guess, config)
            } else {
                points
            }
        };
        let scan_points = normalize_scan_points(scan_points, initial_guess, time)?;

        // Solve
        let (tentative, diag) = bracket_solve_1d_with_diagnostics(
            &objective,
            initial_guess,
            &scan_points,
            config.solver.tolerance(),
            config.solver.max_iterations(),
        )?;

        // W-40: a result is "approximate" if it was NOT derived from a true sign-change
        // bracket (two scan-grid points with opposite signs). This covers:
        //   - `tentative = None` → `resolve_no_bracket` accepted a local |f|-minimum
        //   - `tentative = Some` but found via secant/best-point (no sign change)
        let (solved_value, is_approximate) = match tentative {
            Some(root) => (root, !diag.is_sign_change_bracket),
            None => resolve_no_bracket(NoBracketContext {
                time,
                quote,
                diag: &diag,
                validation_tolerance,
                first_eval_error: first_eval_error.borrow().clone(),
            })?,
        };

        Ok((solved_value, diag.eval_count, is_approximate))
    }
}

/// Compute bootstrap diagnostics via per-knot finite-difference sensitivity.
///
/// For each knot, bumps the solved value by a small amount and re-evaluates
/// the residual for the corresponding quote to estimate dResidual/dKnotValue.
fn compute_bootstrap_diagnostics<T>(
    target: &T,
    quotes: &[T::Quote],
    knots: &[(f64, f64)],
    resid_values: &[f64],
    config: &CalibrationConfig,
) -> CalibrationDiagnostics
where
    T: BootstrapTarget,
    T::Quote: std::fmt::Debug,
{
    let n = resid_values.len();
    let bump_h = super::helpers::diagnostics_bump_h(config);

    let mut per_quote = Vec::with_capacity(n);

    // Sort quotes by time for consistent ordering (matching the bootstrap solve order).
    let sorted_quotes = match sort_quotes_by_time(target, quotes) {
        Ok(sq) => sq,
        Err(_) => {
            // Fall back to basic diagnostics if sorting fails.
            return CalibrationDiagnostics::from_residuals(resid_values);
        }
    };

    // For each sorted quote, compute a finite-difference sensitivity.
    // The knots vector has initial_knots + solved knots. The solved knots
    // correspond 1:1 with the sorted quotes (appended in order).
    let n_initial = if knots.len() >= sorted_quotes.len() {
        knots.len() - sorted_quotes.len()
    } else {
        0
    };

    for (sorted_idx, sq) in sorted_quotes.iter().enumerate() {
        let knot_idx = n_initial + sorted_idx;
        let resid = resid_values.get(sorted_idx).copied().unwrap_or(0.0);

        let sensitivity = if knot_idx < knots.len() {
            let (t, v) = knots[knot_idx];
            let h = bump_h * (1.0 + v.abs());
            let quote = &quotes[sq.original_idx];

            // Central differences: O(h^2) accuracy
            let mut knots_up = knots.to_vec();
            knots_up[knot_idx] = (t, v + h);
            let mut knots_dn = knots.to_vec();
            knots_dn[knot_idx] = (t, v - h);

            let resid_up = target
                .build_curve_for_solver(&knots_up)
                .and_then(|c| target.calculate_residual(&c, quote));
            let resid_dn = target
                .build_curve_for_solver(&knots_dn)
                .and_then(|c| target.calculate_residual(&c, quote));

            match (resid_up, resid_dn) {
                (Ok(r_up), Ok(r_dn)) => (r_up - r_dn) / (2.0 * h),
                (Ok(r_up), Err(_)) => (r_up - resid) / h,
                _ => 0.0,
            }
        } else {
            0.0
        };

        per_quote.push(QuoteQuality {
            quote_label: format!("quote_{sorted_idx:06}"),
            target_value: 0.0,
            fitted_value: resid,
            residual: resid,
            sensitivity: sensitivity.abs(),
        });
    }

    let (max_residual, rms_residual) = super::helpers::residual_stats(resid_values);

    CalibrationDiagnostics {
        per_quote,
        condition_number: None, // Bootstrap is sequential; no J^T*J available.
        singular_values: None,
        max_residual,
        rms_residual,
        r_squared: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::solver::traits::BootstrapTarget;

    #[derive(Debug, Clone)]
    struct DummyTarget;

    #[derive(Debug, Clone, PartialEq)]
    struct DummyCurve(Vec<(f64, f64)>);

    impl BootstrapTarget for DummyTarget {
        type Quote = (f64, f64);
        type Curve = DummyCurve;

        fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
            Ok(quote.0)
        }

        fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
            Ok(DummyCurve(knots.to_vec()))
        }

        fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
            let current = curve
                .0
                .iter()
                .find(|(t, _)| (*t - quote.0).abs() < 1e-12)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            Ok(current - quote.1)
        }

        fn initial_guess(
            &self,
            quote: &Self::Quote,
            _previous_knots: &[(f64, f64)],
        ) -> Result<f64> {
            Ok(quote.1)
        }
    }

    #[test]
    fn bootstrap_is_order_invariant() -> Result<()> {
        let target = DummyTarget;
        let quotes = vec![(0.5, 0.01), (1.0, 0.015), (2.0, 0.02)];
        let mut shuffled = quotes.clone();
        shuffled.reverse();

        let config = CalibrationConfig::default();
        let (curve_a, _) =
            SequentialBootstrapper::bootstrap(&target, &quotes, Vec::new(), &config, None, None)?;
        let (curve_b, _) =
            SequentialBootstrapper::bootstrap(&target, &shuffled, Vec::new(), &config, None, None)?;

        assert_eq!(curve_a, curve_b);
        Ok(())
    }
}

#[cfg(test)]
mod w40_tests {
    //! W-40: `resolve_no_bracket` must flag approximate (no-bracket) knots and
    //! must not silently commit the wrong local minimum for non-monotone residuals.
    use super::*;
    use crate::calibration::solver::traits::BootstrapTarget;

    /// Non-monotone residual: f(x) = (x-2)^2*(x-5)^2 - 1.
    ///
    /// This has two local minima near x=2 and x=5 and does NOT cross zero at
    /// the "best" scan-grid points, but the minimum value f ≈ -1 < 0, so
    /// eventually there IS a sign change once we compute it properly.
    ///
    /// For the W-40 test we construct a residual that has a local |f| minimum
    /// well within `validation_tolerance` but where f does not change sign at
    /// that minimum — meaning the solver can only find it as a "best point",
    /// NOT via a bracket.
    ///
    /// f(x) = (x - root)^2 - eps:   minimum value = -eps (small, negative),
    /// reached at x = root.  If eps is within tolerance the solver would accept
    /// x=root as the "best point". But f changes sign at x = root ± sqrt(eps),
    /// so a sign-change bracket DOES exist; we hide those from the scan grid so
    /// the bracket is never found, forcing the no-bracket path.
    ///
    /// After the fix, the knot must be flagged as approximate in the report.
    #[derive(Debug, Clone)]
    struct NonMonotoneQuote {
        t: f64,
        /// True root of f (sign change occurs here only if scan grid covers it)
        root: f64,
        /// Small offset: f = (x - root)^2 - eps, so the minimum value is -eps
        eps: f64,
    }

    struct NonMonotoneTarget;

    impl BootstrapTarget for NonMonotoneTarget {
        type Quote = NonMonotoneQuote;
        type Curve = f64;

        fn quote_time(&self, quote: &Self::Quote) -> finstack_core::Result<f64> {
            Ok(quote.t)
        }

        fn build_curve(&self, knots: &[(f64, f64)]) -> finstack_core::Result<Self::Curve> {
            knots
                .last()
                .map(|(_, v)| *v)
                .ok_or(finstack_core::Error::Input(
                    finstack_core::InputError::TooFewPoints,
                ))
        }

        fn calculate_residual(
            &self,
            curve: &Self::Curve,
            quote: &Self::Quote,
        ) -> finstack_core::Result<f64> {
            let dx = *curve - quote.root;
            // f(x) = dx^2 - eps: minimum = -eps at x=root, sign changes at x = root ± sqrt(eps)
            Ok(dx * dx - quote.eps)
        }

        fn initial_guess(
            &self,
            quote: &Self::Quote,
            _previous_knots: &[(f64, f64)],
        ) -> finstack_core::Result<f64> {
            Ok(quote.root + 1.0)
        }

        fn scan_points(
            &self,
            quote: &Self::Quote,
            _initial_guess: f64,
        ) -> finstack_core::Result<Vec<f64>> {
            // Deliberately exclude the sign-change region (root ± sqrt(eps)).
            // The minimum at x=root is within tolerance, so without a sign change
            // the solver will accept it as a "best point".
            // We use points that stay in f > 0 region on both sides (far from root),
            // then also include x=root exactly where f = -eps (in-tolerance minimum).
            let sq_eps = quote.eps.sqrt();
            let r = quote.root;
            // Points outside the sign-change bracket but including the minimum:
            Ok(vec![
                r - 3.0 * sq_eps, // f > 0
                r - 2.5 * sq_eps, // f > 0
                r - 2.0 * sq_eps, // f = (4-1)*eps > 0  [actually 3*eps]
                r - 1.5 * sq_eps, // f = (2.25-1)*eps > 0
                r - 0.5 * sq_eps, // f = (0.25-1)*eps < 0  -- INSIDE bracket!
                r,                // f = -eps (minimum, in-tolerance)
                r + 0.5 * sq_eps, // f < 0  -- INSIDE bracket!
                r + 1.5 * sq_eps, // f > 0
                r + 2.0 * sq_eps, // f > 0
                r + 2.5 * sq_eps, // f > 0
                r + 3.0 * sq_eps, // f > 0
            ])
        }
    }

    /// W-40 regression: with the scan grid including sign-change points, the
    /// bracketing solver should find a true root, not accept a local minimum.
    /// After the fix, when NO bracket exists, the result must be flagged approximate.
    ///
    /// This test constructs the case where the bracket IS discoverable (sign
    /// changes are in the grid), but verifies the solver actually finds a real
    /// root (|f| very small, near ±sqrt(eps)).
    #[test]
    fn w40_non_monotone_residual_solver_finds_real_root_not_local_minimum() {
        let target = NonMonotoneTarget;
        let eps = 1e-8; // small: minimum |f| = eps, sign changes at root ± 1e-4
        let q = NonMonotoneQuote {
            t: 1.0,
            root: 0.5,
            eps,
        };
        let cfg = crate::calibration::CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-6) // tolerance >> eps, so the local min is "in tolerance"
                .with_max_iterations(200),
            ..crate::calibration::CalibrationConfig::default()
        };
        // With the scan grid including sign-change points, the solver should bracket
        // and converge to a true root.
        let (curve, report) =
            SequentialBootstrapper::bootstrap(&target, &[q], vec![(0.0, 0.0)], &cfg, None, None)
                .expect("bootstrap should succeed when sign change is in grid");

        // The true roots are at root ± sqrt(eps) = 0.5 ± 1e-4.
        let sq_eps = eps.sqrt();
        let dist_to_nearest_true_root =
            ((curve - (0.5 + sq_eps)).abs()).min((curve - (0.5 - sq_eps)).abs());

        assert!(
            dist_to_nearest_true_root < 1e-3,
            "solver should converge to a true root (sign change), not just any local min; \
             curve={curve}, nearest true root distance={dist_to_nearest_true_root}"
        );
        assert!(report.success, "report should be successful");
    }

    /// W-40 regression: when NO sign-change bracket is discoverable, the solver
    /// must flag the accepted knot as approximate in the report metadata, not
    /// silently commit it as if it were a true root.
    ///
    /// We hide the sign-change points from the grid so the solver can only find
    /// the local minimum, not a proper bracket.
    #[derive(Debug, Clone)]
    struct NonMonotoneNoBracketQuote {
        t: f64,
        root: f64,
        eps: f64,
    }

    struct NonMonotoneNoBracketTarget;

    impl BootstrapTarget for NonMonotoneNoBracketTarget {
        type Quote = NonMonotoneNoBracketQuote;
        type Curve = f64;

        fn quote_time(&self, quote: &Self::Quote) -> finstack_core::Result<f64> {
            Ok(quote.t)
        }

        fn build_curve(&self, knots: &[(f64, f64)]) -> finstack_core::Result<Self::Curve> {
            knots
                .last()
                .map(|(_, v)| *v)
                .ok_or(finstack_core::Error::Input(
                    finstack_core::InputError::TooFewPoints,
                ))
        }

        fn calculate_residual(
            &self,
            curve: &Self::Curve,
            quote: &Self::Quote,
        ) -> finstack_core::Result<f64> {
            let dx = *curve - quote.root;
            // f(x) = dx^2 + eps: strictly positive everywhere, minimum = eps at x=root.
            // No sign change anywhere — a true |f|-minimum with no zero crossing.
            // If eps < tolerance, the minimum is within tolerance and will be accepted
            // as the "best point" without a bracket.
            Ok(dx * dx + quote.eps)
        }

        fn initial_guess(
            &self,
            quote: &Self::Quote,
            _previous_knots: &[(f64, f64)],
        ) -> finstack_core::Result<f64> {
            // Start away from the minimum so the scan grid drives the search.
            Ok(quote.root + 2.0)
        }

        fn scan_points(
            &self,
            quote: &Self::Quote,
            _initial_guess: f64,
        ) -> finstack_core::Result<Vec<f64>> {
            // f(x) = dx^2 + eps >= eps > 0 always. No sign changes possible.
            // Include x=root where |f| = eps < tolerance (the "best point").
            let r = quote.root;
            Ok(vec![
                r - 3.0,
                r - 2.0,
                r - 1.5,
                r - 1.0,
                r - 0.5,
                r, // f = eps (minimum, in-tolerance)
                r + 0.5,
                r + 1.0,
            ])
        }
    }

    /// W-40: when no sign-change bracket is found but best |f| <= tolerance,
    /// the report must flag the result as approximate (not silently treat it as
    /// a true bracketed root). Pre-fix: the report has no such flag. Post-fix:
    /// the report metadata includes "approximate_knots".
    ///
    /// Residual: f(x) = (x - root)^2 + eps, strictly positive everywhere.
    /// No zero crossing exists. The minimum |f| = eps < tolerance is at x=root.
    /// The solver accepts this as "best point" — this is the W-40 defect.
    #[test]
    fn w40_no_bracket_approximate_knot_is_flagged_in_report() {
        let target = NonMonotoneNoBracketTarget;
        let tolerance = 1e-4;
        let eps = tolerance * 0.4; // |f_min| = eps < tolerance at x=root
        let q = NonMonotoneNoBracketQuote {
            t: 1.0,
            root: 0.5,
            eps,
        };
        let cfg = crate::calibration::CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(tolerance)
                .with_max_iterations(200),
            ..crate::calibration::CalibrationConfig::default()
        };

        // After the fix: when a knot is accepted without a bracket, the report
        // must flag it. Pre-fix: this test detects the ABSENCE of the flag.
        let result = SequentialBootstrapper::bootstrap(
            &target,
            &[q],
            vec![(0.0, 0.0)],
            &cfg,
            Some(tolerance),
            None,
        );

        // The test verifies:
        // 1. If bootstrap succeeds (best point accepted), the report MUST contain
        //    an "approximate_knots" metadata key.
        // 2. If bootstrap fails (rejects no-bracket), that's also acceptable — but
        //    the current code DOES accept it silently (the defect).
        match result {
            Ok((_curve, report)) => {
                // Post-fix: must have flag indicating approximate acceptance.
                assert!(
                    report.metadata.contains_key("approximate_knots"),
                    "W-40: bootstrap accepted a knot without a sign-change bracket but \
                     did not flag it as approximate in report.metadata; \
                     metadata keys: {:?}",
                    report.metadata.keys().collect::<Vec<_>>()
                );
            }
            Err(_) => {
                // Also acceptable: rejecting a no-bracket result is a valid fix.
                // This branch means the fix chose to reject rather than flag.
            }
        }
    }
}

#[cfg(test)]
mod solver_tests {
    use super::*;
    use finstack_core::Error;

    #[derive(Debug, Clone)]
    struct DummyQuote {
        t: f64,
        root: f64,
        scale: f64,
        unsorted_scan: bool,
        infeasible_below: Option<f64>,
    }

    struct DummyTarget;

    impl BootstrapTarget for DummyTarget {
        type Quote = DummyQuote;
        type Curve = f64;

        fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
            Ok(quote.t)
        }

        fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
            knots
                .last()
                .map(|(_, v)| *v)
                .ok_or(Error::Input(finstack_core::InputError::TooFewPoints))
        }

        fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
            self.build_curve(knots)
        }

        fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
            if let Some(th) = quote.infeasible_below {
                if *curve < th {
                    return Err(Error::Calibration {
                        message: format!("infeasible curve value {}", curve),
                        category: "test".to_string(),
                    });
                }
            }
            // Residual is scaled in f-space to test tolerance enforcement.
            Ok(quote.scale * (*curve - quote.root))
        }

        fn initial_guess(
            &self,
            _quote: &Self::Quote,
            _previous_knots: &[(f64, f64)],
        ) -> Result<f64> {
            Ok(0.0)
        }

        fn scan_points(&self, quote: &Self::Quote, _initial_guess: f64) -> Result<Vec<f64>> {
            // Dense grid so the bracket solver's debug_assert (>= 8 points) is
            // satisfied. Real BootstrapTarget impls always build dense grids.
            //
            // Note: the grid deliberately excludes the test root (0.5) so the
            // `scale: 1e9` "all evaluations penalised" path remains exercised
            // by `bootstrap_rejects_when_all_objective_evals_are_penalized`.
            let base = vec![-1.0, -0.6, -0.25, 0.0, 0.1, 0.25, 0.7, 0.85, 1.0];
            if quote.unsorted_scan {
                // Same points, deliberately unsorted, to exercise the
                // sorting/dedup path in `normalize_scan_points`.
                Ok(vec![1.0, -0.25, 0.0, 0.7, 0.85, 0.1, 0.25, -0.6, -1.0])
            } else {
                Ok(base)
            }
        }

        fn validate_knot(&self, _time: f64, value: f64) -> Result<()> {
            if !value.is_finite() {
                return Err(Error::Calibration {
                    message: "non-finite knot".to_string(),
                    category: "test".to_string(),
                });
            }
            Ok(())
        }
    }

    #[test]
    fn bootstrap_succeeds_with_unsorted_scan_points() {
        let target = DummyTarget;
        let q = DummyQuote {
            t: 1.0,
            root: 0.5,
            scale: 1.0,
            unsorted_scan: true,
            infeasible_below: None,
        };
        let cfg = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-10)
                .with_max_iterations(200),
            ..CalibrationConfig::default()
        };
        let (curve, report) =
            SequentialBootstrapper::bootstrap(&target, &[q], vec![(0.0, 0.0)], &cfg, None, None)
                .expect("bootstrap should succeed");
        assert!((curve - 0.5).abs() < 1e-6);
        assert!(report.success);
    }

    #[test]
    fn bootstrap_succeeds_with_infeasible_trial_points() {
        // Some objective evaluations error out (infeasible region), but a valid root exists.
        let target = DummyTarget;
        let q = DummyQuote {
            t: 1.0,
            root: 0.5,
            scale: 1.0,
            unsorted_scan: false,
            infeasible_below: Some(0.0),
        };
        let cfg = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-10)
                .with_max_iterations(200),
            ..CalibrationConfig::default()
        };
        let (curve, report) =
            SequentialBootstrapper::bootstrap(&target, &[q], vec![(0.0, 0.0)], &cfg, None, None)
                .expect("bootstrap should succeed despite infeasible points");
        assert!((curve - 0.5).abs() < 1e-6);
        assert!(report.success);
    }

    #[test]
    fn bootstrap_rejects_when_all_objective_evals_are_penalized() {
        // Extremely steep residuals can exceed the objective validity cap used by the
        // bracketing diagnostics. In that case, we should fail with a clear error.
        let target = DummyTarget;
        let q = DummyQuote {
            t: 1.0,
            root: 0.5,
            scale: 1e9, // makes |f| >> OBJECTIVE_VALID_ABS_MAX across the scan grid
            unsorted_scan: false,
            infeasible_below: None,
        };
        let cfg = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-10)
                .with_max_iterations(200),
            ..CalibrationConfig::default()
        };
        let err =
            SequentialBootstrapper::bootstrap(&target, &[q], vec![(0.0, 0.0)], &cfg, None, None)
                .expect_err("should fail when all evaluations are penalized");
        let msg = format!("{err}");
        assert!(
            msg.contains("all")
                && msg.contains("objective evaluations")
                && msg.contains("invalid/penalized"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn bootstrap_enforces_f_space_tolerance_not_just_x_space() {
        // Core Brent termination is x-space based. For large-magnitude roots and steep residuals,
        // x-space termination can occur while |residual| is still far above tolerance.
        // This test ensures the bootstrapper enforces |residual| <= tolerance after solving.
        #[derive(Debug, Clone)]
        struct SteepQuote {
            t: f64,
            root: f64,
            scale: f64,
        }

        struct SteepTarget;

        impl BootstrapTarget for SteepTarget {
            type Quote = SteepQuote;
            type Curve = f64;

            fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
                Ok(quote.t)
            }

            fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
                knots
                    .last()
                    .map(|(_, v)| *v)
                    .ok_or(Error::Input(finstack_core::InputError::TooFewPoints))
            }

            fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
                let dx = *curve - quote.root;
                // Make the true root occur at a sub-ULP shift from `quote.root` at this magnitude
                // so no representable x can achieve |residual| <= tol.
                // This forces the bootstrapper's post-solve f-space tolerance check to trigger.
                Ok(quote.scale * dx + dx * dx * dx + 1e-6)
            }

            fn initial_guess(
                &self,
                quote: &Self::Quote,
                _previous_knots: &[(f64, f64)],
            ) -> Result<f64> {
                // Avoid starting at the root, and avoid symmetric brackets that hit the root at the first midpoint.
                Ok(quote.root + 1.3)
            }

            fn scan_points(&self, quote: &Self::Quote, _initial_guess: f64) -> Result<Vec<f64>> {
                // Deliberately asymmetric bracket so bisection midpoints don't
                // immediately equal `root`. Padded with intermediate points so
                // the solver's debug_assert (>= 8 points) is satisfied.
                Ok(vec![
                    quote.root - 1.0,
                    quote.root - 0.7,
                    quote.root - 0.4,
                    quote.root - 0.1,
                    quote.root + 0.1,
                    quote.root + 0.4,
                    quote.root + 1.0,
                    quote.root + 2.0,
                ])
            }
        }

        let target = SteepTarget;
        let q = SteepQuote {
            t: 1.0,
            root: 1.0e8 + 0.1,
            // Keep |f| within the objective-valid cap for scan points (dx=±1 => |f| ~ 1e4).
            scale: 1.0e4,
        };
        let cfg = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-10)
                .with_max_iterations(200),
            ..CalibrationConfig::default()
        };
        let err =
            SequentialBootstrapper::bootstrap(&target, &[q], vec![(0.0, 0.0)], &cfg, None, None)
                .expect_err("bootstrap should fail due to f-space tolerance enforcement");
        let msg = format!("{err}");
        assert!(
            msg.contains("exceeds tolerance"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn bootstrap_rejects_non_increasing_times() {
        let target = DummyTarget;
        let q1 = DummyQuote {
            t: 1.0,
            root: 0.5,
            scale: 1.0,
            unsorted_scan: false,
            infeasible_below: None,
        };
        let q2 = DummyQuote {
            t: 1.0,
            ..q1.clone()
        };
        let cfg = CalibrationConfig::default();
        let err = SequentialBootstrapper::bootstrap(
            &target,
            &[q1, q2],
            vec![(0.0, 0.0)],
            &cfg,
            None,
            None,
        )
        .expect_err("should reject duplicate times");
        assert!(format!("{err}").contains("duplicate quote times"));
    }

    #[test]
    fn bootstrap_is_deterministic_under_quote_shuffling() {
        let target = DummyTarget;
        let q_short = DummyQuote {
            t: 1.0,
            root: 0.25,
            scale: 1.0,
            unsorted_scan: false,
            infeasible_below: None,
        };
        let q_long = DummyQuote {
            t: 2.0,
            root: 0.75,
            scale: 1.0,
            unsorted_scan: false,
            infeasible_below: None,
        };
        let cfg = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_tolerance(1e-12)
                .with_max_iterations(200),
            ..CalibrationConfig::default()
        };

        let (curve_sorted, report_sorted) = SequentialBootstrapper::bootstrap(
            &target,
            &[q_short.clone(), q_long.clone()],
            vec![(0.0, 0.0)],
            &cfg,
            None,
            None,
        )
        .expect("sorted input should succeed");
        let (curve_shuffled, report_shuffled) = SequentialBootstrapper::bootstrap(
            &target,
            &[q_long, q_short],
            vec![(0.0, 0.0)],
            &cfg,
            None,
            None,
        )
        .expect("shuffled input should succeed");

        assert!((curve_sorted - curve_shuffled).abs() < 1e-12);
        assert_eq!(report_sorted.residuals, report_shuffled.residuals);
        assert!((report_sorted.rmse - report_shuffled.rmse).abs() < 1e-12);
        assert!((report_sorted.objective_value - report_shuffled.objective_value).abs() < 1e-12);
    }
}
