//! Tests for date extension traits

use finstack_quant_core::dates::calendar::TARGET2;
use finstack_quant_core::dates::{Date, DateExt, FiscalConfig, OffsetDateTimeExt};
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn date_ext_is_weekend() {
    // January 4, 2025 is Saturday
    let saturday = make_date(2025, 1, 4);
    assert!(saturday.is_weekend());

    // January 5, 2025 is Sunday
    let sunday = make_date(2025, 1, 5);
    assert!(sunday.is_weekend());

    // January 6, 2025 is Monday
    let monday = make_date(2025, 1, 6);
    assert!(!monday.is_weekend());

    // January 2, 2025 is Thursday
    let thursday = make_date(2025, 1, 2);
    assert!(!thursday.is_weekend());
}

#[test]
fn date_ext_quarter() {
    assert_eq!(make_date(2025, 1, 15).quarter(), 1);
    assert_eq!(make_date(2025, 2, 28).quarter(), 1);
    assert_eq!(make_date(2025, 3, 31).quarter(), 1);

    assert_eq!(make_date(2025, 4, 1).quarter(), 2);
    assert_eq!(make_date(2025, 5, 15).quarter(), 2);
    assert_eq!(make_date(2025, 6, 30).quarter(), 2);

    assert_eq!(make_date(2025, 7, 1).quarter(), 3);
    assert_eq!(make_date(2025, 8, 15).quarter(), 3);
    assert_eq!(make_date(2025, 9, 30).quarter(), 3);

    assert_eq!(make_date(2025, 10, 1).quarter(), 4);
    assert_eq!(make_date(2025, 11, 15).quarter(), 4);
    assert_eq!(make_date(2025, 12, 31).quarter(), 4);
}

#[test]
fn date_ext_fiscal_year_calendar() {
    let config = FiscalConfig::calendar_year();

    let jan_date = make_date(2025, 1, 15);
    assert_eq!(jan_date.fiscal_year(config), 2025);

    let dec_date = make_date(2025, 12, 31);
    assert_eq!(dec_date.fiscal_year(config), 2025);
}

#[test]
fn date_ext_fiscal_year_us_federal() {
    let config = FiscalConfig::us_federal(); // Oct 1 start

    // Sept 30, 2024 is before FY start, belongs to FY 2024
    let sept = make_date(2024, 9, 30);
    assert_eq!(sept.fiscal_year(config), 2024);

    // Oct 1, 2024 is FY start, belongs to FY 2025
    let oct = make_date(2024, 10, 1);
    assert_eq!(oct.fiscal_year(config), 2025);

    // Dec 31, 2024 is in FY 2025
    let dec = make_date(2024, 12, 31);
    assert_eq!(dec.fiscal_year(config), 2025);
}

#[test]
fn date_ext_add_weekdays_forward() {
    // Friday Jan 3, 2025 + 3 weekdays = Wed Jan 8
    let friday = make_date(2025, 1, 3);
    let result = friday.add_weekdays(3);
    assert_eq!(result, make_date(2025, 1, 8)); // Wednesday
}

#[test]
fn date_ext_add_weekdays_backward() {
    // Monday Jan 6, 2025 - 3 weekdays = Wed Jan 1
    let monday = make_date(2025, 1, 6);
    let result = monday.add_weekdays(-3);
    assert_eq!(result, make_date(2025, 1, 1)); // Wednesday
}

#[test]
fn date_ext_add_weekdays_zero() {
    let date = make_date(2025, 1, 15);
    let result = date.add_weekdays(0);
    assert_eq!(result, date);
}

#[test]
fn date_ext_add_weekdays_over_weekend() {
    // Thursday Jan 2 + 1 weekday = Friday Jan 3
    let thursday = make_date(2025, 1, 2);
    let result = thursday.add_weekdays(1);
    assert_eq!(result, make_date(2025, 1, 3));

    // Friday Jan 3 + 1 weekday = Monday Jan 6 (skip weekend)
    let friday = make_date(2025, 1, 3);
    let result = friday.add_weekdays(1);
    assert_eq!(result, make_date(2025, 1, 6));
}

#[test]
fn date_ext_add_business_days_with_calendar() {
    let cal = TARGET2;

    // Friday June 27, 2025 + 3 business days = Wed July 2
    let friday = make_date(2025, 6, 27);
    let result = friday.add_business_days(3, &cal).unwrap();
    assert_eq!(result, make_date(2025, 7, 2));
}

#[test]
fn date_ext_add_business_days_backward() {
    let cal = TARGET2;

    // Monday June 30, 2025 - 3 business days = Wednesday June 25
    let monday = make_date(2025, 6, 30);
    let result = monday.add_business_days(-3, &cal).unwrap();
    assert_eq!(result, make_date(2025, 6, 25));
}

#[test]
fn date_ext_add_business_days_zero() {
    let cal = TARGET2;
    let date = make_date(2025, 6, 27);
    let result = date.add_business_days(0, &cal).unwrap();
    assert_eq!(result, date);
}

#[test]
fn date_ext_next_imm() {
    // After March IMM (March 19, 2025), next is June 18
    let after_march = make_date(2025, 3, 20);
    let next = after_march.next_imm();
    assert_eq!(next, make_date(2025, 6, 18));

    // Before March IMM, next is March 19
    let before_march = make_date(2025, 3, 10);
    let next = before_march.next_imm();
    assert_eq!(next, make_date(2025, 3, 19));
}

#[test]
fn offset_datetime_ext_is_weekend() {
    let dt = make_date(2025, 1, 4)
        .with_hms(10, 30, 0)
        .unwrap()
        .assume_utc();

    assert!(dt.is_weekend()); // Saturday
}

#[test]
fn offset_datetime_ext_quarter() {
    let dt = make_date(2025, 5, 15)
        .with_hms(14, 0, 0)
        .unwrap()
        .assume_utc();

    assert_eq!(dt.quarter(), 2);
}

#[test]
fn offset_datetime_ext_fiscal_year() {
    let config = FiscalConfig::us_federal();
    let dt = make_date(2024, 10, 1)
        .with_hms(9, 0, 0)
        .unwrap()
        .assume_utc();

    assert_eq!(dt.fiscal_year(config), 2025);
}

#[test]
fn offset_datetime_ext_add_weekdays() {
    let dt = make_date(2025, 1, 3) // Friday
        .with_hms(10, 30, 0)
        .unwrap()
        .assume_utc();

    let result = dt.add_weekdays(1);

    assert_eq!(result.date(), make_date(2025, 1, 6)); // Monday
    assert_eq!(result.time(), dt.time()); // Time preserved
}

#[test]
fn offset_datetime_ext_add_business_days() {
    let cal = TARGET2;
    let dt = make_date(2025, 6, 27) // Friday
        .with_hms(15, 45, 30)
        .unwrap()
        .assume_utc();

    let result = dt.add_business_days(3, &cal).unwrap();

    assert_eq!(result.date(), make_date(2025, 7, 2)); // Wednesday
    assert_eq!(result.time(), dt.time()); // Time preserved
}

#[test]
fn offset_datetime_ext_next_imm() {
    let dt = make_date(2025, 3, 10)
        .with_hms(9, 30, 0)
        .unwrap()
        .assume_utc();

    let next = dt.next_imm();

    assert_eq!(next.date(), make_date(2025, 3, 19)); // March IMM
    assert_eq!(next.time(), dt.time()); // Time preserved
}
