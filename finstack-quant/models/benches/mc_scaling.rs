//! Scaling guards for `finstack-quant-models`.
//!
//! Complements `mc_hot_paths.rs` (absolute cost at one size) by measuring how
//! cost grows with path count, step count, asset count, or LMM tenor. Read
//! ns-per-element across sizes: flat is linear; rising means a super-linear
//! term is back.
//!
//! ```sh
//! cargo bench -p finstack-quant-models --bench mc_scaling
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::fractional::HurstExponent;
use finstack_quant_models::monte_carlo::discretization::rough_heston::RoughHestonHybrid;
use finstack_quant_models::monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::{AmericanPut, LsmcConfig, LsmcPricer};
use finstack_quant_models::monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_models::monte_carlo::process::rough_heston::{
    RoughHestonParams, RoughHestonProcess,
};
use finstack_quant_models::monte_carlo::rng::fbm::FractionalNoiseGenerator;
use finstack_quant_models::monte_carlo::rng::volterra::RiemannLiouvilleVolterra;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream};
use fixtures::{
    discount, european_call, exact_gbm, gbm, heston, lmm_process, lmm_scheme, multi_gbm, philox,
    qe_heston, serial_engine, RATE, SEED, SPOT, STRIKE,
};
use std::hint::black_box;

fn scaling_european_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_european_paths");
    let process = gbm();
    let payoff = european_call(52);
    let df = discount();

    for &n in &[2_500_usize, 10_000, 40_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let pricer = EuropeanPricer::new(n)
                .expect("positive path count")
                .with_seed(SEED)
                .with_parallel(false);
            b.iter(|| {
                pricer
                    .price(&process, SPOT, 1.0, 52, &payoff, Currency::USD, df)
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

fn scaling_european_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_european_steps");
    let process = gbm();
    let df = discount();
    let pricer = EuropeanPricer::new(2_000)
        .expect("positive path count")
        .with_seed(SEED)
        .with_parallel(false);

    for &steps in &[52_usize, 126, 252] {
        let payoff = european_call(steps);
        group.throughput(Throughput::Elements(steps as u64));
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, &steps| {
            b.iter(|| {
                pricer
                    .price(&process, SPOT, 1.0, steps, &payoff, Currency::USD, df)
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

fn scaling_heston_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_heston_paths");
    let process = heston();
    let disc = qe_heston();
    let payoff = european_call(52);
    let rng = philox();
    let df = (-0.03_f64).exp();

    for &n in &[1_000_usize, 4_000, 8_000] {
        let engine = serial_engine(n, 52);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                engine
                    .price(
                        &rng,
                        &process,
                        &disc,
                        &[SPOT, 0.04],
                        &payoff,
                        Currency::USD,
                        df,
                    )
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

fn scaling_lsmc_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lsmc_paths");
    let process = gbm();
    let exercise = AmericanPut::new(STRIKE).expect("valid strike");
    let basis = PolynomialBasis::new(2);
    let num_steps = 12;
    let exercise_dates: Vec<usize> = (1..=num_steps).collect();

    for &n in &[1_000_usize, 2_500, 5_000] {
        let config = LsmcConfig::new(n, exercise_dates.clone(), num_steps)
            .expect("valid LSMC config")
            .with_seed(SEED);
        let pricer = LsmcPricer::new(config);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                pricer
                    .price(
                        &process,
                        SPOT,
                        1.0,
                        num_steps,
                        &exercise,
                        &basis,
                        Currency::USD,
                        RATE,
                    )
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

fn scaling_lsq_observations(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lsq_observations");
    let k = 3;

    for &n in &[100_usize, 500, 2_000] {
        let mut design = vec![0.0; n * k];
        let mut y = vec![0.0; n];
        for i in 0..n {
            let x = (i as f64) / (n as f64);
            design[i * k] = 1.0;
            design[i * k + 1] = x;
            design[i * k + 2] = x * x;
            y[i] = 1.0 + 2.0 * x + 3.0 * x * x;
        }
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| solve_least_squares(&design, &y, n, k).expect("should succeed"));
        });
    }
    group.finish();
}

fn scaling_rough_heston_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rough_heston_steps");

    for &num_steps in &[50_usize, 100, 200] {
        let t_max = 1.0_f64;
        let times: Vec<f64> = (0..=num_steps)
            .map(|i| t_max * i as f64 / num_steps as f64)
            .collect();
        let dt = t_max / num_steps as f64;
        let hurst = HurstExponent::new(0.1).expect("valid hurst");
        let params = RoughHestonParams {
            r: 0.03,
            q: 0.0,
            hurst,
            kappa: 2.0,
            theta: 0.04,
            sigma_v: 0.3,
            rho: -0.7,
            v0: 0.04,
        }
        .validate()
        .expect("valid rough Heston params");
        let process = RoughHestonProcess::new(params);
        let scheme = RoughHestonHybrid::new(&times, 0.1).expect("valid scheme");
        let work_size = 2 * num_steps + 1;
        let z = [0.5_f64, -0.3, 0.2];
        let kernel_ops = (num_steps * (num_steps + 1) / 2) as u64;

        group.throughput(Throughput::Elements(kernel_ops));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_steps),
            &num_steps,
            |b, &n| {
                let mut work = vec![0.0; work_size];
                b.iter(|| {
                    let mut x = [SPOT, 0.04];
                    work.fill(0.0);
                    let mut t = 0.0;
                    for _ in 0..n {
                        scheme.step(&process, t, dt, &mut x, &z, &mut work);
                        t += dt;
                    }
                    black_box(x[1])
                });
            },
        );
    }
    group.finish();
}

fn scaling_lmm_forwards(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lmm_forwards");

    for &n in &[10_usize, 20, 40] {
        let process = lmm_process(n);
        let scheme = lmm_scheme();
        let work_size = scheme.work_size(&process);
        let dt = 0.25;
        let z = [0.4_f64, -0.2];
        let initial = vec![0.03; n];
        let steps = n;
        // Predictor-corrector drift is O(alive²) per step; throughput is
        // forward-count so rising ns-per-forward flags a worse-than-quadratic
        // term coming back.
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            let mut work = vec![0.0; work_size];
            b.iter(|| {
                let mut x = initial.clone();
                work.fill(0.0);
                let mut t = 0.0;
                for _ in 0..steps {
                    scheme.step(&process, t, dt, &mut x, &z, &mut work);
                    t += dt;
                }
                black_box(x[0])
            });
        });
    }
    group.finish();
}

fn scaling_multi_gbm_assets(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_multi_gbm_assets");
    let rng = philox();
    let df = discount();

    for &n in &[2_usize, 5, 10] {
        let (process, disc, spots) = multi_gbm(n);
        let payoff = european_call(52);
        let engine = serial_engine(2_000, 52);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                engine
                    .price(&rng, &process, &disc, &spots, &payoff, Currency::USD, df)
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

fn scaling_volterra_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_volterra_steps");
    let mut rng = philox();

    for &n in &[64_usize, 128, 256] {
        let gen = RiemannLiouvilleVolterra::new(1.0, n, 0.1).expect("valid volterra");
        let mut normals = vec![0.0; gen.num_steps() * gen.normals_per_step()];
        rng.fill_std_normals(&mut normals);
        let mut out = vec![0.0; gen.num_steps()];
        let kernel_ops = (n * (n + 1) / 2) as u64;
        group.throughput(Throughput::Elements(kernel_ops));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                gen.generate(&normals, &mut out);
                black_box(out[0])
            });
        });
    }
    group.finish();
}

fn scaling_exact_gbm_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_exact_gbm_steps");
    let process = gbm();
    let scheme = exact_gbm();
    let z = [0.5_f64];

    for &n in &[52_usize, 126, 252, 504] {
        let dt = 1.0 / n as f64;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut x = [SPOT];
                let mut t = 0.0;
                for _ in 0..n {
                    scheme.step(&process, t, dt, &mut x, &z, &mut []);
                    t += dt;
                }
                black_box(x[0])
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scaling_european_paths,
    scaling_european_steps,
    scaling_heston_paths,
    scaling_lsmc_paths,
    scaling_lsq_observations,
    scaling_rough_heston_steps,
    scaling_lmm_forwards,
    scaling_multi_gbm_assets,
    scaling_volterra_steps,
    scaling_exact_gbm_steps
);
criterion_main!(benches);
