//! Selected-case validity and scaling guards for model workloads.
//!
//! Each case constructs and checks its fixture only after Criterion selects it.
//! Throughput units are explicit below; nonlinear kernels are not assumed linear.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/validity.rs"]
mod validity;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::fractional::HurstExponent;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_models::closed_form::{bs_price, heston_call_price_fourier};
use finstack_quant_models::monte_carlo::discretization::rough_heston::RoughHestonHybrid;
use finstack_quant_models::monte_carlo::engine::McPricingInputs;
use finstack_quant_models::monte_carlo::estimate::Estimate;
use finstack_quant_models::monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::{
    AmericanExercise, LsmcConfig, LsmcEngine, LsmcInputs,
};
use finstack_quant_models::monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_models::monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_quant_models::monte_carlo::process::rough_heston::{
    RoughHestonParams, RoughHestonProcess,
};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::rng::fbm::FractionalNoiseGenerator;
use finstack_quant_models::monte_carlo::rng::volterra::RiemannLiouvilleVolterra;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::heston::HestonParams;
use fixtures::{
    DIV, RATE, SEED, SPOT, STRIKE, VOL, discount, european_call, exact_gbm, gbm, heston,
    lmm_process, lmm_scheme, multi_gbm, philox, qe_heston, serial_engine,
};
use std::hint::black_box;
use validity::{
    check_against_reference, check_estimate, check_lsq, check_repeat, check_rough_first_step,
    monthly_bermudan_put_reference,
};

fn call_reference() -> f64 {
    bs_price(SPOT, STRIKE, RATE, DIV, VOL, 1.0, OptionType::Call)
        .expect("Black-Scholes fixture reference")
}

fn scaling_european_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_european_paths");
    for &n in &[2_500_usize, 10_000, 40_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let process = gbm();
            let payoff = european_call(52);
            let df = discount();
            let pricer = EuropeanPricer::new(n)
                .expect("at least two estimators")
                .with_seed(SEED)
                .with_parallel(false);
            // Keep the consumed TimeGrid and pricer-internal preparation timed.
            let price = || {
                pricer
                    .price(
                        &process,
                        SPOT,
                        TimeGrid::uniform(1.0, 52).expect("valid time grid"),
                        &payoff,
                        Currency::USD,
                        df,
                    )
                    .expect("European pricing")
            };
            let first = price();
            check_estimate(&first, n, n);
            check_against_reference(&first, call_reference(), 1e-8);
            check_repeat(&first, &price());
            b.iter(price);
        });
    }
    group.finish();
}

fn scaling_european_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_european_steps");
    for &steps in &[52_usize, 126, 252] {
        // This unit is physical path-steps, not just grid intervals.
        group.throughput(Throughput::Elements((2_000 * steps) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, &steps| {
            let process = gbm();
            let payoff = european_call(steps);
            let df = discount();
            let pricer = EuropeanPricer::new(2_000)
                .expect("at least two estimators")
                .with_seed(SEED)
                .with_parallel(false);
            let price = || {
                pricer
                    .price(
                        &process,
                        SPOT,
                        TimeGrid::uniform(1.0, steps).expect("valid time grid"),
                        &payoff,
                        Currency::USD,
                        df,
                    )
                    .expect("European pricing")
            };
            let first = price();
            check_estimate(&first, 2_000, 2_000);
            check_against_reference(&first, call_reference(), 1e-8);
            check_repeat(&first, &price());
            b.iter(price);
        });
    }
    group.finish();
}

fn scaling_heston_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_heston_paths");
    for &n in &[1_000_usize, 4_000, 8_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let process = heston();
            let disc = qe_heston();
            let payoff = european_call(52);
            let rng = philox();
            let engine = serial_engine(n, 52);
            let inputs = McPricingInputs {
                rng: &rng,
                process: &process,
                disc: &disc,
                initial_state: &[SPOT, 0.04],
                payoff: &payoff,
                currency: Currency::USD,
                discount_factor: (-0.03_f64).exp(),
            };
            let first = engine.price(inputs).expect("Heston QE pricing");
            check_estimate(&first, n, n);
            let reference = heston_call_price_fourier(SPOT, STRIKE, 1.0, process.params(), None)
                .expect("converged Heston Fourier reference");
            // A 52-step QE discretization is approximate; this smoke envelope
            // retains sampling noise plus a fixed five-cent discretization allowance.
            check_against_reference(&first, reference, 0.05);
            check_repeat(&first, &engine.price(inputs).expect("repeat Heston price"));
            b.iter(|| engine.price(inputs).expect("Heston QE pricing"));
        });
    }
    group.finish();
}

fn scaling_lsmc_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lsmc_estimators");
    for &n in &[1_000_usize, 2_500, 5_000] {
        group.throughput(Throughput::Elements((2 * n) as u64));
        let id = format!("{n}_estimators_{}_physical_paths", 2 * n);
        group.bench_with_input(BenchmarkId::from_parameter(id), &n, |b, &n| {
            let process = gbm();
            let exercise = AmericanExercise::new(OptionType::Put, STRIKE).expect("valid strike");
            let basis = PolynomialBasis::new(2).expect("quadratic regression basis");
            let num_steps = 12;
            let config = LsmcConfig::new(n, (1..=num_steps).collect(), num_steps)
                .expect("monthly Bermudan exercise grid")
                .with_seed(SEED)
                .with_parallel(false)
                .with_antithetic(true);
            let pricer = LsmcEngine::new(config);
            let inputs = LsmcInputs {
                process: &process,
                initial_spot: SPOT,
                time_to_maturity: 1.0,
                num_steps,
                exercise: &exercise,
                basis: &basis,
                discount_rate: RATE,
            };
            assert_eq!(pricer.config().exercise_dates, (1..=12).collect::<Vec<_>>());
            let first = pricer.price(&inputs, Currency::USD).expect("LSMC pricing");
            check_estimate(&first, n, 2 * n);
            // In-sample policy fit and tree resolution are part of this fixed
            // smoke allowance, not a claim that LSMC has no regression bias.
            check_against_reference(&first, monthly_bermudan_put_reference(), 0.05);
            check_repeat(
                &first,
                &pricer
                    .price(&inputs, Currency::USD)
                    .expect("repeat LSMC price"),
            );
            b.iter(|| pricer.price(&inputs, Currency::USD).expect("LSMC pricing"));
        });
    }
    group.finish();
}

fn scaling_lsq_observations(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lsq_observations");
    for &n in &[100_usize, 500, 2_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let k = 3;
            let mut design = vec![0.0; n * k];
            let mut y = vec![0.0; n];
            for i in 0..n {
                let x = i as f64 / n as f64;
                design[i * k..(i + 1) * k].copy_from_slice(&[1.0, x, x * x]);
                y[i] = 1.0 + 2.0 * x + 3.0 * x * x;
            }
            let beta = solve_least_squares(&design, &y, n, k).expect("full-rank least squares");
            check_lsq(&beta, &[1.0, 2.0, 3.0], &design, &y);
            b.iter(|| {
                solve_least_squares(black_box(&design), black_box(&y), n, k)
                    .expect("full-rank least squares")
            });
        });
    }
    group.finish();
}

fn scaling_rough_heston_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rough_heston_steps");
    for &num_steps in &[50_usize, 100, 200] {
        let kernel_terms = num_steps * (num_steps + 1) / 2;
        group.throughput(Throughput::Elements(kernel_terms as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_steps),
            &num_steps,
            |b, &n| {
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
                let scheme = RoughHestonHybrid::new(&times, 0.1).expect("valid rough scheme");
                let z = [0.5_f64, -0.3, 0.2];
                let mut work = vec![0.0; scheme.work_size(&process)];
                let mut first_step = [SPOT, 0.04];
                scheme.step(&process, 0.0, dt, &mut first_step, &z, &mut work);
                check_rough_first_step(&first_step, dt, &z);
                let path = |work: &mut [f64]| {
                    let mut x = [SPOT, 0.04];
                    work.fill(0.0);
                    let mut t = 0.0;
                    for _ in 0..n {
                        scheme.step(&process, t, dt, &mut x, black_box(&z), work);
                        t += dt;
                    }
                    assert!(x[0].is_finite() && x[1].is_finite());
                    x
                };
                let first = path(&mut work);
                assert!(first[0] > 0.0 && first[1] >= 0.0);
                let repeated = path(&mut work);
                assert_eq!(first.map(f64::to_bits), repeated.map(f64::to_bits));
                assert_eq!(work[2 * n], n as f64);
                b.iter(|| black_box(path(&mut work)));
            },
        );
    }
    group.finish();
}

fn check_lmm_fixture(process: &LmmProcess, terminal: &[f64], steps: usize) {
    let params = process.params();
    let n = params.num_forwards;
    assert_eq!(terminal.len(), n);
    assert!(terminal.iter().all(|value| value.is_finite()));
    assert!(
        terminal
            .iter()
            .zip(&params.displacements)
            .all(|(f, d)| f + d > 0.0)
    );
    assert_eq!(terminal[0].to_bits(), 0.03_f64.to_bits());
    assert_ne!(terminal[1].to_bits(), 0.03_f64.to_bits());
    // Analytic terminal-forward path: under terminal measure its drift is zero.
    let loading = params.vol_values[0][n - 1];
    let norm_sq = loading[0] * loading[0] + loading[1] * loading[1];
    let shock = loading[0] * 0.4 + loading[1] * -0.2;
    let dt = 2.0 / steps as f64;
    let expected = 0.035 * (steps as f64 * (-0.5 * norm_sq * dt + shock * dt.sqrt())).exp() - 0.005;
    assert!((terminal[n - 1] - expected).abs() < 1e-12);
    // At the homogeneous initial state, the nonterminal drift also has an
    // independent finite sum over the flat factor-loading coefficients.
    let mut drift = vec![0.0; n];
    process.drift(0.0, &vec![0.03; n], &mut drift);
    assert_eq!(drift[0], 0.0);
    assert_eq!(drift[n - 1], 0.0);
    for i in 1..n - 1 {
        let lam = params.vol_values[0][i];
        let later_loading: [f64; 2] = std::array::from_fn(|factor| {
            params.vol_values[0][i + 1..]
                .iter()
                .map(|row| row[factor])
                .sum()
        });
        let expected =
            -0.035 * 0.035 / 1.03 * (lam[0] * later_loading[0] + lam[1] * later_loading[1]);
        assert!((drift[i] - expected).abs() < 1e-15);
    }
    assert!(drift[1] < 0.0);
}

fn bench_lmm_case(b: &mut criterion::Bencher<'_>, forwards: usize, steps: usize) {
    let process = lmm_process(forwards);
    let scheme = lmm_scheme();
    let grid = TimeGrid::uniform(2.0, steps).expect("two-year LMM grid");
    process
        .validate_time_grid(&grid)
        .expect("all annual fixing knots represented");
    let mut work = vec![0.0; scheme.work_size(&process)];
    let z = [0.4_f64, -0.2];
    let initial = vec![0.03; forwards];
    let mut path = || {
        // Retain the initial-state clone and work reset in the measured boundary.
        let mut x = initial.clone();
        work.fill(0.0);
        for step in 0..steps {
            scheme.step(
                &process,
                grid.time(step),
                grid.dt(step),
                &mut x,
                black_box(&z),
                &mut work,
            );
        }
        assert!(x.iter().all(|value| value.is_finite()));
        x
    };
    let first = path();
    check_lmm_fixture(&process, &first, steps);
    let repeated = path();
    assert!(
        first
            .iter()
            .zip(&repeated)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    b.iter(|| black_box(path()));
}

fn scaling_lmm_forwards(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lmm_forwards_fixed_2y_8_steps");
    for &n in &[10_usize, 20, 40, 80] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            bench_lmm_case(b, n, 8)
        });
    }
    group.finish();
}

fn scaling_lmm_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lmm_steps_fixed_2y_20_forwards");
    for &steps in &[8_usize, 16, 32] {
        group.throughput(Throughput::Elements(steps as u64));
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, &steps| {
            bench_lmm_case(b, 20, steps);
        });
    }
    group.finish();
}

fn scaling_lmm_controls(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_lmm_controls_fixed_2y_8_steps");
    for &(id, forwards, factors) in &[
        ("factors3_forwards40", 40_usize, 3_usize),
        ("loading_knots_forwards20", 20, 2),
    ] {
        group.throughput(Throughput::Elements(forwards as u64));
        group.bench_function(id, |b| {
            let mut params = lmm_process(forwards).params().clone();
            params.num_factors = factors;
            if factors == 3 {
                for loading in &mut params.vol_values[0] {
                    loading[2] = 0.02;
                }
            } else {
                let base = params.vol_values[0].clone();
                params.vol_times = vec![0.75, 1.25];
                params.vol_values = vec![
                    base.clone(),
                    base.iter().map(|row| [1.2 * row[0], 0.03, 0.0]).collect(),
                    base.iter().map(|row| [0.8 * row[0], -0.04, 0.0]).collect(),
                ];
            }
            params.validate().expect("supported LMM control");
            let process = LmmProcess::new(params);
            let scheme = lmm_scheme();
            let grid = TimeGrid::uniform(2.0, 8).expect("two-year LMM grid");
            process
                .validate_time_grid(&grid)
                .expect("exact model knots");
            let z = [0.4_f64, -0.2, 0.1];
            let mut work = vec![0.0; scheme.work_size(&process)];
            let path = |work: &mut [f64]| {
                let mut state = process.params().initial_forwards.clone();
                work.fill(0.0);
                for step in 0..grid.num_steps() {
                    scheme.step(
                        &process,
                        grid.time(step),
                        grid.dt(step),
                        &mut state,
                        black_box(&z[..factors]),
                        work,
                    );
                }
                assert!(state.iter().all(|value| value.is_finite()));
                state
            };
            let checked = path(&mut work);
            assert_eq!(checked.len(), forwards);
            assert_eq!(checked[0].to_bits(), 0.03_f64.to_bits());
            assert_ne!(checked[1].to_bits(), 0.03_f64.to_bits());
            assert!(checked.iter().all(|forward| forward + 0.005 > 0.0));
            // The terminal forward remains driftless even with three factors
            // or loading changes. Integrate its exact deterministic log path.
            let mut log_growth = 0.0;
            for step in 0..grid.num_steps() {
                let period = process
                    .params()
                    .vol_times
                    .partition_point(|&t| t <= grid.time(step));
                let loading = process.params().vol_values[period][forwards - 1];
                let variance: f64 = loading[..factors].iter().map(|value| value * value).sum();
                let shock: f64 = loading[..factors].iter().zip(&z).map(|(a, b)| a * b).sum();
                log_growth += -0.5 * variance * grid.dt(step) + shock * grid.dt(step).sqrt();
            }
            let expected = 0.035 * log_growth.exp() - 0.005;
            assert!((checked[forwards - 1] - expected).abs() < 1e-12);
            let repeated = path(&mut work);
            assert!(
                checked
                    .iter()
                    .zip(&repeated)
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            b.iter(|| black_box(path(&mut work)));
        });
    }
    group.finish();
}

fn price_lmm_caplet(params: &LmmParams, grid: &TimeGrid) -> MoneyEstimate {
    params.validate().expect("valid LMM caplet model");
    let process = LmmProcess::new(params.clone());
    process
        .validate_time_grid(grid)
        .expect("valid caplet simulation grid");
    let scheme = lmm_scheme();
    let root_rng = philox();
    let mut state = params.initial_forwards.clone();
    let mut work = vec![0.0; scheme.work_size(&process)];
    let mut z = vec![0.0; params.num_factors];
    let discount_terminal: f64 = params
        .initial_forwards
        .iter()
        .zip(&params.accrual_factors)
        .map(|(forward, accrual)| (1.0 + accrual * forward).recip())
        .product();
    let mut stats = OnlineStats::new();
    for path_id in 0..2_048 {
        let mut rng = root_rng.substream(path_id);
        state.copy_from_slice(&params.initial_forwards);
        work.fill(0.0);
        for step in 0..grid.num_steps() {
            rng.fill_std_normals(&mut z);
            scheme.step(
                &process,
                grid.time(step),
                grid.dt(step),
                &mut state,
                &z,
                &mut work,
            );
        }
        // Fix F2 at T2=2 and pay the annual caplet at T3=3. D3 is
        // P(T2,T3)/P(T2,T40), so this is a terminal-measure estimand.
        let bond_ratio: f64 = state[3..]
            .iter()
            .zip(&params.accrual_factors[3..])
            .map(|(forward, accrual)| 1.0 + accrual * forward)
            .product();
        let value = discount_terminal
            * 1_000_000.0
            * params.accrual_factors[2]
            * (state[2] - 0.03).max(0.0)
            * bond_ratio;
        assert!(value.is_finite() && value >= 0.0);
        stats.update(value);
    }
    MoneyEstimate::from_estimate(Estimate::from_stats(&stats), Currency::USD)
        .expect("finite caplet estimate")
}

fn bench_lmm_caplet(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmm_caplet_terminal_measure");
    group.throughput(Throughput::Elements(2_048));
    group.bench_function("forwards40_estimators2048_steps8", |b| {
        let params = lmm_process(40).params().clone();
        let grid = TimeGrid::uniform(2.0, 8).expect("two-year caplet grid");
        assert_eq!(params.tenors[2], grid.t_max());
        assert_eq!(params.tenors[3], 3.0);
        let checked = price_lmm_caplet(&params, &grid);
        check_estimate(&checked, 2_048, 2_048);
        let loading = params.vol_values[0][2];
        let volatility = loading
            .iter()
            .take(params.num_factors)
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let discount_payment: f64 = params.initial_forwards[..3]
            .iter()
            .zip(&params.accrual_factors[..3])
            .map(|(forward, accrual)| (1.0 + accrual * forward).recip())
            .product();
        let reference = 1_000_000.0
            * params.accrual_factors[2]
            * discount_payment
            * bs_price(0.035, 0.035, 0.0, 0.0, volatility, 2.0, OptionType::Call)
                .expect("displaced-Black caplet reference");
        // The nonterminal predictor/corrector discretization is approximate.
        // This smoke check is distinct from the tighter original-drift panel.
        check_against_reference(&checked, reference, 5.0);
        drop(checked);
        b.iter(|| price_lmm_caplet(black_box(&params), black_box(&grid)));
    });
    group.finish();
}

fn scaling_multi_gbm_assets(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_multi_gbm_assets");
    for &n in &[2_usize, 5, 10] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let (process, disc, spots) = multi_gbm(n);
            let payoff = european_call(52);
            let engine = serial_engine(2_000, 52);
            let rng = philox();
            let inputs = McPricingInputs {
                rng: &rng,
                process: &process,
                disc: &disc,
                initial_state: &spots,
                payoff: &payoff,
                currency: Currency::USD,
                discount_factor: discount(),
            };
            let first = engine.price(inputs).expect("multi-GBM pricing");
            check_estimate(&first, 2_000, 2_000);
            // European reads the first marginal, not a basket. Correlating other
            // assets must not change that marginal's Black-Scholes value.
            check_against_reference(&first, call_reference(), 1e-8);
            check_repeat(
                &first,
                &engine.price(inputs).expect("repeat multi-GBM price"),
            );
            b.iter(|| engine.price(inputs).expect("multi-GBM pricing"));
        });
    }
    group.finish();
}

fn check_volterra_reference(normals: &[f64], out: &[f64]) {
    let n = out.len();
    let h = 0.1_f64;
    let dt = 1.0 / n as f64;
    let alpha = h + 0.5;
    let covariance = dt.powf(alpha) / alpha;
    let l10 = covariance / dt.sqrt();
    let l11 = (dt.powf(2.0 * h) / (2.0 * h) - l10 * l10).sqrt();
    let mut previous = 0.0;
    for k in 1..=n {
        let near = l10 * normals[2 * (k - 1)] + l11 * normals[2 * (k - 1) + 1];
        let mut far = 0.0;
        for j in 0..k - 1 {
            let lag = (k - j) as f64;
            let weight = ((lag * dt).powf(alpha) - ((lag - 1.0) * dt).powf(alpha)) / (alpha * dt);
            far += weight * dt.sqrt() * normals[2 * j];
        }
        let level = (2.0 * h).sqrt() * (near + far);
        assert!((out[k - 1] - (level - previous)).abs() < 1e-11);
        previous = level;
    }
}

fn scaling_volterra_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_volterra_steps");
    for &n in &[64_usize, 128, 256] {
        group.throughput(Throughput::Elements((n * (n + 1) / 2) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let generator = RiemannLiouvilleVolterra::new(1.0, n, 0.1).expect("valid Volterra");
            assert_eq!(generator.num_steps(), n);
            assert_eq!(generator.normals_per_step(), 2);
            // Each selected size owns a fresh seed, independent of registration
            // order/filtering. RNG generation remains outside the measured call.
            let mut rng = philox();
            let mut normals = vec![0.0; 2 * n];
            rng.fill_std_normals(&mut normals);
            let mut out = vec![0.0; n];
            generator.generate(&normals, &mut out);
            assert!(out.iter().all(|value| value.is_finite()));
            assert!(out.iter().skip(1).any(|value| *value != 0.0));
            check_volterra_reference(&normals, &out);
            let mut repeated = vec![f64::NAN; n];
            generator.generate(&normals, &mut repeated);
            assert!(
                out.iter()
                    .zip(&repeated)
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            b.iter(|| {
                generator.generate(black_box(&normals), &mut out);
                assert!(out[n - 1].is_finite());
                black_box(&out);
            });
        });
    }
    group.finish();
}

fn scaling_exact_gbm_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_exact_gbm_steps");
    for &n in &[52_usize, 126, 252, 504] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let process = gbm();
            let scheme = exact_gbm();
            let z = [0.5_f64];
            let dt = 1.0 / n as f64;
            let path = || {
                let mut x = [SPOT];
                let mut t = 0.0;
                for _ in 0..n {
                    scheme.step(&process, t, dt, &mut x, black_box(&z), &mut []);
                    t += dt;
                }
                assert!(x[0].is_finite());
                x
            };
            let expected =
                SPOT * (RATE - DIV - 0.5 * VOL * VOL + VOL * dt.sqrt() * z[0] * n as f64).exp();
            let first = path();
            assert!((first[0] - expected).abs() < 1e-11 * expected);
            assert_eq!(first.map(f64::to_bits), path().map(f64::to_bits));
            b.iter(|| black_box(path()));
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
    scaling_lmm_steps,
    scaling_lmm_controls,
    bench_lmm_caplet,
    scaling_multi_gbm_assets,
    scaling_volterra_steps,
    scaling_exact_gbm_steps
);
criterion_main!(benches);
