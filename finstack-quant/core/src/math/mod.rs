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
mod rounding;
pub mod solver;
pub mod solver_multi;
pub mod special_functions;
pub mod stats;
pub mod summation;
pub mod time_grid;
pub mod units;

pub use compounding::{Compounding, z_spread_discount_factor};
pub use rounding::{ZERO_TOLERANCE, clamp, round_half_away};
