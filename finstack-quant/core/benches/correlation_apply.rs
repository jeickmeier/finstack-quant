//! Benchmarks for correlated-shock transforms.
//!
//! `CorrelationFactor::apply` runs per timestep per path in multi-asset Monte
//! Carlo, so its cost scales with n (asset count) × paths × steps. Measures the
//! exact-triangular hot path and the pivoted dense path separately.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::linalg::cholesky_correlation;
use finstack_quant_core::math::linalg::CorrelationFactor;
use std::hint::black_box;

/// Equicorrelated matrix with off-diagonal `rho` — positive definite for the
/// values used here. Complete pivoting fills the unpermuted upper triangle.
fn equicorrelation(n: usize, rho: f64) -> Vec<f64> {
    (0..n * n)
        .map(|k| {
            let (i, j) = (k / n, k % n);
            if i == j {
                1.0
            } else {
                rho
            }
        })
        .collect()
}

/// Strictly lower-triangular factor with decreasing diagonals.
fn lower_triangular_factor(n: usize) -> Vec<f64> {
    let mut factor = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            factor[i * n + j] = if i == j {
                (n - i) as f64
            } else {
                0.1 * ((i - j) as f64)
            };
        }
    }
    factor
}

fn bench_correlation_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("correlation_factor_apply");
    for &n in &[2usize, 8, 32, 64] {
        let z: Vec<f64> = (0..n).map(|i| ((i % 13) as f64 * 0.07) - 0.4).collect();

        let triangular = CorrelationFactor::from_parts(lower_triangular_factor(n), n, n);
        group.bench_with_input(BenchmarkId::new("triangular", n), &n, |b, _| {
            let mut out = vec![0.0; n];
            b.iter(|| {
                triangular
                    .apply(black_box(&z), &mut out)
                    .expect("matching dimensions");
                black_box(out[n - 1])
            });
        });

        let pivoted =
            cholesky_correlation(&equicorrelation(n, 0.5), n).expect("valid correlation matrix");
        group.bench_with_input(BenchmarkId::new("pivoted_dense", n), &n, |b, _| {
            let mut out = vec![0.0; n];
            b.iter(|| {
                pivoted
                    .apply(black_box(&z), &mut out)
                    .expect("matching dimensions");
                black_box(out[n - 1])
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_correlation_apply);
criterion_main!(benches);
