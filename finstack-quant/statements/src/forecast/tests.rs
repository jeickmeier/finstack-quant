use super::dispatch::ForecastSeries;
use super::*;
use crate::error::Result;
use crate::types::ForecastSpec;
use crate::types::{ForecastBootstrapMode, ForecastFadeShape, ForecastTrendMethod};
use finstack_quant_core::dates::PeriodId;
use serde_json::json;

fn quarters(n: u8) -> Vec<PeriodId> {
    (0..n)
        .map(|i| {
            PeriodId::quarter(2025 + i32::from(i / 4), i % 4 + 1).expect("valid period fixture")
        })
        .collect()
}

fn run(spec: &ForecastSpec, base_value: f64, periods: &[PeriodId]) -> Result<ForecastSeries> {
    apply_forecast_for_node(&spec.clone(), base_value, periods, "node")
}

#[test]
fn typed_parameters_round_trip_the_stored_wire_form() {
    let documents = [
        json!({"method": "forward_fill", "params": {"min": 0.0, "max": 1000.0}}),
        json!({"method": "growth_pct", "params": {"rate": 0.03}}),
        json!({"method": "curve_pct", "params": {"curve": [0.01, 0.02]}}),
        json!({"method": "override", "params": {"overrides": {"2025Q1": 1.0}}}),
        json!({"method": "normal", "params": {"mean": 1.0, "std_dev": 0.1, "seed": "7", "correlation_with": "peer", "correlation": 0.5}}),
        json!({"method": "log_normal", "params": {"mean": 0.0, "std_dev": 0.1, "seed": "7"}}),
        json!({"method": "mean_reverting", "params": {"long_run_mean": 1.0, "reversion_speed": 0.5, "std_dev": 0.1, "seed": "7"}}),
        json!({"method": "bootstrap", "params": {"historical": [1.0, 2.0], "mode": "diff", "seed": "7"}}),
        json!({"method": "time_series", "params": {"historical": [1.0, 2.0], "method": "exponential", "alpha": 0.5, "beta": 0.5, "phi": 0.9}}),
        json!({"method": "seasonal", "params": {"historical": [1.0, 2.0], "season_length": 2, "mode": "additive", "growth": 0.01}}),
        json!({"method": "fade_to_target", "params": {"target": 1.0, "shape": "exponential", "half_life": 2.0}}),
    ];
    for document in documents {
        let spec: ForecastSpec = serde_json::from_value(document.clone()).expect("spec");
        let typed = spec.clone();
        assert_eq!(serde_json::to_value(&typed).expect("serialize"), document);
        let direct: ForecastSpec = serde_json::from_value(document).expect("tagged parameters");
        assert_eq!(direct, typed);
    }
}

#[test]
fn typed_parameters_reject_a_key_outside_the_method() {
    let document = json!({"method": "growth_pct", "params": {"rate": 0.03, "sigma": 1.0}});
    assert!(serde_json::from_value::<ForecastSpec>(document).is_err());
}

#[test]
fn bounds_clamp_deterministic_forecast_output() {
    let periods = quarters(4);
    let mut spec = ForecastSpec::growth(0.10);
    spec = spec.clone().with_param("max", json!(120.0)).unwrap();
    let results = run(&spec, 100.0, &periods).expect("bounded growth");
    // Unbounded: 110, 121, 133.1, 146.41 — everything above 120 pins.
    assert!((results[&periods[0]] - 110.0).abs() < 1e-9);
    assert!((results[&periods[1]] - 120.0).abs() < 1e-9);
    assert!((results[&periods[2]] - 120.0).abs() < 1e-9);
    assert!((results[&periods[3]] - 120.0).abs() < 1e-9);
}

#[test]
fn bounds_clamp_stochastic_forecast_output() {
    let periods = quarters(8);
    let mut spec = ForecastSpec::normal(0.0, 50.0, 42);
    spec = spec.clone().with_param("min", json!(0.0)).unwrap();
    spec = spec.clone().with_param("max", json!(150.0)).unwrap();
    let results = run(&spec, 100.0, &periods).expect("bounded normal");
    for (pid, value) in &results {
        assert!(
            (0.0..=150.0).contains(value),
            "value {value} at {pid:?} escaped the bounds"
        );
    }
}

#[test]
fn bounds_reject_inverted_range() {
    let mut spec = ForecastSpec::forward_fill();
    spec = spec.clone().with_param("min", json!(10.0)).unwrap();
    spec = spec.clone().with_param("max", json!(-10.0)).unwrap();
    let err = spec.validate().expect_err("min > max");
    assert!(err.to_string().contains("above 'max'"), "{err}");
}

#[test]
fn bounds_reject_non_numeric_values() {
    for bad in [json!("high"), serde_json::Value::Null] {
        let spec = ForecastSpec::forward_fill();
        let err = spec.clone().with_param("max", bad).expect_err("bad bound");
        assert!(err.to_string().contains("'max'"), "{err}");
    }
}

#[test]
fn bounds_are_accepted_by_every_method() {
    let specs = [
        ForecastSpec::forward_fill(),
        ForecastSpec::growth(0.01),
        ForecastSpec::curve(vec![0.01]),
        ForecastSpec::normal(0.0, 1.0, 1),
        ForecastSpec::log_normal(0.0, 1.0, 1),
        ForecastSpec::overrides(indexmap::IndexMap::new()),
        ForecastSpec::time_series(vec![1.0, 2.0], ForecastTrendMethod::Linear),
        ForecastSpec::seasonal(vec![1.0; 8], 4, crate::types::SeasonalMode::Additive),
        ForecastSpec::fade_to_target(1.0, ForecastFadeShape::Linear),
        ForecastSpec::mean_reverting(1.0, 0.5, 0.1, 1),
        ForecastSpec::bootstrap(vec![1.0, 2.0], ForecastBootstrapMode::Growth, 1),
    ];
    for mut spec in specs {
        spec = spec.clone().with_param("min", json!(-5.0)).unwrap();
        spec = spec.clone().with_param("max", json!(5.0)).unwrap();
        let parameters = spec.clone();
        assert_eq!(
            parameters.bounds(),
            (Some(-5.0), Some(5.0)),
            "{:?}",
            spec.method()
        );
    }
}

/// Monte Carlo path: `apply_forecast` must return **unclamped**
/// values. The recurrence evolves without bounds; the evaluator clamps
/// only the reported levels after retaining the sampled shocks.
#[test]
fn seeded_mc_forecast_is_not_clamped_before_z_recording() {
    let periods = quarters(8);
    let mut spec = ForecastSpec::normal(0.0, 50.0, 42);
    spec = spec.clone().with_param("min", json!(99.0)).unwrap();
    spec = spec.clone().with_param("max", json!(101.0)).unwrap();
    let parameters = spec.clone();

    let (seeded, _) = apply_forecast(
        &parameters,
        100.0,
        &periods,
        "node",
        Some(stream::PathId {
            run_seed: 0,
            path: 3,
        }),
    )
    .expect("seeded forecast");
    // With sigma = 50 and tight bounds, an unclamped walk must escape
    // [99, 101] somewhere in 8 periods.
    assert!(
        seeded.values().any(|v| !(99.0..=101.0).contains(v)),
        "seeded MC series must be raw (unclamped): {seeded:?}"
    );

    // And the single-run node path with identical params must be clamped.
    let clamped = run(&spec, 100.0, &periods).expect("single-run forecast");
    for value in clamped.values() {
        assert!((99.0..=101.0).contains(value));
    }
}

/// Two nodes configured with the same seed draw different paths.
#[test]
fn node_id_is_mixed_into_the_seed() {
    let periods = quarters(4);
    let parameters = ForecastSpec::normal(0.0, 1.0, 42);
    let a = apply_forecast_for_node(&parameters, 0.0, &periods, "a").expect("a");
    let b = apply_forecast_for_node(&parameters, 0.0, &periods, "b").expect("b");
    assert_ne!(a, b);
}

#[test]
fn a_typed_spec_cannot_hold_a_wrong_type_or_an_unknown_key() {
    let spec = ForecastSpec::growth(0.05);
    spec.validate().expect("well-typed growth spec");
    let err = spec
        .with_param("rate", json!("x"))
        .expect_err("string rate must be rejected");
    assert!(err.to_string().contains("'rate'"), "{err}");

    let err = ForecastSpec::forward_fill()
        .with_param("sigma", json!(1.0))
        .expect_err("unknown key must be rejected");
    assert!(err.to_string().contains("unknown field `sigma`"), "{err}");
}

/// STATEMENTS-CONTRACTS-003 #1: a duplicate, unknown, `null` or
/// method-inapplicable field is rejected when the document is read, and
/// the method choices have no default.
#[test]
fn the_wire_form_rejects_duplicate_unknown_null_and_inapplicable_fields() {
    for (document, cause) in [
        (
            r#"{"method":"growth_pct","params":{"rate":0.1,"rate":0.2}}"#,
            "appears more than once",
        ),
        (
            r#"{"method":"growth_pct","params":{"rate":0.1,"min":null}}"#,
            "'min' is null",
        ),
        (
            r#"{"method":"growth_pct","params":{"rate":0.1},"extra":1}"#,
            "unknown field `extra`",
        ),
        (
            r#"{"method":"growth_pct","params":{"rate":0.1,"seed":"1"}}"#,
            "unknown field `seed`",
        ),
        (
            r#"{"method":"fade_to_target","params":{"target":1.0}}"#,
            "missing field `shape`",
        ),
        (
            r#"{"method":"time_series","params":{"historical":[1.0,2.0]}}"#,
            "missing field `method`",
        ),
        (
            r#"{"method":"bootstrap","params":{"historical":[1.0,2.0],"seed":"1"}}"#,
            "missing field `mode`",
        ),
    ] {
        let error = serde_json::from_str::<ForecastSpec>(document).expect_err(document);
        assert!(error.to_string().contains(cause), "{document}: {error}");
    }
    // A method without required parameters may omit the object.
    let bare: ForecastSpec = serde_json::from_str(r#"{"method":"forward_fill"}"#).expect("bare");
    assert_eq!(bare, ForecastSpec::forward_fill());
    assert_eq!(
        serde_json::to_string(&bare).expect("serialize"),
        r#"{"method":"forward_fill"}"#
    );
    // A linear fade with a half-life reads but fails static validation.
    let fade: ForecastSpec = serde_json::from_str(
        r#"{"method":"fade_to_target","params":{"target":1.0,"shape":"linear","half_life":2.0}}"#,
    )
    .expect("well-typed");
    let error = fade.validate().expect_err("half_life on a linear fade");
    assert!(error.to_string().contains("half_life"), "{error}");
}

/// STATEMENTS-CONTRACTS-003 #4: a seed is an exact `u64`, written as a
/// canonical decimal string so every host carries all 64 bits.
#[test]
fn seeds_are_exact_canonical_decimal_strings() {
    let spec = ForecastSpec::normal(0.0, 1.0, 9_007_199_254_740_993);
    let wire = serde_json::to_value(&spec).expect("serialize");
    assert_eq!(wire["params"]["seed"], json!("9007199254740993"));
    let back: ForecastSpec = serde_json::from_value(wire).expect("deserialize");
    assert_eq!(back, spec);

    for bad in [
        json!(42),
        json!(42.0),
        json!("042"),
        json!("+42"),
        json!("-1"),
        json!("1.5"),
        json!(""),
        json!("18446744073709551616"),
    ] {
        assert!(
            spec.clone().with_param("seed", bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(
        spec.with_param("seed", json!("18446744073709551615"))
            .is_ok()
    );
    // Only a stochastic method takes a seed.
    assert!(
        ForecastSpec::growth(0.05)
            .with_param("seed", json!("1"))
            .is_err()
    );
}

#[test]
fn convenience_ctors_produce_valid_specs() {
    let mut overrides = indexmap::IndexMap::new();
    overrides.insert(
        PeriodId::quarter(2025, 3).expect("valid period fixture"),
        120.0,
    );
    ForecastSpec::overrides(overrides)
        .validate()
        .expect("override spec");
    ForecastSpec::seasonal(vec![1.0; 8], 4, crate::types::SeasonalMode::Additive)
        .validate()
        .expect("seasonal spec");
    ForecastSpec::time_series(vec![1.0, 2.0, 3.0], ForecastTrendMethod::Linear)
        .validate()
        .expect("time-series spec");
}

#[test]
fn unknown_keys_and_variants_rejected_for_fade_to_target() {
    let spec = ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Linear);
    let err = spec
        .with_param("shpae", json!("linear"))
        .expect_err("typo key");
    assert!(err.to_string().contains("shpae"), "{err}");

    let spec = ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Linear);
    let err = spec
        .with_param("shape", json!("sigmoid"))
        .expect_err("unknown shape");
    assert!(err.to_string().contains("sigmoid"), "{err}");

    assert!(
        ForecastSpec::from_params(
            crate::types::ForecastMethod::FadeToTarget,
            indexmap::IndexMap::new(),
        )
        .is_err(),
        "target is required"
    );
}
