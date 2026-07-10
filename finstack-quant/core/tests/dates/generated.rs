//! Generated calendars wiring tests.
//!
//! These tests validate that build-time generated calendars are exposed through the
//! public `finstack_quant_core::dates` API (without relying on internal bitset helpers).

use finstack_quant_core::dates::calendar::{SSE, TARGET2, USNY};
use finstack_quant_core::dates::{CalendarRegistry, Date, HolidayCalendar};
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
fn sse_published_2024_2025_closures_are_embedded() {
    for date in [
        make_date(2024, 2, 9),
        make_date(2024, 4, 5),
        make_date(2024, 6, 10),
        make_date(2024, 9, 16),
        make_date(2024, 9, 17),
        make_date(2025, 1, 28),
        make_date(2025, 6, 2),
        make_date(2025, 10, 8),
    ] {
        assert!(SSE.is_holiday(date), "missing SSE closure {date}");
        assert!(!SSE.is_business_day(date), "SSE should be closed {date}");
    }
}

#[test]
fn holiday_rules_respect_effective_year() {
    use finstack_quant_core::dates::calendar::NYSE;

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

    // Tight boundary years — the exact off-by-one the gating prevents.
    // Juneteenth: 2021-06-19 was a Saturday (observed Fri 06-18), gated off
    // entirely; 2022-06-19 was a Sunday, observed Mon 06-20 — first NYSE close.
    assert!(
        !NYSE.is_holiday(make_date(2021, 6, 18)),
        "Juneteenth must be absent in 2021 (year before from_year=2022)"
    );
    assert!(
        NYSE.is_holiday(make_date(2022, 6, 20)),
        "Juneteenth observed Monday 2022-06-20 is the first NYSE closure"
    );
    // MLK: 1997 3rd Monday = Jan 20 (absent); 1998 3rd Monday = Jan 19 (present).
    assert!(
        !NYSE.is_holiday(make_date(1997, 1, 20)),
        "MLK Day must be absent in 1997 (year before from_year=1998)"
    );
    assert!(
        NYSE.is_holiday(make_date(1998, 1, 19)),
        "MLK Day 1998-01-19 is the first NYSE observance"
    );

    // Un-gated holidays remain valid historically.
    assert!(
        NYSE.is_holiday(make_date(1990, 1, 1)),
        "New Year's Day is always an NYSE holiday"
    );
}

#[test]
fn sifma_cme_respect_effective_year_for_new_holidays() {
    use finstack_quant_core::dates::calendar::{CME, SIFMA};

    // SIFMA (U.S. bond market): Juneteenth from 2022, MLK from 1998, and
    // Columbus Day (2nd Monday October, federal form) from 1971.
    assert!(
        !SIFMA.is_holiday(make_date(2021, 6, 18)),
        "SIFMA Juneteenth must be absent before 2022"
    );
    assert!(
        SIFMA.is_holiday(make_date(2022, 6, 20)),
        "SIFMA Juneteenth observed 2022-06-20"
    );
    assert!(
        !SIFMA.is_holiday(make_date(1997, 1, 20)),
        "SIFMA MLK Day must be absent before 1998"
    );
    assert!(
        SIFMA.is_holiday(make_date(1998, 1, 19)),
        "SIFMA MLK Day present from 1998"
    );
    assert!(
        !SIFMA.is_holiday(make_date(1970, 10, 12)),
        "SIFMA Columbus Day must be absent before 1971"
    );
    assert!(
        SIFMA.is_holiday(make_date(1971, 10, 11)),
        "SIFMA Columbus Day present from 1971"
    );

    // CME: Juneteenth from 2022, MLK from 1998.
    assert!(
        !CME.is_holiday(make_date(2021, 6, 18)),
        "CME Juneteenth must be absent before 2022"
    );
    assert!(
        CME.is_holiday(make_date(2022, 6, 20)),
        "CME Juneteenth observed 2022-06-20"
    );
    assert!(
        !CME.is_holiday(make_date(1997, 1, 20)),
        "CME MLK Day must be absent before 1998"
    );
    assert!(
        CME.is_holiday(make_date(1998, 1, 19)),
        "CME MLK Day present from 1998"
    );

    // Sanity: an un-gated holiday is present historically on both calendars.
    assert!(
        SIFMA.is_holiday(make_date(1990, 7, 4)),
        "Independence Day is always a SIFMA holiday"
    );
    assert!(
        CME.is_holiday(make_date(1990, 12, 25)),
        "Christmas is always a CME holiday"
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
