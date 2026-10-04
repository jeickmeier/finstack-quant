//! 1D root finding.
//!
//! [`BrentSolver`] is the workspace's scalar root finder, used for implied
//! volatility, yield-to-maturity, spread and internal-rate-of-return solves.
//!
//! # Mathematical Foundation
//!
//! Brent's method combines bisection, the secant method, and inverse quadratic
//! interpolation to guarantee convergence while achieving a superlinear
//! convergence rate. It requires a bracketing interval `[a, b]` where `f(a)` and
//! `f(b)` have opposite signs; [`BrentSolver::solve`] searches for one around an
//! initial guess and [`BrentSolver::solve_in_bracket`] takes one directly.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::solver::BrentSolver;
//!
//! let solver = BrentSolver::new();
//! let f = |x: f64| x * x - 2.0;
//! let root = solver.solve(f, 1.5).expect("Root finding should succeed");
//! assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
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

/// Brent's method solver (bracketing required).
///
/// Implements Brent's root-finding algorithm, which combines bisection,
/// secant method, and inverse quadratic interpolation.
///
/// # Algorithm
///
/// Brent's method maintains a bracketing interval [a, b] where f(a) and f(b)
/// have opposite signs. At each iteration, it chooses between:
/// 1. **Inverse quadratic interpolation**: Fast when applicable
/// 2. **Secant method**: Reliable fallback
/// 3. **Bisection**: bracket-preserving progress
///
/// The algorithm selects the step type from the current bracket and convergence
/// criteria.
///
/// # Convergence
///
/// - **Rate**: Superlinear (order ≈ 1.618)
/// - **Bracket condition**: Requires a finite sign-changing bracket
/// - **Derivative-free**: Does not require smooth derivatives
///
/// # Use Cases
///
/// Use instead of Newton-Raphson when:
/// - Function has discontinuous derivatives (e.g., piecewise functions)
/// - Initial guess quality is uncertain
/// - A sign-changing bracket is available
/// - Function evaluation is cheap relative to derivative computation
///
/// Common applications:
/// - Bond yield-to-maturity (when price/yield curve is complex)
/// - Option implied volatility with exotic payoffs
/// - Credit curve calibration
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::solver::BrentSolver;
///
/// let solver = BrentSolver::new();
///
/// // Solve x^3 - 2x - 5 = 0
/// let f = |x: f64| x.powi(3) - 2.0 * x - 5.0;
/// let root = solver.solve(f, 2.0).expect("Root finding should succeed");
///
/// assert!((f(root)).abs() < 1e-10);
/// assert!((root - 2.0946).abs() < 1e-4);
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct BrentSolver {
    /// Convergence tolerance.
    ///
    /// Convergence uses dual tolerances: the residual (`|f(x)| < tol`) and the
    /// bracket/step size. The default `1e-12` is stricter than the `1e-8`
    /// commonly used by market-facing wrappers such as IRR/XIRR and is intended
    /// for generic core numerical solving.
    pub tolerance: f64,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Bracket expansion factor
    pub bracket_expansion: f64,
    /// Initial bracket size (adaptive to initial guess if None)
    pub initial_bracket_size: Option<f64>,
    /// Minimum bound for bracket search (default: -1e6)
    pub bracket_min: f64,
    /// Maximum bound for bracket search (default: 1e6)
    pub bracket_max: f64,
}

impl Default for BrentSolver {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            max_iterations: 100,
            bracket_expansion: 2.0,
            initial_bracket_size: None, // Adaptive by default
            bracket_min: -1e6,
            bracket_max: 1e6,
        }
    }
}

impl BrentSolver {
    /// Create a new Brent solver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set tolerance.
    #[must_use]
    pub fn tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set initial bracket size. If None, will use adaptive sizing.
    #[must_use]
    pub fn initial_bracket_size(mut self, size: Option<f64>) -> Self {
        self.initial_bracket_size = size;
        self
    }

    /// Set maximum iterations.
    #[must_use]
    pub fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set the minimum and maximum bounds for bracket search.
    ///
    /// During bracket expansion, the search will not extend beyond these bounds.
    /// Default bounds are `[-1e6, 1e6]`, which is suitable for most financial
    /// applications (rates, spreads, volatilities).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::math::solver::BrentSolver;
    ///
    /// // For a problem where the root must be positive
    /// let solver = BrentSolver::new()
    ///     .bracket_bounds(0.0, 1e9);
    ///
    /// // For implied volatility (must be positive, typically < 5.0)
    /// let vol_solver = BrentSolver::new()
    ///     .bracket_bounds(1e-6, 5.0);
    /// ```
    #[must_use]
    pub fn bracket_bounds(mut self, min: f64, max: f64) -> Self {
        self.bracket_min = min;
        self.bracket_max = max;
        self
    }

    /// Find bracket around the root starting from initial guess.
    ///
    /// The search is bounded by `bracket_min` and `bracket_max` to prevent
    /// overflow and to constrain the search to a reasonable domain.
    fn find_bracket<Func>(&self, f: &Func, initial_guess: f64) -> Result<[(f64, f64); 2]>
    where
        Func: Fn(f64) -> f64,
    {
        use crate::error::InputError;

        if !initial_guess.is_finite()
            || !self.bracket_min.is_finite()
            || !self.bracket_max.is_finite()
            || self.bracket_min > self.bracket_max
        {
            return Err(crate::Error::Validation(
                "Brent search requires a finite initial guess and finite ordered bounds".into(),
            ));
        }
        let initial_guess = initial_guess.clamp(self.bracket_min, self.bracket_max);
        let max_bracket_width = self.bracket_max - self.bracket_min;

        let initial_size = self.initial_bracket_size.unwrap_or_else(|| {
            // Use 1% of the initial guess magnitude, with a minimum of 0.01
            let adaptive_size = initial_guess.abs() * 0.01;
            if adaptive_size < 1e-6 {
                0.01 // Fallback for values near zero
            } else {
                adaptive_size.min(1.0) // Cap at 1.0 for very large initial guesses
            }
        });

        if !initial_size.is_finite()
            || initial_size <= 0.0
            || !self.bracket_expansion.is_finite()
            || self.bracket_expansion <= 0.0
        {
            return Err(crate::Error::Validation(
                "Brent search requires positive finite bracket size and expansion".into(),
            ));
        }
        let mut a = (initial_guess - initial_size).max(self.bracket_min);
        let mut b = (initial_guess + initial_size).min(self.bracket_max);
        let mut fa = f(a);
        let mut fb = if a.to_bits() == b.to_bits() { fa } else { f(b) };
        let mut expansion_iterations = 0;

        // Expand bracket until we find a sign change
        for _ in 0..20 {
            expansion_iterations += 1;

            if !fa.is_finite() || !fb.is_finite() {
                return Err(InputError::SolverConvergenceFailed {
                    iterations: expansion_iterations,
                    residual: if fa.is_finite() { fa.abs() } else { fb.abs() },
                    last_x: if fa.is_finite() { a } else { b },
                    reason: format!(
                        "bracket search found non-finite value: f({a:.6e}) = {fa}, f({b:.6e}) = {fb}"
                    ),
                }
                .into());
            }

            if fa == 0.0 || fb == 0.0 || fa.signum() != fb.signum() {
                return Ok([(a, fa), (b, fb)]);
            }

            // Expand bracket with overflow protection
            let width = b - a;

            // Stop if bracket is unreasonably wide
            if width >= max_bracket_width {
                break;
            }

            // Expand with bounds checking to prevent overflow
            let next_a = (a - width * self.bracket_expansion).max(self.bracket_min);
            let next_b = (b + width * self.bracket_expansion).min(self.bracket_max);
            if next_a < a {
                a = next_a;
                fa = f(a);
            }
            if next_b > b {
                b = next_b;
                fb = f(b);
            }

            // Stop if we've hit the bounds
            if a <= self.bracket_min && b >= self.bracket_max {
                break;
            }
        }

        if !fa.is_finite() || !fb.is_finite() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: expansion_iterations,
                residual: if fa.is_finite() { fa.abs() } else { fb.abs() },
                last_x: if fa.is_finite() { a } else { b },
                reason: format!(
                    "bracket search found non-finite value at bounds: f({a:.6e}) = {fa}, f({b:.6e}) = {fb}"
                ),
            }
            .into());
        }

        // Final sign change check at the expanded bounds
        if fa == 0.0 || fb == 0.0 || fa.signum() != fb.signum() {
            return Ok([(a, fa), (b, fb)]);
        }

        tracing::debug!(
            algorithm = "brent_bracket_search",
            iterations = expansion_iterations,
            initial_guess,
            a,
            b,
            fa,
            fb,
            bracket_min = self.bracket_min,
            bracket_max = self.bracket_max,
            category = "no_sign_change",
            "brent: bailout — no sign change found within bracket bounds"
        );
        Err(InputError::SolverConvergenceFailed {
            iterations: expansion_iterations,
            residual: fa.abs().min(fb.abs()),
            last_x: initial_guess,
            reason: format!(
                "no sign change found in [{a:.6e}, {b:.6e}] (bounds: [{:.6e}, {:.6e}]): f(a) = {fa:.6e}, f(b) = {fb:.6e} (same sign)",
                self.bracket_min, self.bracket_max
            ),
        }
        .into())
    }
}

impl BrentSolver {
    /// Solve the equation `f(x) = 0` for `x`, searching for a bracket around
    /// `initial_guess`.
    ///
    /// # Arguments
    ///
    /// * `f` - Function to find the root of (where `f(x) = 0`)
    /// * `initial_guess` - Centre of the bracket-expansion search
    ///
    /// # Returns
    ///
    /// A value `x` accepted on a small residual *or* a sufficiently narrow
    /// bracket, so `x` is not guaranteed to satisfy `|f(x)| < tolerance` in
    /// f-units.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::SolverConvergenceFailed`](crate::error::InputError::SolverConvergenceFailed) when:
    /// - Maximum iterations are exceeded without convergence
    /// - The function returns non-finite values (NaN, infinity)
    /// - No bracketing interval is found within the bracket bounds
    pub fn solve<Func>(&self, f: Func, initial_guess: f64) -> Result<f64>
    where
        Func: Fn(f64) -> f64,
    {
        let bracket = self.find_bracket(&f, initial_guess)?;
        self.brent_method(f, bracket)
    }
}

impl BrentSolver {
    /// Solve within a user-provided bracket `[a, b]` without running the
    /// bracket-expansion search.
    ///
    /// Requirements: `f(a)` and `f(b)` must have opposite signs (or one of
    /// them must already be a root). This is the preferred entry point when
    /// the caller already knows a valid bracket; it is strictly more accurate
    /// than a hand-rolled bisection loop because it uses inverse quadratic
    /// interpolation with Brent's classical safeguards.
    ///
    /// # Arguments
    ///
    /// * `f` - Function to find the root of (where `f(x) = 0`)
    /// * `a` - One finite bracket endpoint; the endpoints may be given in either order
    /// * `b` - The other finite bracket endpoint, with `f(a)` and `f(b)` of opposite sign
    ///
    /// # Errors
    ///
    /// Returns an error if the bracket is invalid (same-sign endpoints,
    /// non-finite evaluations) or Brent's method fails to converge within
    /// `max_iterations`.
    pub fn solve_in_bracket<Func>(&self, mut f: Func, a: f64, b: f64) -> Result<f64>
    where
        Func: FnMut(f64) -> f64,
    {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let flo = f(lo);
        let fhi = if lo.to_bits() == hi.to_bits() {
            flo
        } else {
            f(hi)
        };
        self.brent_method(f, [(lo, flo), (hi, fhi)])
    }
}

impl BrentSolver {
    /// Core Brent's method implementation.
    ///
    /// Requirements: `f(lo)` and `f(hi)` must have opposite signs.
    fn brent_method<Func>(&self, mut f: Func, bracket: [(f64, f64); 2]) -> Result<f64>
    where
        Func: FnMut(f64) -> f64,
    {
        use crate::error::InputError;

        let [(lo, flo), (hi, fhi)] = bracket;
        tracing::debug!(
            lo,
            hi,
            tol = self.tolerance,
            max_iter = self.max_iterations,
            "brent: start"
        );

        // Reject non-finite endpoint evaluations
        if !(flo.is_finite() && fhi.is_finite()) {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: if flo.is_finite() { flo.abs() } else { fhi.abs() },
                last_x: if flo.is_finite() { lo } else { hi },
                reason: format!(
                    "bracket endpoints have non-finite values: f({lo:.6e}) = {flo}, f({hi:.6e}) = {fhi}"
                ),
            }
            .into());
        }
        // Early exit if an endpoint is already a root
        if flo == 0.0 {
            return Ok(lo);
        }
        if fhi == 0.0 {
            return Ok(hi);
        }
        // Require a valid bracket
        if flo.signum() == fhi.signum() {
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
                residual: flo.abs().min(fhi.abs()),
                last_x: lo,
                reason: format!(
                    "bracket endpoints have same sign: f({lo:.6e}) = {flo:.6e}, f({hi:.6e}) = {fhi:.6e}"
                ),
            }
            .into());
        }

        let mut a = lo;
        let mut b = hi;
        let mut fa = flo;
        let mut fb = fhi;
        let mut c = a;
        let mut fc = fa;
        let mut d = b - a;
        let mut e = d;

        for iteration in 0..self.max_iterations {
            if fb.signum() == fc.signum() {
                c = a;
                fc = fa;
                d = b - a;
                e = d;
            }
            if fc.abs() < fb.abs() {
                a = b;
                b = c;
                c = a;
                fa = fb;
                fb = fc;
                fc = fa;
            }
            // Convergence checks
            let tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * self.tolerance;
            let xm = 0.5 * (c - b);
            if xm.abs() <= tol1 || fb == 0.0 {
                tracing::debug!(x = b, residual = fb.abs(), "brent: converged");
                return Ok(b);
            }

            if e.abs() >= tol1 && fa.abs() > fb.abs() {
                // Attempt inverse quadratic interpolation or secant
                let s = fb / fa;
                // Exact comparison: standard Brent's method check for coinciding bracket points.
                #[allow(clippy::float_cmp)]
                let (p, q) = if a == c {
                    // Secant method
                    (2.0 * xm * s, 1.0 - s)
                } else {
                    // Inverse quadratic interpolation
                    let q1 = fa / fc;
                    let r = fb / fc;
                    let p = s * (2.0 * xm * q1 * (q1 - r) - (b - a) * (r - 1.0));
                    let q = (q1 - 1.0) * (r - 1.0) * (s - 1.0);
                    (p, q)
                };
                let mut p = p;
                let mut q = q;
                if p > 0.0 {
                    q = -q;
                } else {
                    p = -p;
                }
                let cond1 = 2.0 * p < 3.0 * xm * q - (tol1 * q).abs();
                let cond2 = p < (e * q).abs() * 0.5;
                if cond1 && cond2 {
                    e = d;
                    d = p / q;
                } else {
                    d = xm;
                    e = d;
                }
            } else {
                d = xm;
                e = d;
            }

            a = b;
            fa = fb;
            if d.abs() > tol1 {
                b += d;
            } else {
                b += tol1.copysign(xm);
            }
            fb = f(b);
            if !fb.is_finite() {
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration + 1,
                    residual: fa.abs(),
                    last_x: b,
                    reason: format!("iteration produced non-finite value: f({b:.6e}) = {fb}"),
                }
                .into());
            }
        }

        // Max iterations reached without convergence - return error
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
            last_x: b,
            reason: format!(
                "max iterations ({}) reached without convergence (tolerance: {:.6e}, residual: {:.6e})",
                self.max_iterations, self.tolerance, fb.abs()
            ),
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brent_solver() {
        let solver = BrentSolver::new();

        // Solve x^3 - x - 1 = 0 (has root around 1.32)
        let f = |x: f64| x * x * x - x - 1.0;
        let root = solver
            .solve(f, 1.0)
            .expect("Root finding should succeed in test");

        assert!(f(root).abs() < 1e-10);
        assert!((root - 1.3247179572447).abs() < 1e-6);
    }

    #[test]
    fn test_brent_solver_adaptive_bracket() {
        // Test with large initial guess to verify adaptive bracketing
        let solver = BrentSolver::new();

        // Solve x - 100 = 0 (root at x = 100)
        let f = |x: f64| x - 100.0;
        let root = solver
            .solve(f, 95.0)
            .expect("Root finding should succeed in test"); // Start near the root

        assert!(f(root).abs() < 1e-10);
        assert!((root - 100.0).abs() < 1e-6);

        // Test with configurable bracket size
        let solver_custom = BrentSolver::new().initial_bracket_size(Some(5.0));
        let root2 = solver_custom
            .solve(f, 95.0)
            .expect("Root finding should succeed in test");
        assert!(f(root2).abs() < 1e-10);
    }

    #[test]
    fn test_brent_solve_in_bracket_reorders_and_accepts_endpoint_roots() {
        let solver = BrentSolver::new();
        let f = |x: f64| x - 2.0;

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
    }

    #[test]
    fn test_brent_solve_in_bracket_rejects_same_sign_endpoints() {
        let solver = BrentSolver::new();
        let err = solver
            .solve_in_bracket(|x| x * x + 1.0, -1.0, 1.0)
            .expect_err("same-sign endpoints are not a valid bracket");

        assert!(
            err.to_string().contains("same sign"),
            "unexpected error: {err}"
        );
    }

    // ===== Phase 1 Robustness Tests =====

    #[test]
    fn test_brent_overflow_protection() {
        // Test that Brent solver doesn't overflow on pathological functions
        let solver = BrentSolver::new();

        // Function with no roots (always positive)
        let f = |x: f64| x * x + 1.0;
        let result = solver.solve(f, 0.0);

        // Should fail gracefully, not panic or return NaN
        assert!(result.is_err(), "Should fail to find root of x^2 + 1");
    }

    #[test]
    fn test_brent_pathological_functions() {
        let solver = BrentSolver::new();

        // Flat function (derivative = 0 everywhere)
        let flat = |_x: f64| 1.0;
        assert!(
            solver.solve(flat, 0.0).is_err(),
            "Should reject flat function"
        );

        // Discontinuous function with root at 0
        let step = |x: f64| if x >= 0.0 { 1.0 } else { -1.0 };
        let root = solver
            .solve(step, 0.5)
            .expect("Should find root at discontinuity");
        assert!(root.abs() < 1e-6, "Root: {}", root);
    }

    #[test]
    fn test_brent_max_iterations_returns_error() {
        let solver = BrentSolver::new().max_iterations(2).tolerance(1e-15);

        // A function that converges slowly (root at x ≈ 1.3247)
        let f = |x: f64| x * x * x - x - 1.0;

        let result = solver.solve(f, 1.0);

        assert!(
            result.is_err(),
            "Should return error when max iterations reached without convergence"
        );

        // Verify error contains useful information
        match result {
            Err(crate::Error::Input(crate::error::InputError::SolverConvergenceFailed {
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
        // Test that bracket bounds can be configured
        let solver = BrentSolver::new().bracket_bounds(0.0, 10.0);

        // Function with root at x = 2
        let f = |x: f64| x - 2.0;
        let root = solver
            .solve(f, 5.0)
            .expect("Should find root within custom bounds");
        assert!((root - 2.0).abs() < 1e-10);

        // Test that search fails when root is outside bounds
        let solver_narrow = BrentSolver::new().bracket_bounds(5.0, 10.0);

        // Root at x = 2 is outside [5, 10]
        let result = solver_narrow.solve(f, 7.0);
        assert!(
            result.is_err(),
            "Should fail when root is outside bracket bounds"
        );
    }

    #[test]
    fn test_brent_initial_bracket_respects_bounds() {
        let solver = BrentSolver::new().bracket_bounds(0.0, 2.0);
        for guess in [-1.0, 0.0, 2.0, 3.0] {
            let root = solver
                .solve(
                    |x| {
                        assert!((0.0..=2.0).contains(&x));
                        x.sqrt() - 1.0
                    },
                    guess,
                )
                .expect("bounded root");
            assert!((root - 1.0).abs() < 1e-10);
        }
        let solver = BrentSolver::new().bracket_bounds(0.0, 1.0);
        assert!(solver.solve(|x| x - 1.005, 1.0).is_err());
        assert!(solver.solve(|x| x + 0.005, 0.0).is_err());
        for endpoint in [0.0, 1.0] {
            assert_eq!(solver.solve(|x| x - endpoint, 0.5).unwrap(), endpoint);
        }
    }

    #[test]
    fn test_brent_reuses_bracket_endpoint_evaluations() {
        for (lo, hi, guess, root) in [
            (0.0, 2.0, 0.0, 1.0),
            (0.0, 2.0, 2.0, 1.0),
            (0.0, 2.0, 0.0, 2.0),
            (0.0, 2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0, 3.0),
            (0.0, 2.0, 2.0, -1.0),
            (0.0, 0.005, 0.0, 1.0),
            (1.0, 1.0, 1.0, 1.0),
            (1.0, 1.0, 1.0, 2.0),
        ] {
            let evaluations = std::cell::RefCell::new(Vec::new());
            let result = BrentSolver::new().bracket_bounds(lo, hi).solve(
                |x| {
                    let mut evaluated = evaluations.borrow_mut();
                    assert!(!evaluated.contains(&x), "duplicate evaluation at {x}");
                    evaluated.push(x);
                    x - root
                },
                guess,
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
        for (lo, hi, guess) in [
            (1.0, 0.0, 0.5),
            (f64::NAN, 1.0, 0.5),
            (0.0, f64::INFINITY, 0.5),
            (0.0, 1.0, f64::NAN),
            (0.0, 1.0, f64::INFINITY),
        ] {
            let evaluations = std::cell::Cell::new(0);
            let result = BrentSolver::new().bracket_bounds(lo, hi).solve(
                |x| {
                    evaluations.set(evaluations.get() + 1);
                    x - 0.5
                },
                guess,
            );
            assert!(result.is_err());
            assert_eq!(evaluations.get(), 0);
        }
    }

    #[test]
    fn test_brent_rejects_non_finite_inner_iteration() {
        let solver = BrentSolver::new();
        let f = |x: f64| {
            if x == 0.0 {
                -1.0
            } else if x == 2.0 {
                1.0
            } else {
                f64::NAN
            }
        };

        let result = solver.solve_in_bracket(f, 0.0, 2.0);
        assert!(
            result.is_err(),
            "Brent should reject non-finite values encountered during iteration"
        );
    }
}
