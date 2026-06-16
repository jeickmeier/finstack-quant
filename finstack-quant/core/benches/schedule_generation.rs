//! Benchmarks for date schedule generation.
//!
//! Tests performance of:
//! - ScheduleBuilder with various frequencies (monthly, quarterly, semi-annual, annual)
//! - Stub conventions (short front, short back, long front, long back)
//! - End-of-month handling
//! - IMM and CDS-IMM schedule generation
//! - Business day adjustment overhead
//! - Long-tenor schedule scaling (5Y, 10Y, 30Y)

mod bench_utils;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::{
    BusinessDayConvention, CalendarRegistry, ScheduleBuilder, StubKind, Tenor,
};
use std::hint::black_box;
use time::{Date, Month};

fn start_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).expect("valid bench date")
}

fn end_date_years(years: u32) -> Date {
    Date::from_calendar_date(2025 + years as i32, Month::January, 15).expect("valid bench date")
}

fn bench_frequency_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_frequency");
    let start = start_date();
    let end = end_date_years(10);

    let frequencies = [
        ("monthly", Tenor::monthly()),
        ("quarterly", Tenor::quarterly()),
        ("semi_annual", Tenor::semi_annual()),
        ("annual", Tenor::annual()),
    ];

    for (name, freq) in frequencies {
        bench_iter(&mut group, name, || {
            let sched = ScheduleBuilder::new(start, end)
                .unwrap()
                .frequency(freq)
                .build()
                .unwrap();
            black_box(sched);
        });
    }

    group.finish();
}

fn bench_stub_conventions(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_stub_kind");
    let start = start_date();
    let end = end_date_years(10);

    let stubs = [
        ("none", StubKind::None),
        ("short_front", StubKind::ShortFront),
        ("short_back", StubKind::ShortBack),
        ("long_front", StubKind::LongFront),
        ("long_back", StubKind::LongBack),
    ];

    for (name, stub) in stubs {
        bench_iter(&mut group, name, || {
            let sched = ScheduleBuilder::new(start, end)
                .unwrap()
                .frequency(Tenor::quarterly())
                .stub_rule(stub)
                .build()
                .unwrap();
            black_box(sched);
        });
    }

    group.finish();
}

fn bench_tenor_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_tenor_scaling");
    let start = start_date();

    for years in [1, 2, 5, 10, 20, 30] {
        let end = end_date_years(years);
        group.bench_with_input(
            BenchmarkId::new("quarterly", format!("{years}y")),
            &years,
            |b, _| {
                b.iter(|| {
                    let sched = ScheduleBuilder::new(start, end)
                        .unwrap()
                        .frequency(Tenor::quarterly())
                        .build()
                        .unwrap();
                    black_box(sched);
                })
            },
        );
    }

    for years in [1, 2, 5, 10, 20, 30] {
        let end = end_date_years(years);
        group.bench_with_input(
            BenchmarkId::new("monthly", format!("{years}y")),
            &years,
            |b, _| {
                b.iter(|| {
                    let sched = ScheduleBuilder::new(start, end)
                        .unwrap()
                        .frequency(Tenor::monthly())
                        .build()
                        .unwrap();
                    black_box(sched);
                })
            },
        );
    }

    group.finish();
}

fn bench_end_of_month(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_eom");
    let start = Date::from_calendar_date(2025, Month::January, 31).expect("valid bench date");
    let end = Date::from_calendar_date(2035, Month::January, 31).expect("valid bench date");

    bench_iter(&mut group, "eom_false", || {
        let sched = ScheduleBuilder::new(start, end)
            .unwrap()
            .frequency(Tenor::monthly())
            .end_of_month(false)
            .build()
            .unwrap();
        black_box(sched);
    });

    bench_iter(&mut group, "eom_true", || {
        let sched = ScheduleBuilder::new(start, end)
            .unwrap()
            .frequency(Tenor::monthly())
            .end_of_month(true)
            .build()
            .unwrap();
        black_box(sched);
    });

    group.finish();
}

fn bench_imm_schedules(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_imm");
    let start = start_date();

    for years in [1, 5, 10] {
        let end = end_date_years(years);

        group.bench_with_input(
            BenchmarkId::new("cds_imm", format!("{years}y")),
            &years,
            |b, _| {
                b.iter(|| {
                    let sched = ScheduleBuilder::new(start, end)
                        .unwrap()
                        .cds_imm()
                        .build()
                        .unwrap();
                    black_box(sched);
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("standard_imm", format!("{years}y")),
            &years,
            |b, _| {
                b.iter(|| {
                    let sched = ScheduleBuilder::new(start, end)
                        .unwrap()
                        .imm()
                        .build()
                        .unwrap();
                    black_box(sched);
                })
            },
        );
    }

    group.finish();
}

fn bench_business_day_adjustment(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_bday_adjust");
    let start = start_date();
    let end = end_date_years(10);

    bench_iter(&mut group, "unadjusted", || {
        let sched = ScheduleBuilder::new(start, end)
            .unwrap()
            .frequency(Tenor::quarterly())
            .build()
            .unwrap();
        black_box(sched);
    });

    if let Some(nyse) = CalendarRegistry::global().resolve_str("nyse") {
        bench_iter(&mut group, "mod_following_nyse", || {
            let sched = ScheduleBuilder::new(start, end)
                .unwrap()
                .frequency(Tenor::quarterly())
                .adjust_with(BusinessDayConvention::ModifiedFollowing, nyse)
                .build()
                .unwrap();
            black_box(sched);
        });

        bench_iter(&mut group, "following_nyse", || {
            let sched = ScheduleBuilder::new(start, end)
                .unwrap()
                .frequency(Tenor::quarterly())
                .adjust_with(BusinessDayConvention::Following, nyse)
                .build()
                .unwrap();
            black_box(sched);
        });
    }

    group.finish();
}

fn bench_schedule_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_iteration");
    let start = start_date();
    let end = end_date_years(30);

    let sched = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build()
        .unwrap();

    bench_iter(&mut group, "iterate_30y_monthly", || {
        let count = black_box(sched.clone()).into_iter().count();
        black_box(count);
    });

    bench_iter(&mut group, "collect_30y_monthly", || {
        let dates: Vec<_> = black_box(sched.clone()).into_iter().collect();
        black_box(dates);
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_frequency_variants,
    bench_stub_conventions,
    bench_tenor_scaling,
    bench_end_of_month,
    bench_imm_schedules,
    bench_business_day_adjustment,
    bench_schedule_iteration,
);
criterion_main!(benches);
