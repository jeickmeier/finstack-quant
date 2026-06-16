//! FX settlement date benchmarks (Phase 2).
//!
//! This benchmark suite tests the performance of joint business day
//! calculations for FX spot settlement introduced in Phase 2.1.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::{BusinessDayConvention, Date};
use finstack_quant_valuations::instruments::{add_joint_business_days, roll_spot_date};
use std::hint::black_box;
use time::Month;

/// Benchmark add_joint_business_days for various scenarios
fn bench_add_joint_business_days(c: &mut Criterion) {
    let trade_date = Date::from_calendar_date(2024, Month::December, 27).unwrap(); // Friday before year-end
    let bdc = BusinessDayConvention::Following;

    c.bench_function("add_joint_business_days_usd_eur_2days", |b| {
        b.iter(|| {
            add_joint_business_days(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("target2")),
            )
        })
    });

    c.bench_function("add_joint_business_days_gbp_jpy_2days", |b| {
        b.iter(|| {
            add_joint_business_days(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("gblo")),
                black_box(Some("jpto")),
            )
        })
    });

    // Test with longer horizons
    c.bench_function("add_joint_business_days_usd_eur_5days", |b| {
        b.iter(|| {
            add_joint_business_days(
                black_box(trade_date),
                black_box(5),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("target2")),
            )
        })
    });

    c.bench_function("add_joint_business_days_usd_eur_10days", |b| {
        b.iter(|| {
            add_joint_business_days(
                black_box(trade_date),
                black_box(10),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("target2")),
            )
        })
    });
}

/// Benchmark roll_spot_date for FX settlement
fn bench_roll_spot_date(c: &mut Criterion) {
    let trade_date = Date::from_calendar_date(2024, Month::December, 27).unwrap();
    let bdc = BusinessDayConvention::Following;

    c.bench_function("roll_spot_date_usd_eur_t2", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2), // T+2
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("target2")),
            )
        })
    });

    c.bench_function("roll_spot_date_gbp_jpy_t2", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("gblo")),
                black_box(Some("jpto")),
            )
        })
    });

    c.bench_function("roll_spot_date_usd_gbp_t2", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("gblo")),
            )
        })
    });

    // Test with no calendars (weekends only)
    c.bench_function("roll_spot_date_weekends_only_t2", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(None),
                black_box(None),
            )
        })
    });
}

/// Benchmark FX settlement for different date scenarios
fn bench_fx_settlement_scenarios(c: &mut Criterion) {
    let bdc = BusinessDayConvention::Following;

    // Different trade dates to test holiday handling
    let scenarios = vec![
        (
            "regular_weekday",
            Date::from_calendar_date(2024, Month::June, 5).unwrap(),
        ), // Regular Wednesday
        (
            "before_weekend",
            Date::from_calendar_date(2024, Month::June, 7).unwrap(),
        ), // Friday
        (
            "year_end",
            Date::from_calendar_date(2024, Month::December, 27).unwrap(),
        ), // Friday before year-end
        (
            "near_holiday",
            Date::from_calendar_date(2024, Month::July, 3).unwrap(),
        ), // Wednesday before July 4th
    ];

    let mut group = c.benchmark_group("fx_settlement_scenarios");

    for (name, trade_date) in scenarios {
        group.bench_with_input(
            BenchmarkId::new("usd_eur", name),
            &trade_date,
            |b, &date| {
                b.iter(|| {
                    roll_spot_date(
                        black_box(date),
                        black_box(2),
                        black_box(bdc),
                        black_box(Some("nyse")),
                        black_box(Some("target2")),
                    )
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("gbp_jpy", name),
            &trade_date,
            |b, &date| {
                b.iter(|| {
                    roll_spot_date(
                        black_box(date),
                        black_box(2),
                        black_box(bdc),
                        black_box(Some("gblo")),
                        black_box(Some("jpto")),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark FX settlement for a batch of trades (portfolio-like)
fn bench_fx_settlement_batch(c: &mut Criterion) {
    let bdc = BusinessDayConvention::Following;

    // Create a batch of 100 trades over a month
    let base_date = Date::from_calendar_date(2024, Month::June, 1).unwrap();
    let trade_dates: Vec<Date> = (0..100)
        .map(|i| base_date + time::Duration::days((i * 30) / 100))
        .collect();

    c.bench_function("fx_settlement_batch_100_usd_eur", |b| {
        b.iter(|| {
            trade_dates
                .iter()
                .map(|&date| {
                    roll_spot_date(
                        black_box(date),
                        black_box(2),
                        black_box(bdc),
                        black_box(Some("nyse")),
                        black_box(Some("target2")),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
    });

    c.bench_function("fx_settlement_batch_100_gbp_jpy", |b| {
        b.iter(|| {
            trade_dates
                .iter()
                .map(|&date| {
                    roll_spot_date(
                        black_box(date),
                        black_box(2),
                        black_box(bdc),
                        black_box(Some("gblo")),
                        black_box(Some("jpto")),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
    });
}

/// Benchmark comparison: old approach (calendar days) vs new (joint business days)
///
/// Note: This is conceptual - the old approach is no longer available after Phase 2.
/// We benchmark the new correct approach across different calendar combinations.
fn bench_calendar_complexity(c: &mut Criterion) {
    let trade_date = Date::from_calendar_date(2024, Month::December, 27).unwrap();
    let bdc = BusinessDayConvention::Following;

    // Weekends only (simplest)
    c.bench_function("calendar_complexity_weekends_only", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(None),
                black_box(None),
            )
        })
    });

    // One real calendar (moderate)
    c.bench_function("calendar_complexity_one_calendar_usd", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(None),
            )
        })
    });

    // Two real calendars (full joint calculation)
    c.bench_function("calendar_complexity_two_calendars_usd_eur", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(trade_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("nyse")),
                black_box(Some("target2")),
            )
        })
    });

    // Two calendars with many holidays (GBP/JPY around Golden Week)
    let golden_week_date = Date::from_calendar_date(2025, Month::April, 28).unwrap();
    c.bench_function("calendar_complexity_golden_week_gbp_jpy", |b| {
        b.iter(|| {
            roll_spot_date(
                black_box(golden_week_date),
                black_box(2),
                black_box(bdc),
                black_box(Some("gblo")),
                black_box(Some("jpto")),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_add_joint_business_days,
    bench_roll_spot_date,
    bench_fx_settlement_scenarios,
    bench_fx_settlement_batch,
    bench_calendar_complexity
);
criterion_main!(benches);
