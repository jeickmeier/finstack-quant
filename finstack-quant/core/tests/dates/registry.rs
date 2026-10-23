//! Tests for free calendar resolution APIs.

use super::common::make_date;
use finstack_quant_core::dates::{available_calendars, calendar_by_id_strict, HolidayCalendar};
use finstack_quant_core::types::CalendarId;

#[test]
fn resolves_all_built_in_calendars() {
    let ids = available_calendars();
    assert!(!ids.is_empty());
    for &id in ids {
        assert!(
            calendar_by_id_strict(id).is_ok(),
            "Calendar '{id}' should resolve"
        );
    }
}

#[test]
fn resolution_is_case_insensitive_and_unknown_is_none() {
    assert!(calendar_by_id_strict("gblo").is_ok());
    assert!(calendar_by_id_strict("GBLO").is_ok());
    assert!(calendar_by_id_strict("nonexistent_calendar").is_err());
}

#[test]
fn typed_id_resolves() {
    let id = CalendarId::from("target2");
    let cal = calendar_by_id_strict(id.as_str()).expect("TARGET2 resolves");
    assert!(cal.is_holiday(make_date(2025, 1, 1)));
}

#[test]
fn strict_many_resolution_preserves_order_and_builds_composite() {
    let ids = [
        CalendarId::from("gblo"),
        CalendarId::from("target2"),
        CalendarId::from("nyse"),
    ];
    let calendars: Vec<&dyn HolidayCalendar> = ids
        .iter()
        .map(|id| calendar_by_id_strict(id.as_str()).expect("known calendars resolve"))
        .collect();
    assert_eq!(calendars.len(), 3);
    assert_eq!(calendars[0].metadata().expect("metadata").id, "gblo");
    assert_eq!(calendars[1].metadata().expect("metadata").id, "target2");
    assert_eq!(calendars[2].metadata().expect("metadata").id, "nyse");

    let composite = calendar_by_id_strict("gblo+target2").expect("joint id resolves");
    assert!(composite.is_holiday(make_date(2025, 1, 1)));
    assert!(composite.is_holiday(make_date(2025, 5, 26)));
}

#[test]
fn strict_many_resolution_rejects_unknown_ids() {
    let ids = [
        CalendarId::from("target2"),
        CalendarId::from("unknown_calendar"),
        CalendarId::from("gblo"),
    ];
    let error = ids
        .iter()
        .map(|id| calendar_by_id_strict(id.as_str()))
        .collect::<Result<Vec<_>, _>>()
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
