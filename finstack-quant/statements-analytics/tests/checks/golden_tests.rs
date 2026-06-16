#![allow(clippy::unwrap_used)]

use std::path::Path;

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    credit_underwriting_checks, three_statement_checks, CreditMapping, ThreeStatementMapping,
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

/// Build a deterministic 3-statement model (4 quarters, one intentional BS
/// imbalance in Q4) and run the combined three-statement + credit-underwriting
/// suites.
fn build_and_check_model() -> finstack_quant_statements::checks::CheckReport {
    let model = ModelBuilder::new("golden_3stmt")
        .periods("2025Q1..Q4", None)
        .unwrap()
        // Balance sheet — Q4 equity is 455, not 460 → intentional imbalance
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
                (q(4), s(65.0)),
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

    let suite =
        three_statement_checks(ts_mapping()).merge(credit_underwriting_checks(credit_mapping()));
    suite.run(&model, &results).unwrap()
}

#[test]
fn golden_three_statement_report() {
    let report = build_and_check_model();
    let actual_json = serde_json::to_string_pretty(&report).unwrap();

    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/checks/golden/three_statement_report.json");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&golden_path, &actual_json).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "Golden file not found at {}: {}. Run with UPDATE_GOLDEN=1 to create it.",
            golden_path.display(),
            e,
        )
    });
    assert_eq!(
        actual_json.trim(),
        expected.trim(),
        "Golden snapshot mismatch. Run with UPDATE_GOLDEN=1 to update.",
    );
}
