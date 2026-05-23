//! Merton Monte Carlo structural credit pricing benchmarks.
//!
//! Benchmarks the `MertonMcEngine::price` hot path — the main inner loop
//! iterating over `paths x time_steps x coupon_dates`. This engine is the
//! heaviest single-instrument pricer in the library and has no dedicated
//! benchmark coverage in `mc_pricing.rs` (which only covers Bermudan LSMC).
//!
//! Scenarios:
//! - Path count scaling (1K / 5K / 10K / 50K) — isolates Monte Carlo cost
//! - Tenor scaling (3Y / 5Y / 10Y) — isolates schedule loop cost
//! - Antithetic variates on vs. off
//! - PIK mode: cash vs. PIK vs. PIK-toggle
//! - Barrier type: terminal vs. first-passage (Brownian bridge)

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{
    BarrierCrossing, MertonMcConfig, MertonMcEngine, PikMode, PikSchedule,
};
use finstack_valuations::models::credit::{BarrierType, MertonModel};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

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
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.02,
        },
        finstack_valuations::models::credit::AssetDynamics::GeometricBrownian,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Benchmark: path count scaling (5Y PIK bond, semi-annual coupons)
// ---------------------------------------------------------------------------

fn bench_merton_mc_path_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_paths");

    for n_paths in [1_000usize, 5_000, 10_000, 50_000] {
        let config = MertonMcConfig::new(reference_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(n_paths)
            .seed(42)
            .antithetic(true);

        group.throughput(Throughput::Elements(n_paths as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n_paths), &n_paths, |b, _| {
            b.iter(|| {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(&config),
                    black_box(0.04),
                )
                .unwrap()
                .clean_price_pct
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: tenor scaling (10K paths, semi-annual PIK coupons)
// ---------------------------------------------------------------------------

fn bench_merton_mc_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_tenor");
    const PATHS: usize = 10_000;

    for (label, maturity_years) in [("3Y", 3.0f64), ("5Y", 5.0), ("10Y", 10.0)] {
        let config = MertonMcConfig::new(reference_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(PATHS)
            .seed(42);

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &maturity_years,
            |b, &mat| {
                b.iter(|| {
                    MertonMcEngine::price(
                        black_box(100.0),
                        black_box(0.08),
                        black_box(mat),
                        black_box(2),
                        black_box(&config),
                        black_box(0.04),
                    )
                    .unwrap()
                    .clean_price_pct
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: antithetic variates on vs. off (10K effective paths, 5Y)
// ---------------------------------------------------------------------------

fn bench_merton_mc_antithetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_antithetic");

    let config_on = MertonMcConfig::new(reference_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .antithetic(true);

    let config_off = MertonMcConfig::new(reference_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .antithetic(false);

    group.bench_function("antithetic_on", |b| {
        b.iter(|| {
            MertonMcEngine::price(
                black_box(100.0),
                black_box(0.08),
                black_box(5.0),
                black_box(2),
                black_box(&config_on),
                black_box(0.04),
            )
            .unwrap()
            .clean_price_pct
        });
    });

    group.bench_function("antithetic_off", |b| {
        b.iter(|| {
            MertonMcEngine::price(
                black_box(100.0),
                black_box(0.08),
                black_box(5.0),
                black_box(2),
                black_box(&config_off),
                black_box(0.04),
            )
            .unwrap()
            .clean_price_pct
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: PIK mode comparison (cash vs. PIK vs. toggle)
// ---------------------------------------------------------------------------

fn bench_merton_mc_pik_mode(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_pik_mode");

    let config_cash = MertonMcConfig::new(reference_merton())
        .num_paths(10_000)
        .seed(42);

    let config_pik = MertonMcConfig::new(reference_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);

    let config_toggle = MertonMcConfig::new(reference_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Toggle))
        .num_paths(10_000)
        .seed(42);

    for (label, config) in [
        ("cash", &config_cash),
        ("pik", &config_pik),
        ("toggle", &config_toggle),
    ] {
        group.bench_function(label, |b| {
            b.iter(|| {
                MertonMcEngine::price(
                    black_box(100.0),
                    black_box(0.08),
                    black_box(5.0),
                    black_box(2),
                    black_box(config),
                    black_box(0.04),
                )
                .unwrap()
                .clean_price_pct
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: barrier type — terminal vs. first-passage (Brownian bridge)
// ---------------------------------------------------------------------------

fn bench_merton_mc_barrier_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("merton_mc_barrier");

    let config_terminal = MertonMcConfig::new(reference_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .barrier_crossing(BarrierCrossing::Discrete);

    let config_fp = MertonMcConfig::new(first_passage_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .barrier_crossing(BarrierCrossing::BrownianBridge);

    group.bench_function("terminal_discrete", |b| {
        b.iter(|| {
            MertonMcEngine::price(
                black_box(100.0),
                black_box(0.08),
                black_box(5.0),
                black_box(2),
                black_box(&config_terminal),
                black_box(0.04),
            )
            .unwrap()
            .clean_price_pct
        });
    });

    group.bench_function("first_passage_bridge", |b| {
        b.iter(|| {
            MertonMcEngine::price(
                black_box(100.0),
                black_box(0.08),
                black_box(5.0),
                black_box(2),
                black_box(&config_fp),
                black_box(0.04),
            )
            .unwrap()
            .clean_price_pct
        });
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
