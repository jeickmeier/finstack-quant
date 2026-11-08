//! Benchmarks for correlated-shock transforms.
//!
//! `CorrelationFactor::apply` runs per timestep per path in multi-asset Monte
//! Carlo, so its cost scales with n (asset count) × paths × steps. Measures the
//! exact-triangular hot path and the pivoted dense path separately.
//! Factorization, allocation and checked preflight are excluded. Each timed
//! call applies the existing factor to a fixed complete shock vector.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::linalg::CorrelationFactor;
use finstack_quant_core::math::linalg::cholesky_correlation;
use std::hint::black_box;

/// Equicorrelated matrix with off-diagonal `rho` — positive definite for the
/// values used here. Complete pivoting fills the unpermuted upper triangle.
fn equicorrelation(n: usize, rho: f64) -> Vec<f64> {
    (0..n * n)
        .map(|k| {
            let (i, j) = (k / n, k % n);
            if i == j { 1.0 } else { rho }
        })
        .collect()
}

/// Lower-triangular covariance factor with decreasing diagonals.
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

/// Check every output against an ordered dense scalar multiplication.
fn check_apply(factor: &CorrelationFactor, z: &[f64]) {
    let n = z.len();
    let matrix = factor.factor_matrix();
    assert_eq!(matrix.len(), n * n);
    assert!(matrix.iter().chain(z).all(|value| value.is_finite()));
    let mut out = vec![f64::NAN; n];
    factor.apply(z, &mut out).expect("valid apply preflight");
    assert_eq!(out.len(), n);
    for (row, &value) in matrix.chunks_exact(n).zip(&out) {
        let mut expected = 0.0;
        for (&coefficient, &shock) in row.iter().zip(z) {
            expected += coefficient * shock;
        }
        assert!(value.is_finite());
        assert_eq!(value, expected);
    }
}

fn bench_correlation_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("correlation_factor_apply");
    for &n in &[2usize, 8, 32, 64] {
        group.bench_with_input(BenchmarkId::new("triangular", n), &n, |b, _| {
            let z: Vec<f64> = (0..n).map(|i| ((i % 13) as f64 * 0.07) - 0.4).collect();
            let triangular = CorrelationFactor::from_parts(lower_triangular_factor(n), n)
                .expect("finite square benchmark factor");
            assert!(
                (0..n)
                    .all(|i| (i + 1..n).all(|j| { triangular.factor_matrix()[i * n + j] == 0.0 }))
            );
            check_apply(&triangular, &z);
            let mut out = vec![0.0; n];
            b.iter(|| {
                triangular
                    .apply(black_box(&z), &mut out)
                    .expect("matching dimensions");
                black_box(out.as_slice());
            });
        });

        group.bench_with_input(BenchmarkId::new("pivoted_dense", n), &n, |b, _| {
            let z: Vec<f64> = (0..n).map(|i| ((i % 13) as f64 * 0.07) - 0.4).collect();
            let correlation = equicorrelation(n, 0.5);
            let pivoted = cholesky_correlation(&correlation, n).expect("valid correlation matrix");
            let matrix = pivoted.factor_matrix();
            assert!((0..n).any(|i| (i + 1..n).any(|j| matrix[i * n + j] != 0.0)));
            for i in 0..n {
                for j in 0..n {
                    let reconstructed: f64 =
                        (0..n).map(|k| matrix[i * n + k] * matrix[j * n + k]).sum();
                    assert!((reconstructed - correlation[i * n + j]).abs() <= 1e-12);
                }
            }
            check_apply(&pivoted, &z);
            let mut out = vec![0.0; n];
            b.iter(|| {
                pivoted
                    .apply(black_box(&z), &mut out)
                    .expect("matching dimensions");
                black_box(out.as_slice());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_correlation_apply);
criterion_main!(benches);
