//! Checked serial Monte Carlo pricing, regression and rough-path workloads.
//!
//! Run with `mise run rust-bench-crate -- finstack-quant-models mc_hot_paths`.
//! Selected cases validate untimed outputs before entering the original timed
//! full-pricing or direct-step boundary. See `benches/README.md` for counts.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/validity.rs"]
mod validity;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::fractional::HurstExponent;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_models::closed_form::{
    HestonPricingParams, bs_price, heston_call_price_fourier,
};
use finstack_quant_models::monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston;
use finstack_quant_models::monte_carlo::discretization::rough_heston::RoughHestonHybrid;
use finstack_quant_models::monte_carlo::engine::{
    McEngine, McEngineConfig, McPricingInputs, PathCaptureConfig,
};
use finstack_quant_models::monte_carlo::payoff::asian::{Asian, AveragingMethod};
use finstack_quant_models::monte_carlo::payoff::vanilla::European;
use finstack_quant_models::monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::{
    AmericanExercise, LsmcConfig, LsmcEngine, LsmcInputs,
};
use finstack_quant_models::monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_models::monte_carlo::process::heston::HestonProcess;
use finstack_quant_models::monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_quant_models::monte_carlo::process::rough_heston::{
    RoughHestonParams, RoughHestonProcess,
};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::Discretization;
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::heston::HestonParams;
use std::hint::black_box;
use validity::{
    check_against_reference, check_estimate, check_lsq, check_repeat, check_rough_first_step,
    monthly_bermudan_put_reference,
};

fn serial_config(num_paths: usize, num_steps: usize) -> McEngineConfig {
    let mut config = McEngineConfig::uniform(num_paths, 1.0, num_steps)
        .expect("valid benchmark engine configuration")
        .parallel(false)
        .antithetic(false)
        .path_capture(PathCaptureConfig::new());
    config.target_ci_half_width = None;
    config.chunk_size = None;
    config
}

fn bench_european_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("european_pricer");
    let num_paths = 10_000;
    group.throughput(Throughput::Elements(num_paths as u64));
    group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
        let process = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.20).expect("valid GBM"));
        let payoff = European::new(OptionType::Call, 100.0, 1.0, 252);
        let df = (-0.05_f64).exp();
        let pricer = EuropeanPricer::new(n)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);
        // The public pricer creates its fixed-count, no-capture engine inside
        // price. Keep grid construction timed, as in the original workload.
        let price = || {
            pricer
                .price(
                    &process,
                    100.0,
                    TimeGrid::uniform(1.0, 252).expect("valid time grid"),
                    &payoff,
                    Currency::USD,
                    df,
                )
                .expect("European pricing should succeed")
        };
        let checked = price();
        check_estimate(&checked, n, n);
        check_repeat(&checked, &price());
        let reference = bs_price(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call)
            .expect("valid Black-Scholes reference");
        check_against_reference(&checked, reference, 1e-8);
        b.iter(price);
    });
    group.finish();
}

fn bench_lsmc_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsmc_pricer");
    let num_estimators = 5_000;
    group.throughput(Throughput::Elements((2 * num_estimators) as u64));
    group.bench_with_input(
        BenchmarkId::new("estimators", "5000_physical_paths_10000"),
        &num_estimators,
        |b, &n| {
            let process = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.20).expect("valid GBM"));
            let exercise = AmericanExercise::new(OptionType::Put, 100.0).expect("valid strike");
            let basis = PolynomialBasis::new(2).expect("valid regression basis");
            // Monthly Bermudan exercise excludes valuation-date exercise.
            let num_steps = 12;
            let exercise_dates: Vec<usize> = (1..=num_steps).collect();
            let config = LsmcConfig::new(n, exercise_dates, num_steps)
                .expect("valid LSMC config")
                .with_seed(42)
                .with_parallel(false)
                .with_antithetic(true);
            let pricer = LsmcEngine::new(config);
            let inputs = LsmcInputs {
                process: &process,
                initial_spot: 100.0,
                time_to_maturity: 1.0,
                num_steps,
                exercise: &exercise,
                basis: &basis,
                discount_rate: 0.05,
            };
            let price = || {
                pricer
                    .price(&inputs, Currency::USD)
                    .expect("LSMC pricing should succeed")
            };
            let checked = price();
            check_estimate(&checked, n, 2 * n);
            check_repeat(&checked, &price());
            // Smoke envelope for sampling, tree convergence and the in-sample
            // fitted policy; existing tighter LSMC tests remain authoritative.
            check_against_reference(&checked, monthly_bermudan_put_reference(), 0.05);
            b.iter(price);
        },
    );
    group.finish();
}

fn bench_lsq_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsq_regression");
    let n = 500;
    group.throughput(Throughput::Elements(n as u64));
    group.bench_with_input(BenchmarkId::new("observations", n), &n, |b, &n| {
        let k = 3; // Quadratic basis: {1, x, x^2}.
        let mut design = vec![0.0; n * k];
        let mut y = vec![0.0; n];
        for i in 0..n {
            let x = i as f64 / n as f64;
            design[i * k] = 1.0;
            design[i * k + 1] = x;
            design[i * k + 2] = x * x;
            y[i] = 1.0 + 2.0 * x + 3.0 * x * x + 0.01 * i as f64;
        }
        let solve = || {
            solve_least_squares(black_box(&design), black_box(&y), n, k)
                .expect("least-squares solve should succeed")
        };
        let checked = solve();
        // Since i = n*x, the last response term is linear, not random noise.
        check_lsq(&checked, &[1.0, 2.0 + 0.01 * n as f64, 3.0], &design, &y);
        assert_eq!(checked, solve(), "deterministic regression must repeat");
        b.iter(solve);
    });
    group.finish();
}

fn bench_heston_qe_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("heston_qe_pricer");
    let num_paths = 5_000;
    group.throughput(Throughput::Elements(num_paths as u64));
    group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
        let params =
            HestonPricingParams::new(0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid Heston");
        let process = HestonProcess::new(params);
        let disc = QeHeston::new();
        let payoff = European::new(OptionType::Call, 100.0, 1.0, 252);
        let rng = PhiloxRng::new(42);
        let engine = McEngine::new(serial_config(n, 252));
        let df = (-0.03_f64).exp();
        let price = || {
            engine
                .price(McPricingInputs {
                    rng: &rng,
                    process: &process,
                    disc: &disc,
                    initial_state: &[100.0, 0.04],
                    payoff: &payoff,
                    currency: Currency::USD,
                    discount_factor: df,
                })
                .expect("Heston QE pricing should succeed")
        };
        let checked = price();
        check_estimate(&checked, n, n);
        check_repeat(&checked, &price());
        let reference = heston_call_price_fourier(100.0, 100.0, 1.0, &params, None)
            .expect("converged Heston Fourier reference");
        // This is a 252-step QE smoke check, not a replacement for existing
        // finer-step martingale, convergence and Fourier accuracy tests.
        check_against_reference(&checked, reference, 0.05);
        b.iter(price);
    });
    group.finish();
}

fn bench_rough_heston_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("rough_heston_step");
    for &num_steps in &[100_usize, 252] {
        group.throughput(Throughput::Elements(
            (num_steps * (num_steps + 1) / 2) as u64,
        ));
        group.bench_with_input(BenchmarkId::new("steps", num_steps), &num_steps, |b, &n| {
            let times: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
            let dt = 1.0 / n as f64;
            let process = RoughHestonProcess::new(RoughHestonParams {
                r: 0.03,
                q: 0.0,
                hurst: HurstExponent::new(0.1).expect("valid Hurst exponent"),
                model: HestonParams {
                    v0: 0.04,
                    kappa: 2.0,
                    theta: 0.04,
                    sigma_v: 0.3,
                    rho: -0.7,
                },
            });
            let scheme = RoughHestonHybrid::new(&times, 0.1).expect("valid scheme");
            let mut work = vec![0.0; scheme.work_size(&process)];
            let z = [0.5_f64, -0.3, 0.2];
            let mut first = [100.0_f64, 0.04];
            scheme.step(&process, 0.0, dt, &mut first, &z, &mut work);
            check_rough_first_step(&first, dt, &z);
            let path = |work: &mut [f64]| {
                let mut x = [100.0_f64, 0.04];
                work.fill(0.0);
                let mut t = 0.0;
                for _ in 0..n {
                    scheme.step(&process, t, dt, &mut x, &z, work);
                    t += dt;
                }
                x
            };
            let checked = path(&mut work);
            assert!(checked.iter().all(|value| value.is_finite()));
            assert!(checked[0] > 0.0 && checked[1] >= 0.0);
            assert!(work.iter().all(|value| value.is_finite()));
            assert_eq!(work[2 * n], n as f64);
            assert_eq!(checked.map(f64::to_bits), path(&mut work).map(f64::to_bits));
            b.iter(|| black_box(path(&mut work)));
        });
    }
    group.finish();
}

fn bench_hw1f_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("hw1f_pricer");
    let num_paths = 20_000;
    group.throughput(Throughput::Elements(num_paths as u64));
    group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
        let process = HullWhite1FProcess::new(
            HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid Hull-White parameters"),
        );
        let disc = ExactHullWhite1F::new();
        let payoff = European::new(OptionType::Call, 0.03, 1.0, 252);
        let rng = PhiloxRng::new(42);
        let engine = McEngine::new(serial_config(n, 252));
        let df = (-0.03_f64).exp();
        let price = || {
            engine
                .price(McPricingInputs {
                    rng: &rng,
                    process: &process,
                    disc: &disc,
                    initial_state: &[0.03],
                    payoff: &payoff,
                    currency: Currency::USD,
                    discount_factor: df,
                })
                .expect("Hull-White pricing should succeed")
        };
        let checked = price();
        check_estimate(&checked, n, n);
        check_repeat(&checked, &price());
        // Synthetic call on r_T with a scalar discount multiplier. Its exact
        // normal-call expectation at mean=strike is df*Std[r_T]/sqrt(2*pi).
        let std_dev = 0.01 * ((1.0 - (-0.2_f64).exp()) / 0.2).sqrt();
        let reference = df * std_dev / std::f64::consts::TAU.sqrt();
        check_against_reference(&checked, reference, 1e-10);
        b.iter(price);
    });
    group.finish();
}

fn bench_sobol_bridge_price(c: &mut Criterion) {
    let mut group = c.benchmark_group("sobol_bridge_price");
    let cases = [
        ("arithmetic_uniform8_points4096", 8, 4_096, false, true),
        ("arithmetic_uniform32_points4096", 32, 4_096, false, true),
        ("arithmetic_uniform40_points4096", 40, 4_096, false, true),
        ("arithmetic_uniform32_points16384", 32, 16_384, false, true),
        ("arithmetic_irregular17_points4096", 17, 4_096, true, true),
        (
            "arithmetic_uniform32_points4096_no_bridge",
            32,
            4_096,
            false,
            false,
        ),
    ];
    for (id, steps, points, irregular, bridge) in cases {
        // Units are dependent Sobol points, not IID estimators. Each case uses
        // sixteen independent scrambles, with 256 or 1024 points per scramble.
        group.throughput(Throughput::Elements(points as u64));
        group.bench_function(id, |b| {
            let process = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.20).expect("valid GBM"));
            let payoff = Asian::new(
                OptionType::Call,
                100.0,
                1.0,
                AveragingMethod::Arithmetic,
                (1..=steps).collect(),
            )
            .expect("post-step Asian fixing schedule");
            let config = PathDependentPricerConfig {
                num_paths: points,
                seed: 42,
                use_parallel: false,
                chunk_size: 256,
                path_capture: PathCaptureConfig::new(),
                steps_per_year: 252.0,
                min_steps: 8,
                use_sobol: true,
                antithetic: false,
                use_brownian_bridge: bridge,
            };
            config
                .validate()
                .expect("supported frozen Sobol configuration");
            assert!(
                config
                    .engine_config(TimeGrid::uniform(1.0, steps).expect("grid"))
                    .target_ci_half_width
                    .is_none()
            );
            let pricer = PathDependentPricer::new(config);
            let price = || {
                // Keep construction of the consumed grid inside the full price
                // boundary, including any future grid-owned preparation.
                let grid = if irregular {
                    TimeGrid::from_times(
                        (0..=steps)
                            .map(|i| (i * i) as f64 / (steps * steps) as f64)
                            .collect(),
                    )
                    .expect("irregular grid")
                } else {
                    TimeGrid::uniform(1.0, steps).expect("uniform grid")
                };
                pricer
                    .price(
                        &process,
                        100.0,
                        grid,
                        &payoff,
                        Currency::USD,
                        (-0.05_f64).exp(),
                    )
                    .expect("supported Sobol Asian price")
            };
            let checked = price();
            check_estimate(&checked, points, points);
            assert!(checked.stderr > 0.0);
            drop(checked);
            b.iter(|| black_box(price()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_european_pricer,
    bench_lsmc_pricer,
    bench_lsq_regression,
    bench_heston_qe_pricer,
    bench_rough_heston_step,
    bench_hw1f_pricer,
    bench_sobol_bridge_price
);
criterion_main!(benches);
