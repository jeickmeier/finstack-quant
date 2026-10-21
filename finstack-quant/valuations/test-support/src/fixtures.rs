//! Shared test fixtures for the valuations test suites: a standard base date
//! and USD discount curves.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use time::Month;

// Standard Test Dates

/// Standard base date for most calibration tests (2025-01-02).
///
/// This is a business day (Thursday), avoiding holiday adjustment complications.
pub fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).expect("valid test date: 2025-01-02")
}

// Discount Curve Fixtures

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
///
/// # Arguments
///
/// * `base_date` - Valuation date the knot times are measured from.
/// * `curve_id` - Identifier the market context is keyed by (for example `USD-OIS`).
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
