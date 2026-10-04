//! Numerical helpers: root finding, summation, statistics, distributions, and mathematical functions.
//!
//! The implementations favor deterministic numerical behavior and explicit
//! error returns for invalid inputs.
//!
//! # Root Finding
//!
//! [`BrentSolver`](crate::math::solver::BrentSolver) is the scalar root finder: `solve()` searches for a
//! sign-changing bracket around an initial guess and `solve_in_bracket()` takes
//! one directly. It is derivative-free, so it serves implied volatility,
//! yield, spread and rate-of-return solves alike.
//!
//! [`LevenbergMarquardtSolver`](crate::math::solver_multi::LevenbergMarquardtSolver) solves multi-dimensional least-squares systems
//! (SABR calibration, global curve fits), with a finite-difference or
//! caller-supplied Jacobian.
//!
//! # Examples
//!
//! ## Root finding
//!
//! ```rust
//! use finstack_quant_core::math::BrentSolver;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let root = BrentSolver::new().solve(|x| x * x - 2.0, 1.0)?;
//! assert!((root - 2f64.sqrt()).abs() < 1e-9);
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

/// Round to `digits` decimal places with ties rounding half away from zero
/// (Excel convention): `round_half_away(2.5, 0)` is 3.0 and
/// `round_half_away(-2.5, 0)` is -3.0.
///
/// This is the shared implementation behind the expression-language `round`
/// function; the core vector evaluator and the statements scalar evaluator
/// both delegate here so their semantics cannot drift.
///
/// # Arguments
///
/// * `x` - Value to round; non-finite inputs (NaN, ±inf) pass through
///   unchanged.
/// * `digits` - Number of decimal places to keep. `0` rounds to an integer;
///   negative values round to the left of the decimal point
///   (`round_half_away(1234.0, -2)` is 1200.0).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::round_half_away;
///
/// assert_eq!(round_half_away(2.5, 0), 3.0);
/// assert_eq!(round_half_away(-2.5, 0), -3.0);
/// assert_eq!(round_half_away(3.14159, 2), 3.14);
/// assert_eq!(round_half_away(1234.0, -2), 1200.0);
/// ```
#[must_use]
pub fn round_half_away(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    if digits == 0 {
        return x.round();
    }
    let scale = 10f64.powi(digits);
    let scaled = x * scale;
    if !scaled.is_finite() {
        // Scaling overflowed (huge x with large positive digits); the input
        // is already exact at this precision.
        return x;
    }
    scaled.round() / scale
}

/// Clamp `x` to the inclusive range `[lo, hi]`, returning NaN instead of
/// panicking on invalid input.
///
/// This is the shared implementation behind the expression-language `clamp`
/// function. Unlike [`f64::clamp`], which panics when `lo > hi`, this
/// returns NaN for an inverted range and propagates NaN from any argument,
/// so untrusted formula input can never abort the process.
///
/// # Arguments
///
/// * `x` - Value to restrict to the range; returned unchanged when already
///   inside `[lo, hi]`, otherwise replaced by the nearer bound.
/// * `lo` - Inclusive lower bound; must be `<= hi` for a non-NaN result.
/// * `hi` - Inclusive upper bound; must be `>= lo` for a non-NaN result.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::clamp_or_nan;
///
/// assert_eq!(clamp_or_nan(5.0, 0.0, 10.0), 5.0);
/// assert_eq!(clamp_or_nan(-1.0, 0.0, 10.0), 0.0);
/// assert_eq!(clamp_or_nan(11.0, 0.0, 10.0), 10.0);
/// assert!(clamp_or_nan(5.0, 10.0, 0.0).is_nan()); // inverted range
/// assert!(clamp_or_nan(f64::NAN, 0.0, 10.0).is_nan());
/// ```
#[must_use]
pub fn clamp_or_nan(x: f64, lo: f64, hi: f64) -> f64 {
    if x.is_nan() || lo.is_nan() || hi.is_nan() || lo > hi {
        return f64::NAN;
    }
    x.min(hi).max(lo)
}

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

pub use compounding::Compounding;
pub use consecutive::{count_consecutive, longest_positive_run};
pub use distributions::{
    binomial_pmf_all, binomial_pmf_all_into, binomial_probability, chi_squared_quantile,
    log_factorial,
};
pub use integration::{
    gauss_legendre_grid, gauss_legendre_integrate, gauss_legendre_integrate_adaptive,
    gauss_legendre_integrate_composite, GaussHermiteQuadrature,
};
pub use interp::{
    CubicHermiteStrategy, ExtrapolationPolicy, Interpolator, LinearStrategy, LogLinearStrategy,
    MonotoneConvexStrategy, PiecewiseQuadraticForwardStrategy,
};
pub use linalg::{
    apply_lower_triangular, check_correlation_matrix, cholesky_correlation, cholesky_decomposition,
    symmetric_eigen, validate_correlation_matrix, CholeskyError, CorrelationError,
    CorrelationFactor,
};
pub use probability::{correlation_bounds, joint_probabilities, CorrelatedBernoulli};
pub use random::sobol::{SobolRng, MAX_SOBOL_DIMENSION};
pub use random::{box_muller_transform, Pcg64Rng, RandomNumberGenerator};
pub use solver::BrentSolver;
pub use solver_multi::{AnalyticalDerivatives, LevenbergMarquardtSolver};
pub use special_functions::{
    erf, ln_gamma, log_norm_cdf, norm_cdf, norm_pdf, standard_normal_inv_cdf, student_t_cdf,
    student_t_inv_cdf,
};
pub use stats::{
    correlation, covariance, finite_count, finite_max_or_nan, finite_min_or_nan, mean, mean_or_nan,
    mean_var, median_or_nan, population_variance, quantile, quantile_linear_or_nan,
    sample_std_or_nan, sample_variance_or_nan, variance, OnlineCovariance, OnlineStats,
};
pub use summation::{kahan_sum, neumaier_sum, NeumaierAccumulator};
pub use time_grid::{map_date_to_step, map_exercise_dates_to_steps, TimeGrid, TimeGridError};
