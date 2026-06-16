//! Benchmarks for numerical integration algorithms.
//!
//! Tests performance of:
//! - Simpson's rule (fixed intervals)
//! - Adaptive Simpson's rule
//! - Gauss-Legendre quadrature
//! - Gauss-Hermite quadrature (normal distribution integrals)
//! - Trapezoidal rule

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::integration::{
    adaptive_simpson, gauss_legendre_integrate, gauss_legendre_integrate_adaptive,
    gauss_legendre_integrate_composite, simpson_rule, trapezoidal_rule, GaussHermiteQuadrature,
};
use std::hint::black_box;

// Test functions for integration benchmarks

/// Simple polynomial: x^2
fn polynomial(x: f64) -> f64 {
    x * x
}

/// Oscillatory function: sin(10x)
fn oscillatory(x: f64) -> f64 {
    (10.0 * x).sin()
}

/// Smooth exponential: e^(-x^2)
fn gaussian(x: f64) -> f64 {
    (-x * x).exp()
}

/// Function with mild singularity: 1/sqrt(x) near 0
fn mild_singular(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        1.0 / x.sqrt()
    }
}

/// Black-Scholes integrand approximation (for option pricing)
fn black_scholes_like(x: f64) -> f64 {
    let d1 = (x + 0.5 * 0.04) / 0.2; // simplified BS d1
    (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn bench_simpson_rule(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_simpson");

    // Simple polynomial
    group.bench_function("polynomial_100_intervals", |b| {
        b.iter(|| {
            let result = simpson_rule(black_box(polynomial), black_box(0.0), black_box(1.0), 100)
                .expect("Simpson rule should succeed");
            black_box(result);
        })
    });

    // Oscillatory function
    group.bench_function("oscillatory_100_intervals", |b| {
        b.iter(|| {
            let result = simpson_rule(black_box(oscillatory), black_box(0.0), black_box(1.0), 100)
                .expect("Simpson rule should succeed");
            black_box(result);
        })
    });

    // Compare different interval counts
    for intervals in [10, 50, 100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("gaussian", intervals),
            &intervals,
            |b, &n| {
                b.iter(|| {
                    let result =
                        simpson_rule(black_box(gaussian), black_box(-3.0), black_box(3.0), n)
                            .expect("Simpson rule should succeed");
                    black_box(result);
                })
            },
        );
    }

    group.finish();
}

fn bench_adaptive_simpson(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_adaptive_simpson");

    // Different tolerance levels
    for tol in [1e-4, 1e-6, 1e-8, 1e-10] {
        let tol_str = format!("{:.0e}", tol);

        group.bench_function(BenchmarkId::new("polynomial", &tol_str), |b| {
            b.iter(|| {
                let result = adaptive_simpson(
                    black_box(polynomial),
                    black_box(0.0),
                    black_box(1.0),
                    black_box(tol),
                    100,
                )
                .expect("Adaptive Simpson should succeed");
                black_box(result);
            })
        });

        group.bench_function(BenchmarkId::new("oscillatory", &tol_str), |b| {
            b.iter(|| {
                let result = adaptive_simpson(
                    black_box(oscillatory),
                    black_box(0.0),
                    black_box(std::f64::consts::PI),
                    black_box(tol),
                    100,
                )
                .expect("Adaptive Simpson should succeed");
                black_box(result);
            })
        });

        group.bench_function(BenchmarkId::new("gaussian", &tol_str), |b| {
            b.iter(|| {
                let result = adaptive_simpson(
                    black_box(gaussian),
                    black_box(-5.0),
                    black_box(5.0),
                    black_box(tol),
                    100,
                )
                .expect("Adaptive Simpson should succeed");
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_trapezoidal_rule(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_trapezoidal");

    for intervals in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("polynomial", intervals),
            &intervals,
            |b, &n| {
                b.iter(|| {
                    let result =
                        trapezoidal_rule(black_box(polynomial), black_box(0.0), black_box(1.0), n)
                            .expect("Trapezoidal rule should succeed");
                    black_box(result);
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("gaussian", intervals),
            &intervals,
            |b, &n| {
                b.iter(|| {
                    let result =
                        trapezoidal_rule(black_box(gaussian), black_box(-3.0), black_box(3.0), n)
                            .expect("Trapezoidal rule should succeed");
                    black_box(result);
                })
            },
        );
    }

    group.finish();
}

fn bench_gauss_legendre(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_gauss_legendre");

    // Single interval with different orders
    for order in [2, 4, 8, 16] {
        group.bench_function(BenchmarkId::new("polynomial_single", order), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate(
                    black_box(polynomial),
                    black_box(0.0),
                    black_box(1.0),
                    black_box(order),
                )
                .expect("GL integration should succeed");
                black_box(result);
            })
        });

        group.bench_function(BenchmarkId::new("gaussian_single", order), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate(
                    black_box(gaussian),
                    black_box(-3.0),
                    black_box(3.0),
                    black_box(order),
                )
                .expect("GL integration should succeed");
                black_box(result);
            })
        });
    }

    // Composite GL with multiple panels
    for panels in [4, 10, 20, 50] {
        group.bench_function(BenchmarkId::new("composite_order8", panels), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate_composite(
                    black_box(gaussian),
                    black_box(-5.0),
                    black_box(5.0),
                    8,
                    black_box(panels),
                )
                .expect("Composite GL should succeed");
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_gauss_legendre_adaptive(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_gauss_legendre_adaptive");

    for tol in [1e-4, 1e-6, 1e-8] {
        let tol_str = format!("{:.0e}", tol);

        group.bench_function(BenchmarkId::new("polynomial", &tol_str), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate_adaptive(
                    black_box(polynomial),
                    black_box(0.0),
                    black_box(1.0),
                    8,
                    black_box(tol),
                    20,
                )
                .expect("Adaptive GL should succeed");
                black_box(result);
            })
        });

        group.bench_function(BenchmarkId::new("oscillatory", &tol_str), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate_adaptive(
                    black_box(oscillatory),
                    black_box(0.0),
                    black_box(std::f64::consts::PI),
                    8,
                    black_box(tol),
                    20,
                )
                .expect("Adaptive GL should succeed");
                black_box(result);
            })
        });

        group.bench_function(BenchmarkId::new("mild_singular", &tol_str), |b| {
            b.iter(|| {
                let result = gauss_legendre_integrate_adaptive(
                    black_box(mild_singular),
                    black_box(0.01), // Avoid singularity at 0
                    black_box(1.0),
                    8,
                    black_box(tol),
                    20,
                )
                .expect("Adaptive GL should succeed");
                black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_gauss_hermite(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_gauss_hermite");

    let quad5 = GaussHermiteQuadrature::new(5).expect("5 is valid order");
    let quad7 = GaussHermiteQuadrature::new(7).expect("7 is valid order");
    let quad10 = GaussHermiteQuadrature::new(10).expect("10 is valid order");

    // Polynomial integral over normal distribution
    let x_squared = |x: f64| x * x;
    let x_fourth = |x: f64| x.powi(4);

    group.bench_function("x_squared_order5", |b| {
        b.iter(|| {
            let result = black_box(&quad5).integrate(black_box(x_squared));
            black_box(result);
        })
    });

    group.bench_function("x_squared_order7", |b| {
        b.iter(|| {
            let result = black_box(&quad7).integrate(black_box(x_squared));
            black_box(result);
        })
    });

    group.bench_function("x_squared_order10", |b| {
        b.iter(|| {
            let result = black_box(&quad10).integrate(black_box(x_squared));
            black_box(result);
        })
    });

    group.bench_function("x_fourth_order10", |b| {
        b.iter(|| {
            let result = black_box(&quad10).integrate(black_box(x_fourth));
            black_box(result);
        })
    });

    // More complex integrand (option pricing-like)
    group.bench_function("bs_like_order10", |b| {
        b.iter(|| {
            let result = black_box(&quad10).integrate(black_box(black_scholes_like));
            black_box(result);
        })
    });

    // Adaptive integration
    group.bench_function("adaptive_tol_1e-6", |b| {
        b.iter(|| {
            let result = black_box(&quad5).integrate_adaptive(black_box(x_fourth), 1e-6);
            black_box(result);
        })
    });

    group.finish();
}

fn bench_integration_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_comparison");

    // Compare methods on same problem: ∫₀¹ x² dx = 1/3
    group.bench_function("simpson_100", |b| {
        b.iter(|| {
            let result = simpson_rule(polynomial, 0.0, 1.0, 100).expect("Should succeed");
            black_box(result);
        })
    });

    group.bench_function("trapezoidal_100", |b| {
        b.iter(|| {
            let result = trapezoidal_rule(polynomial, 0.0, 1.0, 100).expect("Should succeed");
            black_box(result);
        })
    });

    group.bench_function("gauss_legendre_order8", |b| {
        b.iter(|| {
            let result = gauss_legendre_integrate(polynomial, 0.0, 1.0, 8).expect("Should succeed");
            black_box(result);
        })
    });

    group.bench_function("gauss_legendre_composite_4panels", |b| {
        b.iter(|| {
            let result = gauss_legendre_integrate_composite(polynomial, 0.0, 1.0, 8, 4)
                .expect("Should succeed");
            black_box(result);
        })
    });

    group.bench_function("adaptive_simpson_1e-8", |b| {
        b.iter(|| {
            let result = adaptive_simpson(polynomial, 0.0, 1.0, 1e-8, 50).expect("Should succeed");
            black_box(result);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simpson_rule,
    bench_adaptive_simpson,
    bench_trapezoidal_rule,
    bench_gauss_legendre,
    bench_gauss_legendre_adaptive,
    bench_gauss_hermite,
    bench_integration_comparison,
);
criterion_main!(benches);
