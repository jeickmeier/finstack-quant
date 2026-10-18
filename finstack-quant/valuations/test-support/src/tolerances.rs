//! Tolerance constants for numerical comparisons in risk tests.
//!
//! **This is the canonical location for tolerance constants in the test suite.**
//!
//! Using named constants instead of magic numbers improves readability
//! and ensures consistency across tests.
//!
//! # Tolerance Hierarchy (strictest to loosest)
//!
//! | Constant      | Value  | Use Case                                        |
//! |---------------|--------|-------------------------------------------------|
//! | `TIGHT`       | 1e-10  | Analytical solutions, roundtrip identity checks |
//! | `STANDARD`    | 1e-6   | Finite-difference vs analytical comparisons     |
//! | `LOOSE`       | 1e-3   | Monte Carlo, cross-methodology comparisons      |
//!
//! # Relative Tolerances
//!
//! | Constant      | Value  | Percentage | Use Case                         |
//! |---------------|--------|------------|----------------------------------|
//! | `PERCENT_001` | 0.0001 | 0.01%      | High-precision relative checks   |
//! | `PERCENT_01`  | 0.001  | 0.1%       | Standard relative comparisons    |
//! | `PERCENT_1`   | 0.01   | 1%         | Loose FD approximations          |
//! | `PERCENT_5`   | 0.05   | 5%         | Second-order greeks via FD       |
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use finstack_quant_valuations_test_support::tolerances;
//! use finstack_quant_test_utils::assert::approx_eq;
//!
//! // For analytical put-call parity check
//! approx_eq(call - put, forward - strike_pv, tolerances::TIGHT, "put-call parity");
//!
//! // For finite-difference delta approximation
//! approx_eq(fd_delta, analytical_delta, tolerances::STANDARD, "delta");
//!
//! // For Monte Carlo pricing
//! approx_eq(mc_price, analytical_price, tolerances::LOOSE, "mc price");
//! ```
//!
//! # Migration Note
//!
//! If you encounter `F64_ABS_TOL_STRICT` or `F64_ABS_TOL_LOOSE` in older code,
//! migrate to this module's constants:
//! - `F64_ABS_TOL_STRICT` (1e-12) → use `TIGHT` (1e-10)
//! - `F64_ABS_TOL_LOOSE` (1e-10) → use `TIGHT` (1e-10)

/// Tight tolerance for bit-exact or analytical comparisons.
///
/// Use when comparing:
/// - Identical calculations that should produce exact results
/// - Analytical formulas (e.g., put-call parity, zero-coupon YTM)
/// - Roundtrip serialization/deserialization
pub const TIGHT: f64 = 1e-10;

/// Standard tolerance for finite-difference vs analytical comparisons.
///
/// Use when comparing:
/// - FD approximations to analytical values
/// - Newton-Raphson solver results
/// - Tree pricing vs closed-form
pub const STANDARD: f64 = 1e-6;

/// Loose tolerance for Monte Carlo or cross-methodology comparisons.
///
/// Use when comparing:
/// - MC results to analytical solutions
/// - Different pricing methodologies
/// - Cross-library validation (with expected methodology differences)
pub const LOOSE: f64 = 1e-3;

/// 0.01% relative tolerance.
///
/// Use for high-precision relative comparisons where the absolute
/// magnitude varies but relative accuracy should be very tight.
pub const PERCENT_001: f64 = 0.0001;

/// 0.1% relative tolerance.
///
/// Use for standard relative comparisons.
pub const PERCENT_01: f64 = 0.001;

/// 1% relative tolerance.
///
/// Use for loose relative comparisons (e.g., FD approximations,
/// textbook benchmark comparisons).
pub const PERCENT_1: f64 = 0.01;

/// 5% relative tolerance.
///
/// Use for very loose comparisons where significant differences are expected,
/// such as second-order greeks computed via finite differences.
pub const PERCENT_5: f64 = 0.05;

/// Small absolute threshold for near-zero checks.
///
/// Use when determining if a value is effectively zero,
/// particularly for relative tolerance fallback logic.
pub const NEAR_ZERO: f64 = 1e-8;
