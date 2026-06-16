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

/// Return `true` iff `a` and `b` straddle zero, i.e. one is strictly positive and the
/// other strictly negative.
///
/// This intentionally replaces `a.signum() != b.signum()` for sign-change detection.
/// `f64::signum` is the wrong tool here:
///   * `signum(+0.0) = +1.0` and `signum(-0.0) = -1.0`, so `signum` reports a "sign
///     change" between `+0.0` and `-0.0` (both are roots, not a bracket) and reports
///     "same sign" between an exact `0.0` root and a positive value (hiding the root).
///   * `signum(NaN) = NaN`, and `NaN != NaN`, so a non-finite objective value would be
///     mistaken for a sign change and drive bisection/false-position on a bogus bracket.
///
/// With explicit `> 0.0` / `< 0.0` comparisons a zero or NaN endpoint yields `false`
/// (no bracket), which is the safe answer — the caller's separate `|f| < tol` and
/// finite-value checks handle exact roots and infeasible evaluations.
#[inline]
fn opposite_signs(a: f64, b: f64) -> bool {
    (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0)
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
use finstack_quant_core::Result;

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

/// Certify that a below-tolerance candidate `x` is a genuine zero *crossing*
/// by probing the objective at `x ± δ` (clamped to the scan bounds).
///
/// A converged candidate can legitimately lack sign-change evidence: a secant
/// iteration converging monotonically from one side of a simple root never
/// produces opposite-sign iterates, and a grid point landing below tolerance
/// may sit between two same-sign scan neighbours. For a simple root the
/// objective straddles zero across `x`; for the W-40 defect case — a tangent
/// `|f|`-minimum that satisfies the tolerance without crossing — both probes
/// respond with the same sign and certification fails, leaving the candidate
/// flagged approximate for the caller's opt-in gate.
///
/// The probe escalates δ by decades because real bootstrap objectives can be
/// locally *flat* around a machine-precision root (curve construction and
/// repricing quantize below ~1e-7 in knot space), which would make a single
/// fixed-δ probe read `f(x ± δ) == f(x)` and mis-flag a genuine root. δ stops
/// escalating once both probes respond (`f != f(x)`): at that scale the sign
/// pattern is conclusive.
///
/// Costs a handful of extra objective evaluations, and only on the
/// no-sign-change-evidence paths. Returns `false` (uncertified) when the
/// probes collapse onto `x` (candidate at a scan bound), never respond, or
/// respond without straddling zero.
fn certify_root_by_local_sign_change(
    objective: &dyn Fn(f64) -> f64,
    x: f64,
    fx: f64,
    diag: &mut BracketDiagnostics,
) -> bool {
    let (lo, hi) = diag.scan_bounds;
    let mut delta = (1e-6 * x.abs()).max(1e-9);
    let max_delta = 0.5 * (hi - lo).abs();
    for _ in 0..8 {
        if delta > max_delta {
            return false;
        }
        let x_lo = (x - delta).max(lo);
        let x_hi = (x + delta).min(hi);
        if x_lo >= x || x_hi <= x {
            return false;
        }
        let f_lo = objective(x_lo);
        diag.update(x_lo, f_lo);
        let f_hi = objective(x_hi);
        diag.update(x_hi, f_hi);
        if opposite_signs(f_lo, f_hi) {
            return true;
        }
        // Both sides responded at this scale and still share a sign: the
        // candidate is a non-crossing |f|-minimum, not a root. "Responded"
        // means an exact (bitwise) change — any movement off the quantized
        // plateau counts.
        if f_lo.is_finite()
            && f_hi.is_finite()
            && f_lo.to_bits() != fx.to_bits()
            && f_hi.to_bits() != fx.to_bits()
        {
            return false;
        }
        delta *= 10.0;
    }
    false
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
    if v0.is_finite() && v0.abs() < OBJECTIVE_VALID_ABS_MAX {
        valid_points.push((initial, v0));
    }

    for &point in scan_points {
        // The initial guess is typically also a scan-grid point (callers seed
        // the grid with it); reuse the already-computed f(initial) instead of
        // re-pricing — one objective evaluation (e.g. a full CDS repricing)
        // saved per pillar. Tolerance matches the grid dedup in
        // `normalize_scan_points`.
        if (point - initial).abs() < 1e-12 {
            continue;
        }
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
    // Whether ANY adjacent pair of valid evaluations straddles zero — i.e. the
    // objective provably crosses zero somewhere in the scanned domain. This is
    // the evidence required to treat a below-tolerance candidate as a genuine
    // root rather than a positive (or negative) |f|-minimum that never crosses.
    // An exact `f == 0.0` evaluation is itself conclusive root evidence
    // (`opposite_signs` deliberately excludes zeros from straddle detection).
    let mut sign_change_observed = valid_points.iter().any(|&(_, f)| f == 0.0);
    for w in valid_points.windows(2) {
        let (x0, f0) = w[0];
        let (x1, f1) = w[1];
        if !opposite_signs(f0, f1) {
            continue;
        }
        sign_change_observed = true;
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

    // Early hit: the initial guess or a scan-grid point already satisfies the
    // f-space tolerance. Return the best such point. It counts as a sign-change
    // root only when the grid evidences a zero crossing (`sign_change_observed`);
    // otherwise it is a below-tolerance |f|-minimum and is flagged approximate
    // via `is_sign_change_bracket = false` for the caller's opt-in gate.
    if let (Some(best_point), Some(best_value)) = (diag.best_point, diag.best_value) {
        if best_value.is_finite() && best_value.abs() < tol {
            diag.bracket_found = true;
            diag.is_sign_change_bracket = sign_change_observed
                || certify_root_by_local_sign_change(objective, best_point, best_value, &mut diag);
            return Ok((Some(best_point), diag));
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

            // Whether any pair of secant iterates straddles zero. If so, the
            // objective provably crosses zero between them and a converged
            // candidate is a genuine sign-change root (the bracket simply was
            // not in the scan grid).
            let mut secant_crossed = false;
            for _ in 0..iters {
                if fx.is_finite() && fx.abs() < tol {
                    diag.bracket_found = true;
                    diag.is_sign_change_bracket = secant_crossed
                        || certify_root_by_local_sign_change(objective, x, fx, &mut diag);
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
                if f_next.is_finite() && opposite_signs(fx, f_next) {
                    secant_crossed = true;
                }

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
    // for well-behaved monotone objectives. If midpoints become invalid/penalized
    // or bisection runs out of iterations, we fall back to bounded false-position
    // updates inside the last-known-good bracket (below).
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

        if opposite_signs(fa, fm) {
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
    // invariant "fa and fb straddle zero" should hold; guard explicitly so that an
    // edge case (e.g. fa or fb became NaN upstream) cannot drive FP on a stale bracket.
    // `opposite_signs` already implies both are finite and non-zero.
    let bracket_valid = opposite_signs(fa, fb) && a < b;
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

        if opposite_signs(fa, fc) {
            b = candidate;
            fb = fc;
        } else if opposite_signs(fc, fb) {
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

    /// The initial guess is also pushed into the scan grid by
    /// `normalize_scan_points`; the solver must reuse f(initial) instead of
    /// re-evaluating the duplicate grid point (one repricing saved per pillar).
    #[test]
    fn initial_guess_not_evaluated_twice() {
        use std::cell::RefCell;

        let evals = RefCell::new(Vec::new());
        let f = |x: f64| {
            evals.borrow_mut().push(x);
            x - 0.5
        };
        let initial = 0.3;
        // Grid contains the initial guess exactly, as normalize_scan_points produces.
        let scan = dense_scan(0.0, 1.0, &[initial]);
        let (root, _) =
            bracket_solve_1d_with_diagnostics(&f, initial, &scan, 1e-12, 100).expect("solver");
        assert!(root.is_some());

        let initial_evals = evals
            .borrow()
            .iter()
            .filter(|&&x| (x - initial).abs() < 1e-12)
            .count();
        assert_eq!(
            initial_evals, 1,
            "f(initial) must be evaluated exactly once, not re-priced as a scan point"
        );
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

    /// Item 8: `opposite_signs` must use explicit `> 0` / `< 0` comparisons, NOT
    /// `f64::signum`. Verifies the three pathological inputs `signum` mishandles.
    #[test]
    fn opposite_signs_handles_signed_zero_and_nan() {
        // Genuine sign change.
        assert!(opposite_signs(1.0, -1.0));
        assert!(opposite_signs(-2.5, 0.3));
        // Same sign.
        assert!(!opposite_signs(1.0, 2.0));
        assert!(!opposite_signs(-1.0, -2.0));

        // Signed zeros: `signum(+0.0)=+1`, `signum(-0.0)=-1` would WRONGLY report a
        // sign change here. A zero is a root, not a bracket boundary.
        assert!(!opposite_signs(0.0, -0.0));
        assert!(!opposite_signs(-0.0, 0.0));
        // A zero against a finite value is NOT a straddle (the zero is the root).
        assert!(!opposite_signs(0.0, 5.0));
        assert!(!opposite_signs(-0.0, 5.0));
        assert!(!opposite_signs(0.0, -5.0));

        // NaN: `signum(NaN)=NaN` and `NaN != NaN` would WRONGLY report a sign change.
        assert!(!opposite_signs(f64::NAN, 1.0));
        assert!(!opposite_signs(1.0, f64::NAN));
        assert!(!opposite_signs(f64::NAN, f64::NAN));
        assert!(!opposite_signs(f64::NAN, -0.0));
    }

    /// Item 8 (regression): an objective that is identically zero (emitting `-0.0`
    /// then `+0.0` across the grid) must NOT be mistaken for a sign-changing bracket.
    /// Under the old `signum`-based check, a `[-0.0, +0.0]` window reported
    /// `signum(-0.0)=-1 != signum(+0.0)=+1` → a spurious bracket → `false-position` /
    /// bisection driven on a non-root, with `is_sign_change_bracket` wrongly `true`.
    ///
    /// We use `tol = 0.0` so the early `|f| < tol` root-return is bypassed and the
    /// windows-scan sign-change logic is genuinely exercised on the signed zeros.
    #[test]
    fn bracket_solver_no_spurious_bracket_from_signed_zero() {
        // f(x) ≡ 0 but with a sign bit flip at x = 0.5.
        let f = |x: f64| if x < 0.5 { -0.0_f64 } else { 0.0_f64 };
        let scan = dense_scan(0.0, 1.0, &[0.25, 0.5, 0.75]);
        let (_root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.3, &scan, 0.0, 100).expect("solver error");
        assert!(
            !diag.is_sign_change_bracket,
            "an identically-zero objective has NO sign-change bracket; \
             signed-zero (-0.0 vs +0.0) must not be mistaken for a straddle"
        );
    }

    /// A genuinely converged root with no grid/iterate sign-change evidence
    /// (valid scan points all one-sided because the other side is penalized)
    /// must be certified exact via the local ±δ sign-change probe — not left
    /// flagged approximate, which would hard-fail strict targets through the
    /// `allow_approximate_knots` gate despite a machine-precision residual.
    #[test]
    fn converged_root_without_grid_straddle_is_certified_exact() {
        // Root at 0.5; everything below 0.4995 is penalized, so all *valid*
        // grid points sit on the positive side — no straddle in the scan.
        let f = |x: f64| if x < 0.4995 { PENALTY } else { x - 0.5 };
        let scan = vec![0.499, 0.55, 0.7, 0.85, 1.0, 1.15, 1.3, 1.45, 1.6, 2.0];
        // Initial guess is already at the root to machine precision (the
        // re-bootstrap-under-bump pattern, e.g. dv01 curve rebuilds).
        let initial = 0.5 + 1e-14;
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, initial, &scan, 1e-12, 100).expect("solver");
        let r = root.expect("converged root must be returned");
        assert!((r - 0.5).abs() < 1e-9, "unexpected root {r}");
        assert!(
            diag.is_sign_change_bracket,
            "a converged candidate certified by the local sign-change probe \
             must be reported as an exact root, not an approximate knot"
        );
    }

    /// A locally *quantized* objective (flat plateaus around the root, the
    /// dv01 re-bootstrap pattern observed on deposit pillars) must still be
    /// certified: the probe escalates δ until the objective responds.
    #[test]
    fn quantized_flat_objective_root_is_certified_via_probe_escalation() {
        // f is x − 0.5 quantized to 1e-4 plateaus, offset so the plateau value
        // at the root is a tiny non-zero residual (avoids the f == 0.0
        // conclusive-root shortcut and forces the certification probe).
        let q = 1e-4;
        let f = move |x: f64| {
            if x < 0.499 {
                PENALTY
            } else {
                ((x - 0.5) / q).round() * q + 3.7e-13
            }
        };
        // Valid grid points all on the positive side (one-sided evidence);
        // 0.45 is penalized, so it widens the scan bounds without providing
        // sign-change evidence.
        let scan = vec![0.45, 0.55, 0.7, 0.85, 1.0, 1.15, 1.3, 1.45, 1.6, 2.0];
        let initial = 0.5 + 1e-14;
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, initial, &scan, 1e-12, 100).expect("solver");
        assert!(root.is_some(), "below-tolerance plateau root returned");
        assert!(
            diag.is_sign_change_bracket,
            "probe escalation must certify a quantized-flat genuine root"
        );
    }

    /// The W-40 defect case — a tangent |f|-minimum satisfying the tolerance
    /// without crossing zero — must NOT be certified by the local probe.
    #[test]
    fn tangent_minimum_below_tolerance_stays_approximate() {
        let f = |x: f64| (x - 0.5) * (x - 0.5) + 1e-9;
        let scan = dense_scan(0.0, 1.0, &[0.5]);
        let (root, diag) =
            bracket_solve_1d_with_diagnostics(&f, 0.5, &scan, 1e-3, 100).expect("solver");
        assert!(root.is_some(), "below-tolerance minimum is still returned");
        assert!(
            !diag.is_sign_change_bracket,
            "a non-crossing |f|-minimum must remain flagged approximate"
        );
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
