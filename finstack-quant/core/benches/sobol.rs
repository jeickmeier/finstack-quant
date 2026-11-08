//! Benchmarks for Sobol quasi-random sequence generation.
//!
//! `fill_std_normals` / `fill_u01` are the innermost loop of every QMC
//! simulation. Scrambled and unscrambled fills are measured separately: the
//! recursive Owen scramble is a fixed per-value hash whose cost can dominate
//! the raw sequence generation, and only a split measurement shows which of
//! the two is worth optimizing.
//!
//! Construction, allocation and preflight are excluded. A fresh benchmark RNG
//! starts after preflight and advances across warmup and measured calls; the
//! timed boundary does not reset it or repeatedly measure the first block.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::random::SobolRng;
use finstack_quant_core::math::special_functions::standard_normal_inv_cdf;
use std::hint::black_box;

/// Points per call; the scalar count is `POINTS * dims`.
const POINTS: usize = 1024;

/// Check batching against direct index reconstruction, independently of advance.
fn check_fill(dims: usize, seed: u64, normals: bool) {
    let mut actual = vec![f64::NAN; POINTS * dims];
    let mut batch_rng = SobolRng::try_new(dims, seed).expect("valid preflight dimension");
    if normals {
        batch_rng.fill_std_normals(&mut actual);
    } else {
        batch_rng.fill_u01(&mut actual);
    }

    let mut reference_rng = SobolRng::try_new(dims, seed).expect("valid reference dimension");
    let mut point = vec![0.0; dims];
    assert_eq!(actual.len(), POINTS * dims);
    for (index, actual_point) in actual.chunks_exact(dims).enumerate() {
        // `skip` reconstructs raw state from index bits; it does not reuse the
        // incremental state exercised by the batch fill under measurement.
        reference_rng.reset();
        reference_rng.skip(index as u64);
        reference_rng.fill_u01(&mut point);
        for (&value, &uniform) in actual_point.iter().zip(&point) {
            assert!(uniform.is_finite() && uniform > 0.0 && uniform < 1.0);
            let expected = if normals {
                standard_normal_inv_cdf(uniform)
            } else {
                uniform
            };
            assert!(value.is_finite());
            assert_eq!(
                value, expected,
                "point {index}, dimension {dims}, seed {seed}"
            );
        }
    }
}

fn bench_fill_std_normals(c: &mut Criterion) {
    let mut group = c.benchmark_group("sobol_fill_std_normals");
    for &dims in &[1usize, 8, 32] {
        for seed in [0u64, 42] {
            let label = if seed == 0 {
                "unscrambled"
            } else {
                "scrambled"
            };
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_{dims}d"), POINTS),
                &POINTS,
                |b, _| {
                    check_fill(dims, seed, true);
                    let mut rng = SobolRng::try_new(dims, seed).expect("valid dimension");
                    let mut out = vec![0.0; POINTS * dims];
                    b.iter(|| {
                        rng.fill_std_normals(&mut out);
                        black_box(out.as_slice());
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_fill_u01(c: &mut Criterion) {
    let mut group = c.benchmark_group("sobol_fill_u01");
    for &dims in &[1usize, 8, 32] {
        for seed in [0u64, 42] {
            let label = if seed == 0 {
                "unscrambled"
            } else {
                "scrambled"
            };
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_{dims}d"), POINTS),
                &POINTS,
                |b, _| {
                    check_fill(dims, seed, false);
                    let mut rng = SobolRng::try_new(dims, seed).expect("valid dimension");
                    let mut out = vec![0.0; POINTS * dims];
                    b.iter(|| {
                        rng.fill_u01(&mut out);
                        black_box(out.as_slice());
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_fill_std_normals, bench_fill_u01);
criterion_main!(benches);
