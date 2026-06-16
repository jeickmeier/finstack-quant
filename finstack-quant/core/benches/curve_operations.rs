//! Benchmarks for term structure curve operations.
//!
//! Tests performance of:
//! - Discount curve lookups (df, zero, forward)
//! - Forward curve operations
//! - Hazard curve operations
//! - Curve building and bumping

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_discount_curve(num_points: usize, style: InterpStyle) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let df = (-0.04 * t).exp(); // 4% flat curve
            (t, df)
        })
        .collect();

    DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .knots(knots)
        .interp(style)
        .build()
        .unwrap()
}

fn create_forward_curve(num_points: usize, tenor: f64) -> ForwardCurve {
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let rate = 0.03 + 0.01 * (t / 5.0); // Slightly upward sloping
            (t, rate)
        })
        .collect();

    ForwardCurve::builder("USD-SOFR-3M", tenor)
        .base_date(base_date())
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn create_hazard_curve(num_points: usize) -> HazardCurve {
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let hazard_rate = 0.02; // Constant 200bp hazard rate
            (t, hazard_rate)
        })
        .collect();

    HazardCurve::builder("CORP-AA")
        .base_date(base_date())
        .knots(knots)
        .build()
        .unwrap()
}

fn bench_discount_curve_df(c: &mut Criterion) {
    let curve = create_discount_curve(20, InterpStyle::Linear);

    c.bench_function("curve_discount_df_single", |b| {
        b.iter(|| {
            let df = black_box(&curve).df(black_box(2.5));
            black_box(df);
        })
    });

    let mut group = c.benchmark_group("curve_discount_df_batch");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.05).collect();
            b.iter(|| {
                let dfs: Vec<_> = times.iter().map(|&t| curve.df(t)).collect();
                black_box(dfs);
            })
        });
    }
    group.finish();
}

fn bench_discount_curve_zero(c: &mut Criterion) {
    let curve = create_discount_curve(20, InterpStyle::Linear);

    c.bench_function("curve_discount_zero_single", |b| {
        b.iter(|| {
            let zero = black_box(&curve).zero(black_box(2.5));
            black_box(zero);
        })
    });

    let mut group = c.benchmark_group("curve_discount_zero_batch");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.05).collect();
            b.iter(|| {
                let zeros: Vec<_> = times.iter().map(|&t| curve.zero(t)).collect();
                black_box(zeros);
            })
        });
    }
    group.finish();
}

fn bench_discount_curve_forward(c: &mut Criterion) {
    let curve = create_discount_curve(20, InterpStyle::Linear);

    c.bench_function("curve_discount_forward_single", |b| {
        b.iter(|| {
            let fwd = black_box(&curve)
                .forward(black_box(1.0), black_box(2.0))
                .unwrap();
            black_box(fwd);
        })
    });

    let mut group = c.benchmark_group("curve_discount_forward_batch");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<(f64, f64)> = (0..size)
                .map(|i| {
                    let t1 = (i as f64) * 0.1;
                    let t2 = t1 + 0.25;
                    (t1, t2)
                })
                .collect();
            b.iter(|| {
                let fwds: Vec<_> = times
                    .iter()
                    .map(|&(t1, t2)| curve.forward(t1, t2))
                    .collect();
                black_box(fwds);
            })
        });
    }
    group.finish();
}

fn bench_forward_curve(c: &mut Criterion) {
    let curve = create_forward_curve(20, 0.25); // 3M tenor

    c.bench_function("curve_forward_rate_single", |b| {
        b.iter(|| {
            let rate = black_box(&curve).rate(black_box(2.5));
            black_box(rate);
        })
    });

    let mut group = c.benchmark_group("curve_forward_rate_batch");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.05).collect();
            b.iter(|| {
                let rates: Vec<_> = times.iter().map(|&t| curve.rate(t)).collect();
                black_box(rates);
            })
        });
    }
    group.finish();
}

fn bench_hazard_curve(c: &mut Criterion) {
    let curve = create_hazard_curve(20);

    c.bench_function("curve_survival_prob_single", |b| {
        b.iter(|| {
            let prob = black_box(&curve).sp(black_box(2.5));
            black_box(prob);
        })
    });

    c.bench_function("curve_default_prob_single", |b| {
        b.iter(|| {
            let prob = black_box(&curve)
                .default_prob(black_box(1.0), black_box(2.5))
                .unwrap();
            black_box(prob);
        })
    });

    let mut group = c.benchmark_group("curve_hazard_survival_batch");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let times: Vec<f64> = (0..size).map(|i| (i as f64) * 0.05).collect();
            b.iter(|| {
                let probs: Vec<_> = times.iter().map(|&t| curve.sp(t)).collect();
                black_box(probs);
            })
        });
    }
    group.finish();
}

fn bench_curve_interp_styles(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_interp_styles");

    let styles = [
        ("Linear", InterpStyle::Linear),
        ("LogLinear", InterpStyle::LogLinear),
        ("CubicHermite", InterpStyle::CubicHermite),
        ("MonotoneConvex", InterpStyle::MonotoneConvex),
        (
            "PiecewiseQuadraticForward",
            InterpStyle::PiecewiseQuadraticForward,
        ),
    ];

    let test_times: Vec<f64> = (0..100).map(|i| (i as f64) * 0.05).collect();

    for (name, style) in styles {
        let curve = create_discount_curve(20, style);
        group.bench_function(name, |b| {
            b.iter(|| {
                let dfs: Vec<_> = test_times.iter().map(|&t| curve.df(t)).collect();
                black_box(dfs);
            })
        });
    }

    group.finish();
}

fn bench_curve_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_building");

    for num_points in [5, 10, 20, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("discount_curve", num_points),
            &num_points,
            |b, &num_points| {
                let knots: Vec<(f64, f64)> = (0..num_points)
                    .map(|i| {
                        let t = (i as f64) * 0.5;
                        let df = (-0.04 * t).exp();
                        (t, df)
                    })
                    .collect();

                b.iter(|| {
                    let curve = DiscountCurve::builder("USD-OIS")
                        .base_date(base_date())
                        .knots(black_box(knots.clone()))
                        .interp(InterpStyle::Linear)
                        .build()
                        .unwrap();
                    black_box(curve);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_discount_curve_df,
    bench_discount_curve_zero,
    bench_discount_curve_forward,
    bench_forward_curve,
    bench_hazard_curve,
    bench_curve_interp_styles,
    bench_curve_building,
);
criterion_main!(benches);
