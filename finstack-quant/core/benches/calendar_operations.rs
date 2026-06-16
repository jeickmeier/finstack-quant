//! Benchmarks for calendar and business day operations.
//!
//! Tests performance of:
//! - Holiday checking across calendars
//! - Business day adjustments
//! - Business day counting between dates
//! - Calendar composite operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::calendar::{GBLO, NYSE, TARGET2, USNY};
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, CompositeCalendar, Date, HolidayCalendar,
};
use std::hint::black_box;
use time::Month;

fn bench_holiday_checks(c: &mut Criterion) {
    let calendars: Vec<(&str, &dyn HolidayCalendar)> = vec![
        ("NYSE", &NYSE as &dyn HolidayCalendar),
        ("TARGET2", &TARGET2),
        ("GBLO", &GBLO),
        ("USNY", &USNY),
    ];

    let test_dates = [
        (
            "weekday",
            Date::from_calendar_date(2025, Month::March, 3).unwrap(),
        ), // Monday
        (
            "weekend",
            Date::from_calendar_date(2025, Month::March, 1).unwrap(),
        ), // Saturday
        (
            "holiday",
            Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        ), // New Year
        (
            "business",
            Date::from_calendar_date(2025, Month::March, 15).unwrap(),
        ), // Regular day
    ];

    for (cal_name, calendar) in &calendars {
        for (date_name, date) in &test_dates {
            c.bench_function(
                &format!("calendar_is_holiday_{}_{}", cal_name, date_name),
                |b| {
                    b.iter(|| {
                        let result = black_box(*calendar).is_holiday(black_box(*date));
                        black_box(result);
                    })
                },
            );

            c.bench_function(
                &format!("calendar_is_business_day_{}_{}", cal_name, date_name),
                |b| {
                    b.iter(|| {
                        let result = black_box(*calendar).is_business_day(black_box(*date));
                        black_box(result);
                    })
                },
            );
        }
    }
}

fn bench_business_day_adjustments(c: &mut Criterion) {
    let calendar = &NYSE;
    let date = Date::from_calendar_date(2025, Month::January, 1).unwrap(); // Holiday

    let conventions = [
        ("Following", BusinessDayConvention::Following),
        ("Preceding", BusinessDayConvention::Preceding),
        (
            "ModifiedFollowing",
            BusinessDayConvention::ModifiedFollowing,
        ),
        (
            "ModifiedPreceding",
            BusinessDayConvention::ModifiedPreceding,
        ),
    ];

    for (name, convention) in conventions {
        c.bench_function(&format!("calendar_adjust_{}", name), |b| {
            b.iter(|| {
                let result =
                    adjust(black_box(date), black_box(convention), black_box(calendar)).unwrap();
                black_box(result);
            })
        });
    }
}

fn bench_business_days_between(c: &mut Criterion) {
    let calendar = &NYSE;
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let periods = [
        (
            "1w",
            Date::from_calendar_date(2025, Month::January, 8).unwrap(),
        ),
        (
            "1m",
            Date::from_calendar_date(2025, Month::February, 1).unwrap(),
        ),
        (
            "3m",
            Date::from_calendar_date(2025, Month::April, 1).unwrap(),
        ),
        (
            "6m",
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        ),
        (
            "1y",
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
        ),
    ];

    for (name, end) in periods {
        c.bench_function(&format!("calendar_business_days_between_{}", name), |b| {
            b.iter(|| {
                // Count business days manually since business_days_between is not a method
                let mut count = 0;
                let mut current = start;
                while current < end {
                    if calendar.is_business_day(current) {
                        count += 1;
                    }
                    current = current.saturating_add(time::Duration::days(1));
                }
                black_box(count);
            })
        });
    }
}

fn bench_composite_calendar(c: &mut Criterion) {
    let calendars: &[&dyn HolidayCalendar] = &[&NYSE as &dyn HolidayCalendar, &TARGET2];
    let composite = CompositeCalendar::new(calendars);
    let date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    c.bench_function("calendar_composite_is_holiday", |b| {
        b.iter(|| {
            let result = black_box(&composite).is_holiday(black_box(date));
            black_box(result);
        })
    });

    c.bench_function("calendar_composite_is_business_day", |b| {
        b.iter(|| {
            let result = black_box(&composite).is_business_day(black_box(date));
            black_box(result);
        })
    });

    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    c.bench_function("calendar_composite_business_days_between", |b| {
        b.iter(|| {
            // Count business days manually
            let mut count = 0;
            let mut current = date;
            while current < end {
                if composite.is_business_day(current) {
                    count += 1;
                }
                current = current.saturating_add(time::Duration::days(1));
            }
            black_box(count);
        })
    });
}

fn bench_calendar_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("calendar_batch");
    let calendar = &NYSE;

    for size in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("check_holidays", size),
            &size,
            |b, &size| {
                let dates: Vec<_> = (0..size)
                    .map(|i| {
                        Date::from_calendar_date(
                            2025,
                            match ((i % 12) + 1) as u8 {
                                1 => Month::January,
                                2 => Month::February,
                                3 => Month::March,
                                4 => Month::April,
                                5 => Month::May,
                                6 => Month::June,
                                7 => Month::July,
                                8 => Month::August,
                                9 => Month::September,
                                10 => Month::October,
                                11 => Month::November,
                                _ => Month::December,
                            },
                            ((i % 28) + 1) as u8,
                        )
                        .unwrap()
                    })
                    .collect();

                b.iter(|| {
                    let results: Vec<_> = dates
                        .iter()
                        .map(|&date| calendar.is_business_day(date))
                        .collect();
                    black_box(results);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_holiday_checks,
    bench_business_day_adjustments,
    bench_business_days_between,
    bench_composite_calendar,
    bench_calendar_batch_operations,
);
criterion_main!(benches);
