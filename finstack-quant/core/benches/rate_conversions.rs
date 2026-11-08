//! Benchmarks for interest rate compounding convention conversions.
//!
//! Tests performance of:
//! - Simple to periodic rate conversions
//! - Periodic to continuous rate conversions
//! - Round-trip conversions
//! - Batch conversion scenarios
//!
//! Each selected case checks its complete output before timing against a
//! direct continuous-equivalent reference, independently of the production
//! discount-factor round trip. Batch output allocation remains timed.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::Compounding;
use std::hint::black_box;

fn assert_rate(actual: f64, expected: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= 1e-12,
        "converted rate {actual} differs from reference {expected}"
    );
}

fn reference_rate(from: Compounding, rate: f64, years: f64, to: Compounding) -> f64 {
    assert!(rate.is_finite() && years.is_finite() && years > 0.0);
    let continuous = match from {
        Compounding::Continuous => rate,
        Compounding::Simple => {
            assert!(1.0 + rate * years > 0.0);
            (rate * years).ln_1p() / years
        }
        Compounding::Periodic(periods) => {
            let n = f64::from(periods.get());
            assert!(rate / n > -1.0);
            n * (rate / n).ln_1p()
        }
    };
    match to {
        Compounding::Continuous => continuous,
        Compounding::Simple => (continuous * years).exp_m1() / years,
        Compounding::Periodic(periods) => {
            let n = f64::from(periods.get());
            n * (continuous / n).exp_m1()
        }
    }
}

fn checked_conversion(from: Compounding, rate: f64, years: f64, to: Compounding) -> f64 {
    let actual = from.convert_rate(rate, years, &to);
    assert_rate(actual, reference_rate(from, rate, years, to));
    actual
}

fn bench_simple_to_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_to_periodic");

    let test_cases = [
        ("short_period", 0.05, 0.25), // 3 months
        ("half_year", 0.05, 0.5),     // 6 months
        ("one_year", 0.05, 1.0),      // 1 year
        ("five_year", 0.05, 5.0),     // 5 years
    ];

    for (name, rate, yf) in test_cases {
        group.bench_function(name, |b| {
            checked_conversion(Compounding::Simple, rate, yf, Compounding::SEMI_ANNUAL);
            b.iter(|| {
                let result = Compounding::Simple.convert_rate(
                    black_box(rate),
                    black_box(yf),
                    &Compounding::SEMI_ANNUAL,
                );
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_periodic_to_continuous(c: &mut Criterion) {
    let mut group = c.benchmark_group("periodic_to_continuous");

    let conventions = [
        ("annual", Compounding::Periodic(std::num::NonZeroU32::MIN)),
        ("semiannual", Compounding::SEMI_ANNUAL),
        ("quarterly", Compounding::QUARTERLY),
        ("monthly", Compounding::MONTHLY),
    ];

    for (name, conv) in conventions {
        group.bench_function(name, |b| {
            checked_conversion(conv, 0.05, 1.0, Compounding::Continuous);
            b.iter(|| {
                let result =
                    conv.convert_rate(black_box(0.05), black_box(1.0), &Compounding::Continuous);
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_continuous_to_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("continuous_to_periodic");

    let conventions = [
        ("annual", Compounding::Periodic(std::num::NonZeroU32::MIN)),
        ("semiannual", Compounding::SEMI_ANNUAL),
        ("quarterly", Compounding::QUARTERLY),
        ("monthly", Compounding::MONTHLY),
    ];

    for (name, conv) in conventions {
        group.bench_function(name, |b| {
            checked_conversion(Compounding::Continuous, 0.05, 1.0, conv);
            b.iter(|| {
                let result =
                    Compounding::Continuous.convert_rate(black_box(0.05), black_box(1.0), &conv);
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_simple_continuous_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_continuous_direct");

    let year_fractions = [("3m", 0.25), ("6m", 0.5), ("1y", 1.0), ("5y", 5.0)];

    for (name, yf) in year_fractions {
        group.bench_function(format!("to_continuous_{}", name), |b| {
            checked_conversion(Compounding::Simple, 0.05, yf, Compounding::Continuous);
            b.iter(|| {
                let result = Compounding::Simple.convert_rate(
                    black_box(0.05),
                    black_box(yf),
                    &Compounding::Continuous,
                );
                black_box(result);
            })
        });

        group.bench_function(format!("from_continuous_{}", name), |b| {
            checked_conversion(Compounding::Continuous, 0.05, yf, Compounding::Simple);
            b.iter(|| {
                let result = Compounding::Continuous.convert_rate(
                    black_box(0.05),
                    black_box(yf),
                    &Compounding::Simple,
                );
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_round_trip_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    group.bench_function("periodic_continuous_periodic", |b| {
        let continuous =
            checked_conversion(Compounding::SEMI_ANNUAL, 0.05, 1.0, Compounding::Continuous);
        assert_rate(
            checked_conversion(
                Compounding::Continuous,
                continuous,
                1.0,
                Compounding::SEMI_ANNUAL,
            ),
            0.05,
        );
        b.iter(|| {
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
        })
    });

    group.bench_function("simple_periodic_simple", |b| {
        let periodic = checked_conversion(Compounding::Simple, 0.05, 1.0, Compounding::SEMI_ANNUAL);
        assert_rate(
            checked_conversion(Compounding::SEMI_ANNUAL, periodic, 1.0, Compounding::Simple),
            0.05,
        );
        b.iter(|| {
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
        })
    });

    group.bench_function("simple_continuous_simple", |b| {
        let continuous =
            checked_conversion(Compounding::Simple, 0.05, 1.0, Compounding::Continuous);
        assert_rate(
            checked_conversion(
                Compounding::Continuous,
                continuous,
                1.0,
                Compounding::Simple,
            ),
            0.05,
        );
        b.iter(|| {
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
        })
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
                let results: Vec<_> = rates
                    .iter()
                    .map(|&rate| {
                        Compounding::SEMI_ANNUAL.convert_rate(rate, 1.0, &Compounding::Continuous)
                    })
                    .collect();
                assert_eq!(rates.len(), size);
                assert_eq!(results.len(), size);
                for (&rate, &actual) in rates.iter().zip(&results) {
                    assert_rate(
                        actual,
                        reference_rate(
                            Compounding::SEMI_ANNUAL,
                            rate,
                            1.0,
                            Compounding::Continuous,
                        ),
                    );
                }
                drop(results);

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
                let results: Vec<_> = rates
                    .iter()
                    .map(|&rate| {
                        Compounding::Simple.convert_rate(rate, 1.0, &Compounding::SEMI_ANNUAL)
                    })
                    .collect();
                assert_eq!(rates.len(), size);
                assert_eq!(results.len(), size);
                for (&rate, &actual) in rates.iter().zip(&results) {
                    assert_rate(
                        actual,
                        reference_rate(Compounding::Simple, rate, 1.0, Compounding::SEMI_ANNUAL),
                    );
                }
                drop(results);

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

    // Scalar conversion only; curve construction is outside this workload.
    group.bench_function("treasury_to_continuous", |b| {
        checked_conversion(
            Compounding::SEMI_ANNUAL,
            0.025,
            1.0,
            Compounding::Continuous,
        );
        b.iter(|| {
            let continuous = Compounding::SEMI_ANNUAL.convert_rate(
                black_box(0.025),
                black_box(1.0),
                &Compounding::Continuous,
            );
            black_box(continuous);
        })
    });

    // Scalar money-market quote conversion; no swap is priced here.
    group.bench_function("libor_to_swap_rate", |b| {
        checked_conversion(Compounding::Simple, 0.035, 0.25, Compounding::SEMI_ANNUAL);
        b.iter(|| {
            let swap = Compounding::Simple.convert_rate(
                black_box(0.035),
                black_box(0.25),
                &Compounding::SEMI_ANNUAL,
            );
            black_box(swap);
        })
    });

    // Scalar annual quote conversion; no bond or option is priced here.
    group.bench_function("corporate_to_continuous", |b| {
        checked_conversion(
            Compounding::Periodic(std::num::NonZeroU32::MIN),
            0.05,
            1.0,
            Compounding::Continuous,
        );
        b.iter(|| {
            let continuous = Compounding::Periodic(std::num::NonZeroU32::MIN).convert_rate(
                black_box(0.05),
                black_box(1.0),
                &Compounding::Continuous,
            );
            black_box(continuous);
        })
    });

    // Scalar continuous quote conversion; no future is priced here.
    group.bench_function("continuous_to_futures", |b| {
        checked_conversion(Compounding::Continuous, 0.04, 1.0, Compounding::QUARTERLY);
        b.iter(|| {
            let futures = Compounding::Continuous.convert_rate(
                black_box(0.04),
                black_box(1.0),
                &Compounding::QUARTERLY,
            );
            black_box(futures);
        })
    });

    group.finish();
}

fn bench_negative_rates(c: &mut Criterion) {
    let mut group = c.benchmark_group("negative_rates");

    // Modern markets sometimes have negative rates
    group.bench_function("negative_periodic_to_continuous", |b| {
        checked_conversion(
            Compounding::SEMI_ANNUAL,
            -0.005,
            1.0,
            Compounding::Continuous,
        );
        b.iter(|| {
            let continuous = Compounding::SEMI_ANNUAL.convert_rate(
                black_box(-0.005),
                black_box(1.0),
                &Compounding::Continuous,
            );
            black_box(continuous);
        })
    });

    group.bench_function("negative_round_trip", |b| {
        let continuous = checked_conversion(
            Compounding::SEMI_ANNUAL,
            -0.005,
            1.0,
            Compounding::Continuous,
        );
        assert_rate(
            checked_conversion(
                Compounding::Continuous,
                continuous,
                1.0,
                Compounding::SEMI_ANNUAL,
            ),
            -0.005,
        );
        b.iter(|| {
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
        })
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
