//! 1D root finding.
//!
//! [`BrentSolver`] is the workspace's scalar root finder, used for implied
//! volatility, yield-to-maturity, spread and internal-rate-of-return solves.
//!
//! # Mathematical Foundation
//!
//! Brent's method combines bisection, the secant method, and inverse quadratic
//! interpolation to find a sign-changing bracket root. [`BrentSolver::solve`]
//! searches for a sign-changing bracket within caller-supplied finite bounds,
//! and [`BrentSolver::solve_in_bracket`] solves directly inside a supplied
//! bracket `[a, b]`. Both accept fallible `FnMut(f64) -> Result<f64>` objectives
//! and propagate callback errors unchanged without evaluating further points.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::solver::BrentSolver;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let solver = BrentSolver::new(1e-12, 100)?;
//! let root = solver.solve(|x| Ok(x * x - 2.0), 1.5, 0.0, 10.0, 0.25, 20)?;
//! assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! See [`docs/REFERENCES.md`](../../../../docs/REFERENCES.md) for canonical
//! anchors:
//!
//! - Brent, R. P. (1973). *Algorithms for Minimization without Derivatives*.
//!   Prentice-Hall. Chapter 4.
//!   ([`brent-1973`](../../../../docs/REFERENCES.md#brent-1973)) `docs/REFERENCES.md#brent-1973`
//! - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 9.3.
//!   ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes)) `docs/REFERENCES.md#press-numerical-recipes`

use crate::Result;

#[inline]
fn norm_zero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

#[inline]
fn half_span(a: f64, b: f64) -> f64 {
    let diff = b - a;
    if diff.is_finite() {
        0.5 * diff
    } else {
        0.5 * b - 0.5 * a
    }
}

#[inline]
fn bracket_converged(b: f64, c: f64, tolerance: f64) -> bool {
    let diff = (c - b).abs();
    if diff.is_finite() {
        diff <= 4.0 * f64::EPSILON * b.abs() + tolerance
    } else {
        (0.5 * c - 0.5 * b).abs() <= 2.0 * f64::EPSILON * b.abs() + 0.5 * tolerance
    }
}

#[inline]
fn opposite_signs(fa: f64, fb: f64) -> bool {
    (fa > 0.0) != (fb > 0.0)
}

/// Brent's method scalar root solver (bracketing required).
///
/// Combines inverse quadratic interpolation, the secant step, and safeguarded
/// bisection over a verified finite sign-changing bracket `[a, b]`.
///
/// # Algorithm & Convergence
///
/// The solver stops only when an evaluated iterate returns computed `f(b) == 0.0`
/// or the coordinate half-bracket width satisfies
/// `|c - b| / 2 <= 2 * f64::EPSILON * |b| + 0.5 * tolerance`.
///
/// - Coordinate width termination does **not** enforce `|f(x)| <= tolerance` in
///   residual units; callers converting roots to prices, rates, or yields should
///   verify their own objective/repricing tolerance after transformation.
/// - Discontinuous sign changes, poles, and even-multiplicity roots can satisfy
///   bracket-width termination or escape bracket expansion.
/// - Lower computed-zero endpoints take priority when both endpoints vanish.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::solver::BrentSolver;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let solver = BrentSolver::new(1e-12, 100)?;
/// let f = |x: f64| x.powi(3) - 2.0 * x - 5.0;
/// let root = solver.solve(|x| Ok(f(x)), 2.0, 0.0, 10.0, 0.5, 20)?;
/// assert!((f(root)).abs() < 1e-10);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Brent, R. P. (1973). *Algorithms for Minimization without Derivatives*.
///   Prentice-Hall. Chapter 4. `docs/REFERENCES.md#brent-1973`
/// - Press, W. H., et al. (2007). *Numerical Recipes: The Art of Scientific Computing*
///   (3rd ed.). Cambridge University Press. Section 9.3. `docs/REFERENCES.md#press-numerical-recipes`
/// - Forsythe, G. E., Malcolm, M. A., & Moler, C. B. (1977). *Computer Methods
///   for Mathematical Computations*. Prentice-Hall.
#[derive(Clone, Debug, PartialEq)]
pub struct BrentSolver {
    tolerance: f64,
    max_iterations: usize,
}

impl BrentSolver {
    /// Construct a validated Brent solver with explicit coordinate tolerance and
    /// iteration budget.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Positive finite coordinate stopping tolerance in `x`-units.
    /// * `max_iterations` - Maximum Brent interior candidate updates (`>= 1`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `tolerance` is non-finite or
    /// `<= 0.0`, or if `max_iterations == 0`.
    pub fn new(tolerance: f64, max_iterations: usize) -> Result<Self> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Brent solver tolerance must be finite and positive, got {tolerance}"
            )));
        }
        if max_iterations == 0 {
            return Err(crate::Error::Validation(
                "Brent solver max_iterations must be at least 1".into(),
            ));
        }
        Ok(Self {
            tolerance,
            max_iterations,
        })
    }

    /// Return the configured coordinate tolerance in `x`-units.
    #[must_use]
    pub fn get_tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Return the configured maximum Brent update iterations.
    #[must_use]
    pub fn get_max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Search `[min, max]` around `initial_guess` for a sign-changing bracket and solve `f(x) = 0`.
    ///
    /// Forms the initial pair `[max(min, initial_guess - initial_half_width),
    /// min(max, initial_guess + initial_half_width)]` and runs up to `max_expansions`
    /// factor-5 total-width expansion steps (sampling left before right) until a
    /// sign-changing bracket or exact zero is located, then runs Brent's method.
    ///
    /// # Arguments
    ///
    /// * `f` - Fallible scalar objective returning `Ok(residual)` in the problem's
    ///   units or propagating the domain/pricing failure immediately.
    /// * `initial_guess` - Finite search centre within `[min, max]`; values outside
    ///   `[min, max]` are rejected rather than clamped.
    /// * `min` - Finite lower bound of the admissible search domain (`min < max`).
    /// * `max` - Finite upper bound of the admissible search domain (`min < max`).
    /// * `initial_half_width` - Positive finite half-width forming the initial search pair;
    ///   the clipped initial pair must have distinct endpoints.
    /// * `max_expansions` - Maximum expansion rounds after the initial pair (`0` disables expansion).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] for invalid search inputs/bounds or count overflow,
    /// propagates the first error from `f` unchanged, or returns
    /// [`crate::InputError::SolverConvergenceFailed`] on non-finite objective output,
    /// missing sign change, stagnation, or iteration exhaustion.
    pub fn solve<Func>(
        &self,
        mut f: Func,
        initial_guess: f64,
        min: f64,
        max: f64,
        initial_half_width: f64,
        max_expansions: usize,
    ) -> Result<f64>
    where
        Func: FnMut(f64) -> Result<f64>,
    {
        match self.find_bracket(
            &mut f,
            initial_guess,
            min,
            max,
            initial_half_width,
            max_expansions,
        )? {
            BracketSearchOutcome::ExactZero(x) => Ok(x),
            BracketSearchOutcome::Bracket(bracket) => self.brent_method(f, bracket),
        }
    }

    /// Solve `f(x) = 0` inside an explicit finite bracket `[a, b]` without search expansion.
    ///
    /// # Arguments
    ///
    /// * `f` - Fallible scalar objective `FnMut(f64) -> Result<f64>`; errors abort immediately.
    /// * `a` - One finite bracket endpoint (endpoints may be passed in either order).
    /// * `b` - The other finite bracket endpoint. If `a == b`, `f` is evaluated once and
    ///   succeeds only when `f(a) == 0.0`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when `a` or `b` is non-finite before calling `f`,
    /// propagates `f`'s first error unchanged, or returns
    /// [`crate::InputError::SolverConvergenceFailed`] when an endpoint or iterate is non-finite,
    /// nonzero endpoints share a sign, representable interior floats exhaust, or `max_iterations`
    /// is exceeded.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::math::solver::BrentSolver;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let root = BrentSolver::new(1e-12, 100)?.solve_in_bracket(|x| Ok(x * x - 2.0), 0.0, 2.0)?;
    /// assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
    /// # Ok(())
    /// # }
    /// ```
    pub fn solve_in_bracket<Func>(&self, mut f: Func, a: f64, b: f64) -> Result<f64>
    where
        Func: FnMut(f64) -> Result<f64>,
    {
        use crate::InputError;

        if !a.is_finite() || !b.is_finite() {
            return Err(crate::Error::Validation(
                "Brent bracket endpoints must be finite".into(),
            ));
        }
        let a = norm_zero(a);
        let b = norm_zero(b);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };

        let flo = f(lo)?;
        if !flo.is_finite() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: flo.abs(),
                last_x: lo,
                reason: format!("lower bracket endpoint evaluated non-finite: f({lo:.6e}) = {flo}"),
            }
            .into());
        }
        if norm_zero(lo).to_bits() == norm_zero(hi).to_bits() {
            if flo == 0.0 {
                return Ok(lo);
            }
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: flo.abs(),
                last_x: lo,
                reason: format!(
                    "degenerate bracket endpoint is not a root: f({lo:.6e}) = {flo:.6e}"
                ),
            }
            .into());
        }

        let fhi = f(hi)?;
        if !fhi.is_finite() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: fhi.abs(),
                last_x: hi,
                reason: format!("upper bracket endpoint evaluated non-finite: f({hi:.6e}) = {fhi}"),
            }
            .into());
        }
        if flo == 0.0 {
            return Ok(lo);
        }
        if fhi == 0.0 {
            return Ok(hi);
        }
        if !opposite_signs(flo, fhi) {
            let (last_x, residual) = if flo.abs() <= fhi.abs() {
                (lo, flo.abs())
            } else {
                (hi, fhi.abs())
            };
            tracing::debug!(
                algorithm = "brent",
                lo,
                hi,
                flo,
                fhi,
                category = "invalid_bracket_same_sign",
                "brent: bailout — bracket endpoints have same sign"
            );
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual,
                last_x,
                reason: format!(
                    "bracket endpoints have same sign: f({lo:.6e}) = {flo:.6e}, f({hi:.6e}) = {fhi:.6e}"
                ),
            }
            .into());
        }
        self.brent_method(f, [(lo, flo), (hi, fhi)])
    }

    fn find_bracket<Func>(
        &self,
        f: &mut Func,
        initial_guess: f64,
        min: f64,
        max: f64,
        initial_half_width: f64,
        max_expansions: usize,
    ) -> Result<BracketSearchOutcome>
    where
        Func: FnMut(f64) -> Result<f64>,
    {
        use crate::InputError;

        if !min.is_finite() || !max.is_finite() || min >= max {
            return Err(crate::Error::Validation(
                "Brent search requires strictly ordered finite bounds min < max".into(),
            ));
        }
        if !initial_guess.is_finite() || initial_guess < min || initial_guess > max {
            return Err(crate::Error::Validation(
                "Brent search requires finite initial_guess within [min, max]".into(),
            ));
        }
        if !initial_half_width.is_finite() || initial_half_width <= 0.0 {
            return Err(crate::Error::Validation(
                "Brent search requires positive finite initial_half_width".into(),
            ));
        }
        if max_expansions
            .checked_mul(2)
            .and_then(|v| v.checked_add(2))
            .is_none()
        {
            return Err(crate::Error::Validation(
                "Brent search max_expansions overflows sample counter".into(),
            ));
        }

        let min = norm_zero(min);
        let max = norm_zero(max);
        let guess = norm_zero(initial_guess);

        let mut a = norm_zero((guess - initial_half_width).max(min));
        let mut b = norm_zero((guess + initial_half_width).min(max));
        if a >= b {
            return Err(crate::Error::Validation(
                "Brent initial search pair collapsed to a single representable coordinate".into(),
            ));
        }

        let mut fa = f(a)?;
        if !fa.is_finite() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: fa.abs(),
                last_x: a,
                reason: format!("initial left sample is non-finite: f({a:.6e}) = {fa}"),
            }
            .into());
        }
        let mut fb = f(b)?;
        if !fb.is_finite() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: fb.abs(),
                last_x: b,
                reason: format!("initial right sample is non-finite: f({b:.6e}) = {fb}"),
            }
            .into());
        }

        if fa == 0.0 {
            return Ok(BracketSearchOutcome::ExactZero(a));
        }
        if fb == 0.0 {
            return Ok(BracketSearchOutcome::ExactZero(b));
        }
        if opposite_signs(fa, fb) {
            return Ok(BracketSearchOutcome::Bracket([(a, fa), (b, fb)]));
        }

        let mut expansions = 0usize;
        for _ in 0..max_expansions {
            let w = b - a;
            let two_w = if w.is_finite() {
                2.0 * w
            } else {
                4.0 * half_span(a, b)
            };
            let next_a = norm_zero((a - two_w).max(min));
            let next_b = norm_zero((b + two_w).min(max));
            if next_a >= a && next_b <= b {
                break;
            }
            expansions += 1;

            if next_a < a {
                let (prev_a, prev_fa) = (a, fa);
                a = next_a;
                fa = f(a)?;
                if !fa.is_finite() {
                    return Err(InputError::SolverConvergenceFailed {
                        iterations: expansions,
                        residual: fa.abs(),
                        last_x: a,
                        reason: format!("left expansion sample is non-finite: f({a:.6e}) = {fa}"),
                    }
                    .into());
                }
                if fa == 0.0 {
                    return Ok(BracketSearchOutcome::ExactZero(a));
                }
                if opposite_signs(fa, prev_fa) {
                    return Ok(BracketSearchOutcome::Bracket([(a, fa), (prev_a, prev_fa)]));
                }
            }

            if next_b > b {
                let (prev_b, prev_fb) = (b, fb);
                b = next_b;
                fb = f(b)?;
                if !fb.is_finite() {
                    return Err(InputError::SolverConvergenceFailed {
                        iterations: expansions,
                        residual: fb.abs(),
                        last_x: b,
                        reason: format!("right expansion sample is non-finite: f({b:.6e}) = {fb}"),
                    }
                    .into());
                }
                if fb == 0.0 {
                    return Ok(BracketSearchOutcome::ExactZero(b));
                }
                if opposite_signs(prev_fb, fb) {
                    return Ok(BracketSearchOutcome::Bracket([(prev_b, prev_fb), (b, fb)]));
                }
            }
        }

        let (last_x, residual) = if fa.abs() <= fb.abs() {
            (a, fa.abs())
        } else {
            (b, fb.abs())
        };
        tracing::debug!(
            algorithm = "brent_bracket_search",
            iterations = expansions,
            initial_guess,
            a,
            b,
            fa,
            fb,
            min,
            max,
            category = "no_sign_change",
            "brent: bailout — no sign change found within search bounds"
        );
        Err(InputError::SolverConvergenceFailed {
            iterations: expansions,
            residual,
            last_x,
            reason: format!(
                "no sign change found in [{a:.6e}, {b:.6e}] (bounds: [{min:.6e}, {max:.6e}]): f(a) = {fa:.6e}, f(b) = {fb:.6e} (same sign)"
            ),
        }
        .into())
    }

    /// Core Brent's method implementation over a verified finite opposite-sign bracket.
    fn brent_method<Func>(&self, mut f: Func, bracket: [(f64, f64); 2]) -> Result<f64>
    where
        Func: FnMut(f64) -> Result<f64>,
    {
        use crate::InputError;

        let [(lo, flo), (hi, fhi)] = bracket;
        tracing::debug!(
            lo,
            hi,
            tol = self.tolerance,
            max_iter = self.max_iterations,
            "brent: start"
        );

        let mut a = lo;
        let mut b = hi;
        let mut fa = flo;
        let mut fb = fhi;
        let mut c = a;
        let mut fc = fa;
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let mut half_d = half_span(b, c);
        let mut half_e = half_d;

        let mut tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * self.tolerance;
        let mut xm = half_span(b, c);
        if fb == 0.0 || bracket_converged(b, c, self.tolerance) {
            tracing::debug!(x = b, residual = fb.abs(), "brent: converged");
            return Ok(norm_zero(b));
        }

        for iteration in 0..self.max_iterations {
            let lo_bc = b.min(c);
            let hi_bc = b.max(c);
            let span = c - b;
            let mid = norm_zero(if span.is_finite() {
                b + 0.5 * span
            } else {
                0.5 * b + 0.5 * c
            });
            if !(mid.is_finite() && mid > lo_bc && mid < hi_bc) {
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration,
                    residual: fb.abs(),
                    last_x: norm_zero(b),
                    reason: format!(
                        "bracket stagnated with no representable interior point in [{lo_bc:.6e}, {hi_bc:.6e}]"
                    ),
                }
                .into());
            }

            let mut use_bisection = true;
            let mut trial = mid;
            let mut candidate_half_d = xm;
            let next_half_e;

            if half_e.abs() >= 0.5 * tol1 && fa.abs() > fb.abs() {
                let s = fb / fa;
                // Exact comparison: standard Brent check for coinciding bracket points.
                #[allow(clippy::float_cmp)]
                let (p_half, q) = if a == c {
                    (xm * s, 1.0 - s)
                } else {
                    let q1 = fa / fc;
                    let r = fb / fc;
                    let p_half = s * (xm * q1 * (q1 - r) - half_span(a, b) * (r - 1.0));
                    let q = (q1 - 1.0) * (r - 1.0) * (s - 1.0);
                    (p_half, q)
                };
                let (p_half, q) = if p_half > 0.0 {
                    (p_half, -q)
                } else {
                    (-p_half, q)
                };
                if q != 0.0 && p_half.is_finite() && q.is_finite() {
                    let half_step = p_half / q;
                    let half_tol = 0.5 * tol1;
                    if half_step.is_finite()
                        && half_step != 0.0
                        && half_step.abs() < 0.5 * half_e.abs()
                        && (half_step > 0.0) == (xm > 0.0)
                        && half_step.abs() < 0.75 * xm.abs() - 0.5 * half_tol
                    {
                        let proposed = norm_zero(if half_step.abs() > half_tol {
                            (b + half_step) + half_step
                        } else {
                            b + tol1.copysign(xm)
                        });
                        if proposed.is_finite()
                            && proposed > lo_bc
                            && proposed < hi_bc
                            && proposed.to_bits() != b.to_bits()
                            && proposed.to_bits() != c.to_bits()
                        {
                            use_bisection = false;
                            trial = proposed;
                            candidate_half_d = half_step;
                        }
                    }
                }
            }

            if use_bisection {
                next_half_e = 0.5 * xm;
                half_d = 0.5 * xm;
            } else {
                next_half_e = half_d;
                half_d = candidate_half_d;
            }
            half_e = next_half_e;

            a = b;
            fa = fb;
            b = trial;
            fb = f(b)?;
            if !fb.is_finite() {
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration + 1,
                    residual: fb.abs(),
                    last_x: b,
                    reason: format!("iteration produced non-finite value: f({b:.6e}) = {fb}"),
                }
                .into());
            }

            if !opposite_signs(fb, fc) {
                c = a;
                fc = fa;
                half_d = half_span(a, b);
                half_e = half_d;
            }
            if fc.abs() < fb.abs() {
                a = b;
                b = c;
                c = a;
                fa = fb;
                fb = fc;
                fc = fa;
            }

            tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * self.tolerance;
            xm = half_span(b, c);
            if fb == 0.0 || bracket_converged(b, c, self.tolerance) {
                tracing::debug!(x = b, residual = fb.abs(), "brent: converged");
                return Ok(norm_zero(b));
            }
        }

        tracing::debug!(
            algorithm = "brent",
            iterations = self.max_iterations,
            last_x = b,
            residual = fb.abs(),
            tolerance = self.tolerance,
            category = "max_iterations_exceeded",
            "brent: bailout — max iterations reached without convergence"
        );
        Err(InputError::SolverConvergenceFailed {
            iterations: self.max_iterations,
            residual: fb.abs(),
            last_x: norm_zero(b),
            reason: format!(
                "max iterations ({}) reached without convergence (tolerance: {:.6e}, residual: {:.6e})",
                self.max_iterations, self.tolerance, fb.abs()
            ),
        }
        .into())
    }
}

enum BracketSearchOutcome {
    ExactZero(f64),
    Bracket([(f64, f64); 2]),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brent_solver() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();

        // Solve x^3 - x - 1 = 0 (has root around 1.32)
        let f = |x: f64| x * x * x - x - 1.0;
        let root = solver
            .solve(|x| Ok(f(x)), 1.0, -1e6, 1e6, 0.01, 20)
            .expect("Root finding should succeed in test");

        assert!(f(root).abs() < 1e-10);
        assert!((root - 1.3247179572447).abs() < 1e-6);
    }

    #[test]
    fn test_brent_solver_bracket_width() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();

        // Solve x - 100 = 0 (root at x = 100)
        let f = |x: f64| x - 100.0;
        let root = solver
            .solve(|x| Ok(f(x)), 95.0, -1e6, 1e6, 0.95, 20)
            .expect("Root finding should succeed in test");

        assert!(f(root).abs() < 1e-10);
        assert!((root - 100.0).abs() < 1e-6);

        let root2 = solver
            .solve(|x| Ok(f(x)), 95.0, -1e6, 1e6, 5.0, 20)
            .expect("Root finding should succeed in test");
        assert!(f(root2).abs() < 1e-10);
    }

    #[test]
    fn test_brent_solve_in_bracket_reorders_and_accepts_endpoint_roots() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let f = |x: f64| Ok(x - 2.0);

        let reversed = solver
            .solve_in_bracket(f, 3.0, 1.0)
            .expect("reversed bracket should be accepted");
        assert!((reversed - 2.0).abs() < 1e-12);

        assert_eq!(
            solver
                .solve_in_bracket(f, 2.0, 5.0)
                .expect("left endpoint root"),
            2.0
        );
        assert_eq!(
            solver
                .solve_in_bracket(f, 0.0, 2.0)
                .expect("right endpoint root"),
            2.0
        );
        // Lower computed-zero wins when both vanish.
        assert_eq!(
            solver
                .solve_in_bracket(|_| Ok(0.0), 3.0, 1.0)
                .expect("lower zero priority"),
            1.0
        );
    }

    #[test]
    fn solve_in_bracket_propagates_objective_errors() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let at_endpoint = solver
            .solve_in_bracket(
                |_| Err(crate::Error::Validation("endpoint".into())),
                0.0,
                1.0,
            )
            .expect_err("endpoint error propagates");
        assert_eq!(at_endpoint, crate::Error::Validation("endpoint".into()));
        let interior = solver
            .solve_in_bracket(
                |x| {
                    if x == 0.0 || x == 2.0 {
                        Ok(x - 1.0)
                    } else {
                        Err(crate::Error::Validation("interior".into()))
                    }
                },
                0.0,
                2.0,
            )
            .expect_err("interior error propagates");
        assert_eq!(interior, crate::Error::Validation("interior".into()));
    }

    #[test]
    fn test_brent_solve_in_bracket_rejects_same_sign_endpoints() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let err = solver
            .solve_in_bracket(|x| Ok(x * x + 1.0), -1.0, 1.0)
            .expect_err("same-sign endpoints are not a valid bracket");

        assert!(
            err.to_string().contains("same sign"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_brent_solve_returns_first_objective_error() {
        // Fails in initial pair: the probe at lower endpoint 0.45 errors before 0.55.
        let evaluations = std::cell::Cell::new(0);
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let err = solver
            .solve(
                |x| {
                    evaluations.set(evaluations.get() + 1);
                    if x < 0.5 {
                        Err(crate::Error::Validation(format!("bad input {x}")))
                    } else {
                        Ok(x - 10.0)
                    }
                },
                0.5,
                0.0,
                100.0,
                0.05,
                20,
            )
            .expect_err("objective error must abort the solve");
        assert!(
            matches!(&err, crate::Error::Validation(msg) if msg.starts_with("bad input")),
            "unexpected error: {err}"
        );
        assert_eq!(evaluations.get(), 1, "no evaluation after the first error");

        // Fails inside Brent iteration: only [0, 2] endpoints evaluate before interior fails.
        let err = solver
            .solve(
                |x| {
                    if x == 0.0 {
                        Ok(-1.0)
                    } else if x == 2.0 {
                        Ok(1.0)
                    } else {
                        Err(crate::Error::Validation("interior".into()))
                    }
                },
                1.0,
                0.0,
                2.0,
                1.0,
                0,
            )
            .expect_err("interior error must abort the solve");
        assert!(matches!(&err, crate::Error::Validation(msg) if msg == "interior"));
    }

    #[test]
    fn test_brent_overflow_protection() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let f = |x: f64| Ok(x * x + 1.0);
        let result = solver.solve(f, 0.0, -1e6, 1e6, 0.01, 20);
        assert!(result.is_err(), "Should fail to find root of x^2 + 1");
    }

    #[test]
    fn test_brent_pathological_functions() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();

        let flat = |_x: f64| Ok(1.0);
        assert!(
            solver.solve(flat, 0.0, -1e6, 1e6, 0.01, 20).is_err(),
            "Should reject flat function"
        );

        let step = |x: f64| Ok(if x >= 0.0 { 1.0 } else { -1.0 });
        let root = solver
            .solve(step, 0.5, -1e6, 1e6, 0.01, 20)
            .expect("Should find root at discontinuity");
        assert!(root.abs() < 1e-6, "Root: {}", root);
    }

    #[test]
    fn test_brent_max_iterations_returns_error() {
        let solver = BrentSolver::new(1e-15, 2).unwrap();
        let f = |x: f64| Ok(x * x * x - x - 1.0);
        let result = solver.solve(f, 1.0, -10.0, 10.0, 0.5, 20);

        match result {
            Err(crate::Error::Input(crate::InputError::SolverConvergenceFailed {
                iterations,
                reason,
                ..
            })) => {
                assert_eq!(iterations, 2, "Should report correct iteration count");
                assert!(
                    reason.contains("max iterations"),
                    "Error message should mention max iterations: {}",
                    reason
                );
            }
            other => panic!("Expected SolverConvergenceFailed error, got {:?}", other),
        }
    }

    #[test]
    fn test_brent_configurable_bracket_bounds() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let f = |x: f64| Ok(x - 2.0);
        let root = solver
            .solve(f, 5.0, 0.0, 10.0, 0.5, 20)
            .expect("Should find root within custom bounds");
        assert!((root - 2.0).abs() < 1e-10);

        let result = solver.solve(f, 7.0, 5.0, 10.0, 0.5, 20);
        assert!(
            result.is_err(),
            "Should fail when root is outside bracket bounds"
        );
    }

    #[test]
    fn test_brent_rejects_out_of_domain_guess_without_clamping() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        for bad_guess in [-1.0, 2.5] {
            let calls = std::cell::Cell::new(0);
            let err = solver
                .solve(
                    |x| {
                        calls.set(calls.get() + 1);
                        Ok(x - 1.0)
                    },
                    bad_guess,
                    0.0,
                    2.0,
                    0.25,
                    10,
                )
                .expect_err("out-of-domain guess must be rejected");
            assert!(matches!(err, crate::Error::Validation(_)));
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn test_brent_reuses_bracket_endpoint_evaluations() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        for (lo, hi, guess, root) in [
            (0.0, 2.0, 0.0, 1.0),
            (0.0, 2.0, 2.0, 1.0),
            (0.0, 2.0, 0.0, 2.0),
            (0.0, 2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0, 3.0),
            (0.0, 2.0, 2.0, -1.0),
            (0.0, 0.005, 0.0, 1.0),
        ] {
            let evaluations = std::cell::RefCell::new(Vec::new());
            let result = solver.solve(
                |x| {
                    let mut evaluated = evaluations.borrow_mut();
                    assert!(!evaluated.contains(&x), "duplicate evaluation at {x}");
                    evaluated.push(x);
                    Ok(x - root)
                },
                guess,
                lo,
                hi,
                0.01,
                20,
            );
            if (lo..=hi).contains(&root) {
                assert!((result.unwrap() - root).abs() < 1e-12);
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn test_brent_invalid_search_inputs_do_not_evaluate_callback() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        for (lo, hi, guess, hw) in [
            (1.0, 0.0, 0.5, 0.1),
            (1.0, 1.0, 1.0, 0.1),
            (f64::NAN, 1.0, 0.5, 0.1),
            (0.0, f64::INFINITY, 0.5, 0.1),
            (0.0, 1.0, f64::NAN, 0.1),
            (0.0, 1.0, f64::INFINITY, 0.1),
            (0.0, 1.0, 0.5, 0.0),
            (0.0, 1.0, 0.5, -0.1),
            (0.0, 1.0, 0.5, f64::NAN),
            (0.0, 1e16, 1e16, 1e-6), // collapsed initial pair
        ] {
            let evaluations = std::cell::Cell::new(0);
            let result = solver.solve(
                |x| {
                    evaluations.set(evaluations.get() + 1);
                    Ok(x - 0.5)
                },
                guess,
                lo,
                hi,
                hw,
                10,
            );
            assert!(result.is_err());
            assert_eq!(evaluations.get(), 0);
        }
    }

    #[test]
    fn test_brent_rejects_non_finite_inner_iteration() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let f = |x: f64| {
            if x == 0.0 {
                -1.0
            } else if x == 2.0 {
                1.0
            } else {
                f64::NAN
            }
        };

        let result = solver.solve_in_bracket(|x| Ok(f(x)), 0.0, 2.0);
        assert!(
            result.is_err(),
            "Brent should reject non-finite values encountered during iteration"
        );
    }

    #[test]
    fn test_brent_find_bracket_catches_one_sided_sign_change() {
        let solver = BrentSolver::new(1e-12, 100).unwrap();
        let root = solver
            .solve(
                |x| Ok((x - 0.05) * (x - 0.45)),
                0.025,
                -10.0,
                10.0,
                0.05,
                20,
            )
            .expect("should bracket root on expansion segment");
        assert!((root - 0.05).abs() < 1e-10 || (root - 0.45).abs() < 1e-10);
    }

    #[test]
    fn brent_core_numerics_05_boundary_cases() {
        // Constructor rejects non-positive/non-finite tolerances & zero budget.
        for bad_tol in [0.0, -1e-8, f64::NAN, f64::INFINITY] {
            assert!(BrentSolver::new(bad_tol, 10).is_err());
        }
        assert!(BrentSolver::new(1e-12, 0).is_err());

        let solver = BrentSolver::new(1e-12, 100).unwrap();

        // Accessors.
        assert_eq!(solver.get_tolerance(), 1e-12);
        assert_eq!(solver.get_max_iterations(), 100);

        // Non-finite brackets rejected before callback.
        for (a, b) in [
            (f64::NAN, 1.0),
            (0.0, f64::NAN),
            (f64::NEG_INFINITY, 1.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(
                solver
                    .solve_in_bracket(|_| unreachable!("non-finite endpoint"), a, b)
                    .is_err()
            );
        }

        // Singleton [x, x]: 1 evaluation; 0 succeeds, nonzero errors.
        let count = std::cell::Cell::new(0);
        assert_eq!(
            solver
                .solve_in_bracket(
                    |_| {
                        count.set(count.get() + 1);
                        Ok(0.0)
                    },
                    -0.0,
                    0.0,
                )
                .unwrap()
                .to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(count.get(), 1);
        assert!(solver.solve_in_bracket(|_| Ok(1.0), 2.0, 2.0).is_err());

        // Opposite tiny subnormal signs preserve sign bracket despite underflowing product.
        let eta = f64::from_bits(1);
        assert_eq!(eta * (-eta), 0.0);
        let root = solver
            .solve_in_bracket(|x| Ok(if x < 0.0 { -eta } else { eta }), -1.0, 1.0)
            .expect("subnormal opposite signs must bracket");
        assert!(root.abs() <= 1e-12);

        // Factor-5 total-width expansion from [-0.1, 0.1] -> [-0.5, 0.5].
        let seen = std::cell::RefCell::new(Vec::new());
        let err = solver
            .solve(
                |x| {
                    seen.borrow_mut().push(x);
                    Ok(1.0)
                },
                0.0,
                -10.0,
                10.0,
                0.1,
                1,
            )
            .unwrap_err();
        assert_eq!(seen.borrow().as_slice(), &[-0.1, 0.1, -0.5, 0.5]);
        assert!(matches!(
            err,
            crate::Error::Input(crate::InputError::SolverConvergenceFailed {
                iterations: 1,
                residual,
                last_x,
                ..
            }) if (residual - 1.0).abs() < 1e-15 && (last_x == -0.5 || last_x == 0.5)
        ));

        // Bad left expansion probe aborts BEFORE evaluating right crossing.
        let seen_lr = std::cell::RefCell::new(Vec::new());
        let err_lr = solver
            .solve(
                |x| {
                    seen_lr.borrow_mut().push(x);
                    if x < -0.15 {
                        Ok(f64::NAN)
                    } else if x > 0.15 {
                        Ok(-1.0)
                    } else {
                        Ok(1.0)
                    }
                },
                0.0,
                -10.0,
                10.0,
                0.1,
                1,
            )
            .unwrap_err();
        assert_eq!(seen_lr.borrow().as_slice(), &[-0.1, 0.1, -0.5]);
        assert!(matches!(
            err_lr,
            crate::Error::Input(crate::InputError::SolverConvergenceFailed { last_x, .. })
                if last_x == -0.5
        ));

        // Full representable [-f64::MAX, f64::MAX] bracket finds 0 safely.
        let max_root = solver
            .solve_in_bracket(|x| Ok(x / f64::MAX), -f64::MAX, f64::MAX)
            .expect("overflow-safe midpoint on [-MAX, MAX]");
        assert_eq!(max_root, 0.0);

        // Final permitted iteration convergence (max_iterations = 1) via width check with fb != 0.
        let one_step = BrentSolver::new(1.1, 1).unwrap();
        let converged = one_step
            .solve_in_bracket(|x| Ok(if x < 1.1 { -1.0 } else { 1.0 }), 0.0, 2.0)
            .expect("iteration 1 bisection to [1.0, 2.0] reaches half-width 0.5 <= 0.55");
        assert_eq!(converged, 1.0);

        // Smallest positive tolerance f64::from_bits(1) must not underflow when halved.
        let sub_tol = f64::from_bits(1);
        let sub_bound = f64::from_bits(2);
        let one_sub = BrentSolver::new(sub_tol, 1).unwrap();
        let sub_err = one_sub
            .solve_in_bracket(
                |x| Ok(if x <= 0.0 { -1.0 } else { 0.5 }),
                -sub_bound,
                sub_bound,
            )
            .expect_err("subnormal tolerance must not falsely converge in 1 step");
        assert!(matches!(
            sub_err,
            crate::Error::Input(crate::InputError::SolverConvergenceFailed { iterations: 1, .. })
        ));
        let two_sub = BrentSolver::new(sub_tol, 2).unwrap();
        let sub_ok = two_sub
            .solve_in_bracket(
                |x| Ok(if x <= 0.0 { -1.0 } else { 0.5 }),
                -sub_bound,
                sub_bound,
            )
            .expect("2 bisections narrow [-1e-323, 1e-323] to subnormal tolerance");
        assert!(sub_ok == 0.0 || sub_ok == sub_tol);

        // Collapsed initial search pair rejects before invoking callback.
        let mut called = false;
        let collapsed = solver.solve(
            |x| {
                called = true;
                Ok(x - 1.0)
            },
            1.0,
            0.0,
            2.0,
            1e-20,
            5,
        );
        assert!(matches!(collapsed, Err(crate::Error::Validation(_))));
        assert!(!called);
    }
}
