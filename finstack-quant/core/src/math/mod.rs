//! Numerical helpers: root finding, summation, statistics, distributions, and mathematical functions.
//!
//! The implementations favor deterministic numerical behavior and explicit
//! error returns for invalid inputs.
//!
//! # Root Finding
//!
//! The `solver` module provides multiple root-finding algorithms:
//! - `NewtonSolver`: Newton iteration with finite-difference or analytic derivatives
//! - `BrentSolver`: bracketed root finding by bisection, secant, and inverse quadratic steps
//!
//! When analytic derivatives are available, `NewtonSolver::solve_with_derivative`
//! avoids finite-difference derivative estimates.
//!
//! ## Solver Selection Guide
//!
//! ### 1D Root Finding (`solver` module)
//!
//! | Use case | Solver | Method | Notes |
//! |----------|-------------------|--------|-----|
//! | **Implied volatility** | `NewtonSolver` | `solve_with_derivative()` | Use when vega is available |
//! | **Yield-to-maturity** | `NewtonSolver` | `solve_with_derivative()` | Use when duration is available |
//! | **IRR/XIRR** | `NewtonSolver` | `solve_with_derivative()` | Uses analytic d(NPV)/dr |
//! | **Bracketed roots** | `BrentSolver` | `solve()` | Requires a sign-changing bracket |
//! | **Smooth function, no derivatives** | `NewtonSolver` | `solve()` | Uses finite differences |
//!
//! ### Multi-Dimensional Optimization (`solver_multi` module)
//!
//! | Use case | Method | Notes |
//! |----------|-------------------|-----|
//! | **SABR calibration** | `solve_system_with_dim_stats()` | System of market quotes, returns stats |
//! | **Curve bootstrapping** | `solve_system_with_jacobian_stats()` | Use when analytic sensitivities are available |
//! | **Simple minimization** | `minimize()` | Scalar objective function |
//! | **With known Jacobian** | `solve_system_with_jacobian_stats()` | Avoids numerical Jacobian estimates |
//!
//!
//! ### Performance Trade-offs
//!
//! Analytic derivatives avoid finite-difference noise when the derivative or
//! Jacobian is already available. Finite differences are useful for black-box
//! objectives where only function values are exposed.
//!
//! # Examples
//!
//! ## Root finding with finite differences
//!
//! ```rust
//! use finstack_quant_core::math::{Solver, mean, variance};
//! use finstack_quant_core::math::solver::NewtonSolver;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let solver = NewtonSolver::new();
//! let root = solver.solve(|x| x * x - 2.0, 1.0)?;
//! assert!((root - 2f64.sqrt()).abs() < 1e-9);
//! # Ok(())
//! # }
//! ```
//!
//! ## Root finding with analytic derivatives
//!
//! ```rust
//! use finstack_quant_core::math::solver::NewtonSolver;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let solver = NewtonSolver::new();
//! let f = |x: f64| x * x - 2.0;
//! let f_prime = |x: f64| 2.0 * x;  // Analytic derivative
//!
//! let root = solver.solve_with_derivative(f, f_prime, 1.0)?;
//! assert!((root - 2f64.sqrt()).abs() < 1e-10);
//! # Ok(())
//! # }
//! ```
//!
//! ## Basic statistics
//!
//! ```rust
//! use finstack_quant_core::math::{mean, variance, population_variance};
//!
//! let data = [1.0, 2.0, 3.0, 4.0];
//! assert_eq!(mean(&data), 2.5);
//! assert_eq!(population_variance(&data), 1.25);
//! assert!((variance(&data) - 5.0 / 3.0).abs() < 1e-10);
//! ```

/// Tolerance for checking if a value is effectively zero.
///
/// Used across the workspace for near-zero guards, safe division, and approximate
/// equality comparisons. Value: 1e-10 (well above f64 machine epsilon ~2.2e-16
/// but small enough to catch actual zeros vs meaningful small values).
pub const ZERO_TOLERANCE: f64 = 1e-10;

pub mod characteristic_function;
pub mod compounding;
/// Consecutive streak counter for return series analysis.
pub mod consecutive;
pub mod distributions;
pub mod fractional;
pub mod integration;
pub mod interp;
pub mod linalg;
pub mod piecewise;
pub mod probability;
pub mod random;
pub mod solver;
pub mod solver_multi;
pub mod special_functions;
pub mod stats;
pub mod summation;
pub mod time_grid;
pub mod volatility;

// Re-exports for ergonomic access
pub use distributions::{
    binomial_pmf_all, binomial_pmf_all_into, binomial_probability, chi_squared_cdf,
    chi_squared_quantile, log_binomial_coefficient, log_factorial,
};
pub use integration::{
    gauss_legendre_integrate, gauss_legendre_integrate_adaptive,
    gauss_legendre_integrate_composite, GaussHermiteQuadrature, GaussLaguerreQuadrature,
};
pub use interp::{
    CubicHermiteStrategy, ExtrapolationPolicy, InterpFn, Interpolator, LinearStrategy,
    LogLinearStrategy, MonotoneConvexStrategy, PiecewiseQuadraticForwardStrategy,
};
pub use linalg::{
    apply_correlation, build_correlation_matrix, cholesky_correlation, cholesky_decomposition,
    symmetric_eigen, validate_correlation_matrix, CholeskyError, CorrelationFactor,
};
pub use probability::{correlation_bounds, joint_probabilities, CorrelatedBernoulli};
pub use random::sobol::{SobolRng, MAX_SOBOL_DIMENSION};
pub use random::{box_muller_transform, Pcg64Rng, RandomNumberGenerator};
// Raw root finding functions are no longer exported - use trait-based solvers instead
pub use compounding::Compounding;
pub use consecutive::count_consecutive;
pub use solver::{BracketHint, BrentSolver, NewtonSolver, Solver};
pub use solver_multi::{AnalyticalDerivatives, LevenbergMarquardtSolver};
pub use special_functions::{
    erf, ln_gamma, norm_cdf, norm_pdf, standard_normal_inv_cdf, student_t_cdf, student_t_inv_cdf,
};
pub use stats::{
    correlation, covariance, finite_count, finite_max_or_nan, finite_min_or_nan, mean, mean_or_nan,
    mean_var, median_or_nan, moment_match, population_variance, quantile, quantile_linear_or_nan,
    required_samples, sample_std_or_nan, sample_variance_or_nan, variance, OnlineCovariance,
    OnlineStats,
};
pub use summation::{kahan_sum, neumaier_sum, NeumaierAccumulator};
pub use time_grid::{
    map_date_to_step, map_dates_to_steps, map_exercise_dates_to_steps, TimeGrid, TimeGridError,
};
