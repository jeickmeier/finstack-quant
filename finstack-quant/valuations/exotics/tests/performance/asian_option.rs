//! Captured-path reference for the valuation performance investigation.
//!
//! This deliberately retains two independent payoff passes as an oracle for
//! the single-pass candidate. It is an ignored accuracy panel, never part of a
//! benchmark callback or a peak-memory measurement.

use super::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_models::monte_carlo::results::MonteCarloResult;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use serde_json::{Value, json};
use time::{Duration, Month};

const MOMENT_TOLERANCE: f64 = 1e-10;
const BASE_SEED: u64 = 11_565_152_576_386_600_568;
const ACCURACY_SEEDS: [u64; 8] = [
    9_255_866_218_220_120_186,
    9_255_867_317_731_748_397,
    9_255_864_019_196_863_764,
    9_255_865_118_708_491_975,
    9_255_861_820_173_607_342,
    9_255_862_919_685_235_553,
    9_255_859_621_150_350_920,
    9_255_860_720_661_979_131,
];

#[derive(Clone, Copy)]
struct Case {
    id: &'static str,
    paths: usize,
    fixings: usize,
    seasoned: bool,
    base_steps: usize,
    merged_steps: usize,
}

const CASES: [Case; 7] = [
    Case {
        id: "asian_option_mc/2500",
        paths: 2_500,
        fixings: 2,
        seasoned: false,
        base_steps: 252,
        merged_steps: 253,
    },
    Case {
        id: "asian_option_mc/paths_2500_fixings_12",
        paths: 2_500,
        fixings: 12,
        seasoned: false,
        base_steps: 252,
        merged_steps: 263,
    },
    Case {
        id: "asian_option_mc/paths_10000_fixings_12",
        paths: 10_000,
        fixings: 12,
        seasoned: false,
        base_steps: 252,
        merged_steps: 263,
    },
    Case {
        id: "asian_option_mc/paths_50000_fixings_12",
        paths: 50_000,
        fixings: 12,
        seasoned: false,
        base_steps: 252,
        merged_steps: 263,
    },
    Case {
        id: "asian_option_mc/paths_10000_fixings_2",
        paths: 10_000,
        fixings: 2,
        seasoned: false,
        base_steps: 252,
        merged_steps: 253,
    },
    Case {
        id: "asian_option_mc/paths_10000_fixings_52",
        paths: 10_000,
        fixings: 52,
        seasoned: false,
        base_steps: 252,
        merged_steps: 303,
    },
    Case {
        id: "asian_option_mc/seasoned_paths_10000_fixings_12_past_6",
        paths: 10_000,
        fixings: 12,
        seasoned: true,
        base_steps: 127,
        merged_steps: 132,
    },
];

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("month"), day).expect("date")
}

fn fixture(case: Case, scenario: &str) -> (AsianOption, MarketContext, Date) {
    let start = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);
    let as_of = if case.seasoned {
        date(2025, 7, 1)
    } else {
        start
    };
    let fixing_dates = match case.fixings {
        2 => vec![date(2025, 7, 1), expiry],
        12 => (2..=12)
            .map(|month| date(2025, month, 1))
            .chain([expiry])
            .collect(),
        52 => (1..=52)
            .map(|i| start + Duration::days(365 * i / 52))
            .collect(),
        _ => unreachable!("sealed fixing-count matrix"),
    };
    let past_fixings = if case.seasoned {
        fixing_dates
            .iter()
            .copied()
            .take(6)
            .zip([98.0, 101.0, 99.0, 102.0, 103.0, 100.0])
            .collect()
    } else {
        Vec::new()
    };
    let mut overrides = InstrumentPricingOverrides::default().with_mc_paths(case.paths);
    if scenario != "base" {
        overrides = overrides.with_mc_seed_scenario(scenario);
    }
    let option = AsianOption {
        id: "ASIAN-BENCH".into(),
        underlying_ticker: "SPOT".into(),
        spot_id: "SPOT".into(),
        strike: 100.0,
        option_type: OptionType::Call,
        expiry,
        quantity: 1.0,
        currency: Currency::USD,
        averaging_method: AveragingMethod::Arithmetic,
        fixing_dates,
        day_count: DayCount::Act365F,
        discount_curve_id: "USD-OIS".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        instrument_pricing_overrides: overrides,
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
        past_fixings,
    };
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (2.0, (-0.10_f64).exp()),
        ])
        .build()
        .expect("discount fixture");
    let surface = finstack_quant_valuations_test_support::volatility::flat_vol_surface(
        "SPOT_VOL",
        &[0.5, 1.0, 2.0],
        &[80.0, 100.0, 120.0],
        0.25,
    );
    let market = MarketContext::new()
        .insert(curve)
        .insert_surface(surface)
        .insert_price("SPOT", MarketScalar::Unitless(100.0))
        .insert_price("SPOT_DIV", MarketScalar::Unitless(0.0));
    (option, market, as_of)
}

#[derive(Clone, Copy)]
struct Moments {
    mean_x: f64,
    mean_y: f64,
    var_x: f64,
    var_y: f64,
    covariance: f64,
}

fn batch_moments(xs: &[f64], ys: &[f64]) -> Moments {
    assert_eq!(xs.len(), ys.len());
    assert!(xs.len() >= 2);
    let mean_x = neumaier_sum(xs.iter().copied()) / xs.len() as f64;
    let mean_y = neumaier_sum(ys.iter().copied()) / ys.len() as f64;
    let denominator = (xs.len() - 1) as f64;
    Moments {
        mean_x,
        mean_y,
        var_x: neumaier_sum(xs.iter().map(|x| (x - mean_x).powi(2))) / denominator,
        var_y: neumaier_sum(ys.iter().map(|y| (y - mean_y).powi(2))) / denominator,
        covariance: neumaier_sum(xs.iter().zip(ys).map(|(x, y)| (x - mean_x) * (y - mean_y)))
            / denominator,
    }
}

fn close(label: &str, actual: f64, reference: f64) {
    assert!(
        actual.is_finite() && reference.is_finite(),
        "{label}: nonfinite value"
    );
    assert!(
        (actual - reference).abs() <= MOMENT_TOLERANCE,
        "{label}: actual={actual:.17e}, reference={reference:.17e}, limit={MOMENT_TOLERANCE}"
    );
}

// Independent joint-normal log-average oracle: dated forwards and a direct
// covariance double sum, without the production carry/control helpers.
fn geometric_oracle(option: &AsianOption, market: &MarketContext, as_of: Date) -> f64 {
    let curve = market.get_discount("USD-OIS").expect("discount curve");
    let future_dates: Vec<_> = option
        .fixing_dates
        .iter()
        .copied()
        .filter(|date| *date > as_of)
        .collect();
    let times: Vec<_> = future_dates
        .iter()
        .map(|date| {
            option
                .day_count
                .year_fraction(as_of, *date, DayCountContext::default())
                .expect("fixing time")
        })
        .collect();
    let log_forwards: Vec<_> = future_dates
        .iter()
        .map(|date| {
            (100.0
                / curve
                    .df_between_dates(as_of, *date)
                    .expect("dated forward DF"))
            .ln()
        })
        .collect();
    let past_log = neumaier_sum(option.past_fixings.iter().map(|(_, value)| value.ln()));
    let n = option.fixing_dates.len() as f64;
    let sigma2 = 0.25_f64.powi(2);
    let mu = (past_log
        + neumaier_sum(
            log_forwards
                .iter()
                .zip(&times)
                .map(|(log_f, t)| log_f - 0.5 * sigma2 * t),
        ))
        / n;
    let variance = sigma2
        * neumaier_sum(
            times
                .iter()
                .flat_map(|a| times.iter().map(move |b| a.min(*b))),
        )
        / (n * n);
    let std_dev = variance.sqrt();
    let forward = (mu + 0.5 * variance).exp();
    let d2 = (mu - option.strike.ln()) / std_dev;
    let d1 = d2 + std_dev;
    let df = curve
        .df_between_dates(as_of, option.expiry)
        .expect("payment DF");
    option.quantity
        * df
        * match option.option_type {
            OptionType::Call => forward * norm_cdf(d1) - option.strike * norm_cdf(d2),
            OptionType::Put => option.strike * norm_cdf(-d2) - forward * norm_cdf(-d1),
        }
}

struct Diagnostic {
    record: Value,
    path_state_bits: Vec<u64>,
    payoff_bits: Vec<[u64; 2]>,
    candidate_bits: [u64; 10],
}

fn require_capture(result: &MonteCarloResult, n: usize, steps: usize) {
    assert_eq!(result.estimate.num_paths, n);
    assert_eq!(result.estimate.num_simulated_paths, n);
    let paths = result.paths.as_ref().expect("full capture");
    assert_eq!(paths.num_paths_total, n);
    assert_eq!(paths.paths.len(), n);
    assert!(
        paths
            .paths
            .iter()
            .all(|path| path.points.len() == steps + 1)
    );
}

fn run_case(case: Case, scenario: &str, use_parallel: bool, retain_path_bits: bool) -> Diagnostic {
    let (option, market, as_of) = fixture(case, scenario);
    option
        .validate_past_fixings(as_of)
        .expect("historical fixing coverage");
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())
        .expect("expiry time");
    let curve = market.get_discount("USD-OIS").expect("curve");
    let df = curve
        .df_between_dates(as_of, option.expiry)
        .expect("payment DF");
    let rate = -df.ln() / t;
    let (hist_sum, hist_log, hist_count) = option.accumulated_state(as_of);
    let mut config =
        finstack_quant_valuations_core::instruments::common_impl::helpers::merged_path_config(
            &PathDependentPricerConfig::default(),
            &option.instrument_pricing_overrides,
        )
        .expect("merged MC config");
    config.seed = option
        .instrument_pricing_overrides
        .model_config
        .mc_seed(&option.id);
    config.use_parallel = use_parallel;
    assert_eq!(config.num_paths, case.paths);
    assert_eq!(config.chunk_size, 1_000);
    assert_eq!(config.steps_per_year, 252.0);
    assert_eq!(config.min_steps, 8);
    assert!(!config.antithetic && !config.use_sobol && !config.use_brownian_bridge);
    let base_steps = ((t * config.steps_per_year).round() as usize).max(config.min_steps);
    assert_eq!(base_steps, case.base_steps);
    let fixing_grid =
        map_fixings_to_steps(&option.fixing_dates, option.day_count, as_of, t, base_steps)
            .expect("actual pricing grid");
    assert_eq!(fixing_grid.time_grid.num_steps(), case.merged_steps);
    let grid_bits: Vec<_> = fixing_grid
        .time_grid
        .times()
        .iter()
        .map(|t| t.to_bits())
        .collect();
    let times = fixing_grid.future_times();
    let fixing_steps = fixing_grid.fixing_steps.clone();
    let (drift, multipliers) =
        asian_drift_and_fixing_multipliers(&option, &market, as_of, 0.0).expect("dated carry");
    let drift = std::sync::Arc::new(drift);
    let process = GbmProcess::new(GbmParams::new(rate, 0.0, 0.25).expect("GBM parameters"))
        .with_drift_schedule(std::sync::Arc::clone(&drift));
    let payoff = |method| {
        Asian::with_history(
            option.option_type,
            option.strike,
            option.quantity,
            method,
            fixing_steps.clone(),
            AsianFixingHistory {
                sum: hist_sum,
                log_sum: hist_log,
                count: hist_count,
            },
        )
        .expect("Asian payoff")
        .with_fixing_multipliers(&multipliers)
        .expect("fixing multipliers")
    };
    let mut capture_config = config.clone();
    capture_config.path_capture = PathCaptureConfig::all();
    let pricer = PathDependentPricer::new(capture_config);
    let arithmetic = pricer
        .price_with_paths_and_grid(
            &process,
            100.0,
            fixing_grid.time_grid.clone(),
            &payoff(AveragingMethod::Arithmetic),
            Currency::USD,
            df,
        )
        .expect("arithmetic capture");
    let geometric = pricer
        .price_with_paths_and_grid(
            &process,
            100.0,
            fixing_grid.time_grid.clone(),
            &payoff(AveragingMethod::Geometric),
            Currency::USD,
            df,
        )
        .expect("geometric capture");
    require_capture(&arithmetic, case.paths, case.merged_steps);
    require_capture(&geometric, case.paths, case.merged_steps);
    let arithmetic_run = arithmetic.run.as_ref().expect("arithmetic run metadata");
    let geometric_run = geometric.run.as_ref().expect("geometric run metadata");
    for run in [arithmetic_run, geometric_run] {
        assert_eq!(run.seed, Some(config.seed));
        assert_eq!(run.num_steps, case.merged_steps);
        assert_eq!(run.chunk_size, 1_000);
        assert_eq!(run.use_parallel, use_parallel);
        assert!(!run.antithetic);
    }
    let mut xs = Vec::with_capacity(case.paths);
    let mut ys = Vec::with_capacity(case.paths);
    let mut path_state_bits = Vec::new();
    let mut payoff_bits = Vec::new();
    for (index, (a, g)) in arithmetic
        .paths
        .as_ref()
        .expect("arithmetic paths")
        .paths
        .iter()
        .zip(&geometric.paths.as_ref().expect("geometric paths").paths)
        .enumerate()
    {
        assert_eq!(a.path_id, index);
        assert_eq!(g.path_id, index);
        for (a_point, g_point) in a.points.iter().zip(&g.points) {
            assert_eq!(a_point.step, g_point.step);
            assert_eq!(a_point.time.to_bits(), g_point.time.to_bits());
            assert_eq!(a_point.state.len(), g_point.state.len());
            for (a_state, g_state) in a_point.state.iter().zip(&g_point.state) {
                assert_eq!(a_state.to_bits(), g_state.to_bits());
                if retain_path_bits {
                    path_state_bits.push(a_state.to_bits());
                }
            }
        }
        assert!(a.final_value.is_finite() && g.final_value.is_finite());
        xs.push(a.final_value);
        ys.push(g.final_value);
        if retain_path_bits {
            payoff_bits.push([a.final_value.to_bits(), g.final_value.to_bits()]);
        }
    }
    // Release the full captures before the separate production invocation.
    drop(arithmetic);
    drop(geometric);
    // Exercise the actual production collector separately from the two
    // captured scalar passes. This also ensures that moments are not merely
    // compared between two test-only versions of the same old algorithm.
    let candidate = PathDependentPricer::new(config.clone())
        .price_asian_pair(
            &process,
            100.0,
            fixing_grid.time_grid,
            &payoff(AveragingMethod::Arithmetic),
            Currency::USD,
            df,
        )
        .expect("production paired moments");
    assert_eq!(candidate.count(), case.paths);
    let batch = batch_moments(&xs, &ys);
    let online = paired_moments(&xs, &ys);
    for (label, actual, reference) in [
        ("arithmetic mean", online.mean_x(), batch.mean_x),
        ("geometric mean", online.mean_y(), batch.mean_y),
        ("arithmetic variance", online.variance_x(), batch.var_x),
        ("geometric variance", online.variance_y(), batch.var_y),
        ("paired covariance", online.covariance(), batch.covariance),
    ] {
        close(label, actual, reference);
    }
    for (label, actual, reference) in [
        (
            "candidate arithmetic mean",
            candidate.mean_x(),
            batch.mean_x,
        ),
        ("candidate geometric mean", candidate.mean_y(), batch.mean_y),
        (
            "candidate arithmetic variance",
            candidate.variance_x(),
            batch.var_x,
        ),
        (
            "candidate geometric variance",
            candidate.variance_y(),
            batch.var_y,
        ),
        (
            "candidate covariance",
            candidate.covariance(),
            batch.covariance,
        ),
    ] {
        close(label, actual, reference);
    }
    let analytic = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot: 100.0,
            strike: option.strike,
            rate,
            volatility: 0.25,
            time_to_expiry: t,
            dividend_yield: 0.0,
            option_type: option.option_type,
        },
        df,
        hist_log,
        hist_count,
        &times,
        Some(drift.as_ref()),
        Some(&multipliers),
    );
    let analytic_reference = geometric_oracle(&option, &market, as_of);
    close(
        "independent geometric control",
        analytic,
        analytic_reference,
    );
    let adjusted = apply_control_variate(
        online.mean_x(),
        online.variance_x(),
        online.mean_y(),
        online.variance_y(),
        online.covariance(),
        analytic,
        case.paths,
    );
    let candidate_adjusted = apply_control_variate(
        candidate.mean_x(),
        candidate.variance_x(),
        candidate.mean_y(),
        candidate.variance_y(),
        candidate.covariance(),
        analytic,
        candidate.count(),
    );
    let beta = if batch.var_y > 1e-10 {
        batch.covariance / batch.var_y
    } else {
        0.0
    };
    let online_beta = if online.variance_y() > 1e-10 {
        online.covariance() / online.variance_y()
    } else {
        0.0
    };
    let candidate_beta = if candidate.variance_y() > 1e-10 {
        candidate.covariance() / candidate.variance_y()
    } else {
        0.0
    };
    close("control beta", online_beta, beta);
    close("candidate control beta", candidate_beta, beta);
    let adjusted_mean = batch.mean_x - beta * (batch.mean_y - analytic_reference);
    let adjusted_var =
        (batch.var_x - 2.0 * beta * batch.covariance + beta * beta * batch.var_y).max(0.0);
    let residual_mean = batch.mean_x - beta * batch.mean_y;
    let residual_var = neumaier_sum(
        xs.iter()
            .zip(&ys)
            .map(|(x, y)| (x - beta * y - residual_mean).powi(2)),
    ) / (case.paths - 1) as f64;
    close("adjusted mean", adjusted.mean, adjusted_mean);
    close(
        "adjusted variance",
        case.paths as f64 * adjusted.stderr.powi(2),
        adjusted_var,
    );
    close("direct residual variance", adjusted_var, residual_var);
    close(
        "candidate adjusted mean",
        candidate_adjusted.mean,
        adjusted_mean,
    );
    close(
        "candidate adjusted variance",
        candidate.count() as f64 * candidate_adjusted.stderr.powi(2),
        adjusted_var,
    );
    // A variance-scale gate remains meaningful at zero; this derived bound
    // avoids dividing by a vanishing reference standard error.
    let reference_stderr = (adjusted_var / case.paths as f64).sqrt();
    assert!(
        (candidate_adjusted.stderr - reference_stderr).abs()
            <= (MOMENT_TOLERANCE / case.paths as f64).sqrt(),
        "candidate adjusted standard error differs from captured reference"
    );
    let geometric_stderr = (batch.var_y / case.paths as f64).sqrt();
    assert!(
        (batch.mean_y - analytic_reference).abs() <= 5.0 * geometric_stderr,
        "{} {scenario}: geometric sample {} vs analytic {}, sampling SE {}",
        case.id,
        batch.mean_y,
        analytic_reference,
        geometric_stderr
    );
    let production = AsianOptionMcPricer {
        config: PathDependentPricerConfig {
            use_parallel,
            ..Default::default()
        },
    }
    .price_dyn(&option, &market, as_of)
    .expect("production MC price");
    assert_eq!(production.value.currency(), Currency::USD);
    close(
        "production vs captured oracle",
        production.value.amount(),
        adjusted.mean,
    );
    close(
        "production vs paired collector",
        production.value.amount(),
        candidate_adjusted.mean,
    );
    let candidate_bits = [
        candidate.mean_x().to_bits(),
        candidate.mean_y().to_bits(),
        candidate.variance_x().to_bits(),
        candidate.variance_y().to_bits(),
        candidate.covariance().to_bits(),
        candidate_beta.to_bits(),
        candidate_adjusted.mean.to_bits(),
        candidate_adjusted.stderr.to_bits(),
        candidate_adjusted.ci_95.0.to_bits(),
        candidate_adjusted.ci_95.1.to_bits(),
    ];
    let record = json!({
        "case": case.id, "scenario": scenario,
        "fixture": { "instrument": option, "market": market, "as_of": as_of, "model": "MonteCarloGBM" },
        "config": { "seed": config.seed, "num_paths": config.num_paths, "use_parallel": config.use_parallel,
            "chunk_size": config.chunk_size, "steps_per_year": config.steps_per_year, "min_steps": config.min_steps,
            "antithetic": false, "use_sobol": false, "use_brownian_bridge": false },
        "grid": { "base_steps": base_steps, "actual_steps": case.merged_steps, "times_f64_bits": grid_bits,
            "fixing_steps": fixing_steps, "future_fixing_times": times, "fixing_multipliers": multipliers },
        "counts": { "independent_estimators": case.paths, "distinct_physical_paths": case.paths,
            "physical_paths_per_payoff_pass": case.paths, "reference_simulated_path_traversals": 2 * case.paths,
            "historical_fixings": hist_count, "future_fixings": case.fixings - hist_count },
        "moments": { "arithmetic_mean": online.mean_x(), "geometric_mean": online.mean_y(),
            "arithmetic_variance": online.variance_x(), "geometric_variance": online.variance_y(), "covariance": online.covariance(),
            "batch_arithmetic_mean": batch.mean_x, "batch_geometric_mean": batch.mean_y,
            "batch_arithmetic_variance": batch.var_x, "batch_geometric_variance": batch.var_y,
            "batch_covariance": batch.covariance, "beta": online_beta, "batch_beta": beta, "analytic_control": analytic,
            "independent_analytic_control": analytic_reference, "adjusted_mean": adjusted.mean,
            "batch_adjusted_mean": adjusted_mean, "batch_adjusted_variance": adjusted_var,
            "batch_adjusted_stderr": (adjusted_var / case.paths as f64).sqrt(),
            "adjusted_variance": case.paths as f64 * adjusted.stderr.powi(2), "adjusted_stderr": adjusted.stderr,
            "adjusted_ci95": adjusted.ci_95, "geometric_stderr": geometric_stderr,
            "arithmetic_stderr": (batch.var_x / case.paths as f64).sqrt(), "direct_residual_variance": residual_var },
        "production_value": production.value.amount(), "currency": "USD",
        "candidate": { "source": "PathDependentPricer::price_asian_pair",
            "independent_estimators": candidate.count(), "configured_physical_paths": config.num_paths,
            "retains_paths": false, "arithmetic_mean": candidate.mean_x(), "geometric_mean": candidate.mean_y(),
            "arithmetic_variance": candidate.variance_x(), "geometric_variance": candidate.variance_y(),
            "covariance": candidate.covariance(), "beta": candidate_beta,
            "analytic_control": analytic, "adjusted_mean": candidate_adjusted.mean,
            "adjusted_variance": candidate.count() as f64 * candidate_adjusted.stderr.powi(2),
            "adjusted_stderr": candidate_adjusted.stderr, "adjusted_ci95": candidate_adjusted.ci_95,
            "moment_estimate_f64_bits": candidate_bits },
        "tolerances": { "absolute_per_unit_moments": MOMENT_TOLERANCE, "geometric_sampling_standard_errors": 5.0 },
        "uncertainty": "Pseudo-random independent paths; adjusted SE is the existing fitted-beta plug-in estimate, not an allowance for deterministic differences."
    });
    Diagnostic {
        record,
        path_state_bits,
        payoff_bits,
        candidate_bits,
    }
}

#[test]
#[ignore = "Full captured-path accuracy panel for the valuation performance investigation"]
fn asian_mc_performance_accuracy_panel() {
    for (index, case) in CASES.into_iter().enumerate() {
        let mut diagnostic = run_case(case, "base", true, index == 0);
        assert_eq!(
            diagnostic.record["config"]["seed"].as_u64(),
            Some(BASE_SEED)
        );
        if index == 0 {
            let serial = run_case(case, "base", false, true);
            assert_eq!(diagnostic.path_state_bits, serial.path_state_bits);
            assert_eq!(diagnostic.payoff_bits, serial.payoff_bits);
            assert_eq!(diagnostic.candidate_bits, serial.candidate_bits);
            diagnostic.record["serial_parallel_path_and_payoff_bits_equal"] = json!(true);
            diagnostic.record["serial_parallel_candidate_moment_bits_equal"] = json!(true);
        }
        println!("FQ_MC_DIAGNOSTIC {}", diagnostic.record);
    }
    for case in [CASES[2], CASES[6]] {
        for (index, seed) in ACCURACY_SEEDS.into_iter().enumerate() {
            let scenario = format!("b4_accuracy_{index:02}");
            let diagnostic = run_case(case, &scenario, true, false);
            assert_eq!(diagnostic.record["config"]["seed"].as_u64(), Some(seed));
            println!("FQ_MC_DIAGNOSTIC {}", diagnostic.record);
        }
    }
}
