//! Forecast counterexamples from the quantitative finance review.

use finstack_quant_statements::evaluator::{EvalWarning, MonteCarloConfig};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::SeasonalMode;
use serde_json::json;

fn quarter(index: u8) -> PeriodId {
    PeriodId::quarter(2025, index).unwrap()
}

fn forecast_model(base: f64, forecast: ForecastSpec) -> FinancialModelSpec {
    ModelBuilder::new("forecast-regression")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value("x", &[(quarter(1), base.into())])
        .forecast("x", forecast)
        .build()
        .unwrap()
}

#[test]
fn override_period_aliases_apply_to_their_parsed_periods() {
    let january = PeriodId::month(2025, 1).unwrap();
    let february = PeriodId::month(2025, 2).unwrap();
    let march = PeriodId::month(2025, 3).unwrap();
    for spelling in ["2025M2", "2025m02", " 2025M02 "] {
        let forecast = ForecastSpec {
            method: ForecastMethod::Override,
            params: [("overrides".into(), json!({spelling: 200.0}))]
                .into_iter()
                .collect(),
        };
        let model = ModelBuilder::new("monthly-overrides")
            .periods("2025M1..M3", Some("2025M1"))
            .unwrap()
            .value("x", &[(january, 100.0.into())])
            .forecast("x", forecast)
            .build()
            .unwrap();
        let result = Evaluator::new().evaluate(&model).unwrap();
        assert_eq!(result.get("x", &february), Some(200.0), "{spelling}");
        assert_eq!(result.get("x", &march), Some(200.0), "{spelling}");
    }
}

#[test]
fn override_duplicate_period_aliases_are_rejected() {
    let forecast = ForecastSpec {
        method: ForecastMethod::Override,
        params: [(
            "overrides".into(),
            json!({"2025Q2": 200.0, "2025q2": 300.0}),
        )]
        .into_iter()
        .collect(),
    };
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
            forecast.params.insert(key.into(), json!(value));
        }
        let model = forecast_model(f64::NAN, forecast);
        let result = Evaluator::new().evaluate(&model).unwrap();
        for index in 1..=4 {
            let period = quarter(index);
            assert!(result.get("x", &period).unwrap().is_nan());
            assert!(result.meta.warnings.iter().any(|warning| matches!(
                warning,
                EvalWarning::NonFiniteValue { node_id, period: warned_period, value }
                    if node_id == "x" && *warned_period == period && value.is_nan()
            )));
        }
    }
}

#[test]
fn malformed_optional_forecast_parameters_fail_in_both_evaluation_modes() {
    let mut bad_growth = ForecastSpec::seasonal(vec![100.0; 8], 4, SeasonalMode::Additive);
    bad_growth.params.insert("growth".into(), json!("0.1"));
    let mut bad_method = ForecastSpec::time_series(vec![90.0, 100.0]);
    bad_method.params.insert("method".into(), json!(true));
    let mut bad_correlation = ForecastSpec::normal(0.0, 1.0, 42);
    bad_correlation
        .params
        .insert("correlation_with".into(), json!(1));
    bad_correlation
        .params
        .insert("correlation".into(), json!("0.5"));
    for invalid in [bad_growth, bad_method, bad_correlation] {
        assert!(invalid.validate().is_err());
        // Mutate a valid model so this exercises runtime validation even when
        // normal construction already validates the forecast specification.
        let mut model = forecast_model(100.0, ForecastSpec::forward_fill());
        model.nodes.get_mut("x").unwrap().forecast = Some(invalid);
        assert!(Evaluator::new().evaluate(&model).is_err());
        assert!(Evaluator::new()
            .evaluate_monte_carlo(&model, &MonteCarloConfig::new(2, 7))
            .is_err());
    }
}

#[test]
fn correlated_forecasts_reject_malformed_optional_parameters() {
    let mut model = forecast_model(100.0, ForecastSpec::normal(0.0, 1.0, 42));
    let mut peer = model.nodes["x"].clone();
    peer.node_id = "peer".into();
    model.nodes.insert("peer".into(), peer);
    let forecast = model.nodes.get_mut("x").unwrap().forecast.as_mut().unwrap();
    forecast
        .params
        .insert("correlation_with".into(), json!("peer"));
    forecast.params.insert("correlation".into(), json!(0.5));
    forecast.params.insert("max".into(), json!("150"));
    let error = Evaluator::new()
        .evaluate_monte_carlo(&model, &MonteCarloConfig::new(2, 7))
        .unwrap_err();
    assert!(error.to_string().contains("max"), "{error}");
}

#[test]
fn extreme_season_lengths_fail_before_allocation() {
    for length in [u64::MAX, u64::from(u32::MAX) + 5] {
        let mut forecast = ForecastSpec::seasonal(vec![100.0; 8], 4, SeasonalMode::Additive);
        forecast
            .params
            .insert("season_length".into(), json!(length));
        let model = forecast_model(100.0, forecast);
        assert!(Evaluator::new().evaluate(&model).is_err());
    }
}

#[test]
fn bounded_constant_moving_average_rejects_overflow() {
    let mut forecast = ForecastSpec::time_series(vec![1e308, 1e308]);
    forecast
        .params
        .insert("method".into(), json!("moving_average"));
    forecast.params.insert("window".into(), json!(2));
    forecast.params.insert("max".into(), json!(100.0));
    let model = forecast_model(1e308, forecast);
    let error = Evaluator::new().evaluate(&model).unwrap_err();
    assert!(error.to_string().contains("non-finite historical average"));
}

#[test]
fn fades_between_finite_opposite_extremes_remain_finite() {
    for exponential in [false, true] {
        let mut forecast = ForecastSpec::fade_to_target(-1e308);
        if exponential {
            forecast.params.insert("shape".into(), json!("exponential"));
            forecast.params.insert("half_life".into(), json!(1.0));
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

#[test]
fn integer_valued_float_seeds_work_and_out_of_range_seeds_fail() {
    let mut forecast = ForecastSpec::normal(0.0, 1.0, 42);
    forecast.params.insert("seed".into(), json!(42.0));
    forecast.validate().unwrap();
    let model = forecast_model(100.0, forecast);
    assert!(Evaluator::new().evaluate(&model).is_ok());

    let mut invalid = ForecastSpec::normal(0.0, 1.0, 42);
    // u64::MAX rounds up to 2^64 as f64 and must not saturate back to u64::MAX.
    invalid.params.insert("seed".into(), json!(u64::MAX as f64));
    assert!(invalid.validate().is_err());
}
