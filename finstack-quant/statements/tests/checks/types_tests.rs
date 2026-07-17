//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::checks::{
    CheckCategory, CheckConfig, CheckFinding, CheckReport, CheckResult, CheckSummary, Materiality,
    PeriodScope, Severity,
};
use finstack_quant_statements::types::NodeId;

// ---------------------------------------------------------------------------
// Severity ordering
// ---------------------------------------------------------------------------

#[test]
fn severity_ordering_info_lt_warning_lt_error() {
    assert!(Severity::Info < Severity::Warning);
    assert!(Severity::Warning < Severity::Error);
    assert!(Severity::Info < Severity::Error);
}

// ---------------------------------------------------------------------------
// Serde round-trips
// ---------------------------------------------------------------------------

#[test]
fn severity_serde_roundtrip() {
    for severity in [Severity::Info, Severity::Warning, Severity::Error] {
        let json = serde_json::to_string(&severity).unwrap();
        let back: Severity = serde_json::from_str(&json).unwrap();
        assert_eq!(severity, back);
    }
}

#[test]
fn severity_serde_snake_case() {
    assert_eq!(serde_json::to_string(&Severity::Info).unwrap(), "\"info\"");
    assert_eq!(
        serde_json::to_string(&Severity::Warning).unwrap(),
        "\"warning\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::Error).unwrap(),
        "\"error\""
    );
}

#[test]
fn check_category_serde_roundtrip() {
    let categories = [
        CheckCategory::AccountingIdentity,
        CheckCategory::CrossStatementReconciliation,
        CheckCategory::InternalConsistency,
        CheckCategory::CreditReasonableness,
        CheckCategory::DataQuality,
    ];
    for cat in categories {
        let json = serde_json::to_string(&cat).unwrap();
        let back: CheckCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, back);
    }
}

#[test]
fn check_category_serde_snake_case() {
    assert_eq!(
        serde_json::to_string(&CheckCategory::AccountingIdentity).unwrap(),
        "\"accounting_identity\""
    );
    assert_eq!(
        serde_json::to_string(&CheckCategory::CrossStatementReconciliation).unwrap(),
        "\"cross_statement_reconciliation\""
    );
    assert_eq!(
        serde_json::to_string(&CheckCategory::InternalConsistency).unwrap(),
        "\"internal_consistency\""
    );
    assert_eq!(
        serde_json::to_string(&CheckCategory::CreditReasonableness).unwrap(),
        "\"credit_reasonableness\""
    );
    assert_eq!(
        serde_json::to_string(&CheckCategory::DataQuality).unwrap(),
        "\"data_quality\""
    );
}

#[test]
fn period_scope_serde_roundtrip() {
    for scope in [
        PeriodScope::AllPeriods,
        PeriodScope::ActualsOnly,
        PeriodScope::ForecastOnly,
    ] {
        let json = serde_json::to_string(&scope).unwrap();
        let back: PeriodScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, back);
    }
}

#[test]
fn materiality_serde_roundtrip() {
    let m = Materiality {
        absolute: 500.0,
        relative_pct: 2.5,
        reference_value: 20_000.0,
        reference_label: "total_assets".into(),
    };
    let json = serde_json::to_string(&m).unwrap();
    let back: Materiality = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
}

// ---------------------------------------------------------------------------
// CheckConfig defaults
// ---------------------------------------------------------------------------

#[test]
fn check_config_defaults() {
    let cfg = CheckConfig::default();
    assert!((cfg.default_tolerance - 0.01).abs() < f64::EPSILON);
    assert!((cfg.materiality_threshold - 0.0).abs() < f64::EPSILON);
    assert_eq!(cfg.min_severity, Severity::Info);
}

#[test]
fn test_check_config_partial_json() {
    let config: CheckConfig = serde_json::from_str("{}").unwrap();
    assert!((config.default_tolerance - 0.01).abs() < f64::EPSILON);
    assert!((config.materiality_threshold - 0.0).abs() < f64::EPSILON);
    assert_eq!(config.min_severity, Severity::Info);
}

// ---------------------------------------------------------------------------
// CheckFinding serde
// ---------------------------------------------------------------------------

#[test]
fn check_finding_serde_roundtrip() {
    let finding = CheckFinding {
        check_id: "bs_identity".into(),
        severity: Severity::Error,
        message: "Assets != Liabilities + Equity".into(),
        period: Some(PeriodId::quarter(2025, 1)),
        materiality: Some(Materiality {
            absolute: 100.0,
            relative_pct: 0.5,
            reference_value: 20_000.0,
            reference_label: "total_assets".into(),
        }),
        nodes: vec![
            NodeId::new("total_assets"),
            NodeId::new("total_liabilities"),
        ],
    };
    let json = serde_json::to_string(&finding).unwrap();
    let back: CheckFinding = serde_json::from_str(&json).unwrap();
    assert_eq!(finding, back);
}

#[test]
fn check_finding_optional_fields_omitted() {
    let finding = CheckFinding {
        check_id: "completeness".into(),
        severity: Severity::Info,
        message: "All good".into(),
        period: None,
        materiality: None,
        nodes: vec![],
    };
    let json = serde_json::to_string(&finding).unwrap();
    assert!(!json.contains("period"));
    assert!(!json.contains("materiality"));
    assert!(!json.contains("nodes"));
}

// ---------------------------------------------------------------------------
// CheckResult.passed logic
// ---------------------------------------------------------------------------

#[test]
fn check_result_passed_true_when_no_findings() {
    let result = CheckResult {
        check_id: "test".into(),
        check_name: "Test Check".into(),
        category: CheckCategory::AccountingIdentity,
        passed: true,
        findings: vec![],
    };
    assert!(result.passed);
}

#[test]
fn check_result_passed_false_with_error_findings() {
    let result = CheckResult {
        check_id: "test".into(),
        check_name: "Test Check".into(),
        category: CheckCategory::AccountingIdentity,
        passed: false,
        findings: vec![CheckFinding {
            check_id: "test".into(),
            severity: Severity::Error,
            message: "Mismatch".into(),
            period: None,
            materiality: None,
            nodes: vec![],
        }],
    };
    assert!(!result.passed);
}

// ---------------------------------------------------------------------------
// CheckSummary
// ---------------------------------------------------------------------------

#[test]
fn check_summary_fields() {
    let summary = CheckSummary {
        total_checks: 5,
        passed: 3,
        failed: 2,
        errors: 2,
        warnings: 1,
        infos: 4,
    };
    assert_eq!(summary.total_checks, 5);
    assert_eq!(summary.passed, 3);
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.errors, 2);
    assert_eq!(summary.warnings, 1);
    assert_eq!(summary.infos, 4);
}

// ---------------------------------------------------------------------------
// CheckReport query methods
// ---------------------------------------------------------------------------

fn sample_report() -> CheckReport {
    let error_finding = CheckFinding {
        check_id: "bs".into(),
        severity: Severity::Error,
        message: "Balance mismatch".into(),
        period: Some(PeriodId::quarter(2025, 1)),
        materiality: Some(Materiality {
            absolute: 1000.0,
            relative_pct: 5.0,
            reference_value: 20_000.0,
            reference_label: "total_assets".into(),
        }),
        nodes: vec![NodeId::new("total_assets")],
    };
    let warning_finding = CheckFinding {
        check_id: "ratio".into(),
        severity: Severity::Warning,
        message: "High leverage".into(),
        period: Some(PeriodId::quarter(2025, 2)),
        materiality: Some(Materiality {
            absolute: 50.0,
            relative_pct: 0.25,
            reference_value: 20_000.0,
            reference_label: "total_debt".into(),
        }),
        nodes: vec![NodeId::new("debt_ratio")],
    };
    let info_finding = CheckFinding {
        check_id: "completeness".into(),
        severity: Severity::Info,
        message: "Optional node missing".into(),
        period: None,
        materiality: None,
        nodes: vec![],
    };

    CheckReport {
        results: vec![
            CheckResult {
                check_id: "bs".into(),
                check_name: "Balance Sheet Identity".into(),
                category: CheckCategory::AccountingIdentity,
                passed: false,
                findings: vec![error_finding],
            },
            CheckResult {
                check_id: "ratio".into(),
                check_name: "Leverage Ratio".into(),
                category: CheckCategory::CreditReasonableness,
                passed: true,
                findings: vec![warning_finding],
            },
            CheckResult {
                check_id: "completeness".into(),
                check_name: "Node Coverage".into(),
                category: CheckCategory::DataQuality,
                passed: true,
                findings: vec![info_finding],
            },
        ],
        summary: CheckSummary {
            total_checks: 3,
            passed: 2,
            failed: 1,
            errors: 1,
            warnings: 1,
            infos: 1,
        },
    }
}

#[test]
fn findings_by_severity() {
    let report = sample_report();
    assert_eq!(report.findings_by_severity(Severity::Error).len(), 1);
    assert_eq!(report.findings_by_severity(Severity::Warning).len(), 1);
    assert_eq!(report.findings_by_severity(Severity::Info).len(), 1);
}

#[test]
fn has_errors_and_warnings() {
    let report = sample_report();
    assert!(report.has_errors());
    assert!(report.has_warnings());
}

#[test]
fn has_errors_false_when_no_errors() {
    let report = CheckReport {
        results: vec![],
        summary: CheckSummary {
            total_checks: 0,
            passed: 0,
            failed: 0,
            errors: 0,
            warnings: 0,
            infos: 0,
        },
    };
    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}
