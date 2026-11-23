//! 1D bracket search, sign-change root certification, and bump-size helpers.

use crate::CalibrationConfig;
use crate::constants::OBJECTIVE_VALID_ABS_MAX;

/// Finite-difference bump step used by both bootstrap and global diagnostics.
///
/// Uses the central-difference `ε^(1/3)` floor shared by the LM solver.
pub(crate) fn diagnostics_bump_h(config: &CalibrationConfig) -> f64 {
    let eps_cbrt = f64::EPSILON.cbrt();
    config.discount_curve.jacobian_step_size.max(eps_cbrt)
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
pub(super) fn opposite_signs(a: f64, b: f64) -> bool {
    (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0)
}

use finstack_quant_core::Result;
use finstack_quant_core::math::solver::BrentSolver;

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
pub(super) fn certify_root_by_local_sign_change(
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

/// Search the feasible side of each sampled validity boundary for a zero crossing.
///
/// A coarse grid can jump from a valid value over a root into an infeasible
/// region (for example, a base correlation that implies negative tranche loss).
/// Bisect only the sampled interval, retaining the valid endpoint. Invalid
/// values delimit the search; they never supply a residual sign or secant slope.
pub(super) fn refine_domain_boundaries(
    objective: &dyn Fn(f64) -> f64,
    valid_points: &mut Vec<(f64, f64)>,
    invalid_points: &[f64],
    diag: &mut BracketDiagnostics,
    initial: f64,
    tol: f64,
    max_iters: usize,
) {
    let mut observations: Vec<(f64, Option<f64>)> = valid_points
        .iter()
        .map(|&(x, value)| (x, Some(value)))
        .chain(invalid_points.iter().map(|&x| (x, None)))
        .collect();
    observations.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut boundaries: Vec<(f64, f64, f64)> = observations
        .windows(2)
        .filter_map(|pair| match (pair[0], pair[1]) {
            ((valid_x, Some(value)), (invalid_x, None))
            | ((invalid_x, None), (valid_x, Some(value))) => Some((valid_x, value, invalid_x)),
            _ => None,
        })
        .collect();
    boundaries.sort_by(|a, b| {
        let distance =
            |boundary: &(f64, f64, f64)| (0.5 * boundary.0 + 0.5 * boundary.2 - initial).abs();
        distance(a).total_cmp(&distance(b))
    });

    for (mut valid_x, mut valid_value, mut invalid_x) in boundaries {
        // At most 64 halvings resolve a sampled interval to floating-point
        // precision without an unbounded search near a pricing-domain edge.
        for _ in 0..max_iters.clamp(50, 64) {
            let midpoint = 0.5 * valid_x + 0.5 * invalid_x;
            if midpoint == valid_x || midpoint == invalid_x {
                break;
            }
            let value = objective(midpoint);
            diag.update(midpoint, value);
            if value.is_finite() && value.abs() < OBJECTIVE_VALID_ABS_MAX {
                valid_points.push((midpoint, value));
                if value.abs() < tol || opposite_signs(valid_value, value) {
                    return;
                }
                valid_x = midpoint;
                valid_value = value;
            } else {
                invalid_x = midpoint;
            }
        }
    }
}

/// Minimum scan-grid size required by the bracket solver.
///
/// The geometric bracket-expansion fallback was removed in favour of letting
/// callers own the scan grid (each `BootstrapTarget` builds a maturity- or
/// rate-aware grid that beats a one-size-fits-all expansion). A grid smaller
/// than this is rejected before any objective evaluations. A bounded domain
/// may contain fewer distinct floating-point values, so this must be a checked
/// error rather than a debug assertion.
pub(super) const MIN_SCAN_GRID_LEN: usize = 8;

/// Order in which the bracket solver evaluates its scan grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanStrategy {
    /// Evaluate every scan point and pick the sign-changing bracket closest to
    /// the initial guess. Safe for non-monotone objectives.
    Exhaustive,
    /// Evaluate scan points by distance from the initial guess and stop at the
    /// first sign-changing bracket. Valid only for objectives monotone in the
    /// solved knot; falls back to the exhaustive result when no bracket is found.
    NearestFirst,
}

/// Bracket and solve a 1-D root on a caller-supplied scan grid, returning the
/// root (if any) together with diagnostics for error reporting.
pub(crate) fn bracket_solve_1d(
    objective: &dyn Fn(f64) -> f64,
    initial: f64,
    scan_points: &[f64],
    tol: f64,
    max_iters: usize,
    scan_strategy: ScanStrategy,
) -> Result<(Option<f64>, BracketDiagnostics)> {
    // The adaptive geometric expansion previously embedded here was removed;
    // callers must now provide a grid dense enough to bracket the root. Return
    // a checked error when a narrow floating-point domain cannot supply it.
    if scan_points.len() < MIN_SCAN_GRID_LEN {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bracket_solve_1d: scan grid has {} points (< {}); \
             supply a grid that spans the feasible region",
            scan_points.len(),
            MIN_SCAN_GRID_LEN
        )));
    }

    let mut diag = BracketDiagnostics::new(scan_points);
    let mut valid_points: Vec<(f64, f64)> = Vec::with_capacity(scan_points.len() + 8);
    let mut invalid_points = Vec::new();

    let v0 = objective(initial);
    diag.update(initial, v0);
    // An exact root at the preferred initial point is conclusive; scanning
    // cannot improve its residual or distance from the requested guess.
    if v0 == 0.0 && tol > 0.0 {
        diag.bracket_found = true;
        diag.is_sign_change_bracket = true;
        return Ok((Some(initial), diag));
    }
    if v0.is_finite() && v0.abs() < OBJECTIVE_VALID_ABS_MAX {
        valid_points.push((initial, v0));
    } else {
        invalid_points.push(initial);
    }

    let mut nearest_first_points = Vec::new();
    let ordered_scan_points = if scan_strategy == ScanStrategy::NearestFirst {
        nearest_first_points.extend_from_slice(scan_points);
        nearest_first_points.sort_by(|a, b| {
            (a - initial)
                .abs()
                .total_cmp(&(b - initial).abs())
                .then_with(|| a.total_cmp(b))
        });
        nearest_first_points.as_slice()
    } else {
        scan_points
    };

    for &point in ordered_scan_points {
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
            invalid_points.push(point);
            continue;
        }
        valid_points.push((point, value));

        if scan_strategy == ScanStrategy::NearestFirst {
            // An exact zero is conclusive. Otherwise stop only after observing
            // a true crossing; a no-bracket case must still exhaust the grid so
            // diagnostics and best-effort behavior remain unchanged.
            if value == 0.0 {
                break;
            }
            valid_points.sort_by(|a, b| a.0.total_cmp(&b.0));
            if valid_points
                .windows(2)
                .any(|pair| opposite_signs(pair[0].1, pair[1].1))
            {
                break;
            }
        }
    }

    if valid_points.is_empty() {
        return Ok((None, diag));
    }

    valid_points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let needs_domain_refinement = !invalid_points.is_empty()
        && !valid_points.iter().any(|(_, value)| value.abs() < tol)
        && !valid_points
            .windows(2)
            .any(|pair| opposite_signs(pair[0].1, pair[1].1));
    if needs_domain_refinement {
        refine_domain_boundaries(
            objective,
            &mut valid_points,
            &invalid_points,
            &mut diag,
            initial,
            tol,
            max_iters,
        );
    }

    // Robust bracket selection:
    // sort by x and choose the bracket whose midpoint is closest to the initial guess.
    valid_points.sort_by(|a, b| a.0.total_cmp(&b.0));
    struct Bracket {
        left: (f64, f64),
        right: (f64, f64),
        score: f64,
    }
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
            Some(bracket) => score < bracket.score,
        };
        if replace {
            best_bracket = Some(Bracket {
                left: (x0, f0),
                right: (x1, f1),
                score,
            });
        }
    }

    // Early hit: the initial guess or a scan-grid point already satisfies the
    // f-space tolerance. Return the best such point. It counts as a sign-change
    // root only when the grid evidences a zero crossing (`sign_change_observed`);
    // otherwise it is a below-tolerance |f|-minimum and is flagged approximate
    // via `is_sign_change_bracket = false` for the caller's opt-in gate.
    if let (Some(best_point), Some(best_value)) = (diag.best_point, diag.best_value)
        && best_value.is_finite()
        && best_value.abs() < tol
    {
        diag.bracket_found = true;
        diag.is_sign_change_bracket = sign_change_observed
            || certify_root_by_local_sign_change(objective, best_point, best_value, &mut diag);
        return Ok((Some(best_point), diag));
    }

    let Some(Bracket {
        left: (mut a, mut fa),
        right: (mut b, mut fb),
        ..
    }) = best_bracket
    else {
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
    // opposite signs). Mark this before entering Brent/false-position so
    // callers can distinguish true bracketed roots from approximate |f|-minima.
    diag.is_sign_change_bracket = true;

    // The authoritative scan already paid for both endpoint values. Brent
    // uses safeguarded interpolation first, but may not accept an x-space stop
    // as convergence. Keep the same per-phase iteration budgets and retain
    // the last feasible bracket for the established Illinois recovery below.
    let (lo, hi) = (a, b);
    let init_a = if lo == 0.0 { 0.0 } else { lo };
    let init_b = if hi == 0.0 { 0.0 } else { hi };
    let mut cached_a = Some((init_a.to_bits(), fa));
    let mut cached_b = Some((init_b.to_bits(), fb));
    let mut brent_evals = vec![(init_a.to_bits(), fa), (init_b.to_bits(), fb)];
    let mut invalid_interior = None;
    let root = BrentSolver::new(tol.max(f64::MIN_POSITIVE), max_iters.max(50))?.solve_in_bracket(
        |x| {
            let x_bits = if x == 0.0 { 0.0_f64 } else { x }.to_bits();
            if cached_a.is_some_and(|(bits, _)| bits == x_bits) {
                return Ok(cached_a.take().map_or(fa, |(_, v)| v));
            }
            if cached_b.is_some_and(|(bits, _)| bits == x_bits) {
                return Ok(cached_b.take().map_or(fb, |(_, v)| v));
            }
            let value = objective(x);
            diag.update(x, value);
            if !value.is_finite() || value.abs() >= OBJECTIVE_VALID_ABS_MAX {
                invalid_interior = Some(x);
                return Err(finstack_quant_core::Error::Validation(
                    "invalid interior objective value".into(),
                ));
            }
            brent_evals.push((x_bits, value));
            if a < x && x < b {
                if opposite_signs(fa, value) {
                    b = x;
                    fb = value;
                } else if opposite_signs(value, fb) {
                    a = x;
                    fa = value;
                }
            }
            Ok(value)
        },
        lo,
        hi,
    );
    if let Ok(root) = root {
        let root_bits = if root == 0.0 { 0.0_f64 } else { root }.to_bits();
        let f_root = brent_evals
            .iter()
            .rev()
            .find(|&&(bits, _)| bits == root_bits)
            .map(|&(_, v)| v);
        if f_root.is_some_and(|v| v.is_finite() && v.abs() < tol) {
            diag.bracket_found = true;
            return Ok((Some(root), diag));
        }
    }

    let mut recovery_budget = max_iters.max(50);
    if let Some(invalid) = invalid_interior {
        // An infeasible interpolation does not imply that the midpoint is
        // infeasible. Try it once before Illinois recovery, charging the call
        // to the same existing recovery budget. Never install invalid values.
        let midpoint = 0.5 * (a + b);
        if midpoint.to_bits() != invalid.to_bits() {
            let value = objective(midpoint);
            diag.update(midpoint, value);
            recovery_budget -= 1;
            if value.is_finite() && value.abs() < tol {
                diag.bracket_found = true;
                return Ok((Some(midpoint), diag));
            }
            if value.is_finite() && value.abs() < OBJECTIVE_VALID_ABS_MAX {
                if opposite_signs(fa, value) {
                    b = midpoint;
                    fb = value;
                } else if opposite_signs(value, fb) {
                    a = midpoint;
                    fa = value;
                }
            }
        }
    }

    // If we already met tolerance, return the best observed point.
    if let (Some(best_point), Some(best_value)) = (diag.best_point, diag.best_value)
        && best_value.is_finite()
        && best_value.abs() < tol
    {
        diag.bracket_found = true;
        return Ok((Some(best_point), diag));
    }

    // A failed interpolation or an x-space stop leaves the last finite
    // sign-changing bracket intact. Preserve bounded Illinois recovery for
    // penalized interiors and residual-space polishing.
    let bracket_valid = opposite_signs(fa, fb) && a < b;
    if !bracket_valid {
        return Ok((diag.best_point, diag));
    }

    // Illinois false position halves a repeatedly retained endpoint.
    // `retained`: -1 = a, +1 = b.
    let mut retained: i8 = 0;
    for _ in 0..recovery_budget {
        let denom = fb - fa;
        let mut candidate = if denom.is_finite() && denom.abs() > f64::EPSILON {
            a - fa * (b - a) / denom
        } else {
            0.5 * a + 0.5 * b
        };
        if !candidate.is_finite() || candidate <= a || candidate >= b {
            candidate = 0.5 * a + 0.5 * b;
            if !candidate.is_finite() || candidate <= a || candidate >= b {
                break;
            }
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
            if retained == -1 {
                fa *= 0.5;
            }
            retained = -1;
        } else if opposite_signs(fc, fb) {
            a = candidate;
            fa = fc;
            if retained == 1 {
                fb *= 0.5;
            }
            retained = 1;
        } else {
            break;
        }
    }

    Ok((None, diag))
}
