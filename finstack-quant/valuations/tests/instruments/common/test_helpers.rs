//! Shared test helpers and fixtures for common module tests.
//!
//! Provides:
//! - Standard market data fixtures
//! - Test curve builders (discount, forward, hazard, vol surfaces)
//! - Comparison utilities with tolerance
//! - Common test scenarios
//! - Standardized test dates
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::common::test_helpers::*;
//!
//! // Use standard test dates
//! let as_of = dates::TODAY;
//! let maturity = dates::five_years_hence();
//!
//! // Build curves with default day count
//! let disc = flat_discount_curve(0.05, as_of, "USD-OIS");
//!
//! // Build curves with custom day count
//! let disc = flat_discount_curve_with_dc(0.05, as_of, "USD-OIS", DayCount::Act365F);
//!
//! // Use tolerance presets
//! assert_approx_eq_with_config(actual, expected, tolerances::NUMERICAL, "test");
//! ```

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use time::macros::date;
use time::Month;

/// Tolerance tiers for different test categories.
///
/// Use these standardized tolerances instead of ad-hoc hardcoded values
/// to ensure consistency across the test suite.
///
/// ## Tolerance Hierarchy (strictest to loosest)
///
/// | Level | Constant | Value | Use Case |
/// |-------|----------|-------|----------|
/// | 1 | `ANALYTICAL` | 1e-6 (0.0001%) | Closed-form solutions (put-call parity, zero-coupon YTM) |
/// | 2 | `NUMERICAL` | 1e-4 (0.01%) | Iterative methods (Newton-Raphson, tree pricing) |
/// | 3 | `CURVE_PRICING` | 5e-3 (0.5%) | Curve-based valuations with convention differences |
/// | 4 | `RELATIVE` | 1e-2 (1%) | Proportional comparisons, textbook benchmarks |
/// | 5 | `BUMP_VS_ANALYTICAL` | 1.5e-2 (1.5%) | Bump-and-reprice vs analytical approximations |
/// | 6 | `STATISTICAL` | 2e-2 (2%) | Monte Carlo and statistical tests |
///
/// # Examples
///
/// ```rust,ignore
/// use crate::common::test_helpers::tolerances;
///
/// // Use appropriate tolerance for test type
/// assert!((actual - expected).abs() < tolerances::NUMERICAL);
///
/// // Or use with assert_approx_eq_with_config
/// assert_approx_eq_with_config(actual, expected, tolerances::ANALYTICAL, "put-call parity");
/// ```
pub mod tolerances {
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

    /// Statistical/Monte Carlo methods.
    /// These have inherent sampling variance.
    pub const STATISTICAL: f64 = 2e-2; // 2%
}

// =============================================================================
// Standard Test Dates
// =============================================================================

/// Standard test dates for consistent test fixtures.
///
/// Using standardized dates ensures deterministic tests and makes it easy
/// to reason about time-dependent calculations.
///
/// # Note on Module-Specific Test Dates
///
/// Some instrument modules define their own local `test_date()` functions with
/// different values (e.g., `fx_spot/common.rs` uses 2025-01-15). These are
/// intentionally different to test specific scenarios within those modules.
/// For new tests, prefer using constants from this `dates` module for consistency.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::common::test_helpers::dates;
///
/// let as_of = dates::TODAY;
/// let maturity = dates::five_years_hence();
/// ```
pub mod dates {
    use super::*;

    /// Standard "today" date for tests: 2024-01-01
    pub const TODAY: Date = date!(2024 - 01 - 01);

    /// Alternative "today" for tests needing a weekday: 2024-01-02 (Tuesday)
    pub const TODAY_WEEKDAY: Date = date!(2024 - 01 - 02);

    /// IMM date for quarterly rolls: 2024-03-20 (March IMM)
    pub const IMM_DATE: Date = date!(2024 - 03 - 20);

    /// One year from TODAY
    pub fn one_year_hence() -> Date {
        date!(2025 - 01 - 01)
    }

    /// Two years from TODAY
    #[allow(dead_code)]
    pub fn two_years_hence() -> Date {
        date!(2026 - 01 - 01)
    }

    /// Five years from TODAY
    pub fn five_years_hence() -> Date {
        date!(2029 - 01 - 01)
    }

    /// Ten years from TODAY
    pub fn ten_years_hence() -> Date {
        date!(2034 - 01 - 01)
    }

    /// Create a date with explicit year offset from TODAY
    pub fn years_hence(years: i32) -> Date {
        Date::from_ordinal_date(2024 + years, 1).expect("valid date")
    }
}

/// Scale tolerance with value magnitude, with a minimum absolute floor.
///
/// Useful for property tests where tolerance should scale with the value being tested
/// but should never go below a minimum threshold.
///
/// # Arguments
/// * `base_tol` - Base relative tolerance (e.g., 1e-4 for 0.01%)
/// * `value` - The value to scale against
/// * `min_abs` - Minimum absolute tolerance floor
///
/// # Returns
/// The larger of (value * base_tol) and min_abs
pub fn scaled_tolerance(base_tol: f64, value: f64, min_abs: f64) -> f64 {
    (value.abs() * base_tol).max(min_abs)
}

// =============================================================================
// Assertion Helpers
// =============================================================================

/// Assert two floats are approximately equal with given tolerance.
///
/// # Arguments
///
/// * `actual` - Computed value
/// * `expected` - Expected value
/// * `tolerance` - Absolute tolerance
/// * `msg` - Descriptive message for failure
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64, msg: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff < tolerance,
        "{}: expected {}, got {} (diff: {:.6e})",
        msg,
        expected,
        actual,
        diff
    );
}

/// Assert two floats are approximately equal with both relative and absolute tolerance.
///
/// Uses relative tolerance for large values and absolute tolerance for near-zero values.
/// This is the recommended assertion for most financial calculations.
///
/// # Arguments
///
/// * `actual` - Computed value
/// * `expected` - Expected value
/// * `rel_tol` - Relative tolerance (e.g., `tolerances::NUMERICAL`)
/// * `abs_tol` - Absolute tolerance floor (e.g., 1e-10)
/// * `msg` - Descriptive message for failure
///
/// # Examples
///
/// ```rust,ignore
/// assert_approx_eq_dual(actual, expected, tolerances::NUMERICAL, 1e-10, "NPV calculation");
/// ```
pub fn assert_approx_eq_dual(actual: f64, expected: f64, rel_tol: f64, abs_tol: f64, msg: &str) {
    let diff = (actual - expected).abs();
    let rel_diff = if expected.abs() > 1e-12 {
        diff / expected.abs()
    } else {
        diff
    };

    let passes = diff <= abs_tol || rel_diff <= rel_tol;
    assert!(
        passes,
        "{}: expected {}, got {} (abs_diff={:.6e}, rel_diff={:.6e})",
        msg, expected, actual, diff, rel_diff
    );
}

/// Assert two floats are approximately equal with relative tolerance.
///
/// Falls back to absolute tolerance for near-zero expected values.
pub fn assert_relative_eq(actual: f64, expected: f64, rel_tolerance: f64, msg: &str) {
    if expected.abs() < 1e-10 {
        // For near-zero values, use absolute tolerance
        assert_approx_eq(actual, expected, 1e-10, msg);
    } else {
        let rel_diff = ((actual - expected) / expected).abs();
        assert!(
            rel_diff < rel_tolerance,
            "{}: expected {}, got {} (relative diff: {:.2}%)",
            msg,
            expected,
            actual,
            rel_diff * 100.0
        );
    }
}

/// Assert Money values are approximately equal.
///
/// Verifies currency matches and amounts are within tolerance.
pub fn assert_money_eq(actual: Money, expected: Money, tolerance: f64, msg: &str) {
    assert_eq!(
        actual.currency(),
        expected.currency(),
        "{}: currency mismatch",
        msg
    );
    assert_approx_eq(actual.amount(), expected.amount(), tolerance, msg);
}

/// Assert a value is within a range (inclusive).
pub fn assert_in_range(value: f64, min: f64, max: f64, msg: &str) {
    assert!(
        value >= min && value <= max,
        "{}: expected value in [{}, {}], got {}",
        msg,
        min,
        max,
        value
    );
}

// =============================================================================
// Curve Builders - Discount Curves
// =============================================================================

/// Create a flat discount curve for testing with simple knots.
///
/// Uses `dates::TODAY` as base date and LogLinear interpolation.
/// For more control, use `flat_discount_curve()` or `flat_discount_curve_with_dc()`.
pub fn flat_curve(rate: f64, curve_id: &str) -> DiscountCurve {
    let base = dates::TODAY;
    let t_max = 30.0; // 30 years
    let df_max = (-rate * t_max).exp();

    DiscountCurve::builder(curve_id)
        .base_date(base)
        .knots([(0.0, 1.0), (t_max, df_max)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap()
}

/// Create a flat discount curve with custom rate and base date.
///
/// Uses ACT/360 day count by default. Handles negative and zero rates
/// by switching to linear interpolation and allowing non-monotonic DFs.
///
/// # Arguments
///
/// * `rate` - Continuously compounded rate
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
///
/// # Examples
///
/// ```rust,ignore
/// let curve = flat_discount_curve(0.05, dates::TODAY, "USD-OIS");
/// ```
pub fn flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    flat_discount_curve_with_dc(rate, base_date, curve_id, DayCount::Act360)
}

/// Create a flat discount curve with custom day count convention.
///
/// This is the most flexible discount curve builder for tests. Handles:
/// - Negative rates (uses linear interpolation, allows non-monotonic DFs)
/// - Zero rates (uses linear interpolation, allows non-monotonic DFs)
/// - Standard knot points: 0, 1, 5, 10, 30 years
///
/// # Arguments
///
/// * `rate` - Continuously compounded rate
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
/// * `day_count` - Day count convention for the curve
///
/// # Examples
///
/// ```rust,ignore
/// // Standard USD curve with ACT/360
/// let usd_curve = flat_discount_curve_with_dc(0.05, as_of, "USD-OIS", DayCount::Act360);
///
/// // EUR curve with ACT/365F
/// let eur_curve = flat_discount_curve_with_dc(0.03, as_of, "EUR-OIS", DayCount::Act365F);
/// ```
pub fn flat_discount_curve_with_dc(
    rate: f64,
    base_date: Date,
    curve_id: &str,
    day_count: DayCount,
) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(day_count)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ]);

    // For negative or zero rates, DFs may be flat or increasing
    // Use linear interpolation and allow non-monotonic for robustness
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.interp(InterpStyle::Linear).validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

// =============================================================================
// Curve Builders - Forward Curves
// =============================================================================

/// Create a flat forward curve for testing.
///
/// Uses ACT/360 day count and 3M (0.25Y) tenor by default.
///
/// # Arguments
///
/// * `rate` - Forward rate level
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
///
/// # Examples
///
/// ```rust,ignore
/// let fwd_curve = flat_forward_curve(0.05, dates::TODAY, "USD-SOFR-3M");
/// ```
pub fn flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    flat_forward_curve_with_tenor(rate, base_date, curve_id, 0.25, DayCount::Act360)
}

/// Create a flat forward curve with custom tenor and day count.
///
/// # Arguments
///
/// * `rate` - Forward rate level
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
/// * `tenor` - Forward rate tenor in years (e.g., 0.25 for 3M)
/// * `day_count` - Day count convention
///
/// # Examples
///
/// ```rust,ignore
/// // 3M SOFR curve
/// let sofr_3m = flat_forward_curve_with_tenor(0.05, as_of, "USD-SOFR-3M", 0.25, DayCount::Act360);
///
/// // 6M EURIBOR curve
/// let euribor_6m = flat_forward_curve_with_tenor(0.03, as_of, "EUR-EURIBOR-6M", 0.5, DayCount::Act360);
/// ```
pub fn flat_forward_curve_with_tenor(
    rate: f64,
    base_date: Date,
    curve_id: &str,
    tenor: f64,
    day_count: DayCount,
) -> ForwardCurve {
    ForwardCurve::builder(curve_id, tenor)
        .base_date(base_date)
        .day_count(day_count)
        .knots([(0.0, rate), (10.0, rate), (30.0, rate)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

/// Create an upward-sloping forward curve for testing rate sensitivity.
///
/// Provides a realistic upward-sloping term structure for testing
/// instruments sensitive to the shape of the forward curve.
///
/// # Arguments
///
/// * `base_rate` - Starting rate at short end
/// * `long_rate` - Rate at long end (30Y)
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
pub fn sloped_forward_curve(
    base_rate: f64,
    long_rate: f64,
    base_date: Date,
    curve_id: &str,
) -> ForwardCurve {
    // Linear interpolation between base_rate and long_rate
    let rate_1y = base_rate + (long_rate - base_rate) * (1.0 / 30.0);
    let rate_5y = base_rate + (long_rate - base_rate) * (5.0 / 30.0);
    let rate_10y = base_rate + (long_rate - base_rate) * (10.0 / 30.0);

    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, base_rate),
            (1.0, rate_1y),
            (5.0, rate_5y),
            (10.0, rate_10y),
            (30.0, long_rate),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

// =============================================================================
// Curve Builders - Hazard Curves
// =============================================================================

/// Create a flat hazard curve with recovery rate.
///
/// # Arguments
///
/// * `hazard_rate` - Flat hazard rate (e.g., 0.01 for 1%)
/// * `recovery` - Recovery rate (e.g., 0.40 for 40%)
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
///
/// # Examples
///
/// ```rust,ignore
/// let hazard = flat_hazard_curve(0.01, 0.40, dates::TODAY, "CREDIT");
/// ```
pub fn flat_hazard_curve(
    hazard_rate: f64,
    recovery: f64,
    base_date: Date,
    curve_id: &str,
) -> HazardCurve {
    flat_hazard_curve_with_knots(
        hazard_rate,
        recovery,
        base_date,
        curve_id,
        &[1.0, 5.0, 10.0],
    )
}

/// Create a flat hazard curve with custom knot points.
///
/// # Arguments
///
/// * `hazard_rate` - Flat hazard rate
/// * `recovery` - Recovery rate
/// * `base_date` - Curve base date
/// * `curve_id` - Curve identifier
/// * `knot_times` - Array of knot times in years
pub fn flat_hazard_curve_with_knots(
    hazard_rate: f64,
    recovery: f64,
    base_date: Date,
    curve_id: &str,
    knot_times: &[f64],
) -> HazardCurve {
    let knots: Vec<(f64, f64)> = knot_times.iter().map(|&t| (t, hazard_rate)).collect();

    HazardCurve::builder(curve_id)
        .base_date(base_date)
        .recovery_rate(recovery)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .unwrap()
}

/// Create a standard upward-sloping discount curve.
///
/// Useful for testing instruments sensitive to curve shape.
pub fn upward_curve(curve_id: &str) -> DiscountCurve {
    let base = dates::TODAY;

    DiscountCurve::builder(curve_id)
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap()
}

// =============================================================================
// Standard Market Context Builders
// =============================================================================

/// Create a standard market context with USD and EUR curves.
///
/// Includes:
/// - USD-OIS (5% flat discount)
/// - USD-SOFR-3M (5.2% flat discount for 3M SOFR)
/// - EUR-OIS (3% flat discount)
/// - EUR-EURIBOR-6M (3.2% flat discount for 6M EURIBOR)
///
/// Uses `test_date()` as the base date (2025-01-01).
pub fn standard_market() -> MarketContext {
    MarketContext::new()
        .insert(flat_curve(0.05, "USD-OIS"))
        .insert(flat_curve(0.052, "USD-SOFR-3M"))
        .insert(flat_curve(0.03, "EUR-OIS"))
        .insert(flat_curve(0.032, "EUR-EURIBOR-6M"))
}

/// Create a USD swap market context with discount and forward curves.
///
/// Standard setup for IRS, swaption, and other USD rate derivative tests.
///
/// # Arguments
///
/// * `base_date` - Market date
/// * `rate` - Flat rate for all curves (simplifies par swap construction)
pub fn usd_swap_market(base_date: Date, rate: f64) -> MarketContext {
    let disc = flat_discount_curve(rate, base_date, "USD-OIS");
    let fwd = flat_forward_curve(rate, base_date, "USD-SOFR-3M");

    MarketContext::new().insert(disc).insert(fwd)
}

/// Create a USD swap market context with different discount and forward rates.
///
/// Useful for testing off-market swaps where disc ≠ fwd.
///
/// # Arguments
///
/// * `base_date` - Market date
/// * `disc_rate` - Discount curve rate
/// * `fwd_rate` - Forward curve rate
pub fn usd_swap_market_split(base_date: Date, disc_rate: f64, fwd_rate: f64) -> MarketContext {
    let disc = flat_discount_curve(disc_rate, base_date, "USD-OIS");
    let fwd = flat_forward_curve(fwd_rate, base_date, "USD-SOFR-3M");

    MarketContext::new().insert(disc).insert(fwd)
}

/// Create a credit market context with discount and hazard curves.
///
/// Standard setup for CDS and other credit derivative tests.
///
/// # Arguments
///
/// * `base_date` - Market date
/// * `disc_rate` - Discount curve rate
/// * `hazard_rate` - Hazard rate
/// * `recovery` - Recovery rate
pub fn credit_market(
    base_date: Date,
    disc_rate: f64,
    hazard_rate: f64,
    recovery: f64,
) -> MarketContext {
    let disc = flat_discount_curve(disc_rate, base_date, "USD_DISC");
    let hazard = flat_hazard_curve(hazard_rate, recovery, base_date, "CREDIT");

    MarketContext::new().insert(disc).insert(hazard)
}

/// Create test money in USD
pub fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD)
}

/// Create test money in EUR
pub fn eur(amount: f64) -> Money {
    Money::new(amount, Currency::EUR)
}

/// Create test money in GBP
#[allow(dead_code)]
pub fn gbp(amount: f64) -> Money {
    Money::new(amount, Currency::GBP)
}

/// Standard day count for testing
pub fn standard_dc() -> DayCount {
    DayCount::Act365F
}

/// Calculate year fraction for testing
pub fn year_fraction(start: Date, end: Date) -> f64 {
    use finstack_quant_core::dates::DayCountContext;
    standard_dc()
        .year_fraction(start, end, DayCountContext::default())
        .unwrap_or(0.0)
}

/// Black-Scholes analytical solution for European call (for validation)
pub fn black_scholes_call(
    spot: f64,
    strike: f64,
    rate: f64,
    vol: f64,
    time: f64,
    div_yield: f64,
) -> f64 {
    use finstack_quant_core::math::norm_cdf;

    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }

    let sqrt_t = time.sqrt();
    let d1 = ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * sqrt_t);
    let d2 = d1 - vol * sqrt_t;

    let discount = (-rate * time).exp();
    let forward_discount = (-(rate - div_yield) * time).exp();

    spot * forward_discount * norm_cdf(d1) - strike * discount * norm_cdf(d2)
}

/// Black-Scholes analytical solution for European put (for validation)
pub fn black_scholes_put(
    spot: f64,
    strike: f64,
    rate: f64,
    vol: f64,
    time: f64,
    div_yield: f64,
) -> f64 {
    let call = black_scholes_call(spot, strike, rate, vol, time, div_yield);
    let pv_strike = strike * (-rate * time).exp();
    let pv_spot = spot * (-div_yield * time).exp();

    // Put-call parity: P = C - S*e^(-qT) + K*e^(-rT)
    call - pv_spot + pv_strike
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Tolerance Tests
    // =========================================================================

    #[test]
    fn test_approx_eq() {
        assert_approx_eq(1.0001, 1.0, 0.001, "Should be approximately equal");
    }

    #[test]
    fn test_approx_eq_dual() {
        // Large value - uses relative tolerance
        assert_approx_eq_dual(1000.001, 1000.0, 1e-4, 1e-10, "Large value relative");

        // Small value - falls back to absolute tolerance
        assert_approx_eq_dual(1e-11, 0.0, 1e-4, 1e-10, "Near-zero absolute");
    }

    #[test]
    fn test_scaled_tolerance() {
        // For large values, tolerance should scale
        assert!((scaled_tolerance(1e-4, 1000.0, 0.01) - 0.1).abs() < 1e-10);

        // For small values, minimum floor should apply
        assert!((scaled_tolerance(1e-4, 0.001, 0.01) - 0.01).abs() < 1e-10);

        // Zero value uses minimum
        assert!((scaled_tolerance(1e-4, 0.0, 0.01) - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_tolerance_tiers_ordering() {
        // Verify tolerance tiers are in expected order (strictest to loosest)
        const _: () = {
            assert!(tolerances::ANALYTICAL < tolerances::NUMERICAL);
            assert!(tolerances::NUMERICAL < tolerances::CURVE_PRICING);
            assert!(tolerances::CURVE_PRICING < tolerances::RELATIVE);
            assert!(tolerances::RELATIVE < tolerances::BUMP_VS_ANALYTICAL);
            assert!(tolerances::BUMP_VS_ANALYTICAL < tolerances::STATISTICAL);
        };
    }

    #[test]
    #[should_panic]
    fn test_approx_eq_fails() {
        assert_approx_eq(1.1, 1.0, 0.01, "Should fail");
    }

    #[test]
    fn test_relative_eq() {
        assert_relative_eq(
            100.0,
            99.5,
            tolerances::RELATIVE,
            "Within relative tolerance",
        );
    }

    #[test]
    fn test_in_range() {
        assert_in_range(5.0, 0.0, 10.0, "In range");
        assert_in_range(0.0, 0.0, 10.0, "At lower bound");
        assert_in_range(10.0, 0.0, 10.0, "At upper bound");
    }

    // =========================================================================
    // Date Tests
    // =========================================================================

    #[test]
    fn test_standard_dates() {
        // Verify date constants are correct
        assert_eq!(dates::TODAY, date!(2024 - 01 - 01));
        assert_eq!(dates::TODAY_WEEKDAY, date!(2024 - 01 - 02));
        assert_eq!(dates::IMM_DATE, date!(2024 - 03 - 20));

        // Verify date functions
        assert_eq!(dates::one_year_hence(), date!(2025 - 01 - 01));
        assert_eq!(dates::five_years_hence(), date!(2029 - 01 - 01));
        assert_eq!(dates::ten_years_hence(), date!(2034 - 01 - 01));
    }

    #[test]
    fn test_years_hence() {
        assert_eq!(dates::years_hence(0), date!(2024 - 01 - 01));
        assert_eq!(dates::years_hence(5), date!(2029 - 01 - 01));
        assert_eq!(dates::years_hence(30), date!(2054 - 01 - 01));
    }

    // =========================================================================
    // Curve Builder Tests
    // =========================================================================

    #[test]
    fn test_flat_discount_curve_with_dc() {
        let curve = flat_discount_curve_with_dc(0.05, dates::TODAY, "TEST", DayCount::Act365F);
        assert_eq!(curve.id().as_str(), "TEST");
        assert_eq!(curve.base_date(), dates::TODAY);

        // DF at t=0 should be 1.0
        assert!((curve.df(0.0) - 1.0).abs() < 1e-10);

        // DF at t=1 should be approximately exp(-0.05)
        let expected_df_1y = (-0.05_f64).exp();
        assert!((curve.df(1.0) - expected_df_1y).abs() < 1e-6);
    }

    #[test]
    fn test_flat_discount_curve_negative_rate() {
        // Should handle negative rates without panicking
        let curve = flat_discount_curve(-0.01, dates::TODAY, "NEG-RATE");
        assert!(curve.df(1.0) > 1.0, "DF should be > 1 for negative rates");
    }

    #[test]
    fn test_flat_forward_curve() {
        let curve = flat_forward_curve(0.05, dates::TODAY, "USD-SOFR-3M");
        assert_eq!(curve.id().as_str(), "USD-SOFR-3M");
        assert_eq!(curve.base_date(), dates::TODAY);

        // Forward rate should be 5% everywhere
        assert!((curve.rate(1.0) - 0.05).abs() < 1e-10);
        assert!((curve.rate(5.0) - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_sloped_forward_curve() {
        let curve = sloped_forward_curve(0.03, 0.06, dates::TODAY, "SLOPED");

        // Start rate should be base_rate
        assert!((curve.rate(0.0) - 0.03).abs() < 1e-6);

        // End rate should be long_rate
        assert!((curve.rate(30.0) - 0.06).abs() < 1e-6);

        // Middle should be interpolated
        let rate_15y = curve.rate(15.0);
        assert!(rate_15y > 0.03 && rate_15y < 0.06);
    }

    #[test]
    fn test_flat_hazard_curve() {
        let curve = flat_hazard_curve(0.01, 0.40, dates::TODAY, "CREDIT");
        assert_eq!(curve.id().as_str(), "CREDIT");
        assert!((curve.recovery_rate() - 0.40).abs() < 1e-10);

        // Hazard rate should be 1% everywhere
        assert!((curve.hazard_rate(1.0) - 0.01).abs() < 1e-6);
        assert!((curve.hazard_rate(5.0) - 0.01).abs() < 1e-6);
    }

    // =========================================================================
    // Market Context Tests
    // =========================================================================

    #[test]
    fn test_standard_market_has_curves() {
        let market = standard_market();
        assert!(market.get_discount("USD-OIS").is_ok());
        assert!(market.get_discount("EUR-OIS").is_ok());
    }

    #[test]
    fn test_usd_swap_market() {
        let market = usd_swap_market(dates::TODAY, 0.05);

        assert!(market.get_discount("USD-OIS").is_ok());
        assert!(market.get_forward("USD-SOFR-3M").is_ok());
    }

    #[test]
    fn test_usd_swap_market_split() {
        let market = usd_swap_market_split(dates::TODAY, 0.05, 0.06);

        let disc = market.get_discount("USD-OIS").unwrap();
        let fwd = market.get_forward("USD-SOFR-3M").unwrap();

        // Discount curve rate ≈ 5%
        let df_1y = disc.df(1.0);
        assert!(((-df_1y.ln()) - 0.05).abs() < 1e-4);

        // Forward curve rate = 6%
        assert!((fwd.rate(1.0) - 0.06).abs() < 1e-6);
    }

    #[test]
    fn test_credit_market() {
        let market = credit_market(dates::TODAY, 0.05, 0.01, 0.40);

        assert!(market.get_discount("USD_DISC").is_ok());
        assert!(market.get_hazard("CREDIT").is_ok());
    }

    // =========================================================================
    // Money Helper Tests
    // =========================================================================

    #[test]
    fn test_money_helpers() {
        let base = usd(100.0);
        let bumped = usd(100.0 + tolerances::NUMERICAL * 0.5);
        assert_money_eq(
            base,
            bumped,
            tolerances::NUMERICAL,
            "USD helper within tolerance",
        );

        let eur_value = eur(50.0);
        assert_eq!(eur_value.currency(), Currency::EUR);
    }

    // =========================================================================
    // Legacy Helper Tests
    // =========================================================================

    #[test]
    fn test_upward_curve_builder() {
        let curve = upward_curve("UPWARD");
        assert_eq!(curve.id().as_str(), "UPWARD");
        let df_short = curve.df(1.0);
        let df_long = curve.df(10.0);
        assert!(df_long < df_short);
    }

    #[test]
    fn test_year_fraction_helper() {
        let start = Date::from_calendar_date(2024, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let yf = year_fraction(start, end);
        // Act365F: 366 days (2024 is a leap year) / 365 = 1.00274...
        assert!(
            (yf - 1.00274).abs() < 1e-4,
            "Expected yf ≈ 1.00274 for leap year, got {}",
            yf
        );
        assert_eq!(standard_dc(), DayCount::Act365F);
    }

    #[test]
    fn test_black_scholes_put_call_parity() {
        let spot = 100.0;
        let strike = 100.0;
        let rate = 0.05;
        let vol = 0.20;
        let time = 1.0;
        let div = 0.0;

        let call = black_scholes_call(spot, strike, rate, vol, time, div);
        let put = black_scholes_put(spot, strike, rate, vol, time, div);

        // Put-call parity: C - P = S - K*e^(-rT)
        let lhs = call - put;
        let rhs = spot - strike * (-rate * time).exp();

        assert_approx_eq(lhs, rhs, tolerances::ANALYTICAL, "Put-call parity");
    }

    #[test]
    fn test_flat_curve_creation() {
        let curve = flat_curve(0.05, "TEST");
        assert_eq!(curve.id().as_str(), "TEST");
        assert_eq!(curve.base_date(), dates::TODAY);
    }
}
