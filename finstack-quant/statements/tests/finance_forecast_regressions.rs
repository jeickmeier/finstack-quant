//! Forecast counterexamples from the quantitative finance review.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::evaluator::MonteCarloConfig;
use finstack_quant_statements::types::SeasonalMode;
use finstack_quant_statements::types::{FinancialModelSpec, ForecastMethod, ForecastSpec};
use finstack_quant_statements::types::{ForecastFadeShape, ForecastTrendMethod};
use serde_json::json;

fn quarter(index: u8) -> PeriodId {
    PeriodId::quarter(2025, index).unwrap()
}

fn forecast_model(base: f64, forecast: ForecastSpec) -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("forecast-regression");
        model_builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
        model_builder.value("x", &[(quarter(1), base.into())]);
        model_builder.forecast("x", forecast);
        model_builder.build().unwrap()
    }
}

#[test]
fn override_period_aliases_apply_to_their_parsed_periods() {
    let january = PeriodId::month(2025, 1).unwrap();
    let february = PeriodId::month(2025, 2).unwrap();
    let march = PeriodId::month(2025, 3).unwrap();
    for spelling in ["2025M2", "2025m02", " 2025M02 "] {
        let forecast = ForecastSpec::from_params(
            ForecastMethod::Override,
            [("overrides".into(), json!({spelling: 200.0}))]
                .into_iter()
                .collect(),
        )
        .unwrap();
        let model = {
            let mut model_builder = ModelBuilder::new("monthly-overrides");
            model_builder.periods("2025M1..M3", Some("2025M1")).unwrap();
            model_builder.value("x", &[(january, 100.0.into())]);
            model_builder.forecast("x", forecast);
            model_builder.build().unwrap()
        };
        let result = Evaluator::new().evaluate(&model).unwrap();
        assert_eq!(result.get("x", &february), Some(200.0), "{spelling}");
        assert_eq!(result.get("x", &march), Some(200.0), "{spelling}");
    }
}

#[test]
fn override_duplicate_period_aliases_are_rejected() {
    let forecast = ForecastSpec::from_params(
        ForecastMethod::Override,
        [(
            "overrides".into(),
            json!({"2025Q2": 200.0, "2025q2": 300.0}),
        )]
        .into_iter()
        .collect(),
    )
    .unwrap();
    let model = forecast_model(100.0, forecast);
    let error = Evaluator::new().evaluate(&model).unwrap_err();
    assert!(error.to_string().contains("Duplicate override"), "{error}");
}

#[test]
fn multiplicative_forecast_rejects_between_observation_trend_crossing() {
    // All observations are positive, but endpoint trend extrapolation crosses
    // zero between observations. Ratios against that trend previously produced
    // negative revenue forecasts (-16.93, 106.92, -21.30).
    let model = forecast_model(
        10.0,
        ForecastSpec::seasonal(
            vec![1.0, 1.0, 1.0, 1.0, 10.0, 10.0, 10.0, 10.0],
            4,
            SeasonalMode::Multiplicative,
        ),
    );
    let error = Evaluator::new().evaluate(&model).unwrap_err();
    assert!(error.to_string().contains("crosses or touches zero"));
    assert!(error.to_string().contains("additive"));
}

#[test]
fn forecast_bounds_preserve_missing_observations_and_warnings() {
    for bounds in [
        vec![("min", 0.0)],
        vec![("max", 100.0)],
        vec![("min", 0.0), ("max", 100.0)],
    ] {
        let mut forecast = ForecastSpec::forward_fill();
        for (key, value) in bounds {
            forecast = forecast.with_param(key, json!(value)).unwrap();
        }
        let mut model_builder = ModelBuilder::new("forecast-regression");
        model_builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
        model_builder.value("x", &[(quarter(1), f64::NAN.into())]);
        model_builder.forecast("x", forecast);
        let error = model_builder
            .build()
            .expect_err("a non-finite observation is not admitted");
        assert!(error.to_string().contains("non-finite"), "{error}");
    }
}

/// A typed specification cannot be given a malformed optional parameter:
/// the wrong JSON type is rejected when the parameter is set.
#[test]
fn malformed_optional_forecast_parameters_cannot_be_constructed() {
    let seasonal = ForecastSpec::seasonal(vec![100.0; 8], 4, SeasonalMode::Additive);
    let trend = ForecastSpec::time_series(vec![90.0, 100.0], ForecastTrendMethod::Linear);
    let normal = ForecastSpec::normal(0.0, 1.0, 42);
    for (spec, key, value) in [
        (seasonal, "growth", json!("0.1")),
        (trend, "method", json!(true)),
        (normal.clone(), "correlation_with", json!(1)),
        (normal, "correlation", json!("0.5")),
    ] {
        let error = spec.with_param(key, value).expect_err(key);
        assert!(error.to_string().contains(key), "{key}: {error}");
    }
}

#[test]
fn correlated_forecasts_reject_malformed_optional_parameters() {
    // A typed specification cannot hold a malformed parameter at all.
    let error = ForecastSpec::normal(0.0, 1.0, 42)
        .with_param("correlation_with", json!("peer"))
        .unwrap()
        .with_param("correlation", json!(0.5))
        .unwrap()
        .with_param("max", json!("150"))
        .unwrap_err();
    assert!(error.to_string().contains("max"), "{error}");
}

#[test]
fn extreme_season_lengths_fail_before_allocation() {
    for length in [u64::MAX, u64::from(u32::MAX) + 5] {
        let mut forecast = ForecastSpec::seasonal(vec![100.0; 8], 4, SeasonalMode::Additive);
        forecast = forecast
            .clone()
            .with_param("season_length", json!(length))
            .unwrap();
        let model = forecast_model(100.0, forecast);
        assert!(Evaluator::new().evaluate(&model).is_err());
    }
}

#[test]
fn bounded_constant_moving_average_rejects_overflow() {
    let mut forecast = ForecastSpec::time_series(vec![1e308, 1e308], ForecastTrendMethod::Linear);
    forecast = forecast
        .clone()
        .with_param("method", json!("moving_average"))
        .unwrap();
    forecast = forecast.clone().with_param("window", json!(2)).unwrap();
    forecast = forecast.clone().with_param("max", json!(100.0)).unwrap();
    let model = forecast_model(1e308, forecast);
    let error = Evaluator::new().evaluate(&model).unwrap_err();
    assert!(error.to_string().contains("non-finite historical average"));
}

#[test]
fn fades_between_finite_opposite_extremes_remain_finite() {
    for exponential in [false, true] {
        let mut forecast = ForecastSpec::fade_to_target(-1e308, ForecastFadeShape::Linear);
        if exponential {
            forecast = forecast
                .clone()
                .with_param("shape", json!("exponential"))
                .unwrap();
            forecast = forecast
                .clone()
                .with_param("half_life", json!(1.0))
                .unwrap();
        }
        let model = forecast_model(1e308, forecast);
        let result = Evaluator::new().evaluate(&model).unwrap();
        for index in 2..=4 {
            let value = result.get("x", &quarter(index)).unwrap();
            assert!(value.is_finite());
            assert!((-1e308..=1e308).contains(&value));
        }
        if exponential {
            assert_eq!(result.get("x", &quarter(2)), Some(0.0));
        } else {
            assert_eq!(result.get("x", &quarter(4)), Some(-1e308));
        }
    }
}

/// A seed is an exact unsigned 64-bit integer. Neighbouring seeds above
/// 2^53, which a float cannot tell apart, draw different paths.
#[test]
fn seeds_are_exact_above_two_to_the_53() {
    let path = |seed: u64| {
        let model = forecast_model(100.0, ForecastSpec::normal(0.0, 1.0, seed));
        let result = Evaluator::new().evaluate(&model).unwrap();
        result.get("x", &quarter(4)).unwrap()
    };
    assert_ne!(path(9_007_199_254_740_992), path(9_007_199_254_740_993));
    assert_eq!(path(u64::MAX), path(u64::MAX));

    let spec = ForecastSpec::normal(0.0, 1.0, 42);
    // A float seed, and one beyond u64, are not seeds.
    for bad in [
        json!(42.0),
        json!(u64::MAX as f64),
        json!("18446744073709551616"),
    ] {
        assert!(spec.clone().with_param("seed", bad).is_err());
    }
}

fn shock_chain_model(method: ForecastMethod, tiny: bool, rho: f64) -> FinancialModelSpec {
    let sigma = if tiny { 1e-20 } else { 0.1 };
    let peer = match method {
        ForecastMethod::Normal => ForecastSpec::normal(0.0, sigma, 42),
        ForecastMethod::LogNormal => ForecastSpec::log_normal(0.0, sigma, 42),
        _ => ForecastSpec::mean_reverting(if tiny { 1e16 } else { 100.0 }, 0.1, sigma, 42),
    };
    let mut follower = ForecastSpec::normal(0.0, 1.0, 19);
    follower = follower
        .clone()
        .with_param("correlation_with", json!("peer"))
        .unwrap();
    follower = follower
        .clone()
        .with_param("correlation", json!(rho))
        .unwrap();
    let mut child = ForecastSpec::normal(0.0, 1.0, 23);
    child = child
        .clone()
        .with_param("correlation_with", json!("follower"))
        .unwrap();
    child = child.clone().with_param("correlation", json!(0.6)).unwrap();
    let mut builder = ModelBuilder::new("direct-shocks");
    builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
    builder.value(
        "peer",
        &[(quarter(1), if tiny { 1e16.into() } else { 100.0.into() })],
    );
    builder.forecast("peer", peer);
    let mut mixed = builder.mixed("follower");
    mixed.values(&[(quarter(1), 100.0.into())]);
    mixed.formula("peer * 0 + 100").unwrap();
    mixed.forecast(follower);
    builder = mixed.build().unwrap();
    let mut mixed = builder.mixed("child");
    mixed.values(&[(quarter(1), 100.0.into())]);
    mixed.formula("follower * 0 + 100").unwrap();
    mixed.forecast(child);
    builder = mixed.build().unwrap();
    builder.build().unwrap()
}

fn metric_paths(
    results: &finstack_quant_statements::evaluator::MonteCarloResults,
    metric: &str,
) -> Vec<f64> {
    use finstack_quant_core::table::TableColumnData;
    let columns = &results.path_data.as_ref().unwrap().columns;
    let (TableColumnData::String(metrics), TableColumnData::Float64(values)) =
        (&columns[2].data, &columns[3].data)
    else {
        panic!("path table shape")
    };
    metrics
        .iter()
        .zip(values)
        .filter_map(|(name, value)| (name == metric).then_some(*value))
        .collect()
}

#[test]
fn rounded_peer_levels_preserve_correlated_paths_and_chains() {
    let config = MonteCarloConfig::new(32, 7).with_path_data(true);
    for method in [
        ForecastMethod::Normal,
        ForecastMethod::LogNormal,
        ForecastMethod::MeanReverting,
    ] {
        for rho in [-1.0, 0.0, 0.6, 1.0] {
            let baseline = Evaluator::new()
                .evaluate_monte_carlo(&shock_chain_model(method, false, rho), &config)
                .unwrap();
            let rounded = Evaluator::new()
                .evaluate_monte_carlo(&shock_chain_model(method, true, rho), &config)
                .unwrap();
            assert!(metric_paths(&rounded, "peer").iter().all(|v| *v == 1e16));
            for metric in ["follower", "child"] {
                assert_eq!(
                    metric_paths(&rounded, metric),
                    metric_paths(&baseline, metric),
                    "{method:?}, rho={rho}, {metric}"
                );
            }
        }
    }
}

#[test]
fn clipping_and_explicit_overrides_preserve_latent_correlated_paths() {
    let baseline = shock_chain_model(ForecastMethod::Normal, true, 0.6);
    let config = MonteCarloConfig::new(32, 7).with_path_data(true);
    let expected = Evaluator::new()
        .evaluate_monte_carlo(&baseline, &config)
        .unwrap();
    for override_explicit in [false, true] {
        let mut model = baseline.clone();
        let peer = model.nodes.get_mut("peer").unwrap();
        if override_explicit {
            peer.values
                .as_mut()
                .unwrap()
                .insert(quarter(3), 200.0.into());
        } else {
            let params = &mut peer.forecast.as_mut().unwrap().params();
            params.insert("min".into(), json!(100.0));
            params.insert("max".into(), json!(100.0));
        }
        let actual = Evaluator::new()
            .evaluate_monte_carlo(&model, &config)
            .unwrap();
        for metric in ["follower", "child"] {
            assert_eq!(
                metric_paths(&actual, metric),
                metric_paths(&expected, metric)
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn direct_shocks_have_identical_serial_and_parallel_wire_results() {
    let model = shock_chain_model(ForecastMethod::LogNormal, true, 0.6);
    let config = MonteCarloConfig::new(64, 7).with_path_data(true);
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                serde_json::to_value(
                    Evaluator::new()
                        .evaluate_monte_carlo(&model, &config)
                        .unwrap(),
                )
                .unwrap()
            })
    };
    assert_eq!(run(1), run(4));
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let (mean_x, mean_y) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mean_x) * (b - mean_y);
        sxx += (a - mean_x) * (a - mean_x);
        syy += (b - mean_y) * (b - mean_y);
    }
    sxy / (sxx * syy).sqrt()
}

/// Stochastic nodes with no configured correlation are independent on every
/// path. Sharing one RNG increment across the nodes of a path left fixed
/// node pairs correlated at about 0.07 (`ebitda`/`tax` at step 5).
#[test]
fn uncorrelated_nodes_draw_independent_shocks_on_each_path() {
    let names = [
        "ebitda",
        "tax",
        "sga",
        "utilities",
        "arr",
        "wages",
        "capex",
        "revenue",
        "cogs",
        "opex",
    ];
    let paths = 20_000_usize;
    // Five standard errors: with 675 comparisons per run a false alarm has
    // probability below 1e-3.
    let bound = 5.0 / (paths as f64).sqrt();
    for (config_seed, distinct_node_seeds) in [(7, false), (2024, false), (7, true), (2024, true)] {
        let mut builder = ModelBuilder::new("mc-independence");
        builder.periods("2025Q1..2028Q4", Some("2025Q1")).unwrap();
        for (index, id) in names.iter().copied().enumerate() {
            let node_seed = if distinct_node_seeds {
                1000 + 17 * index as u64
            } else {
                42
            };
            builder.value(id, &[(quarter(1), 0.0.into())]);
            // Full reversion to zero with unit volatility: the level is the shock.
            builder.forecast(id, ForecastSpec::mean_reverting(0.0, 1.0, 1.0, node_seed));
        }
        let results = Evaluator::new()
            .evaluate_monte_carlo(
                &builder.build().unwrap(),
                &MonteCarloConfig::new(paths, config_seed).with_path_data(true),
            )
            .unwrap();
        let series: Vec<Vec<f64>> = names
            .iter()
            .map(|name| metric_paths(&results, name))
            .collect();
        let steps = series[0].len() / paths;
        assert_eq!(steps, 15);
        for first in 0..names.len() {
            for second in first + 1..names.len() {
                for step in 0..steps {
                    let at_step = |values: &[f64]| -> Vec<f64> {
                        values.iter().skip(step).step_by(steps).copied().collect()
                    };
                    let r = pearson(&at_step(&series[first]), &at_step(&series[second]));
                    assert!(
                        r.abs() < bound,
                        "seed {config_seed}, distinct node seeds {distinct_node_seeds}: \
                         {}/{} step {} correlation {r:.4}",
                        names[first],
                        names[second],
                        step + 1
                    );
                }
            }
        }
    }
}

/// A zero correlation leaves a node's paths exactly as they are without any
/// configured correlation.
#[test]
fn zero_correlation_keeps_the_uncorrelated_draws() {
    let model = |correlated: bool| {
        let mut node = ForecastSpec::normal(0.0, 1.0, 19);
        if correlated {
            node = node
                .clone()
                .with_param("correlation_with", json!("peer"))
                .unwrap();
            node = node.clone().with_param("correlation", json!(0.0)).unwrap();
        }
        let mut builder = ModelBuilder::new("zero-correlation");
        builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
        builder.value("node", &[(quarter(1), 100.0.into())]);
        builder.forecast("node", node);
        builder.value("peer", &[(quarter(1), 100.0.into())]);
        builder.forecast("peer", ForecastSpec::normal(0.0, 1.0, 42));
        builder.build().unwrap()
    };
    let config = MonteCarloConfig::new(64, 7).with_path_data(true);
    let paths = |correlated: bool| {
        let results = Evaluator::new()
            .evaluate_monte_carlo(&model(correlated), &config)
            .unwrap();
        metric_paths(&results, "node")
    };
    assert_eq!(paths(true), paths(false));
}
