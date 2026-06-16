//! Tests for calendar registry functionality

use finstack_quant_core::dates::calendar::{GBLO, NYSE, TARGET2};
use finstack_quant_core::dates::{CalendarRegistry, CompositeCalendar, CompositeMode};
use finstack_quant_core::dates::{Date, HolidayCalendar};
use finstack_quant_core::types::CalendarId;
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn registry_resolves_all_built_in_calendars() {
    let registry = CalendarRegistry::global();
    let ids = registry.available_ids();

    assert!(!ids.is_empty());

    // Test that all IDs resolve successfully
    for &id in ids {
        let cal = registry.resolve_str(id);
        assert!(cal.is_some(), "Calendar '{}' should resolve", id);
    }
}

#[test]
fn registry_resolve_str_is_case_insensitive() {
    let registry = CalendarRegistry::global();

    // Test lowercase
    let lower = registry.resolve_str("gblo");
    assert!(lower.is_some());

    // Test uppercase (should still work due to to_lowercase in implementation)
    let upper = registry.resolve_str("GBLO");
    assert!(upper.is_some());
}

#[test]
fn registry_resolve_returns_none_for_unknown() {
    let registry = CalendarRegistry::global();
    let unknown = registry.resolve_str("nonexistent_calendar");
    assert!(unknown.is_none());
}

#[test]
fn registry_resolve_with_calendar_id() {
    let registry = CalendarRegistry::global();

    let id = CalendarId::from(TARGET2.id());
    let cal = registry.resolve(&id);
    assert!(cal.is_some());

    // Verify the calendar works
    let jan1 = make_date(2025, 1, 1);
    assert!(cal.unwrap().is_holiday(jan1));
}

#[test]
fn registry_resolve_many_vec_builds_list() {
    let registry = CalendarRegistry::global();

    let ids = [
        CalendarId::from(TARGET2.id()),
        CalendarId::from(GBLO.id()),
        CalendarId::from(NYSE.id()),
    ];

    let calendars = registry.resolve_many_vec(&ids);

    assert_eq!(calendars.len(), 3);

    // Verify each calendar is functional
    let test_date = make_date(2025, 1, 1);
    for cal in &calendars {
        let _ = cal.is_holiday(test_date);
    }
}

#[test]
fn registry_resolve_many_into_composite() {
    let registry = CalendarRegistry::global();

    let ids = [CalendarId::from(TARGET2.id()), CalendarId::from(GBLO.id())];

    let calendars = registry.resolve_many_vec(&ids);
    let composite = CompositeCalendar::with_mode(&calendars[..], CompositeMode::Union);

    // Test that composite works
    let jan1 = make_date(2025, 1, 1);
    assert!(composite.is_holiday(jan1));

    // Test UK-specific holiday
    let may26 = make_date(2025, 5, 26); // Spring bank holiday
    assert!(composite.is_holiday(may26));
}

#[test]
fn registry_resolve_many_handles_unknown_ids() {
    let registry = CalendarRegistry::global();

    let ids = [
        CalendarId::from(TARGET2.id()),
        CalendarId::from("unknown_calendar"),
        CalendarId::from(GBLO.id()),
    ];

    let calendars = registry.resolve_many_vec(&ids);

    // Should only resolve the valid ones
    assert_eq!(calendars.len(), 2);
}

#[test]
fn registry_available_ids_matches_all_ids() {
    let registry = CalendarRegistry::global();
    let ids = registry.available_ids();

    // Should contain known calendars
    assert!(ids.contains(&"gblo"));
    assert!(ids.contains(&"target2"));
    assert!(ids.contains(&"nyse"));
    assert!(ids.contains(&"usny"));
}

#[test]
fn registry_is_singleton() {
    let reg1 = CalendarRegistry::global();
    let reg2 = CalendarRegistry::global();

    // Should be the same instance
    let ptr1 = reg1 as *const CalendarRegistry;
    let ptr2 = reg2 as *const CalendarRegistry;
    assert_eq!(ptr1, ptr2);
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

#[test]
fn resolve_many_preserves_order() {
    let registry = CalendarRegistry::global();

    let ids = [
        CalendarId::from(GBLO.id()),
        CalendarId::from(TARGET2.id()),
        CalendarId::from(NYSE.id()),
    ];

    let calendars = registry.resolve_many_vec(&ids);

    // Verify order is preserved by checking metadata
    let meta0 = calendars[0].metadata().unwrap();
    let meta1 = calendars[1].metadata().unwrap();
    let meta2 = calendars[2].metadata().unwrap();

    assert_eq!(meta0.id, "gblo");
    assert_eq!(meta1.id, "target2");
    assert_eq!(meta2.id, "nyse");
}
