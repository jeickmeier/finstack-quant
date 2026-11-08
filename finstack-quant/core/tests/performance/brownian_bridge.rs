//! Exact Brownian bridge references for the CCM performance campaign.
//!
//! The oracle derives its own midpoint order and uses ordered-set neighbour
//! lookup, independently of the production bridge's stored step table. Records
//! use hexadecimal float-bit strings and are exported only when requested.

use finstack_quant_core::Error;
use finstack_quant_core::math::random::BrownianBridge;
use finstack_quant_core::math::time_grid::TimeGrid;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;

const SEEDS: [u64; 5] = [0, 7, 13, 42, 0x1234_5678];

struct GridCase {
    id: String,
    constructor: &'static str,
    times: Vec<f64>,
    dts: Vec<f64>,
}

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn bit_values(values: &[f64]) -> Vec<String> {
    values.iter().map(|&value| bits(value)).collect()
}

fn assert_same_bits(actual: &[f64], expected: &[f64], case: &str) {
    assert_eq!(actual.len(), expected.len(), "{case}: output length");
    for (point, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(actual.is_finite(), "{case}: non-finite point {point}");
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{case}: point {point}"
        );
    }
}

fn shocks(num_steps: usize, seed: u64) -> Vec<f64> {
    // A bounded deterministic mixed-sign vector, not a statistical RNG claim.
    let offset = (seed % 101) as usize;
    (0..num_steps)
        .map(|index| ((index * 37 + offset * 19) % 101) as f64 * 0.031 - 1.5)
        .collect()
}

fn reference_path(z: &[f64], times: &[f64]) -> Vec<f64> {
    fn order(left: usize, right: usize, out: &mut Vec<usize>) {
        if right - left > 1 {
            let midpoint = (left + right) / 2;
            out.push(midpoint);
            order(left, midpoint, out);
            order(midpoint, right, out);
        }
    }

    let num_steps = z.len();
    assert_eq!(times.len(), num_steps + 1);
    let mut out = vec![f64::NAN; num_steps + 1];
    out[0] = 0.0;
    if num_steps == 0 {
        return out;
    }
    out[num_steps] = z[0] * times[num_steps].sqrt();
    let mut construction_order = Vec::new();
    order(0, num_steps, &mut construction_order);
    assert_eq!(construction_order.len(), num_steps - 1);
    let mut populated = BTreeSet::from([0, num_steps]);
    for (shock, index) in construction_order.into_iter().enumerate() {
        let left = *populated
            .range(..index)
            .next_back()
            .expect("left neighbour");
        let right = *populated
            .range(index + 1..)
            .next()
            .expect("right neighbour");
        let alpha = (times[index] - times[left]) / (times[right] - times[left]);
        let mean = out[left] + alpha * (out[right] - out[left]);
        let variance = ((times[index] - times[left]) * (times[right] - times[index]))
            / (times[right] - times[left]);
        out[index] = mean + variance.sqrt() * z[shock + 1];
        populated.insert(index);
    }
    out
}

fn grid_case(id: String, constructor: &'static str, grid: TimeGrid) -> GridCase {
    GridCase {
        id,
        constructor,
        times: grid.times().to_vec(),
        dts: grid.dts().to_vec(),
    }
}

fn cases() -> Vec<GridCase> {
    let mut cases = vec![GridCase {
        id: "zero_step_kernel".to_string(),
        constructor: "explicit_zero_step_kernel",
        times: vec![0.0],
        dts: Vec::new(),
    }];
    for steps in [1, 8, 16, 32, 40, 64, 256] {
        cases.push(grid_case(
            format!("uniform{steps}"),
            "TimeGrid::uniform",
            TimeGrid::uniform(1.0, steps).expect("valid uniform grid"),
        ));
    }
    for steps in [3, 17, 39] {
        let times = (0..=steps)
            .map(|index| (index * index) as f64 / (steps * steps) as f64)
            .collect();
        cases.push(grid_case(
            format!("irregular{steps}"),
            "TimeGrid::from_times",
            TimeGrid::from_times(times).expect("valid irregular grid"),
        ));
    }
    let uniform = TimeGrid::uniform(1.0, 40).expect("valid uniform grid");
    cases.push(grid_case(
        "uniform40_rebuilt_from_times".to_string(),
        "TimeGrid::from_times",
        TimeGrid::from_times(uniform.times().to_vec()).expect("valid reconstructed grid"),
    ));
    let changed_times = (0..=17)
        .map(|index| (index * index) as f64 / 289.0 * 1.75)
        .collect();
    cases.push(grid_case(
        "irregular17_changed_horizon".to_string(),
        "TimeGrid::from_times",
        TimeGrid::from_times(changed_times).expect("valid changed grid"),
    ));
    cases
}

fn valid_records() -> Vec<Value> {
    let mut records = Vec::new();
    for case in cases() {
        let num_steps = case.times.len() - 1;
        assert_eq!(case.dts.len(), num_steps);
        for seed in SEEDS {
            let z = shocks(num_steps, seed);
            let expected = reference_path(&z, &case.times);
            let bridge = BrownianBridge::new(num_steps);
            let mut path = vec![f64::NAN; num_steps + 1];
            bridge
                .construct_path(&z, &mut path, &case.times)
                .expect("valid bridge grid");
            let case_id = format!("{}/seed_{seed:016x}", case.id);
            assert_same_bits(&path, &expected, &case_id);
            // This derived observation pins stored-dt normalization arithmetic;
            // it is not an invocation of the private models increment helper.
            let increments: Vec<f64> = path
                .windows(2)
                .zip(&case.dts)
                .map(|(pair, dt)| (pair[1] - pair[0]) / dt.sqrt())
                .collect();
            let reference_increments: Vec<f64> = expected
                .windows(2)
                .zip(&case.dts)
                .map(|(pair, dt)| (pair[1] - pair[0]) / dt.sqrt())
                .collect();
            assert_same_bits(&increments, &reference_increments, &case_id);
            records.push(json!({
                "case_id": case_id,
                "grid_id": case.id,
                "constructor_kind": case.constructor,
                "num_steps": num_steps,
                "num_factors": 1,
                "sobol_dimension": num_steps,
                "supported_production_sobol_dimension": (1..=40).contains(&num_steps),
                "seed_hex": format!("{seed:016x}"),
                "shock_generator": "bounded_modular_mixed_sign_v1",
                "times_bits": bit_values(&case.times),
                "dts_bits": bit_values(&case.dts),
                "shocks_bits": bit_values(&z),
                "path_bits": bit_values(&path),
                "reference_path_bits": bit_values(&expected),
                "derived_increments_from_stored_dts_bits": bit_values(&increments),
                "reference_derived_increments_bits": bit_values(&reference_increments),
                "increment_observation_kind": "test-only path differences divided by stored sqrt(dt)",
                "maximum_absolute_path_error_bits": bits(0.0),
                "exact_path_and_derived_increment_bits": true,
            }));
        }
    }
    assert_eq!(records.len(), 65);
    records
}

fn invalid_record(
    id: &str,
    bridge_steps: usize,
    z: Vec<f64>,
    times: Vec<f64>,
    output_len: usize,
) -> Value {
    let bridge = BrownianBridge::new(bridge_steps);
    let mut output = vec![0.125; output_len];
    let before = output.clone();
    let error = bridge
        .construct_path(&z, &mut output, &times)
        .expect_err("invalid bridge fixture");
    let message = match error {
        Error::Validation(message) => message,
        other => panic!("{id}: expected Validation, got {other:?}"),
    };
    assert_eq!(
        bit_values(&output),
        bit_values(&before),
        "{id}: validation mutated output"
    );
    json!({
        "case_id": id,
        "bridge_steps": bridge_steps,
        "shocks_bits": bit_values(&z),
        "times_bits": bit_values(&times),
        "output_len": output_len,
        "before_output_bits": bit_values(&before),
        "after_output_bits": bit_values(&output),
        "expected_error_variant": "Validation",
        "actual_error_variant": "Validation",
        "error_message": message,
        "output_unchanged": true,
    })
}

fn invalid_records() -> Vec<Value> {
    let z = shocks(4, 42);
    let times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    vec![
        invalid_record("short_output", 4, z.clone(), times.clone(), 4),
        invalid_record("long_output", 4, z.clone(), times, 6),
        invalid_record(
            "bridge_shock_count_mismatch",
            4,
            shocks(2, 42),
            vec![0.0, 0.5, 1.0],
            3,
        ),
        invalid_record("short_times", 4, z.clone(), vec![0.0, 0.25, 0.5, 1.0], 5),
        invalid_record(
            "long_times",
            4,
            z.clone(),
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0],
            5,
        ),
        invalid_record(
            "nonzero_origin",
            4,
            z.clone(),
            vec![0.1, 0.25, 0.5, 0.75, 1.0],
            5,
        ),
        invalid_record(
            "nan_origin",
            4,
            z.clone(),
            vec![f64::NAN, 0.25, 0.5, 0.75, 1.0],
            5,
        ),
        invalid_record(
            "nan_interior",
            4,
            z.clone(),
            vec![0.0, 0.25, f64::NAN, 0.75, 1.0],
            5,
        ),
        invalid_record(
            "infinite_terminal",
            4,
            z.clone(),
            vec![0.0, 0.25, 0.5, 0.75, f64::INFINITY],
            5,
        ),
        invalid_record(
            "duplicate_time",
            4,
            z.clone(),
            vec![0.0, 0.25, 0.25, 0.75, 1.0],
            5,
        ),
        invalid_record("descending_time", 4, z, vec![0.0, 0.5, 0.25, 0.75, 1.0], 5),
        invalid_record("zero_step_nonzero_origin", 0, Vec::new(), vec![0.1], 1),
        invalid_record(
            "zero_bridge_with_one_shock",
            0,
            vec![1.0],
            vec![0.0, 1.0],
            2,
        ),
        invalid_record(
            "one_step_negative_terminal",
            1,
            vec![1.0],
            vec![0.0, -0.25],
            2,
        ),
    ]
}

#[test]
fn brownian_bridge_matches_independent_reference_bits() {
    let _ = valid_records();
}

#[test]
fn brownian_bridge_grid_constructors_preserve_stored_dt_contract() {
    let uniform = TimeGrid::uniform(1.0, 40).expect("valid uniform grid");
    let rebuilt = TimeGrid::from_times(uniform.times().to_vec()).expect("valid reconstructed grid");
    assert_eq!(bit_values(uniform.times()), bit_values(rebuilt.times()));
    assert!(
        uniform
            .dts()
            .iter()
            .zip(rebuilt.dts())
            .any(|(left, right)| left.to_bits() != right.to_bits()),
        "fixture must expose dt differences"
    );
    let z = shocks(40, 42);
    let mut uniform_path = vec![f64::NAN; 41];
    let mut rebuilt_path = vec![f64::NAN; 41];
    BrownianBridge::new(40)
        .construct_path(&z, &mut uniform_path, uniform.times())
        .expect("uniform path");
    BrownianBridge::new(40)
        .construct_path(&z, &mut rebuilt_path, rebuilt.times())
        .expect("rebuilt path");
    assert_same_bits(
        &uniform_path,
        &rebuilt_path,
        "identical knots, distinct stored dts",
    );
}

#[test]
fn brownian_bridge_reconstruction_uses_changed_grid() {
    let grids = cases();
    let base = grids
        .iter()
        .find(|case| case.id == "irregular17")
        .expect("base fixture");
    let changed = grids
        .iter()
        .find(|case| case.id == "irregular17_changed_horizon")
        .expect("changed fixture");
    let z = shocks(17, 42);
    let mut paths = Vec::new();
    for grid in [base, changed] {
        let mut path = vec![f64::NAN; 18];
        BrownianBridge::new(17)
            .construct_path(&z, &mut path, &grid.times)
            .expect("rebuilt bridge");
        assert_same_bits(&path, &reference_path(&z, &grid.times), &grid.id);
        paths.push(bit_values(&path));
    }
    assert_ne!(paths[0], paths[1], "changed horizon must change the path");
}

#[test]
fn brownian_bridge_rejects_invalid_inputs_without_mutation() {
    assert_eq!(invalid_records().len(), 14);
}

#[test]
#[ignore = "CCM reference exporter; run explicitly with CCM_CORE_EXPORT_DIR"]
fn export_brownian_bridge_reference() {
    let records = valid_records();
    let failures = invalid_records();
    assert_eq!(failures.len(), 14);
    let payload = json!({
        "schema_version": 1,
        "owner": "finstack-quant-core",
        "panel": "CCM-I4-brownian-bridge",
        "reference_kind": "direct_recursive_ordered_set_c0",
        "accuracy_policy": "every path and test-derived increment float bit must match",
        "absolute_error_allowance_bits": bits(0.0),
        "float_encoding": "16-digit hexadecimal IEEE-754 f64 bit strings",
        "grid_case_count": 13,
        "seed_count": SEEDS.len(),
        "valid_record_count": records.len(),
        "invalid_record_count": failures.len(),
        "records": records,
        "invalid_inputs": failures,
    });
    if let Some(directory) = std::env::var_os("CCM_CORE_EXPORT_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create requested core export directory");
        let destination = directory.join("brownian-bridge-reference.json");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .expect("create new requested Brownian bridge reference export");
        serde_json::to_writer_pretty(file, &payload)
            .expect("write requested Brownian bridge reference export");
        eprintln!("Brownian bridge reference: {}", destination.display());
    }
}
