//! Tests for the path-dependent arms of [`simulate_paths`]: `lmm`,
//! `rough_bergomi`, `rough_heston` and `cheyette_rough`, and the fractional
//! noise choice.

use super::tests::{assert_mean, engine_states, error_message, mean_var, spec, terminal};
use super::*;
use crate::monte_carlo::engine_fractional::simulate_path_fractional;
use crate::monte_carlo::rng::fbm::FractionalNoiseGenerator;
use crate::monte_carlo::rng::volterra::RiemannLiouvilleVolterra;
use crate::monte_carlo::traits::{PathState, Payoff};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
use finstack_quant_core::math::fractional::HurstExponent;
use finstack_quant_core::money::Money;

/// Steps of the shared uniform grid on `[0, 1]`; `0.5` is a grid time, as the
/// LMM fixture needs.
const STEPS: usize = 8;

const FBMS: [FbmSpec; 4] = [
    FbmSpec::Volterra {},
    FbmSpec::Cholesky {},
    FbmSpec::WindowedConditional {
        near_field_size: None,
    },
    FbmSpec::WindowedConditional {
        near_field_size: Some(3),
    },
];

fn hurst() -> HurstExponent {
    HurstExponent::new(0.1).unwrap()
}

/// Three semi-annual forwards fixing at 0.5, 1.0 and 1.5, two factors.
fn lmm_params() -> LmmParams {
    LmmParams {
        num_forwards: 3,
        num_factors: 2,
        tenors: vec![0.5, 1.0, 1.5, 2.0],
        accrual_factors: vec![0.5, 0.5, 0.5],
        displacements: vec![0.005, 0.005, 0.005],
        vol_times: vec![],
        vol_values: vec![vec![
            [0.15, 0.05, 0.0],
            [0.12, 0.08, 0.0],
            [0.10, 0.10, 0.0],
        ]],
        initial_forwards: vec![0.03, 0.032, 0.034],
    }
}

fn rough_bergomi_params() -> RoughBergomiParams {
    let xi = ForwardVarianceCurve::from_points(&[(0.0, 0.04), (1.0, 0.05)]).unwrap();
    RoughBergomiParams::new(0.03, 0.01, hurst(), 1.2, -0.7, xi).unwrap()
}

fn rough_heston_params() -> RoughHestonParams {
    RoughHestonParams {
        r: 0.03,
        q: 0.01,
        hurst: hurst(),
        kappa: 2.0,
        theta: 0.04,
        sigma_v: 0.3,
        rho: -0.7,
        v0: 0.04,
    }
}

fn cheyette_params() -> CheyetteRoughVolParams {
    CheyetteRoughVolParams::new(
        0.03,
        ForwardVarianceCurve::flat(0.005).unwrap(),
        hurst(),
        1.5,
        -0.5,
        &[(0.0, 0.02), (10.0, 0.03)],
    )
    .unwrap()
}

/// Every path-dependent process with a valid initial state, in
/// `ProcessSpec` order.
fn processes() -> Vec<(ProcessSpec, Vec<f64>)> {
    vec![
        (ProcessSpec::Lmm(lmm_params()), vec![0.03, 0.032, 0.034]),
        (
            ProcessSpec::RoughBergomi(rough_bergomi_params()),
            vec![100.0],
        ),
        (
            ProcessSpec::RoughHeston(rough_heston_params()),
            vec![100.0, 0.04],
        ),
        (
            ProcessSpec::CheyetteRough(cheyette_params()),
            vec![0.0, 0.0],
        ),
    ]
}

fn uses_fbm(process: &ProcessSpec) -> bool {
    matches!(
        process,
        ProcessSpec::RoughBergomi(_) | ProcessSpec::CheyetteRough(_)
    )
}

/// A small spec per process, and per fractional generator where one applies.
fn small_specs() -> Vec<PathSimulationSpec> {
    let mut specs = Vec::new();
    for (process, x0) in processes() {
        let mut s = spec(process, &x0, STEPS);
        s.num_paths = 7;
        if uses_fbm(&s.process) {
            for fbm in FBMS {
                s.fbm = Some(fbm);
                specs.push(s.clone());
            }
        }
        s.fbm = None;
        specs.push(s);
    }
    specs
}

fn label(spec: &PathSimulationSpec) -> String {
    format!("{} + {:?}", spec.process.tag(), spec.fbm)
}

// ---------------------------------------------------------------------------
// Shape, names, determinism, wire format
// ---------------------------------------------------------------------------

#[test]
fn tags_shapes_and_factor_names_follow_the_state_layout() {
    let expected: [(&str, &[&str]); 4] = [
        ("lmm", &["forward_0", "forward_1", "forward_2"]),
        ("rough_bergomi", &["spot"]),
        ("rough_heston", &["spot", "variance"]),
        ("cheyette_rough", &["x", "y"]),
    ];
    for ((process, x0), (tag, names)) in processes().into_iter().zip(expected) {
        assert_eq!(serde_json::to_value(&process).unwrap()["type"], tag);
        assert_eq!(process.tag(), tag);
        let mut s = spec(process, &x0, STEPS);
        s.num_paths = 5;
        for antithetic in [false, true] {
            s.antithetic = antithetic;
            let out = simulate_paths(&s).unwrap();
            let stored = if antithetic { 10 } else { 5 };
            assert_eq!(out.factor_names, names, "{tag}");
            assert_eq!(out.dim, x0.len(), "{tag}");
            assert_eq!(out.num_paths, 5);
            assert_eq!(out.num_simulated_paths, stored);
            assert_eq!(out.times.len(), STEPS + 1);
            assert_eq!(out.values.len(), stored * (STEPS + 1) * x0.len());
            for path in out.values.chunks((STEPS + 1) * x0.len()) {
                assert_eq!(path[..x0.len()], x0[..], "{tag}");
            }
        }
    }
}

#[test]
fn same_spec_reproduces_and_a_new_seed_or_generator_differs() {
    for s in small_specs() {
        let first = simulate_paths(&s).unwrap();
        assert_eq!(first, simulate_paths(&s).unwrap(), "{}", label(&s));
        let mut reseeded = s.clone();
        reseeded.seed += 1;
        assert_ne!(
            first.values,
            simulate_paths(&reseeded).unwrap().values,
            "{}",
            label(&s)
        );
    }
    for (process, x0) in processes() {
        if !uses_fbm(&process) {
            continue;
        }
        let mut s = spec(process, &x0, STEPS);
        s.num_paths = 3;
        let default = simulate_paths(&s).unwrap();
        let mut runs = Vec::new();
        for fbm in FBMS {
            s.fbm = Some(fbm);
            runs.push(simulate_paths(&s).unwrap().values);
        }
        // An omitted generator is the Volterra one; the others change the paths.
        assert_eq!(default.values, runs[0]);
        for other in &runs[1..] {
            assert_ne!(&runs[0], other, "{}", s.process.tag());
        }
        assert_ne!(runs[2], runs[3], "window length must matter");
    }
}

#[test]
fn serial_and_parallel_runs_are_identical() {
    for mut s in small_specs() {
        for antithetic in [false, true] {
            s.antithetic = antithetic;
            assert_eq!(
                dispatch(&s, false).unwrap(),
                dispatch(&s, true).unwrap(),
                "{}",
                label(&s)
            );
        }
    }
}

#[test]
fn stream_values_do_not_depend_on_the_number_of_streams_or_on_pairing() {
    for few in small_specs() {
        let mut many = few.clone();
        many.num_paths = 12;
        let (few_out, many_out) = (
            simulate_paths(&few).unwrap(),
            simulate_paths(&many).unwrap(),
        );
        assert_eq!(
            few_out.values,
            many_out.values[..few_out.values.len()],
            "{}",
            label(&few)
        );

        // The first leg of every antithetic pair is the unpaired stream.
        let mut paired = few.clone();
        paired.antithetic = true;
        let paired_out = simulate_paths(&paired).unwrap();
        let path_len = few_out.times.len() * few_out.dim;
        for (stream, path) in few_out.values.chunks(path_len).enumerate() {
            let start = 2 * stream * path_len;
            assert_eq!(
                path,
                &paired_out.values[start..start + path_len],
                "{}",
                label(&few)
            );
            assert_ne!(
                path,
                &paired_out.values[start + path_len..start + 2 * path_len],
                "{}",
                label(&few)
            );
        }
    }
}

#[test]
fn specs_round_trip_through_json_without_changing_the_paths() {
    for s in small_specs() {
        let json = serde_json::to_string(&s).unwrap();
        let restored: PathSimulationSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(
            simulate_paths(&s).unwrap(),
            simulate_paths(&restored).unwrap(),
            "{json}"
        );
    }
    for (fbm, json) in FBMS.into_iter().zip([
        r#"{"type":"volterra"}"#,
        r#"{"type":"cholesky"}"#,
        r#"{"type":"windowed_conditional","near_field_size":null}"#,
        r#"{"type":"windowed_conditional","near_field_size":3}"#,
    ]) {
        assert_eq!(serde_json::to_string(&fbm).unwrap(), json);
        assert_eq!(serde_json::from_str::<FbmSpec>(json).unwrap(), fbm);
    }
    assert_eq!(
        serde_json::from_str::<FbmSpec>(r#"{"type":"windowed_conditional"}"#).unwrap(),
        FBMS[2]
    );
}

#[test]
fn json_specs_need_every_curve_and_reject_unknown_fields() {
    let parse = |process: serde_json::Value| {
        serde_json::from_value::<PathSimulationSpec>(serde_json::json!({
            "process": process,
            "initial_state": [100.0],
            "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 4},
            "num_paths": 2,
            "seed": 1,
        }))
    };
    let tagged = |process: &ProcessSpec| serde_json::to_value(process).unwrap();

    let bergomi = tagged(&ProcessSpec::RoughBergomi(rough_bergomi_params()));
    assert_eq!(bergomi["hurst"], serde_json::json!({"h": 0.1}));
    assert!(parse(bergomi.clone()).is_ok());
    // The forward variance curve has no default.
    let mut missing = bergomi.clone();
    missing.as_object_mut().unwrap().remove("xi");
    assert!(parse(missing).is_err());
    let mut bad_hurst = bergomi;
    bad_hurst["hurst"]["h"] = serde_json::json!(1.5);
    assert!(parse(bad_hurst).is_err());

    for (process, _) in processes() {
        let mut value = tagged(&process);
        value["unknown"] = serde_json::json!(1.0);
        assert!(parse(value).is_err(), "{}", process.tag());
    }
    assert!(serde_json::from_str::<FbmSpec>(r#"{"type":"hybrid"}"#).is_err());
    assert!(serde_json::from_str::<FbmSpec>(r#"{"type":"cholesky","near_field_size":3}"#).is_err());
}

// ---------------------------------------------------------------------------
// Agreement with the production drivers
// ---------------------------------------------------------------------------

/// Records the spot seen at every payoff event.
#[derive(Debug, Clone, Default)]
struct RecordSpot {
    spots: Vec<f64>,
}

impl Payoff for RecordSpot {
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        self.spots.extend(state.spot());
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
        Money::new(0.0, currency)
    }

    fn reset(&mut self) {
        self.spots.clear();
    }
}

/// Each rough Bergomi stream is bit for bit the path that
/// `simulate_path_fractional` produces when driven the way the equity option
/// pricer drives it (generator normals, Volterra increments, driving normals,
/// then the injected step loop) on that stream's Philox substream.
#[test]
fn rough_bergomi_streams_match_the_pricer_path_function() {
    let (expiry, num_steps, num_paths, seed) = (0.75, 12, 6, 77);
    let params = rough_bergomi_params();
    let mut s = spec(
        ProcessSpec::RoughBergomi(params.clone()),
        &[100.0],
        num_steps,
    );
    s.time_grid = TimeGridSpec::Uniform { expiry, num_steps };
    s.num_paths = num_paths;
    s.seed = seed;
    let out = simulate_paths(&s).unwrap();

    let process = RoughBergomiProcess::new(params);
    let disc = RoughBergomiEuler::new(hurst());
    let fbm_gen = RiemannLiouvilleVolterra::new(expiry, num_steps, 0.1).unwrap();
    let time_grid = TimeGrid::uniform(expiry, num_steps).unwrap();
    let root = PhiloxRng::new(seed);
    let mut expected = Vec::new();
    for path in 0..num_paths {
        let mut rng = root.substream(path as u64);
        let mut fbm_normals = vec![0.0; num_steps * fbm_gen.normals_per_step()];
        let mut fbm_increments = vec![0.0; num_steps];
        let mut driving_normals = vec![0.0; num_steps];
        rng.fill_std_normals(&mut fbm_normals);
        fbm_gen.generate(&fbm_normals, &mut fbm_increments);
        fbm_gen.driving_normals_into(&fbm_normals, &mut driving_normals);

        let mut payoff = RecordSpot::default();
        simulate_path_fractional(
            &mut rng,
            &time_grid,
            &process,
            &disc,
            &[100.0],
            &mut payoff,
            Currency::USD,
            &fbm_increments,
            1,
            Some((2, &driving_normals)),
            &mut [0.0],
            &mut [0.0; 3],
            &mut [0.0],
        )
        .unwrap();
        expected.extend(payoff.spots);
    }
    assert_eq!(out.values.len(), num_paths * (num_steps + 1));
    assert_eq!(
        out.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}

/// The LMM arm is the loop of the Bermudan swaption pricer: one substream per
/// path, one vector of normals per step, no engine-side correlation.
#[test]
fn lmm_streams_match_the_pricer_loop() {
    let params = lmm_params();
    let mut s = spec(
        ProcessSpec::Lmm(params.clone()),
        &params.initial_forwards,
        STEPS,
    );
    s.num_paths = 5;
    let out = simulate_paths(&s).unwrap();

    let process = LmmProcess::new(params.clone().validate().unwrap());
    let disc = LmmPredictorCorrector::new();
    let time_grid = TimeGrid::uniform(1.0, STEPS).unwrap();
    let root = PhiloxRng::new(s.seed);
    let mut expected = Vec::new();
    for path in 0..5 {
        let mut rng = root.substream(path);
        let mut x = params.initial_forwards.clone();
        let mut work = vec![0.0; disc.work_size(&process)];
        let mut z = vec![0.0; params.num_factors];
        expected.extend_from_slice(&x);
        for step in 0..STEPS {
            rng.fill_std_normals(&mut z);
            disc.step(
                &process,
                time_grid.time(step),
                time_grid.dt(step),
                &mut x,
                &z,
                &mut work,
            );
            expected.extend_from_slice(&x);
        }
    }
    assert_eq!(out.values, expected);
}

/// The rough Heston arm is `McEngine` with the hybrid scheme built on the
/// pricer's `expiry · i / n` kernel times.
#[test]
fn rough_heston_streams_match_the_engine() {
    let mut s = spec(
        ProcessSpec::RoughHeston(rough_heston_params()),
        &[100.0, 0.04],
        6,
    );
    s.num_paths = 8;
    let process = RoughHestonProcess::new(rough_heston_params().validate().unwrap());
    let times: Vec<f64> = (0..=6).map(|i| 1.0 * i as f64 / 6.0).collect();
    let scheme = RoughHestonHybrid::new(&times, 0.1).unwrap();
    assert_eq!(
        simulate_paths(&s).unwrap().values,
        engine_states(&process, &scheme, &[100.0, 0.04], 8)
    );
}

// ---------------------------------------------------------------------------
// Model properties
// ---------------------------------------------------------------------------

const N: usize = 20_000;

/// The left-point log-Euler step makes the discounted spot an exact
/// martingale for every generator: the step's variance depends only on
/// earlier normals.
#[test]
fn rough_bergomi_spot_recovers_the_forward_under_every_generator() {
    for fbm in FBMS {
        for antithetic in [false, true] {
            let mut s = spec(
                ProcessSpec::RoughBergomi(rough_bergomi_params()),
                &[100.0],
                STEPS,
            );
            s.num_paths = N;
            s.fbm = Some(fbm);
            s.antithetic = antithetic;
            let out = simulate_paths(&s).unwrap();
            assert!(out.values.iter().all(|spot| *spot > 0.0));
            let mut spots = terminal(&out, 0);
            if antithetic {
                // Pair averages are the independent samples.
                spots = spots
                    .chunks(2)
                    .map(|pair| 0.5 * (pair[0] + pair[1]))
                    .collect();
            }
            assert_mean(
                &spots,
                100.0 * (0.02_f64).exp(),
                &format!("rough Bergomi forward, {fbm:?}, antithetic {antithetic}"),
            );
        }
    }
}

#[test]
fn rough_heston_variance_is_non_negative_and_spot_recovers_the_forward() {
    let mut s = spec(
        ProcessSpec::RoughHeston(rough_heston_params()),
        &[100.0, 0.04],
        16,
    );
    s.num_paths = N;
    let out = simulate_paths(&s).unwrap();
    for state in out.values.chunks(2) {
        assert!(state[0] > 0.0 && state[1] >= 0.0, "{state:?}");
    }
    assert_mean(
        &terminal(&out, 0),
        100.0 * (0.02_f64).exp(),
        "rough Heston forward",
    );
}

/// Under the terminal measure the last forward is a martingale, displaced
/// forwards stay positive, and a forward is frozen from its fixing date.
#[test]
fn lmm_forwards_fix_stay_positive_and_the_last_is_a_martingale() {
    let params = lmm_params();
    let mut s = spec(
        ProcessSpec::Lmm(params.clone()),
        &params.initial_forwards,
        STEPS,
    );
    s.num_paths = N;
    let out = simulate_paths(&s).unwrap();
    let fixing_step = STEPS / 2;
    for path in out.values.chunks((STEPS + 1) * 3) {
        for state in path.chunks(3) {
            assert!(state.iter().all(|forward| forward + 0.005 > 0.0));
        }
        let fixed = path[fixing_step * 3];
        assert_ne!(fixed, 0.03);
        for step in fixing_step..=STEPS {
            assert_eq!(path[step * 3], fixed);
        }
    }
    assert_mean(&terminal(&out, 2), 0.034, "last LMM forward");
    let (_, var) = mean_var(&terminal(&out, 2));
    assert!(var > 0.0);
}

/// `E[σ²(t)] = σ₀²` whenever the driver's level variance is `t^(2H)`, so the
/// mean of `y` follows the deterministic recursion
/// `y ← y + (σ₀² − 2κ y) Δt`. A window shorter than the path truncates the
/// level variance and is exempt.
#[test]
fn cheyette_rough_accumulated_variance_has_the_compensated_mean() {
    let (kappa, sigma0, dt) = (0.03, 0.005_f64, 1.0 / STEPS as f64);
    let expected = (0..STEPS).fold(0.0, |y, _| y + (sigma0 * sigma0 - 2.0 * kappa * y) * dt);
    for fbm in FBMS {
        let mut s = spec(
            ProcessSpec::CheyetteRough(cheyette_params()),
            &[0.0, 0.0],
            STEPS,
        );
        s.num_paths = N;
        s.fbm = Some(fbm);
        let out = simulate_paths(&s).unwrap();
        assert!(out.values.iter().all(|value| value.is_finite()));
        for state in out.values.chunks(2) {
            assert!(state[1] >= 0.0);
        }
        if fbm != FBMS[3] {
            assert_mean(
                &terminal(&out, 1),
                expected,
                &format!("Cheyette y, {fbm:?}"),
            );
        }
        let (_, var_x) = mean_var(&terminal(&out, 0));
        assert!(var_x > 0.0);
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn fbm_is_rejected_for_processes_without_fractional_noise() {
    let gbm = ProcessSpec::Gbm(GbmParams::new(0.03, 0.0, 0.2).unwrap());
    let mut candidates = vec![(gbm, vec![100.0])];
    candidates.extend(
        processes()
            .into_iter()
            .filter(|(process, _)| !uses_fbm(process)),
    );
    assert_eq!(candidates.len(), 3);
    for (process, x0) in candidates {
        let mut s = spec(process, &x0, STEPS);
        s.num_paths = 2;
        for fbm in FBMS {
            s.fbm = Some(fbm);
            let message = error_message(&s);
            assert!(
                message.contains("fbm") && message.contains(s.process.tag()),
                "{message}"
            );
        }
    }
}

#[test]
fn only_the_default_scheme_is_available() {
    for (process, x0) in processes() {
        for scheme in [
            SchemeSpec::Euler,
            SchemeSpec::LogEuler,
            SchemeSpec::Milstein,
        ] {
            let mut s = spec(process.clone(), &x0, STEPS);
            s.num_paths = 2;
            s.scheme = scheme;
            let message = error_message(&s);
            assert!(
                message.contains("is not available for process")
                    && message.contains(s.process.tag()),
                "{message}"
            );
        }
    }
}

#[test]
fn volterra_needs_a_uniform_grid_and_the_exact_generators_do_not() {
    let mut s = spec(
        ProcessSpec::RoughBergomi(rough_bergomi_params()),
        &[100.0],
        STEPS,
    );
    s.num_paths = 2;
    s.time_grid = TimeGridSpec::Times {
        times: vec![0.0, 0.1, 0.3, 1.0],
    };
    assert!(error_message(&s).contains("equally spaced"));
    for fbm in &FBMS[1..] {
        s.fbm = Some(*fbm);
        assert_eq!(simulate_paths(&s).unwrap().times, [0.0, 0.1, 0.3, 1.0]);
    }
    s.fbm = None;
    s.time_grid = TimeGridSpec::Times {
        times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
    };
    assert_eq!(simulate_paths(&s).unwrap().times.len(), 5);
}

#[test]
fn initial_states_must_agree_with_the_parameters() {
    let mut s = spec(ProcessSpec::Lmm(lmm_params()), &[0.03, 0.032, 0.035], STEPS);
    s.num_paths = 2;
    assert!(error_message(&s).contains("initial_forwards"));
    s.initial_state = vec![0.03, 0.032];
    assert!(error_message(&s).contains("initial_state must hold 3"));

    let mut s = spec(
        ProcessSpec::RoughHeston(rough_heston_params()),
        &[100.0, 0.05],
        STEPS,
    );
    s.num_paths = 2;
    assert!(error_message(&s).contains("v0"));
    s.initial_state = vec![-1.0, 0.04];
    assert!(error_message(&s).contains("strictly positive"));

    let mut s = spec(
        ProcessSpec::RoughBergomi(rough_bergomi_params()),
        &[0.0],
        STEPS,
    );
    s.num_paths = 2;
    assert!(error_message(&s).contains("strictly positive"));
}

#[test]
fn out_of_range_parameters_and_grids_are_errors() {
    let run = |process: ProcessSpec, x0: &[f64]| {
        let mut s = spec(process, x0, STEPS);
        s.num_paths = 2;
        error_message(&s)
    };

    let mut bergomi = rough_bergomi_params();
    bergomi.eta = -1.0;
    assert!(run(ProcessSpec::RoughBergomi(bergomi), &[100.0]).contains("eta"));

    let mut heston = rough_heston_params();
    heston.kappa = 0.0;
    assert!(run(ProcessSpec::RoughHeston(heston), &[100.0, 0.04]).contains("kappa"));
    // The hybrid kernel is defined for rough exponents only.
    let mut heston = rough_heston_params();
    heston.hurst = HurstExponent::new(0.6).unwrap();
    assert!(run(ProcessSpec::RoughHeston(heston), &[100.0, 0.04]).contains("Hurst"));

    let mut cheyette = cheyette_params();
    cheyette.rho = 1.5;
    assert!(run(ProcessSpec::CheyetteRough(cheyette), &[0.0, 0.0]).contains("rho"));

    let mut lmm = lmm_params();
    lmm.num_factors = 5;
    assert!(run(ProcessSpec::Lmm(lmm), &[0.03, 0.032, 0.034]).contains("factors"));
    // A fixing date inside the horizon must be a grid time.
    let mut s = spec(
        ProcessSpec::Lmm(lmm_params()),
        &[0.03, 0.032, 0.034],
        STEPS + 1,
    );
    s.num_paths = 2;
    assert!(error_message(&s).contains("fixing date"));

    // Quadratic generator and kernel storage is bounded before allocation.
    let mut s = spec(
        ProcessSpec::RoughHeston(rough_heston_params()),
        &[100.0, 0.04],
        8_001,
    );
    s.num_paths = 1;
    assert!(error_message(&s).contains("at most 8000 steps"));
    let mut s = spec(
        ProcessSpec::RoughBergomi(rough_bergomi_params()),
        &[100.0],
        8_001,
    );
    s.num_paths = 1;
    s.fbm = Some(FbmSpec::Cholesky {});
    assert!(error_message(&s).contains("generator weights"));
    s.time_grid = TimeGridSpec::Uniform {
        expiry: 1.0,
        num_steps: STEPS,
    };
    s.fbm = Some(FbmSpec::WindowedConditional {
        near_field_size: Some(0),
    });
    assert!(error_message(&s).contains("window"));
}
