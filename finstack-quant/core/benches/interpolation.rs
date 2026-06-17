//! Benchmarks for interpolation algorithms.
//!
//! Tests performance of:
//! - Linear interpolation
//! - Log-linear interpolation
//! - Cubic Hermite interpolation
//! - Monotone convex interpolation
//! - Batch interpolation operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::interp::{
    CubicHermiteStrategy, ExtrapolationPolicy, InterpFn, Interpolator, LinearStrategy,
    LogLinearStrategy, MonotoneConvexStrategy, PiecewiseQuadraticForwardStrategy, ValidationPolicy,
};
use std::hint::black_box;

fn create_test_curve(num_points: usize) -> (Box<[f64]>, Box<[f64]>) {
    let knots: Vec<f64> = (0..num_points)
        .map(|i| (i as f64) * 0.5) // 0.0, 0.5, 1.0, 1.5, ...
        .collect();
    let dfs: Vec<f64> = knots
        .iter()
        .map(|&t| (-0.04 * t).exp()) // Discount curve at 4%
        .collect();
    (knots.into_boxed_slice(), dfs.into_boxed_slice())
}

fn bench_linear_interp(c: &mut Criterion) {
    let (knots, dfs) = create_test_curve(20);
    let interp = Interpolator::<LinearStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();

    c.bench_function("interp_linear_single", |b| {
        b.iter(|| {
            let value = black_box(&interp).interp(black_box(2.5));
            black_box(value);
        })
    });

    let mut group = c.benchmark_group("interp_linear_batch");
    {
        let size = 100;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.1).collect();
            b.iter(|| {
                let values: Vec<_> = times.iter().map(|&t| interp.interp(t)).collect();
                black_box(values);
            })
        });
    }
    group.finish();
}

fn bench_log_linear_interp(c: &mut Criterion) {
    let (knots, dfs) = create_test_curve(20);
    let interp = Interpolator::<LogLinearStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();

    c.bench_function("interp_log_linear_single", |b| {
        b.iter(|| {
            let value = black_box(&interp).interp(black_box(2.5));
            black_box(value);
        })
    });

    let mut group = c.benchmark_group("interp_log_linear_batch");
    {
        let size = 100;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.1).collect();
            b.iter(|| {
                let values: Vec<_> = times.iter().map(|&t| interp.interp(t)).collect();
                black_box(values);
            })
        });
    }
    group.finish();
}

fn bench_cubic_hermite_interp(c: &mut Criterion) {
    let (knots, dfs) = create_test_curve(20);
    let interp = Interpolator::<CubicHermiteStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();

    c.bench_function("interp_cubic_hermite_single", |b| {
        b.iter(|| {
            let value = black_box(&interp).interp(black_box(2.5));
            black_box(value);
        })
    });

    let mut group = c.benchmark_group("interp_cubic_hermite_batch");
    {
        let size = 100;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.1).collect();
            b.iter(|| {
                let values: Vec<_> = times.iter().map(|&t| interp.interp(t)).collect();
                black_box(values);
            })
        });
    }
    group.finish();
}

fn bench_monotone_convex_interp(c: &mut Criterion) {
    let (knots, dfs) = create_test_curve(20);
    let interp = Interpolator::<MonotoneConvexStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();

    c.bench_function("interp_monotone_convex_single", |b| {
        b.iter(|| {
            let value = black_box(&interp).interp(black_box(2.5));
            black_box(value);
        })
    });

    let mut group = c.benchmark_group("interp_monotone_convex_batch");
    {
        let size = 100;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.1).collect();
            b.iter(|| {
                let values: Vec<_> = times.iter().map(|&t| interp.interp(t)).collect();
                black_box(values);
            })
        });
    }
    group.finish();
}

fn bench_interp_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("interp_comparison");
    let (knots, dfs) = create_test_curve(50);

    let linear = Interpolator::<LinearStrategy>::new(
        knots.clone(),
        dfs.clone(),
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();
    let log_linear = Interpolator::<LogLinearStrategy>::new(
        knots.clone(),
        dfs.clone(),
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();
    let cubic_hermite = Interpolator::<CubicHermiteStrategy>::new(
        knots.clone(),
        dfs.clone(),
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();
    let monotone_convex = Interpolator::<MonotoneConvexStrategy>::new(
        knots.clone(),
        dfs.clone(),
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();
    let piecewise_quadratic_forward = Interpolator::<PiecewiseQuadraticForwardStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();

    let test_times: Vec<f64> = (0..100).map(|i| (i as f64) * 0.05).collect();

    group.bench_function("Linear", |b| {
        b.iter(|| {
            let values: Vec<_> = test_times.iter().map(|&t| linear.interp(t)).collect();
            black_box(values);
        })
    });

    group.bench_function("LogLinear", |b| {
        b.iter(|| {
            let values: Vec<_> = test_times.iter().map(|&t| log_linear.interp(t)).collect();
            black_box(values);
        })
    });

    group.bench_function("CubicHermite", |b| {
        b.iter(|| {
            let values: Vec<_> = test_times
                .iter()
                .map(|&t| cubic_hermite.interp(t))
                .collect();
            black_box(values);
        })
    });

    group.bench_function("MonotoneConvex", |b| {
        b.iter(|| {
            let values: Vec<_> = test_times
                .iter()
                .map(|&t| monotone_convex.interp(t))
                .collect();
            black_box(values);
        })
    });

    group.bench_function("PiecewiseQuadraticForward", |b| {
        b.iter(|| {
            let values: Vec<_> = test_times
                .iter()
                .map(|&t| piecewise_quadratic_forward.interp(t))
                .collect();
            black_box(values);
        })
    });

    group.finish();
}

fn bench_interp_extrapolation(c: &mut Criterion) {
    let (knots, dfs) = create_test_curve(10);
    let interp_flat_zero = Interpolator::<LinearStrategy>::new(
        knots.clone(),
        dfs.clone(),
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .unwrap();
    let interp_flat_fwd = Interpolator::<LinearStrategy>::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatForward,
        ValidationPolicy::Strict,
    )
    .unwrap();

    c.bench_function("interp_extrap_flat_zero_left", |b| {
        b.iter(|| {
            let value = black_box(&interp_flat_zero).interp(black_box(-1.0));
            black_box(value);
        })
    });

    c.bench_function("interp_extrap_flat_zero_right", |b| {
        b.iter(|| {
            let value = black_box(&interp_flat_zero).interp(black_box(100.0));
            black_box(value);
        })
    });

    c.bench_function("interp_extrap_flat_fwd_left", |b| {
        b.iter(|| {
            let value = black_box(&interp_flat_fwd).interp(black_box(-1.0));
            black_box(value);
        })
    });

    c.bench_function("interp_extrap_flat_fwd_right", |b| {
        b.iter(|| {
            let value = black_box(&interp_flat_fwd).interp(black_box(100.0));
            black_box(value);
        })
    });
}

criterion_group!(
    benches,
    bench_linear_interp,
    bench_log_linear_interp,
    bench_cubic_hermite_interp,
    bench_monotone_convex_interp,
    bench_interp_comparison,
    bench_interp_extrapolation,
);
criterion_main!(benches);
