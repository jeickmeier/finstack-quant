//! Forecast counterexamples from the quantitative finance review.

use crate::AllMetrics;
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

/// An override key is the canonical text of its period: another spelling of
/// the same period is rejected when the specification is validated, so two
/// keys can never name one period.
#[test]
fn override_keys_are_canonical_period_identifiers() {
    let keyed = |spelling: &str| {
        ForecastSpec::from_params(
            ForecastMethod::Override,
            [("overrides".into(), json!({spelling: 200.0}))]
                .into_iter()
                .collect(),
        )
        .unwrap()
    };
    let canonical = PeriodId::month(2025, 2).unwrap().to_string();
    assert!(keyed(&canonical).validate().is_ok());
    for spelling in ["2025m02", " 2025M02 ", "2025q2", "not-a-period"] {
        let error = keyed(spelling).validate().unwrap_err();
        assert!(
            error.to_string().contains("canonical"),
            "{spelling}: {error}"
        );
    }
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
    // A season the history cannot cover twice is a static error, raised
    // before anything is sized by the season length.
    for length in [u64::MAX, u64::from(u32::MAX) + 5] {
        let forecast = ForecastSpec::seasonal(vec![100.0; 8], 4, SeasonalMode::Additive)
            .with_param("season_length", json!(length))
            .unwrap();
        let error = forecast.validate().unwrap_err();
        assert!(
            error.to_string().contains("two complete seasons"),
            "{error}"
        );
    }
}

#[test]
fn a_moving_average_of_extreme_finite_levels_is_representable() {
    let mut forecast = ForecastSpec::time_series(vec![1e308, 1e308], ForecastTrendMethod::Linear);
    forecast = forecast
        .clone()
        .with_param("method", json!("moving_average"))
        .unwrap();
    forecast = forecast.clone().with_param("window", json!(2)).unwrap();
    forecast = forecast.clone().with_param("max", json!(100.0)).unwrap();
    let model = forecast_model(1e308, forecast);
    // The mean of two levels of 1e308 is 1e308: the window is accumulated
    // on scaled values, so its partial sum does not overflow. The bound then
    // clips the published level.
    let result = Evaluator::new().evaluate(&model).unwrap();
    assert_eq!(result.get("x", &quarter(2)), Some(100.0));
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
    let config = MonteCarloConfig::new(32, 7, [""; 0]).with_path_data(true);
    for method in [
        ForecastMethod::Normal,
        ForecastMethod::LogNormal,
        ForecastMethod::MeanReverting,
    ] {
        for rho in [-1.0, 0.0, 0.6, 1.0] {
            let baseline = Evaluator::new()
                .evaluate_all_metrics(&shock_chain_model(method, false, rho), &config)
                .unwrap();
            let rounded = Evaluator::new()
                .evaluate_all_metrics(&shock_chain_model(method, true, rho), &config)
                .unwrap();
            // The lognormal level is restored from its logarithm.
            assert!(
                metric_paths(&rounded, "peer")
                    .iter()
                    .all(|v| (*v / 1e16 - 1.0).abs() < 1e-12)
            );
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
    let config = MonteCarloConfig::new(32, 7, [""; 0]).with_path_data(true);
    let expected = Evaluator::new()
        .evaluate_all_metrics(&baseline, &config)
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
            let forecast = peer.forecast.take().unwrap();
            peer.forecast = Some(
                forecast
                    .with_param("min", json!(100.0))
                    .unwrap()
                    .with_param("max", json!(100.0))
                    .unwrap(),
            );
        }
        let actual = Evaluator::new()
            .evaluate_all_metrics(&model, &config)
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
    let config = MonteCarloConfig::new(64, 7, [""; 0]).with_path_data(true);
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                serde_json::to_value(
                    Evaluator::new()
                        .evaluate_all_metrics(&model, &config)
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
            .evaluate_all_metrics(
                &builder.build().unwrap(),
                &MonteCarloConfig::new(paths, config_seed, [""; 0]).with_path_data(true),
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
    let config = MonteCarloConfig::new(64, 7, [""; 0]).with_path_data(true);
    let paths = |correlated: bool| {
        let results = Evaluator::new()
            .evaluate_all_metrics(&model(correlated), &config)
            .unwrap();
        metric_paths(&results, "node")
    };
    assert_eq!(paths(true), paths(false));
}

// STATEMENTS-EVALUATION-005 qualification: one forecast suffix per node, an
// explicit start level or the slot before the suffix as base, and bounds on
// published outputs only.

fn growth_model(
    forecast: ForecastSpec,
    actual: Option<f64>,
    explicit: &[(u8, f64)],
) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("suffix");
    builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
    let values: Vec<_> = actual
        .map(|value| (quarter(1), value))
        .into_iter()
        .chain(
            explicit
                .iter()
                .map(|(index, value)| (quarter(*index), *value)),
        )
        .collect();
    builder.value_scalar("x", &values);
    builder.forecast("x", forecast);
    builder.build().unwrap()
}

#[test]
fn an_explicit_value_mid_suffix_does_not_change_the_recurrence() {
    let plain = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(0.1), Some(100.0), &[]))
        .unwrap();
    let overridden = Evaluator::new()
        .evaluate(&growth_model(
            ForecastSpec::growth(0.1),
            Some(100.0),
            &[(3, 999.0)],
        ))
        .unwrap();
    // Q3 publishes the explicit value; Q4 continues the raw recurrence
    // 100 x 1.1^3, not 999 x 1.1.
    assert_eq!(overridden.get("x", &quarter(3)), Some(999.0));
    for index in [2, 4] {
        assert_eq!(
            overridden.get("x", &quarter(index)),
            plain.get("x", &quarter(index))
        );
    }
    assert!((plain.get("x", &quarter(4)).unwrap() - 133.1).abs() < 1e-9);

    // The same holds for a seasonal phase and a stochastic draw sequence.
    for forecast in [
        ForecastSpec::seasonal(
            vec![10.0, 20.0, 30.0, 40.0, 11.0, 21.0, 31.0, 41.0],
            4,
            SeasonalMode::Additive,
        ),
        ForecastSpec::normal(0.0, 5.0, 11),
    ] {
        let plain = Evaluator::new()
            .evaluate(&growth_model(forecast.clone(), Some(100.0), &[]))
            .unwrap();
        let overridden = Evaluator::new()
            .evaluate(&growth_model(forecast, Some(100.0), &[(2, 1.0)]))
            .unwrap();
        for index in [3, 4] {
            assert_eq!(
                overridden.get("x", &quarter(index)),
                plain.get("x", &quarter(index))
            );
        }
    }
}

#[test]
fn the_base_is_the_start_level_or_the_slot_before_the_suffix() {
    use finstack_quant_statements::evaluator::UnavailableReason;
    // The slot before the suffix holds no value: the forecast is unavailable
    // with a reason, not an error and not zero.
    let missing = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(0.1), None, &[]))
        .unwrap();
    for index in [2, 3, 4] {
        let cell = missing.get_cell("x", &quarter(index)).unwrap();
        assert_eq!(cell.reason, Some(UnavailableReason::MissingForecastBase));
    }
    // An explicit value inside the suffix is still published.
    let published = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(0.1), None, &[(3, 7.0)]))
        .unwrap();
    assert_eq!(published.get("x", &quarter(3)), Some(7.0));
    assert_eq!(
        published.get_cell("x", &quarter(4)).unwrap().reason,
        Some(UnavailableReason::MissingForecastBase)
    );

    // A selected start level is the base, with or without a preceding value.
    let start = ForecastSpec::growth(0.1)
        .with_param("start", json!(200.0))
        .unwrap();
    for actual in [None, Some(100.0)] {
        let result = Evaluator::new()
            .evaluate(&growth_model(start.clone(), actual, &[]))
            .unwrap();
        assert!((result.get("x", &quarter(2)).unwrap() - 220.0).abs() < 1e-9);
        assert!((result.get("x", &quarter(4)).unwrap() - 266.2).abs() < 1e-9);
    }
    assert!(
        ForecastSpec::growth(0.1)
            .with_param("start", json!(f64::NAN))
            .is_err()
    );
}

#[test]
fn bounds_clip_published_outputs_and_do_not_feed_the_next_step() {
    use finstack_quant_statements::evaluator::EvalWarning;
    // Growth of -50% from 100: raw 50, 25, 12.5. A floor of 30 clips Q3 and
    // Q4; if clipping fed the recurrence, Q4 would be 15, then floored at 30
    // from a different raw value.
    let floored = ForecastSpec::growth(-0.5)
        .with_param("min", json!(30.0))
        .unwrap();
    let result = Evaluator::new()
        .evaluate(&growth_model(floored, Some(100.0), &[]))
        .unwrap();
    assert_eq!(result.get("x", &quarter(2)), Some(50.0));
    assert_eq!(result.get("x", &quarter(3)), Some(30.0));
    assert_eq!(result.get("x", &quarter(4)), Some(30.0));
    let clipped: Vec<(PeriodId, f64, f64)> = result
        .meta
        .warnings
        .iter()
        .filter_map(|warning| match warning {
            EvalWarning::ForecastClipped {
                node_id,
                period,
                raw,
                published,
            } if node_id == "x" => Some((*period, *raw, *published)),
            _ => None,
        })
        .collect();
    assert_eq!(
        clipped,
        [(quarter(3), 25.0, 30.0), (quarter(4), 12.5, 30.0)],
        "each clip records the raw level the recurrence kept"
    );
}

// --- C3: deterministic forecasts and fades (STATEMENTS-EVALUATION-006) ---

#[test]
fn growth_below_minus_one_hundred_percent_flips_the_sign() {
    // -100% gives zero and stays there.
    let zero = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(-1.0), Some(100.0), &[]))
        .unwrap();
    for index in [2, 3, 4] {
        assert_eq!(zero.get("x", &quarter(index)), Some(0.0));
    }
    // -150% multiplies by -0.5 each period: -50, 25, -12.5.
    let flipped = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(-1.5), Some(100.0), &[]))
        .unwrap();
    for (index, expected) in [(2, -50.0), (3, 25.0), (4, -12.5)] {
        assert_eq!(flipped.get("x", &quarter(index)), Some(expected));
    }
    // The same recurrence on a negative level and through a curve.
    let curve = Evaluator::new()
        .evaluate(&growth_model(
            ForecastSpec::curve(vec![-2.0, 0.5, -1.0]),
            Some(-40.0),
            &[],
        ))
        .unwrap();
    for (index, expected) in [(2, 40.0), (3, 60.0), (4, 0.0)] {
        assert_eq!(curve.get("x", &quarter(index)), Some(expected));
    }
}

#[test]
fn override_slots_before_the_first_key_carry_the_base_or_are_unavailable() {
    use finstack_quant_statements::evaluator::UnavailableReason;
    let keyed = || ForecastSpec::overrides([(quarter(3), 130.0)].into_iter().collect());
    // With a base, the leading slot carries it; the key is carried after.
    let based = Evaluator::new()
        .evaluate(&growth_model(keyed(), Some(100.0), &[]))
        .unwrap();
    for (index, expected) in [(2, 100.0), (3, 130.0), (4, 130.0)] {
        assert_eq!(based.get("x", &quarter(index)), Some(expected));
    }
    // Without one, the leading slot is unavailable with a reason and the
    // keyed levels are still published.
    let unbased = Evaluator::new()
        .evaluate(&growth_model(keyed(), None, &[]))
        .unwrap();
    assert_eq!(
        unbased.get_cell("x", &quarter(2)).unwrap().reason,
        Some(UnavailableReason::MissingForecastBase)
    );
    for index in [3, 4] {
        assert_eq!(unbased.get("x", &quarter(index)), Some(130.0));
    }
}

#[test]
fn geometric_fade_is_representable_when_the_endpoint_ratio_is_not() {
    // 1e-200 to 1e200 over three periods: the ratio 1e400 overflows, the
    // levels 1e-200 x 10^(400 t / 3) do not.
    for (base, target) in [(1e-200, 1e200), (1e200, 1e-200), (-1e200, -1e-200)] {
        let result = Evaluator::new()
            .evaluate(&growth_model(
                ForecastSpec::fade_to_target(target, ForecastFadeShape::Geometric),
                Some(base),
                &[],
            ))
            .unwrap();
        let exponent =
            |t: f64| base.abs().log10() + (target.abs().log10() - base.abs().log10()) * t / 3.0;
        for (index, t) in [(2, 1.0), (3, 2.0)] {
            let value = result.get("x", &quarter(index)).unwrap();
            assert_eq!(value.signum(), target.signum());
            assert!((value.abs().log10() - exponent(t)).abs() < 1e-9);
        }
        assert_eq!(
            result.get("x", &quarter(4)).unwrap().to_bits(),
            target.to_bits()
        );
    }
}

#[test]
fn geometric_fade_rejects_opposite_sign_and_zero_endpoints() {
    // A zero target, and a start level across zero, fail when the
    // specification is built.
    assert!(
        ForecastSpec::fade_to_target(0.0, ForecastFadeShape::Geometric)
            .validate()
            .is_err()
    );
    assert!(
        ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Geometric)
            .with_param("start", json!(-10.0))
            .unwrap()
            .validate()
            .is_err()
    );
    // A base across zero from the target fails when the forecast runs.
    for base in [-100.0, 0.0] {
        let model = growth_model(
            ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Geometric),
            Some(base),
            &[],
        );
        let error = Evaluator::new().evaluate(&model).unwrap_err();
        assert!(error.to_string().contains("same sign"), "{error}");
    }
    // An exponential fade without a half-life fails statically.
    assert!(
        ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Exponential)
            .validate()
            .is_err()
    );
}

#[test]
fn exponential_fade_halves_the_remaining_gap_each_half_life() {
    let fade = ForecastSpec::fade_to_target(100.0, ForecastFadeShape::Exponential)
        .with_param("half_life", json!(1.0))
        .unwrap();
    let result = Evaluator::new()
        .evaluate(&growth_model(fade, Some(-60.0), &[]))
        .unwrap();
    for (index, expected) in [(2, 20.0), (3, 60.0), (4, 80.0)] {
        assert!((result.get("x", &quarter(index)).unwrap() - expected).abs() < 1e-12);
    }
}

// --- C4: stochastic kernels and versioned streams (STATEMENTS-EVALUATION-007) ---

/// Mean and variance of node `x` in the last period across Monte Carlo paths.
fn terminal_moments(forecast: ForecastSpec, base: f64, paths: usize) -> (f64, f64) {
    let config = MonteCarloConfig::new(paths, 20_261_117, [""; 0]).with_path_data(true);
    let results = Evaluator::new()
        .evaluate_all_metrics(&growth_model(forecast, Some(base), &[]), &config)
        .unwrap();
    // Path data is ordered by path, then period: the last period of each
    // path is every third value.
    let terminal: Vec<f64> = metric_paths(&results, "x")
        .chunks(3)
        .map(|path| path[2])
        .collect();
    assert_eq!(terminal.len(), paths);
    let mean = terminal.iter().sum::<f64>() / paths as f64;
    let variance = terminal.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (paths as f64 - 1.0);
    (mean, variance)
}

/// Sample moments after three steps against closed forms derived
/// independently of the kernels.
#[test]
fn stochastic_kernels_match_closed_form_moments() {
    let paths = 20_000;
    let n = paths as f64;
    // Normal walk from -50 (a signed level): mean x0 + 3 mu, variance 3 sigma^2.
    let (mean, variance) = terminal_moments(ForecastSpec::normal(2.0, 4.0, 1), -50.0, paths);
    let (expected_mean, expected_variance) = (-50.0 + 3.0 * 2.0, 3.0 * 16.0);
    assert!((mean - expected_mean).abs() < 4.0 * (expected_variance / n).sqrt());
    // The sample variance of a normal has standard error var * sqrt(2 / (n - 1)).
    assert!((variance - expected_variance).abs() < 4.0 * expected_variance * (2.0 / n).sqrt());

    // Lognormal: the expected gross multiplier per step is exp(mu), so the
    // mean is x0 exp(3 mu) and the variance mean^2 (exp(3 sigma^2) - 1).
    let (mu, sigma) = (0.03, 0.2);
    let (mean, variance) = terminal_moments(ForecastSpec::log_normal(mu, sigma, 2), 80.0, paths);
    let expected_mean = 80.0 * (3.0 * mu).exp();
    let expected_variance = expected_mean.powi(2) * ((3.0 * sigma * sigma).exp() - 1.0);
    assert!((mean - expected_mean).abs() < 4.0 * (expected_variance / n).sqrt());
    assert!((variance / expected_variance - 1.0).abs() < 0.1);

    // Mean reversion from -10 (signed) toward 5 with kappa = 0.4:
    // mean theta + (x0 - theta)(1 - kappa)^3, variance sigma^2 (1 + a^2 + a^4)
    // with a = 1 - kappa.
    let (theta, kappa, sigma) = (5.0, 0.4, 1.5);
    let (mean, variance) = terminal_moments(
        ForecastSpec::mean_reverting(theta, kappa, sigma, 3),
        -10.0,
        paths,
    );
    let a: f64 = 1.0 - kappa;
    let expected_mean = theta + (-10.0 - theta) * a.powi(3);
    let expected_variance = sigma * sigma * (1.0 + a.powi(2) + a.powi(4));
    assert!((mean - expected_mean).abs() < 4.0 * (expected_variance / n).sqrt());
    assert!((variance - expected_variance).abs() < 4.0 * expected_variance * (2.0 / n).sqrt());
}

/// A single run replays from the seed and stream its provenance records,
/// with no access to the key derivation.
#[test]
fn a_single_run_replays_from_its_recorded_stream() {
    use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
    let seed = 9_007_199_254_740_993;
    let result = Evaluator::new()
        .evaluate(&growth_model(
            ForecastSpec::normal(1.5, 3.0, seed),
            Some(100.0),
            &[],
        ))
        .unwrap();
    let streams = &result.provenance.as_ref().unwrap().forecast_streams;
    assert_eq!(streams.len(), 1);
    let recorded = &streams[0];
    assert_eq!(
        (
            recorded.node_id.as_str(),
            recorded.seed,
            recorded.stream_version
        ),
        ("x", seed, 1)
    );
    let mut rng = Pcg64Rng::new_with_stream(recorded.state, recorded.stream);
    let mut level = 100.0;
    for index in [2, 3, 4] {
        level += 1.5 + 3.0 * rng.normal(0.0, 1.0);
        assert_eq!(
            result.get("x", &quarter(index)).unwrap().to_bits(),
            level.to_bits()
        );
    }
    // The record survives the wire with its integers exact.
    let wire = serde_json::to_value(recorded).unwrap();
    assert_eq!(wire["seed"], json!("9007199254740993"));
    assert_eq!(
        serde_json::from_value::<finstack_quant_statements::evaluator::ForecastStream>(wire)
            .unwrap(),
        *recorded
    );
    // A model without a stochastic forecast records no stream.
    let deterministic = Evaluator::new()
        .evaluate(&growth_model(ForecastSpec::growth(0.1), Some(100.0), &[]))
        .unwrap();
    assert!(
        deterministic
            .provenance
            .unwrap()
            .forecast_streams
            .is_empty()
    );
}

/// Zero volatility is the deterministic transition, exactly.
#[test]
fn zero_volatility_is_the_deterministic_transition() {
    let normal = Evaluator::new()
        .evaluate(&growth_model(
            ForecastSpec::normal(-2.5, 0.0, 9),
            Some(-10.0),
            &[],
        ))
        .unwrap();
    for (index, expected) in [(2, -12.5), (3, -15.0), (4, -17.5)] {
        assert_eq!(normal.get("x", &quarter(index)), Some(expected));
    }
    let reverting = Evaluator::new()
        .evaluate(&growth_model(
            ForecastSpec::mean_reverting(0.0, 0.5, 0.0, 9),
            Some(-16.0),
            &[],
        ))
        .unwrap();
    for (index, expected) in [(2, -8.0), (3, -4.0), (4, -2.0)] {
        assert_eq!(reverting.get("x", &quarter(index)), Some(expected));
    }
}

#[test]
fn bootstrap_modes_state_their_domains() {
    use finstack_quant_statements::types::ForecastBootstrapMode;
    let history = vec![100.0, 110.0, 99.0];
    // Growth needs strictly positive history, start level and anchor.
    assert!(
        ForecastSpec::bootstrap(vec![100.0, 0.0, 5.0], ForecastBootstrapMode::Growth, 1)
            .validate()
            .is_err()
    );
    assert!(
        ForecastSpec::bootstrap(vec![100.0], ForecastBootstrapMode::Diff, 1)
            .validate()
            .is_err()
    );
    let growth = ForecastSpec::bootstrap(history.clone(), ForecastBootstrapMode::Growth, 1);
    assert!(
        growth
            .clone()
            .with_param("start", json!(-1.0))
            .unwrap()
            .validate()
            .is_err()
    );
    let error = Evaluator::new()
        .evaluate(&growth_model(growth.clone(), Some(-5.0), &[]))
        .unwrap_err();
    assert!(error.to_string().contains("strictly positive"), "{error}");
    // Every growth step is one of the two observed growth rates.
    let result = Evaluator::new()
        .evaluate(&growth_model(growth, Some(200.0), &[]))
        .unwrap();
    let mut level = 200.0;
    for index in [2, 3, 4] {
        let next = result.get("x", &quarter(index)).unwrap();
        let rate: f64 = next / level - 1.0;
        assert!(
            (rate - 0.1).abs() < 1e-12 || (rate + 0.1).abs() < 1e-12,
            "{rate}"
        );
        level = next;
    }
    // Difference mode supports signed levels: every step is +10 or -11.
    let diff = ForecastSpec::bootstrap(history, ForecastBootstrapMode::Diff, 1);
    let result = Evaluator::new()
        .evaluate(&growth_model(diff, Some(-3.0), &[]))
        .unwrap();
    let mut level = -3.0;
    for index in [2, 3, 4] {
        let next = result.get("x", &quarter(index)).unwrap();
        assert!([10.0, -11.0].contains(&(next - level)), "{}", next - level);
        level = next;
    }
}

// --- C5: latent one-peer correlation (STATEMENTS-EVALUATION-008) ---

fn correlated_with(forecast: ForecastSpec, peer: &str, rho: f64) -> ForecastSpec {
    forecast
        .with_param("correlation_with", json!(peer))
        .unwrap()
        .with_param("correlation", json!(rho))
        .unwrap()
}

/// Scalar nodes at 100 in the one actual period, declared in the given order.
fn peer_model_builder(nodes: &[(&str, ForecastSpec)]) -> ModelBuilder {
    let mut builder = ModelBuilder::new("peers");
    builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
    for (node, forecast) in nodes {
        builder.value_scalar(*node, &[(quarter(1), 100.0)]);
        builder.forecast(*node, forecast.clone());
    }
    builder
}

fn peer_model(nodes: &[(&str, ForecastSpec)]) -> FinancialModelSpec {
    peer_model_builder(nodes).build().unwrap()
}

/// First-period changes of a node across paths: with a unit-volatility
/// driftless normal walk from 100 this is the node's standardized shock.
fn first_shocks(model: &FinancialModelSpec, node: &str, paths: usize) -> Vec<f64> {
    let config = MonteCarloConfig::new(paths, 5, [""; 0]).with_path_data(true);
    let results = Evaluator::new()
        .evaluate_all_metrics(model, &config)
        .unwrap();
    metric_paths(&results, node)
        .chunks(3)
        .map(|path| path[0] - 100.0)
        .collect()
}

fn sample_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let (mean_x, mean_y) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let covariance: f64 = x
        .iter()
        .zip(y)
        .map(|(a, b)| (a - mean_x) * (b - mean_y))
        .sum();
    let variance =
        |values: &[f64], mean: f64| -> f64 { values.iter().map(|v| (v - mean).powi(2)).sum() };
    covariance / (variance(x, mean_x) * variance(y, mean_y)).sqrt()
}

/// A three-node chain gives the same paths whatever order its nodes are
/// declared in, and each link has the configured correlation; the ends of
/// the chain have the product of the two coefficients.
#[test]
fn a_three_node_chain_is_ordered_by_its_peers_not_its_declaration() {
    let unit = |seed| ForecastSpec::normal(0.0, 1.0, seed);
    let head = ("head", unit(1));
    let middle = ("middle", correlated_with(unit(2), "head", 0.8));
    let tail = ("tail", correlated_with(unit(3), "middle", -0.5));
    let forward = peer_model(&[head.clone(), middle.clone(), tail.clone()]);
    let reversed = peer_model(&[tail, middle, head]);

    let paths = 20_000;
    let shocks = |model: &FinancialModelSpec, node: &str| first_shocks(model, node, paths);
    for node in ["head", "middle", "tail"] {
        assert_eq!(shocks(&forward, node), shocks(&reversed, node), "{node}");
    }
    let (head, middle, tail) = (
        shocks(&forward, "head"),
        shocks(&forward, "middle"),
        shocks(&forward, "tail"),
    );
    // The standard error of a sample correlation is about (1 - r^2) / sqrt(n),
    // at most 0.0071 here; four standard errors is under 0.03.
    assert!((sample_correlation(&head, &middle) - 0.8).abs() < 0.03);
    assert!((sample_correlation(&middle, &tail) + 0.5).abs() < 0.03);
    assert!((sample_correlation(&head, &tail) + 0.4).abs() < 0.03);
    // Each mixed shock keeps unit variance.
    for shocks in [&middle, &tail] {
        let variance = shocks.iter().map(|z| z * z).sum::<f64>() / paths as f64;
        assert!((variance - 1.0).abs() < 0.04, "{variance}");
    }
}

/// At the endpoints the peer shock is copied or negated exactly.
#[test]
fn unit_correlation_copies_or_negates_the_peer_shock_exactly() {
    for rho in [1.0, -1.0] {
        let model = peer_model(&[
            (
                "follower",
                correlated_with(ForecastSpec::normal(0.0, 1.0, 2), "peer", rho),
            ),
            ("peer", ForecastSpec::normal(0.0, 1.0, 1)),
        ]);
        let peer = first_shocks(&model, "peer", 64);
        let follower = first_shocks(&model, "follower", 64);
        for (peer, follower) in peer.iter().zip(&follower) {
            assert_eq!(follower.to_bits(), (rho * peer).to_bits());
        }
    }
}

#[test]
fn peer_cycles_and_volatility_free_peers_are_rejected_at_build() {
    let unit = |seed| ForecastSpec::normal(0.0, 1.0, seed);
    // A two-node and a three-node cycle.
    let two = peer_model_builder(&[
        ("a", correlated_with(unit(1), "b", 0.5)),
        ("b", correlated_with(unit(2), "a", 0.5)),
    ])
    .build()
    .unwrap_err();
    assert!(two.to_string().contains("cycle"), "{two}");
    let three = peer_model_builder(&[
        ("a", correlated_with(unit(1), "b", 0.5)),
        ("b", correlated_with(unit(2), "c", 0.5)),
        ("c", correlated_with(unit(3), "a", 0.5)),
    ])
    .build()
    .unwrap_err();
    assert!(three.to_string().contains("cycle"), "{three}");
    // A zero coefficient is not a dependency, so it closes no cycle.
    peer_model(&[
        ("a", correlated_with(unit(1), "b", 0.5)),
        ("b", correlated_with(unit(2), "a", 0.0)),
    ]);
    // An active correlation needs positive volatility on both nodes.
    let flat = ForecastSpec::normal(0.0, 0.0, 9);
    for nodes in [
        [
            ("a", correlated_with(unit(1), "b", 0.5)),
            ("b", flat.clone()),
        ],
        [("a", correlated_with(flat, "b", 0.5)), ("b", unit(2))],
    ] {
        let error = peer_model_builder(&nodes).build().unwrap_err();
        assert!(error.to_string().contains("std_dev"), "{error}");
    }
    // The peer is a known, distinct stochastic node.
    for peer in ["a", "missing"] {
        assert!(
            peer_model_builder(&[("a", correlated_with(unit(1), peer, 0.5))])
                .build()
                .is_err()
        );
    }
}

/// A non-zero correlation needs Monte Carlo; a zero one is an ordinary node.
#[test]
fn active_correlation_is_rejected_in_a_single_run() {
    let unit = |seed| ForecastSpec::normal(0.0, 1.0, seed);
    let model = |rho| peer_model(&[("a", correlated_with(unit(1), "b", rho)), ("b", unit(2))]);
    let error = Evaluator::new().evaluate(&model(0.5)).unwrap_err();
    assert!(error.to_string().contains("Monte Carlo"), "{error}");
    let uncorrelated = Evaluator::new()
        .evaluate(&peer_model(&[("a", unit(1)), ("b", unit(2))]))
        .unwrap();
    let zero = Evaluator::new().evaluate(&model(0.0)).unwrap();
    assert_eq!(
        zero.get("a", &quarter(4)),
        uncorrelated.get("a", &quarter(4))
    );
}

/// The peer's shocks are generated even when it publishes explicit values
/// through the whole suffix, so the follower's paths do not change.
#[test]
fn peer_shocks_follow_the_peer_forecast_not_its_published_cells() {
    let unit = |seed| ForecastSpec::normal(0.0, 1.0, seed);
    let follower = ("follower", correlated_with(unit(2), "peer", 0.7));
    let config = MonteCarloConfig::new(8, 5, [""; 0]).with_path_data(true);

    // The peer publishes explicit values in every forecast period.
    let mut published = peer_model_builder(std::slice::from_ref(&follower));
    published.value_scalar(
        "peer",
        &[
            (quarter(1), 100.0),
            (quarter(2), 1.0),
            (quarter(3), 2.0),
            (quarter(4), 3.0),
        ],
    );
    published.forecast("peer", unit(1));
    let published = published.build().unwrap();
    let plain = peer_model(&[follower.clone(), ("peer", unit(1))]);
    let run = |model: &FinancialModelSpec| {
        let results = Evaluator::new()
            .evaluate_all_metrics(model, &config)
            .unwrap();
        metric_paths(&results, "follower")
    };
    assert_eq!(run(&published), run(&plain));
}

// --- C6: trend extrapolation (STATEMENTS-EVALUATION-009) ---

fn trend_values(forecast: ForecastSpec) -> [f64; 3] {
    let result = Evaluator::new()
        .evaluate(&growth_model(forecast, Some(100.0), &[]))
        .unwrap();
    [2, 3, 4].map(|index| result.get("x", &quarter(index)).unwrap())
}

/// Damped Holt against a recursion worked by hand for [10, 12, 15] with
/// alpha = beta = 0.5 and phi = 0.9: the initial trend is (15 - 10) / 2 =
/// 2.5; the updates give L = 12.125, B = 2.1875 and then L = 14.546875,
/// B = 2.1953125; the forecasts add B times 0.9, 1.71 and 2.439.
#[test]
fn damped_holt_matches_a_hand_recursion() {
    let holt = ForecastSpec::time_series(vec![10.0, 12.0, 15.0], ForecastTrendMethod::Exponential)
        .with_param("alpha", json!(0.5))
        .unwrap()
        .with_param("beta", json!(0.5))
        .unwrap()
        .with_param("phi", json!(0.9))
        .unwrap();
    let expected = [
        14.546875 + 2.1953125 * 0.9,
        14.546875 + 2.1953125 * 1.71,
        14.546875 + 2.1953125 * 2.439,
    ];
    for (value, expected) in trend_values(holt).iter().zip(expected) {
        assert!((value - expected).abs() < 1e-12, "{value} vs {expected}");
    }
}

/// The moving-average rule: with the window equal to the history the
/// forecast is the constant mean; with one more observation the trend is
/// the difference of the two trailing means, centred by (W - 1) / 2.
#[test]
fn moving_average_is_constant_when_the_window_is_the_history() {
    let moving = |history: Vec<f64>| {
        ForecastSpec::time_series(history, ForecastTrendMethod::MovingAverage)
            .with_param("window", json!(3))
            .unwrap()
    };
    assert_eq!(trend_values(moving(vec![10.0, 20.0, 60.0])), [30.0; 3]);
    // Means 30 and 20, trend 10: 30 + 10 (1 + h) continues the line.
    assert_eq!(
        trend_values(moving(vec![10.0, 20.0, 30.0, 40.0])),
        [50.0, 60.0, 70.0]
    );
}

/// The linear trend keeps a small slope at a large level.
#[test]
fn linear_trend_keeps_a_small_slope_at_a_large_level() {
    let history: Vec<f64> = (1..=6).map(|t| 1e15 + 3.0 * f64::from(t)).collect();
    let values = trend_values(ForecastSpec::time_series(
        history,
        ForecastTrendMethod::Linear,
    ));
    for (step, value) in values.iter().enumerate() {
        let expected = 1e15 + 3.0 * (7 + step) as f64;
        assert!((value - expected).abs() <= 0.5, "{value} vs {expected}");
    }
}

/// Trend parameters are checked when the specification is validated, with
/// no forecast period needed.
#[test]
fn trend_parameters_are_checked_statically() {
    let exponential = |alpha: f64, beta: f64| {
        ForecastSpec::time_series(vec![1.0, 2.0, 3.0], ForecastTrendMethod::Exponential)
            .with_param("alpha", json!(alpha))
            .unwrap()
            .with_param("beta", json!(beta))
            .unwrap()
    };
    assert!(exponential(0.5, 0.5).validate().is_ok());
    for (alpha, beta) in [(1.0, 0.5), (0.5, 1.0), (0.0, 0.5), (0.5, 0.0)] {
        assert!(
            exponential(alpha, beta).validate().is_err(),
            "{alpha}, {beta}"
        );
    }
    // The damping factor may be exactly one, and not above it.
    for (phi, valid) in [(1.0, true), (0.5, true), (1.5, false), (0.0, false)] {
        let spec = exponential(0.5, 0.5).with_param("phi", json!(phi)).unwrap();
        assert_eq!(spec.validate().is_ok(), valid, "{phi}");
    }
    // The exponential method needs both smoothing factors.
    assert!(
        ForecastSpec::time_series(vec![1.0, 2.0, 3.0], ForecastTrendMethod::Exponential)
            .validate()
            .is_err()
    );
    // At least two observations, and a window no longer than the history.
    assert!(
        ForecastSpec::time_series(vec![1.0], ForecastTrendMethod::Linear)
            .validate()
            .is_err()
    );
    let window = |window: u64| {
        ForecastSpec::time_series(vec![1.0, 2.0, 3.0], ForecastTrendMethod::MovingAverage)
            .with_param("window", json!(window))
            .unwrap()
    };
    assert!(window(3).validate().is_ok());
    assert!(window(4).validate().is_err());
}

// --- C7: seasonal decomposition (STATEMENTS-EVALUATION-010) ---

/// Through the evaluator: two exact cycles of `2 t + s` forecast the final
/// trend plus each phase's effect, and a multiplicative series with a zero
/// observation forecasts its pattern at the trend level.
#[test]
fn seasonal_forecasts_continue_trend_and_phase_from_the_history() {
    let pattern = [3.0, -1.0, -4.0, 2.0];
    let history: Vec<f64> = (0..8)
        .map(|i| 2.0 * (i + 1) as f64 + pattern[i % 4])
        .collect();
    let additive = trend_values(ForecastSpec::seasonal(history, 4, SeasonalMode::Additive));
    for (value, effect) in additive.iter().zip(pattern) {
        assert!((value - (16.0 + effect)).abs() < 1e-12, "{value}");
    }
    let multiplicative = trend_values(ForecastSpec::seasonal(
        vec![0.0, 20.0, 30.0, 50.0, 0.0, 20.0, 30.0, 50.0],
        4,
        SeasonalMode::Multiplicative,
    ));
    for (value, expected) in multiplicative.iter().zip([0.0, 20.0, 30.0]) {
        assert!((value - expected).abs() < 1e-12, "{value}");
    }
    // Static requirements: two complete seasons, and growth above -1 in
    // multiplicative mode.
    assert!(
        ForecastSpec::seasonal(vec![1.0; 7], 4, SeasonalMode::Additive)
            .validate()
            .is_err()
    );
    assert!(
        ForecastSpec::seasonal(vec![1.0; 8], 4, SeasonalMode::Multiplicative)
            .with_param("growth", json!(-1.0))
            .unwrap()
            .validate()
            .is_err()
    );
}
