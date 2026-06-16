//! Benchmarks for statistical and special functions.
//!
//! Tests performance of:
//! - Normal distribution CDF and PDF
//! - Inverse normal CDF (quantile function)
//! - Error function
//! - Binomial probability and distribution
//! - Beta distribution sampling
//! - Basic statistics (mean, variance, covariance)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::distributions::{
    binomial_distribution, binomial_probability, log_binomial_coefficient, log_factorial,
    sample_beta,
};
use finstack_quant_core::math::random::Pcg64Rng;
use finstack_quant_core::math::special_functions::{
    erf, norm_cdf, norm_pdf, standard_normal_inv_cdf,
};
use finstack_quant_core::math::stats::{correlation, covariance, mean, mean_var, variance};
use finstack_quant_core::math::RandomNumberGenerator;
use std::hint::black_box;

fn bench_normal_cdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_norm_cdf");

    // Single evaluations at different points
    group.bench_function("single_at_0", |b| {
        b.iter(|| {
            let result = norm_cdf(black_box(0.0));
            black_box(result);
        })
    });

    group.bench_function("single_at_1", |b| {
        b.iter(|| {
            let result = norm_cdf(black_box(1.0));
            black_box(result);
        })
    });

    group.bench_function("single_at_2", |b| {
        b.iter(|| {
            let result = norm_cdf(black_box(2.0));
            black_box(result);
        })
    });

    // Extreme tail (important for risk metrics)
    group.bench_function("single_at_4_tail", |b| {
        b.iter(|| {
            let result = norm_cdf(black_box(4.0));
            black_box(result);
        })
    });

    // Batch evaluations
    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            let points: Vec<f64> = (0..size)
                .map(|i| -4.0 + 8.0 * (i as f64) / (size as f64))
                .collect();
            b.iter(|| {
                let results: Vec<f64> = points.iter().map(|&x| norm_cdf(x)).collect();
                black_box(results);
            })
        });
    }

    group.finish();
}

fn bench_normal_pdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_norm_pdf");

    group.bench_function("single_at_0", |b| {
        b.iter(|| {
            let result = norm_pdf(black_box(0.0));
            black_box(result);
        })
    });

    group.bench_function("single_at_1", |b| {
        b.iter(|| {
            let result = norm_pdf(black_box(1.0));
            black_box(result);
        })
    });

    // Batch evaluations
    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            let points: Vec<f64> = (0..size)
                .map(|i| -4.0 + 8.0 * (i as f64) / (size as f64))
                .collect();
            b.iter(|| {
                let results: Vec<f64> = points.iter().map(|&x| norm_pdf(x)).collect();
                black_box(results);
            })
        });
    }

    group.finish();
}

fn bench_normal_inv_cdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_norm_inv_cdf");

    // Single evaluations at different quantiles
    group.bench_function("single_at_0.5", |b| {
        b.iter(|| {
            let result = standard_normal_inv_cdf(black_box(0.5));
            black_box(result);
        })
    });

    group.bench_function("single_at_0.95", |b| {
        b.iter(|| {
            let result = standard_normal_inv_cdf(black_box(0.95));
            black_box(result);
        })
    });

    group.bench_function("single_at_0.99", |b| {
        b.iter(|| {
            let result = standard_normal_inv_cdf(black_box(0.99));
            black_box(result);
        })
    });

    // Extreme tail (for VaR calculations)
    group.bench_function("single_at_0.999", |b| {
        b.iter(|| {
            let result = standard_normal_inv_cdf(black_box(0.999));
            black_box(result);
        })
    });

    // Batch evaluations (common in Monte Carlo)
    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            let probs: Vec<f64> = (1..=size)
                .map(|i| (i as f64) / (size as f64 + 1.0))
                .collect();
            b.iter(|| {
                let results: Vec<f64> = probs.iter().map(|&p| standard_normal_inv_cdf(p)).collect();
                black_box(results);
            })
        });
    }

    group.finish();
}

fn bench_error_function(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_erf");

    group.bench_function("single_at_0", |b| {
        b.iter(|| {
            let result = erf(black_box(0.0));
            black_box(result);
        })
    });

    group.bench_function("single_at_1", |b| {
        b.iter(|| {
            let result = erf(black_box(1.0));
            black_box(result);
        })
    });

    // Batch evaluations
    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            let points: Vec<f64> = (0..size)
                .map(|i| -3.0 + 6.0 * (i as f64) / (size as f64))
                .collect();
            b.iter(|| {
                let results: Vec<f64> = points.iter().map(|&x| erf(x)).collect();
                black_box(results);
            })
        });
    }

    group.finish();
}

fn bench_binomial_probability(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_binomial_prob");

    // Single probability calculations
    group.bench_function("single_n10_k5_p0.5", |b| {
        b.iter(|| {
            let result = binomial_probability(black_box(10), black_box(5), black_box(0.5));
            black_box(result);
        })
    });

    group.bench_function("single_n100_k50_p0.5", |b| {
        b.iter(|| {
            let result = binomial_probability(black_box(100), black_box(50), black_box(0.5));
            black_box(result);
        })
    });

    // Credit portfolio: 100 names, 5% PD
    group.bench_function("credit_n100_k5_p0.05", |b| {
        b.iter(|| {
            let result = binomial_probability(black_box(100), black_box(5), black_box(0.05));
            black_box(result);
        })
    });

    // Large n (stress test)
    group.bench_function("single_n1000_k500_p0.5", |b| {
        b.iter(|| {
            let result = binomial_probability(black_box(1000), black_box(500), black_box(0.5));
            black_box(result);
        })
    });

    group.finish();
}

fn bench_binomial_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_binomial_dist");

    // Full distribution generation
    for n in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::new("full_dist", n), &n, |b, &n| {
            b.iter(|| {
                let dist = binomial_distribution(black_box(n), black_box(0.5)).unwrap();
                black_box(dist);
            })
        });
    }

    // Credit portfolio distribution
    group.bench_function("credit_portfolio_100", |b| {
        b.iter(|| {
            let dist = binomial_distribution(black_box(100), black_box(0.05)).unwrap();
            black_box(dist);
        })
    });

    group.finish();
}

fn bench_log_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_log_functions");

    // Log factorial
    for n in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::new("log_factorial", n), &n, |b, &n| {
            b.iter(|| {
                let result = log_factorial(black_box(n));
                black_box(result);
            })
        });
    }

    // Log binomial coefficient
    for (n, k) in [(10, 5), (50, 25), (100, 50), (500, 250)] {
        let label = format!("{}_{}", n, k);
        group.bench_function(BenchmarkId::new("log_binomial_coef", label), |b| {
            b.iter(|| {
                let result = log_binomial_coefficient(black_box(n), black_box(k));
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_beta_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_beta_sampling");

    // Single samples with different shape parameters
    group.bench_function("sample_beta_1_1", |b| {
        let mut rng = Pcg64Rng::new(42);
        b.iter(|| {
            let result = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, 1.0);
            let _ = black_box(result);
        })
    });

    group.bench_function("sample_beta_2_2", |b| {
        let mut rng = Pcg64Rng::new(42);
        b.iter(|| {
            let result = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 2.0, 2.0);
            let _ = black_box(result);
        })
    });

    group.bench_function("sample_beta_4_2", |b| {
        let mut rng = Pcg64Rng::new(42);
        b.iter(|| {
            let result = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 4.0, 2.0);
            let _ = black_box(result);
        })
    });

    // Small shape parameters (tests Ahrens-Dieter branch)
    group.bench_function("sample_beta_0.5_0.5", |b| {
        let mut rng = Pcg64Rng::new(42);
        b.iter(|| {
            let result = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 0.5, 0.5);
            let _ = black_box(result);
        })
    });

    // Batch sampling
    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            let mut rng = Pcg64Rng::new(42);
            b.iter(|| {
                let samples: Vec<f64> = (0..size)
                    .map(|_| {
                        sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 4.0, 2.0)
                            .expect("Beta(4,2) should succeed")
                    })
                    .collect();
                black_box(samples);
            })
        });
    }

    group.finish();
}

fn bench_basic_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_basic_stats");

    // Create test data
    let data_100: Vec<f64> = (0..100)
        .map(|i| i as f64 * 0.1 + 0.5 * (i as f64 / 10.0).sin())
        .collect();
    let data_1000: Vec<f64> = (0..1000)
        .map(|i| i as f64 * 0.01 + 0.5 * (i as f64 / 100.0).sin())
        .collect();

    // Mean
    group.bench_function("mean_100", |b| {
        b.iter(|| {
            let result = mean(black_box(&data_100));
            black_box(result);
        })
    });

    group.bench_function("mean_1000", |b| {
        b.iter(|| {
            let result = mean(black_box(&data_1000));
            black_box(result);
        })
    });

    // Variance
    group.bench_function("variance_100", |b| {
        b.iter(|| {
            let result = variance(black_box(&data_100));
            black_box(result);
        })
    });

    group.bench_function("variance_1000", |b| {
        b.iter(|| {
            let result = variance(black_box(&data_1000));
            black_box(result);
        })
    });

    // Mean and variance together
    group.bench_function("mean_var_100", |b| {
        b.iter(|| {
            let result = mean_var(black_box(&data_100));
            black_box(result);
        })
    });

    group.bench_function("mean_var_1000", |b| {
        b.iter(|| {
            let result = mean_var(black_box(&data_1000));
            black_box(result);
        })
    });

    group.finish();
}

fn bench_covariance_correlation(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_covariance");

    // Create correlated test data
    let x_100: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
    let y_100: Vec<f64> = x_100.iter().map(|&x| 2.0 * x + 0.5).collect();

    let x_1000: Vec<f64> = (0..1000).map(|i| i as f64 * 0.01).collect();
    let y_1000: Vec<f64> = x_1000.iter().map(|&x| 2.0 * x + 0.5).collect();

    // Covariance
    group.bench_function("covariance_100", |b| {
        b.iter(|| {
            let result = covariance(black_box(&x_100), black_box(&y_100));
            black_box(result);
        })
    });

    group.bench_function("covariance_1000", |b| {
        b.iter(|| {
            let result = covariance(black_box(&x_1000), black_box(&y_1000));
            black_box(result);
        })
    });

    // Correlation
    group.bench_function("correlation_100", |b| {
        b.iter(|| {
            let result = correlation(black_box(&x_100), black_box(&y_100));
            black_box(result);
        })
    });

    group.bench_function("correlation_1000", |b| {
        b.iter(|| {
            let result = correlation(black_box(&x_1000), black_box(&y_1000));
            black_box(result);
        })
    });

    group.finish();
}

fn bench_cdf_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_cdf_roundtrip");

    // CDF -> Inv CDF roundtrip (common in copula models)
    group.bench_function("roundtrip_single", |b| {
        b.iter(|| {
            let x = black_box(1.5);
            let p = norm_cdf(x);
            let x_back = standard_normal_inv_cdf(p);
            black_box(x_back);
        })
    });

    // Batch roundtrip
    for size in [10, 100] {
        group.bench_with_input(
            BenchmarkId::new("roundtrip_batch", size),
            &size,
            |b, &size| {
                let x_values: Vec<f64> = (0..size)
                    .map(|i| -3.0 + 6.0 * (i as f64) / (size as f64))
                    .collect();
                b.iter(|| {
                    let results: Vec<f64> = x_values
                        .iter()
                        .map(|&x| {
                            let p = norm_cdf(x);
                            standard_normal_inv_cdf(p)
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
    bench_normal_cdf,
    bench_normal_pdf,
    bench_normal_inv_cdf,
    bench_error_function,
    bench_binomial_probability,
    bench_binomial_distribution,
    bench_log_functions,
    bench_beta_sampling,
    bench_basic_statistics,
    bench_covariance_correlation,
    bench_cdf_roundtrip,
);
criterion_main!(benches);
