//! CCM-I5 numerical gates frozen before the performance baseline.
//!
//! The original pairwise drift/step lives in a separate test-only source file.
//! Candidate comparisons use matched, prerecorded innovations, not ordinary MC
//! confidence intervals as permission for a numerical change.

use crate::lmm_reference as reference;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use finstack_quant_models::monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};
use serde_json::{Value, json};

const SEEDS: [u64; 3] = [7, 42, 20_261_107];
const PATH_STREAMS: usize = 64;
const CAPLET_STREAMS: usize = 2_048;
const NOTIONAL: f64 = 1_000_000.0;

// Explicitly named, deterministic non-cryptographic fingerprints. The campaign
// archives source files and their cryptographic hashes separately.
struct Fingerprint(u64);

impl Fingerprint {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }

    fn values(&mut self, values: &[f64]) {
        for value in values {
            self.bytes(&value.to_bits().to_le_bytes());
        }
    }

    fn finish(&self) -> String {
        format!("fnv1a64:{:016x}", self.0)
    }
}

fn json_fingerprint(value: &impl serde::Serialize) -> String {
    let mut hash = Fingerprint::new();
    hash.bytes(&serde_json::to_vec(value).expect("serialize finite numerical fixture"));
    hash.finish()
}

struct Fixture {
    id: &'static str,
    params: LmmParams,
    grid: TimeGrid,
}

fn base_params(n: usize, factors: usize) -> LmmParams {
    LmmParams {
        num_forwards: n,
        num_factors: factors,
        tenors: (0..=n).map(|i| i as f64).collect(),
        accrual_factors: vec![1.0; n],
        displacements: vec![0.005; n],
        vol_times: Vec::new(),
        vol_values: vec![
            (0..n)
                .map(|i| {
                    [
                        0.15 - 0.002 * i as f64,
                        0.05,
                        if factors == 3 { 0.02 } else { 0.0 },
                    ]
                })
                .collect(),
        ],
        initial_forwards: vec![0.03; n],
    }
}

fn knot_params(n: usize) -> LmmParams {
    let mut params = base_params(n, 2);
    params.vol_times = vec![0.75, 1.25];
    params.vol_values.push(
        (0..n)
            .map(|i| [1.2 * (0.15 - 0.002 * i as f64), 0.03, 0.0])
            .collect(),
    );
    params.vol_values.push(
        (0..n)
            .map(|i| [0.8 * (0.15 - 0.002 * i as f64), -0.04, 0.0])
            .collect(),
    );
    params
}

fn fixtures() -> Vec<Fixture> {
    let specs = [
        ("scaling_lmm_forwards_fixed_2y_8_steps/10", 10, 8, 2),
        ("scaling_lmm_forwards_fixed_2y_8_steps/20", 20, 8, 2),
        ("scaling_lmm_forwards_fixed_2y_8_steps/40", 40, 8, 2),
        ("scaling_lmm_forwards_fixed_2y_8_steps/80", 80, 8, 2),
        ("scaling_lmm_steps_fixed_2y_20_forwards/16", 20, 16, 2),
        ("scaling_lmm_steps_fixed_2y_20_forwards/32", 20, 32, 2),
        (
            "scaling_lmm_controls_fixed_2y_8_steps/factors3_forwards40",
            40,
            8,
            3,
        ),
    ];
    let mut result: Vec<Fixture> = specs
        .into_iter()
        .map(|(id, n, steps, factors)| Fixture {
            id,
            params: base_params(n, factors),
            grid: TimeGrid::uniform(2.0, steps).expect("valid frozen LMM grid"),
        })
        .collect();
    result.push(Fixture {
        id: "scaling_lmm_controls_fixed_2y_8_steps/loading_knots_forwards20",
        params: knot_params(20),
        grid: TimeGrid::uniform(2.0, 8).expect("valid frozen knot grid"),
    });
    for fixture in &result {
        fixture
            .params
            .validate()
            .expect("valid frozen LMM parameters");
        LmmProcess::new(fixture.params.clone())
            .validate_time_grid(&fixture.grid)
            .expect("all fixing and loading knots represented exactly");
    }
    result
}

struct Errors {
    count: u64,
    squared: f64,
    max_abs: f64,
    max_bound_fraction: f64,
    worst: Value,
    reference_values: Fingerprint,
    candidate_values: Fingerprint,
}

impl Errors {
    fn new() -> Self {
        Self {
            count: 0,
            squared: 0.0,
            max_abs: 0.0,
            max_bound_fraction: 0.0,
            worst: Value::Null,
            reference_values: Fingerprint::new(),
            candidate_values: Fingerprint::new(),
        }
    }

    fn observe(&mut self, actual: f64, expected: f64, bound: f64, location: Value) {
        assert!(actual.is_finite() && expected.is_finite() && bound.is_finite() && bound > 0.0);
        let error = (actual - expected).abs();
        assert!(
            error <= bound,
            "LMM error {error} exceeds {bound} at {location}"
        );
        let fraction = error / bound;
        if self.count == 0 || fraction > self.max_bound_fraction {
            self.worst = json!({"location": location, "actual": actual, "reference": expected, "bound": bound});
        }
        self.count += 1;
        self.squared += error * error;
        self.max_abs = self.max_abs.max(error);
        self.max_bound_fraction = self.max_bound_fraction.max(fraction);
        self.reference_values.values(&[expected]);
        self.candidate_values.values(&[actual]);
    }

    fn record(&self) -> Value {
        json!({
            "components": self.count,
            "max_absolute_error": self.max_abs,
            "rms_error": (self.squared / self.count.max(1) as f64).sqrt(),
            "max_bound_fraction": self.max_bound_fraction,
            "worst": self.worst,
            "reference_values_fingerprint": self.reference_values.finish(),
            "candidate_values_fingerprint": self.candidate_values.finish(),
        })
    }
}

fn compare_drift(params: &LmmParams, t: f64, state: &[f64], location: Value, errors: &mut Errors) {
    let process = LmmProcess::new(params.clone());
    let mut actual = vec![0.0; params.num_forwards];
    let mut expected = vec![0.0; params.num_forwards];
    process.drift(t, state, &mut actual);
    reference::drift(params, t, state, &mut expected);
    let first = reference::first_alive(params, t);
    let all_zero = params
        .vol_values
        .iter()
        .flatten()
        .flatten()
        .all(|&value| value == 0.0);
    for i in 0..params.num_forwards {
        let scale = reference::contribution_scale(params, t, state, i);
        let bound = 512.0 * f64::EPSILON * scale + 1e-18;
        errors.observe(
            actual[i],
            expected[i],
            bound,
            json!({"case": location, "forward": i, "condition_scale": scale}),
        );
        if i < first || i == params.num_forwards - 1 || all_zero {
            assert_eq!(
                actual[i].to_bits(),
                expected[i].to_bits(),
                "exact drift zero identity"
            );
            assert_eq!(actual[i], 0.0);
        }
    }
}

fn scalar_panel() -> Value {
    let mut errors = Errors::new();
    let mut runtime_probe_records = Vec::new();
    for fixture in fixtures() {
        let params = &fixture.params;
        let n = params.num_forwards;
        let mut times = vec![0.0, 0.5, 2.0];
        for knot in [0.75_f64, 1.0, 1.25] {
            times.extend([
                f64::from_bits(knot.to_bits() - 1),
                knot,
                f64::from_bits(knot.to_bits() + 1),
            ]);
        }
        let homogeneous = params.initial_forwards.clone();
        let sloped: Vec<f64> = (0..n).map(|i| 0.01 + 0.0005 * i as f64).collect();
        let negative = vec![-0.004; n];
        let mut absorbed = homogeneous.clone();
        absorbed[2] = -params.displacements[2];
        let mut tiny = absorbed.clone();
        tiny[2] += 1e-15;
        let states = [
            ("homogeneous", homogeneous),
            ("sloped", sloped),
            ("negative", negative),
            ("absorbed_runtime", absorbed),
            ("tiny_displaced_level", tiny),
        ];
        for t in times {
            for (name, state) in &states {
                compare_drift(
                    params,
                    t,
                    state,
                    json!({"fixture": fixture.id, "time_bits": t.to_bits(), "state": name}),
                    &mut errors,
                );
            }
        }
        // The B1 homogeneous assertion remains tighter than the general path gate.
        let mut actual = vec![0.0; n];
        let process = LmmProcess::new(params.clone());
        process.drift(0.0, &params.initial_forwards, &mut actual);
        let mut expected = vec![0.0; n];
        reference::drift(params, 0.0, &params.initial_forwards, &mut expected);
        for (&a, &b) in actual.iter().zip(&expected) {
            assert!((a - b).abs() < 1e-15);
        }
    }
    let mut params = base_params(10, 2);
    for mode in ["rank_one", "zero_columns", "signed_cancellation"] {
        for (i, row) in params.vol_values[0].iter_mut().enumerate() {
            *row = match mode {
                "rank_one" => [0.15 - 0.002 * i as f64, 0.0, 0.0],
                "zero_columns" => [0.0; 3],
                _ => [if i % 2 == 0 { 0.10 } else { -0.10 }, 0.05, 0.0],
            };
        }
        params
            .validate()
            .expect("supported two-factor covariance control");
        compare_drift(
            &params,
            0.0,
            &params.initial_forwards,
            json!({"fixture": mode}),
            &mut errors,
        );
    }
    // Stored representable states straddle the actual skip decision. No decimal
    // literal is claimed to produce a denominator exactly equal to 1e-15.
    let params = base_params(4, 2);
    let near = -1.0_f64 + 1e-15;
    let probes = [
        f64::from_bits(near.to_bits() + 1),
        near,
        f64::from_bits(near.to_bits() - 1),
        -1.0,
    ];
    let mut saw_skip = false;
    let mut saw_keep = false;
    for value in probes {
        let mut state = params.initial_forwards.clone();
        state[2] = value;
        let denominator = 1.0 + params.accrual_factors[2] * state[2];
        let skip = denominator.abs() < 1e-15;
        saw_skip |= skip;
        saw_keep |= !skip;
        runtime_probe_records.push(json!({"state_bits": value.to_bits(), "denominator": denominator, "denominator_bits": denominator.to_bits(), "skipped": skip}));
        compare_drift(
            &params,
            0.0,
            &state,
            json!({"fixture": "denominator_runtime_probe", "state_bits": value.to_bits(), "skipped": skip}),
            &mut errors,
        );
    }
    assert!(saw_skip && saw_keep);
    json!({"errors": errors.record(), "denominator_probes": runtime_probe_records})
}

fn record_fixture(fixture: &Fixture) -> Value {
    let mut grid_hash = Fingerprint::new();
    grid_hash.values(fixture.grid.times());
    let dts: Vec<f64> = (0..fixture.grid.num_steps())
        .map(|i| fixture.grid.dt(i))
        .collect();
    grid_hash.values(&dts);
    let mut alive_steps = 0_u64;
    let mut pair_factor_terms = 0_u64;
    for step in 0..fixture.grid.num_steps() {
        let alive = fixture.params.num_forwards
            - reference::first_alive(&fixture.params, fixture.grid.time(step));
        alive_steps += alive as u64;
        pair_factor_terms +=
            (2 * alive * alive.saturating_sub(1) / 2 * fixture.params.num_factors) as u64;
    }
    json!({
        "id": fixture.id,
        "parameters_fingerprint": json_fingerprint(&fixture.params),
        "grid_fingerprint": grid_hash.finish(),
        "grid_times_bits": fixture.grid.times().iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
        "grid_dt_bits": dts.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
        "forwards": fixture.params.num_forwards,
        "factors": fixture.params.num_factors,
        "steps": fixture.grid.num_steps(),
        "alive_forward_steps_per_leg": alive_steps,
        "drift_calls_per_leg": 2 * fixture.grid.num_steps(),
        "original_pair_factor_terms_per_leg": pair_factor_terms,
    })
}

fn compare_paths(fixture: &Fixture) -> Value {
    let params = &fixture.params;
    let process = LmmProcess::new(params.clone());
    let scheme = LmmPredictorCorrector::new();
    let n = params.num_forwards;
    let mut errors = Errors::new();
    let mut innovations_hash = Fingerprint::new();
    let mut original_hash = Fingerprint::new();
    let mut candidate_hash = Fingerprint::new();
    for seed in SEEDS {
        let root = PhiloxRng::new(seed);
        for stream in 0..PATH_STREAMS {
            let mut rng = root.substream(stream as u64);
            let mut innovations = vec![0.0; fixture.grid.num_steps() * params.num_factors];
            rng.fill_std_normals(&mut innovations);
            innovations_hash.values(&innovations);
            for mirror in [false, true] {
                let mut actual = params.initial_forwards.clone();
                let mut expected = params.initial_forwards.clone();
                let mut work = vec![0.0; scheme.work_size(&process)];
                let mut reference_work = vec![0.0; 3 * n];
                original_hash.values(&expected);
                candidate_hash.values(&actual);
                for step in 0..fixture.grid.num_steps() {
                    let t = fixture.grid.time(step);
                    let dt = fixture.grid.dt(step);
                    let start = step * params.num_factors;
                    let z: Vec<f64> = innovations[start..start + params.num_factors]
                        .iter()
                        .map(|&normal| if mirror { -normal } else { normal })
                        .collect();
                    let before = actual.clone();
                    let reference_before = expected.clone();
                    scheme.step(&process, t, dt, &mut actual, &z, &mut work);
                    reference::step(params, t, dt, &mut expected, &z, &mut reference_work);
                    let first = reference::first_alive(params, t);
                    for i in 0..n {
                        let bound = 1e-13 * expected[i].abs().max(0.01) + 1e-16;
                        errors.observe(actual[i], expected[i], bound, json!({"seed": seed, "stream": stream, "mirror": mirror, "step": step + 1, "forward": i}));
                        assert!(actual[i] + params.displacements[i] > 0.0);
                        assert!(expected[i] + params.displacements[i] > 0.0);
                        assert!(1.0 + params.accrual_factors[i] * actual[i] > 0.0);
                        assert!(1.0 + params.accrual_factors[i] * expected[i] > 0.0);
                        assert_eq!(
                            (1.0 + params.accrual_factors[i] * actual[i]).abs() < 1e-15,
                            (1.0 + params.accrual_factors[i] * expected[i]).abs() < 1e-15
                        );
                        if i < first {
                            assert_eq!(
                                actual[i].to_bits(),
                                before[i].to_bits(),
                                "dead forward must remain frozen"
                            );
                            assert_eq!(expected[i].to_bits(), reference_before[i].to_bits());
                            if params.tenors[i] == 0.0 {
                                assert_eq!(actual[i].to_bits(), expected[i].to_bits());
                            }
                        }
                    }
                    let mut terminal_drift = vec![0.0; n];
                    process.drift(t, &actual, &mut terminal_drift);
                    assert_eq!(terminal_drift[n - 1], 0.0);
                    original_hash.values(&expected);
                    candidate_hash.values(&actual);
                }
            }
        }
    }
    json!({
        "fixture": record_fixture(fixture),
        "streams_per_seed": PATH_STREAMS,
        "legs_per_stream": 2,
        "innovations_fingerprint": innovations_hash.finish(),
        "reference_paths_fingerprint": original_hash.finish(),
        "candidate_paths_fingerprint": candidate_hash.finish(),
        "errors": errors.record(),
    })
}

fn path_panel() -> Vec<Value> {
    let mut cases = fixtures();
    cases.push(Fixture {
        id: "correctness_only_irregular_knot_grid",
        params: knot_params(10),
        grid: TimeGrid::from_times(vec![0.0, 0.2, 0.75, 1.0, 1.25, 1.6, 2.0, 2.2, 3.0])
            .expect("valid irregular correctness grid"),
    });
    for fixture in &cases {
        LmmProcess::new(fixture.params.clone())
            .validate_time_grid(&fixture.grid)
            .expect("correctness grid must contain all model knots");
    }
    cases.iter().map(compare_paths).collect()
}

fn caplet_components(params: &LmmParams, state: &[f64]) -> [f64; 4] {
    // F2 fixes at T2 and pays at T3. D3=P(T2,T3)/P(T2,T40).
    let payoff = NOTIONAL * params.accrual_factors[2] * (state[2] - 0.03).max(0.0);
    let ratio: f64 = (3..params.num_forwards)
        .map(|j| 1.0 + params.accrual_factors[j] * state[j])
        .product();
    let initial_discount = params
        .initial_forwards
        .iter()
        .zip(&params.accrual_factors)
        .map(|(forward, accrual)| 1.0 + accrual * forward)
        .product::<f64>()
        .recip();
    let deflated = payoff * ratio;
    [payoff, ratio, deflated, initial_discount * deflated]
}

fn caplet_seed(params: &LmmParams, seed: u64, streams: usize) -> Value {
    params.validate().expect("valid caplet model");
    let grid = TimeGrid::uniform(2.0, 8).expect("caplet fixing grid");
    let process = LmmProcess::new(params.clone());
    process
        .validate_time_grid(&grid)
        .expect("caplet grid must represent fixing knots");
    let scheme = LmmPredictorCorrector::new();
    let root = PhiloxRng::new(seed);
    let mut reference_stats = OnlineStats::new();
    let mut candidate_stats = OnlineStats::new();
    let mut delta_stats = OnlineStats::new();
    let mut max_path_difference = 0.0_f64;
    let mut worst = Value::Null;
    let mut original_hash = Fingerprint::new();
    let mut candidate_hash = Fingerprint::new();
    let mut innovations_hash = Fingerprint::new();
    for stream in 0..streams {
        let mut rng = root.substream(stream as u64);
        let mut innovations = vec![0.0; grid.num_steps() * params.num_factors];
        rng.fill_std_normals(&mut innovations);
        innovations_hash.values(&innovations);
        let mut actual = params.initial_forwards.clone();
        let mut expected = params.initial_forwards.clone();
        let mut work = vec![0.0; scheme.work_size(&process)];
        let mut reference_work = vec![0.0; 3 * params.num_forwards];
        for step in 0..grid.num_steps() {
            let start = step * params.num_factors;
            let z = &innovations[start..start + params.num_factors];
            scheme.step(
                &process,
                grid.time(step),
                grid.dt(step),
                &mut actual,
                z,
                &mut work,
            );
            reference::step(
                params,
                grid.time(step),
                grid.dt(step),
                &mut expected,
                z,
                &mut reference_work,
            );
        }
        let actual_values = caplet_components(params, &actual);
        let expected_values = caplet_components(params, &expected);
        assert!(
            actual_values
                .iter()
                .chain(&expected_values)
                .all(|value| value.is_finite())
        );
        original_hash.values(&expected_values);
        candidate_hash.values(&actual_values);
        let difference = actual_values[3] - expected_values[3];
        let normalized = difference.abs() / NOTIONAL;
        assert!(normalized <= 1e-10);
        if stream == 0 || normalized > max_path_difference {
            max_path_difference = normalized;
            worst = json!({"stream": stream, "candidate_components": actual_values, "reference_components": expected_values, "pv_difference": difference});
        }
        reference_stats.update(expected_values[3]);
        candidate_stats.update(actual_values[3]);
        delta_stats.update(difference);
    }
    assert!(delta_stats.mean().abs() / NOTIONAL <= 1e-12);
    assert!(delta_stats.stderr() / NOTIONAL <= 1e-12);
    assert!(delta_stats.mean().abs() + 4.0 * delta_stats.stderr() <= 1e-11 * NOTIONAL);
    if reference_stats.stderr() == 0.0 {
        assert_eq!(
            candidate_stats.mean().to_bits(),
            reference_stats.mean().to_bits()
        );
        assert_eq!(max_path_difference, 0.0);
    } else {
        assert!(delta_stats.mean().abs() <= 0.01 * reference_stats.stderr());
    }
    let mut grid_hash = Fingerprint::new();
    grid_hash.values(grid.times());
    json!({
        "seed": seed, "independent_estimators": streams, "physical_paths": streams,
        "antithetic": false, "capture": false, "auto_stop": false,
        "parameters_fingerprint": json_fingerprint(params), "grid_fingerprint": grid_hash.finish(),
        "innovations_fingerprint": innovations_hash.finish(),
        "reference_components_fingerprint": original_hash.finish(), "candidate_components_fingerprint": candidate_hash.finish(),
        "reference_mean": reference_stats.mean(), "reference_stderr": reference_stats.stderr(),
        "candidate_mean": candidate_stats.mean(), "candidate_stderr": candidate_stats.stderr(),
        "paired_mean_delta": delta_stats.mean(), "paired_delta_stderr": delta_stats.stderr(),
        "max_path_pv_difference_per_notional": max_path_difference, "worst": worst,
    })
}

fn caplet_panel() -> Vec<Value> {
    let params = base_params(40, 2);
    let mut records: Vec<Value> = SEEDS
        .into_iter()
        .map(|seed| caplet_seed(&params, seed, CAPLET_STREAMS))
        .collect();
    let mut zero = params;
    zero.vol_values[0].fill([0.0; 3]);
    records
        .push(json!({"zero_standard_error_control": caplet_seed(&zero, SEEDS[0], PATH_STREAMS)}));
    records
}

fn assert_validation(result: Result<()>) {
    assert!(matches!(result, Err(Error::Validation(_))));
}

fn validation_panel() {
    for factors in [1, 4] {
        assert_validation(base_params(4, factors).validate());
    }
    let base = base_params(4, 2);
    let mut invalid = base.clone();
    invalid.initial_forwards[1] = f64::NAN;
    assert_validation(invalid.validate());
    invalid = base.clone();
    invalid.initial_forwards[1] = -invalid.displacements[1];
    assert_validation(invalid.validate());
    invalid = base.clone();
    invalid.vol_values[0].pop();
    assert_validation(invalid.validate());
    invalid = base.clone();
    invalid.vol_values[0][1][0] = f64::INFINITY;
    assert_validation(invalid.validate());
    let process = LmmProcess::new(base.clone());
    let missing_fixing = TimeGrid::from_times(vec![0.0, 0.5, 1.5, 2.0]).expect("valid time grid");
    assert_validation(process.validate_time_grid(&missing_fixing));
    let knot = knot_params(4);
    let knot_process = LmmProcess::new(knot.clone());
    let missing_vol =
        TimeGrid::from_times(vec![0.0, 0.5, 1.0, 1.25, 2.0]).expect("valid time grid");
    assert_validation(knot_process.validate_time_grid(&missing_vol));
    let scheme = LmmPredictorCorrector::new();
    for (params, t, dt) in [(&base, 0.0, 1.25), (&knot, 0.5, 0.5)] {
        let process = LmmProcess::new(params.clone());
        let mut actual = params.initial_forwards.clone();
        let mut expected = actual.clone();
        let mut work = vec![0.0; scheme.work_size(&process)];
        let mut reference_work = vec![0.0; 3 * params.num_forwards];
        scheme.step(&process, t, dt, &mut actual, &[0.4, -0.2], &mut work);
        reference::step(
            params,
            t,
            dt,
            &mut expected,
            &[0.4, -0.2],
            &mut reference_work,
        );
        assert!(actual.iter().all(|value| value.is_nan()));
        assert!(expected.iter().all(|value| value.is_nan()));
    }
    // The displaced boundary is legal as a runtime absorbed state, while the
    // same value above is rejected as an initial validated model.
    let mut actual = base.initial_forwards.clone();
    actual[2] = -base.displacements[2];
    let mut expected = actual.clone();
    let mut work = vec![0.0; 3 * base.num_forwards];
    let mut reference_work = work.clone();
    scheme.step(&process, 0.0, 0.25, &mut actual, &[0.4, -0.2], &mut work);
    reference::step(
        &base,
        0.0,
        0.25,
        &mut expected,
        &[0.4, -0.2],
        &mut reference_work,
    );
    assert_eq!(actual[2].to_bits(), (-base.displacements[2]).to_bits());
    for (&a, &b) in actual.iter().zip(&expected) {
        assert!((a - b).abs() <= 1e-13 * b.abs().max(0.01) + 1e-16);
    }
}

#[test]
fn lmm_original_pairwise_scalar_contract() {
    scalar_panel();
}

#[test]
fn lmm_original_reference_preserves_validation_and_absorption() {
    validation_panel();
}

#[test]
#[ignore = "thorough: CCM-I5 matched-path numerical panel and optional export"]
fn lmm_original_reference_numerical_panel() {
    validation_panel();
    let scalar = scalar_panel();
    let paths = path_panel();
    let caplet = caplet_panel();
    let mut source = Fingerprint::new();
    source.bytes(include_bytes!("lmm_reference.rs"));
    let record = json!({
        "schema": "finstack.ccm.lmm_numerics.v1", "reference_revision": "827ebb75",
        "reference_source_fingerprint": source.finish(), "fingerprint_algorithm": "FNV-1a-64 over declared bytes/little-endian f64 bits; non-cryptographic",
        "seeds": SEEDS, "path_streams_per_seed": PATH_STREAMS,
        "scalar": scalar, "paths": paths, "caplet": caplet,
        "validation_and_absorption_checks": "passed",
        "bounds": {"scalar": "512*EPSILON*absolute_factor_contribution_scale+1e-18", "forward_path": "1e-13*max(.01,abs(reference))+1e-16", "paired_max_pv_per_notional": 1e-10, "paired_mean_per_notional": 1e-12, "paired_stderr_per_notional": 1e-12, "paired_mean_plus_4stderr_per_notional": 1e-11, "mean_fraction_of_original_estimator_stderr": 0.01},
    });
    if let Some(path) = std::env::var_os("CCM_MODEL_NUMERICS_OUTPUT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&record).expect("serialize numerical record"),
        )
        .expect("write requested LMM numerical record");
    }
}
