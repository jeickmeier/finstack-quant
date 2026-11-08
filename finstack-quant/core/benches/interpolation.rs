//! Checked interpolation queries; construction is excluded from timing.
//!
//! Batches include collecting and dropping a fresh result vector. The flat
//! continuously compounded 4% pillars permit independent scalar references.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::interp::{
    CubicHermiteStrategy, ExtrapolationPolicy, InterpolationStrategy, Interpolator, LinearStrategy,
    LogLinearStrategy, MonotoneConvexStrategy, PiecewiseQuadraticForwardStrategy, ValidationPolicy,
};
use std::hint::black_box;

#[derive(Clone, Copy)]
enum Reference {
    Linear,
    FlatForward,
    Hermite,
}

fn create_test_curve(num_points: usize) -> (Box<[f64]>, Box<[f64]>) {
    let knots: Vec<f64> = (0..num_points).map(|i| (i as f64) * 0.5).collect();
    let dfs: Vec<f64> = knots.iter().map(|&t| (-0.04 * t).exp()).collect();
    (knots.into_boxed_slice(), dfs.into_boxed_slice())
}

fn create_interp<S: InterpolationStrategy>(num_points: usize) -> Interpolator<S> {
    let (knots, dfs) = create_test_curve(num_points);
    Interpolator::new(
        knots,
        dfs,
        ExtrapolationPolicy::FlatZero,
        ValidationPolicy::Strict,
    )
    .expect("valid positive, increasing interpolation fixture")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(actual.is_finite(), "non-finite interpolation result");
    assert!(
        (actual - expected).abs() <= 2e-13 * expected.abs().max(1.0),
        "interpolation result {actual} differs from scalar reference {expected}"
    );
}

// Closed-form reference for this uniform exponential fixture, independent of
// the production segment search and strategy objects. Hermite slopes are the
// uniform-grid monotone harmonic means, with one-sided endpoint estimates.
fn expected_df(time: f64, num_points: usize, reference: Reference) -> f64 {
    let time = time.clamp(0.0, (num_points - 1) as f64 * 0.5);
    if matches!(reference, Reference::FlatForward) {
        return (-0.04 * time).exp();
    }
    let segment = ((time / 0.5).floor() as usize).min(num_points - 2);
    let value = |index: usize| (-0.02 * index as f64).exp();
    let fraction = (time - segment as f64 * 0.5) / 0.5;
    let left = value(segment);
    let right = value(segment + 1);
    if matches!(reference, Reference::Linear) {
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

fn check_queries<S: InterpolationStrategy>(
    interp: &Interpolator<S>,
    times: &[f64],
    num_points: usize,
    reference: Reference,
) {
    assert!(!times.is_empty());
    for &time in times {
        assert_close(
            interp.interp(time),
            expected_df(time, num_points, reference),
        );
    }
}

fn bench_strategy<S: InterpolationStrategy>(c: &mut Criterion, name: &str, reference: Reference) {
    c.bench_function(&format!("interp_{name}_single"), |b| {
        let interp = create_interp::<S>(20);
        check_queries(&interp, &[2.5], 20, reference);
        b.iter(|| black_box(black_box(&interp).interp(black_box(2.5))));
    });

    let mut group = c.benchmark_group(format!("interp_{name}_batch"));
    let size = 100;
    group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
        let interp = create_interp::<S>(20);
        let times: Vec<f64> = (0..size).map(|i| i as f64 * 0.1).collect();
        check_queries(&interp, &times, 20, reference);
        b.iter(|| {
            let values: Vec<_> = black_box(&times)
                .iter()
                .map(|&t| interp.interp(t))
                .collect();
            black_box(values)
        });
    });
    group.finish();
}

fn bench_scalar_and_batch(c: &mut Criterion) {
    bench_strategy::<LinearStrategy>(c, "linear", Reference::Linear);
    bench_strategy::<LogLinearStrategy>(c, "log_linear", Reference::FlatForward);
    bench_strategy::<CubicHermiteStrategy>(c, "cubic_hermite", Reference::Hermite);
    bench_strategy::<MonotoneConvexStrategy>(c, "monotone_convex", Reference::FlatForward);
}

fn bench_comparison<S: InterpolationStrategy>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    reference: Reference,
) {
    group.bench_function(name, |b| {
        let interp = create_interp::<S>(50);
        let times: Vec<f64> = (0..100).map(|i| i as f64 * 0.05).collect();
        check_queries(&interp, &times, 50, reference);
        b.iter(|| {
            let values: Vec<_> = black_box(&times)
                .iter()
                .map(|&t| interp.interp(t))
                .collect();
            black_box(values)
        });
    });
}

fn bench_interp_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("interp_comparison");
    bench_comparison::<LinearStrategy>(&mut group, "linear", Reference::Linear);
    bench_comparison::<LogLinearStrategy>(&mut group, "log_linear", Reference::FlatForward);
    bench_comparison::<CubicHermiteStrategy>(&mut group, "CubicHermite", Reference::Hermite);
    bench_comparison::<MonotoneConvexStrategy>(
        &mut group,
        "MonotoneConvex",
        Reference::FlatForward,
    );
    bench_comparison::<PiecewiseQuadraticForwardStrategy>(
        &mut group,
        "PiecewiseQuadraticForward",
        Reference::FlatForward,
    );
    group.finish();
}

fn bench_interp_extrapolation(c: &mut Criterion) {
    for (name, policy, time) in [
        (
            "interp_extrap_flat_zero_left",
            ExtrapolationPolicy::FlatZero,
            -1.0,
        ),
        (
            "interp_extrap_flat_zero_right",
            ExtrapolationPolicy::FlatZero,
            100.0,
        ),
        (
            "interp_extrap_flat_fwd_left",
            ExtrapolationPolicy::FlatForward,
            -1.0,
        ),
        (
            "interp_extrap_flat_fwd_right",
            ExtrapolationPolicy::FlatForward,
            100.0,
        ),
    ] {
        c.bench_function(name, |b| {
            let (knots, dfs) = create_test_curve(10);
            let boundary = if time < 0.0 { 0 } else { knots.len() - 1 };
            let expected = if matches!(policy, ExtrapolationPolicy::FlatZero) {
                dfs[boundary]
            } else {
                let (left, right) = if time < 0.0 { (0, 1) } else { (8, 9) };
                dfs[boundary]
                    + (dfs[right] - dfs[left]) / (knots[right] - knots[left])
                        * (time - knots[boundary])
            };
            let interp =
                Interpolator::<LinearStrategy>::new(knots, dfs, policy, ValidationPolicy::Strict)
                    .expect("valid extrapolation fixture");
            assert_close(interp.interp(time), expected);
            b.iter(|| black_box(black_box(&interp).interp(black_box(time))));
        });
    }
}

criterion_group!(
    benches,
    bench_scalar_and_batch,
    bench_interp_comparison,
    bench_interp_extrapolation
);
criterion_main!(benches);
