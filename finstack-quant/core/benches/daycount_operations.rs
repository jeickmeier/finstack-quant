//! Benchmarks for day count conventions.
//!
//! Tests performance of:
//! - Year fraction calculations for various conventions
//! - Calendar-based business day counting (Bus/252)
//! - ISMA vs ISDA ActAct variants
//! - 30/360 family conventions

mod bench_utils;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::calendar::TARGET2;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use std::hint::black_box;
use time::Month;

fn bench_daycount_year_fraction(c: &mut Criterion) {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end_1y = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let end_5y = Date::from_calendar_date(2030, Month::January, 1).unwrap();
    let end_10y = Date::from_calendar_date(2035, Month::January, 1).unwrap();

    let conventions = [
        ("Act360", DayCount::Act360),
        ("Act365F", DayCount::Act365F),
        ("ActAct", DayCount::ActAct),
        ("Thirty360", DayCount::Thirty360),
        ("ThirtyE360", DayCount::ThirtyE360),
    ];

    let mut group = c.benchmark_group("daycount_year_fraction");

    for (name, convention) in conventions {
        for (suffix, end) in [("1y", end_1y), ("5y", end_5y), ("10y", end_10y)] {
            bench_iter(&mut group, format!("{}_{}", name, suffix), || {
                let yf = black_box(convention)
                    .year_fraction(black_box(start), black_box(end), DayCountContext::default())
                    .unwrap();
                black_box(yf);
            });
        }
    }

    group.finish();
}

fn bench_daycount_actact_isma(c: &mut Criterion) {
    let start = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2024, Month::July, 1).unwrap();

    let frequencies = [
        ("Annual", Tenor::annual()),
        ("SemiAnnual", Tenor::semi_annual()),
        ("Quarterly", Tenor::quarterly()),
        ("Monthly", Tenor::monthly()),
    ];

    for (name, freq) in frequencies {
        bench_utils::bench_with_criterion(c, format!("daycount_actact_isma_{}", name), || {
            let yf = DayCount::ActActIsma
                .year_fraction(
                    black_box(start),
                    black_box(end),
                    DayCountContext {
                        calendar: None,
                        frequency: Some(freq),
                        bus_basis: None,
                        coupon_period: None,
                    },
                )
                .unwrap();
            black_box(yf);
        });
    }
}

fn bench_daycount_bus252(c: &mut Criterion) {
    let calendar = &TARGET2;

    let periods = [("1m", 1, 1), ("3m", 1, 1), ("6m", 1, 1), ("1y", 1, 1)];

    for (name, start_month, end_month) in periods {
        let start = Date::from_calendar_date(2025, Month::January, start_month).unwrap();
        let end = Date::from_calendar_date(2025, Month::January, end_month).unwrap();

        bench_utils::bench_with_criterion(c, format!("daycount_bus252_{}", name), || {
            let yf = DayCount::Bus252
                .year_fraction(
                    black_box(start),
                    black_box(end),
                    DayCountContext {
                        calendar: Some(calendar),
                        frequency: None,
                        bus_basis: None,
                        coupon_period: None,
                    },
                )
                .unwrap();
            black_box(yf);
        });
    }
}

fn bench_daycount_batch_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("daycount_batch");
    let convention = DayCount::Act360;
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for size in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("year_fractions", size),
            &size,
            |b, &size| {
                // Generate dates at monthly intervals
                let dates: Vec<_> = (1..=size)
                    .map(|i| {
                        Date::from_calendar_date(
                            2025 + (i / 12),
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
                            1,
                        )
                        .unwrap()
                    })
                    .collect();

                b.iter(|| {
                    let results: Vec<_> = dates
                        .iter()
                        .map(|&end| {
                            convention
                                .year_fraction(start, end, DayCountContext::default())
                                .unwrap()
                        })
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
    bench_daycount_year_fraction,
    bench_daycount_actact_isma,
    bench_daycount_bus252,
    bench_daycount_batch_calculations,
);
criterion_main!(benches);
