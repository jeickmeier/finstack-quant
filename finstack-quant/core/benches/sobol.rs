//! Benchmarks for Sobol quasi-random sequence generation.
//!
//! `fill_std_normals` / `fill_u01` are the innermost loop of every QMC
//! simulation. Scrambled and unscrambled fills are measured separately: the
//! recursive Owen scramble is a fixed per-value hash whose cost can dominate
//! the raw sequence generation, and only a split measurement shows which of
//! the two is worth optimizing.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::math::random::SobolRng;
use std::hint::black_box;

/// Points per iteration for each dimensionality (fixed total work).
const POINTS: usize = 1024;

fn bench_fill_std_normals(c: &mut Criterion) {
    let mut group = c.benchmark_group("sobol_fill_std_normals");
    for &dims in &[1usize, 8, 32] {
        for seed in [0u64, 42] {
            let label = if seed == 0 {
                "unscrambled"
            } else {
                "scrambled"
            };
            let mut rng = SobolRng::try_new(dims, seed).expect("valid dimension");
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_{dims}d"), POINTS),
                &POINTS,
                |b, _| {
                    let mut out = vec![0.0; POINTS * dims];
                    b.iter(|| {
                        rng.fill_std_normals(&mut out);
                        black_box(out[out.len() - 1])
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
            let mut rng = SobolRng::try_new(dims, seed).expect("valid dimension");
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_{dims}d"), POINTS),
                &POINTS,
                |b, _| {
                    let mut out = vec![0.0; POINTS * dims];
                    b.iter(|| {
                        rng.fill_u01(&mut out);
                        black_box(out[out.len() - 1])
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_fill_std_normals, bench_fill_u01);
criterion_main!(benches);
