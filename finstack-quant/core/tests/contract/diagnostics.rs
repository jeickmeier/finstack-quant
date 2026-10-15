use finstack_quant_core::contract::{
    ContractError, Diagnostic, LoadLimits, LoadPhase, Severity, ValidationReport,
};
#[cfg(feature = "json-schema")]
use schemars::schema_for;
use serde_json::json;

fn diagnostic(code: &str, severity: Severity) -> Diagnostic {
    Diagnostic::new(code, LoadPhase::Semantic, severity, "position is invalid")
        .with_pointer("/positions/0")
        .with_contract("finstack_quant.portfolio")
        .with_expected_version(1)
        .with_actual_version(2)
        .with_artifact_hash("sha256:abc")
        .with_revision_id("rev-1")
        .with_instrument_id("instrument-1")
        .with_position_id("position-1")
}

fn json_roundtrip(report: &ValidationReport) -> ValidationReport {
    serde_json::from_value(serde_json::to_value(report).unwrap()).unwrap()
}

#[test]
fn diagnostics_have_strict_stable_serde_names() {
    let value = serde_json::to_value(diagnostic(
        "contract/version-unsupported",
        Severity::Warning,
    ))
    .unwrap();

    assert_eq!(value["phase"], "semantic");
    assert_eq!(value["severity"], "warning");
    assert_eq!(value["pointer"], "/positions/0");
    assert_eq!(value["contract"], "finstack_quant.portfolio");
    assert_eq!(value["expected_version"], 1);
    assert_eq!(value["actual_version"], 2);
    assert_eq!(value["artifact_hash"], "sha256:abc");
    assert_eq!(value["revision_id"], "rev-1");
    assert_eq!(value["instrument_id"], "instrument-1");
    assert_eq!(value["position_id"], "position-1");

    let mut with_unknown = value;
    with_unknown
        .as_object_mut()
        .unwrap()
        .insert("unexpected".to_string(), json!(true));
    assert!(serde_json::from_value::<Diagnostic>(with_unknown).is_err());
}

#[test]
fn diagnostic_types_publish_json_schemas() {
    let diagnostic_schema = serde_json::to_value(schema_for!(Diagnostic)).unwrap();
    let report_schema = serde_json::to_value(schema_for!(ValidationReport)).unwrap();

    assert_eq!(
        diagnostic_schema["title"],
        serde_json::Value::String("Diagnostic".to_string())
    );
    assert_eq!(
        report_schema["title"],
        serde_json::Value::String("ValidationReport".to_string())
    );
    assert!(report_schema["properties"]["discarded_errors"].is_null());
}

#[test]
fn validation_report_wire_fields_remain_unchanged() {
    let limits = LoadLimits::default().with_max_diagnostics(0);
    let mut report = ValidationReport::default();
    report.push_bounded(&limits, diagnostic("error/one", Severity::Error));

    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!({"diagnostics": [], "truncated": true})
    );
}

#[test]
fn parse_error_diagnostic_preserves_location_and_pointer() {
    let error = serde_json::from_str::<serde_json::Value>("{\n  \"value\":\n}").unwrap_err();
    let diagnostic = Diagnostic::from_parse_error(&error, Some("/value".to_string()));

    assert_eq!(diagnostic.code, "contract/parse-error");
    assert_eq!(diagnostic.phase, LoadPhase::Parse);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.pointer.as_deref(), Some("/value"));
    assert!(diagnostic.message.contains(&format!(
        "line {}, column {}",
        error.line(),
        error.column()
    )));
}

#[test]
fn validation_report_detects_errors_and_bounds_diagnostics() {
    let limits = LoadLimits::default().with_max_diagnostics(2);
    let mut report = ValidationReport::default();

    report.push_bounded(&limits, diagnostic("warning/one", Severity::Warning));
    assert!(!report.has_errors());
    assert!(!report.truncated);

    report.push_bounded(&limits, diagnostic("error/two", Severity::Error));
    assert!(report.has_errors());
    assert!(!report.truncated);

    report.push_bounded(&limits, diagnostic("error/three", Severity::Error));
    assert_eq!(report.diagnostics.len(), 2);
    assert!(report.truncated);
    assert_eq!(
        ContractError::Report(Box::new(report)).to_string(),
        "validation failed with 1 error(s)"
    );
}

#[test]
fn warning_filled_limit_retains_error_across_json_roundtrip() {
    let limits = LoadLimits::default().with_max_diagnostics(1);
    let mut report = ValidationReport::default();

    report.push_bounded(&limits, diagnostic("warning/one", Severity::Warning));
    report.push_bounded(&limits, diagnostic("error/two", Severity::Error));

    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].severity, Severity::Error);
    assert!(report.truncated);
    let restored = json_roundtrip(&report);
    assert!(restored.has_errors());
    assert_eq!(
        ContractError::Report(Box::new(restored)).to_string(),
        "validation failed with 1 error(s)"
    );
}

#[test]
fn zero_diagnostic_limit_fails_closed_across_json_roundtrip() {
    let limits = LoadLimits::default().with_max_diagnostics(0);
    let mut report = ValidationReport::default();

    report.push_bounded(&limits, diagnostic("error/one", Severity::Error));

    assert!(report.diagnostics.is_empty());
    assert!(report.truncated);
    let restored = json_roundtrip(&report);
    assert!(restored.has_errors());
    assert_eq!(
        ContractError::Report(Box::new(restored)).to_string(),
        "validation failed with 1 error(s)"
    );
}

#[test]
fn zero_diagnostic_limit_warning_only_report_also_fails_closed() {
    let limits = LoadLimits::default().with_max_diagnostics(0);
    let mut report = ValidationReport::default();

    report.push_bounded(&limits, diagnostic("warning/one", Severity::Warning));

    let restored = json_roundtrip(&report);
    assert!(restored.diagnostics.is_empty());
    assert!(restored.truncated);
    assert!(restored.has_errors());
    assert_eq!(
        ContractError::Report(Box::new(restored)).to_string(),
        "validation failed with 1 error(s)"
    );
}

#[test]
fn positive_limit_warning_only_report_remains_non_fatal() {
    let limits = LoadLimits::default().with_max_diagnostics(1);
    let mut report = ValidationReport::default();

    report.push_bounded(&limits, diagnostic("warning/one", Severity::Warning));
    report.push_bounded(&limits, diagnostic("warning/two", Severity::Warning));

    let restored = json_roundtrip(&report);
    assert_eq!(restored.diagnostics.len(), 1);
    assert!(restored.truncated);
    assert!(!restored.has_errors());
}

#[test]
fn report_contract_error_counts_only_error_diagnostics() {
    let limits = LoadLimits::default().with_max_diagnostics(3);
    let mut report = ValidationReport::default();
    report.push_bounded(&limits, diagnostic("warning/one", Severity::Warning));
    report.push_bounded(&limits, diagnostic("error/one", Severity::Error));
    report.push_bounded(&limits, diagnostic("error/two", Severity::Error));

    assert_eq!(
        ContractError::Report(Box::new(report)).to_string(),
        "validation failed with 2 error(s)"
    );
}
