//! Solver helpers and common penalty/diagnostics utilities for calibration.
//!
//! This module intentionally contains the implementation logic that `calibration/mod.rs`
//! re-exports. Keeping it here allows `mod.rs` to stay export-only.

use crate::calibration::constants::OBJECTIVE_VALID_ABS_MAX;
use crate::calibration::CalibrationConfig;

/// Finite-difference bump step used by both bootstrap and global diagnostics.
/// Returns `max(config.discount_curve.jacobian_step_size, 1e-8)`.
pub(crate) fn diagnostics_bump_h(config: &CalibrationConfig) -> f64 {
    config.discount_curve.jacobian_step_size.max(1e-8)
}

/// `(max_abs_residual, rms_residual)` over a residual vector — shared between the
/// bootstrap and global diagnostics computations so the two paths cannot drift.
pub(crate) fn residual_stats(resid_values: &[f64]) -> (f64, f64) {
    let max_residual = resid_values.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
    let rms_residual = if resid_values.is_empty() {
        0.0
    } else {
        (resid_values.iter().map(|r| r * r).sum::<f64>() / resid_values.len() as f64).sqrt()
    };
    (max_residual, rms_residual)
}
#[cfg(test)]
use crate::calibration::constants::PENALTY;
use finstack_core::Result;

/// Diagnostics from bracketing scan, useful for error reporting.
///
/// Tracks the effectiveness of the initial scan grid and identifies the
/// best points observed if formal convergence fails.
#[derive(Debug, Clone)]
pub(crate) struct BracketDiagnostics {
    /// Whether the solver returned a candidate (via bracket, secant, or best-point).
    pub bracket_found: bool,
    /// Whether the returned candidate came from a true sign-change bracket
    /// (i.e., two scan-grid points with opposite-sign objective values were found
    /// and used to converge). When `false` but `bracket_found` is `true`, the
    /// candidate was accepted via the no-bracket secant fallback or as a local
    /// |f|-minimum — it should be treated as approximate (W-40).
    pub is_sign_change_bracket: bool,
    /// Best candidate point (minimum |f|) observed during the scan.
    pub best_point: Option<f64>,
    /// Best objective value (minimum |f|) observed during the scan.
    pub best_value: Option<f64>,
    /// Total number of objective evaluations performed.
    pub eval_count: usize,
    /// Number of valid (non-penalized, non-NaN) evaluations.
    pub valid_eval_count: usize,
    /// Scan bounds used by the grid search [lo, hi].
    pub scan_bounds: (f64, f64),
}

impl BracketDiagnostics {
    fn new(scan_points: &[f64]) -> Self {
        let lo = scan_points.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = scan_points
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        Self {
            bracket_found: false,
            is_sign_change_bracket: false,
            best_point: None,
            best_value: None,
            eval_count: 0,
            valid_eval_count: 0,
            scan_bounds: (lo, hi),
        }
    }

    fn update(&mut self, point: f64, value: f64) {
        self.eval_count += 1;
        if value.is_finite() && value.abs() < OBJECTIVE_VALID_ABS_MAX {
            self.valid_eval_count += 1;
            let is_better = match self.best_value {
                None => true,
                Some(best) => value.abs() < best.abs(),
            };
            if is_better {
                self.best_point = Some(point);
                self.best_value = Some(value);
            }
        }
    }
}

/// Minimum scan-grid size enforced in debug builds.
///
/// The geometric bracket-expansion fallback was removed in favour of letting
/// callers own the scan grid (each `BootstrapTarget` builds a maturity- or
/// rate-aware grid that beats a one-size-fits-all expansion). A grid smaller
/// than this almost certainly indicates a caller bug, not a deliberate choice.
#[cfg(debug_assertions)]
const MIN_DEBUG_SCAN_GRID_LEN: usize = 8;

/// Like `bracket_solve_1d` but also returns diagnostics for error reporting.
pub(crate) fn bracket_solve_1d_with_diagnostics(
    objective: &dyn Fn(f64) -> f64,
    initial: f64,
    scan_points: &[f64],
    tol: f64,
    max_iters: usize,
) -> Result<(Option<f64>, BracketDiagnostics)> {
    // The adaptive geometric expansion previously embedded here was removed;
    // callers must now provide a grid dense enough to bracket the root. Catch
    // sparse grids early in debug builds so the regression surfaces in tests
    // rather than as a silent "no bracket found" at the validation step.
    #[cfg(debug_assertions)]
    debug_assert!(
        scan_points.len() >= MIN_DEBUG_SCAN_GRID_LEN,
        "bracket_solve_1d_with_diagnostics: scan grid has {} points (< {}); \
         the adaptive bracket-expansion fallback was removed, so callers must \
         supply a grid that spans the feasible region",
        scan_points.len(),
        MIN_DEBUG_SCAN_GRID_LEN
    );

    let mut diag = BracketDiagnostics::new(scan_points);
    let mut valid_points: Vec<(f64, f64)> = Vec::with_capacity(scan_points.len() + 8);

    let v0 = objective(initial);
    diag.update(initial, v0);
    if v0.is_finite() && v0.abs() < tol {
        diag.bracket_found = true;
        return Ok((Some(initial), diag));
    }
    if v0.is_finite() && v0.abs() < OBJECTIVE_VALID_ABS_MAX {
        valid_points.push((initial, v0));
    }

    for &point in scan_points {
        let value = objective(point);
        diag.update(point, value);

        if !value.is_finite() || value.abs() >= OBJECTIVE_VALID_ABS_MAX {
            continue;
        }
        valid_points.push((point, value));
    }

    if valid_points.is_empty() {
        return Ok((None, diag));
    }

    // Robust bracket selection:
    // sort by x and choose the bracket whose midpoint is closest to the initial guess.
    valid_points.sort_by(|a, b| a.0.total_cmp(&b.0));
    type Bracket = ((f64, f64), (f64, f64), f64); // ((x0,f0),(x1,f1),score)
    let mut best_bracket: Option<Bracket> = None;
    for w in valid_points.windows(2) {
        let (x0, f0) = w[0];
        let (x1, f1) = w[1];
        if f0.abs() < tol {
            diag.bracket_found = true;
            return Ok((Some(x0), diag));
        }
        if f1.abs() < tol {
            diag.bracket_found = true;
            return Ok((Some(x1), diag));
        }
        if f0.signum() == f1.signum() {
            continue;
        }
        let mid = 0.5 * (x0 + x1);
        let score = (mid - initial).abs();
        let replace = match &best_bracket {
            None => true,
            Some((_, _, best_score)) => score < *best_score,
        };
        if replace {
            best_bracket = Some(((x0, f0), (x1, f1), score));
        }
    }

    let Some(((mut a, mut fa), (mut b, mut fb), _)) = best_bracket else {
        // No sign-change found. Run a bounded secant fallback from the best
        // observed point. NOTE: any result from this fallback is NOT a true
        // sign-change root; callers check `is_sign_change_bracket` to detect this.
        //
        // S3: previously this used Newton-with-central-FD, costing 3 objective
        // evaluations per step (the iterate plus two FD probes). Each call
        // can be expensive — for distressed credit, one CDS pricing involves
        // ~750 protection-leg sub-window integrals — so the FD overhead
        // dominated. Secant achieves the same superlinear convergence with
        // 1 evaluation per step by reusing the (x, f(x)) pair from the prior
        // iterate; we prime it from the best observed scan-grid point and
        // its closest valid neighbour.
        if let Some(x0) = diag.best_point {
            let lo = diag.scan_bounds.0;
            let hi = diag.scan_bounds.1;
            let iters = max_iters.clamp(50, 200);

            // Bootstrap a second `(x_prev, f_prev)` from the closest valid
            // scan point so the secant slope is meaningful from step 1.
            // valid_points is already sorted by x.
            let Some(mut fx) = diag.best_value else {
                return Ok((None, diag));
            };
            let mut x = x0;
            let (mut x_prev, mut f_prev) = {
                let nearest = valid_points
                    .iter()
                    .filter(|(xp, _)| (xp - x).abs() > 1e-16)
                    .min_by(|(xa, _), (xb, _)| (xa - x).abs().total_cmp(&(xb - x).abs()));
                match nearest {
                    Some(&(xp, fp)) => (xp, fp),
                    None => {
                        // Fall back to a one-sided FD probe to get a slope.
                        let h = (1e-6_f64).max(1e-6 * x.abs());
                        let xp = (x + h).clamp(lo, hi);
                        if (xp - x).abs() < 1e-16 {
                            return Ok((None, diag));
                        }
                        let fp = objective(xp);
                        diag.update(xp, fp);
                        if !fp.is_finite() {
                            return Ok((None, diag));
                        }
                        (xp, fp)
                    }
                }
            };

            for _ in 0..iters {
                if fx.is_finite() && fx.abs() < tol {
                    diag.bracket_found = true;
                    return Ok((Some(x), diag));
                }
                if !fx.is_finite() || fx.abs() >= OBJECTIVE_VALID_ABS_MAX {
                    break;
                }

                let dx = x - x_prev;
                let df = fx - f_prev;
                if !df.is_finite() || df.abs() < 1e-16 || dx.abs() < 1e-16 {
                    break;
                }
                let slope = df / dx;
                let x_next = (x - fx / slope).clamp(lo, hi);
                if !x_next.is_finite() || (x_next - x).abs() < 1e-16 {
                    break;
                }

                let f_next = objective(x_next);
                diag.update(x_next, f_next);

                // Slide the window forward.
                x_prev = x;
                f_prev = fx;
                x = x_next;
                fx = f_next;
            }
        }

        return Ok((None, diag));
    };

    // W-40: a sign-change bracket was found (two consecutive scan points with
    // opposite signs). Mark this before entering bisection/false-position so
    // callers can distinguish true bracketed roots from approximate |f|-minima.
    diag.is_sign_change_bracket = true;

    // Market-standard: bracket is valid; converge primarily on f-space (|f| < tol).
    // We prefer a simple bisection on the bracket to guarantee reduction in |f|
    // for well-behaved monotone objectives. If midpoints become invalid/penalized,
    // we fall back to Brent+Newton.
    //
    // X-space early-break: when the bracket width collapses to machine precision
    // bisection cannot further improve the candidate, but the false-position
    // fallback below may still reduce |f| via secant-style updates inside the
    // tight bracket. So we `break` (don't return) and let the fallback polish.
    let x_tol_abs = 1e-14_f64;
    let x_tol_rel = 1e-12_f64;
    for _ in 0..max_iters.max(50) {
        let m = 0.5 * (a + b);
        let fm = objective(m);
        diag.update(m, fm);

        if fm.is_finite() && fm.abs() < tol {
            diag.bracket_found = true;
            return Ok((Some(m), diag));
        }

        if !fm.is_finite() || fm.abs() >= OBJECTIVE_VALID_ABS_MAX {
            // Midpoint produced a penalized/infeasible value. Stop bisecting; leave
            // (a,fa,b,fb) at their last good values so the false-position fallback
            // below still has a valid sign-changing bracket.
            break;
        }

        if fa.signum() != fm.signum() {
            b = m;
            fb = fm;
        } else {
            a = m;
            fa = fm;
        }

        let bracket_width = b - a;
        let x_tol = x_tol_abs.max(x_tol_rel * (a.abs().max(b.abs())));
        if bracket_width <= x_tol {
            break;
        }
    }

    // If we already met tolerance, return the best observed point.
    if let (Some(best_point), Some(best_value)) = (diag.best_point, diag.best_value) {
        if best_value.is_finite() && best_value.abs() < tol {
            diag.bracket_found = true;
            return Ok((Some(best_point), diag));
        }
    }

    // The false-position fallback requires a valid sign-changing bracket. After bisection
    // either: (a) we converged (handled above), (b) bisection_ok = false because a midpoint
    // produced |f| >= OBJECTIVE_VALID_ABS_MAX and `(a,fa,b,fb)` are last-known-good, or
    // (c) we ran out of iterations with a still-valid bracket. In all three cases the
    // invariant `fa.signum() != fb.signum()` should hold; guard explicitly so that an
    // edge case (e.g. fa or fb became NaN upstream) cannot drive FP on a stale bracket.
    let bracket_valid = fa.is_finite() && fb.is_finite() && fa.signum() != fb.signum() && a < b;
    if !bracket_valid {
        return Ok((diag.best_point, diag));
    }

    // Fallback: stay inside the discovered bracket with bounded false-position updates.
    for _ in 0..max_iters.max(50) {
        let denom = fb - fa;
        let mut candidate = if denom.is_finite() && denom.abs() > f64::EPSILON {
            a - fa * (b - a) / denom
        } else {
            0.5 * (a + b)
        };
        if !candidate.is_finite() || candidate <= a || candidate >= b {
            candidate = 0.5 * (a + b);
        }

        let fc = objective(candidate);
        diag.update(candidate, fc);
        if fc.is_finite() && fc.abs() < tol {
            diag.bracket_found = true;
            return Ok((Some(candidate), diag));
        }
        if !fc.is_finite() || fc.abs() >= OBJECTIVE_VALID_ABS_MAX {
            break;
        }

        if fa.signum() != fc.signum() {
            b = candidate;
            fb = fc;
        } else if fc.signum() != fb.signum() {
            a = candidate;
            fa = fc;
        } else {
            break;
        }
    }

    Ok((None, diag))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a dense scan grid that satisfies the caller-side debug invariant.
    /// Tests that previously hand-crafted 2-5 point grids would now trip the
    /// debug_assert added with the bracket-expansion removal.
    fn dense_scan(lo: f64, hi: f64, anchors: &[f64]) -> Vec<f64> {
        let mut pts = anchors.to_vec();
        let n = 12usize;
        for i in 0..n {
            let t = i as f64 / (n - 1) as f64;
            pts.push(lo + t * (hi - lo));
        }
        pts.sort_by(|a, b| a.total_cmp(b));
        pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        pts
    }

    #[test]
    fn test_bracket_solve_1d_finds_root() {
        // f(x) = x - 0.5 has root at 0.5
        let f = |x: f64| x - 0.5;
        let scan = dense_scan(-1.0, 1.0, &[0.25, 0.75]);
        let (root, _) =
            bracket_solve_1d_with_diagnostics(&f, 0.0, &scan, 1e-12, 100).expect("solver error");
        let r = root.expect("root should be Some");
        assert!((r - 0.5).abs() < 1e-9, "root inaccurate: {}", r);
    }

    #[test]
    fn test_bracket_diagnostics_tracking() {
        // f(x) = x - 0.5 has root at 0.5
        let f = |x: f64| x - 0.5;
        let scan = dense_scan(0.0, 1.0, &[0.5]);
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.3, &scan, 1e-12, 100).expect("solver error");

        assert!(root.is_some());
        assert!(diag.bracket_found);
        // At least 1 eval (initial) + some scan points before finding bracket
        assert!(diag.eval_count >= 1, "eval_count={}", diag.eval_count);
        assert!(
            diag.valid_eval_count >= 1,
            "valid_eval_count={}",
            diag.valid_eval_count
        );
        assert_eq!(diag.scan_bounds, (0.0, 1.0));
    }

    #[test]
    fn bracket_solver_evaluates_authoritative_scan_grid() {
        // Stronger contract than the original "did we see point 42.0":
        //   * Every scan-grid point must be evaluated before bracket selection
        //     (no silent early-exit dropping caller-supplied grid).
        //   * The bracket selected must be the one whose midpoint is closest
        //     to the initial guess (matches helpers.rs:118 sort-then-score).
        use std::cell::RefCell;

        let seen = RefCell::new(Vec::new());
        let f = |x: f64| {
            seen.borrow_mut().push(x);
            x - 0.5
        };
        // Dense grid; include a far-away outlier to verify no early-exit.
        let scan = {
            let mut s = dense_scan(0.0, 1.0, &[0.49]);
            s.push(42.0);
            s.sort_by(|a, b| a.total_cmp(b));
            s
        };
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.49, &scan, 1e-12, 100).expect("solver error");

        let r = root.expect("root should be found");
        assert!((r - 0.5).abs() < 1e-9, "incorrect root: {}", r);

        let observed: Vec<f64> = seen.borrow().iter().copied().collect();
        for x in &scan {
            assert!(
                observed.iter().any(|s| (s - x).abs() < 1e-12),
                "solver did not evaluate scan grid point {x}; observed = {observed:?}"
            );
        }
        assert!(
            diag.eval_count >= scan.len(),
            "eval_count {} < scan grid size {}",
            diag.eval_count,
            scan.len()
        );
    }

    #[test]
    fn bracket_solver_no_silent_wrong_root_on_sparse_grid() {
        // S1: a sparse grid that *just barely* fails to bracket the root must
        // not silently return a wrong answer via the Newton fallback. Either
        // we converge near the true root (good) or return None (good); we must
        // never return a value far from the actual root.
        // Note: the test is run in release-mode CI where the debug_assert is
        // inactive; in debug builds the sparse grid will trip the assertion.
        // We use exactly MIN_DEBUG_SCAN_GRID_LEN points so debug builds still
        // exercise the path under test.
        let f = |x: f64| x.powi(3) - 2.0 * x + 1.0; // roots at 1, ~0.618, ~-1.618
                                                    // Points that bracket the root at x=1 only via Newton-fallback secant.
        let scan: Vec<f64> = (0..8).map(|i| -2.0 + 0.5 * (i as f64)).collect();
        // = [-2, -1.5, -1, -0.5, 0, 0.5, 1, 1.5]
        let (root, _diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.95, &scan, 1e-9, 100).expect("solver error");
        if let Some(r) = root {
            // Must be near a real root; never far from one.
            let d_to_real_roots = [1.0_f64, 0.6180339887, -1.6180339887]
                .iter()
                .map(|root| (r - root).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(
                d_to_real_roots < 1e-6,
                "solver returned r={r} which is not near any real root of x^3-2x+1"
            );
        }
    }

    #[test]
    fn test_bracket_diagnostics_no_bracket() {
        // f(x) = x^2 + 1 has no real root
        let f = |x: f64| x * x + 1.0;
        let scan = dense_scan(0.0, 2.0, &[0.5, 1.0, 1.5]);
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 1.0, &scan, 1e-12, 100).expect("solver error");

        assert!(root.is_none());
        assert!(!diag.bracket_found);
        assert!(diag.eval_count >= 5);
        // Best point should be at x=0 where f(0)=1 is minimum
        assert!(diag.best_point.is_some());
        assert!((diag.best_point.expect("best_point asserted above") - 0.0).abs() < 0.01);
        assert!((diag.best_value.expect("best_value should exist") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_bracket_diagnostics_penalized_values() {
        // f(x) returns PENALTY for x < 0.5, otherwise x - 0.5
        let f = |x: f64| if x < 0.5 { PENALTY } else { x - 0.75 };
        let scan = dense_scan(0.0, 1.0, &[0.5, 0.75]);
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.5, &scan, 1e-12, 100).expect("solver error");

        // Should find root at 0.75
        assert!(root.is_some());
        // Only values >= 0.5 are valid (not penalized)
        assert!(diag.valid_eval_count < diag.eval_count);
    }

    #[test]
    fn test_bracket_fallback_stays_inside_discovered_domain() {
        let f = |x: f64| {
            if !(0.0..=1.0).contains(&x) || (0.30..0.70).contains(&x) {
                PENALTY
            } else {
                x - 0.15
            }
        };
        let scan = dense_scan(0.0, 1.0, &[0.0, 0.15, 0.25, 0.75, 0.9, 1.0]);
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.8, &scan, 1e-12, 100).expect("solver error");

        let root = root.expect("bounded fallback should recover the bracketed root");
        assert!(
            (root - 0.15).abs() < 1e-8,
            "unexpected root from bounded fallback: {root}"
        );
        assert!(diag.bracket_found, "fallback root should be bracket-safe");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "bracket_solve_1d_with_diagnostics: scan grid has")]
    fn debug_assert_rejects_sparse_scan_grid() {
        // C4: callers that supply a too-sparse scan grid trip the debug
        // assertion. This ensures the contract "callers own a dense grid"
        // is enforced at test time, not silently when calibration regresses.
        let f = |x: f64| x - 0.5;
        let scan = [0.0, 1.0]; // only 2 points
        let _ =
            bracket_solve_1d_with_diagnostics(&f, 0.5, &scan, 1e-12, 100).expect("solver error");
    }
}
