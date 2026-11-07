//! Monte Carlo pricing benchmarks (public API).
//!
//! Benchmarks LSMC Bermudan swaption pricing via the public pricer API.

use finstack_quant_models::types::OptionType;

use criterion::{Bencher, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::process::ou::calibrate_theta_from_curve;
use finstack_quant_models::rates::clock::ModelDiscountCurve;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_models::rates::hw1f::{Hw1fMcConfig, build_event_aligned_grid};
use finstack_quant_models::trees::hull_white_tree::{HullWhiteTree, HullWhiteTreeConfig};
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
    PreparedHullWhiteModel,
};
use finstack_quant_valuations::pricer::{ModelKey, Pricer};
use std::hint::black_box;
use time::Month;

#[path = "support/record.rs"]
mod record;
mod support;

fn assert_usd_value(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn build_swaption(as_of: Date) -> BermudanSwaption {
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let mut swaption = BermudanSwaption::new(
        finstack_quant_valuations::instruments::rates::swaption::BermudanSwaptionParams {
            id: "BERM-LSMC-BENCH".into(),
            option_type: OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            strike: 0.03,
            underlying_start_date: swap_start,
            underlying_maturity: swap_end,
            exercise_schedule: BermudanSchedule::co_terminal(
                first_exercise,
                swap_end,
                Tenor::semi_annual(),
            )
            .expect("valid Bermudan schedule"),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            vol_surface_id: "USD-VOL".into(),
        },
    )
    .expect("valid Bermudan swaption");
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.01);
    swaption
}

fn build_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve");

    MarketContext::new().insert(curve)
}

fn capture_result(
    case: &str,
    swaption: &BermudanSwaption,
    market: &MarketContext,
    as_of: Date,
    config: &BermudanSwaptionPricerConfig,
    value: &Money,
    pricer: &BermudanSwaptionPricer,
) {
    record::capture(case, || {
        // Full diagnostics are capture-only: normal preflight and timed calls
        // retain their original Money extraction and drop boundary.
        let result = pricer
            .price_dyn(swaption, market, as_of)
            .expect("capture pricing diagnostics");
        assert_eq!(
            result.value, *value,
            "capture must replay the preflight value"
        );
        let model = pricer.key().model;
        let is_mc = model == ModelKey::MonteCarloHullWhite1F;
        if is_mc {
            for metric in [
                "mc_stderr",
                "lsmc_num_paths",
                "lsmc_seed",
                "lsmc_ci95_low",
                "lsmc_ci95_high",
            ] {
                assert!(
                    result
                        .measures
                        .get(metric)
                        .expect("LSMC diagnostic")
                        .is_finite()
                );
            }
            let expected_estimators = if config.mc.oos_lsmc {
                config.mc.num_paths / 2
            } else {
                config.mc.num_paths
            };
            assert_eq!(
                result.measures["lsmc_num_paths"],
                expected_estimators as f64
            );
            assert_eq!(result.measures["lsmc_seed"], config.mc.seed as f64);
            assert!(result.measures["mc_stderr"] >= 0.0);
            assert!(result.measures["lsmc_ci95_low"] <= result.value.amount());
            assert!(result.measures["lsmc_ci95_high"] >= result.value.amount());
        }
        let horizon = swaption.time_to_maturity(as_of).expect("capture horizon");
        let exercise_times = swaption
            .exercise_times(as_of)
            .expect("capture exercise times");
        let overrides = &swaption.instrument_pricing_overrides.model_config;
        let parameters = HullWhiteCalibrationParams::new(
            overrides
                .hw1f_mean_reversion
                .expect("fixture mean reversion"),
            overrides.hw1f_sigma.expect("fixture short-rate volatility"),
        )
        .expect("capture Hull-White parameters");
        let times = if model == ModelKey::MonteCarloHullWhite1F {
            let grid = match config.mc_base_steps {
                Some(base_steps) => TimeGrid::uniform_with_required_times(
                    horizon,
                    base_steps as f64 / horizon,
                    base_steps,
                    &exercise_times,
                )
                .expect("capture base grid"),
                None => {
                    build_event_aligned_grid(
                        &exercise_times,
                        horizon,
                        config.mc.min_steps_between_events,
                    )
                    .expect("capture event grid")
                    .0
                }
            };
            grid.times().to_vec()
        } else {
            let curve = market
                .get_discount("USD-OIS")
                .expect("capture discount curve");
            let model_curve = ModelDiscountCurve::new(curve.as_ref(), as_of)
                .expect("capture model discount clock");
            HullWhiteTree::calibrate(
                HullWhiteTreeConfig::new(parameters.kappa, parameters.sigma, config.tree_steps),
                &model_curve,
                horizon,
                &exercise_times,
            )
            .expect("capture calibrated tree")
            .time_grid()
            .to_vec()
        };
        let exercise_indices: Vec<_> = exercise_times
            .iter()
            .map(|event| {
                times
                    .iter()
                    .position(|node| (node - event).abs() < 1e-10)
                    .expect("capture contractual exercise node")
            })
            .collect();
        let theta = is_mc.then(|| {
            let curve = market.get_discount("USD-OIS").expect("capture theta curve");
            let model_curve = ModelDiscountCurve::new(curve.as_ref(), as_of)
                .expect("capture theta discount clock");
            let discount_fn = |time| model_curve.get_df(time).unwrap_or(f64::NAN);
            let calibrated = calibrate_theta_from_curve(
                parameters.kappa,
                parameters.sigma,
                discount_fn,
                &times,
            )
            .expect("capture fitted theta schedule");
            serde_json::json!({
                "knot_count": calibrated.theta_times().len(),
                "time_bits": calibrated.theta_times().iter().map(|time| time.to_bits()).collect::<Vec<_>>(),
                "value_bits": calibrated.theta_values().iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
            })
        });
        serde_json::json!({
            "fixture": {
                "instrument": swaption,
                "market": market,
                "as_of": as_of,
                "model": model,
                "model_parameters": parameters,
                "config": {
                    "tree_steps": config.tree_steps,
                    "mc": config.mc,
                    "mc_base_steps": config.mc_base_steps,
                    "prepared_model": config.prepared_model.is_some(),
                },
                "prepared_model_recipe": config.prepared_model.as_ref().map(|_| serde_json::json!({
                    "parameters": parameters,
                    "steps": config.tree_steps,
                    "discount_curve_id": "USD-OIS",
                    "as_of": as_of,
                    "horizon": horizon,
                    "mandatory_times": exercise_times,
                })),
                "seed": is_mc.then_some(config.mc.seed),
                "independent_estimators": is_mc.then_some(if config.mc.oos_lsmc {
                    config.mc.num_paths / 2
                } else {
                    config.mc.num_paths
                }),
                "configured_streams": is_mc.then_some(config.mc.num_paths),
                "physical_paths": is_mc.then_some(config.mc.simulated_path_count()),
                "base_steps": if is_mc { config.mc_base_steps } else { Some(config.tree_steps) },
                "actual_steps": times.len() - 1,
                "grid_time_bits": times.iter().map(|time| time.to_bits()).collect::<Vec<_>>(),
                "theta": theta,
                "exercise_times": exercise_times,
                "exercise_indices": exercise_indices,
            },
            "output": {
                "value": result.value,
                "value_bits": result.value.amount().to_bits(),
                "measures": result.measures,
            },
        })
    });
}

fn bench_lsmc_case(b: &mut Bencher<'_>, case: &str, num_paths: usize, base_steps: Option<usize>) {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swaption = build_swaption(as_of);
    let market = build_market(as_of);
    let config = BermudanSwaptionPricerConfig {
        mc: Hw1fMcConfig {
            num_paths,
            seed: 42,
            ..BermudanSwaptionPricerConfig::DEFAULT_MC
        },
        mc_base_steps: base_steps,
        ..Default::default()
    };
    let pricer = BermudanSwaptionPricer::lsmc_with_config(config.clone());
    assert_eq!(pricer.key().model, ModelKey::MonteCarloHullWhite1F);
    support::bench(
        b,
        || {
            pricer
                .price_dyn(black_box(&swaption), black_box(&market), as_of)
                .expect("lsmc price")
                .value
        },
        |value| {
            assert_usd_value(value);
            capture_result(case, &swaption, &market, as_of, &config, value, &pricer);
        },
    );
}

fn bench_bermudan_lsmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("mc_bermudan_lsmc");
    let num_paths = 50_000;
    group.throughput(Throughput::Elements(num_paths as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter(num_paths),
        &num_paths,
        |b, &n| {
            bench_lsmc_case(b, "mc_bermudan_lsmc/50000", n, None);
        },
    );
    group.finish();
}

fn bench_bermudan_lsmc_axes(c: &mut Criterion) {
    let mut group = c.benchmark_group("mc_bermudan_lsmc_axes");
    for (num_paths, base_steps) in [
        (5_000, 128),
        (20_000, 128),
        (50_000, 128),
        (20_000, 64),
        (20_000, 256),
    ] {
        let suffix = format!("paths_{num_paths}/base_{base_steps}");
        let case = format!("mc_bermudan_lsmc_axes/{suffix}");
        group.throughput(Throughput::Elements(num_paths as u64));
        group.bench_function(suffix, |b| {
            bench_lsmc_case(b, &case, num_paths, Some(base_steps));
        });
    }
    group.finish();
}

fn bench_bermudan_hw_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("mc_bermudan_hw_tree");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    group.bench_function("calibrate_each_price", |b| {
        let swaption = build_swaption(as_of);
        let market = build_market(as_of);
        let config = BermudanSwaptionPricerConfig {
            tree_steps: 100,
            ..Default::default()
        };
        let calibrate_each = BermudanSwaptionPricer::tree_with_config(config.clone());
        assert_eq!(calibrate_each.key().model, ModelKey::HullWhite1F);
        support::bench(
            b,
            || {
                calibrate_each
                    .price_dyn(black_box(&swaption), black_box(&market), as_of)
                    .expect("hw tree price")
                    .value
            },
            |value| {
                assert_usd_value(value);
                capture_result(
                    "mc_bermudan_hw_tree/calibrate_each_price",
                    &swaption,
                    &market,
                    as_of,
                    &config,
                    value,
                    &calibrate_each,
                );
            },
        );
    });

    group.bench_function("pre_calibrated_100_steps", |b| {
        let swaption = build_swaption(as_of);
        let market = build_market(as_of);
        let hw_params = HullWhiteCalibrationParams::default();
        let ttm = swaption.time_to_maturity(as_of).expect("ttm");
        let disc = market.get_discount("USD-OIS").expect("USD-OIS");
        let cached = PreparedHullWhiteModel::prepare(
            hw_params,
            100,
            disc.as_ref(),
            as_of,
            ttm,
            &swaption.exercise_times(as_of).expect("exercise times"),
        )
        .expect("pre-calibrated HW tree");
        let config = BermudanSwaptionPricerConfig {
            tree_steps: 100,
            prepared_model: Some(cached),
            ..Default::default()
        };
        let reused = BermudanSwaptionPricer::tree_with_config(config.clone());
        assert_eq!(reused.key().model, ModelKey::HullWhite1F);
        support::bench(
            b,
            || {
                reused
                    .price_dyn(black_box(&swaption), black_box(&market), as_of)
                    .expect("cached hw tree price")
                    .value
            },
            |value| {
                assert_usd_value(value);
                capture_result(
                    "mc_bermudan_hw_tree/pre_calibrated_100_steps",
                    &swaption,
                    &market,
                    as_of,
                    &config,
                    value,
                    &reused,
                );
            },
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_bermudan_hw_tree,
    bench_bermudan_lsmc,
    bench_bermudan_lsmc_axes
);
criterion_main!(benches);
