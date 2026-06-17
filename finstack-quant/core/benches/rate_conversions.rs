//! Benchmarks for interest rate compounding convention conversions.
//!
//! Tests performance of:
//! - Simple to periodic rate conversions
//! - Periodic to continuous rate conversions
//! - Round-trip conversions
//! - Batch conversion scenarios

#[path = "support/bench_utils.rs"]
mod bench_utils;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::Compounding;
use std::hint::black_box;

fn bench_simple_to_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_to_periodic");

    let test_cases = [
        ("short_period", 0.05, 0.25), // 3 months
        ("half_year", 0.05, 0.5),     // 6 months
        ("one_year", 0.05, 1.0),      // 1 year
        ("five_year", 0.05, 5.0),     // 5 years
    ];

    for (name, rate, yf) in test_cases {
        bench_iter(&mut group, name, || {
            let result = Compounding::Simple.convert_rate(
                black_box(rate),
                black_box(yf),
                &Compounding::SEMI_ANNUAL,
            );
            black_box(result);
        });
    }

    group.finish();
}

fn bench_periodic_to_continuous(c: &mut Criterion) {
    let mut group = c.benchmark_group("periodic_to_continuous");

    let conventions = [
        ("annual", Compounding::Annual),
        ("semiannual", Compounding::SEMI_ANNUAL),
        ("quarterly", Compounding::QUARTERLY),
        ("monthly", Compounding::MONTHLY),
    ];

    for (name, conv) in conventions {
        bench_iter(&mut group, name, || {
            let result =
                conv.convert_rate(black_box(0.05), black_box(1.0), &Compounding::Continuous);
            black_box(result);
        });
    }

    group.finish();
}

fn bench_continuous_to_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("continuous_to_periodic");

    let conventions = [
        ("annual", Compounding::Annual),
        ("semiannual", Compounding::SEMI_ANNUAL),
        ("quarterly", Compounding::QUARTERLY),
        ("monthly", Compounding::MONTHLY),
    ];

    for (name, conv) in conventions {
        bench_iter(&mut group, name, || {
            let result =
                Compounding::Continuous.convert_rate(black_box(0.05), black_box(1.0), &conv);
            black_box(result);
        });
    }

    group.finish();
}

fn bench_simple_continuous_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_continuous_direct");

    let year_fractions = [("3m", 0.25), ("6m", 0.5), ("1y", 1.0), ("5y", 5.0)];

    for (name, yf) in year_fractions {
        bench_iter(&mut group, format!("to_continuous_{}", name), || {
            let result = Compounding::Simple.convert_rate(
                black_box(0.05),
                black_box(yf),
                &Compounding::Continuous,
            );
            black_box(result);
        });

        bench_iter(&mut group, format!("from_continuous_{}", name), || {
            let result = Compounding::Continuous.convert_rate(
                black_box(0.05),
                black_box(yf),
                &Compounding::Simple,
            );
            black_box(result);
        });
    }

    group.finish();
}

fn bench_round_trip_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    bench_iter(&mut group, "periodic_continuous_periodic", || {
        let continuous = Compounding::SEMI_ANNUAL.convert_rate(
            black_box(0.05),
            black_box(1.0),
            &Compounding::Continuous,
        );
        let back = Compounding::Continuous.convert_rate(
            black_box(continuous),
            black_box(1.0),
            &Compounding::SEMI_ANNUAL,
        );
        black_box(back);
    });

    bench_iter(&mut group, "simple_periodic_simple", || {
        let periodic = Compounding::Simple.convert_rate(
            black_box(0.05),
            black_box(1.0),
            &Compounding::SEMI_ANNUAL,
        );
        let back = Compounding::SEMI_ANNUAL.convert_rate(
            black_box(periodic),
            black_box(1.0),
            &Compounding::Simple,
        );
        black_box(back);
    });

    bench_iter(&mut group, "simple_continuous_simple", || {
        let continuous = Compounding::Simple.convert_rate(
            black_box(0.05),
            black_box(1.0),
            &Compounding::Continuous,
        );
        let back = Compounding::Continuous.convert_rate(
            black_box(continuous),
            black_box(1.0),
            &Compounding::Simple,
        );
        black_box(back);
    });

    group.finish();
}

fn bench_batch_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_conversions");

    {
        let size = 100;
        group.bench_with_input(
            BenchmarkId::new("periodic_to_continuous", size),
            &size,
            |b, &size| {
                let rates: Vec<f64> = (0..size).map(|i| 0.01 + (i as f64) * 0.0001).collect();

                b.iter(|| {
                    let results: Vec<_> = rates
                        .iter()
                        .map(|&rate| {
                            Compounding::SEMI_ANNUAL.convert_rate(
                                rate,
                                1.0,
                                &Compounding::Continuous,
                            )
                        })
                        .collect();
                    black_box(results);
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("simple_to_periodic", size),
            &size,
            |b, &size| {
                let rates: Vec<f64> = (0..size).map(|i| 0.01 + (i as f64) * 0.0001).collect();

                b.iter(|| {
                    let results: Vec<_> = rates
                        .iter()
                        .map(|&rate| {
                            Compounding::Simple.convert_rate(rate, 1.0, &Compounding::SEMI_ANNUAL)
                        })
                        .collect();
                    black_box(results);
                })
            },
        );
    }

    group.finish();
}

fn bench_market_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_scenarios");

    // US Treasury: semi-annual to continuous (zero curve construction)
    bench_iter(&mut group, "treasury_to_continuous", || {
        let continuous = Compounding::SEMI_ANNUAL.convert_rate(
            black_box(0.025),
            black_box(1.0),
            &Compounding::Continuous,
        );
        black_box(continuous);
    });

    // LIBOR: simple to periodic (swap pricing)
    bench_iter(&mut group, "libor_to_swap_rate", || {
        let swap = Compounding::Simple.convert_rate(
            black_box(0.035),
            black_box(0.25),
            &Compounding::SEMI_ANNUAL,
        );
        black_box(swap);
    });

    // Corporate bond: annual to continuous (option pricing)
    bench_iter(&mut group, "corporate_to_continuous", || {
        let continuous = Compounding::Annual.convert_rate(
            black_box(0.05),
            black_box(1.0),
            &Compounding::Continuous,
        );
        black_box(continuous);
    });

    // Derivatives: continuous to quarterly (futures pricing)
    bench_iter(&mut group, "continuous_to_futures", || {
        let futures = Compounding::Continuous.convert_rate(
            black_box(0.04),
            black_box(1.0),
            &Compounding::QUARTERLY,
        );
        black_box(futures);
    });

    group.finish();
}

fn bench_negative_rates(c: &mut Criterion) {
    let mut group = c.benchmark_group("negative_rates");

    // Modern markets sometimes have negative rates
    bench_iter(&mut group, "negative_periodic_to_continuous", || {
        let continuous = Compounding::SEMI_ANNUAL.convert_rate(
            black_box(-0.005),
            black_box(1.0),
            &Compounding::Continuous,
        );
        black_box(continuous);
    });

    bench_iter(&mut group, "negative_round_trip", || {
        let continuous = Compounding::SEMI_ANNUAL.convert_rate(
            black_box(-0.005),
            black_box(1.0),
            &Compounding::Continuous,
        );
        let back = Compounding::Continuous.convert_rate(
            black_box(continuous),
            black_box(1.0),
            &Compounding::SEMI_ANNUAL,
        );
        black_box(back);
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_to_periodic,
    bench_periodic_to_continuous,
    bench_continuous_to_periodic,
    bench_simple_continuous_direct,
    bench_round_trip_conversions,
    bench_batch_conversions,
    bench_market_scenarios,
    bench_negative_rates,
);
criterion_main!(benches);
