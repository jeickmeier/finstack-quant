//! Generic solver interfaces for 1D root finding.
//!
//! This module provides unified interfaces for 1D root finding algorithms
//! commonly used in financial computations such as implied volatility calculation,
//! yield-to-maturity solving, and internal rate of return computation.
//!
//! # Algorithms
//!
//! - [`NewtonSolver`]: Newton-Raphson method with finite difference derivatives (or analytic via [`solve_with_derivative`](NewtonSolver::solve_with_derivative))
//! - [`BrentSolver`]: Brent's method (robust bracketing method)
//!
//! # Mathematical Foundation
//!
//! ## Newton-Raphson Method
//!
//! Iteratively refines an initial guess using:
//! ```text
//! x_{n+1} = x_n - f(x_n) / f'(x_n)
//! ```
//!
//! Convergence is quadratic near the root but requires a good initial guess
//! and may fail if the derivative is small or the function is non-smooth.
//!
//! ## Brent's Method
//!
//! Combines bisection, secant method, and inverse quadratic interpolation
//! to guarantee convergence while achieving superlinear convergence rate.
//! Requires a bracketing interval [a, b] where f(a) and f(b) have opposite signs.
//!
//! # Examples
//!
//! ## Newton-Raphson for square root
//!
//! ```
//! use finstack_core::math::solver::{NewtonSolver, Solver};
//!
//! let solver = NewtonSolver::new().tolerance(1e-10);
//! let f = |x: f64| x * x - 2.0;
//! let root = solver.solve(f, 1.0).expect("Root finding should succeed");
//! assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
//! ```
//!
//! ## Brent's method for transcendental equation
//!
//! ```
//! use finstack_core::math::solver::{BrentSolver, Solver};
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
//! - **Newton-Raphson**:
//!   - Press, W. H., et al. (2007). *Numerical Recipes: The Art of Scientific Computing*
//!     (3rd ed.). Cambridge University Press. Section 9.4.
//!     ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes))
//!   - Burden, R. L., & Faires, J. D. (2010). *Numerical Analysis* (9th ed.).
//!     Brooks/Cole. Section 2.3.
//!
//! - **Brent's Method**:
//!   - Brent, R. P. (1973). *Algorithms for Minimization without Derivatives*.
//!     Prentice-Hall. Chapter 4.
//!     ([`brent-1973`](../../../../docs/REFERENCES.md#brent-1973))
//!   - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 9.3.
//!     ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes))

use crate::Result;

/// Domain-specific hints for initial bracket sizing in Brent's method.
///
/// Different financial quantities have typical ranges that can dramatically
/// improve convergence speed when the bracket is appropriately sized.
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::solver::{BrentSolver, BracketHint, Solver};
///
/// // For implied volatility (typically 0.01 to 2.0)
/// let solver = BrentSolver::new().bracket_hint(BracketHint::ImpliedVol);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BracketHint {
    /// Implied volatility: σ typically in [0.01, 2.0], initial bracket ±0.2
    ImpliedVol,
    /// Interest rate: r typically in [-0.05, 0.30], initial bracket ±0.02
    Rate,
    /// Credit spread: spread typically in [0, 0.05], initial bracket ±0.005
    Spread,
    /// Yield-to-maturity: similar to rates, initial bracket ±0.02
    Ytm,
    /// Internal Rate of Return (IRR/XIRR): typically in [-0.5, 1.0], initial bracket ±0.5
    ///
    /// IRR calculations can have roots across a very wide range:
    /// - Private equity/VC: +100% to +500% returns are common
    /// - Distressed investments: -50% to -90% returns possible
    /// - Typical investments: -10% to +30%
    ///
    /// The larger bracket (±0.5) allows the solver to find roots across this
    /// wide range while still converging quickly for typical cases.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_core::math::solver::{BrentSolver, BracketHint, Solver};
    ///
    /// let solver = BrentSolver::new()
    ///     .bracket_hint(BracketHint::Xirr)
    ///     .bracket_bounds(-0.99, 10.0);  // Allow up to 1000% returns
    /// ```
    Xirr,
    /// Custom bracket size
    Custom(f64),
}

impl BracketHint {
    /// Convert hint to initial bracket size.
    // Semantic table: Rate and Ytm are distinct hints that happen to share a size.
    #[inline]
    #[allow(clippy::match_same_arms)]
    pub fn to_bracket_size(self) -> f64 {
        match self {
            BracketHint::ImpliedVol => 0.2,
            BracketHint::Rate => 0.02,
            BracketHint::Spread => 0.005,
            BracketHint::Ytm => 0.02,
            BracketHint::Xirr => 0.5,
            BracketHint::Custom(size) => size,
        }
    }
}

/// Generic solver trait for 1D root finding.
///
/// Provides a unified interface for numerical root-finding algorithms that solve
/// the equation `f(x) = 0` for `x`. Implementations may use different algorithms
/// with varying convergence guarantees and performance characteristics.
///
/// # Required Methods
///
/// Implementors must provide:
/// - [`solve`](Self::solve): Find a root given a function and initial guess
///
/// # Provided Implementations
///
/// The following solvers implement this trait:
/// - [`NewtonSolver`]: Newton iteration with finite-difference or analytic derivatives
/// - [`BrentSolver`]: Bracketed method combining bisection, secant, and inverse quadratic steps
///
/// # Implementation Guide
///
/// When implementing this trait:
/// 1. Validate that `initial_guess` is finite
/// 2. Handle non-finite function values gracefully (return error, don't diverge)
/// 3. Respect reasonable iteration limits to prevent infinite loops
/// 4. Use appropriate convergence criteria (both |f(x)| and |x_n - x_{n-1}|)
///
/// # Examples
///
/// ## Using a solver
///
/// ```rust
/// use finstack_core::math::solver::{Solver, NewtonSolver};
///
/// fn find_yield<S: Solver>(solver: &S, target_price: f64) -> finstack_core::Result<f64> {
///     let price_error = |y: f64| {
///         // Price as function of yield (simplified)
///         100.0 / (1.0 + y) - target_price
///     };
///     solver.solve(price_error, 0.05)
/// }
/// ```
///
/// ## Implementing a custom solver
///
/// ```rust
/// use finstack_core::math::solver::Solver;
/// use finstack_core::Result;
///
/// struct BisectionSolver {
///     tolerance: f64,
///     max_iterations: usize,
/// }
///
/// impl Solver for BisectionSolver {
///     fn solve<F>(&self, f: F, initial_guess: f64) -> Result<f64>
///     where
///         F: Fn(f64) -> f64,
///     {
///         // Custom bisection implementation
///         // (simplified - real impl would need proper bracketing)
///         let mut x = initial_guess;
///         for _ in 0..self.max_iterations {
///             let fx = f(x);
///             if fx.abs() < self.tolerance {
///                 return Ok(x);
///             }
///             x -= fx * 0.01; // Simple step
///         }
///         // Per the trait contract: error on non-convergence rather than
///         // returning the unconverged iterate.
///         Err(finstack_core::error::InputError::SolverConvergenceFailed {
///             iterations: self.max_iterations,
///             residual: f(x).abs(),
///             last_x: x,
///             reason: "max iterations reached without convergence".to_string(),
///         }
///         .into())
///     }
/// }
/// ```
///
/// # See Also
///
/// - [`NewtonSolver`] for smooth functions
/// - [`BrentSolver`] for sign-changing brackets
/// - [`crate::math::solver_multi::LevenbergMarquardtSolver`] for multi-dimensional problems
pub trait Solver: Send + Sync {
    /// Solve the equation `f(x) = 0` for `x`.
    ///
    /// # Arguments
    ///
    /// * `f` - Function to find the root of (where `f(x) = 0`)
    /// * `initial_guess` - Starting point for the iteration
    ///
    /// # Returns
    ///
    /// A value `x` accepted by the solver's convergence criteria. These are
    /// solver-dependent: [`NewtonSolver`] accepts on a small residual
    /// (`|f(x)| < tol × residual_scale`) or a small step, while [`BrentSolver`]
    /// accepts on a small residual *or* a sufficiently narrow bracket — so the
    /// returned `x` is not guaranteed to satisfy `|f(x)| < tolerance` in
    /// f-units for Brent's method.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::SolverConvergenceFailed`](crate::error::InputError::SolverConvergenceFailed) when:
    /// - Maximum iterations exceeded without convergence
    /// - Function returns non-finite values (NaN, infinity)
    /// - Derivative is too small (for Newton-based methods)
    /// - No bracketing interval found (for Brent's method)
    fn solve<Func>(&self, f: Func, initial_guess: f64) -> Result<f64>
    where
        Func: Fn(f64) -> f64;
}

/// Newton-Raphson solver with automatic derivative estimation.
///
/// Implements the classic Newton-Raphson root finding algorithm with finite
/// difference approximation for the derivative. This provides a balance between
/// convergence speed and ease of use (no need to provide analytical derivatives).
///
/// # Algorithm
///
/// The solver iterates using:
/// ```text
/// x_{n+1} = x_n - f(x_n) / f'(x_n)
/// ```
///
/// where `f'(x)` is approximated using scale-adaptive central differences:
/// ```text
/// h = base_step × max(|x|, 1.0)
/// f'(x) ≈ (f(x + h) - f(x - h)) / (2h)
/// ```
///
/// The adaptive step size prevents catastrophic cancellation for large-magnitude
/// problems while maintaining accuracy for small values.
///
/// # Convergence
///
/// - **Rate**: Quadratic near the root (number of correct digits roughly doubles each iteration)
/// - **Requirements**: Good initial guess, smooth function, non-zero derivative
/// - **Failure modes**: May diverge if initial guess is poor or derivative is near zero
///
/// # Use Cases
///
/// - Implied volatility calculation (Black-Scholes)
/// - Yield-to-maturity solving
/// - Internal rate of return (IRR)
/// - Duration-matched portfolio optimization
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::solver::{NewtonSolver, Solver};
///
/// // Solve for implied volatility (simplified example)
/// let target_price = 10.5;
/// let solver = NewtonSolver::new().tolerance(1e-6);
///
/// let price_error = |vol: f64| {
///     // In practice, this would call Black-Scholes formula
///     let price = vol * 100.0; // Simplified
///     price - target_price
/// };
///
/// let implied_vol = solver.solve(price_error, 0.2).expect("Root finding should succeed");
/// assert!((price_error(implied_vol)).abs() < 1e-6);
/// ```
///
/// # References
///
/// - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 9.4.
///   Recommends h ≈ sqrt(epsilon) × max(|x|, 1) for scale-adaptive derivatives.
/// - Ralston, A., & Rabinowitz, P. (2001). *A First Course in Numerical Analysis*
///   (2nd ed.). Dover. Chapter 8.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NewtonSolver {
    /// Convergence tolerance.
    ///
    /// Convergence uses dual tolerances:
    /// - residual: `|f(x)| < tol × residual_scale` (absolute in f-units, with an
    ///   optional caller-supplied scale; see [`residual_scale`](Self::residual_scale)),
    /// - step: `|x_new - x_old| < tol × max(1, |x_new|)` (relative for large `|x|`,
    ///   absolute near zero).
    ///
    /// The default `1e-12` is stricter than the `1e-8` commonly used for
    /// market-standard pricing helpers and is intended for generic numerical
    /// root-finding inside core math routines.
    pub tolerance: f64,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Base finite difference step for derivative estimation (scaled adaptively)
    pub fd_step: f64,
    /// Minimum derivative threshold (absolute guard)
    pub min_derivative: f64,
    /// Relative minimum derivative threshold (derivative / function value)
    pub min_derivative_rel: f64,
    /// Scale applied to the residual convergence test: `|f(x)| < tolerance × residual_scale`.
    ///
    /// Defaults to `1.0` (purely absolute residual test). Set this to the natural
    /// magnitude of `f` (e.g. a notional, or a reference price) when residuals are
    /// expressed in units far from O(1) — for dollar-scale objectives an absolute
    /// `1e-12` residual is unattainable in `f64` and the solver would otherwise
    /// rely solely on the step test.
    pub residual_scale: f64,
}

impl Default for NewtonSolver {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            max_iterations: 50,
            fd_step: f64::EPSILON.cbrt(),
            min_derivative: 1e-14,
            min_derivative_rel: 1e-6,
            residual_scale: 1.0,
        }
    }
}

impl NewtonSolver {
    /// Create a new Newton solver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set tolerance.
    #[must_use]
    pub fn tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set maximum iterations.
    #[must_use]
    pub fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set minimum derivative threshold (absolute).
    #[must_use]
    pub fn min_derivative(mut self, min_derivative: f64) -> Self {
        self.min_derivative = min_derivative;
        self
    }

    /// Set relative minimum derivative threshold.
    #[must_use]
    pub fn min_derivative_rel(mut self, min_derivative_rel: f64) -> Self {
        self.min_derivative_rel = min_derivative_rel;
        self
    }

    /// Set the residual scale used in the residual convergence test.
    ///
    /// The residual test becomes `|f(x)| < tolerance × residual_scale`. Use the
    /// natural magnitude of the objective (e.g. notional or reference price) when
    /// residuals are not O(1). Defaults to `1.0`.
    #[must_use]
    pub fn residual_scale(mut self, residual_scale: f64) -> Self {
        self.residual_scale = residual_scale;
        self
    }

    /// Compute scale-adaptive finite difference step.
    ///
    /// Uses the formula: h = base_step × max(|x|, 1.0)
    ///
    /// This prevents catastrophic cancellation for large-magnitude problems
    /// while maintaining accuracy for small values.
    ///
    /// # References
    ///
    /// Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 9.4.
    #[inline]
    fn adaptive_fd_step(&self, x: f64) -> f64 {
        let scale = x.abs().max(1.0);
        self.fd_step * scale
    }
}

impl Solver for NewtonSolver {
    fn solve<Func>(&self, f: Func, initial_guess: f64) -> Result<f64>
    where
        Func: Fn(f64) -> f64,
    {
        // Use automatic differentiation via scale-adaptive finite differences
        let derivative = |x: f64| -> f64 {
            let h = self.adaptive_fd_step(x);
            let f_plus = f(x + h);
            let f_minus = f(x - h);
            (f_plus - f_minus) / (2.0 * h)
        };

        self.newton_method(&f, derivative, initial_guess)
    }
}

impl NewtonSolver {
    /// Solve using Newton-Raphson with an analytic derivative.
    ///
    /// **Recommended over [`solve()`](Solver::solve) when derivatives are available.**
    /// This method provides better performance and numerical stability compared to
    /// the automatic finite-difference approach.
    ///
    /// # Performance Benefits
    ///
    /// - **2× fewer function evaluations**: No need to compute `f(x+h)` and `f(x-h)`
    /// - **Better numerical stability**: Avoids finite-difference cancellation errors
    /// - **Faster convergence**: Exact derivatives lead to more accurate Newton steps
    ///
    /// # Evaluation count
    ///
    /// | Method | Function Evals/Iter | Typical Iterations | Total Evals |
    /// |--------|---------------------|-------------------|-------------|
    /// | `solve()` (finite diff) | 3 (f, f+h, f-h) | 5-10 | 15-30 |
    /// | `solve_with_derivative()` | 2 (f, f') | 4-8 | 8-16 |
    ///
    /// # When to Use
    ///
    /// Use this method when you can cheaply compute the derivative analytically:
    /// - **XIRR/IRR**: Derivative of NPV with respect to rate is known analytically
    /// - **Implied volatility**: Vega (∂Price/∂σ) is available from option pricing
    /// - **Yield-to-maturity**: Duration (∂Price/∂y) is known from bond pricing
    /// - **Calibration**: When instrument sensitivities are already computed
    ///
    /// **Don't use** when:
    /// - Derivative is expensive to compute (use finite diff instead)
    /// - Prototyping (finite diff is simpler initially)
    /// - Function is not smooth (use [`BrentSolver`] instead)
    ///
    /// # Arguments
    ///
    /// * `f` - Function to find the root of (f(x) = 0)
    /// * `f_prime` - Derivative of f with respect to x
    /// * `initial_guess` - Starting point for iteration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::math::solver::NewtonSolver;
    ///
    /// let solver = NewtonSolver::new();
    ///
    /// // Solve x^2 - 4 = 0 with analytic derivative (2x)
    /// let f = |x: f64| x * x - 4.0;
    /// let f_prime = |x: f64| 2.0 * x;
    ///
    /// let root = solver.solve_with_derivative(f, f_prime, 1.0)
    ///     .expect("Root finding should succeed");
    /// assert!((root - 2.0).abs() < 1e-10);
    /// ```
    ///
    /// # References
    ///
    /// - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 9.4.
    ///   "When derivatives are available analytically, Newton-Raphson is the method of choice."
    pub fn solve_with_derivative<F, G>(&self, f: F, f_prime: G, initial_guess: f64) -> Result<f64>
    where
        F: Fn(f64) -> f64,
        G: Fn(f64) -> f64,
    {
        self.newton_method(&f, f_prime, initial_guess)
    }

    /// Core Newton-Raphson method implementation.
    fn newton_method<Func, DFunc>(&self, f: Func, f_prime: DFunc, x0: f64) -> Result<f64>
    where
        Func: Fn(f64) -> f64,
        DFunc: Fn(f64) -> f64,
    {
        use crate::error::InputError;

        tracing::debug!(
            x0,
            tol = self.tolerance,
            max_iter = self.max_iterations,
            "newton: start"
        );

        let mut x = x0;
        let mut last_fx = f64::NAN;
        let mut last_fpx = f64::NAN;

        for iteration in 0..self.max_iterations {
            let fx = f(x);
            last_fx = fx;

            if !fx.is_finite() {
                tracing::debug!(
                    algorithm = "newton",
                    iteration,
                    last_x = x,
                    residual = fx,
                    category = "non_finite_function",
                    "newton: bailout — function returned non-finite value"
                );
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration,
                    residual: fx,
                    last_x: x,
                    reason: format!("function returned non-finite value: {fx}"),
                }
                .into());
            }

            // Residual test: absolute in f-units, scaled by the optional
            // caller-supplied residual_scale (default 1.0).
            if fx.abs() < self.tolerance * self.residual_scale {
                tracing::debug!(iteration, x, residual = fx.abs(), "newton: converged");
                return Ok(x);
            }

            let fpx = f_prime(x);
            last_fpx = fpx;

            if !fpx.is_finite() {
                tracing::debug!(
                    algorithm = "newton",
                    iteration,
                    last_x = x,
                    residual = fx.abs(),
                    derivative = fpx,
                    category = "non_finite_derivative",
                    "newton: bailout — derivative returned non-finite value"
                );
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration,
                    residual: fx.abs(),
                    last_x: x,
                    reason: format!("derivative returned non-finite value: {fpx}"),
                }
                .into());
            }

            // Reject when derivative is too small (absolute OR relative to function value)
            // to prevent divergent Newton steps where f/f' overflows
            if fpx.abs() < self.min_derivative || fpx.abs() < self.min_derivative_rel * fx.abs() {
                tracing::debug!(
                    algorithm = "newton",
                    iteration,
                    last_x = x,
                    residual = fx.abs(),
                    derivative_abs = fpx.abs(),
                    min_derivative = self.min_derivative,
                    category = "derivative_too_small",
                    "newton: bailout — derivative too small for stable Newton step"
                );
                return Err(InputError::SolverConvergenceFailed {
                    iterations: iteration,
                    residual: fx.abs(),
                    last_x: x,
                    reason: format!(
                        "derivative too small: |f'(x)| = {:.6e} < min_derivative = {:.6e}",
                        fpx.abs(),
                        self.min_derivative
                    ),
                }
                .into());
            }

            let x_new = x - fx / fpx;

            // Check for convergence in x. Scale-aware: relative for |x| > 1
            // (a 1e-12 absolute step is below f64 resolution at x ≈ 1e6),
            // absolute near zero so tiny-scale roots are not declared
            // converged prematurely.
            if (x_new - x).abs() < self.tolerance * x_new.abs().max(1.0) {
                return Ok(x_new);
            }

            x = x_new;
        }

        tracing::debug!(
            algorithm = "newton",
            iterations = self.max_iterations,
            last_x = x,
            residual = last_fx.abs(),
            last_derivative = last_fpx,
            tolerance = self.tolerance,
            category = "max_iterations_exceeded",
            "newton: bailout — max iterations reached without convergence"
        );
        Err(InputError::SolverConvergenceFailed {
            iterations: self.max_iterations,
            residual: last_fx.abs(),
            last_x: x,
            reason: format!(
                "max iterations ({}) reached without convergence (tolerance: {:.6e}, last f'(x): {:.6e})",
                self.max_iterations, self.tolerance, last_fpx
            ),
        }
        .into())
    }
}

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
/// use finstack_core::math::solver::{BrentSolver, Solver};
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
///   Prentice-Hall. Chapter 4.
/// - Press, W. H., et al. (2007). *Numerical Recipes: The Art of Scientific Computing*
///   (3rd ed.). Cambridge University Press. Section 9.3.
/// - Forsythe, G. E., Malcolm, M. A., & Moler, C. B. (1977). *Computer Methods
///   for Mathematical Computations*. Prentice-Hall.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
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

    /// Set bracket size using a domain-specific hint.
    ///
    /// This improves convergence speed by using an appropriate initial bracket
    /// for the problem domain.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::math::solver::{BrentSolver, BracketHint, Solver};
    ///
    /// // For implied volatility solving
    /// let solver = BrentSolver::new()
    ///     .bracket_hint(BracketHint::ImpliedVol);
    ///
    /// // For yield-to-maturity solving
    /// let ytm_solver = BrentSolver::new()
    ///     .bracket_hint(BracketHint::Ytm);
    /// ```
    #[must_use]
    pub fn bracket_hint(mut self, hint: BracketHint) -> Self {
        self.initial_bracket_size = Some(hint.to_bracket_size());
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
    /// use finstack_core::math::solver::{BrentSolver, Solver};
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
    fn find_bracket<Func>(&self, f: &Func, initial_guess: f64) -> Result<(f64, f64)>
    where
        Func: Fn(f64) -> f64,
    {
        use crate::error::InputError;

        // Use configurable bounds
        let max_bracket_width = self.bracket_max - self.bracket_min;

        // Calculate adaptive initial bracket size
        let initial_size = self.initial_bracket_size.unwrap_or_else(|| {
            // Use 1% of the initial guess magnitude, with a minimum of 0.01
            let adaptive_size = initial_guess.abs() * 0.01;
            if adaptive_size < 1e-6 {
                0.01 // Fallback for values near zero
            } else {
                adaptive_size.min(1.0) // Cap at 1.0 for very large initial guesses
            }
        });

        let mut a = initial_guess - initial_size;
        let mut b = initial_guess + initial_size;
        let mut expansion_iterations = 0;

        // Expand bracket until we find a sign change
        for _ in 0..20 {
            expansion_iterations += 1;
            let fa = f(a);
            let fb = f(b);

            // Check for non-finite function values
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

            if fa * fb < 0.0 {
                return Ok((a, b));
            }

            // Expand bracket with overflow protection
            let width = b - a;

            // Stop if bracket is unreasonably wide
            if width > max_bracket_width {
                break;
            }

            // Expand with bounds checking to prevent overflow
            a = (a - width * self.bracket_expansion).max(self.bracket_min);
            b = (b + width * self.bracket_expansion).min(self.bracket_max);

            // Stop if we've hit the bounds
            if a <= self.bracket_min && b >= self.bracket_max {
                break;
            }
        }

        // Compute final values - check for sign change one more time
        let fa = f(a);
        let fb = f(b);

        // Check for non-finite values at final bounds
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
        if fa * fb < 0.0 {
            return Ok((a, b));
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

impl Solver for BrentSolver {
    fn solve<Func>(&self, f: Func, initial_guess: f64) -> Result<f64>
    where
        Func: Fn(f64) -> f64,
    {
        let (a, b) = self.find_bracket(&f, initial_guess)?;
        self.brent_method(f, a, b)
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
    /// # Errors
    ///
    /// Returns an error if the bracket is invalid (same-sign endpoints,
    /// non-finite evaluations) or Brent's method fails to converge within
    /// `max_iterations`.
    pub fn solve_in_bracket<Func>(&self, f: Func, a: f64, b: f64) -> Result<f64>
    where
        Func: FnMut(f64) -> f64,
    {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.brent_method(f, lo, hi)
    }
}

impl BrentSolver {
    /// Core Brent's method implementation.
    ///
    /// Requirements: `f(lo)` and `f(hi)` must have opposite signs.
    fn brent_method<Func>(&self, mut f: Func, lo: f64, hi: f64) -> Result<f64>
    where
        Func: FnMut(f64) -> f64,
    {
        use crate::error::InputError;

        tracing::debug!(
            lo,
            hi,
            tol = self.tolerance,
            max_iter = self.max_iterations,
            "brent: start"
        );

        let flo = f(lo);
        let fhi = f(hi);
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
    fn test_newton_solver() {
        let solver = NewtonSolver::new();

        // Solve x^2 - 2 = 0 (root should be sqrt(2))
        let f = |x: f64| x * x - 2.0;
        let root = solver
            .solve(f, 1.0)
            .expect("Root finding should succeed in test");

        assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
    }

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

    // ===== Phase 1 Robustness Tests =====

    #[test]
    fn test_newton_scale_robustness() {
        // Test solver across magnitude scales 10^-6 to 10^6
        let solver = NewtonSolver::new();

        for exp in -6..=6 {
            let target = 10f64.powi(exp);
            let f = |x: f64| x * x - target;
            let root = solver
                .solve(f, target.sqrt() * 0.9)
                .unwrap_or_else(|_| panic!("Root finding should succeed at scale 10^{}", exp));

            // Use scale-aware tolerance accounting for floating point precision
            // For small targets, absolute error dominates; for large targets, relative error
            let abs_tolerance: f64 = 1e-12; // Absolute tolerance near machine precision
            let rel_tolerance = 1e-10 * target; // Relative tolerance
            let tolerance = abs_tolerance.max(rel_tolerance);

            assert!(
                (f(root)).abs() < tolerance,
                "Failed at scale 10^{}: residual {} (tolerance {})",
                exp,
                f(root),
                tolerance
            );
        }
    }

    #[test]
    fn test_newton_shallow_slope() {
        // Test Newton solver with shallow derivative but valid root
        let solver = NewtonSolver::new();

        // f(x) = x^4 - 1e-8, root at x ≈ 0.01
        // At x=0.01: f'(x) = 4x^3 = 4e-6 (was rejected by legacy guard)
        let f = |x: f64| x.powi(4) - 1e-8;
        let root = solver
            .solve(f, 0.02)
            .expect("Should solve function with shallow derivative");

        assert!((f(root)).abs() < 1e-12, "Residual: {}", f(root));
        assert!((root - 0.01).abs() < 1e-6, "Root: {}", root);
    }

    #[test]
    fn test_newton_relative_step_converges_at_large_scale() {
        // Root at x = 1e6. With a purely absolute 1e-12 step test the Newton
        // step |dx| can never get below 1e-12 at x ≈ 1e6 (f64 resolution there
        // is ~1.2e-10), so the solver would exhaust iterations. The scale-aware
        // step test |dx| < tol·max(1, |x|) converges.
        let solver = NewtonSolver::new().residual_scale(1e12);
        let f = |x: f64| x * x - 1e12; // root at 1e6, dollar-scale residuals
        let root = solver
            .solve(f, 9.0e5)
            .expect("scale-aware Newton should converge for a root at 1e6");
        assert!((root - 1e6).abs() < 1e-4, "root {root} should be near 1e6");
    }

    #[test]
    fn test_newton_tiny_scale_root_not_falsely_converged() {
        // Root at x = 1e-8. The step test is absolute (tol·max(1,|x|) = tol)
        // near zero, so a tiny-scale problem must still resolve the root
        // accurately rather than declaring convergence on a relatively-large
        // step. residual_scale stays at the default 1.0.
        let solver = NewtonSolver::new();
        let f = |x: f64| x - 1e-8;
        let root = solver
            .solve(f, 1.0)
            .expect("Newton should converge for tiny-scale linear root");
        assert!(
            (root - 1e-8).abs() < 1e-12,
            "root {root} must resolve the tiny-scale root, not stop early"
        );
    }

    #[test]
    fn test_newton_residual_scale_builder() {
        // residual_scale loosens only the residual test, in f-units.
        let f = |x: f64| 1e6 * (x - 2.0);
        // Default residual_scale=1.0: residual test needs |f| < 1e-12 which is
        // achievable here via the step test; both should agree on the root.
        let strict = NewtonSolver::new().solve(f, 1.0).expect("converges");
        let scaled = NewtonSolver::new()
            .residual_scale(1e6)
            .solve(f, 1.0)
            .expect("converges");
        assert!((strict - 2.0).abs() < 1e-9);
        assert!((scaled - 2.0).abs() < 1e-9);
    }

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
    fn test_newton_adaptive_fd_step() {
        // Verify adaptive FD step prevents cancellation errors
        let solver = NewtonSolver::new();

        // Large-scale problem: x^2 = 1,000,000
        let f_large = |x: f64| x * x - 1_000_000.0;
        let root_large = solver
            .solve(f_large, 900.0)
            .expect("Should solve large-scale problem");
        assert!((root_large - 1000.0).abs() < 1e-6);

        // Small-scale problem: x^2 = 0.000001
        let f_small = |x: f64| x * x - 1e-6;
        let root_small = solver
            .solve(f_small, 0.0009)
            .expect("Should solve small-scale problem");
        assert!((root_small - 0.001).abs() < 1e-9);
    }

    #[test]
    fn test_solver_convergence_comparison() {
        // Compare Newton vs Brent on well-behaved function
        let newton = NewtonSolver::new();
        let brent = BrentSolver::new();

        let f = |x: f64| x * x * x - x - 1.0; // Root ≈ 1.3247

        let root_newton = newton.solve(f, 1.0).expect("Newton should converge");
        let root_brent = brent.solve(f, 1.0).expect("Brent should converge");

        // Both should find the same root
        assert!((root_newton - root_brent).abs() < 1e-6);
        assert!((f(root_newton)).abs() < 1e-10);
    }

    #[test]
    fn test_configurable_min_derivative() {
        // Test that min_derivative threshold is configurable
        let _solver_strict = NewtonSolver::new().min_derivative(1e-10);
        let solver_permissive = NewtonSolver::new().min_derivative(1e-16);

        // Function with very small derivative
        let f = |x: f64| x.powi(5) - 1e-12;

        // Permissive solver should succeed where strict might fail
        let root = solver_permissive
            .solve(f, 0.001)
            .expect("Permissive solver should handle shallow slopes");
        assert!((f(root)).abs() < 1e-12);
    }

    #[test]
    fn test_solve_with_derivative_quadratic() {
        // Test analytic derivative on simple quadratic
        let solver = NewtonSolver::new();

        // Solve x^2 - 4 = 0 (root at x = 2)
        let f = |x: f64| x * x - 4.0;
        let f_prime = |x: f64| 2.0 * x;

        let root = solver
            .solve_with_derivative(f, f_prime, 1.0)
            .expect("Root finding with analytic derivative should succeed");

        assert!((root - 2.0).abs() < 1e-10);
        assert!((f(root)).abs() < 1e-10);
    }

    #[test]
    fn test_solve_with_derivative_vs_finite_diff() {
        // Compare analytic derivative to finite difference on same function
        let solver = NewtonSolver::new();

        // Cubic function: x^3 - 2x - 5 = 0
        let f = |x: f64| x.powi(3) - 2.0 * x - 5.0;
        let f_prime = |x: f64| 3.0 * x.powi(2) - 2.0;

        let root_analytic = solver
            .solve_with_derivative(f, f_prime, 2.0)
            .expect("Analytic derivative should succeed");

        let root_fd = solver
            .solve(f, 2.0)
            .expect("Finite difference should succeed");

        // Both should converge to same root
        assert!(
            (root_analytic - root_fd).abs() < 1e-9,
            "Analytic and FD roots differ: {} vs {}",
            root_analytic,
            root_fd
        );
        assert!((f(root_analytic)).abs() < 1e-10);
    }

    #[test]
    fn test_solve_with_derivative_exponential() {
        // Test on transcendental equation: e^x - 3x = 0
        let solver = NewtonSolver::new();

        let f = |x: f64| x.exp() - 3.0 * x;
        let f_prime = |x: f64| x.exp() - 3.0;

        let root = solver
            .solve_with_derivative(f, f_prime, 1.0)
            .expect("Should solve exponential equation");

        assert!((f(root)).abs() < 1e-10);
        // One root is around x ≈ 0.619 (there's also one near 1.512)
        assert!(root > 0.0 && root < 2.0);
    }

    #[test]
    fn test_brent_max_iterations_returns_error() {
        // Test that Brent solver returns an error when max iterations is reached
        // without convergence, rather than silently returning a non-root value.

        // Create a solver with very few iterations so it won't converge
        let solver = BrentSolver::new().max_iterations(2).tolerance(1e-15); // Extremely tight tolerance

        // A function that converges slowly (root at x ≈ 1.3247)
        let f = |x: f64| x * x * x - x - 1.0;

        let result = solver.solve(f, 1.0);

        // Should return an error, not a potentially incorrect result
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
    fn test_bracket_hint_xirr() {
        // Test that BracketHint::Xirr produces the expected bracket size
        assert_eq!(BracketHint::Xirr.to_bracket_size(), 0.5);

        // Test IRR-like problem with wide range of possible roots
        // NPV = -100 + 250/(1+r)^1 = 0 => r = 1.5 (150% return)
        let solver = BrentSolver::new()
            .bracket_hint(BracketHint::Xirr)
            .bracket_bounds(-0.99, 10.0); // Allow extreme returns

        let npv = |r: f64| -100.0 + 250.0 / (1.0 + r);
        let irr = solver.solve(npv, 0.1).expect("Should find IRR");
        assert!(
            (irr - 1.5).abs() < 1e-10,
            "Expected IRR of 1.5 (150%), got {}",
            irr
        );

        // Test negative IRR scenario
        // NPV = -100 + 50/(1+r)^1 = 0 => r = -0.5 (-50% return)
        let npv_loss = |r: f64| -100.0 + 50.0 / (1.0 + r);
        let irr_loss = solver
            .solve(npv_loss, 0.1)
            .expect("Should find negative IRR");
        assert!(
            (irr_loss - (-0.5)).abs() < 1e-10,
            "Expected IRR of -0.5 (-50%), got {}",
            irr_loss
        );
    }

    #[test]
    fn test_all_bracket_hints() {
        // Verify all bracket hints produce reasonable values
        assert_eq!(BracketHint::ImpliedVol.to_bracket_size(), 0.2);
        assert_eq!(BracketHint::Rate.to_bracket_size(), 0.02);
        assert_eq!(BracketHint::Spread.to_bracket_size(), 0.005);
        assert_eq!(BracketHint::Ytm.to_bracket_size(), 0.02);
        assert_eq!(BracketHint::Xirr.to_bracket_size(), 0.5);
        assert_eq!(BracketHint::Custom(0.123).to_bracket_size(), 0.123);
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

        let result = solver.brent_method(f, 0.0, 2.0);
        assert!(
            result.is_err(),
            "Brent should reject non-finite values encountered during iteration"
        );
    }
}
