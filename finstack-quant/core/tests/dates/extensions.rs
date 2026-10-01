//! Tests for date extension traits

use finstack_quant_core::dates::calendar::TARGET2;
use finstack_quant_core::dates::{Date, DateExt, FiscalConfig};
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn date_arithmetic_rejects_range_overflow() {
    let anchor = make_date(2025, 1, 1);
    for months in [i32::MIN, i32::MAX] {
        assert!(anchor.add_months(months).is_err());
    }
    for weekdays in [i32::MIN, i32::MAX] {
        assert!(anchor.add_weekdays(weekdays).is_err());
    }
    for days in [i64::MIN, i64::MAX] {
        assert!(anchor.add_days(days).is_err());
    }
    assert!(Date::MAX.add_months(1).is_err());
    assert!(Date::MIN.add_months(-1).is_err());
    assert!(Date::MAX.add_weekdays(1).is_err());
    assert!(Date::MIN.add_weekdays(-1).is_err());
    assert!(Date::MAX.add_business_days(1, &TARGET2).is_err());
    assert!(Date::MIN.add_business_days(-1, &TARGET2).is_err());
    assert!(anchor.add_business_days(i32::MAX, &TARGET2).is_err());
    assert_eq!(Date::MAX.add_days(0).unwrap(), Date::MAX);
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
    assert_eq!(make_date(2025, 4, 1).quarter(), 2);
    assert_eq!(make_date(2025, 9, 30).quarter(), 3);
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
    let result = friday.add_weekdays(3).expect("valid date shift");
    assert_eq!(result, make_date(2025, 1, 8)); // Wednesday
}

#[test]
fn date_ext_add_weekdays_backward() {
    // Monday Jan 6, 2025 - 3 weekdays = Wed Jan 1
    let monday = make_date(2025, 1, 6);
    let result = monday.add_weekdays(-3).expect("valid date shift");
    assert_eq!(result, make_date(2025, 1, 1)); // Wednesday
}

#[test]
fn date_ext_add_weekdays_zero() {
    let date = make_date(2025, 1, 15);
    let result = date.add_weekdays(0).expect("valid date shift");
    assert_eq!(result, date);
}

#[test]
fn date_ext_add_weekdays_over_weekend() {
    // Thursday Jan 2 + 1 weekday = Friday Jan 3
    let thursday = make_date(2025, 1, 2);
    let result = thursday.add_weekdays(1).expect("valid date shift");
    assert_eq!(result, make_date(2025, 1, 3));

    // Friday Jan 3 + 1 weekday = Monday Jan 6 (skip weekend)
    let friday = make_date(2025, 1, 3);
    let result = friday.add_weekdays(1).expect("valid date shift");
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
