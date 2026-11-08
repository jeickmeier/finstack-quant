//! Public Sobol pricing records for CCM-I4, independent of bridge preparation.
//!
//! Floating observations are encoded as hexadecimal bit strings. Export C0
//! before candidate implementation; compare whole records without loosening
//! the exact preparation gate or pooling QMC points as IID observations.

use finstack_quant_core::Error;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::random::SobolRng;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::{
    AsianFixingSchedule, bs_price, geometric_asian_price_times,
};
use finstack_quant_models::monte_carlo::engine::PathCaptureConfig;
use finstack_quant_models::monte_carlo::payoff::asian::{Asian, AveragingMethod};
use finstack_quant_models::monte_carlo::payoff::vanilla::European;
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_models::monte_carlo::results::MonteCarloResult;
use finstack_quant_models::types::OptionType;
use serde_json::{Value, json};
use std::path::PathBuf;

const SEEDS: [u64; 5] = [0, 7, 13, 42, 0x1234_5678];
const SCRAMBLE_SALT: u64 = 0x534F_424F_4C53_4352;
const WEYL_INCREMENT: u64 = 0x9E37_79B9_7F4A_7C15;
const RATE: f64 = 0.05;
const DIVIDEND: f64 = 0.02;
const VOLATILITY: f64 = 0.20;

#[derive(Clone, Copy)]
struct Case {
    id: &'static str,
    steps: usize,
    points: usize,
    irregular: bool,
    bridge: bool,
}

const TIMING_CASES: [Case; 6] = [
    Case {
        id: "arithmetic_uniform8_points4096",
        steps: 8,
        points: 4_096,
        irregular: false,
        bridge: true,
    },
    Case {
        id: "arithmetic_uniform32_points4096",
        steps: 32,
        points: 4_096,
        irregular: false,
        bridge: true,
    },
    Case {
        id: "arithmetic_uniform40_points4096",
        steps: 40,
        points: 4_096,
        irregular: false,
        bridge: true,
    },
    Case {
        id: "arithmetic_uniform32_points16384",
        steps: 32,
        points: 16_384,
        irregular: false,
        bridge: true,
    },
    Case {
        id: "arithmetic_irregular17_points4096",
        steps: 17,
        points: 4_096,
        irregular: true,
        bridge: true,
    },
    Case {
        id: "arithmetic_uniform32_points4096_no_bridge",
        steps: 32,
        points: 4_096,
        irregular: false,
        bridge: false,
    },
];

const CAPTURE_CASE: Case = Case {
    id: "arithmetic_irregular5_points35_capture",
    steps: 5,
    points: 35,
    irregular: true,
    bridge: true,
};

fn bits(value: f64) -> String {
    assert!(value.is_finite());
    format!("{:016x}", value.to_bits())
}

fn float_bits(values: &[f64]) -> Vec<String> {
    values.iter().map(|&value| bits(value)).collect()
}

fn encode_float_numbers(value: Value) -> Value {
    match value {
        Value::Number(number) if number.is_f64() => {
            Value::String(bits(number.as_f64().expect("floating JSON number")))
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(encode_float_numbers).collect())
        }
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, encode_float_numbers(value)))
                .collect(),
        ),
        other => other,
    }
}

fn money_record(value: Money) -> Value {
    let decimal = value.amount_decimal();
    json!({
        "currency": value.currency().to_string(),
        "mantissa": decimal.mantissa().to_string(),
        "scale": decimal.scale(),
        "amount_bits": bits(value.amount()),
    })
}

fn grid(case: Case) -> TimeGrid {
    if case.irregular {
        TimeGrid::from_times(
            (0..=case.steps)
                .map(|i| (i * i) as f64 / (case.steps * case.steps) as f64)
                .collect(),
        )
        .expect("irregular frozen grid")
    } else {
        TimeGrid::uniform(1.0, case.steps).expect("uniform frozen grid")
    }
}

fn config(case: Case, seed: u64, capture: bool) -> PathDependentPricerConfig {
    PathDependentPricerConfig {
        num_paths: case.points,
        seed,
        use_parallel: false,
        chunk_size: 256,
        path_capture: if capture {
            PathCaptureConfig::all().with_payoffs()
        } else {
            PathCaptureConfig::new()
        },
        steps_per_year: 252.0,
        min_steps: 8,
        use_sobol: true,
        antithetic: false,
        use_brownian_bridge: case.bridge,
    }
}

fn replicate_record(points: usize, seed: u64) -> Value {
    let replicates = 16.min(points);
    json!({
        "total_dependent_points": points,
        "physical_paths": points,
        "independent_scrambled_replicates": replicates,
        "replicates": (0..replicates).map(|index| json!({
            "index": index,
            "points": points / replicates + usize::from(index < points % replicates),
            "scramble_seed_hex": format!("{:016x}", (seed ^ SCRAMBLE_SALT)
                .wrapping_add((index as u64).wrapping_mul(WEYL_INCREMENT)).max(1)),
        })).collect::<Vec<_>>(),
    })
}

fn run(case: Case, seed: u64, averaging: AveragingMethod, capture: bool) -> MonteCarloResult {
    let process =
        GbmProcess::new(GbmParams::new(RATE, DIVIDEND, VOLATILITY).expect("valid frozen GBM"));
    let payoff = Asian::new(
        OptionType::Call,
        100.0,
        1.0,
        averaging,
        (1..=case.steps).collect(),
    )
    .expect("post-step Asian fixing schedule");
    PathDependentPricer::new(config(case, seed, capture))
        .price_with_paths_and_grid(
            &process,
            100.0,
            grid(case),
            &payoff,
            Currency::USD,
            (-RATE).exp(),
        )
        .expect("supported public Sobol pricing")
}

fn check_result(result: &MonteCarloResult, case: Case, seed: u64, capture: bool) {
    let estimate = &result.estimate;
    assert_eq!(estimate.num_paths, case.points);
    assert_eq!(estimate.num_simulated_paths, case.points);
    for value in [estimate.mean, estimate.ci_95.0, estimate.ci_95.1] {
        assert_eq!(value.currency(), Currency::USD);
        assert!(value.amount().is_finite());
    }
    assert!(estimate.mean.amount() >= 0.0);
    assert!(estimate.stderr.is_finite() && estimate.stderr > 0.0);
    assert!(
        estimate
            .std_dev
            .is_some_and(|value| value.is_finite() && value > 0.0)
    );
    assert!(estimate.ci_95.0.amount() <= estimate.mean.amount());
    assert!(estimate.mean.amount() <= estimate.ci_95.1.amount());
    let metadata = result.run.as_ref().expect("public run metadata");
    assert_eq!(metadata.seed, Some(seed));
    assert!(!metadata.use_parallel && !metadata.antithetic);
    assert_eq!(metadata.chunk_size, 256);
    assert_eq!(metadata.num_steps, case.steps);
    assert_eq!(result.paths.is_some(), capture);
    if !capture {
        assert!(estimate.median.is_none() && estimate.percentile_25.is_none());
        assert!(
            estimate.percentile_75.is_none() && estimate.min.is_none() && estimate.max.is_none()
        );
    }
}

fn observation(case: Case, seed: u64, averaging: AveragingMethod, capture: bool) -> Value {
    let result = run(case, seed, averaging, capture);
    check_result(&result, case, seed, capture);
    result_record(case, seed, averaging, capture, &result)
}

fn result_record(
    case: Case,
    seed: u64,
    averaging: AveragingMethod,
    capture: bool,
    result: &MonteCarloResult,
) -> Value {
    let time_grid = grid(case);
    let dts: Vec<f64> = (0..case.steps).map(|step| time_grid.dt(step)).collect();
    let mut record = json!({
        "case_id": format!("sobol_bridge_price/{}", case.id),
        "seed_hex": format!("{seed:016x}"),
        "grid": { "constructor": if case.irregular { "from_times_i_squared" } else { "uniform" },
            "steps": case.steps, "factors": 1, "sobol_dimension": case.steps,
            "times_bits": float_bits(time_grid.times()), "dts_bits": float_bits(&dts) },
        "config": { "num_paths": case.points, "use_parallel": false, "chunk_size": 256,
            "path_capture": { "enabled": capture, "mode": "all", "capture_payoffs": capture },
            "steps_per_year_bits": bits(252.0), "min_steps": 8, "use_sobol": true,
            "antithetic": false, "use_brownian_bridge": case.bridge, "target_ci_half_width": null },
        "process": { "type": "gbm", "spot_bits": bits(100.0), "rate_bits": bits(RATE),
            "dividend_bits": bits(DIVIDEND), "volatility_bits": bits(VOLATILITY),
            "drift_schedule": null },
        "payoff": { "type": "asian", "option_type": "call", "averaging": averaging.to_string(),
            "strike_bits": bits(100.0), "quantity_bits": bits(1.0),
            "fixing_steps": (1..=case.steps).collect::<Vec<_>>(),
            "fixing_multipliers_bits": vec![bits(1.0); case.steps],
            "history": { "count": 0, "sum_bits": bits(0.0), "log_sum_bits": bits(0.0) } },
        "discount_bits": bits((-RATE).exp()),
        "qmc_counts": replicate_record(case.points, seed),
        "result": encode_float_numbers(serde_json::to_value(result).expect("serialize full result")),
        "money_exact": { "mean": money_record(result.estimate.mean),
            "ci_lower": money_record(result.estimate.ci_95.0), "ci_upper": money_record(result.estimate.ci_95.1) },
        "comparison_policy": "exact_complete_record_same_target",
    });
    if capture {
        record["independent_direct_reference"] = check_captured_reference(result, case, seed);
    }
    record
}

// Recompute bridge topology recursively, without using BrownianBridge or a
// candidate prepared coefficient table. Retain C0 arithmetic ordering.
fn direct_bridge(z: &[f64], times: &[f64]) -> Vec<f64> {
    fn fill(left: usize, right: usize, z: &[f64], times: &[f64], w: &mut [f64], next: &mut usize) {
        if right - left <= 1 {
            return;
        }
        let mid = (left + right) / 2;
        let alpha = (times[mid] - times[left]) / (times[right] - times[left]);
        let mean = w[left] + alpha * (w[right] - w[left]);
        let variance = ((times[mid] - times[left]) * (times[right] - times[mid]))
            / (times[right] - times[left]);
        w[mid] = mean + variance.sqrt() * z[*next];
        *next += 1;
        fill(left, mid, z, times, w, next);
        fill(mid, right, z, times, w, next);
    }
    let mut w = vec![0.0; z.len() + 1];
    w[z.len()] = z[0] * times[z.len()].sqrt();
    let mut next = 1;
    fill(0, z.len(), z, times, &mut w, &mut next);
    assert_eq!(next, z.len());
    w
}

fn check_captured_reference(result: &MonteCarloResult, case: Case, seed: u64) -> Value {
    let paths = result
        .paths
        .as_ref()
        .expect("explicit all-capture diagnostic");
    assert_eq!(paths.num_paths_total, case.points);
    assert_eq!(paths.paths.len(), case.points);
    let time_grid = grid(case);
    let replicates = 16.min(case.points);
    let mut path_id = 0;
    let mut records = Vec::with_capacity(case.points);
    for replicate in 0..replicates {
        let scramble_seed = (seed ^ SCRAMBLE_SALT)
            .wrapping_add((replicate as u64).wrapping_mul(WEYL_INCREMENT))
            .max(1);
        let mut rng =
            SobolRng::try_new(case.steps, scramble_seed).expect("supported reference dimension");
        let points = case.points / replicates + usize::from(replicate < case.points % replicates);
        for point_index in 0..points {
            let mut z = vec![0.0; case.steps];
            rng.fill_std_normals(&mut z);
            let w = direct_bridge(&z, time_grid.times());
            let increments: Vec<f64> = (0..case.steps)
                .map(|step| (w[step + 1] - w[step]) / time_grid.dt(step).sqrt())
                .collect();
            let path = &paths.paths[path_id];
            assert_eq!(path.path_id, path_id);
            assert_eq!(path.points.len(), case.steps + 1);
            assert!(path.irr.is_none());
            let mut spot = 100.0;
            let mut sum = 0.0;
            let mut compensation = 0.0;
            for (step, captured) in path.points.iter().enumerate() {
                let expected_time = if step == 0 {
                    0.0
                } else {
                    time_grid.time(step - 1) + time_grid.dt(step - 1)
                };
                if step > 0 {
                    let dt = time_grid.dt(step - 1);
                    let drift = (RATE - DIVIDEND) * dt - 0.5 * VOLATILITY * VOLATILITY * dt;
                    spot *= (drift + VOLATILITY * dt.sqrt() * increments[step - 1]).exp();
                    let y = spot - compensation;
                    let updated = sum + y;
                    compensation = (updated - sum) - y;
                    sum = updated;
                }
                let average = if step == 0 { 0.0 } else { sum / step as f64 };
                let payoff = Money::new((average - 100.0).max(0.0), Currency::USD)
                    .expect("finite independent payoff")
                    .amount();
                assert_eq!(captured.step, step);
                assert_eq!(captured.time.to_bits(), expected_time.to_bits());
                assert_eq!(captured.state.len(), 1);
                assert_eq!(
                    captured.state[0].to_bits(),
                    spot.to_bits(),
                    "path {path_id}, step {step}"
                );
                assert_eq!(
                    captured.payoff_value.map(f64::to_bits),
                    Some(payoff.to_bits())
                );
                assert!(captured.cashflows.is_empty());
                if step == case.steps {
                    assert_eq!(
                        path.final_value.to_bits(),
                        (payoff * (-RATE).exp()).to_bits()
                    );
                }
            }
            records.push(json!({ "path_id": path_id, "replicate": replicate, "point_in_replicate": point_index,
                "normal_bits": float_bits(&z), "direct_bridge_bits": float_bits(&w),
                "normalized_increment_bits": float_bits(&increments) }));
            path_id += 1;
        }
    }
    assert_eq!(path_id, case.points);
    json!({ "kind": "direct_recursive_bridge_original_division_and_scalar_exact_gbm",
        "complete_path_state_payoff_cashflow_bits_equal": true, "paths": records })
}

fn price_records() -> Vec<Value> {
    let mut records = Vec::new();
    for case in TIMING_CASES {
        records.push(observation(case, 42, AveragingMethod::Arithmetic, false));
    }
    for case in [TIMING_CASES[1], TIMING_CASES[4]] {
        for seed in SEEDS.into_iter().filter(|&seed| seed != 42) {
            records.push(observation(case, seed, AveragingMethod::Arithmetic, false));
        }
    }
    records
}

fn capture_records() -> Vec<Value> {
    SEEDS
        .into_iter()
        .map(|seed| observation(CAPTURE_CASE, seed, AveragingMethod::Arithmetic, true))
        .collect()
}

fn geometric_accuracy_records() -> Vec<Value> {
    let case = Case {
        id: "geometric_irregular17_points16384_accuracy",
        points: 16_384,
        ..TIMING_CASES[4]
    };
    let time_grid = grid(case);
    let reference = geometric_asian_price_times(
        100.0,
        100.0,
        DIVIDEND,
        VOLATILITY,
        &AsianFixingSchedule {
            t_expiry: 1.0,
            df: (-RATE).exp(),
            fixing_times: &time_grid.times()[1..],
        },
        OptionType::Call,
    )
    .expect("exact geometric Asian reference");
    SEEDS
        .into_iter()
        .map(|seed| {
            let result = run(case, seed, AveragingMethod::Geometric, false);
            check_result(&result, case, seed, false);
            let error = (result.estimate.mean.amount() - reference).abs();
            let bound = 6.0 * result.estimate.stderr;
            assert!(
                error < bound,
                "seed {seed}: geometric error {error} >= six-replicate-stderr bound {bound}"
            );
            let mut record = result_record(case, seed, AveragingMethod::Geometric, false, &result);
            record["accuracy"] = json!({ "reference_kind": "exact_discrete_geometric_asian",
            "reference_bits": bits(reference), "absolute_error_bits": bits(error),
            "bound_bits": bits(bound), "absolute_bias_bits": bits(0.0),
            "gate": "strict_absolute_error_less_than_six_independent_replicate_stderr" });
            record
        })
        .collect()
}

fn european_accuracy_record() -> Value {
    let case = Case {
        id: "european_uniform16_points16384_accuracy",
        steps: 16,
        points: 16_384,
        irregular: false,
        bridge: true,
    };
    let process =
        GbmProcess::new(GbmParams::new(RATE, 0.0, VOLATILITY).expect("existing European fixture"));
    let payoff = European::new(OptionType::Call, 100.0, 1.0, case.steps);
    let result = PathDependentPricer::new(config(case, 42, false))
        .price_with_paths_and_grid(
            &process,
            100.0,
            grid(case),
            &payoff,
            Currency::USD,
            (-RATE).exp(),
        )
        .expect("existing European QMC control");
    check_result(&result, case, 42, false);
    let reference = bs_price(100.0, 100.0, RATE, 0.0, VOLATILITY, 1.0, OptionType::Call)
        .expect("Black-Scholes reference");
    let error = (result.estimate.mean.amount() - reference).abs();
    assert!(error < 6.0 * result.estimate.stderr);
    assert!(result.estimate.stderr < 0.05);
    let time_grid = grid(case);
    let dts: Vec<f64> = (0..case.steps).map(|step| time_grid.dt(step)).collect();
    json!({ "case_id": case.id, "seed_hex": format!("{:016x}", 42),
        "grid": { "constructor": "uniform", "steps": case.steps,
            "times_bits": float_bits(time_grid.times()), "dts_bits": float_bits(&dts),
            "factors": 1, "sobol_dimension": case.steps },
        "config": { "num_paths": case.points, "use_parallel": false, "chunk_size": 256,
            "path_capture": { "enabled": false, "mode": "all", "capture_payoffs": false },
            "steps_per_year_bits": bits(252.0), "min_steps": 8, "use_sobol": true,
            "antithetic": false, "use_brownian_bridge": true, "target_ci_half_width": null },
        "process": { "type": "gbm", "spot_bits": bits(100.0), "rate_bits": bits(RATE),
            "dividend_bits": bits(0.0), "volatility_bits": bits(VOLATILITY), "drift_schedule": null },
        "payoff": { "type": "european", "option_type": "call", "strike_bits": bits(100.0),
            "quantity_bits": bits(1.0), "maturity_step": case.steps },
        "discount_bits": bits((-RATE).exp()),
        "reference_kind": "existing_black_scholes_qmc_gate", "reference_bits": bits(reference),
        "absolute_error_bits": bits(error), "bound_bits": bits(6.0 * result.estimate.stderr),
        "stderr_upper_bound_bits": bits(0.05), "qmc_counts": replicate_record(case.points, 42),
        "result": encode_float_numbers(serde_json::to_value(&result).expect("serialize European result")),
        "money_exact": { "mean": money_record(result.estimate.mean),
            "ci_lower": money_record(result.estimate.ci_95.0), "ci_upper": money_record(result.estimate.ci_95.1) } })
}

fn error_records() -> Vec<Value> {
    let process = GbmProcess::new(GbmParams::new(RATE, DIVIDEND, VOLATILITY).expect("GBM"));
    let mut records = Vec::new();
    for kind in [
        "parallel",
        "antithetic",
        "dimension_41",
        "one_point",
        "zero_chunk",
        "zero_capture_sample",
    ] {
        let mut case = TIMING_CASES[1];
        let mut configured = config(case, 42, false);
        match kind {
            "parallel" => configured.use_parallel = true,
            "antithetic" => configured.antithetic = true,
            "dimension_41" => case.steps = 41,
            "one_point" => configured.num_paths = 1,
            "zero_chunk" => configured.chunk_size = 0,
            "zero_capture_sample" => configured.path_capture = PathCaptureConfig::sample(0, 42),
            _ => unreachable!("enumerated error fixture"),
        }
        let payoff = Asian::new(
            OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            (1..=case.steps).collect(),
        )
        .expect("payoff");
        let error = PathDependentPricer::new(configured)
            .price(
                &process,
                100.0,
                grid(case),
                &payoff,
                Currency::USD,
                (-RATE).exp(),
            )
            .expect_err("unsupported execution must fail before simulation");
        assert!(matches!(&error, Error::Validation(_)));
        records.push(
            json!({ "case_id": kind, "error_variant": "Validation", "message": error.to_string() }),
        );
    }
    records
}

#[test]
fn brownian_bridge_public_pricing_reference_panel() {
    assert_eq!(price_records().len(), 14);
}

#[test]
fn brownian_bridge_direct_capture_and_unequal_replicate_panel() {
    assert_eq!(capture_records().len(), SEEDS.len());
}

#[test]
fn brownian_bridge_exact_geometric_and_european_accuracy() {
    assert_eq!(geometric_accuracy_records().len(), SEEDS.len());
    european_accuracy_record();
}

#[test]
fn brownian_bridge_public_capability_errors() {
    assert_eq!(error_records().len(), 6);
}

#[test]
#[ignore = "CCM numerical export; set CCM_MODELS_EXPORT_DIR to a new revision-specific directory"]
fn export_brownian_bridge_reference() {
    let directory =
        PathBuf::from(std::env::var_os("CCM_MODELS_EXPORT_DIR").expect("CCM_MODELS_EXPORT_DIR"));
    std::fs::create_dir_all(&directory).expect("create CCM export directory");
    let record = json!({ "schema": "ccm-models-brownian-bridge-v1",
        "comparison": "exact whole-record values on the same target; no tolerance relaxation after C0",
        "price_records": price_records(), "capture_records": capture_records(),
        "geometric_accuracy_records": geometric_accuracy_records(),
        "european_accuracy_record": european_accuracy_record(), "required_errors": error_records() });
    let path = directory.join("brownian-bridge-reference.json");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("new revision-specific export; never overwrite historical records");
    serde_json::to_writer_pretty(file, &record).expect("write CCM reference");
    println!("CCM Brownian bridge reference: {}", path.display());
}
