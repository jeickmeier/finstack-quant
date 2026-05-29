//! Generated calendars wiring tests.
//!
//! These tests validate that build-time generated calendars are exposed through the
//! public `finstack_core::dates` API (without relying on internal bitset helpers).

use finstack_core::dates::calendar::{TARGET2, USNY};
use finstack_core::dates::{CalendarRegistry, Date, HolidayCalendar};
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn generated_calendar_constants_exist_and_work() {
    // Jan 1 is a holiday for TARGET2
    let jan1 = make_date(2025, 1, 1);
    assert!(TARGET2.is_holiday(jan1));

    // Weekends are never business days (trait default)
    let sat = make_date(2025, 1, 4);
    assert!(!USNY.is_business_day(sat));
}

#[test]
fn holiday_rules_respect_effective_year() {
    use finstack_core::dates::calendar::NYSE;

    // Juneteenth: NYSE first closed in 2022. Pre-adoption dates must NOT be
    // holidays (regression: previously every year 1970–2150 was marked).
    assert!(
        !NYSE.is_holiday(make_date(1990, 6, 19)),
        "Juneteenth must not be an NYSE holiday before 2022"
    );
    assert!(
        NYSE.is_holiday(make_date(2023, 6, 19)),
        "Juneteenth 2023 (Monday) is an NYSE holiday"
    );

    // MLK Day (3rd Monday of January): NYSE observed from 1998.
    // 1990's 3rd Monday is Jan 15; must not be a holiday.
    assert!(
        !NYSE.is_holiday(make_date(1990, 1, 15)),
        "MLK Day must not be an NYSE holiday before 1998"
    );
    // 2023's 3rd Monday is Jan 16.
    assert!(
        NYSE.is_holiday(make_date(2023, 1, 16)),
        "MLK Day 2023 is an NYSE holiday"
    );

    // Un-gated holidays remain valid historically.
    assert!(
        NYSE.is_holiday(make_date(1990, 1, 1)),
        "New Year's Day is always an NYSE holiday"
    );
}

#[test]
fn calendar_registry_resolves_generated_calendars() {
    let registry = CalendarRegistry::global();

    let target2 = registry
        .resolve_str("target2")
        .expect("TARGET2 should be resolvable");

    let jan1 = make_date(2025, 1, 1);
    assert!(target2.is_holiday(jan1));
}
