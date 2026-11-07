//! Merton Monte Carlo structural credit pricing benchmarks.
//!
//! Benchmarks the `MertonMcEngine::price` hot path — the main inner loop
//! iterating over `paths x time_steps x coupon_dates`. This engine is the
//! heaviest single-instrument pricer in the library and has no dedicated
//! benchmark coverage in `mc_pricing.rs` (which only covers Bermudan LSMC).
//!
//! Scenarios:
//! - Representative path count (10K) — isolates Monte Carlo cost
//! - Representative tenor (5Y) — isolates schedule loop cost
//! - Antithetic variates on vs. off
//! - PIK mode: cash vs. PIK vs. PIK-toggle
//! - Barrier type: terminal vs. first-passage (Brownian bridge)

#![allow(clippy::unwrap_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_models::credit::{MertonBarrierType, MertonModel};
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::merton_mc::{
    MertonMcEngine, MertonMcResult, MertonMcRun,
};
use finstack_quant_valuations::instruments::{
    BarrierCrossing, MertonMcConfig, PikMode, PikSchedule,
};
use std::hint::black_box;

mod support;

fn assert_merton_result(result: &MertonMcResult, run: &MertonMcRun) {
    for value in [
        result.clean_price_pct,
        result.dirty_price_pct,
        result.expected_loss,
        result.unexpected_loss,
        result.expected_shortfall_95,
        result.average_pik_fraction,
        result.effective_spread_bp,
        result.standard_error,
        result.path_statistics.default_rate,
        result.path_statistics.avg_default_time,
        result.path_statistics.avg_terminal_notional,
        result.path_statistics.avg_recovery_rate,
        result.path_statistics.pik_exercise_rate,
    ] {
        assert!(value.is_finite(), "non-finite Merton MC output");
    }
    assert!(result.standard_error >= 0.0);
    assert!((0.0..=1.0).contains(&result.path_statistics.default_rate));
    // The result counts independent estimators; each antithetic pair counts once.
    assert_eq!(result.num_paths, run.num_paths);
}

// Shared fixtures

/// Reference Merton model: asset=200, vol=25%, barrier=100, r=4%.
fn reference_merton() -> MertonModel {
    MertonModel::new(200.0, 0.25, 100.0, 0.04).unwrap()
}

/// Reference Merton model with first-passage barrier.
fn first_passage_merton() -> MertonModel {
    MertonModel::new_with_dynamics(
        200.0,
        0.25,
        100.0,
        0.04,
        0.0,
        MertonBarrierType::FirstPassage {
            barrier_growth_rate: 0.02,
        },
        finstack_quant_models::credit::AssetDynamics::GeometricBrownian,
    )
    .unwrap()
}

// Benchmark: path count scaling (5Y PIK bond, semi-annual coupons)

fn bench_merton_mc_path_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_paths");

    let n_paths = 10_000;
    let config = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik));
    let config_run = MertonMcRun {
        num_paths: (n_paths) / 2,
        seed: 42,
        antithetic: true,
    };

    group.throughput(Throughput::Elements(n_paths as u64));
    group.bench_with_input(BenchmarkId::from_parameter(n_paths), &n_paths, |b, _| {
        support::bench(
            b,
            || {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config),
                    black_box(&config_run),
                    black_box(0.04),
                )
                .expect("Merton MC price")
            },
            |result| assert_merton_result(result, &config_run),
        );
    });

    group.finish();
}

// Benchmark: tenor scaling (10K paths, semi-annual PIK coupons)

fn bench_merton_mc_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_tenor");
    const PATHS: usize = 10_000;

    let label = "5Y";
    let maturity_years = 5.0;
    let config = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik));
    let config_run = MertonMcRun {
        num_paths: (PATHS) / 2,
        seed: 42,
        antithetic: true,
    };

    group.bench_with_input(
        BenchmarkId::from_parameter(label),
        &maturity_years,
        |b, &mat| {
            support::bench(
                b,
                || {
                    MertonMcEngine::price(
                        black_box(100.0),
                        black_box(0.08),
                        black_box(mat),
                        black_box(2),
                        black_box(&config),
                        black_box(&config_run),
                        black_box(0.04),
                    )
                    .expect("Merton MC price")
                },
                |result| assert_merton_result(result, &config_run),
            );
        },
    );

    group.finish();
}

// Benchmark: antithetic variates on vs. off (10K effective paths, 5Y)

fn bench_merton_mc_antithetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_antithetic");

    let config_on = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik));
    let config_on_run = MertonMcRun {
        num_paths: 5000,
        seed: 42,
        antithetic: true,
    };

    let config_off = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik));
    let config_off_run = MertonMcRun {
        num_paths: 10_000,
        seed: 42,
        antithetic: false,
    };

    group.bench_function("antithetic_on", |b| {
        support::bench(
            b,
            || {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config_on),
                    black_box(&config_on_run),
                    black_box(0.04),
                )
                .expect("Merton MC price")
            },
            |result| assert_merton_result(result, &config_on_run),
        );
    });

    group.bench_function("antithetic_off", |b| {
        support::bench(
            b,
            || {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config_off),
                    black_box(&config_off_run),
                    black_box(0.04),
                )
                .expect("Merton MC price")
            },
            |result| assert_merton_result(result, &config_off_run),
        );
    });

    group.finish();
}

// Benchmark: PIK mode comparison (cash vs. PIK vs. toggle)

fn bench_merton_mc_pik_mode(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_pik_mode");

    let config_cash =
        MertonMcConfig::new(reference_merton(), 0.40).expect("0.40 recovery should be valid");

    let config_pik = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik));

    let config_toggle = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Toggle));

    let run = MertonMcRun {
        num_paths: 5_000,
        seed: 42,
        antithetic: true,
    };

    for (label, config) in [
        ("cash", &config_cash),
        ("pik", &config_pik),
        ("toggle", &config_toggle),
    ] {
        group.bench_function(label, |b| {
            support::bench(
                b,
                || {
                    MertonMcEngine::price(
                        black_box(100.0),
                        black_box(0.08),
                        black_box(5.0),
                        black_box(2),
                        black_box(config),
                        black_box(&run),
                        black_box(0.04),
                    )
                    .expect("Merton MC price")
                },
                |result| assert_merton_result(result, &run),
            );
        });
    }

    group.finish();
}

// Benchmark: barrier type — terminal vs. first-passage (Brownian bridge)

fn bench_merton_mc_barrier_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_barrier");

    let config_terminal = MertonMcConfig::new(reference_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .barrier_crossing(BarrierCrossing::Discrete);
    let config_terminal_run = MertonMcRun {
        num_paths: 5000,
        seed: 42,
        antithetic: true,
    };

    let config_fp = MertonMcConfig::new(first_passage_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .barrier_crossing(BarrierCrossing::BrownianBridge);
    let config_fp_run = MertonMcRun {
        num_paths: 5000,
        seed: 42,
        antithetic: true,
    };

    group.bench_function("terminal_discrete", |b| {
        support::bench(
            b,
            || {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config_terminal),
                    black_box(&config_terminal_run),
                    black_box(0.04),
                )
                .expect("Merton MC price")
            },
            |result| assert_merton_result(result, &config_terminal_run),
        );
    });

    group.bench_function("first_passage_bridge", |b| {
        support::bench(
            b,
            || {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config_fp),
                    black_box(&config_fp_run),
                    black_box(0.04),
                )
                .expect("Merton MC price")
            },
            |result| assert_merton_result(result, &config_fp_run),
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_merton_mc_path_count,
    bench_merton_mc_tenor,
    bench_merton_mc_antithetic,
    bench_merton_mc_pik_mode,
    bench_merton_mc_barrier_type,
);
criterion_main!(benches);
