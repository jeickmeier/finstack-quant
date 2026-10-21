//! Tolerance constants for numerical comparisons in tests.
//!
//! This is the one tolerance namespace of the test-support crate. Use the
//! named tiers instead of ad-hoc literals so tests stay consistent.
//!
//! # Tolerance Hierarchy (strictest to loosest)
//!
//! | Constant | Value | Use Case |
//! |----------|-------|----------|
//! | `TIGHT` | 1e-10 | Bit-near identities, roundtrips, exact analytical checks |
//! | `ANALYTICAL` | 1e-6 (0.0001%) | Closed-form solutions (put-call parity, zero-coupon YTM) |
//! | `NUMERICAL` | 1e-4 (0.01%) | Iterative methods (Newton-Raphson, tree pricing) |
//! | `CURVE_PRICING` | 5e-3 (0.5%) | Curve-based valuations with convention differences |
//! | `RELATIVE` | 1e-2 (1%) | Proportional comparisons, textbook benchmarks |
//! | `BUMP_VS_ANALYTICAL` | 1.5e-2 (1.5%) | Bump-and-reprice vs analytical approximations |
//! | `STATISTICAL` | 2e-2 (2%) | Monte Carlo, statistical tests and FD greeks |
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_valuations_test_support::tolerances;
//!
//! let (actual, expected): (f64, f64) = (1.0 + 1e-8, 1.0);
//! assert!((actual - expected).abs() < tolerances::ANALYTICAL);
//! ```

/// Tight tolerance for bit-near or exact analytical comparisons.
///
/// Use when comparing identical calculations, roundtrip
/// serialization/deserialization, or exact closed-form identities.
pub const TIGHT: f64 = 1e-10;

/// Analytical calculations (e.g., put-call parity, zero-coupon YTM).
/// These have closed-form solutions and should be very precise.
pub const ANALYTICAL: f64 = 1e-6; // 0.0001%

/// Numerical methods (e.g., tree pricing, Newton-Raphson solvers).
/// These involve iterative convergence and may have small residual errors.
pub const NUMERICAL: f64 = 1e-4; // 0.01%

/// Curve-based pricing with potential convention mismatches.
/// Accounts for compounding convention differences (e.g., semi-annual vs continuous).
pub const CURVE_PRICING: f64 = 5e-3; // 0.5%

/// Relative tolerance for scaling comparisons.
/// Used when comparing proportional changes across different scales.
pub const RELATIVE: f64 = 1e-2; // 1%

/// Bump-and-reprice vs analytical approximation comparisons.
///
/// Used when comparing numerical bump-based sensitivities (e.g., DV01 computed
/// via curve parallel shift) against analytical approximations (e.g., DV01 ≈
/// -Price × ModDur × 0.0001). The difference arises from:
/// - Compounding convention mismatch (continuous vs periodic): ~0.6%
/// - Curve-based vs yield-based rate definitions
/// - Convexity effects (negligible for 1bp bumps)
///
/// For par bonds on flat curves, actual differences are typically 0.5-1.5%.
pub const BUMP_VS_ANALYTICAL: f64 = 1.5e-2; // 1.5%

/// Statistical/Monte Carlo methods and finite-difference greeks.
/// These have inherent sampling or discretisation error.
pub const STATISTICAL: f64 = 2e-2; // 2%

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerance_tiers_are_ordered() {
        const _: () = {
            assert!(TIGHT < ANALYTICAL);
            assert!(ANALYTICAL < NUMERICAL);
            assert!(NUMERICAL < CURVE_PRICING);
            assert!(CURVE_PRICING < RELATIVE);
            assert!(RELATIVE < BUMP_VS_ANALYTICAL);
            assert!(BUMP_VS_ANALYTICAL < STATISTICAL);
        };
    }
}
