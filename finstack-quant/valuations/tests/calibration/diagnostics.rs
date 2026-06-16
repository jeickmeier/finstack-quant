//! Tests for `EnvelopeError` and the static envelope validator.

use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::errors::EnvelopeError;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, StepParams,
    CALIBRATION_SCHEMA,
};
use finstack_quant_valuations::calibration::api::validate::{
    dependency_graph_json, dry_run, validate,
};

fn empty_envelope(id: &str) -> CalibrationEnvelope {
    CalibrationEnvelope {
        schema_url: None,
        schema: CALIBRATION_SCHEMA.to_string(),
        plan: CalibrationPlan {
            id: id.to_string(),
            description: None,
            quote_sets: HashMap::default(),
            steps: Vec::new(),
            settings: Default::default(),
        },
        market_data: Vec::new(),
        prior_market: Vec::new(),
    }
}

fn discount_step(id: &str, quote_set: &str, curve_id: &str) -> CalibrationStep {
    let params: DiscountCurveParams = serde_json::from_value(serde_json::json!({
        "curve_id": curve_id,
        "currency": "USD",
        "base_date": "2026-05-08",
    }))
    .expect("default discount params");
    CalibrationStep {
        id: id.to_string(),
        quote_set: quote_set.to_string(),
        params: StepParams::Discount(params),
    }
}

#[test]
fn envelope_error_display_includes_step_id() {
    let err = EnvelopeError::MissingDependency {
        step_index: 2,
        step_id: "cdx_hazard".to_string(),
        step_kind: "hazard".to_string(),
        missing_id: "USD-OIS".to_string(),
        missing_kind: "discount".to_string(),
        available: vec!["EUR-OIS".to_string()],
    };
    let s = format!("{err}");
    assert!(s.contains("step[2]"), "missing step index: {s}");
    assert!(s.contains("cdx_hazard"), "missing step id: {s}");
    assert!(s.contains("USD-OIS"), "missing the missing_id: {s}");
    assert!(s.contains("EUR-OIS"), "missing available list: {s}");
}

#[test]
fn envelope_error_serializes_with_kind_tag() {
    let err = EnvelopeError::UndefinedQuoteSet {
        step_index: 1,
        step_id: "test_step".to_string(),
        ref_name: "missing_set".to_string(),
        available: vec!["set_a".to_string(), "set_b".to_string()],
        suggestion: Some("set_a".to_string()),
    };
    let json = err.to_json();
    assert!(json.contains("\"kind\": \"undefined_quote_set\""));
    assert!(json.contains("\"ref_name\": \"missing_set\""));
    assert!(json.contains("\"suggestion\": \"set_a\""));
}

#[test]
fn solver_not_converged_includes_worst_quote() {
    let err = EnvelopeError::SolverNotConverged {
        step_id: "discount_step".to_string(),
        max_residual: 1.27e-3,
        tolerance: 1.0e-6,
        iterations: 50,
        worst_quote_id: Some("USD-IRS-30Y".to_string()),
        worst_quote_residual: Some(1.27e-3),
    };
    let s = format!("{err}");
    assert!(s.contains("USD-IRS-30Y"));
    assert!(s.contains("did not converge"));
}

#[test]
fn envelope_error_kind_str_matches_serialized_tag() {
    let err = EnvelopeError::QuoteDataInvalid {
        step_id: "discount".to_string(),
        quote_id: "USD-IRS-2Y".to_string(),
        reason: "rate is NaN".to_string(),
    };
    assert_eq!(err.kind_str(), "quote_data_invalid");
    assert!(err.to_json().contains("\"kind\": \"quote_data_invalid\""));
}

#[test]
fn envelope_error_step_id_returns_some_for_step_bound_variants() {
    let err = EnvelopeError::SolverNotConverged {
        step_id: "discount_step".to_string(),
        max_residual: 1.0,
        tolerance: 1e-6,
        iterations: 1,
        worst_quote_id: None,
        worst_quote_residual: None,
    };
    assert_eq!(err.step_id(), Some("discount_step"));

    let parse = EnvelopeError::JsonParse {
        message: "x".to_string(),
        line: None,
        col: None,
    };
    assert!(parse.step_id().is_none());
}

#[test]
fn validate_empty_envelope_has_no_errors() {
    let env = empty_envelope("empty");
    let report = validate(&env);
    assert!(report.errors.is_empty());
    assert!(report.dependency_graph.nodes.is_empty());
}

#[test]
fn validate_step_with_undefined_quote_set_errors() {
    let mut env = empty_envelope("test");
    env.plan
        .steps
        .push(discount_step("d", "nonexistent_set", "USD-OIS"));
    let report = validate(&env);
    assert!(report.errors.iter().any(|e| matches!(
        e,
        EnvelopeError::UndefinedQuoteSet { ref_name, .. } if ref_name == "nonexistent_set"
    )));
}

#[test]
fn dry_run_returns_json_for_minimal_envelope() {
    let env = empty_envelope("smoke");
    let json = serde_json::to_string(&env).expect("serialize");
    let report_json = dry_run(&json).expect("dry_run succeeds");
    assert!(report_json.contains("\"errors\""));
    assert!(report_json.contains("\"dependency_graph\""));
}

#[test]
fn dependency_graph_json_for_empty_plan_is_well_formed() {
    let env = empty_envelope("smoke");
    let json = serde_json::to_string(&env).expect("serialize");
    let graph_json = dependency_graph_json(&json).expect("dep graph succeeds");
    assert!(graph_json.contains("\"initial_ids\""));
    assert!(graph_json.contains("\"nodes\""));
}

#[test]
fn calibration_report_populates_worst_quote_from_residuals() {
    use finstack_quant_valuations::calibration::CalibrationReport;
    use std::collections::BTreeMap;

    let mut residuals = BTreeMap::new();
    residuals.insert("USD-IRS-2Y".to_string(), 1.0e-10);
    residuals.insert("USD-IRS-30Y".to_string(), 1.27e-3);
    residuals.insert("USD-IRS-5Y".to_string(), -3.0e-7);

    let report = CalibrationReport::new(residuals, 50, false, "did not converge");
    assert_eq!(report.worst_quote_id.as_deref(), Some("USD-IRS-30Y"));
    assert!(report.worst_quote_residual.is_some());
    let r = report.worst_quote_residual.expect("residual");
    assert!((r - 1.27e-3).abs() < 1e-15);
}

#[test]
fn worst_quote_prefers_penalty_over_finite_residuals() {
    // A solver that drove one quote to a penalty sentinel (NaN / INFINITY)
    // should surface *that* quote as worst, not the next-largest finite
    // residual. Otherwise diagnostic messages would point at a quote that's
    // probably fine and hide the actually broken one.
    use finstack_quant_valuations::calibration::CalibrationReport;
    use std::collections::BTreeMap;

    let mut residuals = BTreeMap::new();
    residuals.insert("USD-IRS-2Y".to_string(), 1.0e-10);
    // Large finite residual — would win without penalty handling.
    residuals.insert("USD-IRS-30Y".to_string(), 1.27e-3);
    // Solver flagged this one as failed.
    residuals.insert("USD-IRS-FAILED".to_string(), f64::NAN);

    let report = CalibrationReport::new(residuals, 50, false, "did not converge");
    assert_eq!(report.worst_quote_id.as_deref(), Some("USD-IRS-FAILED"));
}

#[test]
fn envelope_error_propagates_into_finstack_quant_core_error() {
    let err = EnvelopeError::UndefinedQuoteSet {
        step_index: 0,
        step_id: "s".to_string(),
        ref_name: "missing".to_string(),
        available: Vec::new(),
        suggestion: None,
    };
    let core_err: finstack_quant_core::Error = err.into();
    match core_err {
        finstack_quant_core::Error::Calibration { category, .. } => {
            assert_eq!(category, "undefined_quote_set");
        }
        other => panic!("expected Calibration error, got {other:?}"),
    }
}
