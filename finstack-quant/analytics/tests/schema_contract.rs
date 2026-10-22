//! Generated analytics schemas describe exactly what serde writes.
//!
//! Every contract with a custom wire shape (non-finite `f64` sentinels, the
//! `Performance` state document) is serialized, validated against its
//! registered schema and read back.

use finstack_quant_analytics::schema::ARTIFACTS;
use finstack_quant_analytics::{
    BetaResult, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns, MultiFactorResult,
    Performance, PeriodStats, RollingGreeks,
};
use finstack_quant_core::dates::{Date, PeriodKind};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use time::Month;

fn schema(type_name: &str) -> Value {
    ARTIFACTS
        .iter()
        .find(|artifact| artifact.type_name() == type_name)
        .unwrap_or_else(|| panic!("{type_name} is not registered"))
        .generate()
        .expect("schema renders")
}

/// Serialize `value`, validate it against the `type_name` schema, read it back
/// and require the second serialization to match the first.
fn assert_wire_matches_schema<T: Serialize + DeserializeOwned>(type_name: &str, value: &T) {
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema(type_name))
        .expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&wire)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{type_name}: {errors:?}\n{wire}");
    let back: T = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(serde_json::to_value(&back).expect("reserialize"), wire);
}

fn from_json<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("fixture deserializes")
}

#[test]
fn non_finite_results_validate_against_their_schemas() {
    let beta: BetaResult = from_json(json!({
        "beta": "nan", "std_err": "inf", "ci_lower": "-inf", "ci_upper": 1.25, "n_obs": 6, "confidence_level": 0.95
    }));
    assert_wire_matches_schema("BetaResult", &beta);

    let greeks: GreeksResult = from_json(json!({
        "alpha": 0.01, "beta": "nan", "r_squared": "nan", "adjusted_r_squared": "-inf", "n_obs": 6
    }));
    assert_wire_matches_schema("GreeksResult", &greeks);

    let multi: MultiFactorResult = from_json(json!({
        "alpha": 0.02, "betas": [0.5, 1.1], "r_squared": "nan",
        "adjusted_r_squared": "nan", "residual_vol": 0.1, "n_obs": 6
    }));
    assert_wire_matches_schema("MultiFactorResult", &multi);

    let stats: PeriodStats = from_json(json!({
        "best": 0.05, "worst": -0.02, "consecutive_wins": 3, "consecutive_losses": 1,
        "win_rate": 0.6, "avg_return": 0.01, "avg_win": 0.02, "avg_loss": -0.01,
        "payoff_ratio": "inf", "profit_factor": "inf", "cpc_ratio": "inf",
        "kelly_criterion": 0.4
    }));
    assert_wire_matches_schema("PeriodStats", &stats);

    let series: DatedSeries = from_json(json!({
        "values": [0.1, "nan", "inf"],
        "dates": ["2025-01-02", "2025-01-03", "2025-01-06"],
        "value_column": "sharpe"
    }));
    assert_wire_matches_schema("DatedSeries", &series);
}

#[test]
fn schema_rejects_an_unknown_sentinel() {
    let validator = jsonschema::options()
        .build(&schema("BetaResult"))
        .expect("schema compiles");
    let wire = json!({ "beta": "infinite", "std_err": 0.1, "ci_lower": 0.0, "ci_upper": 1.0, "n_obs": 6, "confidence_level": 0.95 });
    assert!(!validator.is_valid(&wire));
}

#[test]
fn performance_state_validates_against_its_schema() {
    let dates: Vec<Date> = (2..=8)
        .map(|day| Date::from_calendar_date(2025, Month::January, day).expect("valid date"))
        .collect();
    let prices = vec![
        vec![100.0, 101.0, 100.5, 102.0, 103.0, 102.5, 104.0],
        vec![50.0, 50.5, 50.2, 51.0, 51.5, 51.2, 52.0],
    ];
    let perf = Performance::new(
        dates,
        prices,
        vec!["SPY".into(), "QQQ".into()],
        Some("SPY"),
        PeriodKind::Daily,
    )
    .expect("valid panel");
    assert_wire_matches_schema("Performance", &perf);
}

fn assert_strict_result<T: Serialize + DeserializeOwned>(
    type_name: &str,
    wire: Value,
    required_metadata: &[&str],
) {
    let value: T = from_json(wire.clone());
    assert_wire_matches_schema(type_name, &value);
    let validator = jsonschema::options()
        .build(&schema(type_name))
        .expect("valid schema");
    let mut extra = wire.clone();
    extra
        .as_object_mut()
        .expect("object")
        .insert("unknown_result_field".to_owned(), json!(true));
    assert!(
        serde_json::from_value::<T>(extra.clone()).is_err(),
        "{type_name} accepted an unknown field"
    );
    assert!(
        !validator.is_valid(&extra),
        "{type_name} schema accepted an unknown field"
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

#[test]
fn result_metadata_is_required_and_unknown_fields_are_rejected() {
    assert_strict_result::<BetaResult>(
        "BetaResult",
        json!({
            "beta": "nan", "std_err": "nan", "ci_lower": "nan", "ci_upper": "nan",
            "n_obs": 2, "confidence_level": 0.95
        }),
        &["n_obs", "confidence_level"],
    );
    assert_strict_result::<GreeksResult>(
        "GreeksResult",
        json!({
            "alpha": "nan", "beta": "nan", "r_squared": "nan", "adjusted_r_squared": "nan", "n_obs": 1
        }),
        &["n_obs"],
    );
    assert_strict_result::<MultiFactorResult>(
        "MultiFactorResult",
        json!({
            "alpha": 0.0, "betas": [1.0], "r_squared": "nan", "adjusted_r_squared": "nan", "residual_vol": 0.0, "n_obs": 4
        }),
        &["n_obs"],
    );
    assert_strict_result::<DrawdownEpisode>(
        "DrawdownEpisode",
        json!({
            "start": "2025-01-01", "valley": "2025-01-02", "end": null, "duration_days": 2,
            "max_drawdown": -0.1, "near_recovery_threshold": -0.001, "truncated_at_start": true
        }),
        &["truncated_at_start"],
    );
    assert_strict_result::<PeriodStats>(
        "PeriodStats",
        json!({
            "best": 0.01, "worst": -0.01, "consecutive_wins": 1, "consecutive_losses": 1,
            "win_rate": 0.5, "avg_return": 0.0, "avg_win": 0.01, "avg_loss": -0.01,
            "payoff_ratio": 1.0, "profit_factor": 1.0, "cpc_ratio": 0.5, "kelly_criterion": 0.0
        }),
        &[],
    );
    assert_strict_result::<RollingGreeks>(
        "RollingGreeks",
        json!({
            "dates": ["2025-01-02"], "alphas": [0.0], "betas": [1.0]
        }),
        &[],
    );
    assert_strict_result::<LookbackReturns>(
        "LookbackReturns",
        json!({
            "ticker_names": ["FUND"], "mtd": [0.1], "qtd": [0.1], "ytd": [0.1], "fytd": [0.1]
        }),
        &[],
    );
}

#[test]
fn drawdown_recovery_date_remains_optional() {
    let episode: DrawdownEpisode = from_json(json!({
        "start": "2025-01-01", "valley": "2025-01-02", "duration_days": 2,
        "max_drawdown": -0.1, "near_recovery_threshold": -0.001, "truncated_at_start": false
    }));
    assert!(episode.end.is_none());
}
