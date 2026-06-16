//! CDS conventions and calendar validation tests.
//!
//! Validates that:
//! 1. CDS convention calendar IDs resolve to valid holiday calendars
//! 2. Settlement delays match ISDA standards (T+3 NA, T+1 EU, T+3 Asia)
//! 3. Day counts match regional conventions
//!
//! # Market Standards References
//!
//! - ISDA 2014 Credit Derivatives Definitions
//! - ISDA "Big Bang" Protocol (June 20, 2009) - European T+1 settlement
//! - ISDA "Small Bang" Protocol (July 27, 2009) - European convention updates

use finstack_quant_core::dates::calendar::calendar_by_id;
use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CDSConvention;
use time::{Date, Month};

/// Verify that all CDS convention calendar IDs resolve to valid calendars.
#[test]
fn test_cds_convention_calendars_resolve() {
    // North American convention should resolve to NYSE calendar
    let na_calendar_id = CDSConvention::IsdaNa.default_calendar();
    let na_calendar = calendar_by_id(na_calendar_id);
    assert!(
        na_calendar.is_some(),
        "North American calendar '{}' should resolve to a valid calendar",
        na_calendar_id
    );

    // European convention should resolve to TARGET2 calendar
    let eu_calendar_id = CDSConvention::IsdaEu.default_calendar();
    let eu_calendar = calendar_by_id(eu_calendar_id);
    assert!(
        eu_calendar.is_some(),
        "European calendar '{}' should resolve to a valid calendar",
        eu_calendar_id
    );

    // Asian convention should resolve to Tokyo calendar
    let as_calendar_id = CDSConvention::IsdaAs.default_calendar();
    let as_calendar = calendar_by_id(as_calendar_id);
    assert!(
        as_calendar.is_some(),
        "Asian calendar '{}' should resolve to a valid calendar",
        as_calendar_id
    );
}

/// Verify settlement delays match ISDA standards.
///
/// - North America: T+3
/// - Europe: T+1 (post-2009 Big Bang protocol)
/// - Asia: T+3
#[test]
fn test_cds_settlement_delays_isda_standard() {
    // North American: T+3
    assert_eq!(
        CDSConvention::IsdaNa.settlement_delay(),
        3,
        "North American CDS should have T+3 settlement"
    );

    // European: T+1 (post-2009 Big Bang)
    assert_eq!(
        CDSConvention::IsdaEu.settlement_delay(),
        1,
        "European CDS should have T+1 settlement (post-2009 Big Bang protocol)"
    );

    // Asian: T+3
    assert_eq!(
        CDSConvention::IsdaAs.settlement_delay(),
        3,
        "Asian CDS should have T+3 settlement"
    );
}

/// Verify day count conventions match ISDA standards.
///
/// - North America/Europe: ACT/360
/// - Asia: ACT/365F
#[test]
fn test_cds_day_count_conventions() {
    // North American: ACT/360
    assert_eq!(
        CDSConvention::IsdaNa.day_count(),
        DayCount::Act360,
        "North American CDS should use ACT/360"
    );

    // European: ACT/360
    assert_eq!(
        CDSConvention::IsdaEu.day_count(),
        DayCount::Act360,
        "European CDS should use ACT/360"
    );

    // Asian: ACT/365F
    assert_eq!(
        CDSConvention::IsdaAs.day_count(),
        DayCount::Act365F,
        "Asian CDS should use ACT/365F"
    );
}

/// Verify that the NYSE calendar correctly identifies US market holidays.
#[test]
fn test_nyse_calendar_holidays() {
    let calendar = calendar_by_id("nyse").expect("NYSE calendar should exist");

    // US Independence Day (July 4th)
    let independence_day = Date::from_calendar_date(2025, Month::July, 4).unwrap();
    assert!(
        calendar.is_holiday(independence_day),
        "July 4th should be a NYSE holiday"
    );

    // Christmas Day
    let christmas = Date::from_calendar_date(2025, Month::December, 25).unwrap();
    assert!(
        calendar.is_holiday(christmas),
        "December 25th should be a NYSE holiday"
    );

    // Regular business day
    let regular_day = Date::from_calendar_date(2025, Month::March, 10).unwrap();
    assert!(
        !calendar.is_holiday(regular_day),
        "March 10, 2025 (Monday) should not be a NYSE holiday"
    );
}

/// Verify that the TARGET2 calendar correctly identifies ECB holidays.
#[test]
fn test_target2_calendar_holidays() {
    let calendar = calendar_by_id("target2").expect("TARGET2 calendar should exist");

    // New Year's Day
    let new_year = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    assert!(
        calendar.is_holiday(new_year),
        "January 1st should be a TARGET2 holiday"
    );

    // Good Friday (April 18, 2025)
    let good_friday = Date::from_calendar_date(2025, Month::April, 18).unwrap();
    assert!(
        calendar.is_holiday(good_friday),
        "Good Friday should be a TARGET2 holiday"
    );

    // Easter Monday (April 21, 2025)
    let easter_monday = Date::from_calendar_date(2025, Month::April, 21).unwrap();
    assert!(
        calendar.is_holiday(easter_monday),
        "Easter Monday should be a TARGET2 holiday"
    );

    // Regular business day
    let regular_day = Date::from_calendar_date(2025, Month::March, 10).unwrap();
    assert!(
        !calendar.is_holiday(regular_day),
        "March 10, 2025 (Monday) should not be a TARGET2 holiday"
    );
}

/// Verify that the Tokyo calendar correctly identifies Japanese market holidays.
#[test]
fn test_tokyo_calendar_holidays() {
    let calendar = calendar_by_id("jpto").expect("Tokyo calendar should exist");

    // New Year's Day
    let new_year = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    assert!(
        calendar.is_holiday(new_year),
        "January 1st should be a Tokyo holiday"
    );

    // Coming of Age Day (2nd Monday of January - Jan 13, 2025)
    let coming_of_age = Date::from_calendar_date(2025, Month::January, 13).unwrap();
    assert!(
        calendar.is_holiday(coming_of_age),
        "Coming of Age Day should be a Tokyo holiday"
    );

    // Regular business day
    let regular_day = Date::from_calendar_date(2025, Month::March, 10).unwrap();
    assert!(
        !calendar.is_holiday(regular_day),
        "March 10, 2025 (Monday) should not be a Tokyo holiday"
    );
}

/// Verify all CDS conventions have consistent settings.
#[test]
fn test_cds_conventions_consistency() {
    for convention in [
        CDSConvention::IsdaNa,
        CDSConvention::IsdaEu,
        CDSConvention::IsdaAs,
    ] {
        // All conventions should have quarterly payment frequency
        let freq = convention.frequency();
        assert_eq!(
            freq.months(),
            Some(3),
            "{:?} should have quarterly (3M) payment frequency",
            convention
        );

        // All conventions should use Modified Following BDC
        let bdc = convention.business_day_convention();
        assert_eq!(
            bdc,
            finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            "{:?} should use Modified Following business day convention",
            convention
        );

        // All conventions should use Short Front stub
        let stub = convention.stub_convention();
        assert_eq!(
            stub,
            finstack_quant_core::dates::StubKind::ShortFront,
            "{:?} should use Short Front stub convention",
            convention
        );
    }
}

/// Test that detect_from_currency returns expected conventions.
#[test]
fn test_cds_convention_currency_detection() {
    use finstack_quant_core::currency::Currency;

    // North American currencies
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::USD),
        CDSConvention::IsdaNa
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::CAD),
        CDSConvention::IsdaNa
    );

    // European currencies
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::EUR),
        CDSConvention::IsdaEu
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::GBP),
        CDSConvention::IsdaEu
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::CHF),
        CDSConvention::IsdaEu
    );

    // Asian currencies
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::JPY),
        CDSConvention::IsdaAs
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::AUD),
        CDSConvention::IsdaAs
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::HKD),
        CDSConvention::IsdaAs
    );
    assert_eq!(
        CDSConvention::detect_from_currency(Currency::SGD),
        CDSConvention::IsdaAs
    );
}
