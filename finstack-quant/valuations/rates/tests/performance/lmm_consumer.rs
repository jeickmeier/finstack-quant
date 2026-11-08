//! Frozen CCM-I5 consumer panel; original dynamics and bond-ratio oracle.

use super::*;
use crate::swaption::lmm_pricer::BermudanSwaptionLmmPricer;
use crate::swaption::{BermudanSchedule, BermudanSwaption, BermudanSwaptionParams};
use finstack_quant_core::canonical::content_hash;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::money::Money;
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::pricer::Pricer;
use serde_json::{Value, json};
use time::macros::date;

// One immutable test-only original dynamics reference, shared with models.
#[path = "../../../../models/tests/performance/lmm_reference.rs"]
#[allow(dead_code)]
mod original;

const SEEDS: [u64; 3] = [7, 42, 20_261_107];
const NOTIONAL: f64 = 1_000_000.0;
const PATH_RELATIVE_BOUND: f64 = 1e-13;
const PATH_ABSOLUTE_BOUND: f64 = 1e-16;
const MAX_PATH_PV_PER_NOTIONAL: f64 = 1e-10;
const MEAN_PV_PER_NOTIONAL: f64 = 1e-12;
const STDERR_PV_PER_NOTIONAL: f64 = 1e-12;
const PAIRED_ENVELOPE_PER_NOTIONAL: f64 = 1e-11;

struct Fixture {
    as_of: Date,
    option: BermudanSwaption,
    market: MarketContext,
    params: LmmParams,
    exercises: Vec<f64>,
    grid: TimeGrid,
    indices: Vec<usize>,
    swap: LmmBermudanSwap,
    config: Hw1fMcConfig,
}

fn fixture(two_exercises: bool, seed: u64) -> Fixture {
    let as_of = date!(2025 - 01 - 02);
    let start = date!(2026 - 01 - 02);
    let end = date!(2036 - 01 - 02);
    let dates = if two_exercises {
        vec![start, date!(2028 - 01 - 02)]
    } else {
        vec![start]
    };
    let mut option = BermudanSwaption::new(BermudanSwaptionParams {
        id: if two_exercises {
            "CCM-LMM-two_exercise_20f_2048_estimators"
        } else {
            "CCM-LMM-single_exercise_20f_2048_estimators"
        }
        .into(),
        option_type: OptionType::Call,
        notional: Money::new(NOTIONAL, Currency::USD).expect("notional"),
        strike: 0.03,
        underlying_start_date: start,
        underlying_maturity: end,
        exercise_schedule: BermudanSchedule::new(dates),
        discount_curve_id: "D".into(),
        forward_curve_id: "D".into(),
        vol_surface_id: "UNUSED".into(),
    })
    .expect("supported LMM contract");
    option.underlying_fixed_leg.frequency = Tenor::semi_annual();
    option.underlying_fixed_leg.day_count = DayCount::Act360;
    option.underlying_fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    option.underlying_float_leg.frequency = Tenor::semi_annual();
    option.underlying_float_leg.day_count = DayCount::Act360;
    option.underlying_float_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    option
        .instrument_pricing_overrides
        .model_config
        .lmm_base_vol = Some(0.15);
    let curve = DiscountCurve::builder("D")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([0.0_f64, 1.0, 3.0, 7.0, 12.0].map(|t| (t, (-0.03 * t).exp())))
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("single-curve market");
    let market = MarketContext::new().insert(curve);
    let curve = market.get_discount("D").expect("discount curve");
    let params = BermudanSwaptionLmmPricer::build_lmm_params(&option, curve.as_ref(), as_of, 0.15)
        .expect("canonical LMM params");
    assert_eq!(params.num_forwards, 20);
    assert_eq!(params.num_factors, 2);
    let config = Hw1fMcConfig {
        num_paths: 2_048,
        seed,
        antithetic: true,
        min_steps_between_events: 4,
        basis_degree: 2,
        oos_lsmc: false,
    };
    let exercises = option.exercise_times(as_of).expect("exercise model times");
    let (grid, indices) =
        build_exercise_aligned_grid(&exercises, &params, 4).expect("canonical grid");
    for (&exercise, &index) in exercises.iter().zip(&indices) {
        assert_eq!(exercise.to_bits(), grid.time(index).to_bits());
    }
    let swap = LmmBermudanSwap {
        strike: 0.03,
        payer: true,
        notional: NOTIONAL,
        discount_factor_terminal: curve
            .df_between_dates(as_of, end)
            .expect("terminal discount"),
        currency: Currency::USD,
    };
    Fixture {
        as_of,
        option,
        market,
        params,
        exercises,
        grid,
        indices,
        swap,
        config,
    }
}

type ExerciseStates = Vec<Vec<Vec<f64>>>;

struct PairedStates {
    current: ExerciseStates,
    original: ExerciseStates,
    max_path_error: f64,
    path_rms_error: f64,
    worst: (usize, usize, usize),
    innovations: u64,
}

fn fingerprint_values(hash: &mut u64, values: &[f64]) {
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
}

fn state_fingerprint(states: &ExerciseStates) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for path in states {
        for state in path {
            fingerprint_values(&mut hash, state);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn simulate_paired(f: &Fixture) -> PairedStates {
    let process = LmmProcess::new(f.params.clone());
    let scheme = LmmPredictorCorrector::new();
    let root = PhiloxRng::new(f.config.seed);
    let mut result = PairedStates {
        current: Vec::with_capacity(f.config.simulated_path_count()),
        original: Vec::with_capacity(f.config.simulated_path_count()),
        max_path_error: 0.0,
        path_rms_error: 0.0,
        worst: (0, 0, 0),
        innovations: 0xcbf2_9ce4_8422_2325,
    };
    let mut sum_squares = 0.0;
    let mut coordinates = 0usize;
    for stream in 0..f.config.num_paths {
        for leg in 0..2 {
            let mut rng = root.substream(stream as u64);
            let mut current = f.params.initial_forwards.clone();
            let mut old = current.clone();
            let mut work = vec![0.0; 3 * f.params.num_forwards];
            let mut old_work = work.clone();
            let mut z = vec![0.0; f.params.num_factors];
            let mut current_exercises = vec![Vec::new(); f.exercises.len()];
            let mut original_exercises = current_exercises.clone();
            for step in 0..f.grid.num_steps() {
                rng.fill_std_normals(&mut z);
                if leg == 1 {
                    for shock in &mut z {
                        *shock = -*shock;
                    }
                }
                fingerprint_values(&mut result.innovations, &z);
                let time = f.grid.time(step);
                let dt = f.grid.dt(step);
                let frozen = original::first_alive(&f.params, time);
                let before = current[..frozen].to_vec();
                let old_before = old[..frozen].to_vec();
                scheme.step(&process, time, dt, &mut current, &z, &mut work);
                original::step(&f.params, time, dt, &mut old, &z, &mut old_work);
                for i in 0..f.params.num_forwards {
                    assert!(current[i].is_finite() && old[i].is_finite());
                    assert!(current[i] + f.params.displacements[i] > 0.0);
                    let error = (current[i] - old[i]).abs();
                    let bound = PATH_RELATIVE_BOUND * old[i].abs().max(0.01) + PATH_ABSOLUTE_BOUND;
                    assert!(
                        error <= bound,
                        "stream={stream} leg={leg} step={step} forward={i}: {error}>{bound}"
                    );
                    if i < frozen {
                        assert_eq!(current[i].to_bits(), before[i].to_bits());
                        assert_eq!(old[i].to_bits(), old_before[i].to_bits());
                    }
                    if error > result.max_path_error {
                        result.max_path_error = error;
                        result.worst = (2 * stream + leg, step + 1, i);
                    }
                    sum_squares += error * error;
                    coordinates += 1;
                }
                for (ex, &index) in f.indices.iter().enumerate() {
                    if step + 1 == index {
                        current_exercises[ex] = current.clone();
                        original_exercises[ex] = old.clone();
                    }
                }
            }
            assert!(
                current_exercises
                    .iter()
                    .all(|state| state.len() == f.params.num_forwards)
            );
            result.current.push(current_exercises);
            result.original.push(original_exercises);
        }
    }
    result.path_rms_error = (sum_squares / coordinates as f64).sqrt();
    result
}

struct PolicyValues {
    values: Vec<f64>,
    choices: Vec<usize>,
}

fn policy_values(states: &ExerciseStates, f: &Fixture) -> PolicyValues {
    assert!(!f.config.oos_lsmc && f.config.basis_degree == 2);
    let basis = |sr: f64, ann: f64| [1.0, sr, ann, sr * sr, sr * ann];
    let mut result = PolicyValues {
        values: vec![0.0; states.len()],
        choices: vec![usize::MAX; states.len()],
    };
    for ex in (0..f.exercises.len()).rev() {
        let first = first_alive_forward(&f.params.tenors[..f.params.num_forwards], f.exercises[ex]);
        let mut exercise_values = Vec::with_capacity(states.len());
        let mut features = Vec::with_capacity(states.len());
        for path in states {
            let forwards = &path[ex];
            let (rate, annuity) = compute_swap_rate_and_annuity(
                forwards,
                &f.params.accrual_factors,
                first,
                f.params.num_forwards,
            );
            let intrinsic = (rate - f.swap.strike) * annuity * NOTIONAL;
            let numeraire = pathwise_terminal_numeraire(forwards, &f.params, f.exercises[ex]);
            exercise_values.push(if numeraire > 0.0 {
                intrinsic / numeraire
            } else {
                0.0
            });
            features.push((rate, annuity));
        }
        if ex == f.exercises.len() - 1 {
            for (i, &value) in exercise_values.iter().enumerate() {
                if value > 0.0 {
                    result.values[i] = value;
                    result.choices[i] = ex;
                }
            }
            continue;
        }
        let itm: Vec<_> = exercise_values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v > 0.0).then_some(i))
            .collect();
        if itm.len() > f.config.basis_degree + 3 {
            let mut design = Vec::with_capacity(itm.len() * 5);
            let mut continuation = Vec::with_capacity(itm.len());
            for &i in &itm {
                design.extend_from_slice(&basis(features[i].0, features[i].1));
                continuation.push(result.values[i]);
            }
            let coefficients = solve_least_squares(&design, &continuation, itm.len(), 5)
                .expect("unchanged SVD regression");
            for &i in &itm {
                let continuation: f64 = basis(features[i].0, features[i].1)
                    .iter()
                    .zip(&coefficients)
                    .map(|(a, b)| a * b)
                    .sum();
                if exercise_values[i] > continuation {
                    result.values[i] = exercise_values[i];
                    result.choices[i] = ex;
                }
            }
        } else {
            for &i in &itm {
                if exercise_values[i] > result.values[i] {
                    result.values[i] = exercise_values[i];
                    result.choices[i] = ex;
                }
            }
        }
    }
    result
}

fn independent_deflated_payoff(forwards: &[f64], f: &Fixture) -> f64 {
    // Physical co-terminal membership and inseparable payoff/deflator from
    // D_j=P(t,T_j)/P(t,T_N). This does not call either production payoff helper.
    let first =
        f.params.tenors[..f.params.num_forwards].partition_point(|&tenor| tenor < f.exercises[0]);
    let mut ratios = vec![1.0; f.params.num_forwards + 1];
    for j in (first..f.params.num_forwards).rev() {
        ratios[j] = ratios[j + 1] * (1.0 + f.params.accrual_factors[j] * forwards[j]);
    }
    let annuity: f64 = (first..f.params.num_forwards)
        .map(|j| f.params.accrual_factors[j] * ratios[j + 1])
        .sum();
    ((ratios[first] - 1.0) / annuity - f.swap.strike).max(0.0) * annuity * NOTIONAL
}

fn check_case(two_exercises: bool, seed: u64) -> Value {
    let f = fixture(two_exercises, seed);
    let paths = simulate_paired(&f);
    let current = policy_values(&paths.current, &f);
    let old = policy_values(&paths.original, &f);
    assert_eq!(
        current.choices, old.choices,
        "frozen panel exercise decisions"
    );
    let mut differences = OnlineStats::new();
    let mut reference_stats = OnlineStats::new();
    let mut current_stats = OnlineStats::new();
    let mut maximum_path_difference = 0.0_f64;
    for (now, before) in current
        .values
        .chunks_exact(2)
        .zip(old.values.chunks_exact(2))
    {
        for leg in 0..2 {
            let error = f.swap.discount_factor_terminal * (now[leg] - before[leg]).abs();
            maximum_path_difference = maximum_path_difference.max(error);
            assert!(error / NOTIONAL <= MAX_PATH_PV_PER_NOTIONAL);
        }
        let before = f.swap.discount_factor_terminal * (0.5 * before[0] + 0.5 * before[1]);
        let now = f.swap.discount_factor_terminal * (0.5 * now[0] + 0.5 * now[1]);
        reference_stats.update(before);
        current_stats.update(now);
        differences.update(now - before);
    }
    assert!(differences.mean().abs() / NOTIONAL <= MEAN_PV_PER_NOTIONAL);
    assert!(differences.stderr() / NOTIONAL <= STDERR_PV_PER_NOTIONAL);
    assert!(
        (differences.mean().abs() + 4.0 * differences.stderr()) / NOTIONAL
            <= PAIRED_ENVELOPE_PER_NOTIONAL
    );
    if reference_stats.stderr() > 0.0 {
        assert!(differences.mean().abs() <= 0.01 * reference_stats.stderr());
    } else {
        assert_eq!(differences.mean(), 0.0);
        assert_eq!(differences.stderr(), 0.0);
        assert_eq!(maximum_path_difference, 0.0);
        assert_eq!(
            current_stats.mean().to_bits(),
            reference_stats.mean().to_bits()
        );
    }
    let mut ratio_error = 0.0_f64;
    if !two_exercises {
        for (state, &value) in paths.original.iter().zip(&old.values) {
            let independent = independent_deflated_payoff(&state[0], &f);
            let error = f.swap.discount_factor_terminal * (independent - value).abs();
            ratio_error = ratio_error.max(error);
            assert!(error / NOTIONAL <= MEAN_PV_PER_NOTIONAL);
        }
    }
    let engine = price_bermudan_lmm(&f.params, &f.exercises, &f.swap, &f.config)
        .expect("private engine price");
    assert_eq!(engine.mean.currency(), Currency::USD);
    assert_eq!(engine.ci_95.0.currency(), Currency::USD);
    assert_eq!(engine.ci_95.1.currency(), Currency::USD);
    assert_eq!(engine.num_paths, 2_048);
    assert_eq!(engine.num_simulated_paths, 4_096);
    assert!((engine.mean.amount() - current_stats.mean()).abs() / NOTIONAL <= MEAN_PV_PER_NOTIONAL);
    assert!((engine.stderr - current_stats.stderr()).abs() / NOTIONAL <= STDERR_PV_PER_NOTIONAL);
    let public = BermudanSwaptionLmmPricer::with_config(f.config)
        .price_dyn(&f.option, &f.market, f.as_of)
        .expect("complete public consumer");
    assert_eq!(public.value.currency(), Currency::USD);
    assert!(
        (public.value.amount() - engine.mean.amount()).abs() / NOTIONAL <= MEAN_PV_PER_NOTIONAL
    );
    json!({
        "case":if two_exercises {"mc_lmm_consumer/two_exercise_20f_2048_estimators"} else {"mc_lmm_consumer/single_exercise_20f_2048_estimators"},
        "seed":seed,
        "config":f.config,
        "params":f.params,
        "actual_steps":f.grid.num_steps(),
        "grid_bits":f.grid.times().iter().map(|t| t.to_bits()).collect::<Vec<_>>(),
        "grid_hash":content_hash(&f.grid.times().iter().map(|t| t.to_bits()).collect::<Vec<_>>()).expect("grid hash"),
        "exercise_times":f.exercises,
        "exercise_indices":f.indices,
        "reference_source_hash":content_hash(&include_str!("../../../../models/tests/performance/lmm_reference.rs")).expect("reference hash"),
        "fingerprint_algorithm":"FNV-1a64 over raw little-endian f64 bits; diagnostic, not cryptographic",
        "innovations_fingerprint":format!("fnv1a64:{:016x}",paths.innovations),
        "reference_states_fingerprint":state_fingerprint(&paths.original),
        "current_states_fingerprint":state_fingerprint(&paths.current),
        "reference_choices_hash":content_hash(&old.choices).expect("choice hash"),
        "current_choices_hash":content_hash(&current.choices).expect("choice hash"),
        "max_path_error":paths.max_path_error,
        "rms_path_error":paths.path_rms_error,
        "worst_path_step_forward":paths.worst,
        "max_path_pv_difference":maximum_path_difference,
        "paired_mean_difference":differences.mean(),
        "paired_stderr_difference":differences.stderr(),
        "independent_numeraire_max_pv_error":ratio_error,
        "reference_mean":reference_stats.mean(),
        "reference_stderr":reference_stats.stderr(),
        "engine":engine,
        "public_value_bits":public.value.amount().to_bits(),
    })
}

#[test]
fn ccm_lmm_consumer_fixtures_are_supported() {
    for two in [false, true] {
        let f = fixture(two, 42);
        assert_eq!(f.config.simulated_path_count(), 4_096);
        assert_eq!(f.exercises.len(), if two { 2 } else { 1 });
        assert_eq!(f.params.num_forwards, 20);
        assert!(f.swap.discount_factor_terminal > 0.0);
    }
}

#[test]
#[ignore = "slow: CCM-I5 independent original-drift consumer panel"]
fn ccm_lmm_consumer_panel() {
    let mut cases = Vec::new();
    for two in [false, true] {
        for seed in SEEDS {
            cases.push(check_case(two, seed));
        }
    }
    let record = json!({
        "schema":"ccm.lmm_consumer_numerics/1",
        "bounds":{
            "path_relative":PATH_RELATIVE_BOUND,"path_absolute":PATH_ABSOLUTE_BOUND,
            "max_path_pv_per_notional":MAX_PATH_PV_PER_NOTIONAL,
            "mean_pv_per_notional":MEAN_PV_PER_NOTIONAL,
            "stderr_pv_per_notional":STDERR_PV_PER_NOTIONAL,
            "paired_four_stderr_per_notional":PAIRED_ENVELOPE_PER_NOTIONAL,
            "mean_fraction_of_reference_stderr":0.01,
            "exercise_decision_changes":0,
        },
        "seeds":SEEDS,"cases":cases,
        "existing_default_pv_pin":"0x40c98c29d93c5d7b; unchanged source test remains required",
    });
    if let Ok(path) = std::env::var("CCM_LMM_CONSUMER_NUMERICS_OUTPUT") {
        let bytes = serde_json::to_vec_pretty(&record).expect("numerical record JSON");
        assert!(bytes.len() < 500_000, "keep numerical export compact");
        let path = std::path::Path::new(&path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("export directory");
        }
        std::fs::write(path, bytes).expect("numerical export");
    }
}
