//! Checked term-structure queries and construction.
//!
//! Query fixtures and preflights are created only for the selected case.
//! Batches include result-vector allocation/drop. Construction includes the
//! supplied knot-vector clone, validation, interpolation setup and curve drop.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::{Compounding, interp::InterpStyle};
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid fixture date")
}

fn discount_knots(num_points: usize) -> Vec<(f64, f64)> {
    (0..num_points)
        .map(|i| {
            let time = i as f64 * 0.5;
            (time, (-0.04 * time).exp())
        })
        .collect()
}

fn create_discount_curve(num_points: usize, style: InterpStyle) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .knots(discount_knots(num_points))
        .interp(style)
        .build()
        .expect("valid positive discount fixture")
}

fn create_forward_curve(num_points: usize, tenor: f64) -> ForwardCurve {
    let knots = (0..num_points).map(|i| {
        let time = i as f64 * 0.5;
        (time, 0.03 + 0.01 * (time / 5.0))
    });
    ForwardCurve::builder("USD-SOFR-3M", tenor)
        .base_date(base_date())
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid forward fixture")
}

fn create_hazard_curve(num_points: usize) -> HazardCurve {
    HazardCurve::builder("CORP-AA")
        .base_date(base_date())
        .knots((0..num_points).map(|i| (i as f64 * 0.5, 0.02)))
        .recovery_rate(0.40)
        .build()
        .expect("valid constant-hazard fixture")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(actual.is_finite(), "non-finite curve result");
    assert!(
        (actual - expected).abs() <= 2e-13 * expected.abs().max(1.0),
        "curve result {actual} differs from scalar reference {expected}"
    );
}

fn check_discount_curve(curve: &DiscountCurve, num_points: usize, style: InterpStyle) {
    assert_eq!(curve.id().as_str(), "USD-OIS");
    assert_eq!(curve.base_date(), base_date());
    assert_eq!(curve.interp_style(), style);
    assert_eq!(curve.len(), num_points);
    assert_eq!(curve.knots().len(), curve.dfs().len());
    for (i, (&time, &df)) in curve.knots().iter().zip(curve.dfs()).enumerate() {
        assert_eq!(time, i as f64 * 0.5);
        assert_close(df, (-0.04 * time).exp());
        assert_close(curve.df(time), df);
    }
}

// Independent closed forms for the uniform exponential pillars. Linear extends
// its endpoint secant under the fixture's default flat-forward policy. Cubic
// Hermite uses harmonic interior and one-sided endpoint slopes on this grid.
fn expected_df(time: f64, num_points: usize, style: InterpStyle) -> f64 {
    if !matches!(style, InterpStyle::Linear | InterpStyle::CubicHermite) {
        return (-0.04 * time).exp();
    }
    let segment = ((time / 0.5).floor().max(0.0) as usize).min(num_points - 2);
    let value = |index: usize| (-0.02 * index as f64).exp();
    let fraction = (time - segment as f64 * 0.5) / 0.5;
    let left = value(segment);
    let right = value(segment + 1);
    if style == InterpStyle::Linear {
        return left + fraction * (right - left);
    }
    let slope = |index: usize| {
        let delta = |i: usize| (value(i + 1) - value(i)) / 0.5;
        if index == 0 {
            (3.0 * delta(0) - delta(1)) / 2.0
        } else if index == num_points - 1 {
            (3.0 * delta(index - 1) - delta(index - 2)) / 2.0
        } else {
            2.0 / (1.0 / delta(index - 1) + 1.0 / delta(index))
        }
    };
    let t2 = fraction * fraction;
    let t3 = t2 * fraction;
    (2.0 * t3 - 3.0 * t2 + 1.0) * left
        + (t3 - 2.0 * t2 + fraction) * 0.5 * slope(segment)
        + (-2.0 * t3 + 3.0 * t2) * right
        + (t3 - t2) * 0.5 * slope(segment + 1)
}

fn query_times(size: usize) -> Vec<f64> {
    (0..size).map(|i| i as f64 * 0.05).collect()
}

fn bench_discount_curve_df(c: &mut Criterion) {
    c.bench_function("curve_discount_df_single", |b| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        assert_close(curve.df(2.5), expected_df(2.5, 20, InterpStyle::Linear));
        b.iter(|| black_box(black_box(&curve).df(black_box(2.5))));
    });
    let mut group = c.benchmark_group("curve_discount_df_batch");
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        let times = query_times(size);
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        for &time in &times {
            assert_close(curve.df(time), expected_df(time, 20, InterpStyle::Linear));
        }
        b.iter(|| {
            let dfs: Vec<_> = black_box(&times).iter().map(|&t| curve.df(t)).collect();
            black_box(dfs)
        });
    });
    group.finish();
}

fn bench_discount_curve_zero(c: &mut Criterion) {
    c.bench_function("curve_discount_zero_single", |b| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        assert_close(curve.zero_rate(2.5, Compounding::Continuous), 0.04);
        b.iter(|| black_box(black_box(&curve).zero_rate(black_box(2.5), Compounding::Continuous)));
    });
    let mut group = c.benchmark_group("curve_discount_zero_batch");
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        let times = query_times(size);
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        for &time in &times {
            let expected = if time == 0.0 {
                0.0
            } else {
                -expected_df(time, 20, InterpStyle::Linear).ln() / time
            };
            assert_close(curve.zero_rate(time, Compounding::Continuous), expected);
        }
        b.iter(|| {
            let zeros: Vec<_> = black_box(&times)
                .iter()
                .map(|&t| curve.zero_rate(t, Compounding::Continuous))
                .collect();
            black_box(zeros)
        });
    });
    group.finish();
}

fn bench_discount_curve_forward(c: &mut Criterion) {
    c.bench_function("curve_discount_forward_single", |b| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        assert_close(
            curve.forward(1.0, 2.0).expect("valid forward interval"),
            0.04,
        );
        b.iter(|| {
            black_box(
                black_box(&curve)
                    .forward(black_box(1.0), black_box(2.0))
                    .expect("valid forward interval"),
            )
        });
    });
    let mut group = c.benchmark_group("curve_discount_forward_batch");
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let curve = create_discount_curve(20, InterpStyle::Linear);
        let times: Vec<_> = (0..size)
            .map(|i| {
                let start = i as f64 * 0.1;
                (start, start + 0.25)
            })
            .collect();
        check_discount_curve(&curve, 20, InterpStyle::Linear);
        for &(start, end) in &times {
            let expected = -(expected_df(end, 20, InterpStyle::Linear)
                / expected_df(start, 20, InterpStyle::Linear))
            .ln()
                / (end - start);
            assert_close(
                curve.forward(start, end).expect("valid forward interval"),
                expected,
            );
        }
        b.iter(|| {
            let forwards: Vec<_> = black_box(&times)
                .iter()
                .map(|&(start, end)| curve.forward(start, end).expect("valid forward interval"))
                .collect();
            black_box(forwards)
        });
    });
    group.finish();
}

fn bench_forward_curve(c: &mut Criterion) {
    c.bench_function("curve_forward_rate_single", |b| {
        let curve = create_forward_curve(20, 0.25);
        assert_eq!(curve.id().as_str(), "USD-SOFR-3M");
        assert_eq!(curve.base_date(), base_date());
        assert_eq!(curve.tenor(), 0.25);
        assert_eq!(curve.knots().len(), 20);
        assert_close(curve.rate(2.5), 0.035);
        b.iter(|| black_box(black_box(&curve).rate(black_box(2.5))));
    });
    let mut group = c.benchmark_group("curve_forward_rate_batch");
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let curve = create_forward_curve(20, 0.25);
        let times = query_times(size);
        assert_eq!(curve.knots().len(), 20);
        assert_eq!(curve.tenor(), 0.25);
        for &time in &times {
            assert_close(curve.rate(time), 0.03 + 0.002 * time);
        }
        b.iter(|| {
            let rates: Vec<_> = black_box(&times).iter().map(|&t| curve.rate(t)).collect();
            black_box(rates)
        });
    });
    group.finish();
}

fn bench_hazard_curve(c: &mut Criterion) {
    c.bench_function("curve_survival_prob_single", |b| {
        let curve = create_hazard_curve(20);
        assert_eq!(curve.id().as_str(), "CORP-AA");
        assert_eq!(curve.base_date(), base_date());
        assert_eq!(curve.recovery_rate(), 0.40);
        assert_close(curve.sp(2.5), (-0.02_f64 * 2.5).exp());
        b.iter(|| black_box(black_box(&curve).sp(black_box(2.5))));
    });
    c.bench_function("curve_default_prob_single", |b| {
        let curve = create_hazard_curve(20);
        let expected = (-0.02_f64).exp() - (-0.02_f64 * 2.5).exp();
        assert_close(
            curve
                .default_prob(1.0, 2.5)
                .expect("ordered default interval"),
            expected,
        );
        b.iter(|| {
            black_box(
                black_box(&curve)
                    .default_prob(black_box(1.0), black_box(2.5))
                    .expect("ordered default interval"),
            )
        });
    });
    let mut group = c.benchmark_group("curve_hazard_survival_batch");
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let curve = create_hazard_curve(20);
        let times = query_times(size);
        assert_eq!(curve.recovery_rate(), 0.40);
        for &time in &times {
            assert_close(curve.sp(time), (-0.02 * time).exp());
        }
        b.iter(|| {
            let probabilities: Vec<_> = black_box(&times).iter().map(|&t| curve.sp(t)).collect();
            black_box(probabilities)
        });
    });
    group.finish();
}

fn bench_curve_interp_styles(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_interp_styles");
    for (name, style) in [
        ("linear", InterpStyle::Linear),
        ("log_linear", InterpStyle::LogLinear),
        ("CubicHermite", InterpStyle::CubicHermite),
        ("MonotoneConvex", InterpStyle::MonotoneConvex),
        (
            "PiecewiseQuadraticForward",
            InterpStyle::PiecewiseQuadraticForward,
        ),
    ] {
        group.bench_function(name, |b| {
            let curve = create_discount_curve(20, style);
            let times = query_times(100);
            check_discount_curve(&curve, 20, style);
            for &time in &times {
                assert_close(curve.df(time), expected_df(time, 20, style));
            }
            b.iter(|| {
                let dfs: Vec<_> = black_box(&times).iter().map(|&t| curve.df(t)).collect();
                black_box(dfs)
            });
        });
    }
    group.finish();
}

fn bench_curve_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_building");
    let num_points = 50;
    group.bench_with_input(
        BenchmarkId::new("discount_curve", num_points),
        &num_points,
        |b, &num_points| {
            let knots = discount_knots(num_points);
            let build = || {
                DiscountCurve::builder("USD-OIS")
                    .base_date(base_date())
                    .knots(black_box(knots.clone()))
                    .interp(InterpStyle::Linear)
                    .build()
                    .expect("valid discount build")
            };
            check_discount_curve(&build(), num_points, InterpStyle::Linear);
            b.iter(|| black_box(build()));
        },
    );
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
    bench_curve_building
);
criterion_main!(benches);
