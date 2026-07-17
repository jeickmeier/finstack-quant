//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::checks::{
    CheckCategory, CheckFinding, CheckReport, CheckResult, CheckSummary, Materiality, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements_analytics::analysis::checks::CheckReportRenderer;

fn sample_report() -> CheckReport {
    let error_result = CheckResult {
        check_id: "balance_sheet_articulation".into(),
        check_name: "Balance Sheet Articulation".into(),
        category: CheckCategory::AccountingIdentity,
        passed: false,
        findings: vec![CheckFinding {
            check_id: "balance_sheet_articulation".into(),
            severity: Severity::Error,
            message: "Balance sheet does not articulate: imbalance = 50.00".into(),
            period: Some(PeriodId::quarter(2025, 1)),
            materiality: Some(Materiality {
                absolute: 50.0,
                relative_pct: 2.5,
                reference_value: 2000.0,
                reference_label: "total_assets".into(),
            }),
            nodes: vec![
                NodeId::new("total_assets"),
                NodeId::new("total_liabilities"),
            ],
        }],
    };

    let warning_result = CheckResult {
        check_id: "leverage_range".into(),
        check_name: "Leverage Range".into(),
        category: CheckCategory::CreditReasonableness,
        passed: true,
        findings: vec![CheckFinding {
            check_id: "leverage_range".into(),
            severity: Severity::Warning,
            message: "Debt/EBITDA 7.00x in 2025Q2 outside Warning range".into(),
            period: Some(PeriodId::quarter(2025, 2)),
            materiality: None,
            nodes: vec![NodeId::new("debt"), NodeId::new("ebitda")],
        }],
    };

    let passing_result = CheckResult {
        check_id: "non_finite".into(),
        check_name: "Non-Finite Value".into(),
        category: CheckCategory::DataQuality,
        passed: true,
        findings: vec![],
    };

    CheckReport {
        results: vec![error_result, warning_result, passing_result],
        summary: CheckSummary {
            total_checks: 3,
            passed: 2,
            failed: 1,
            errors: 1,
            warnings: 1,
            infos: 0,
        },
    }
}

// ============================================================================
// Text renderer
// ============================================================================

#[test]
fn render_text_has_header_and_summary() {
    let text = CheckReportRenderer::render_text(&sample_report());
    assert!(text.contains("Check Report"));
    assert!(text.contains("3 checks"));
    assert!(text.contains("2 passed"));
    assert!(text.contains("1 failed"));
}

#[test]
fn render_text_has_error_section() {
    let text = CheckReportRenderer::render_text(&sample_report());
    assert!(text.contains("ERRORS"));
    assert!(text.contains("[ERROR]"));
    assert!(text.contains("balance_sheet_articulation"));
    assert!(text.contains("Materiality: 50.00"));
}

#[test]
fn render_text_has_warning_section() {
    let text = CheckReportRenderer::render_text(&sample_report());
    assert!(text.contains("WARNINGS"));
    assert!(text.contains("[WARN]"));
    assert!(text.contains("leverage_range"));
}

#[test]
fn render_text_omits_empty_info_section() {
    let text = CheckReportRenderer::render_text(&sample_report());
    assert!(!text.contains("── INFO ──"));
}

#[test]
fn render_text_shows_nodes() {
    let text = CheckReportRenderer::render_text(&sample_report());
    assert!(text.contains("total_assets"));
    assert!(text.contains("total_liabilities"));
}

// ============================================================================
// HTML renderer
// ============================================================================

#[test]
fn render_html_produces_div() {
    let html = CheckReportRenderer::render_html(&sample_report());
    assert!(html.starts_with("<div"));
    assert!(html.contains("</div>"));
}

#[test]
fn render_html_has_summary() {
    let html = CheckReportRenderer::render_html(&sample_report());
    assert!(html.contains("3"));
    assert!(html.contains("2 passed"));
    assert!(html.contains("1 failed"));
}

#[test]
fn render_html_has_error_section() {
    let html = CheckReportRenderer::render_html(&sample_report());
    assert!(html.contains("Errors"));
    assert!(html.contains("balance_sheet_articulation"));
}
