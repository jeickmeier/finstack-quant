//! Hot-path benchmarks for `finstack-quant-monte-carlo`.
//!
//! Covers the highest-iteration workloads:
//!
//! - European option pricing (GBM + exact discretization)
//! - LSMC backward induction for American options
//! - LSQ regression (SVD solve per exercise date)
//!
//! Run with:
//! ```sh
//! cargo bench -p finstack-quant-monte-carlo
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::fractional::HurstExponent;
use finstack_quant_monte_carlo::discretization::rough_heston::RoughHestonHybrid;
use finstack_quant_monte_carlo::discretization::{ExactHullWhite1F, QeHeston};
use finstack_quant_monte_carlo::engine::McEngine;
use finstack_quant_monte_carlo::payoff::vanilla::EuropeanCall;
use finstack_quant_monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_monte_carlo::pricer::lsmc::{AmericanPut, LsmcConfig, LsmcPricer};
use finstack_quant_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_monte_carlo::process::gbm::GbmProcess;
use finstack_quant_monte_carlo::process::heston::HestonProcess;
use finstack_quant_monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_quant_monte_carlo::process::rough_heston::{RoughHestonParams, RoughHestonProcess};
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::traits::Discretization;
use std::hint::black_box;

// ---------------------------------------------------------------------------
// European pricer: GBM + ExactGbm at various path counts
// ---------------------------------------------------------------------------

fn bench_european_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("european_pricer");
    let process = GbmProcess::with_params(0.05, 0.02, 0.20).unwrap();
    let payoff = EuropeanCall::new(100.0, 1.0, 252);
    let df = (-0.05_f64).exp();

    {
        let &num_paths = &10_000;
        group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
            let pricer = EuropeanPricer::new(n).with_seed(42).with_parallel(false);
            b.iter(|| {
                pricer
                    .price(&process, 100.0, 1.0, 252, &payoff, Currency::USD, df)
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// LSMC: American put backward induction at various path counts
// ---------------------------------------------------------------------------

fn bench_lsmc_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsmc_pricer");
    let process = GbmProcess::with_params(0.05, 0.02, 0.20).unwrap();
    let exercise = AmericanPut::new(100.0).expect("valid strike");
    let basis = PolynomialBasis::new(2);

    // Monthly exercise dates over 1 year (12 steps, 12 exercise opportunities)
    let num_steps = 12;
    let exercise_dates: Vec<usize> = (1..=num_steps).collect();

    {
        let &num_paths = &5_000;
        group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
            let config = LsmcConfig::new(n, exercise_dates.clone(), num_steps)
                .expect("valid LSMC config")
                .with_seed(42);
            let pricer = LsmcPricer::new(config);
            b.iter(|| {
                pricer
                    .price(
                        &process,
                        100.0,
                        1.0,
                        num_steps,
                        &exercise,
                        &basis,
                        Currency::USD,
                        0.05,
                    )
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// LSQ regression: SVD solve at various observation counts
// ---------------------------------------------------------------------------

fn bench_lsq_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsq_regression");
    let k = 3; // cubic basis: {1, x, x^2}

    for &n in &[500] {
        // Build a deterministic design matrix and response vector
        let mut design = vec![0.0; n * k];
        let mut y = vec![0.0; n];
        for i in 0..n {
            let x = (i as f64) / (n as f64);
            design[i * k] = 1.0;
            design[i * k + 1] = x;
            design[i * k + 2] = x * x;
            y[i] = 1.0 + 2.0 * x + 3.0 * x * x + 0.01 * (i as f64);
        }

        group.bench_with_input(BenchmarkId::new("observations", n), &n, |b, _| {
            b.iter(|| solve_least_squares(&design, &y, n, k).expect("should succeed"));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Heston QE: stochastic-vol European pricing via the generic engine.
// Exercises populate_path_state (default impl), the QE variance step, and the
// per-step dt-constant transcendentals.
// ---------------------------------------------------------------------------

fn bench_heston_qe_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("heston_qe_pricer");
    let process =
        HestonProcess::with_params(0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid Heston");
    let disc = QeHeston::new();
    let payoff = EuropeanCall::new(100.0, 1.0, 252);
    let rng = PhiloxRng::new(42);
    let df = (-0.03_f64).exp();

    let num_paths = 5_000;
    group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
        let engine = McEngine::builder()
            .num_paths(n)
            .uniform_grid(1.0, 252)
            .parallel(false)
            .build()
            .expect("valid engine config");
        b.iter(|| {
            engine
                .price(
                    &rng,
                    &process,
                    &disc,
                    &[100.0, 0.04],
                    &payoff,
                    Currency::USD,
                    df,
                )
                .expect("pricing should succeed")
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Rough Heston hybrid step: the O(n^2)-per-path Volterra discretization.
// Drives a single 252-step path's worth of `step()` calls, mirroring the
// engine's per-path loop, isolating the discretization cost.
// ---------------------------------------------------------------------------

fn bench_rough_heston_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("rough_heston_step");

    for &num_steps in &[100_usize, 252] {
        let t_max = 1.0_f64;
        let times: Vec<f64> = (0..=num_steps)
            .map(|i| t_max * i as f64 / num_steps as f64)
            .collect();
        let dt = t_max / num_steps as f64;
        let hurst = HurstExponent::new(0.1).expect("valid hurst");
        let params = RoughHestonParams::new(0.03, 0.0, hurst, 2.0, 0.04, 0.3, -0.7, 0.04)
            .expect("valid rough Heston params");
        let process = RoughHestonProcess::new(params);
        let scheme = RoughHestonHybrid::new(&times, 0.1).expect("valid scheme");
        let work_size = 2 * num_steps + 1;
        let z = [0.5_f64, -0.3];

        group.bench_with_input(BenchmarkId::new("steps", num_steps), &num_steps, |b, &n| {
            let mut work = vec![0.0; work_size];
            b.iter(|| {
                let mut x = [100.0_f64, 0.04];
                work.iter_mut().for_each(|w| *w = 0.0);
                let mut t = 0.0;
                for _ in 0..n {
                    scheme.step(&process, t, dt, &mut x, &z, &mut work);
                    t += dt;
                }
                black_box(x[1])
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Hull-White 1F exact: a short-rate exact scheme whose per-step cost is
// dominated by the `dt`-dependent transcendentals hoisted by `prepare`.
// ---------------------------------------------------------------------------

fn bench_hw1f_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("hw1f_pricer");
    let process = HullWhite1FProcess::new(HullWhite1FParams::new(0.1, 0.01, 0.03));
    let disc = ExactHullWhite1F::new();
    let payoff = EuropeanCall::new(0.03, 1.0, 252);
    let rng = PhiloxRng::new(42);
    let df = (-0.03_f64).exp();

    let num_paths = 20_000;
    group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
        let engine = McEngine::builder()
            .num_paths(n)
            .uniform_grid(1.0, 252)
            .parallel(false)
            .build()
            .expect("valid engine config");
        b.iter(|| {
            engine
                .price(&rng, &process, &disc, &[0.03], &payoff, Currency::USD, df)
                .expect("pricing should succeed")
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_european_pricer,
    bench_lsmc_pricer,
    bench_lsq_regression,
    bench_heston_qe_pricer,
    bench_rough_heston_step,
    bench_hw1f_pricer
);
criterion_main!(benches);
