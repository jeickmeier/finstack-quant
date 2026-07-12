//! FX Settlement Integration Tests
//!
//! Validates FX spot date calculations against ISDA conventions with real holiday
//! calendars. These tests ensure that our joint business day counting logic
//! matches market-standard settlement behavior.
//!
//! # Test Coverage
//!
//! - USD/EUR around New Year (joint holidays on both calendars)
//! - GBP/JPY around UK/JP holidays (asymmetric holiday closures)
//! - USD/GBP around US/UK holidays
//! - Edge cases: Friday trades, multiple consecutive holidays
//!
//! # Convention Notes
//!
//! FX spot settlement follows T+N **joint business days**:
//! - A day is business only if it's business on BOTH currency calendars
//! - Standard spot lag is T+2 for most pairs (T+1 for some like USD/CAD)
//! - Settlement calendars depend on currency pair (not exchange)
//!
//! # Golden File Updates
//!
//! If these tests fail after legitimate convention changes:
//! 1. Review the calendar rules in `finstack-quant/core/data/calendars/`
//! 2. Verify expected dates match published ISDA/bank calendars
//! 3. Update `fx_spot_dates.json` with documented rationale
//!
//! # References
//!
//! - ISDA FX Settlement Calendar: https://www.isda.org/category/legal/confirmation-closeout-settlement/
//! - Bloomberg FXFA function for spot date verification
//! - ECB TARGET2 calendar: https://www.ecb.europa.eu/paym/target/target2/profuse/calendar/html/index.en.html

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::dates::fx::{add_joint_business_days, fx_spot_date, resolve_calendar};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::FxSpot;
use time::Month;

/// Test USD/EUR spot settlement around New Year's Day 2024
///
/// New Year's Day (Jan 1) is a joint holiday for both NYSE and TARGET2 calendars.
/// This test verifies that spot settlement correctly skips the joint closure.
///
/// # Trade Details
/// - Currency Pair: USD/EUR
/// - Trade Date: Friday, December 29, 2023
/// - Spot Lag: T+2 (standard for USD/EUR)
/// - Base Calendar: NYSE (US business days)
/// - Quote Calendar: TARGET2 (Eurozone business days)
///
/// # Expected Settlement
/// From Friday Dec 29, 2023:
/// - Skip: Sat Dec 30, Sun Dec 31 (weekend)
/// - Skip: Mon Jan 1, 2024 (New Year's Day - **joint closure**)
/// - Count: Tue Jan 2, 2024 (business day 1)
/// - Count: Wed Jan 3, 2024 (business day 2) ← **Spot Date**
///
/// # Legacy Behavior Difference
/// Pre-Phase 2 implementation incorrectly added calendar days:
/// - Dec 29 + 2 calendar days = Dec 31 (Sunday)
/// - Adjusted to Mon Jan 1 (New Year's Day) - **wrong**
///
/// This test validates the fix to use joint business day counting.
#[test]
fn test_usd_eur_spot_new_year_2024() {
    // Trade on Friday before New Year's Day
    let trade_date = create_date(2023, Month::December, 29).expect("Valid date");

    // CLS-consistent rule: USD calendar gates only the final value date
    // . Same result here since
    // Jan 1 is a joint closure.
    let spot = fx_spot_date(
        trade_date,
        2,               // T+2
        Some("nyse"),    // USD calendar
        Some("target2"), // EUR calendar
        Some("nyse"),    // USD side
    )
    .expect("Valid calendars");

    // Expected: Wednesday, January 3, 2024
    // (skips weekend + Jan 1 joint holiday, then counts 2 business days)
    let expected = create_date(2024, Month::January, 3).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "USD/EUR T+2 from Dec 29, 2023 should be Jan 3, 2024 (skips Jan 1 joint closure)"
    );
}

/// Test USD/EUR spot settlement around Christmas 2024
///
/// Christmas (Dec 25) is a joint holiday; Dec 26 (Boxing Day) is a holiday
/// on TARGET2 but not NYSE.
///
/// # Trade Details
/// - Currency Pair: USD/EUR
/// - Trade Date: Monday, December 23, 2024
/// - Spot Lag: T+2
/// - Base Calendar: NYSE (US)
/// - Quote Calendar: TARGET2 (Eurozone)
///
/// # Expected Settlement
/// From Monday Dec 23, 2024:
/// - Count: Tue Dec 24 (business day 1 - **both open**)
/// - Skip: Wed Dec 25 (Christmas - **joint closure**)
/// - Skip: Thu Dec 26 (Boxing Day - **TARGET2 closed, joint skip**)
/// - Count: Fri Dec 27 (business day 2) ← **Spot Date**
///
/// # Key Point
/// Even though NYSE may be open on Dec 26, we skip it because TARGET2 is closed.
/// Joint business day logic requires both calendars to be open.
#[test]
fn test_usd_eur_spot_christmas_2024() {
    let trade_date = create_date(2024, Month::December, 23).expect("Valid date");

    // CLS-consistent rule : same result here
    // since the skipped days are TARGET2 (non-USD) closures.
    let spot = fx_spot_date(trade_date, 2, Some("nyse"), Some("target2"), Some("nyse"))
        .expect("Valid calendars");

    // Expected: Friday, December 27, 2024
    let expected = create_date(2024, Month::December, 27).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "USD/EUR T+2 from Dec 23, 2024 should be Dec 27 (skips Christmas + Boxing Day)"
    );
}

/// Test GBP/JPY spot settlement with asymmetric holidays
///
/// UK Early May Bank Holiday (first Monday in May) vs Japan Golden Week holidays.
/// This tests the case where both calendars have holidays around the same period.
///
/// # Trade Details
/// - Currency Pair: GBP/JPY
/// - Trade Date: Thursday, May 1, 2025
/// - Spot Lag: T+2
/// - Base Calendar: GBLO (London)
/// - Quote Calendar: JPX (Tokyo Stock Exchange)
///
/// # Expected Settlement
/// From Thursday May 1, 2025:
/// - Count: Fri May 2 (business day 1 - **both open**)
/// - Skip: Sat May 3, Sun May 4 (weekend)
/// - Skip: Mon May 5 (Children's Day - **JPX closed**, UK bank holiday - **GBLO closed**)
/// - Skip: Tue May 6 (Substitute holiday for Greenery Day - **JPX closed**)
/// - Count: Wed May 7 (business day 2) ← **Spot Date**
///
/// # Convention Note
/// Japan's Golden Week 2025:
/// - May 3 (Sat): Constitution Day
/// - May 4 (Sun): Greenery Day
/// - May 5 (Mon): Children's Day (JPX closed)
/// - May 6 (Tue): Substitute holiday for Greenery Day (JPX closed)
///
/// UK Early May Bank Holiday is first Monday in May (May 5, 2025).
/// Joint closure on May 5, JPX-only closure on May 6 → spot on May 7.
#[test]
fn test_gbp_jpy_spot_may_bank_holiday_2025() {
    let trade_date = create_date(2025, Month::May, 1).expect("Valid date");

    let spot = add_joint_business_days(
        trade_date,
        2,
        Some("gblo"), // GBP calendar
        Some("jpx"),  // JPY calendar (Tokyo Stock Exchange)
        None,
    )
    .expect("Valid calendars");

    // Expected: Wednesday, May 7, 2025
    // (May 5 is joint closure UK+JP, May 6 is JPX substitute holiday)
    let expected = create_date(2025, Month::May, 7).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "GBP/JPY T+2 from May 1, 2025 should be May 7 (skips May 5 joint closure + May 6 JPX substitute holiday)"
    );
}

/// Test GBP/JPY around UK Spring Bank Holiday (last Monday in May)
///
/// # Trade Details
/// - Currency Pair: GBP/JPY
/// - Trade Date: Thursday, May 22, 2025
/// - Spot Lag: T+2
/// - Base Calendar: GBLO
/// - Quote Calendar: JPX
///
/// # Expected Settlement
/// From Thursday May 22, 2025:
/// - Count: Fri May 23 (business day 1)
/// - Skip: Sat May 24, Sun May 25 (weekend)
/// - Skip: Mon May 26 (UK Spring Bank Holiday - **GBLO closed**)
/// - Count: Tue May 27 (business day 2) ← **Spot Date**
#[test]
fn test_gbp_jpy_spot_spring_bank_holiday_2025() {
    let trade_date = create_date(2025, Month::May, 22).expect("Valid date");

    let spot = add_joint_business_days(trade_date, 2, Some("gblo"), Some("jpx"), None)
        .expect("Valid calendars");

    // Expected: Tuesday, May 27, 2025
    let expected = create_date(2025, Month::May, 27).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "GBP/JPY T+2 from May 22, 2025 should be May 27 (skips Spring Bank Holiday)"
    );
}

/// Test USD/GBP around US Independence Day (July 4)
///
/// Independence Day is a US holiday (NYSE closed) but not a UK holiday (GBLO open).
///
/// # Trade Details
/// - Currency Pair: USD/GBP
/// - Trade Date: Wednesday, July 2, 2025
/// - Spot Lag: T+2
/// - Base Calendar: NYSE
/// - Quote Calendar: GBLO
///
/// # Expected Settlement (CLS-consistent rule)
/// From Wednesday July 2, 2025:
/// - Count: Thu July 3 (business day 1 - **both open**)
/// - Count: Fri July 4 (Independence Day - USD does not gate intermediate days,
///   GBLO open, so it counts as business day 2) — but the **final value date**
///   must be a good USD day, so spot rolls forward
/// - Skip: Sat July 5, Sun July 6 (weekend)
/// - Settle: Mon July 7 ← **Spot Date** (first day good in all calendars)
#[test]
fn test_usd_gbp_spot_july_4th_2025() {
    let trade_date = create_date(2025, Month::July, 2).expect("Valid date");

    // CLS-consistent rule .
    let spot = fx_spot_date(trade_date, 2, Some("nyse"), Some("gblo"), Some("nyse"))
        .expect("Valid calendars");

    // Expected: Monday, July 7, 2025
    let expected = create_date(2025, Month::July, 7).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "USD/GBP T+2 from July 2, 2025 should be July 7 (skips July 4 US holiday)"
    );
}

/// Test add_joint_business_days with multiple consecutive holidays
///
/// This tests the robustness of the joint business day counting when there are
/// extended holiday periods on both calendars.
///
/// # Scenario
/// - Start: Friday, December 20, 2024
/// - Add: 5 joint business days
/// - Calendars: NYSE and TARGET2
///
/// # Expected Count
/// From Friday Dec 20, 2024:
/// - Skip: Sat Dec 21, Sun Dec 22 (weekend)
/// - Count: Mon Dec 23 (business day 1)
/// - Count: Tue Dec 24 (business day 2) - **both open**
/// - Skip: Wed Dec 25 (Christmas - **joint closure**)
/// - Skip: Thu Dec 26 (Boxing Day - **TARGET2 closed**)
/// - Count: Fri Dec 27 (business day 3)
/// - Skip: Sat Dec 28, Sun Dec 29 (weekend)
/// - Count: Mon Dec 30 (business day 4)
/// - Count: Tue Dec 31 (business day 5) ← **Result**
#[test]
fn test_add_joint_business_days_christmas_week_2024() {
    let start = create_date(2024, Month::December, 20).expect("Valid date");

    let result = add_joint_business_days(
        start,
        5, // 5 joint business days
        Some("nyse"),
        Some("target2"),
        None,
    )
    .expect("Valid calendars");

    // Expected: Tuesday, December 31, 2024
    let expected = create_date(2024, Month::December, 31).expect("Valid expected date");

    assert_eq!(
        result, expected,
        "Adding 5 joint business days from Dec 20, 2024 should land on Dec 31"
    );
}

/// Test edge case: Friday trade requiring spot to roll past weekend + holiday
///
/// # Scenario
/// - Trade Date: Friday, January 17, 2025 (before MLK Day)
/// - Currency Pair: USD/GBP
/// - Spot Lag: T+2
///
/// # Expected Settlement (CLS-consistent rule)
/// From Friday Jan 17, 2025:
/// - Skip: Sat Jan 18, Sun Jan 19 (weekend)
/// - Count: Mon Jan 20 (MLK Day - NYSE closed, but USD does **not** gate
///   intermediate days; GBLO open → business day 1)
/// - Count: Tue Jan 21 (business day 2, good in all calendars) ← **Spot Date**
///
/// Expectation updated from Jan 22 to Jan 21 per the CLS-consistent FX spot
/// rule :
/// a US holiday on an intermediate day must not delay a USD pair's spot date.
#[test]
fn test_usd_gbp_spot_mlk_day_2025() {
    let trade_date = create_date(2025, Month::January, 17).expect("Valid date");

    let spot = fx_spot_date(trade_date, 2, Some("nyse"), Some("gblo"), Some("nyse"))
        .expect("Valid calendars");

    // Expected: Tuesday, January 21, 2025
    let expected = create_date(2025, Month::January, 21).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "USD/GBP T+2 from Jan 17, 2025 should be Jan 21 (MLK Day at T+1 does not delay spot)"
    );
}

/// Test error handling: unknown base calendar
#[test]
fn test_unknown_base_calendar_errors() {
    let trade_date = create_date(2024, Month::January, 15).expect("Valid date");

    let result = add_joint_business_days(
        trade_date,
        2,
        Some("unknown_calendar"), // Invalid
        Some("gblo"),
        None,
    );

    assert!(result.is_err(), "Unknown base calendar should return error");

    // Verify it's a CalendarNotFound error
    if let Err(err) = result {
        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("CalendarNotFound") || err_str.contains("unknown_calendar"),
            "Error should mention CalendarNotFound or the unknown calendar name, got: {}",
            err_str
        );
    }
}

/// Test error handling: unknown quote calendar
#[test]
fn test_unknown_quote_calendar_errors() {
    let trade_date = create_date(2024, Month::January, 15).expect("Valid date");

    let result = add_joint_business_days(
        trade_date,
        2,
        Some("nyse"),
        Some("invalid_cal"), // Invalid
        None,
    );

    assert!(
        result.is_err(),
        "Unknown quote calendar should return error"
    );

    if let Err(err) = result {
        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("CalendarNotFound") || err_str.contains("invalid_cal"),
            "Error should mention CalendarNotFound or the invalid calendar, got: {}",
            err_str
        );
    }
}

/// Test weekends-only calendar (explicit None)
///
/// When no calendar is specified (None), we should get weekends-only behavior
/// without erroring. This is useful for testing or for currencies without
/// established holiday calendars.
#[test]
fn test_weekends_only_no_holidays() {
    let trade_date = create_date(2024, Month::January, 15).expect("Valid date"); // Monday

    let spot = add_joint_business_days(
        trade_date, 2, None, // Weekends-only
        None, // Weekends-only
        None,
    )
    .expect("None should use weekends-only");

    // Expected: Wednesday, January 17, 2024 (simple +2 business days, no holidays)
    let expected = create_date(2024, Month::January, 17).expect("Valid expected date");

    assert_eq!(
        spot, expected,
        "Weekends-only should add 2 business days with no holiday skips"
    );
}

/// Verify calendar resolution returns correct calendar
#[test]
fn test_resolve_calendar_returns_correct_calendar() {
    // Test that valid calendars resolve successfully
    let nyse = resolve_calendar(Some("nyse")).expect("NYSE should exist");
    let gblo = resolve_calendar(Some("gblo")).expect("GBLO should exist");
    let _target2 = resolve_calendar(Some("target2")).expect("TARGET2 should exist");
    let _jpx = resolve_calendar(Some("jpx")).expect("JPX should exist");

    // Verify they're actually different calendars by checking a known holiday
    // MLK Day 2024 (Jan 15) - NYSE closed, others may be open
    let mlk_day = create_date(2024, Month::January, 15).expect("Valid date");

    // NYSE should be closed on MLK Day
    assert!(
        !nyse.as_holiday_calendar().is_business_day(mlk_day),
        "NYSE should be closed on MLK Day"
    );

    // GBLO should be open on MLK Day (not a UK holiday)
    assert!(
        gblo.as_holiday_calendar().is_business_day(mlk_day),
        "GBLO should be open on MLK Day (not a UK holiday)"
    );

    // Verify None returns weekends-only
    let weekends = resolve_calendar(None).expect("None should return weekends-only");

    // Monday should be a business day in weekends-only
    let monday = create_date(2024, Month::January, 15).expect("Valid date");
    assert!(
        weekends.as_holiday_calendar().is_business_day(monday),
        "Weekends-only should treat Monday as business day"
    );

    // Saturday should not be a business day
    let saturday = create_date(2024, Month::January, 13).expect("Valid date");
    assert!(
        !weekends.as_holiday_calendar().is_business_day(saturday),
        "Weekends-only should treat Saturday as non-business day"
    );
}

/// Performance test: verify iteration limit doesn't trigger for reasonable inputs
///
/// The implementation has a MAX_ITERS safety limit. This test ensures that
/// normal holiday scenarios don't approach that limit.
#[test]
fn test_add_joint_business_days_iteration_limit() {
    // Adding 20 business days should complete well under MAX_ITERS (1000)
    let start = create_date(2024, Month::January, 1).expect("Valid date");

    let result = add_joint_business_days(
        start,
        20, // 20 business days
        Some("nyse"),
        Some("target2"),
        None,
    );

    // Should succeed without hitting iteration limit
    assert!(
        result.is_ok(),
        "Adding 20 business days should not hit iteration limit"
    );

    // Result should be in late January / early February
    let computed = result.expect("Should succeed");
    assert!(
        computed.month() == Month::January || computed.month() == Month::February,
        "20 business days from Jan 1 should be in Jan or Feb, got: {:?}",
        computed
    );
}

/// Instrument-level regression: EUR/USD traded the business day before a US
/// holiday must NOT settle one day late.
///
/// EUR/USD traded Thu 2025-07-03; Fri 2025-07-04 (US Independence Day) is a
/// good EUR (TARGET2) day, so under the CLS-consistent rule it counts as the
/// first intermediate day and spot is Mon 2025-07-07 — not Tue 2025-07-08 as
/// the old symmetric joint-calendar roll produced.
///
/// /// "Major — schedules" FX spot finding.
#[test]
fn test_fx_spot_instrument_eur_usd_us_holiday_intermediate_2025() {
    let trade_date = create_date(2025, Month::July, 3).expect("Valid date");

    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .expect("valid notional")
        .with_base_calendar_id("target2")
        .with_quote_calendar_id("usny")
        .with_settlement_lag_days(2);

    let settlement = fx
        .effective_settlement_date(trade_date)
        .expect("settlement date");

    let expected = create_date(2025, Month::July, 7).expect("Valid expected date");
    assert_eq!(
        settlement, expected,
        "EUR/USD traded 2025-07-03 must settle Mon 2025-07-07; a US holiday at \
         T+1 must not delay spot"
    );
}

/// Instrument-level check: a US holiday on the FINAL value date still rolls
/// spot forward.
///
/// EUR/USD traded Wed 2025-07-02: the T+2 candidate is Fri 2025-07-04 (good
/// EUR day), but the value date itself must be good in USD, so spot rolls to
/// Mon 2025-07-07.
#[test]
fn test_fx_spot_instrument_eur_usd_us_holiday_final_date_2025() {
    let trade_date = create_date(2025, Month::July, 2).expect("Valid date");

    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .expect("valid notional")
        .with_base_calendar_id("target2")
        .with_quote_calendar_id("usny")
        .with_settlement_lag_days(2);

    let settlement = fx
        .effective_settlement_date(trade_date)
        .expect("settlement date");

    let expected = create_date(2025, Month::July, 7).expect("Valid expected date");
    assert_eq!(
        settlement, expected,
        "a US holiday on the final value date must roll EUR/USD spot forward"
    );
}
