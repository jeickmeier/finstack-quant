//! Benchmarks for day count conventions.
//!
//! Tests performance of:
//! - Year fraction calculations for various conventions
//! - Calendar-based business day counting (Bus/252)
//! - ISMA vs ISDA ActAct variants
//! - 30/360 family conventions

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::{calendar_by_id_strict, Date, DayCount, DayCountContext, Tenor};
use std::hint::black_box;
use time::Month;

fn bench_daycount_year_fraction(c: &mut Criterion) {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end_1y = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let end_5y = Date::from_calendar_date(2030, Month::January, 1).unwrap();
    let end_10y = Date::from_calendar_date(2035, Month::January, 1).unwrap();

    let conventions = [
        ("act_360", DayCount::Act360),
        ("act_365f", DayCount::Act365F),
        ("act_act", DayCount::ActAct),
        ("30_360", DayCount::Thirty360),
        ("30e_360", DayCount::ThirtyE360),
    ];

    let mut group = c.benchmark_group("daycount_year_fraction");

    for (name, convention) in conventions {
        for (suffix, end) in [("1y", end_1y), ("5y", end_5y), ("10y", end_10y)] {
            group.bench_function(format!("{}_{}", name, suffix), |b| {
                b.iter(|| {
                    let yf = black_box(convention)
                        .year_fraction(black_box(start), black_box(end), DayCountContext::default())
                        .unwrap();
                    black_box(yf);
                })
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

    for (name, frequency) in frequencies {
        c.bench_function(&format!("daycount_actact_isma_{}", name), |b| {
            b.iter(|| {
                let yf = DayCount::ActActIsma
                    .year_fraction(
                        black_box(start),
                        black_box(end),
                        DayCountContext {
                            calendar: None,
                            frequency: Some(frequency),
                            bus_basis: None,
                            coupon_period: Some((start, end)),
                            end_is_termination_date: false,
                        },
                    )
                    .unwrap();
                black_box(yf);
            })
        });
    }
}

fn bench_daycount_bus252(c: &mut Criterion) {
    let calendar = calendar_by_id_strict("target2").unwrap();

    let periods = [("1m", 1, 1), ("3m", 1, 1), ("6m", 1, 1), ("1y", 1, 1)];

    for (name, start_month, end_month) in periods {
        let start = Date::from_calendar_date(2025, Month::January, start_month).unwrap();
        let end = Date::from_calendar_date(2025, Month::January, end_month).unwrap();

        c.bench_function(&format!("daycount_bus252_{}", name), |b| {
            b.iter(|| {
                let yf = DayCount::Bus252
                    .year_fraction(
                        black_box(start),
                        black_box(end),
                        DayCountContext {
                            calendar: Some(calendar),
                            frequency: None,
                            bus_basis: None,
                            coupon_period: None,
                            end_is_termination_date: false,
                        },
                    )
                    .unwrap();
                black_box(yf);
            })
        });
    }
}

fn bench_daycount_batch_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("daycount_batch");
    let convention = DayCount::Act360;
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    {
        let size = 100;
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
