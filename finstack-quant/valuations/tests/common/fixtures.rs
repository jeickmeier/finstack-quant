//! Shared test fixtures for the models test suite.
//!
//! Provides reusable builders for common test data structures like
//! discount curves, market contexts, and standard dates.
//!
//! # Tolerance Constants
//!
//! For tolerance constants, use the `tolerances` module instead of defining
//! new constants here. This module re-exports common tolerances for convenience.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use time::Month;

// Re-export tolerances from the canonical location
pub use super::tolerances::{NEAR_ZERO, STANDARD, TIGHT};

// =============================================================================
// Standard Test Dates
// =============================================================================

/// Standard base date for most calibration tests (2025-01-02).
///
/// This is a business day (Thursday), avoiding holiday adjustment complications.
pub fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).expect("valid test date: 2025-01-02")
}

/// IMM-style base date for CDS tests (March 20th).
///
/// ISDA CDS conventions use IMM dates (Mar 20, Jun 20, Sep 20, Dec 20).
pub fn imm_base_date() -> Date {
    Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date: 2025-03-20")
}

// =============================================================================
// Discount Curve Fixtures
// =============================================================================

/// Implied continuously compounded rate used for standard test curves (~2.02%).
///
/// The discount factors in `usd_discount_curve` are derived from this rate:
/// - DF(1Y) = exp(-0.0202 * 1) ≈ 0.98
/// - DF(3Y) = exp(-0.0206 * 3) ≈ 0.94
/// - DF(5Y) = exp(-0.0211 * 5) ≈ 0.90
/// - DF(10Y) = exp(-0.0223 * 10) ≈ 0.80
///
/// Note: The curve has a slight upward slope in implied zero rates.
pub const IMPLIED_TEST_RATE: f64 = 0.0202;

/// Creates a standard USD discount curve for testing.
///
/// Uses an upward-sloping term structure with implied zero rates starting
/// around 2% at the short end. Discount factors are chosen to be round
/// numbers for easy debugging while remaining economically reasonable.
///
/// # Provenance
///
/// | Tenor | DF   | Implied Zero Rate |
/// |-------|------|-------------------|
/// | 0Y    | 1.00 | N/A               |
/// | 1Y    | 0.98 | 2.02%             |
/// | 3Y    | 0.94 | 2.06%             |
/// | 5Y    | 0.90 | 2.11%             |
/// | 10Y   | 0.80 | 2.23%             |
///
/// # Arguments
/// * `base_date` - The curve's base/valuation date
/// * `curve_id` - Identifier for the curve (e.g., "USD-OIS")
///
/// # Returns
/// A discount curve with typical short/medium/long tenors.
pub fn usd_discount_curve(base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),   // DF at t=0 is always 1.0
            (1.0, 0.98),  // Implied rate: -ln(0.98)/1 ≈ 2.02%
            (3.0, 0.94),  // Implied rate: -ln(0.94)/3 ≈ 2.06%
            (5.0, 0.90),  // Implied rate: -ln(0.90)/5 ≈ 2.11%
            (10.0, 0.80), // Implied rate: -ln(0.80)/10 ≈ 2.23%
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid test discount curve")
}

/// Creates a minimal USD discount curve with fewer knots.
///
/// Useful for tests that don't need a full term structure. Uses the same
/// implied rate methodology as `usd_discount_curve`.
///
/// # Provenance
///
/// | Tenor | DF   | Implied Zero Rate |
/// |-------|------|-------------------|
/// | 0Y    | 1.00 | N/A               |
/// | 1Y    | 0.98 | 2.02%             |
/// | 5Y    | 0.90 | 2.11%             |
pub fn usd_discount_curve_minimal(base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),  // DF at t=0 is always 1.0
            (1.0, 0.98), // Implied rate: -ln(0.98)/1 ≈ 2.02%
            (5.0, 0.90), // Implied rate: -ln(0.90)/5 ≈ 2.11%
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid minimal test discount curve")
}

/// Creates a discount curve with monotone convex interpolation.
///
/// Suitable for forward curve calibration tests where smooth forwards matter.
/// Uses a ~4.5% continuously compounded rate.
///
/// # Provenance
///
/// | Tenor | DF     | Implied Zero Rate |
/// |-------|--------|-------------------|
/// | 0Y    | 1.0000 | N/A               |
/// | 3M    | 0.9888 | 4.50%             |
/// | 6M    | 0.9775 | 4.55%             |
/// | 1Y    | 0.9550 | 4.60%             |
/// | 2Y    | 0.9100 | 4.72%             |
pub fn usd_discount_curve_monotone_convex(base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),     // DF at t=0 is always 1.0
            (0.25, 0.9888), // Implied rate: -ln(0.9888)/0.25 ≈ 4.50%
            (0.5, 0.9775),  // Implied rate: -ln(0.9775)/0.5 ≈ 4.55%
            (1.0, 0.9550),  // Implied rate: -ln(0.9550)/1 ≈ 4.60%
            (2.0, 0.9100),  // Implied rate: -ln(0.9100)/2 ≈ 4.72%
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .expect("valid monotone convex test discount curve")
}

// =============================================================================
// Market Context Fixtures
// =============================================================================

/// Creates a market context with a standard USD-OIS discount curve.
pub fn market_context_with_usd_discount(base_date: Date) -> MarketContext {
    let curve = usd_discount_curve(base_date, "USD-OIS");
    MarketContext::new().insert(curve)
}

/// Creates a market context with a minimal USD-OIS discount curve.
pub fn market_context_with_minimal_discount(base_date: Date) -> MarketContext {
    let curve = usd_discount_curve_minimal(base_date, "USD-OIS");
    MarketContext::new().insert(curve)
}

// =============================================================================
// Build Context Helpers
// =============================================================================

/// Standard notional for test instruments ($1M).
pub const STANDARD_NOTIONAL: f64 = 1_000_000.0;

/// Standard currency for USD tests.
pub const USD: Currency = Currency::USD;

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::assertions::assert_approx_eq;
    use crate::common::tolerances;

    // =========================================================================
    // Date Fixture Tests
    // =========================================================================

    #[test]
    fn test_base_date_is_valid_business_day() {
        let date = base_date();
        assert_eq!(date.year(), 2025);
        assert_eq!(date.month(), Month::January);
        assert_eq!(date.day(), 2);
        // January 2, 2025 is a Thursday (weekday)
        assert!(
            date.weekday() != time::Weekday::Saturday && date.weekday() != time::Weekday::Sunday,
            "base_date should be a weekday"
        );
    }

    #[test]
    fn test_imm_base_date_is_march_20() {
        let date = imm_base_date();
        assert_eq!(date.year(), 2025);
        assert_eq!(date.month(), Month::March);
        assert_eq!(date.day(), 20);
    }

    // =========================================================================
    // Discount Curve Tests
    // =========================================================================

    #[test]
    fn test_usd_discount_curve_construction() {
        let date = base_date();
        let curve = usd_discount_curve(date, "USD-OIS");

        assert_eq!(curve.id().as_str(), "USD-OIS");
        assert_eq!(curve.base_date(), date);
    }

    #[test]
    fn test_usd_discount_curve_df_at_zero_is_one() {
        let curve = usd_discount_curve(base_date(), "TEST");
        let df_0 = curve.df(0.0);
        assert_approx_eq(df_0, 1.0, TIGHT);
    }

    #[test]
    fn test_usd_discount_curve_df_values_match_provenance() {
        let curve = usd_discount_curve(base_date(), "TEST");

        // Verify documented DF values
        assert_approx_eq(curve.df(1.0), 0.98, STANDARD);
        assert_approx_eq(curve.df(3.0), 0.94, STANDARD);
        assert_approx_eq(curve.df(5.0), 0.90, STANDARD);
        assert_approx_eq(curve.df(10.0), 0.80, STANDARD);
    }

    #[test]
    fn test_usd_discount_curve_implied_rates_are_reasonable() {
        let curve = usd_discount_curve(base_date(), "TEST");

        // Verify implied zero rates are in reasonable range (1-5%)
        for t in [1.0, 3.0, 5.0, 10.0] {
            let df = curve.df(t);
            let implied_rate = -df.ln() / t;
            assert!(
                (0.01..=0.05).contains(&implied_rate),
                "Implied rate at {t}Y should be in [1%, 5%], got {:.2}%",
                implied_rate * 100.0
            );
        }
    }

    #[test]
    fn test_usd_discount_curve_is_monotonically_decreasing() {
        let curve = usd_discount_curve(base_date(), "TEST");

        let tenors = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
        for i in 1..tenors.len() {
            let df_prev = curve.df(tenors[i - 1]);
            let df_curr = curve.df(tenors[i]);
            assert!(
                df_curr < df_prev,
                "DF should decrease: DF({}) = {} >= DF({}) = {}",
                tenors[i],
                df_curr,
                tenors[i - 1],
                df_prev
            );
        }
    }

    #[test]
    fn test_usd_discount_curve_minimal_construction() {
        let date = base_date();
        let curve = usd_discount_curve_minimal(date, "USD-OIS-MIN");

        assert_eq!(curve.id().as_str(), "USD-OIS-MIN");
        assert_approx_eq(curve.df(0.0), 1.0, TIGHT);
        assert_approx_eq(curve.df(1.0), 0.98, STANDARD);
        assert_approx_eq(curve.df(5.0), 0.90, STANDARD);
    }

    #[test]
    fn test_usd_discount_curve_monotone_convex_construction() {
        let date = base_date();
        let curve = usd_discount_curve_monotone_convex(date, "USD-MC");

        assert_eq!(curve.id().as_str(), "USD-MC");
        assert_approx_eq(curve.df(0.0), 1.0, TIGHT);
        assert_approx_eq(curve.df(0.25), 0.9888, STANDARD);
        assert_approx_eq(curve.df(1.0), 0.9550, STANDARD);
    }

    // =========================================================================
    // Market Context Tests
    // =========================================================================

    #[test]
    fn test_market_context_with_usd_discount() {
        let market = market_context_with_usd_discount(base_date());
        assert!(
            market.get_discount("USD-OIS").is_ok(),
            "Market should contain USD-OIS curve"
        );
    }

    #[test]
    fn test_market_context_with_minimal_discount() {
        let market = market_context_with_minimal_discount(base_date());
        let curve = market.get_discount("USD-OIS").expect("should have curve");
        // Minimal curve should still work for basic discounting
        assert_approx_eq(curve.df(1.0), 0.98, STANDARD);
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_standard_notional_value() {
        assert_eq!(STANDARD_NOTIONAL, 1_000_000.0);
    }

    #[test]
    fn test_usd_currency_constant() {
        assert_eq!(USD, Currency::USD);
    }

    #[test]
    fn test_implied_test_rate_matches_curve() {
        let curve = usd_discount_curve(base_date(), "TEST");
        let df_1y = curve.df(1.0);
        let actual_rate = -df_1y.ln();

        // IMPLIED_TEST_RATE should be approximately the 1Y zero rate
        assert_approx_eq(actual_rate, IMPLIED_TEST_RATE, tolerances::LOOSE);
    }
}
