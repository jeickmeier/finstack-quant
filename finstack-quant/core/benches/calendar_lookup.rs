//! Measurement for the `is_holiday` rule-scan disposition.
//!
//! `Calendar::is_holiday` evaluates every holiday rule linearly per query. The
//! module documentation characterises this as "a short linear scan; typically a
//! handful of rules per calendar". Rule counts actually range from 7 (`asx`) to
//! 50 (`sse`, `cnbe`), so this bench measures the spread across that range and
//! the business-day-adjustment shape that consumes it.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::calendar_by_id_strict;
use std::hint::black_box;
use time::{Date, Month};

/// Calendars spanning the observed rule-count range, low to high.
const CALENDARS: &[(&str, usize)] = &[
    ("asx", 7),
    ("nyse", 15),
    ("jpx", 20),
    ("bse", 44),
    ("cnbe", 50),
];

fn sample_dates() -> Vec<Date> {
    // One full year of consecutive dates: the shape a schedule build produces.
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    (0..365)
        .map(|i| {
            start
                .checked_add(time::Duration::days(i))
                .expect("in range")
        })
        .collect()
}

/// Per-query cost as a function of rule count. If the scan is genuinely cheap
/// the curve is flat; if it is linear in rules, `cnbe` costs ~7x `asx`.
fn bench_is_holiday_by_rule_count(c: &mut Criterion) {
    let dates = sample_dates();
    let mut group = c.benchmark_group("calendar_is_holiday_by_rule_count");
    for (id, rules) in CALENDARS {
        let Ok(cal) = calendar_by_id_strict(id) else {
            continue;
        };
        group.bench_with_input(BenchmarkId::new(*id, rules), rules, |b, _| {
            b.iter(|| {
                let mut n = 0usize;
                for d in &dates {
                    if cal.is_holiday(black_box(*d)) {
                        n += 1;
                    }
                }
                black_box(n)
            });
        });
    }
    group.finish();
}

/// `is_business_day` is the function the adjustment path actually calls.
fn bench_is_business_day(c: &mut Criterion) {
    let dates = sample_dates();
    let mut group = c.benchmark_group("calendar_is_business_day");
    for (id, rules) in CALENDARS {
        let Ok(cal) = calendar_by_id_strict(id) else {
            continue;
        };
        group.bench_with_input(BenchmarkId::new(*id, rules), rules, |b, _| {
            b.iter(|| {
                let mut n = 0usize;
                for d in &dates {
                    if cal.is_business_day(black_box(*d)) {
                        n += 1;
                    }
                }
                black_box(n)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_is_holiday_by_rule_count,
    bench_is_business_day
);
criterion_main!(benches);
