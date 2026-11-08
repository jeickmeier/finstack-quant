//! Benchmarks for credit rating-migration Monte Carlo.
//!
//! `MigrationSimulator::empirical_matrix` and `simulate` run one Gillespie path
//! per sample (millions in a VaR/CVA run), so this measures the per-path cost —
//! the path that shares the `RatingScale` via `Arc` instead of cloning it.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_models::credit::migration::{
    GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale, TransitionMatrix,
};
use std::hint::black_box;

const HORIZON: f64 = 5.0;
const RECORDED_PATHS: usize = 1000;
const PATHS_PER_STATE: usize = 500;

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

/// Independent transition probabilities: default is an independent 2%/year
/// killing clock; otherwise 8%/year Poisson downgrades stop at the final rating.
fn expected_probability(from: usize, to: usize, n: usize) -> f64 {
    if from == n - 1 {
        return if to == from { 1.0 } else { 0.0 };
    }
    let survival = (-0.02 * HORIZON).exp();
    if to == n - 1 {
        return 1.0 - survival;
    }
    if to < from {
        return 0.0;
    }
    let x = 0.08 * HORIZON;
    let mut poisson = (-x).exp();
    if to == n - 2 {
        let mut earlier = 0.0;
        for k in 0..n - 2 - from {
            earlier += poisson;
            poisson *= x / (k + 1) as f64;
        }
        survival * (1.0 - earlier)
    } else {
        for k in 0..to - from {
            poisson *= x / (k + 1) as f64;
        }
        survival * poisson
    }
}

fn check_probability(actual: f64, expected: f64, samples: usize) {
    assert!(actual.is_finite() && (0.0..=1.0).contains(&actual));
    // Six binomial standard deviations plus a finite-sample allowance. This
    // checks the independent CTMC law without demanding exact MC convergence.
    let tolerance =
        6.0 * (expected * (1.0 - expected) / samples as f64).sqrt() + 6.0 / samples as f64;
    assert!(
        (actual - expected).abs() <= tolerance,
        "migration probability {actual} differs from {expected} by more than {tolerance}"
    );
}

fn check_paths(paths: &[RatingPath], repeated: &[RatingPath], sim: &MigrationSimulator) {
    let n = sim.generator().n_states();
    assert_eq!(paths.len(), RECORDED_PATHS);
    assert_eq!(repeated.len(), paths.len());
    let mut terminal_counts = vec![0usize; n];
    for (path, repeat) in paths.iter().zip(repeated) {
        assert_eq!(path.transitions(), repeat.transitions(), "seed must repeat");
        assert_eq!(path.horizon(), HORIZON);
        assert_eq!(repeat.horizon(), path.horizon());
        assert_eq!(path.scale().labels(), sim.generator().scale().labels());
        assert_eq!(repeat.scale().labels(), path.scale().labels());
        assert_eq!(path.scale().default_state(), Some(n - 1));
        let events = path.transitions();
        assert_eq!(events.first(), Some(&(0.0, 0)));
        assert_eq!(path.n_transitions(), events.len() - 1);
        for &(time, state) in events {
            assert!(time.is_finite() && (0.0..=HORIZON).contains(&time));
            assert!(state < n);
        }
        for pair in events.windows(2) {
            let (previous_time, previous_state) = pair[0];
            let (time, state) = pair[1];
            assert!(time > previous_time);
            assert!(previous_state < n - 1, "default must be absorbing");
            assert!(state == previous_state + 1 || state == n - 1);
        }
        let &(terminal_time, terminal_state) = events.last().expect("initial event");
        assert_eq!(path.state_at(HORIZON), terminal_state);
        assert_eq!(path.defaulted(), terminal_state == n - 1);
        assert_eq!(
            path.default_time(),
            (terminal_state == n - 1).then_some(terminal_time)
        );
        terminal_counts[terminal_state] += 1;
    }
    for (to, count) in terminal_counts.into_iter().enumerate() {
        check_probability(
            count as f64 / RECORDED_PATHS as f64,
            expected_probability(0, to, n),
            RECORDED_PATHS,
        );
    }
}

fn check_matrix(matrix: &TransitionMatrix, repeated: &TransitionMatrix, sim: &MigrationSimulator) {
    let n = sim.generator().n_states();
    assert_eq!(matrix.n_states(), n);
    assert_eq!(matrix.horizon(), HORIZON);
    assert_eq!(repeated.horizon(), matrix.horizon());
    assert_eq!(matrix.scale().labels(), sim.generator().scale().labels());
    assert_eq!(repeated.scale().labels(), matrix.scale().labels());
    assert_eq!(matrix.scale().default_state(), Some(n - 1));
    let rows = matrix.to_rows();
    assert_eq!(rows, repeated.to_rows(), "seed must repeat");
    assert_eq!(rows.len(), n);
    for (from, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), n);
        assert!((row.iter().sum::<f64>() - 1.0).abs() <= 1e-12);
        for (to, &actual) in row.iter().enumerate() {
            check_probability(actual, expected_probability(from, to, n), PATHS_PER_STATE);
            assert_eq!(
                matrix.probability_by_index(from, to).expect("valid state"),
                actual
            );
            if from == n - 1 {
                assert_eq!(actual, if to == from { 1.0 } else { 0.0 });
            } else if to < from {
                assert_eq!(actual, 0.0, "fixture cannot upgrade a rating");
            }
        }
    }
}

fn benchmark_migration(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration");

    for &n in &[5usize, 10] {
        group.throughput(Throughput::Elements(RECORDED_PATHS as u64));
        group.bench_with_input(
            BenchmarkId::new("simulate_1000_physical_paths", n),
            &n,
            |b, &n| {
                let sim =
                    MigrationSimulator::new(build_generator(n), HORIZON).expect("valid simulator");
                {
                    let paths = sim
                        .simulate(0, RECORDED_PATHS, 42)
                        .expect("valid simulation");
                    let repeated = sim
                        .simulate(0, RECORDED_PATHS, 42)
                        .expect("valid simulation");
                    check_paths(&paths, &repeated, &sim);
                }
                b.iter(|| {
                    black_box(
                        sim.simulate(black_box(0), black_box(RECORDED_PATHS), black_box(42))
                            .expect("benchmark uses a valid initial state"),
                    )
                });
            },
        );

        // Each initial state receives PATHS_PER_STATE paths, including default.
        let physical_paths = n * PATHS_PER_STATE;
        group.throughput(Throughput::Elements(physical_paths as u64));
        group.bench_with_input(
            BenchmarkId::new(
                format!("empirical_matrix_500_per_state_{physical_paths}_physical_paths"),
                n,
            ),
            &n,
            |b, &n| {
                let sim =
                    MigrationSimulator::new(build_generator(n), HORIZON).expect("valid simulator");
                {
                    let matrix = sim
                        .empirical_matrix(PATHS_PER_STATE, 7)
                        .expect("valid matrix");
                    let repeated = sim
                        .empirical_matrix(PATHS_PER_STATE, 7)
                        .expect("valid matrix");
                    check_matrix(&matrix, &repeated, &sim);
                }
                b.iter(|| {
                    black_box(
                        sim.empirical_matrix(black_box(PATHS_PER_STATE), black_box(7))
                            .expect("benchmark uses a positive path count"),
                    )
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_migration);
criterion_main!(benches);
