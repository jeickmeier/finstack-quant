//! Independent tree check for the frozen Hull–White performance fixture.
//!
//! This ignored panel is separate from benchmark timing and peak-memory runs.
//! Its in-sample LSMC intervals describe sampling uncertainty conditional on
//! the fitted exercise policy; they do not cover exercise-regression bias.
//! The stricter existing out-of-sample acceptance fixture remains separate.

use super::*;
use crate::swaption::{BermudanSchedule, BermudanSwaptionParams};
use finstack_quant_core::canonical::{CANONICAL_VERSION, content_hash};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_models::types::OptionType;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use time::macros::date;

const TREE_REFINEMENT_NOTIONAL_FRACTION: f64 = 1e-5;
const MC_STDERR_MULTIPLIER: f64 = 4.0;
const MC_TREE_VALUE_FRACTION: f64 = 0.03;
const ACCURACY_SEEDS: [u64; 4] = [17, 29, 42, 12_345];

#[derive(Clone, Copy)]
struct Case {
    id: &'static str,
    estimators: usize,
    base_steps: Option<usize>,
}

const CASES: [Case; 6] = [
    Case {
        id: "mc_bermudan_lsmc/50000",
        estimators: 50_000,
        base_steps: None,
    },
    Case {
        id: "mc_bermudan_lsmc_axes/paths_5000/base_128",
        estimators: 5_000,
        base_steps: Some(128),
    },
    Case {
        id: "mc_bermudan_lsmc_axes/paths_20000/base_128",
        estimators: 20_000,
        base_steps: Some(128),
    },
    Case {
        id: "mc_bermudan_lsmc_axes/paths_50000/base_128",
        estimators: 50_000,
        base_steps: Some(128),
    },
    Case {
        id: "mc_bermudan_lsmc_axes/paths_20000/base_64",
        estimators: 20_000,
        base_steps: Some(64),
    },
    Case {
        id: "mc_bermudan_lsmc_axes/paths_20000/base_256",
        estimators: 20_000,
        base_steps: Some(256),
    },
];

struct Fixture {
    as_of: Date,
    swaption: BermudanSwaption,
    market: MarketContext,
}

struct TreeReference {
    value: f64,
    record_hash: String,
}

/// Match `benches/mc_pricing.rs` without sharing test work with its callbacks.
fn fixture() -> Fixture {
    let as_of = date!(2025 - 01 - 01);
    let swap_end = date!(2030 - 01 - 01);
    let first_exercise = date!(2026 - 01 - 01);
    let mut swaption = BermudanSwaption::new(BermudanSwaptionParams {
        id: "BERM-LSMC-BENCH".into(),
        option_type: OptionType::Call,
        notional: Money::new(10_000_000.0, Currency::USD).expect("fixture notional"),
        strike: 0.03,
        underlying_start_date: as_of,
        underlying_maturity: swap_end,
        exercise_schedule: BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            Tenor::semi_annual(),
        )
        .expect("fixture exercise schedule"),
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-OIS".into(),
        vol_surface_id: "USD-VOL".into(),
    })
    .expect("fixture swaption");
    let overrides = &mut swaption.instrument_pricing_overrides.model_config;
    overrides.hw1f_mean_reversion = Some(0.03);
    overrides.hw1f_sigma = Some(0.01);
    assert_eq!(overrides.mc_paths, None);
    assert_eq!(overrides.tree_steps, None);
    assert_eq!(
        swaption.exercise_schedule.effective_dates(),
        [
            date!(2026 - 01 - 01),
            date!(2026 - 07 - 01),
            date!(2027 - 01 - 01),
            date!(2027 - 07 - 01),
            date!(2028 - 01 - 03),
            date!(2028 - 07 - 03),
            date!(2029 - 01 - 01),
            date!(2029 - 07 - 02),
        ]
    );
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
        .expect("fixture curve");
    Fixture {
        as_of,
        swaption,
        market: MarketContext::new().insert(curve),
    }
}

fn configuration_record(config: &BermudanSwaptionPricerConfig) -> Value {
    assert!(config.prepared_model.is_none());
    json!({
        "tree_steps": config.tree_steps,
        "mc": config.mc,
        "mc_base_steps": config.mc_base_steps,
        "prepared_model": false,
    })
}

fn grid_record(times: &[f64], exercise_times: &[f64], indices: &[usize]) -> Value {
    assert!(times.len() > 1);
    assert_eq!(times[0], 0.0);
    assert!(times.iter().all(|time| time.is_finite()));
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(exercise_times.len(), indices.len());
    for (&event, &index) in exercise_times.iter().zip(indices) {
        assert!((times[index] - event).abs() < 1e-10);
    }
    let time_bits: Vec<_> = times.iter().map(|time| time.to_bits()).collect();
    json!({
        "clock": finstack_quant_core::dates::DayCount::Act365F,
        "horizon": times[times.len() - 1],
        "actual_steps": times.len() - 1,
        "time_bits_hash": content_hash(&time_bits).expect("grid hash"),
        "time_bits": time_bits,
        "exercise_times": exercise_times,
        "exercise_indices": indices,
    })
}

fn output_record(result: &ValuationResult) -> Value {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite());
    assert!(result.measures.values().all(|value| value.is_finite()));
    let measure_bits: BTreeMap<_, _> = result
        .measures
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_bits()))
        .collect();
    json!({
        "value": result.value,
        "value_bits": result.value.amount().to_bits(),
        "measures": result.measures,
        "measure_bits": measure_bits,
    })
}

fn tree_reference(fixture: &Fixture, fixture_record: &Value) -> TreeReference {
    let swaption = &fixture.swaption;
    let horizon = swaption.time_to_maturity(fixture.as_of).expect("horizon");
    let exercise_times = swaption
        .exercise_times(fixture.as_of)
        .expect("exercise times");
    let curve = fixture
        .market
        .get_discount("USD-OIS")
        .expect("discount curve");
    let mut values = Vec::new();
    let mut trees = Vec::new();
    for tree_steps in [400, 800] {
        let config = BermudanSwaptionPricerConfig {
            tree_steps,
            ..Default::default()
        };
        let pricer = BermudanSwaptionPricer::tree_with_config(config.clone());
        let result = pricer
            .price_dyn(swaption, &fixture.market, fixture.as_of)
            .expect("matched tree reference price");
        assert!(result.value.amount() > 0.0);
        let parameters = pricer
            .effective_hw_params(swaption, &fixture.market)
            .expect("tree parameters");
        // Diagnostic reconstruction only. The price above uses its ordinary
        // calibrate-each-price route, as does the established reference test.
        let prepared = PreparedHullWhiteModel::prepare(
            parameters,
            tree_steps,
            curve.as_ref(),
            fixture.as_of,
            horizon,
            &exercise_times,
        )
        .expect("tree diagnostic grid");
        let times = prepared.tree().time_grid();
        let exercise_indices: Vec<_> = exercise_times
            .iter()
            .map(|event| {
                times
                    .iter()
                    .position(|node| (node - event).abs() < 1e-10)
                    .expect("tree exercise node")
            })
            .collect();
        let config_record = configuration_record(&config);
        values.push(result.value.amount());
        trees.push(json!({
            "base_steps": tree_steps,
            "config_hash": content_hash(&config_record).expect("tree config hash"),
            "config": config_record,
            "model_parameters": parameters,
            "grid": grid_record(times, &exercise_times, &exercise_indices),
            "output": output_record(&result),
        }));
    }
    let refinement_difference = (values[1] - values[0]).abs();
    let refinement_bound = swaption.notional.amount() * TREE_REFINEMENT_NOTIONAL_FRACTION;
    let record = json!({
        "case": "hw_performance_tree_reference/400_800",
        "kind": "tree_reference",
        "canonical_version": CANONICAL_VERSION,
        "fixture_hash": content_hash(fixture_record).expect("fixture hash"),
        "fixture": fixture_record,
        "trees": trees,
        "reference_base_steps": 800,
        "reference_pv": values[1],
        "reference_pv_bits": values[1].to_bits(),
        "refinement_difference_usd": refinement_difference,
        "strict_refinement_bound_usd": refinement_bound,
    });
    let record_hash = content_hash(&record).expect("reference record hash");
    println!("FQ_HW_DIAGNOSTIC {record}");
    assert!(
        refinement_difference < refinement_bound,
        "matched 400/800 tree refinement {values:?}: difference {refinement_difference} must be below {refinement_bound} USD"
    );
    TreeReference {
        value: values[1],
        record_hash,
    }
}

fn mc_diagnostic(
    fixture: &Fixture,
    fixture_record: &Value,
    reference: &TreeReference,
    case: Case,
    seed: u64,
) {
    let config = BermudanSwaptionPricerConfig {
        mc: Hw1fMcConfig {
            num_paths: case.estimators,
            seed,
            ..BermudanSwaptionPricerConfig::DEFAULT_MC
        },
        mc_base_steps: case.base_steps,
        ..Default::default()
    };
    assert!(config.mc.antithetic);
    assert!(!config.mc.oos_lsmc);
    assert_eq!(config.mc.basis_degree, 3);
    assert_eq!(config.mc.min_steps_between_events, 2);
    let swaption = &fixture.swaption;
    let horizon = swaption.time_to_maturity(fixture.as_of).expect("horizon");
    let exercise_times = swaption
        .exercise_times(fixture.as_of)
        .expect("exercise times");
    let (grid, exercise_indices) = config
        .lsmc_grid(&exercise_times, horizon)
        .expect("production MC grid");
    let pricer = BermudanSwaptionPricer::lsmc_with_config(config.clone());
    let parameters = pricer
        .effective_hw_params(swaption, &fixture.market)
        .expect("MC parameters");
    let curve = fixture
        .market
        .get_discount("USD-OIS")
        .expect("discount curve");
    let model_curve =
        ModelDiscountCurve::new(curve.as_ref(), fixture.as_of).expect("dated model discount curve");
    let discount_fn = |time| model_curve.get_df(time).unwrap_or(f64::NAN);
    let theta_times: Vec<_> = grid
        .times()
        .iter()
        .copied()
        .filter(|&time| time <= horizon)
        .collect();
    let calibrated = calibrate_theta_from_curve(
        parameters.kappa,
        parameters.sigma,
        discount_fn,
        &theta_times,
    )
    .expect("production theta preparation");
    let theta = json!({
        "knot_count": calibrated.theta_times().len(),
        "time_bits": calibrated.theta_times().iter().map(|time| time.to_bits()).collect::<Vec<_>>(),
        "value_bits": calibrated.theta_values().iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
    });
    let result = pricer
        .price_dyn(swaption, &fixture.market, fixture.as_of)
        .expect("performance fixture LSMC price");
    let stderr = result.measures["mc_stderr"];
    let ci_low = result.measures["lsmc_ci95_low"];
    let ci_high = result.measures["lsmc_ci95_high"];
    assert!(stderr >= 0.0);
    assert_eq!(result.measures["lsmc_num_paths"], case.estimators as f64);
    assert_eq!(result.measures["lsmc_seed"], seed as f64);
    assert!(ci_low <= result.value.amount());
    assert!(ci_high >= result.value.amount());
    let absolute_error = (result.value.amount() - reference.value).abs();
    let bound = MC_STDERR_MULTIPLIER * stderr + MC_TREE_VALUE_FRACTION * reference.value;
    let config_record = configuration_record(&config);
    let in_seed_panel = case.estimators == 50_000 && case.base_steps == Some(128);
    let record = json!({
        "case": case.id,
        "kind": "mc_accuracy",
        "canonical_version": CANONICAL_VERSION,
        "panel_membership": {
            "benchmark_seed_42": seed == 42,
            "accuracy_seed_panel": in_seed_panel,
        },
        "fixture_hash": content_hash(fixture_record).expect("fixture hash"),
        "fixture": fixture_record,
        "model": ModelKey::MonteCarloHullWhite1F,
        "model_parameters": parameters,
        "config_hash": content_hash(&config_record).expect("MC config hash"),
        "config": config_record,
        "seed": seed,
        "configured_streams": config.mc.num_paths,
        "pricing_estimators": result.measures["lsmc_num_paths"],
        "configured_physical_paths": config.mc.simulated_path_count(),
        "physical_count_source": "derived from configured streams and antithetic multiplicity; not published by this pricer",
        "base_steps": case.base_steps,
        "grid": grid_record(grid.times(), &exercise_times, &exercise_indices),
        "theta_hash": content_hash(&theta).expect("theta hash"),
        "theta": theta,
        "initial_short_rate": fd_instantaneous_forward(&discount_fn, 0.0).expect("initial short rate"),
        "output": output_record(&result),
        "stderr": stderr,
        "ci95": [ci_low, ci_high],
        "interval_interpretation": "in-sample LSMC sampling diagnostic conditional on the fitted exercise policy; excludes regression bias",
        "tree_reference_record_hash": reference.record_hash,
        "tree_reference_pv": reference.value,
        "absolute_reference_error_usd": absolute_error,
        "strict_reference_bound_usd": bound,
        "bound_formula": {"stderr_multiplier": MC_STDERR_MULTIPLIER, "tree_pv_fraction": MC_TREE_VALUE_FRACTION},
    });
    println!("FQ_HW_DIAGNOSTIC {record}");
    assert!(
        absolute_error < bound,
        "{} seed={seed}: LSMC={} tree={} stderr={stderr}, absolute error {absolute_error} must be below {bound} USD",
        case.id,
        result.value.amount(),
        reference.value,
    );
}

#[test]
#[ignore = "Full Hull-White benchmark accuracy panel; run with the scoped --slow task"]
fn hw_lsmc_performance_accuracy_panel() {
    let fixture = fixture();
    let fixture_record = json!({
        "instrument": fixture.swaption,
        "market": fixture.market,
        "as_of": fixture.as_of,
        "exercise_dates": fixture.swaption.exercise_schedule.effective_dates(),
        "accuracy_seeds": ACCURACY_SEEDS,
    });
    let reference = tree_reference(&fixture, &fixture_record);
    for case in CASES {
        mc_diagnostic(&fixture, &fixture_record, &reference, case, 42);
    }
    let pivot = CASES[3];
    // Seed 42 at the pivot was already evaluated above and belongs to both
    // panels. It is one observation, not an independent accuracy replicate.
    for seed in ACCURACY_SEEDS.into_iter().filter(|&seed| seed != 42) {
        mc_diagnostic(&fixture, &fixture_record, &reference, pivot, seed);
    }
}
