//! Shared quadrature work tracking and adaptive Simpson integration.

use crate::math::interp::exact_f64_eq;

use crate::Error;
use crate::InputError;
use crate::math::summation::NeumaierAccumulator;

/// Largest sample magnitude for which small fixed-weight sums (such as the
/// Gauss-Hermite sum whose weights total `√π` or the Simpson panel sum
/// `fa + 4 fm + fb`) cannot overflow.
pub(super) const DIRECT_SUM_LIMIT: f64 = f64::MAX / 8.0;

/// Largest count that converts to `f64` without losing its identity.
pub(super) const MAX_EXACT_COUNT: u64 = 1 << 53;

/// Samples the root comparison of an adaptive Simpson panel needs.
pub(super) const SIMPSON_ROOT_EVALS: usize = 5;

/// Build the computation failure shared by the Simpson rules.
///
/// `evaluations` is the number of callback invocations actually made and
/// `last_x` the coordinate the failure refers to, never a configured limit.
pub(super) fn quadrature_failure(
    rule: &str,
    evaluations: usize,
    residual: f64,
    last_x: f64,
    reason: &str,
) -> Error {
    InputError::SolverConvergenceFailed {
        iterations: evaluations,
        residual,
        last_x,
        reason: format!("{rule}: {reason}"),
    }
    .into()
}

/// Multiply a weighted average by the interval width without forming an
/// overflowing `b - a` when the product itself is representable.
#[inline]
pub(super) fn scale_by_width(average: f64, a: f64, b: f64) -> f64 {
    let width = b - a;
    if width.is_finite() {
        average * width
    } else {
        average * b - average * a
    }
}

/// Point at `fraction` of the way from `lo` to `hi`, exact at both ends and
/// formed without an overflowing span when the endpoints allow it.
#[inline]
pub(super) fn interval_point(lo: f64, hi: f64, fraction: f64) -> f64 {
    if fraction == 0.0 {
        return lo;
    }
    if exact_f64_eq(fraction, 1.0) {
        return hi;
    }
    let span = hi - lo;
    if span.is_finite() {
        fraction.mul_add(span, lo)
    } else {
        fraction.mul_add(hi, (1.0 - fraction) * lo)
    }
}

/// Apply the requested orientation, keeping an exact zero as `+0.0`.
#[inline]
pub(super) fn oriented(value: f64, negate: bool) -> f64 {
    if value == 0.0 {
        0.0
    } else if negate {
        -value
    } else {
        value
    }
}

#[inline]
pub(super) fn midpoint(a: f64, b: f64) -> f64 {
    if (a >= 0.0) == (b >= 0.0) {
        a + 0.5 * (b - a)
    } else {
        0.5 * a + 0.5 * b
    }
}

#[inline]
pub(super) fn half_width(a: f64, b: f64) -> f64 {
    let span = b - a;
    if span.is_finite() {
        0.5 * span
    } else {
        0.5 * b - 0.5 * a
    }
}

/// Explicit work controls of one adaptive run.
#[derive(Clone, Copy)]
pub(super) struct QuadratureLimits {
    /// Deepest bisection level allowed, with the root panel at depth zero.
    pub(super) max_depth: usize,
    /// Hard limit on callback invocations.
    pub(super) max_evals: usize,
}

/// Actual work done so far by one adaptive run.
pub(super) struct QuadratureWork {
    pub(super) rule: &'static str,
    pub(super) max_evals: usize,
    pub(super) evaluations: usize,
    pub(super) last_x: f64,
    pub(super) depth: usize,
}

impl QuadratureWork {
    pub(super) fn failure(&self, residual: f64, x: f64, reason: &str) -> Error {
        quadrature_failure(
            self.rule,
            self.evaluations,
            residual,
            x,
            &format!("{reason} (panel depth {})", self.depth),
        )
    }

    /// Evaluate the callback once at `x`, enforcing the evaluation limit and
    /// rejecting a non-finite component.
    pub(super) fn sample<const N: usize, F>(&mut self, f: &mut F, x: f64) -> Result<[f64; N], Error>
    where
        F: FnMut(f64) -> Result<[f64; N], Error>,
    {
        if self.evaluations >= self.max_evals {
            return Err(self.failure(
                f64::NAN,
                x,
                &format!(
                    "evaluation limit {} reached before the required sample",
                    self.max_evals
                ),
            ));
        }
        self.evaluations += 1;
        self.last_x = x;
        let values = f(x)?;
        if let Some(component) = values.iter().position(|value| !value.is_finite()) {
            return Err(self.failure(
                f64::NAN,
                x,
                &format!(
                    "integrand{} was not finite",
                    component_label::<N>(component)
                ),
            ));
        }
        Ok(values)
    }
}

/// Label a pair component in tuple order; a scalar rule has no label.
pub(super) fn component_label<const N: usize>(component: usize) -> String {
    if N == 1 {
        String::new()
    } else {
        format!(" component {}", component + 1)
    }
}

/// One pending Simpson panel `[a, b]` with its cached samples at `a`, `b` and
/// the midpoint (`fc`), its one-panel estimate and its local error share.
pub(super) struct SimpsonPanel<const N: usize> {
    pub(super) a: f64,
    pub(super) b: f64,
    pub(super) fa: [f64; N],
    pub(super) fb: [f64; N],
    pub(super) fc: [f64; N],
    pub(super) whole: [f64; N],
    pub(super) tol: [f64; N],
    pub(super) depth: usize,
}

pub(super) fn simpson_panel<const N: usize>(
    a: f64,
    b: f64,
    fa: &[f64; N],
    fm: &[f64; N],
    fb: &[f64; N],
) -> Result<[f64; N], usize> {
    let mut out = [0.0; N];
    for i in 0..N {
        let direct = fa[i].abs() <= DIRECT_SUM_LIMIT
            && fm[i].abs() <= DIRECT_SUM_LIMIT
            && fb[i].abs() <= DIRECT_SUM_LIMIT;
        let (factor, value) = if direct {
            let sum = fa[i] + 4.0 * fm[i] + fb[i];
            (sum, (half_width(a, b) / 3.0) * sum)
        } else {
            let average = fa[i] / 6.0 + fm[i] * (2.0 / 3.0) + fb[i] / 6.0;
            (average, scale_by_width(average, a, b))
        };
        if !value.is_finite() || (factor != 0.0 && value == 0.0) {
            return Err(i);
        }
        out[i] = value;
    }
    Ok(out)
}

/// Shared adaptive Simpson engine for `N` integrands on one mesh.
///
/// Iterative, left panel before right, with an internal stack bounded by
/// `max_depth + 1`. Each component has its own budget, split as
/// `left = tol / 2`, `right = tol - left`.
pub(super) fn adaptive_simpson_core<const N: usize, F>(
    rule: &'static str,
    f: &mut F,
    a: f64,
    b: f64,
    tol: [f64; N],
    limits: QuadratureLimits,
) -> Result<[f64; N], Error>
where
    F: FnMut(f64) -> Result<[f64; N], Error>,
{
    let QuadratureLimits {
        max_depth,
        max_evals,
    } = limits;
    if !a.is_finite() || !b.is_finite() {
        return Err(Error::Validation(format!(
            "{rule}: endpoints must be finite, got [{a}, {b}]"
        )));
    }
    if tol.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return Err(Error::Validation(format!(
            "{rule}: every tolerance must be finite and nonnegative, got {tol:?}"
        )));
    }
    if max_evals == 0 {
        return Err(Error::Validation(format!(
            "{rule}: max_evals must be positive"
        )));
    }
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok([0.0; N]);
    }
    let (lo, hi, negate) = if a < b { (a, b, false) } else { (b, a, true) };
    let mut work = QuadratureWork {
        rule,
        max_evals,
        evaluations: 0,
        last_x: lo,
        depth: 0,
    };
    if max_evals < SIMPSON_ROOT_EVALS {
        return Err(work.failure(
            f64::NAN,
            lo,
            &format!(
                "evaluation limit {max_evals} is below the {SIMPSON_ROOT_EVALS} samples the \
                 root comparison needs"
            ),
        ));
    }
    const NOT_BISECTABLE: &str = "panel cannot be bisected in binary64 on a nonzero interval";
    const NOT_REPRESENTABLE: &str = "panel estimate is not representable";

    let root_mid = midpoint(lo, hi);
    if !(lo < root_mid && root_mid < hi) {
        return Err(work.failure(f64::NAN, root_mid, NOT_BISECTABLE));
    }
    let fa = work.sample(f, lo)?;
    let fb = work.sample(f, hi)?;
    let fc = work.sample(f, root_mid)?;
    let whole = simpson_panel(lo, hi, &fa, &fc, &fb).map_err(|component| {
        work.failure(
            f64::NAN,
            root_mid,
            &format!("{NOT_REPRESENTABLE}{}", component_label::<N>(component)),
        )
    })?;

    let mut stack = vec![SimpsonPanel {
        a: lo,
        b: hi,
        fa,
        fb,
        fc,
        whole,
        tol,
        depth: 0,
    }];
    let mut values: [NeumaierAccumulator; N] = std::array::from_fn(|_| NeumaierAccumulator::new());
    let mut errors: [NeumaierAccumulator; N] = std::array::from_fn(|_| NeumaierAccumulator::new());
    let mut deepest = 0;

    while let Some(panel) = stack.pop() {
        work.depth = panel.depth;
        deepest = deepest.max(panel.depth);
        let c = midpoint(panel.a, panel.b);
        let d = midpoint(panel.a, c);
        let e = midpoint(c, panel.b);
        if !(panel.a < d && d < c && c < e && e < panel.b) {
            return Err(work.failure(f64::NAN, c, NOT_BISECTABLE));
        }
        let fd = work.sample(f, d)?;
        let fe = work.sample(f, e)?;
        let unusable = |component: usize| {
            work.failure(
                f64::NAN,
                c,
                &format!("{NOT_REPRESENTABLE}{}", component_label::<N>(component)),
            )
        };
        let left = simpson_panel(panel.a, c, &panel.fa, &fd, &panel.fc).map_err(unusable)?;
        let right = simpson_panel(c, panel.b, &panel.fc, &fe, &panel.fb).map_err(unusable)?;
        let mut total = [0.0; N];
        let mut estimate = [0.0; N];
        for i in 0..N {
            total[i] = left[i] + right[i];
            estimate[i] = (total[i] - panel.whole[i]).abs() / 15.0;
            if !total[i].is_finite() || !estimate[i].is_finite() {
                return Err(unusable(i));
            }
        }

        let within_share = (0..N).all(|i| estimate[i] <= panel.tol[i]);
        if within_share || panel.depth >= max_depth {
            for i in 0..N {
                values[i].add(total[i]).map_err(|_| unusable(i))?;
                errors[i].add(estimate[i]).map_err(|_| unusable(i))?;
            }
            continue;
        }

        let left_tol: [f64; N] = std::array::from_fn(|i| panel.tol[i] / 2.0);
        let right_tol: [f64; N] = std::array::from_fn(|i| panel.tol[i] - left_tol[i]);
        // Right is pushed first so the left panel is refined first.
        stack.push(SimpsonPanel {
            a: c,
            b: panel.b,
            fa: panel.fc,
            fb: panel.fb,
            fc: fe,
            whole: right,
            tol: right_tol,
            depth: panel.depth + 1,
        });
        stack.push(SimpsonPanel {
            a: panel.a,
            b: c,
            fa: panel.fa,
            fb: panel.fc,
            fc: fd,
            whole: left,
            tol: left_tol,
            depth: panel.depth + 1,
        });
    }

    work.depth = deepest;
    let mut out = [0.0; N];
    for i in 0..N {
        let label = component_label::<N>(i);
        let value = values[i].get_total().map_err(|_| {
            work.failure(
                f64::NAN,
                work.last_x,
                &format!("summed integral{label} is not representable"),
            )
        })?;
        let estimate = errors[i].get_total().map_err(|_| {
            work.failure(
                f64::NAN,
                work.last_x,
                &format!("summed error estimate{label} is not representable"),
            )
        })?;
        if estimate > tol[i] {
            return Err(work.failure(
                estimate,
                work.last_x,
                &format!(
                    "summed error estimate{label} {estimate:.2e} exceeds tolerance {:.2e} on \
                     [{a:.6e}, {b:.6e}] with max_depth {max_depth}",
                    tol[i]
                ),
            ));
        }
        out[i] = oriented(value, negate);
    }
    Ok(out)
}

/// Adaptive Simpson quadrature with an absolute global error budget.
///
/// Each panel compares its one-panel Simpson value `S1` with the sum `S2` of
/// its two half panels, uses `e = |S2 - S1| / 15` as the error estimate and
/// keeps the uncorrected `S2`. A panel whose estimate exceeds its share of the
/// budget is bisected, the left half receiving `tol / 2` and the right half
/// the remainder, until `max_depth`. Terminal values and estimates are summed
/// with compensated totals; the call succeeds only when the summed estimate is
/// at most `tol`. Endpoint and midpoint samples are cached and reused, so the
/// root costs five evaluations and every bisection two more.
///
/// The estimate is an operational difference, not a bound on the true error.
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
/// * `tol` - Finite, nonnegative absolute budget for the sum of all terminal
///   error estimates, in integral units. Zero is allowed and requests a zero
///   computed estimate, not exact integration.
/// * `max_depth` - Deepest bisection level, with the root panel at depth zero.
///   Depth zero still performs the root comparison. A depth-limited panel may
///   exceed its local share if the summed estimate still fits `tol`.
/// * `max_evals` - Positive hard limit on callback invocations, at least five
///   for the root comparison. The run never exceeds it and returns no partial
///   integral when it is reached.
///
/// # Returns
///
/// The finite signed integral estimate whose summed error estimate is at most
/// `tol`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, a negative or
/// non-finite `tol`, or `max_evals == 0`. Returns the callback's own error
/// unchanged. Returns [`InputError::SolverConvergenceFailed`] (a computation
/// error) when the summed estimate exceeds `tol`, when `max_evals` is reached
/// or is below five, when a sample or panel value is NaN, infinite or lost to
/// underflow, or when a panel cannot be bisected in binary64. Its `iterations`
/// is the number of callbacks actually made, `last_x` the coordinate concerned
/// and `reason` names the cause and the panel depth reached.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::adaptive_simpson;
///
/// let integral = adaptive_simpson(
///     |x| Ok((10.0 * x).sin()),
///     0.0,
///     std::f64::consts::PI,
///     1e-6,
///     40,
///     100_000,
/// )?;
/// assert!(integral.abs() < 1e-5);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn adaptive_simpson<F>(
    mut f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_depth: usize,
    max_evals: usize,
) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<f64, Error>,
{
    let mut scalar = |x: f64| f(x).map(|value| [value]);
    let limits = QuadratureLimits {
        max_depth,
        max_evals,
    };
    adaptive_simpson_core("adaptive_simpson", &mut scalar, a, b, [tol], limits).map(|[value]| value)
}

/// Integrate two functions on one shared adaptive Simpson mesh.
///
/// The callback returns both integrands at each node and one call counts as
/// one evaluation. The mesh is refined when either component exceeds its own
/// share; each component's budget is split and summed independently, so a
/// large value or spare tolerance in one component cannot consume the other's
/// budget. The formulas are those of [`adaptive_simpson`], applied per
/// component. Results need not match two independent scalar runs bit for bit,
/// because the refined meshes can differ.
///
/// # Arguments
///
/// * `f` - Fallible callback returning both integrands in tuple order at one
///   domain coordinate. It must describe one deterministic function for the
///   whole call. The first error it returns stops the run and is returned
///   unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` runs the same
///   increasing-bound refinement and negates both results; `b == a` returns
///   `(+0.0, +0.0)` without calling `f`.
/// * `tol` - `[tol0, tol1]`: finite, nonnegative absolute budgets, one per
///   component in tuple order, each in its own component's integral units.
/// * `max_depth` - Deepest bisection level, with the root panel at depth zero.
/// * `max_evals` - Positive hard limit on callback invocations, at least five
///   for the root comparison.
///
/// # Returns
///
/// The two finite signed integral estimates in tuple order, each with a summed
/// error estimate at most its own budget.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, a negative or
/// non-finite budget, or `max_evals == 0`. Returns the callback's own error
/// unchanged. Returns [`InputError::SolverConvergenceFailed`] (a computation
/// error) under the conditions listed for [`adaptive_simpson`]; `reason`
/// names the failing component, the first in tuple order when both fail at
/// one sample or gate.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::adaptive_simpson_pair;
///
/// let (square, cube) = adaptive_simpson_pair(
///     |x| Ok((x * x, x * x * x)),
///     0.0,
///     1.0,
///     [1e-10, 1e-10],
///     20,
///     10_000,
/// )?;
/// assert!((square - 1.0 / 3.0).abs() < 1e-10);
/// assert!((cube - 0.25).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn adaptive_simpson_pair<F>(
    mut f: F,
    a: f64,
    b: f64,
    tol: [f64; 2],
    max_depth: usize,
    max_evals: usize,
) -> Result<(f64, f64), Error>
where
    F: FnMut(f64) -> Result<(f64, f64), Error>,
{
    let mut paired = |x: f64| f(x).map(|(first, second)| [first, second]);
    let limits = QuadratureLimits {
        max_depth,
        max_evals,
    };
    adaptive_simpson_core("adaptive_simpson_pair", &mut paired, a, b, tol, limits)
        .map(|[first, second]| (first, second))
}

// Gauss–Legendre Quadrature (finite intervals)
