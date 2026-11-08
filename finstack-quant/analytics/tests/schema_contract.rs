//! Serde/schema agreement and restoration admission for analytics result carriers.

use finstack_quant_analytics::schema::ARTIFACTS;
use finstack_quant_analytics::{
    BetaResult, CorrelationResult, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns,
    MultiFactorResult, Performance, PeriodStats, RollingGreeks,
};
use finstack_quant_core::dates::{Date, FiscalConfig, PeriodKind};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use time::Month;

fn schema(type_name: &str) -> Value {
    ARTIFACTS
        .iter()
        .find(|artifact| artifact.type_name() == type_name)
        .unwrap_or_else(|| panic!("{type_name} is not registered"))
        .generate()
        .expect("schema renders")
}

fn assert_wire_matches_schema<T: Serialize + DeserializeOwned>(type_name: &str, value: &T) {
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema(type_name))
        .expect("schema compiles");
    let errors: Vec<_> = validator
        .iter_errors(&wire)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{type_name}: {errors:?}\n{wire}");
    let back: T = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(serde_json::to_value(&back).expect("reserialize"), wire);
}

fn assert_strict_result<T: Serialize + DeserializeOwned>(
    type_name: &str,
    value: &T,
    required_metadata: &[&str],
) {
    assert_wire_matches_schema(type_name, value);
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .build(&schema(type_name))
        .expect("schema compiles");
    let mut extra = wire.clone();
    extra
        .as_object_mut()
        .expect("object")
        .insert("unknown_result_field".into(), json!(true));
    assert!(
        serde_json::from_value::<T>(extra.clone()).is_err(),
        "{type_name} accepted unknown field"
    );
    assert!(
        !validator.is_valid(&extra),
        "{type_name} schema accepted unknown field"
    );
    for field in required_metadata {
        let mut missing = wire.clone();
        missing.as_object_mut().expect("object").remove(*field);
        assert!(
            serde_json::from_value::<T>(missing.clone()).is_err(),
            "{type_name} accepted missing {field}"
        );
        assert!(
            !validator.is_valid(&missing),
            "{type_name} schema accepted missing {field}"
        );
    }
}

fn fixture() -> Performance {
    let dates = (1..=4)
        .map(|day| Date::from_calendar_date(2025, Month::January, day).unwrap())
        .collect();
    Performance::new(&finstack_quant_analytics::PerformancePanel {
        dates,
        series: vec![vec![-0.1, 0.1, 0.0]],
        kind: finstack_quant_analytics::PanelSeriesKind::Returns,
        ticker_names: vec!["FUND".into()],
        benchmark_ticker: Some("FUND".into()),
        frequency: Some(PeriodKind::Daily),
    })
    .unwrap()
}

#[test]
fn computed_result_carriers_round_trip_through_their_schemas() {
    let perf = fixture();
    assert_strict_result::<Performance>(
        "Performance",
        &perf,
        &[
            "price_dates",
            "returns",
            "return_spans",
            "ticker_names",
            "start_idx",
            "end_idx",
        ],
    );
    assert_strict_result::<BetaResult>(
        "BetaResult",
        &perf.beta().unwrap()[0],
        &["n_obs", "confidence_level"],
    );
    assert_strict_result::<GreeksResult>("GreeksResult", &perf.greeks(0.0).unwrap()[0], &["n_obs"]);
    let factor = [-0.1, 0.1, 0.0];
    assert_strict_result::<MultiFactorResult>(
        "MultiFactorResult",
        &perf.multi_factor_greeks(0, &[&factor], 0.0).unwrap(),
        &["n_obs"],
    );
    assert_strict_result::<PeriodStats>(
        "PeriodStats",
        &perf.period_stats(0, PeriodKind::Daily, None).unwrap(),
        &["n_obs", "wins", "losses"],
    );
    assert_strict_result::<DrawdownEpisode>(
        "DrawdownEpisode",
        &perf.drawdown_details(0, 1).unwrap()[0],
        &["start", "valley", "duration_days", "max_drawdown"],
    );
    assert_strict_result::<RollingGreeks>(
        "RollingGreeks",
        &perf.rolling_greeks(0, 1, 0.0).unwrap(),
        &["dates", "alphas", "betas"],
    );
    assert_strict_result::<DatedSeries>(
        "DatedSeries",
        &perf.rolling_sharpe(0, 1, 0.0).unwrap(),
        &["dates", "values", "value_column"],
    );
    assert_strict_result::<LookbackReturns>(
        "LookbackReturns",
        &perf
            .lookback_returns(None, Some(FiscalConfig::calendar_year()))
            .unwrap(),
        &[
            "ref_date",
            "ticker_names",
            "fiscal_config",
            "mtd_samples",
            "qtd_samples",
            "ytd_samples",
            "fytd_samples",
        ],
    );
    assert_strict_result::<CorrelationResult>(
        "CorrelationResult",
        &perf.correlation_matrix().unwrap(),
        &["tickers", "matrix", "n_obs", "start_date", "end_date"],
    );
}

#[test]
fn unavailable_rolling_and_regression_results_preserve_nan_sentinels() {
    let mut perf = fixture();
    let date = perf.get_dates()[0];
    perf.reset_date_range(date, date).unwrap();
    let beta = perf.beta().unwrap()[0].clone();
    let beta_wire = serde_json::to_value(&beta).unwrap();
    for field in ["beta", "std_err", "ci_lower", "ci_upper"] {
        assert_eq!(beta_wire[field], "nan");
    }
    assert_wire_matches_schema("BetaResult", &beta);
    let rolling = fixture().rolling_greeks(0, 1, 0.0).unwrap();
    let rolling_wire = serde_json::to_value(&rolling).unwrap();
    assert_eq!(rolling_wire["alphas"], json!(["nan", "nan", "nan"]));
    assert_eq!(rolling_wire["betas"], json!(["nan", "nan", "nan"]));
    assert_wire_matches_schema("RollingGreeks", &rolling);
    let single_sided = Performance::new(&crate::support::returns_panel(
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        vec![
            Date::from_calendar_date(2025, Month::January, 2).unwrap(),
            Date::from_calendar_date(2025, Month::January, 3).unwrap(),
        ],
        vec![vec![0.1, 0.2]],
        vec!["GAIN".into()],
        None,
        PeriodKind::Daily,
    ))
    .unwrap()
    .period_stats(0, PeriodKind::Daily, None)
    .unwrap();
    assert!(single_sided.profit_factor.is_infinite());
    assert!(single_sided.payoff_ratio.is_nan());
    assert_wire_matches_schema("PeriodStats", &single_sided);
}

#[test]
fn malformed_result_state_and_unknown_sentinels_are_rejected() {
    let perf = fixture();
    let mut beta = serde_json::to_value(&perf.beta().unwrap()[0]).unwrap();
    beta["beta"] = json!("infinite");
    assert!(serde_json::from_value::<BetaResult>(beta.clone()).is_err());
    assert!(
        !jsonschema::options()
            .build(&schema("BetaResult"))
            .unwrap()
            .is_valid(&beta)
    );
    let mut rolling = serde_json::to_value(perf.rolling_greeks(0, 1, 0.0).unwrap()).unwrap();
    rolling["betas"] = json!(["nan"]);
    assert!(serde_json::from_value::<RollingGreeks>(rolling).is_err());
    let mut stats =
        serde_json::to_value(perf.period_stats(0, PeriodKind::Daily, None).unwrap()).unwrap();
    stats["wins"] = json!(99);
    assert!(serde_json::from_value::<PeriodStats>(stats).is_err());
    let mut correlation = serde_json::to_value(perf.correlation_matrix().unwrap()).unwrap();
    correlation["matrix"] = json!([[0.9]]);
    assert!(serde_json::from_value::<CorrelationResult>(correlation).is_err());
}

#[test]
fn open_drawdown_recovery_date_is_optional() {
    let mut wire = serde_json::to_value(&fixture().drawdown_details(0, 1).unwrap()[0]).unwrap();
    wire.as_object_mut().unwrap().remove("end");
    let episode: DrawdownEpisode = serde_json::from_value(wire).unwrap();
    assert!(episode.end.is_none());
}
