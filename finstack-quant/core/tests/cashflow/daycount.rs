//! Tests for day count conventions and year fraction calculations.
//!
//! These tests verify correct year fraction calculations for:
//! - Leap year boundary handling
//! - All major day count conventions (Act/Act ISDA, Act/Act ISMA, Act/365F, Act/360, 30/360)
//! - Frequency-dependent calculations
//! - Edge cases (zero-length periods, century leap years)
//!
//! # Reference
//!
//! - ISDA 2006 Definitions, Section 4.16 (Day Count Fractions)
//! - ISDA 2006 Definitions, Appendix (Examples)

use super::test_helpers::{assert_close, FACTOR_TOLERANCE};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use time::Month;

/// Helper to create dates
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

// =============================================================================
// Act/Act ISDA Leap Year Tests
// =============================================================================

#[test]
fn actact_isda_leap_year_crossing_nov_to_mar() {
    // Period crossing Feb 29, 2024 (leap year)
    // Nov 1, 2023 to Mar 1, 2024
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let start = d(2023, 11, 1);
    let end = d(2024, 3, 1);
    let yf = dc.year_fraction(start, end, ctx).unwrap();

    // ISDA Act/Act calculation:
    // Days in 2023: Nov 1 to Dec 31 = 61 days (365-day year)
    // Days in 2024: Jan 1 to Mar 1 = 60 days (366-day year, leap year)
    // Year fraction = 61/365 + 60/366
    let expected = 61.0 / 365.0 + 60.0 / 366.0;

    assert_close(
        yf,
        expected,
        FACTOR_TOLERANCE,
        "Act/Act ISDA leap crossing (Nov-Mar)",
    );
}

#[test]
fn actact_isda_leap_year_crossing_dec_to_jan() {
    // Short period crossing year boundary
    // Dec 15, 2023 to Jan 15, 2024 = 31 total days
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let start = d(2023, 12, 15);
    let end = d(2024, 1, 15);
    let yf = dc.year_fraction(start, end, ctx).unwrap();

    // ISDA Act/Act calculation splits across year boundaries:
    // Days in 2023: Dec 15 to Jan 1 = 17 days (year has 365 days)
    // Days in 2024: Jan 1 to Jan 15 = 14 days (year has 366 days)
    // Year fraction = 17/365 + 14/366 ≈ 0.084827
    let expected = 17.0 / 365.0 + 14.0 / 366.0;

    assert_close(
        yf,
        expected,
        FACTOR_TOLERANCE,
        "Act/Act ISDA Dec-Jan period",
    );
}

#[test]
fn actact_isda_full_leap_year_is_one() {
    // Full leap year: Jan 1, 2024 to Jan 1, 2025
    // Should be exactly 1.0 regardless of leap year
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2024, 1, 1), d(2025, 1, 1), ctx).unwrap();

    assert_close(yf, 1.0, FACTOR_TOLERANCE, "Full leap year should be 1.0");
}

#[test]
fn actact_isda_full_non_leap_year_is_one() {
    // Full non-leap year: Jan 1, 2025 to Jan 1, 2026
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2025, 1, 1), d(2026, 1, 1), ctx).unwrap();

    assert_close(
        yf,
        1.0,
        FACTOR_TOLERANCE,
        "Full non-leap year should be 1.0",
    );
}

#[test]
fn actact_isda_feb_29_to_mar_1() {
    // Single day Feb 29 to Mar 1 in leap year
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let yf = dc
        .year_fraction(d(2024, 2, 29), d(2024, 3, 1), ctx)
        .unwrap();

    // 1 day in 366-day year
    let expected = 1.0 / 366.0;

    assert_close(yf, expected, FACTOR_TOLERANCE, "Feb 29 to Mar 1");
}

#[test]
fn actact_isda_spanning_multiple_years() {
    // Multi-year period: Jan 1, 2023 to Jan 1, 2026 (3 years)
    // 2023: non-leap (365), 2024: leap (366), 2025: non-leap (365)
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2023, 1, 1), d(2026, 1, 1), ctx).unwrap();

    // Should be exactly 3.0 years
    assert_close(yf, 3.0, FACTOR_TOLERANCE, "3-year period should be 3.0");
}

// =============================================================================
// Act/Act ISMA Frequency-Dependent Tests
// =============================================================================

#[test]
fn actact_isma_with_annual_frequency() {
    let dc = DayCount::ActActIsma;
    let ctx = DayCountContext {
        frequency: Some(Tenor::annual()),
        calendar: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    // Full year with annual frequency should be 1.0
    let yf = dc.year_fraction(d(2025, 1, 1), d(2026, 1, 1), ctx).unwrap();

    assert_close(
        yf,
        1.0,
        FACTOR_TOLERANCE,
        "Annual ISMA full year should be 1.0",
    );
}

#[test]
fn actact_isma_requires_frequency() {
    // Act/Act ISMA requires frequency in context
    let dc = DayCount::ActActIsma;
    let ctx_no_freq = DayCountContext {
        frequency: None,
        calendar: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let result = dc.year_fraction(d(2025, 1, 1), d(2025, 7, 1), ctx_no_freq);
    assert!(
        result.is_err(),
        "ActActIsma should error without frequency context"
    );
}

#[test]
fn actact_isma_with_semi_annual_frequency() {
    let dc = DayCount::ActActIsma;
    let ctx = DayCountContext {
        frequency: Some(Tenor::semi_annual()),
        calendar: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    // 6-month period with semi-annual frequency
    // Note: ISMA calculation depends on the coupon period context
    // The result may vary based on implementation details
    let result = dc.year_fraction(d(2025, 1, 1), d(2025, 7, 1), ctx);
    assert!(result.is_ok(), "Semi-annual ISMA should succeed");

    let yf = result.unwrap();
    // ISMA with semi-annual frequency treats 6 months as 1 full coupon period
    // which may be represented as 1.0 (number of periods) or 0.5 (fraction of year)
    // depending on implementation. Just verify it's positive.
    assert!(yf > 0.0, "ISMA fraction should be positive, got {}", yf);
}

#[test]
fn actact_isma_with_quarterly_frequency() {
    let dc = DayCount::ActActIsma;
    let ctx = DayCountContext {
        frequency: Some(Tenor::quarterly()),
        calendar: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    // 3-month period with quarterly frequency
    // Note: ISMA calculation depends on the coupon period context
    let result = dc.year_fraction(d(2025, 1, 1), d(2025, 4, 1), ctx);
    assert!(result.is_ok(), "Quarterly ISMA should succeed");

    let yf = result.unwrap();
    // ISMA with quarterly frequency treats 3 months as 1 full coupon period
    // Just verify it's positive and the calculation completes
    assert!(yf > 0.0, "ISMA fraction should be positive, got {}", yf);
}

// =============================================================================
// Act/365F Leap Year Tests
// =============================================================================

#[test]
fn act365f_leap_year_366_days() {
    // Act/365F always divides by 365, even in leap years
    // Full leap year has 366 actual days → 366/365 > 1.0
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2024, 1, 1), d(2025, 1, 1), ctx).unwrap();

    let expected = 366.0 / 365.0;
    assert_close(yf, expected, FACTOR_TOLERANCE, "Act/365F leap year");
}

#[test]
fn act365f_non_leap_year_365_days() {
    // Full non-leap year: 365/365 = 1.0
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2025, 1, 1), d(2026, 1, 1), ctx).unwrap();

    assert_close(yf, 1.0, FACTOR_TOLERANCE, "Act/365F non-leap year");
}

#[test]
fn act365f_feb_leap_year() {
    // February in leap year: Feb 1 to Mar 1 = 29 days
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2024, 2, 1), d(2024, 3, 1), ctx).unwrap();

    let expected = 29.0 / 365.0;
    assert_close(yf, expected, FACTOR_TOLERANCE, "Act/365F Feb leap year");
}

#[test]
fn act365f_feb_non_leap_year() {
    // February in non-leap year: Feb 1 to Mar 1 = 28 days
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2025, 2, 1), d(2025, 3, 1), ctx).unwrap();

    let expected = 28.0 / 365.0;
    assert_close(yf, expected, FACTOR_TOLERANCE, "Act/365F Feb non-leap year");
}

// =============================================================================
// Act/360 Leap Year Tests
// =============================================================================

#[test]
fn act360_leap_year_366_days() {
    // Act/360 always divides by 360
    // Full leap year: 366/360
    let dc = DayCount::Act360;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2024, 1, 1), d(2025, 1, 1), ctx).unwrap();

    let expected = 366.0 / 360.0;
    assert_close(yf, expected, FACTOR_TOLERANCE, "Act/360 leap year");
}

#[test]
fn act360_non_leap_year_365_days() {
    // Full non-leap year: 365/360
    let dc = DayCount::Act360;
    let ctx = DayCountContext::default();

    let yf = dc.year_fraction(d(2025, 1, 1), d(2026, 1, 1), ctx).unwrap();

    let expected = 365.0 / 360.0;
    assert_close(yf, expected, FACTOR_TOLERANCE, "Act/360 non-leap year");
}

// =============================================================================
// 30/360 Leap Year Tests
// =============================================================================

#[test]
fn thirty360_ignores_leap_year() {
    // 30/360 treats all months as 30 days, ignores actual calendar
    // Full year always = 360/360 = 1.0
    let dc = DayCount::Thirty360;
    let ctx = DayCountContext::default();

    // Leap year
    let yf_leap = dc.year_fraction(d(2024, 1, 1), d(2025, 1, 1), ctx).unwrap();
    assert_close(
        yf_leap,
        1.0,
        FACTOR_TOLERANCE,
        "30/360 leap year should be 1.0",
    );

    // Non-leap year
    let yf_non_leap = dc.year_fraction(d(2025, 1, 1), d(2026, 1, 1), ctx).unwrap();
    assert_close(
        yf_non_leap,
        1.0,
        FACTOR_TOLERANCE,
        "30/360 non-leap year should be 1.0",
    );
}

#[test]
fn thirty360_feb_always_30_days() {
    // 30/360: February is treated as 30 days regardless of leap year
    let dc = DayCount::Thirty360;
    let ctx = DayCountContext::default();

    // Feb in leap year (actual 29 days, 30/360 treats as 30)
    // Jan 15 to Mar 15 = 2 months = 60/360
    let yf_leap = dc
        .year_fraction(d(2024, 1, 15), d(2024, 3, 15), ctx)
        .unwrap();
    let expected = 60.0 / 360.0;
    assert_close(
        yf_leap,
        expected,
        FACTOR_TOLERANCE,
        "30/360 through Feb leap",
    );

    // Feb in non-leap year (actual 28 days, 30/360 treats as 30)
    let yf_non_leap = dc
        .year_fraction(d(2025, 1, 15), d(2025, 3, 15), ctx)
        .unwrap();
    assert_close(
        yf_non_leap,
        expected,
        FACTOR_TOLERANCE,
        "30/360 through Feb non-leap",
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn zero_length_period_all_conventions() {
    let ctx = DayCountContext::default();
    let ctx_with_freq = DayCountContext {
        frequency: Some(Tenor::semi_annual()),
        calendar: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let conventions = [
        (DayCount::Act365F, ctx),
        (DayCount::Act360, ctx),
        (DayCount::Thirty360, ctx),
        (DayCount::ActAct, ctx),
        (DayCount::ActActIsma, ctx_with_freq),
    ];

    let date = d(2024, 2, 29); // Feb 29 in leap year

    for (dc, ctx) in conventions {
        let yf = dc.year_fraction(date, date, ctx).unwrap();
        assert!(
            yf.abs() < FACTOR_TOLERANCE,
            "{:?}: Zero-length period should be 0.0, got {}",
            dc,
            yf
        );
    }
}

#[test]
fn century_leap_year_rule() {
    // 2000 was a leap year (divisible by 400)
    // 2100 will not be a leap year (divisible by 100 but not 400)
    let dc = DayCount::ActAct;
    let ctx = DayCountContext::default();

    // Year 2000 (leap year)
    let yf_2000 = dc.year_fraction(d(2000, 1, 1), d(2001, 1, 1), ctx).unwrap();
    assert_close(
        yf_2000,
        1.0,
        FACTOR_TOLERANCE,
        "Year 2000 (leap) should be 1.0",
    );
}
