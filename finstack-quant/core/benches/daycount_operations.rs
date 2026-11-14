//! Checked day-count conventions, effective ISMA contexts and TARGET2 Bus/252.
//! Fixtures and numerical preflights run only inside each selected case.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::dates::{
    Date, DateExt, DayCount, DayCountContext, Tenor, calendar_by_id_strict,
};
use std::hint::black_box;
use time::Month;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid benchmark date")
}

fn assert_fraction(actual: f64, expected: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= 1e-12,
        "year fraction {actual} differs from reference {expected}"
    );
}

fn bench_daycount_year_fraction(c: &mut Criterion) {
    let conventions = [
        ("act_360", DayCount::Act360),
        ("act_365f", DayCount::Act365F),
        ("act_act", DayCount::ActAct),
        ("30_360", DayCount::Thirty360),
        ("30e_360", DayCount::ThirtyE360),
    ];
    let mut group = c.benchmark_group("daycount_year_fraction");
    for (name, convention) in conventions {
        for years in [1, 5, 10] {
            group.bench_function(format!("{name}_{years}y"), |b| {
                let start = date(2025, Month::January, 1);
                let end = date(2025 + years, Month::January, 1);
                let days = (end - start).whole_days() as f64;
                let expected = match convention {
                    DayCount::Act360 => days / 360.0,
                    DayCount::Act365F => days / 365.0,
                    _ => f64::from(years),
                };
                let actual = convention
                    .year_fraction(start, end, DayCountContext::default())
                    .expect("valid day-count fixture");
                assert_fraction(actual, expected);
                b.iter(|| {
                    let yf = black_box(convention)
                        .year_fraction(black_box(start), black_box(end), DayCountContext::default())
                        .expect("valid day-count fixture");
                    black_box(yf)
                });
            });
        }
    }
    group.finish();
}

fn bench_daycount_actact_isma(c: &mut Criterion) {
    let frequencies = [
        ("Annual", Tenor::annual(), 2025, Month::January, 1.0),
        ("SemiAnnual", Tenor::semi_annual(), 2024, Month::July, 0.5),
        ("Quarterly", Tenor::quarterly(), 2024, Month::April, 0.25),
        (
            "Monthly",
            Tenor::monthly(),
            2024,
            Month::February,
            1.0 / 12.0,
        ),
    ];
    for (name, frequency, end_year, end_month, expected) in frequencies {
        c.bench_function(&format!("daycount_actact_isma_frequency_{name}"), |b| {
            let start = date(2024, Month::January, 1);
            let end = date(end_year, end_month, 1);
            let ctx = DayCountContext {
                frequency: Some(frequency),
                coupon_period: Some((start, end)),
                ..DayCountContext::default()
            };
            assert!(start < end);
            assert_fraction(
                DayCount::ActActIsma
                    .year_fraction(start, end, ctx)
                    .expect("regular coupon"),
                expected,
            );
            b.iter(|| {
                black_box(
                    DayCount::ActActIsma
                        .year_fraction(black_box(start), black_box(end), ctx)
                        .expect("regular coupon"),
                )
            });
        });
    }
    c.bench_function("daycount_actact_isma_reference_partial", |b| {
        let start = date(2024, Month::February, 15);
        let end = date(2024, Month::May, 15);
        let reference_start = date(2024, Month::January, 1);
        let reference_end = date(2024, Month::July, 1);
        let ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some((reference_start, reference_end)),
            ..DayCountContext::default()
        };
        assert!(reference_start < start && end < reference_end);
        let expected = (end - start).whole_days() as f64
            / (reference_end - reference_start).whole_days() as f64
            * 0.5;
        assert_fraction(
            DayCount::ActActIsma
                .year_fraction(start, end, ctx)
                .expect("partial coupon"),
            expected,
        );
        b.iter(|| {
            black_box(
                DayCount::ActActIsma
                    .year_fraction(black_box(start), black_box(end), ctx)
                    .expect("partial coupon"),
            )
        });
    });
}

fn bench_daycount_bus252(c: &mut Criterion) {
    let periods = [
        ("1m", 2025, Month::February),
        ("3m", 2025, Month::April),
        ("6m", 2025, Month::July),
        ("1y", 2026, Month::January),
    ];
    for (name, end_year, end_month) in periods {
        c.bench_function(&format!("daycount_bus252_{name}"), |b| {
            let calendar = calendar_by_id_strict("target2").expect("TARGET2 calendar");
            let start = date(2025, Month::January, 1);
            let end = date(end_year, end_month, 1);
            assert!(start < end);
            // TARGET2 closes on these six 2025 dates. Count weekdays directly,
            // independently of calendar classification and cached prefix sums.
            let closures = [
                date(2025, Month::January, 1),
                date(2025, Month::April, 18),
                date(2025, Month::April, 21),
                date(2025, Month::May, 1),
                date(2025, Month::December, 25),
                date(2025, Month::December, 26),
            ];
            let expected_days = (0..(end - start).whole_days())
                .map(|offset| start + time::Duration::days(offset))
                .filter(|day| !day.is_weekend() && !closures.contains(day))
                .count();
            assert!(expected_days > 0);
            let ctx = DayCountContext {
                calendar: Some(calendar),
                ..DayCountContext::default()
            };
            assert_fraction(
                DayCount::Bus252
                    .year_fraction(start, end, ctx)
                    .expect("valid Bus/252"),
                expected_days as f64 / 252.0,
            );
            b.iter(|| {
                black_box(
                    DayCount::Bus252
                        .year_fraction(black_box(start), black_box(end), ctx)
                        .expect("valid Bus/252"),
                )
            });
        });
    }
    c.bench_function("daycount_bus252_empty", |b| {
        let calendar = calendar_by_id_strict("target2").expect("TARGET2 calendar");
        let start = date(2025, Month::January, 1);
        let ctx = DayCountContext {
            calendar: Some(calendar),
            ..DayCountContext::default()
        };
        assert_fraction(
            DayCount::Bus252
                .year_fraction(start, start, ctx)
                .expect("empty period"),
            0.0,
        );
        b.iter(|| {
            black_box(
                DayCount::Bus252
                    .year_fraction(black_box(start), black_box(start), ctx)
                    .expect("empty period"),
            )
        });
    });
}

fn bench_daycount_batch_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("daycount_batch");
    let size = 100;
    group.bench_with_input(
        BenchmarkId::new("year_fractions", size),
        &size,
        |b, &size| {
            let start = date(2025, Month::January, 1);
            let dates: Vec<_> = (1..=size)
                .map(|i| {
                    date(
                        2025 + i / 12,
                        Month::try_from((i % 12 + 1) as u8).expect("month"),
                        1,
                    )
                })
                .collect();
            let calculate = || {
                dates
                    .iter()
                    .map(|&end| {
                        DayCount::Act360
                            .year_fraction(start, end, DayCountContext::default())
                            .expect("valid batch date")
                    })
                    .collect::<Vec<_>>()
            };
            let results = calculate();
            assert_eq!(results.len(), size as usize);
            for (&end, &actual) in dates.iter().zip(&results) {
                assert_fraction(actual, (end - start).whole_days() as f64 / 360.0);
            }
            b.iter(|| black_box(calculate()));
        },
    );
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
