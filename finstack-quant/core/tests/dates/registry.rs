//! Tests for free calendar resolution APIs.

use finstack_quant_core::dates::calendar::{GBLO, NYSE, TARGET2};
use finstack_quant_core::dates::{
    available_calendars, calendar_by_id, calendars_by_ids, CompositeCalendar, CompositeMode, Date,
    HolidayCalendar,
};
use finstack_quant_core::types::CalendarId;
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), d).expect("valid date")
}

#[test]
fn resolves_all_built_in_calendars() {
    let ids = available_calendars();
    assert!(!ids.is_empty());
    for &id in ids {
        assert!(
            calendar_by_id(id).is_some(),
            "Calendar '{id}' should resolve"
        );
    }
}

#[test]
fn resolution_is_case_insensitive_and_unknown_is_none() {
    assert!(calendar_by_id("gblo").is_some());
    assert!(calendar_by_id("GBLO").is_some());
    assert!(calendar_by_id("nonexistent_calendar").is_none());
}

#[test]
fn typed_id_resolves() {
    let id = CalendarId::from(TARGET2.id());
    let cal = calendar_by_id(id.as_str()).expect("TARGET2 resolves");
    assert!(cal.is_holiday(make_date(2025, 1, 1)));
}

#[test]
fn strict_many_resolution_preserves_order_and_builds_composite() {
    let ids = [
        CalendarId::from(GBLO.id()),
        CalendarId::from(TARGET2.id()),
        CalendarId::from(NYSE.id()),
    ];
    let calendars = calendars_by_ids(&ids).expect("known calendars resolve");
    assert_eq!(calendars.len(), 3);
    assert_eq!(calendars[0].metadata().expect("metadata").id, "gblo");
    assert_eq!(calendars[1].metadata().expect("metadata").id, "target2");
    assert_eq!(calendars[2].metadata().expect("metadata").id, "nyse");

    let composite = CompositeCalendar::with_mode(&calendars[..2], CompositeMode::Union);
    assert!(composite.is_holiday(make_date(2025, 1, 1)));
    assert!(composite.is_holiday(make_date(2025, 5, 26)));
}

#[test]
fn strict_many_resolution_rejects_unknown_ids() {
    let ids = [
        CalendarId::from(TARGET2.id()),
        CalendarId::from("unknown_calendar"),
        CalendarId::from(GBLO.id()),
    ];
    let error = calendars_by_ids(&ids)
        .err()
        .expect("unknown calendar must fail the whole resolution");
    assert!(error.to_string().contains("unknown_calendar"));
}

#[test]
fn available_ids_contains_market_calendars() {
    let ids = available_calendars();
    assert!(ids.contains(&"gblo"));
    assert!(ids.contains(&"target2"));
    assert!(ids.contains(&"nyse"));
    assert!(ids.contains(&"usny"));
}

#[test]
fn calendar_id_equality_and_hashing() {
    use std::collections::HashSet;

    let id1 = CalendarId::from("gblo");
    let id2 = CalendarId::from("gblo");
    let id3 = CalendarId::from("target2");

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);

    let mut set = HashSet::new();
    set.insert(id1);
    assert!(set.contains(&id2));
    assert!(!set.contains(&id3));
}
