//! Benchmarks for credit rating-migration Monte Carlo.
//!
//! `MigrationSimulator::empirical_matrix` and `simulate` run one Gillespie path
//! per sample (millions in a VaR/CVA run), so this measures the per-path cost —
//! the path that shares the `RatingScale` via `Arc` instead of cloning it.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::credit::migration::simulation::MigrationSimulator;
use finstack_quant_core::credit::migration::{GeneratorMatrix, RatingScale};
use rand::SeedableRng;
use rand_pcg::Pcg64;
use std::hint::black_box;

/// Build a valid `n`-state generator: each non-absorbing state migrates to its
/// neighbour and (slowly) to the absorbing default state; the last state is
/// absorbing. Rows sum to zero with non-negative off-diagonals.
fn build_generator(n: usize) -> GeneratorMatrix {
    let labels: Vec<String> = (0..n - 1)
        .map(|i| format!("R{i}"))
        .chain(std::iter::once("D".to_string()))
        .collect();
    let scale = RatingScale::custom(labels).expect("valid scale");

    let mut data = vec![0.0f64; n * n];
    for i in 0..n - 1 {
        let to_default = 0.02;
        let to_next = if i + 1 == n - 1 { 0.0 } else { 0.08 };
        data[i * n + i] = -(to_default + to_next);
        if to_next > 0.0 {
            data[i * n + (i + 1)] = to_next;
        }
        data[i * n + (n - 1)] += to_default;
    }
    GeneratorMatrix::new(scale, &data).expect("valid generator")
}

fn benchmark_migration(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration");

    for &n in &[5usize, 10] {
        let gen = build_generator(n);
        let sim = MigrationSimulator::new(gen, 5.0).expect("valid simulator");

        group.bench_with_input(BenchmarkId::new("simulate_1000", n), &n, |b, _| {
            let mut rng = Pcg64::seed_from_u64(42);
            b.iter(|| {
                let paths = sim.simulate(black_box(0), black_box(1000), &mut rng);
                black_box(paths.len())
            });
        });

        group.bench_with_input(BenchmarkId::new("empirical_matrix_500", n), &n, |b, _| {
            let mut rng = Pcg64::seed_from_u64(7);
            b.iter(|| {
                let m = sim.empirical_matrix(black_box(500), &mut rng);
                black_box(m.n_states())
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_migration);
criterion_main!(benches);
