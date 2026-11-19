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
//! use finstack_quant_core::math::solver::BrentSolver;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let root = BrentSolver::new(1e-9, 100)?.solve(|x| Ok(x * x - 2.0), 1.0, 0.0, 10.0, 0.5, 20)?;
//! assert!((root - 2f64.sqrt()).abs() < 1e-9);
//! # Ok(())
//! # }
//! ```
//!
//! ## Basic statistics
//!
//! ```rust
//! use finstack_quant_core::math::stats::{mean, population_variance, variance};
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let data = [1.0, 2.0, 3.0, 4.0];
//! assert_eq!(mean(&data)?, Some(2.5));
//! assert_eq!(population_variance(&data)?, Some(1.25));
//! assert!((variance(&data)?.unwrap() - 5.0 / 3.0).abs() < 1e-10);
//! # Ok(())
//! # }
//! ```

/// Tolerance for checking if a value is effectively zero.
///
/// Used across the workspace for near-zero guards, safe division, and approximate
/// equality comparisons. Value: 1e-10 (well above f64 machine epsilon ~2.2e-16
/// but small enough to catch actual zeros vs meaningful small values).
pub const ZERO_TOLERANCE: f64 = 1e-10;

/// Quantize a finite binary64 number to decimal places, rounding ties away from zero.
/// The decimal target is converted once to the nearest binary64 value.
/// Returns an error if that conversion overflows.
///
/// # Arguments
///
/// * `x` - Finite input as represented in binary64. A rounded zero retains its sign.
/// * `digits` - Decimal places to retain. Negative values round to multiples of ten.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::round_half_away;
/// assert_eq!(round_half_away(-2.5, 0)?, -3.0);
/// assert_eq!(round_half_away(2.675, 2)?, 2.67);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn round_half_away(x: f64, digits: i32) -> crate::Result<f64> {
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::ToPrimitive;

    if !x.is_finite() {
        return Err(crate::Error::Validation(
            "round requires a finite value".into(),
        ));
    }
    if digits == 0 {
        return Ok(x.round());
    }
    if digits >= 324 || (digits > 0 && x.fract() == 0.0) {
        return Ok(x);
    }
    if digits <= -309 {
        return Ok(0.0_f64.copysign(x));
    }
    let magnitude = BigRational::from_float(x.abs())
        .ok_or_else(|| crate::Error::Validation("round requires a finite value".into()))?;
    let power = BigInt::from(10u8).pow(digits.unsigned_abs());
    let rounded = if digits > 0 {
        (magnitude * &power).round() / power
    } else {
        (magnitude / &power).round() * power
    };
    rounded
        .to_f64()
        .filter(|v| v.is_finite())
        .map(|v| v.copysign(x))
        .ok_or_else(|| crate::Error::Validation("rounded result exceeds binary64 range".into()))
}

/// Clamp a finite value to an inclusive, ordered interval.
///
/// # Arguments
///
/// * `x` - Finite value to restrict. An in-range value retains its zero sign.
/// * `lo` - Finite inclusive lower bound.
/// * `hi` - Finite inclusive upper bound, at least `lo`.
pub fn clamp(x: f64, lo: f64, hi: f64) -> crate::Result<f64> {
    if !x.is_finite() || !lo.is_finite() || !hi.is_finite() || lo > hi {
        return Err(crate::Error::Validation(
            "clamp requires finite values and ordered bounds".into(),
        ));
    }
    if (lo..=hi).contains(&x) {
        return Ok(x);
    }
    Ok(if x < lo { lo } else { hi })
}

mod compounding;
pub mod distributions;
pub mod fractional;
pub mod integration;
pub mod interp;
pub mod linalg;
pub(crate) mod log_ratio;
pub mod piecewise;
pub mod probability;
pub mod random;
pub mod solver;
pub mod solver_multi;
pub mod special_functions;
pub mod stats;
pub mod summation;
pub mod time_grid;
pub mod units;

pub use compounding::{Compounding, z_spread_discount_factor};
