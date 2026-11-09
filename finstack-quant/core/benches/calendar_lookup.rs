//! Warm year-cache calendar classification of all 365 dates in 2025.
//!
//! Calendar resolution, date construction, known-date checks and cache warming
//! are outside timing and run only for the selected case. These are not raw
//! rule-scan or cold year-materialization measurements.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::dates::{DateExt, HolidayCalendar, calendar_by_id_strict};
use std::hint::black_box;
use time::{Date, Month};

const CALENDARS: &[&str] = &["asx", "nyse", "jpx", "bse", "sse"];

fn checked_fixture(id: &str) -> (&'static dyn HolidayCalendar, Vec<Date>) {
    let cal = calendar_by_id_strict(id).expect("registered benchmark calendar");
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let dates: Vec<_> = (0..365).map(|i| start + time::Duration::days(i)).collect();
    assert_eq!(dates.len(), 365);
    assert_eq!(dates.last().expect("nonempty dates").year(), 2025);

    // Published fixed-date closures independent of the full-year count.
    let holiday = match id {
        "bse" => Date::from_calendar_date(2025, Month::August, 15),
        "sse" => Date::from_calendar_date(2025, Month::October, 1),
        _ => Date::from_calendar_date(2025, Month::January, 1),
    }
    .expect("valid closure date");
    assert!(
        cal.is_holiday(holiday).expect("calendar lookup"),
        "{id}: expected fixed-date closure"
    );
    assert!(!cal.is_business_day(holiday).expect("calendar lookup"));
    let open = Date::from_calendar_date(2025, Month::January, 15).expect("valid open date");
    assert!(
        !cal.is_holiday(open).expect("calendar lookup"),
        "{id}: expected ordinary weekday"
    );
    assert!(cal.is_business_day(open).expect("calendar lookup"));

    let mut holidays = 0;
    let mut business_days = 0;
    for &date in &dates {
        let holiday = cal.is_holiday(date).expect("calendar lookup");
        let business = cal.is_business_day(date).expect("calendar lookup");
        assert_eq!(business, !date.is_weekend() && !holiday);
        holidays += usize::from(holiday);
        business_days += usize::from(business);
    }
    assert!(holidays > 0 && holidays < dates.len());
    assert!((200..=262).contains(&business_days));
    (cal, dates)
}

fn bench_is_holiday(c: &mut Criterion) {
    let mut group = c.benchmark_group("calendar_is_holiday_warm_2025");
    for id in CALENDARS {
        group.bench_with_input(BenchmarkId::new(*id, 365), id, |b, id| {
            let (cal, dates) = checked_fixture(id);
            b.iter(|| {
                let mut count = 0usize;
                for date in &dates {
                    count +=
                        usize::from(cal.is_holiday(black_box(*date)).expect("calendar lookup"));
                }
                black_box(count)
            });
        });
    }
    group.finish();
}

fn bench_is_business_day(c: &mut Criterion) {
    let mut group = c.benchmark_group("calendar_is_business_day_warm_2025");
    for id in CALENDARS {
        group.bench_with_input(BenchmarkId::new(*id, 365), id, |b, id| {
            let (cal, dates) = checked_fixture(id);
            b.iter(|| {
                let mut count = 0usize;
                for date in &dates {
                    count += usize::from(
                        cal.is_business_day(black_box(*date))
                            .expect("calendar lookup"),
                    );
                }
                black_box(count)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_is_holiday, bench_is_business_day);
criterion_main!(benches);
