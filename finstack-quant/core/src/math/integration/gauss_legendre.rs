//! Gauss-Legendre composite and adaptive quadrature.

use super::adaptive::*;

use crate::Error;
use crate::math::summation::NeumaierAccumulator;

/// Return Gauss–Legendre nodes and weights for supported orders.
pub(super) fn gl_nodes_weights(order: usize) -> Result<(&'static [f64], &'static [f64]), Error> {
    // Nodes/weights for symmetric [-1,1] intervals
    // Orders supported: 2, 4, 8, 16
    match order {
        2 => Ok((
            &[-0.577_350_269_189_625_7, 0.577_350_269_189_625_7],
            &[1.0, 1.0],
        )),
        4 => Ok((
            &[
                -0.861_136_311_594_052_6,
                -0.339_981_043_584_856_3,
                0.339_981_043_584_856_3,
                0.861_136_311_594_052_6,
            ],
            &[
                0.347_854_845_137_453_85,
                0.652_145_154_862_546_1,
                0.652_145_154_862_546_1,
                0.347_854_845_137_453_85,
            ],
        )),
        8 => Ok((
            &[
                -0.960_289_856_497_536_3,
                -0.796_666_477_413_626_7,
                -0.525_532_409_916_329,
                -0.183_434_642_495_649_8,
                0.183_434_642_495_649_8,
                0.525_532_409_916_329,
                0.796_666_477_413_626_7,
                0.960_289_856_497_536_3,
            ],
            &[
                0.101_228_536_290_376_26,
                0.222_381_034_453_374_48,
                0.313_706_645_877_887_27,
                0.362_683_783_378_361_96,
                0.362_683_783_378_361_96,
                0.313_706_645_877_887_27,
                0.222_381_034_453_374_48,
                0.101_228_536_290_376_26,
            ],
        )),
        16 => Ok((
            &[
                -0.989_400_934_991_649_9,
                -0.944_575_023_073_232_6,
                -0.865_631_202_387_831_8,
                -0.755_404_408_355_003,
                -0.617_876_244_402_643_8,
                -0.458_016_777_657_227_37,
                -0.281_603_550_779_258_9,
                -0.095_012_509_837_637_44,
                0.095_012_509_837_637_44,
                0.281_603_550_779_258_9,
                0.458_016_777_657_227_37,
                0.617_876_244_402_643_8,
                0.755_404_408_355_003,
                0.865_631_202_387_831_8,
                0.944_575_023_073_232_6,
                0.989_400_934_991_649_9,
            ],
            &[
                0.027_152_459_411_754_095,
                0.062_253_523_938_647_894,
                0.095_158_511_682_492_78,
                0.124_628_971_255_533_88,
                0.149_595_988_816_576_73,
                0.169_156_519_395_002_54,
                0.182_603_415_044_923_58,
                0.189_450_610_455_068_5,
                0.189_450_610_455_068_5,
                0.182_603_415_044_923_58,
                0.169_156_519_395_002_54,
                0.149_595_988_816_576_73,
                0.124_628_971_255_533_88,
                0.095_158_511_682_492_78,
                0.062_253_523_938_647_894,
                0.027_152_459_411_754_095,
            ],
        )),
        _ => Err(crate::Error::Validation(
            "unsupported Gauss-Legendre order; supported orders are 2, 4, 8 and 16".to_string(),
        )),
    }
}

/// One Gauss–Legendre panel on the increasing interval `[a, b]`.
///
/// Maps the stored weight-one rule on `[-1, 1]` by a stable midpoint and
/// half-width and returns `half * sum(w_i * f(x_i))`. Every node costs one
/// counted evaluation.
pub(super) fn gauss_legendre_panel<F>(
    work: &mut QuadratureWork,
    f: &mut F,
    a: f64,
    b: f64,
    rule: (&[f64], &[f64]),
) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<[f64; 1], Error>,
{
    let (nodes, weights) = rule;
    let half = half_width(a, b);
    let mid = midpoint(a, b);
    let mut sum = NeumaierAccumulator::new();
    let mut previous = a;
    for (&node, &weight) in nodes.iter().zip(weights) {
        let x = mid + half * node;
        if !(previous < x && x < b) {
            return Err(work.failure(
                f64::NAN,
                x,
                "nodes are not distinct inside their panel in binary64",
            ));
        }
        previous = x;
        let [sample] = work.sample(f, x)?;
        let term = weight * sample;
        if sample != 0.0 && term == 0.0 {
            return Err(work.failure(f64::NAN, x, "a nonzero weighted sample underflowed to zero"));
        }
        sum.add(term)
            .map_err(|_| work.failure(f64::NAN, x, LEGENDRE_NOT_REPRESENTABLE))?;
    }
    let sum = sum
        .get_total()
        .map_err(|_| work.failure(f64::NAN, mid, LEGENDRE_NOT_REPRESENTABLE))?;
    let value = sum * half;
    if !value.is_finite() || (sum != 0.0 && value == 0.0) {
        return Err(work.failure(f64::NAN, mid, LEGENDRE_NOT_REPRESENTABLE));
    }
    Ok(value)
}

pub(super) const LEGENDRE_NOT_REPRESENTABLE: &str = "panel estimate is not representable";

/// Build a composite Gauss-Legendre grid over a finite interval.
///
/// Each returned pair contains an integration abscissa and its fully scaled
/// weight. Summing `weight * f(abscissa)` therefore evaluates the composite
/// rule without a second panel-width factor.
///
/// # Arguments
///
/// * `a` - Finite lower integration bound.
/// * `b` - Finite upper integration bound, strictly greater than `a`.
/// * `order` - Nodes per panel; supported values are 2, 4, 8, and 16.
/// * `panels` - Positive number of equal-width panels.
///
/// # Errors
///
/// Returns [`Error::Validation`] when either bound is non-finite, the
/// interval is empty or reversed, `panels` is zero, or `order` is unsupported.
pub fn gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Result<Vec<(f64, f64)>, Error> {
    if panels == 0 || !(a.is_finite() && b.is_finite()) || b <= a {
        return Err(crate::Error::Validation(
            "Gauss-Legendre grid needs finite bounds with b > a and at least one panel".to_string(),
        ));
    }
    let (nodes, weights) = gl_nodes_weights(order)?;
    let count = order
        .checked_mul(panels)
        .filter(|_| panels as u128 <= 1u128 << 53)
        .ok_or_else(|| Error::Validation("Legendre grid count exceeds capacity".into()))?;
    let mut grid = Vec::new();
    grid.try_reserve_exact(count)
        .map_err(|e| Error::Validation(format!("Legendre grid allocation: {e}")))?;
    let mut left = a;
    for panel in 0..panels {
        let right = interval_point(a, b, (panel + 1) as f64 / panels as f64);
        let half = half_width(left, right);
        let mid = midpoint(left, right);
        let mut previous = left;
        for (&node, &w) in nodes.iter().zip(weights) {
            let point = half.mul_add(node, mid);
            let weight = half * w;
            if !(point > previous && point < right && weight > 0.0 && weight.is_finite()) {
                return Err(Error::Internal(
                    "unrepresentable Legendre node or weight".into(),
                ));
            }
            grid.push((point, weight));
            previous = point;
        }
        left = right;
    }
    Ok(grid)
}

/// Composite Gauss–Legendre rule on `panels` equal sub-intervals.
///
/// Applies the `order`-point rule to each panel and sums the scaled weighted
/// samples once with a compensated total. More panels do not raise the rule's
/// polynomial degree; no accuracy claim is made for arbitrary integrands.
///
/// # Arguments
///
/// * `f` - Fallible integrand, evaluated once at each of the `order * panels`
///   nodes in increasing coordinate order. It must describe one deterministic
///   function for the whole call. The first error it returns stops the rule
///   and is returned unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` evaluates the same
///   increasing-bound rule and negates the result; `b == a` returns `+0.0`
///   without calling `f`.
/// * `order` - Nodes per panel; supported values are 2, 4, 8 and 16.
/// * `panels` - Positive whole number of equal-width panels, at most `2^53`.
///
/// # Returns
///
/// The finite composite estimate of the signed integral of `f` from `a` to `b`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, an unsupported
/// `order`, or a `panels` that is zero, above `2^53`, or whose node count
/// overflows. Returns the callback's own error unchanged. Returns
/// [`InputError::SolverConvergenceFailed`](crate::InputError::SolverConvergenceFailed) (a computation error) when a sample
/// is NaN or infinite, when a nonzero weighted sample underflows to zero, or
/// when the sum is not representable; `iterations` is the number of callbacks
/// made and `last_x` the coordinate concerned.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::gauss_legendre_integrate_composite;
///
/// let integral = gauss_legendre_integrate_composite(|x| Ok(x * x), 0.0, 1.0, 4, 2)?;
/// assert!((integral - 1.0 / 3.0).abs() < 1e-14);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn gauss_legendre_integrate_composite<F>(
    mut f: F,
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<f64, Error>,
{
    const RULE: &str = "gauss_legendre_integrate_composite";
    if !a.is_finite() || !b.is_finite() {
        return Err(Error::Validation(format!(
            "{RULE}: endpoints must be finite, got [{a}, {b}]"
        )));
    }
    gl_nodes_weights(order)?;
    if panels == 0 || panels as u64 > MAX_EXACT_COUNT || order.checked_mul(panels).is_none() {
        return Err(Error::Validation(format!(
            "{RULE}: panels must be a positive count that converts to f64 exactly, got {panels}"
        )));
    }
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok(0.0);
    }
    let (lo, hi, negate) = if a < b { (a, b, false) } else { (b, a, true) };
    let grid = gauss_legendre_grid(lo, hi, order, panels)?;
    let mut total = NeumaierAccumulator::new();
    for (index, (x, weight)) in grid.into_iter().enumerate() {
        let sample = f(x)?;
        let made = index + 1;
        if !sample.is_finite() {
            return Err(quadrature_failure(
                RULE,
                made,
                f64::NAN,
                x,
                "integrand was not finite",
            ));
        }
        let term = weight * sample;
        if sample != 0.0 && term == 0.0 {
            return Err(quadrature_failure(
                RULE,
                made,
                f64::NAN,
                x,
                "a nonzero weighted sample underflowed to zero",
            ));
        }
        total.add(term).map_err(|_| {
            quadrature_failure(RULE, made, f64::NAN, x, "weighted sum is not representable")
        })?;
    }
    let value = total.get_total().map_err(|_| {
        quadrature_failure(
            RULE,
            order * panels,
            f64::NAN,
            hi,
            "weighted sum is not representable",
        )
    })?;
    Ok(oriented(value, negate))
}

/// One pending adaptive Gauss–Legendre panel with its known one-panel value.
pub(super) struct LegendrePanel {
    a: f64,
    b: f64,
    coarse: f64,
    tol: f64,
    depth: usize,
}

/// Adaptive Gauss–Legendre quadrature with an absolute global error budget.
///
/// Each panel compares its one-panel value `Q1` with the sum `Q2` of the same
/// rule on its two halves, uses `e = |Q2 - Q1|` as the error estimate and
/// keeps `Q2`. A panel whose estimate exceeds its share of the budget is
/// bisected, the left half receiving `tol / 2` and the right half the
/// remainder, until `max_depth`; each child reuses the half value its parent
/// already computed. Terminal values and estimates are summed with compensated
/// totals and the call succeeds only when the summed estimate is at most `tol`.
/// The root costs `3 * order` evaluations and every bisection `2 * order` more.
///
/// The estimate is an operational difference, not a Kronrod rule or a bound on
/// the true error.
///
/// # Arguments
///
/// * `f` - Fallible integrand. It must describe one deterministic function for
///   the whole call; mutable diagnostics are allowed. The first error it
///   returns stops the run and is returned unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` runs the same
///   increasing-bound refinement and negates the result; `b == a` returns
///   `+0.0` without calling `f`.
/// * `order` - Nodes per panel; supported values are 2, 4, 8 and 16.
/// * `tol` - Finite, nonnegative absolute budget for the sum of all terminal
///   error estimates, in integral units. Zero is allowed and requests a zero
///   computed estimate, not exact integration.
/// * `max_depth` - Deepest bisection level, with the root panel at depth zero.
///   Depth zero still performs the root comparison. A depth-limited panel may
///   exceed its local share if the summed estimate still fits `tol`.
/// * `max_evals` - Positive hard limit on callback invocations, at least
///   `3 * order` for the root comparison. The run never exceeds it and returns
///   no partial integral when it is reached.
///
/// # Returns
///
/// The finite signed integral estimate whose summed error estimate is at most
/// `tol`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, a negative or
/// non-finite `tol`, an unsupported `order`, or `max_evals == 0`. Returns the
/// callback's own error unchanged. Returns
/// [`InputError::SolverConvergenceFailed`](crate::InputError::SolverConvergenceFailed) (a computation error) when the
/// summed estimate exceeds `tol`, when `max_evals` is reached or is below
/// `3 * order`, when a sample or panel value is NaN, infinite or lost to
/// underflow, or when a panel cannot be bisected in binary64. Its `iterations`
/// is the number of callbacks actually made, `last_x` the coordinate concerned
/// and `reason` names the cause and the panel depth reached.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive;
///
/// let integral = gauss_legendre_integrate_adaptive(
///     |x| Ok((-x * x).exp()),
///     -5.0,
///     5.0,
///     8,      // order
///     1e-8,   // tolerance
///     20,     // max depth
///     10_000, // max evaluations
/// )?;
///
/// // Should be close to sqrt(π) ≈ 1.7725
/// assert!((integral - std::f64::consts::PI.sqrt()).abs() < 1e-6);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn gauss_legendre_integrate_adaptive<F>(
    mut f: F,
    a: f64,
    b: f64,
    order: usize,
    tol: f64,
    max_depth: usize,
    max_evals: usize,
) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<f64, Error>,
{
    const RULE: &str = "gauss_legendre_integrate_adaptive";
    if !a.is_finite() || !b.is_finite() {
        return Err(Error::Validation(format!(
            "{RULE}: endpoints must be finite, got [{a}, {b}]"
        )));
    }
    if !tol.is_finite() || tol < 0.0 {
        return Err(Error::Validation(format!(
            "{RULE}: tolerance must be finite and nonnegative, got {tol}"
        )));
    }
    let rule = gl_nodes_weights(order)?;
    if max_evals == 0 {
        return Err(Error::Validation(format!(
            "{RULE}: max_evals must be positive"
        )));
    }
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok(0.0);
    }
    let (lo, hi, negate) = if a < b { (a, b, false) } else { (b, a, true) };
    let mut work = QuadratureWork {
        rule: RULE,
        max_evals,
        evaluations: 0,
        last_x: lo,
        depth: 0,
    };
    let root_evals = 3 * order;
    if max_evals < root_evals {
        return Err(work.failure(
            f64::NAN,
            lo,
            &format!(
                "evaluation limit {max_evals} is below the {root_evals} samples the root \
                 comparison needs"
            ),
        ));
    }
    let mut sampler = |x: f64| f(x).map(|value| [value]);
    let coarse = gauss_legendre_panel(&mut work, &mut sampler, lo, hi, rule)?;
    let mut stack = vec![LegendrePanel {
        a: lo,
        b: hi,
        coarse,
        tol,
        depth: 0,
    }];
    let mut value = NeumaierAccumulator::new();
    let mut error = NeumaierAccumulator::new();
    let mut deepest = 0;

    while let Some(panel) = stack.pop() {
        work.depth = panel.depth;
        deepest = deepest.max(panel.depth);
        let mid = midpoint(panel.a, panel.b);
        if !(panel.a < mid && mid < panel.b) {
            return Err(work.failure(
                f64::NAN,
                mid,
                "panel cannot be bisected in binary64 on a nonzero interval",
            ));
        }
        let left = gauss_legendre_panel(&mut work, &mut sampler, panel.a, mid, rule)?;
        let right = gauss_legendre_panel(&mut work, &mut sampler, mid, panel.b, rule)?;
        let refined = left + right;
        let estimate = (refined - panel.coarse).abs();
        if !refined.is_finite() || !estimate.is_finite() {
            return Err(work.failure(f64::NAN, mid, LEGENDRE_NOT_REPRESENTABLE));
        }
        if estimate <= panel.tol || panel.depth >= max_depth {
            value
                .add(refined)
                .map_err(|_| work.failure(f64::NAN, mid, LEGENDRE_NOT_REPRESENTABLE))?;
            error
                .add(estimate)
                .map_err(|_| work.failure(f64::NAN, mid, LEGENDRE_NOT_REPRESENTABLE))?;
            continue;
        }
        let left_tol = panel.tol / 2.0;
        // Right is pushed first so the left panel is refined first. Each child
        // carries the half value just computed as its own one-panel value.
        stack.push(LegendrePanel {
            a: mid,
            b: panel.b,
            coarse: right,
            tol: panel.tol - left_tol,
            depth: panel.depth + 1,
        });
        stack.push(LegendrePanel {
            a: panel.a,
            b: mid,
            coarse: left,
            tol: left_tol,
            depth: panel.depth + 1,
        });
    }

    work.depth = deepest;
    let value = value.get_total().map_err(|_| {
        work.failure(
            f64::NAN,
            work.last_x,
            "summed integral is not representable",
        )
    })?;
    let estimate = error.get_total().map_err(|_| {
        work.failure(
            f64::NAN,
            work.last_x,
            "summed error estimate is not representable",
        )
    })?;
    if estimate > tol {
        return Err(work.failure(
            estimate,
            work.last_x,
            &format!(
                "summed error estimate {estimate:.2e} exceeds tolerance {tol:.2e} on \
                 [{a:.6e}, {b:.6e}] with max_depth {max_depth}"
            ),
        ));
    }
    Ok(oriented(value, negate))
}
