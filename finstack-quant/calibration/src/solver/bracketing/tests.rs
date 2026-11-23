use super::bracket::*;
use crate::constants::PENALTY;

mod unit_tests {
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
        let (root, _) = bracket_solve_1d(&f, 0.0, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");
        let r = root.expect("root should be Some");
        assert!((r - 0.5).abs() < 1e-9, "root inaccurate: {}", r);
    }

    #[test]
    fn test_bracket_diagnostics_tracking() {
        // f(x) = x - 0.5 has root at 0.5
        let f = |x: f64| x - 0.5;
        let scan = dense_scan(0.0, 1.0, &[0.5]);
        let (root, diag) = bracket_solve_1d(&f, 0.3, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");

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
        let (root, diag) = bracket_solve_1d(&f, 0.49, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");

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
    fn nearest_first_scan_stops_after_local_bracket() {
        use std::cell::Cell;

        let evals = Cell::new(0_usize);
        let f = |x: f64| {
            evals.set(evals.get() + 1);
            x - 0.5
        };
        let scan = dense_scan(-10.0, 10.0, &[0.49, 0.50, 0.51]);
        let (root, diag) =
            bracket_solve_1d(&f, 0.49, &scan, 1e-12, 100, ScanStrategy::NearestFirst)
                .expect("solver error");

        let root = root.expect("root should be found");
        assert!((root - 0.5).abs() < 1e-12);
        assert!(diag.is_sign_change_bracket);
        assert!(
            diag.eval_count < scan.len(),
            "nearest-first scan evaluated the full grid: {} evaluations for {} points",
            diag.eval_count,
            scan.len()
        );
        assert_eq!(diag.eval_count, evals.get());
    }

    #[test]
    fn nearest_first_no_bracket_still_exhausts_scan() {
        let f = |x: f64| x * x + 1.0;
        let scan = dense_scan(-2.0, 2.0, &[0.0]);
        let (root, diag) = bracket_solve_1d(&f, 0.0, &scan, 1e-12, 100, ScanStrategy::NearestFirst)
            .expect("solver error");

        assert!(root.is_none());
        assert!(
            diag.eval_count >= scan.len(),
            "no-bracket path must evaluate the authoritative grid"
        );
    }

    #[test]
    fn bracket_solver_no_silent_wrong_root_on_sparse_grid() {
        // S1: a sparse grid that *just barely* fails to bracket the root must
        // not silently return a wrong answer via the Newton fallback. Either
        // we converge near the true root (good) or return None (good); we must
        // never return a value far from the actual root.
        // Use the minimum accepted grid size to exercise the sparsest
        // supported search in both debug and release builds.
        let f = |x: f64| x.powi(3) - 2.0 * x + 1.0; // roots at 1, ~0.618, ~-1.618
        // Points that bracket the root at x=1 only via Newton-fallback secant.
        let scan: Vec<f64> = (0..8).map(|i| -2.0 + 0.5 * (i as f64)).collect();
        // = [-2, -1.5, -1, -0.5, 0, 0.5, 1, 1.5]
        let (root, _diag) = bracket_solve_1d(&f, 0.95, &scan, 1e-9, 100, ScanStrategy::Exhaustive)
            .expect("solver error");
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
        let (root, _) = bracket_solve_1d(&f, initial, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver");
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
    fn bracket_solver_refines_feasible_side_before_an_invalid_scan_node() {
        // The root lies in a narrow valid interval beyond the only feasible
        // scan node. A secant from x=0.25 overshoots into the invalid region.
        let scan = [0.25, 0.375, 0.5, 0.6, 0.7, 0.8, 0.9, 0.999];
        for mirrored in [false, true] {
            let objective = |x: f64| {
                let coordinate = if mirrored { 1.0 - x } else { x };
                if !(0.25..=0.351).contains(&coordinate) {
                    PENALTY
                } else {
                    (coordinate - 0.35) * (1.0 + coordinate)
                }
            };
            let scan: Vec<f64> = scan
                .iter()
                .map(|&x| if mirrored { 1.0 - x } else { x })
                .collect();
            let initial = if mirrored { 0.75 } else { 0.25 };
            for strategy in [ScanStrategy::Exhaustive, ScanStrategy::NearestFirst] {
                let (root, diagnostics) =
                    bracket_solve_1d(&objective, initial, &scan, 1e-12, 100, strategy)
                        .expect("bounded domain refinement");
                let expected = if mirrored { 0.65 } else { 0.35 };
                assert!((root.expect("reachable root") - expected).abs() < 1e-10);
                assert!(diagnostics.is_sign_change_bracket);
            }
        }
    }

    #[test]
    fn bracket_solver_does_not_treat_an_invalid_boundary_as_a_zero_crossing() {
        let scan = [0.25, 0.375, 0.5, 0.6, 0.7, 0.8, 0.9, 0.999];
        let objective = |x: f64| {
            if (0.25..=0.351).contains(&x) {
                0.4 - x
            } else {
                PENALTY
            }
        };
        let (root, diagnostics) = bracket_solve_1d(
            &objective,
            0.25,
            &scan,
            1e-12,
            100,
            ScanStrategy::NearestFirst,
        )
        .expect("bounded search");
        assert!(root.is_none());
        assert!(!diagnostics.is_sign_change_bracket);
        assert!(diagnostics.best_value.expect("valid observations") >= 0.049 - 1e-15);
    }

    #[test]
    fn test_bracket_diagnostics_no_bracket() {
        // f(x) = x^2 + 1 has no real root
        let f = |x: f64| x * x + 1.0;
        let scan = dense_scan(0.0, 2.0, &[0.5, 1.0, 1.5]);
        let (root, diag) = bracket_solve_1d(&f, 1.0, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");

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
        let (root, diag) = bracket_solve_1d(&f, 0.5, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");

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
        let (root, diag) = bracket_solve_1d(&f, 0.8, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect("solver error");

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
        let (_root, diag) = bracket_solve_1d(&f, 0.3, &scan, 0.0, 100, ScanStrategy::Exhaustive)
            .expect("solver error");
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
            bracket_solve_1d(&f, initial, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
                .expect("solver");
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
            bracket_solve_1d(&f, initial, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
                .expect("solver");
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
            bracket_solve_1d(&f, 0.5, &scan, 1e-3, 100, ScanStrategy::Exhaustive).expect("solver");
        assert!(root.is_some(), "below-tolerance minimum is still returned");
        assert!(
            !diag.is_sign_change_bracket,
            "a non-crossing |f|-minimum must remain flagged approximate"
        );
    }

    #[test]
    fn sparse_scan_grid_returns_error_before_pricing() {
        let calls = std::cell::Cell::new(0);
        let f = |x: f64| {
            calls.set(calls.get() + 1);
            x - 0.5
        };
        let scan = [0.0, 1.0]; // only 2 points
        let error = bracket_solve_1d(&f, 0.5, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
            .expect_err("insufficient scan grid must return a checked error");
        assert!(error.to_string().contains("scan grid has 2 points"));
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn bracket_solver_never_accepts_width_without_residual() {
        let objective = |x: f64| if x < 0.37123 { -1.0 } else { 1.0 };
        let scan = dense_scan(0.0, 1.0, &[]);
        let (root, diag) =
            bracket_solve_1d(&objective, 0.5, &scan, 1e-12, 100, ScanStrategy::Exhaustive)
                .expect("diagnostics");
        assert!(root.is_none());
        assert!(diag.is_sign_change_bracket);
        assert!(!diag.bracket_found);
        assert_eq!(diag.best_value.expect("finite values").abs(), 1.0);
    }

    #[test]
    fn bracket_solver_preserves_feasible_bracket_after_invalid_interpolation() {
        let calls = std::cell::Cell::new(0);
        let invalid = std::cell::Cell::new(0);
        let objective = |x: f64| {
            calls.set(calls.get() + 1);
            // A rejected interpolation must never replace a finite endpoint.
            if (0.397..0.3985).contains(&x) {
                invalid.set(invalid.get() + 1);
                PENALTY
            } else {
                x * x - 0.16
            }
        };
        let scan = dense_scan(0.0, 1.0, &[0.35, 0.45]);
        let (root, diag) = bracket_solve_1d(
            &objective,
            0.48,
            &scan,
            1e-12,
            100,
            ScanStrategy::Exhaustive,
        )
        .expect("solve");
        let root = root.expect("feasible root");
        assert!((root * root - 0.16).abs() < 1e-12);
        assert!(diag.is_sign_change_bracket);
        assert_eq!(calls.get(), diag.eval_count);
        assert!(
            invalid.get() > 0,
            "must exercise invalid interpolation recovery"
        );
    }
}

mod production_credit_solver_audit {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn production_credit_audit_exact_initial_root_needs_one_evaluation() {
        let calls = Cell::new(0);
        let objective = |x: f64| {
            calls.set(calls.get() + 1);
            x - 6.0
        };
        let (root, diagnostics) = bracket_solve_1d(
            &objective,
            6.0,
            &[2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0],
            1e-10,
            100,
            ScanStrategy::Exhaustive,
        )
        .expect("exact initial root");
        assert_eq!(root, Some(6.0));
        assert!(diagnostics.is_sign_change_bracket);
        assert_eq!(calls.get(), 1, "an exact initial root requires no scan");
    }
}
