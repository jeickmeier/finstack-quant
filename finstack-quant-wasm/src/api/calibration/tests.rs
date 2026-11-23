use super::execute::*;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationSchema,
};
use finstack_quant_calibration::api::validate;

fn empty_envelope_json() -> String {
    let plan = CalibrationPlan {
        id: "empty".to_string(),
        description: None,
        quote_sets: Default::default(),
        steps: Vec::new(),
        settings: Default::default(),
    };
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: CalibrationSchema::CURRENT,
        plan,
        market_data: Vec::new(),
        prior_market: Vec::new(),
    };
    serde_json::to_string(&envelope).expect("serialize")
}

#[test]
fn validated_envelope_accepts_empty_plan() {
    let json = empty_envelope_json();
    let parsed = validate::parse_envelope(&json).expect("parse");
    validate::validate_calibration(&parsed).expect("validate");
    assert_eq!(parsed.plan.id, "empty");
}

#[test]
fn validation_and_execution_reject_semantically_invalid_requests() {
    let mut value: serde_json::Value =
        serde_json::from_str(&empty_envelope_json()).expect("empty envelope parses");
    value["plan"]["steps"] = serde_json::json!([{
        "id": "discount_step",
        "quote_set": "missing_quotes",
        "kind": "discount",
        "curve_id": "USD-OIS",
        "currency": "USD",
        "base_date": "2026-05-08"
    }]);
    let json = value.to_string();

    for error in [
        ExecuteError::from(
            validate::validate_calibration(&validate::parse_envelope(&json).expect("parse"))
                .expect_err("validation must reject undefined quote sets"),
        ),
        engine::calibrate_from_json(&json).expect_err("execution must reject undefined quote sets"),
    ] {
        let details = error.details();
        assert_eq!(details.stage.as_str(), "ingestion");
        assert_eq!(details.category, "undefined_quote_set");
        assert!(details.cause.contains("missing_quotes"));
    }
}

#[test]
fn calibrate_empty_plan_succeeds() {
    let json = empty_envelope_json();
    let result = engine::calibrate_from_json(&json).expect("execute");
    let parsed = serde_json::to_value(&result).expect("json");
    assert!(parsed.is_object());
    assert!(parsed.get("result").is_some());
}

#[test]
fn validate_calibration_returns_the_typed_envelope() {
    let envelope = validate::parse_envelope(&empty_envelope_json()).expect("parse");
    validate::validate_calibration(&envelope).expect("validate");
    assert_eq!(envelope.plan.id, "empty");
    assert!(validate::parse_envelope("{ not json").is_err());
}

#[test]
fn dry_run_accepts_empty_plan() {
    let json = empty_envelope_json();
    let report = validate::dry_run(&json).expect("dry_run");
    assert!(report.errors.is_empty());
    let report_json = report.to_json_pretty().expect("serialize report");
    let parsed: serde_json::Value = serde_json::from_str(&report_json).expect("json");
    assert!(parsed.get("errors").is_some());
    assert!(parsed.get("dependency_graph").is_some());
}

#[test]
fn dry_run_rejects_malformed_json() {
    assert!(validate::dry_run("not json").is_err());
}

#[test]
fn strict_parse_preserves_diagnostic() {
    let error = ExecuteError::from(
        validate::parse_envelope("{ not valid json").expect_err("malformed JSON must error"),
    );
    let details = error.details();
    assert_eq!(details.stage.as_str(), "ingestion");
    assert_eq!(details.category, "strict_load");
    assert!(!details.cause.is_empty());
    assert!(
        error
            .details()
            .to_json()
            .contains("\"stage\":\"ingestion\"")
    );
}

#[test]
fn validated_envelope_preserves_schema() {
    let json = empty_envelope_json();
    let parsed = validate::parse_envelope(&json).expect("parse");
    validate::validate_calibration(&parsed).expect("validate");
    let canonical = parsed.to_json_pretty().expect("serialize envelope");
    assert_ne!(
        canonical, "{}",
        "canonical envelope JSON must not collapse to an empty object"
    );
    let (reparsed, _report) = CalibrationEnvelope::from_slice_strict(
        canonical.as_bytes(),
        &finstack_quant_core::contract::LoadLimits::default(),
    )
    .expect("canonical JSON must round-trip strictly");
    assert_eq!(reparsed.schema, CalibrationSchema::Calibration);
}

#[test]
fn content_hashes_match_the_rust_typed_hashes() {
    let json = empty_envelope_json();
    let (envelope, _report) = CalibrationEnvelope::from_slice_strict(
        json.as_bytes(),
        &finstack_quant_core::contract::LoadLimits::default(),
    )
    .expect("envelope");
    assert_eq!(
        validate::parse_envelope(&json)
            .expect("parse")
            .content_hash()
            .expect("hash"),
        envelope.content_hash().expect("typed hash"),
    );
    assert!(validate::parse_envelope("{ not json").is_err());
    assert!(load_result("{ not json").is_err());
}

#[test]
fn calibrate_preserves_malformed_envelope_diagnostic() {
    let error = engine::calibrate_from_json("{ this is not valid json")
        .expect_err("malformed envelope must error");
    let details = error.details();
    let json = error.details().to_json();
    assert_eq!(details.stage.as_str(), "ingestion");
    assert_eq!(details.category, "strict_load");
    assert!(
        json.contains("strict_load") && json.contains("cause"),
        "diagnostic JSON should carry the structured load error, got: {json}"
    );
    assert!(
        !details.cause.is_empty(),
        "diagnostic cause must not be empty"
    );
}
