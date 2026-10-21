//! Shared curve builders, standard markets and comparison helpers for instrument tests.
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
//! use finstack_quant_valuations_test_support::test_helpers::*;
//!
//! // Use standard test dates
//! let as_of = dates::TODAY;
//! let maturity = dates::five_years_hence();
//!
//! // Build curves with default day count
//! let disc = flat_discount_curve(0.05, as_of, "USD-OIS");
//!
//! // Build curves with custom day count
//! let disc = flat_discount_curve_with_day_count(0.05, as_of, "USD-OIS", DayCount::Act365F);
//!
//! // Use tolerance presets from the crate's `tolerances` module
//! approx_eq(actual, expected, tolerances::NUMERICAL, "test");
//! ```

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
pub use finstack_quant_test_utils::assert::approx_eq;
use time::macros::date;

// Standard Test Dates

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
/// use finstack_quant_valuations_test_support::test_helpers::dates;
///
/// let as_of = dates::TODAY;
/// let maturity = dates::five_years_hence();
/// ```
pub mod dates {
    use super::*;

    /// Standard "today" date for tests: 2024-01-01
    pub const TODAY: Date = date!(2024 - 01 - 01);

    /// One year from TODAY
    pub fn one_year_hence() -> Date {
        date!(2025 - 01 - 01)
    }

    /// Five years from TODAY
    pub fn five_years_hence() -> Date {
        date!(2029 - 01 - 01)
    }

    /// Create a date with explicit year offset from TODAY
    ///
    /// # Arguments
    ///
    /// * `years` - Whole years added to `TODAY` (2024-01-01); negative values move back.
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

// Assertion Helpers

// Curve Builders - Discount Curves

/// Create a flat discount curve with custom rate and base date.
///
/// Uses ACT/360 day count by default. Handles negative and zero rates
/// by switching to linear interpolation and allowing non-monotonic DFs.
///
/// # Arguments
///
/// * `rate` - Continuously compounded zero rate, as a decimal, applied at every knot.
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
///
/// # Examples
///
/// ```rust,ignore
/// let curve = flat_discount_curve(0.05, dates::TODAY, "USD-OIS");
/// ```
pub fn flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    flat_discount_curve_with_day_count(rate, base_date, curve_id, DayCount::Act360)
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
/// * `rate` - Continuously compounded zero rate, as a decimal, applied at every knot.
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
/// * `day_count` - Day count convention the curve converts dates to year fractions with.
///
/// # Examples
///
/// ```rust,ignore
/// // Standard USD curve with ACT/360
/// let usd_curve = flat_discount_curve_with_day_count(0.05, as_of, "USD-OIS", DayCount::Act360);
///
/// // EUR curve with ACT/365F
/// let eur_curve = flat_discount_curve_with_day_count(0.03, as_of, "EUR-OIS", DayCount::Act365F);
/// ```
pub fn flat_discount_curve_with_day_count(
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

// Curve Builders - Forward Curves

/// Create a flat forward curve for testing.
///
/// Uses ACT/360 day count and 3M (0.25Y) tenor by default.
///
/// # Arguments
///
/// * `rate` - Simple forward rate, as a decimal, written at every knot.
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
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
/// * `rate` - Simple forward rate, as a decimal, written at every knot.
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
/// * `tenor` - Index tenor of the forward rate in years (e.g., 0.25 for 3M)
/// * `day_count` - Day count convention the curve converts dates to year fractions with.
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

// Curve Builders - Hazard Curves

/// Create a flat hazard curve with recovery rate.
///
/// # Arguments
///
/// * `hazard_rate` - Flat annual default intensity, as a decimal (e.g., 0.01 for 1%).
/// * `recovery` - Recovery rate on default as a decimal fraction of notional (e.g., 0.40 for 40%).
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
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
/// * `hazard_rate` - Flat annual default intensity, as a decimal, applied at every knot.
/// * `recovery` - Recovery rate on default as a decimal fraction of notional.
/// * `base_date` - Curve base date; knot times are measured from it.
/// * `curve_id` - Identifier the market context is keyed by.
/// * `knot_times` - Knot times in years from `base_date`, ascending.
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

// Standard Market Context Builders

/// Create a USD swap market context with discount and forward curves.
///
/// Standard setup for IRS, swaption, and other USD rate derivative tests.
///
/// # Arguments
///
/// * `base_date` - Valuation date of the context and base date of both curves.
/// * `rate` - Continuously compounded rate, as a decimal, used for the discount and the forward curve (simplifies par swap construction).
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
/// * `base_date` - Valuation date of the context and base date of both curves.
/// * `disc_rate` - Continuously compounded zero rate, as a decimal, of the `USD-OIS` discount curve.
/// * `fwd_rate` - Simple forward rate, as a decimal, of the `USD-SOFR-3M` projection curve.
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
/// * `base_date` - Valuation date of the context and base date of both curves.
/// * `disc_rate` - Continuously compounded zero rate, as a decimal, of the discount curve.
/// * `hazard_rate` - Flat annual default intensity of the hazard curve, as a decimal.
/// * `recovery` - Recovery rate on default as a decimal fraction of notional.
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
///
/// # Arguments
///
/// * `amount` - Monetary amount in US dollars.
pub fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

/// Create test money in EUR
///
/// # Arguments
///
/// * `amount` - Monetary amount in euros.
pub fn eur(amount: f64) -> Money {
    Money::new(amount, Currency::EUR).expect("valid money fixture")
}

/// Standard day count for testing
pub fn standard_day_count() -> DayCount {
    DayCount::Act365F
}

/// Calculate year fraction for testing
///
/// # Arguments
///
/// * `start` - First date of the accrual period, inclusive.
/// * `end` - Last date of the accrual period, exclusive; measured with the standard test day count.
pub fn year_fraction(start: Date, end: Date) -> f64 {
    use finstack_quant_core::dates::DayCountContext;
    standard_day_count()
        .year_fraction(start, end, DayCountContext::default())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    // Tolerance Tests

    #[test]
    fn test_scaled_tolerance() {
        // For large values, tolerance should scale
        assert!((scaled_tolerance(1e-4, 1000.0, 0.01) - 0.1).abs() < 1e-10);

        // For small values, minimum floor should apply
        assert!((scaled_tolerance(1e-4, 0.001, 0.01) - 0.01).abs() < 1e-10);

        // Zero value uses minimum
        assert!((scaled_tolerance(1e-4, 0.0, 0.01) - 0.01).abs() < 1e-10);
    }

    // Date Tests

    #[test]
    fn test_standard_dates() {
        // Verify date constants are correct
        assert_eq!(dates::TODAY, date!(2024 - 01 - 01));

        // Verify date functions
        assert_eq!(dates::one_year_hence(), date!(2025 - 01 - 01));
        assert_eq!(dates::five_years_hence(), date!(2029 - 01 - 01));
    }

    #[test]
    fn test_years_hence() {
        assert_eq!(dates::years_hence(0), date!(2024 - 01 - 01));
        assert_eq!(dates::years_hence(5), date!(2029 - 01 - 01));
        assert_eq!(dates::years_hence(30), date!(2054 - 01 - 01));
    }

    // Curve Builder Tests

    #[test]
    fn test_flat_discount_curve_with_day_count() {
        let curve =
            flat_discount_curve_with_day_count(0.05, dates::TODAY, "TEST", DayCount::Act365F);
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
    fn test_flat_hazard_curve() {
        let curve = flat_hazard_curve(0.01, 0.40, dates::TODAY, "CREDIT");
        assert_eq!(curve.id().as_str(), "CREDIT");
        assert!((curve.recovery_rate() - 0.40).abs() < 1e-10);

        // Hazard rate should be 1% everywhere
        assert!((curve.hazard_rate(1.0) - 0.01).abs() < 1e-6);
        assert!((curve.hazard_rate(5.0) - 0.01).abs() < 1e-6);
    }

    // Market Context Tests

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
        assert_eq!(standard_day_count(), DayCount::Act365F);
    }
}
