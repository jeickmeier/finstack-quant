#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::Severity;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    credit_underwriting_checks, three_statement_checks, CheckReportRenderer, CreditMapping,
    ThreeStatementMapping,
};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

fn ts_mapping() -> ThreeStatementMapping {
    ThreeStatementMapping {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        cash_node: NodeId::new("cash"),
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        ppe_node: None,
        depreciation_node: None,
        interest_expense_node: Some(NodeId::new("interest_expense")),
        tax_expense_node: None,
        pretax_income_node: None,
        cfo_node: None,
        cfi_node: None,
        cff_node: None,
        total_cf_node: Some(NodeId::new("total_cf")),
        capex_node: None,
        dividends_node: None,
    }
}

fn credit_mapping() -> CreditMapping {
    CreditMapping {
        debt_node: NodeId::new("total_debt"),
        ebitda_node: NodeId::new("ebitda"),
        interest_expense_node: NodeId::new("interest_expense"),
        fcf_node: None,
        cash_node: None,
        cash_burn_node: None,
        leverage_warn: None,
        coverage_min_warn: None,
    }
}

// ============================================================================
// Imbalanced model — intentional BS error + high leverage warning
// ============================================================================

#[test]
fn integration_imbalanced_model_catches_errors_and_warnings() {
    let model = ModelBuilder::new("imbalanced_3stmt")
        .periods("2025Q1..Q4", None)
        .unwrap()
        // Balance sheet
        .value(
            "total_assets",
            &[
                (q(1), s(1000.0)),
                (q(2), s(1050.0)),
                (q(3), s(1100.0)),
                (q(4), s(1150.0)),
            ],
        )
        .value(
            "total_liabilities",
            &[
                (q(1), s(600.0)),
                (q(2), s(630.0)),
                (q(3), s(660.0)),
                (q(4), s(690.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (q(1), s(400.0)),
                (q(2), s(420.0)),
                (q(3), s(440.0)),
                (q(4), s(455.0)), // intentionally wrong — should be 460
            ],
        )
        .value(
            "cash",
            &[
                (q(1), s(100.0)),
                (q(2), s(120.0)),
                (q(3), s(135.0)),
                (q(4), s(150.0)),
            ],
        )
        .value(
            "retained_earnings",
            &[
                (q(1), s(200.0)),
                (q(2), s(220.0)),
                (q(3), s(240.0)),
                (q(4), s(255.0)),
            ],
        )
        // Income statement
        .value(
            "revenue",
            &[
                (q(1), s(500.0)),
                (q(2), s(525.0)),
                (q(3), s(550.0)),
                (q(4), s(575.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), s(20.0)),
                (q(2), s(20.0)),
                (q(3), s(20.0)),
                (q(4), s(15.0)),
            ],
        )
        .value(
            "ebitda",
            &[
                (q(1), s(80.0)),
                (q(2), s(84.0)),
                (q(3), s(88.0)),
                (q(4), s(65.0)), // deliberately low for leverage warning
            ],
        )
        .value(
            "interest_expense",
            &[
                (q(1), s(30.0)),
                (q(2), s(30.0)),
                (q(3), s(30.0)),
                (q(4), s(30.0)),
            ],
        )
        // Cash flow
        .value(
            "total_cf",
            &[
                (q(1), s(20.0)),
                (q(2), s(20.0)),
                (q(3), s(15.0)),
                (q(4), s(15.0)),
            ],
        )
        // Debt
        .value(
            "total_debt",
            &[
                (q(1), s(400.0)),
                (q(2), s(420.0)),
                (q(3), s(440.0)),
                (q(4), s(455.0)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let mapping = ts_mapping();
    let cm = credit_mapping();
    let suite = three_statement_checks(mapping).merge(credit_underwriting_checks(cm));
    let report = suite.run(&model, &results).unwrap();

    // --- Errors ---
    assert!(report.has_errors(), "report should have errors");
    let errors = report.findings_by_severity(Severity::Error);
    assert!(
        !errors.is_empty(),
        "should have at least 1 error-level finding"
    );

    let bs_error = errors
        .iter()
        .find(|f| f.check_id == "balance_sheet_articulation")
        .expect("balance sheet articulation error expected");
    let mat = bs_error.materiality.as_ref().unwrap();
    assert!(
        (mat.absolute - 5.0).abs() < 0.1,
        "BS imbalance should be ~5.0, got {}",
        mat.absolute,
    );
    assert_eq!(bs_error.period, Some(q(4)), "BS error should be in Q4",);

    // --- Warnings ---
    assert!(report.has_warnings(), "report should have warnings");
    let warnings = report.findings_by_severity(Severity::Warning);
    assert!(
        !warnings.is_empty(),
        "should have at least 1 warning-level finding"
    );

    let leverage_warn = warnings
        .iter()
        .find(|f| f.check_id == "leverage_range")
        .expect("leverage range warning expected");
    assert_eq!(
        leverage_warn.period,
        Some(q(4)),
        "leverage warning should be in Q4",
    );
    assert!(
        leverage_warn.message.contains("2025Q4"),
        "leverage warning message should mention Q4 period",
    );

    // --- Renderer ---
    let text = CheckReportRenderer::render_text(&report);
    assert!(!text.is_empty(), "rendered text should not be empty");
    assert!(
        text.contains("ERRORS"),
        "rendered text should contain ERRORS section",
    );
    assert!(
        text.contains("WARNINGS"),
        "rendered text should contain WARNINGS section",
    );
}

// ============================================================================
// Balanced model — all checks pass
// ============================================================================

#[test]
fn integration_balanced_model_passes_all_checks() {
    let model = ModelBuilder::new("balanced_3stmt")
        .periods("2025Q1..Q4", None)
        .unwrap()
        // Balance sheet — assets = liabilities + equity every period
        .value(
            "total_assets",
            &[
                (q(1), s(1000.0)),
                (q(2), s(1050.0)),
                (q(3), s(1100.0)),
                (q(4), s(1150.0)),
            ],
        )
        .value(
            "total_liabilities",
            &[
                (q(1), s(600.0)),
                (q(2), s(630.0)),
                (q(3), s(660.0)),
                (q(4), s(690.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (q(1), s(400.0)),
                (q(2), s(420.0)),
                (q(3), s(440.0)),
                (q(4), s(460.0)), // correctly balanced
            ],
        )
        .value(
            "cash",
            &[
                (q(1), s(100.0)),
                (q(2), s(120.0)),
                (q(3), s(135.0)),
                (q(4), s(150.0)),
            ],
        )
        // RE(t) = RE(t-1) + NI(t): 200+20=220, 220+20=240, 240+15=255
        .value(
            "retained_earnings",
            &[
                (q(1), s(200.0)),
                (q(2), s(220.0)),
                (q(3), s(240.0)),
                (q(4), s(255.0)),
            ],
        )
        // Income statement
        .value(
            "revenue",
            &[
                (q(1), s(500.0)),
                (q(2), s(525.0)),
                (q(3), s(550.0)),
                (q(4), s(575.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), s(20.0)),
                (q(2), s(20.0)),
                (q(3), s(20.0)),
                (q(4), s(15.0)),
            ],
        )
        // EBITDA increasing — no trend warning
        .value(
            "ebitda",
            &[
                (q(1), s(80.0)),
                (q(2), s(84.0)),
                (q(3), s(88.0)),
                (q(4), s(92.0)),
            ],
        )
        .value(
            "interest_expense",
            &[
                (q(1), s(30.0)),
                (q(2), s(30.0)),
                (q(3), s(30.0)),
                (q(4), s(30.0)),
            ],
        )
        // Cash reconciles: cash(t) = cash(t-1) + total_cf(t)
        .value(
            "total_cf",
            &[
                (q(1), s(0.0)),
                (q(2), s(20.0)),
                (q(3), s(15.0)),
                (q(4), s(15.0)),
            ],
        )
        // Debt decreasing — no trend warning; leverage stays within (0, 6)
        .value(
            "total_debt",
            &[
                (q(1), s(400.0)),
                (q(2), s(395.0)),
                (q(3), s(390.0)),
                (q(4), s(385.0)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let mapping = ts_mapping();
    let cm = credit_mapping();
    let suite = three_statement_checks(mapping).merge(credit_underwriting_checks(cm));
    let report = suite.run(&model, &results).unwrap();

    assert!(
        !report.has_errors(),
        "balanced model should have no errors, got: {:?}",
        report.findings_by_severity(Severity::Error),
    );
    assert!(
        !report.has_warnings(),
        "balanced model should have no warnings, got: {:?}",
        report.findings_by_severity(Severity::Warning),
    );
    assert!(
        report.summary.total_checks > 0,
        "suite should have executed checks",
    );
    assert!(
        report.summary.passed > 0,
        "balanced model should have passing checks",
    );
}
