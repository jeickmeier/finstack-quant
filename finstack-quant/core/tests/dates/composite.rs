//! Tests for composite calendar functionality

use finstack_quant_core::dates::calendar::{GBLO, NYSE, TARGET2, USNY};
use finstack_quant_core::dates::{CompositeCalendar, CompositeMode};
use finstack_quant_core::dates::{Date, HolidayCalendar};
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn composite_union_any_holiday() {
    let t2 = TARGET2;
    let gb = GBLO;
    let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

    let composite = CompositeCalendar::new(&calendars);

    // Jan 1 is a holiday in both
    let jan1 = make_date(2025, 1, 1);
    assert!(composite.is_holiday(jan1));

    // Spring bank holiday (May 26, 2025) is only in GBLO
    let may26 = make_date(2025, 5, 26);
    assert!(GBLO.is_holiday(may26));
    assert!(!TARGET2.is_holiday(may26));
    assert!(composite.is_holiday(may26)); // Union includes it

    // Regular business day in both
    let regular_day = make_date(2025, 6, 18); // Wednesday
    assert!(!composite.is_holiday(regular_day));
}

#[test]
fn composite_intersection_all_holidays() {
    let t2 = TARGET2;
    let gb = GBLO;
    let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

    let composite = CompositeCalendar::with_mode(&calendars, CompositeMode::Intersection);

    // Jan 1 is a holiday in both
    let jan1 = make_date(2025, 1, 1);
    assert!(composite.is_holiday(jan1));

    // Spring bank holiday (May 26, 2025) is only in GBLO, not TARGET2
    let may26 = make_date(2025, 5, 26);
    assert!(!composite.is_holiday(may26)); // Intersection requires both

    // Christmas is in both
    let christmas = make_date(2025, 12, 25);
    assert!(composite.is_holiday(christmas));
}

#[test]
fn composite_empty_calendars_union() {
    let calendars: &[&dyn HolidayCalendar] = &[];
    let composite = CompositeCalendar::new(calendars);

    // Empty union should have no holidays
    let any_date = make_date(2025, 1, 1);
    assert!(!composite.is_holiday(any_date));
}

#[test]
fn composite_empty_calendars_intersection() {
    let calendars: &[&dyn HolidayCalendar] = &[];
    let composite = CompositeCalendar::with_mode(calendars, CompositeMode::Intersection);

    // Empty intersection should have no holidays
    let any_date = make_date(2025, 1, 1);
    assert!(!composite.is_holiday(any_date));
}

#[test]
fn composite_single_calendar_behaves_like_original() {
    let t2 = TARGET2;
    let calendars = [&t2 as &dyn HolidayCalendar];

    let composite_union = CompositeCalendar::new(&calendars);
    let composite_inter = CompositeCalendar::with_mode(&calendars, CompositeMode::Intersection);

    // Test several dates
    for day in 1..=28 {
        let date = make_date(2025, 1, day);
        let original = t2.is_holiday(date);
        assert_eq!(composite_union.is_holiday(date), original);
        assert_eq!(composite_inter.is_holiday(date), original);
    }
}

#[test]
fn composite_multiple_calendars_union() {
    let nyse = NYSE;
    let usny = USNY;
    let t2 = TARGET2;

    let calendars = [
        &nyse as &dyn HolidayCalendar,
        &usny as &dyn HolidayCalendar,
        &t2 as &dyn HolidayCalendar,
    ];

    let composite = CompositeCalendar::new(&calendars);

    // Any US or EU holiday should be marked
    let us_labor_day = make_date(2025, 9, 1); // First Monday of Sept
    assert!(composite.is_holiday(us_labor_day));

    // Christmas is a holiday everywhere
    let christmas = make_date(2025, 12, 25);
    assert!(composite.is_holiday(christmas));
}

#[test]
fn composite_business_day_trait_implementation() {
    let t2 = TARGET2;
    let gb = GBLO;
    let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

    let composite = CompositeCalendar::new(&calendars);

    // Weekends should not be business days
    let saturday = make_date(2025, 1, 4);
    assert!(!composite.is_business_day(saturday));

    let sunday = make_date(2025, 1, 5);
    assert!(!composite.is_business_day(sunday));

    // Holiday should not be business day
    let holiday = make_date(2025, 1, 1);
    assert!(!composite.is_business_day(holiday));

    // Regular weekday should be business day
    let weekday = make_date(2025, 1, 2);
    assert!(composite.is_business_day(weekday));
}
