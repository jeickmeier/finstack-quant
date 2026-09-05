//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    credit_underwriting_checks, lbo_model_checks, three_statement_checks, CreditMapping,
    ThreeStatementMapping,
};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

fn minimal_ts_mapping() -> ThreeStatementMapping {
    ThreeStatementMapping {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        cash_node: NodeId::new("cash"),
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        ppe_node: None,
        depreciation_node: None,
        interest_expense_node: None,
        tax_expense_node: None,
        pretax_income_node: None,
        cfo_node: None,
        cfi_node: None,
        cff_node: None,
        total_cf_node: None,
        capex_node: None,
        dividends_node: None,
        ppe_additions_node: None,
        intangible_additions_node: None,
        dividends_equity_node: None,
        debt_balance_nodes: vec![],
        cs_interest_node: None,
        wc_change_cf_node: None,
        current_assets_nodes: vec![],
        current_liabilities_nodes: vec![],
    }
}

fn full_ts_mapping() -> ThreeStatementMapping {
    ThreeStatementMapping {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        cash_node: NodeId::new("cash"),
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        ppe_node: Some(NodeId::new("ppe")),
        depreciation_node: Some(NodeId::new("depreciation")),
        interest_expense_node: Some(NodeId::new("interest_expense")),
        tax_expense_node: Some(NodeId::new("tax_expense")),
        pretax_income_node: Some(NodeId::new("pretax_income")),
        cfo_node: Some(NodeId::new("cfo")),
        cfi_node: Some(NodeId::new("cfi")),
        cff_node: Some(NodeId::new("cff")),
        total_cf_node: Some(NodeId::new("total_cf")),
        capex_node: Some(NodeId::new("capex")),
        dividends_node: Some(NodeId::new("dividends")),
        ppe_additions_node: None,
        intangible_additions_node: None,
        dividends_equity_node: None,
        debt_balance_nodes: vec![],
        cs_interest_node: None,
        wc_change_cf_node: None,
        current_assets_nodes: vec![],
        current_liabilities_nodes: vec![],
    }
}

fn credit_mapping() -> CreditMapping {
    CreditMapping {
        debt_node: NodeId::new("debt"),
        ebitda_node: NodeId::new("ebitda"),
        interest_expense_node: NodeId::new("interest_expense"),
        fcf_node: Some(NodeId::new("fcf")),
        cash_node: None,
        cash_burn_node: None,
        leverage_warn: None,
        coverage_min_warn: None,
    }
}

// Suite check counts

#[test]
fn three_statement_minimal_has_expected_checks() {
    let suite = three_statement_checks(minimal_ts_mapping());
    // BS articulation, RE reconciliation, GrowthRate, NonFinite, MissingValue = 5
    assert_eq!(suite.len(), 5, "minimal mapping should produce 5 checks");
}

#[test]
fn three_statement_full_has_expected_checks() {
    let suite = three_statement_checks(full_ts_mapping());
    // BS articulation, RE reconciliation, Cash recon, Depreciation recon,
    // EffectiveTaxRate, GrowthRate, NonFinite, MissingValue = 8.
    // Capex / dividend / interest / working-capital reconciliations stay off:
    // this fixture has no additions, equity-dividend, debt-balance or
    // working-capital nodes to reconcile against.
    assert_eq!(suite.len(), 8, "full mapping should produce 8 checks");
}

#[test]
fn partial_mapping_skips_optional_checks() {
    let mut mapping = full_ts_mapping();
    mapping.total_cf_node = None; // disables cash recon
    mapping.ppe_node = None; // disables depreciation recon

    let suite = three_statement_checks(mapping);
    // BS articulation, RE reconciliation, EffectiveTaxRate, GrowthRate,
    // NonFinite, MissingValue = 6
    assert_eq!(
        suite.len(),
        6,
        "removing optional nodes should skip their checks"
    );
}

#[test]
fn credit_suite_has_expected_checks() {
    let suite = credit_underwriting_checks(credit_mapping());
    // NonFinite, Leverage, Coverage, FcfSign, TrendEbitda, TrendDebt = 6
    assert_eq!(suite.len(), 6, "credit suite should produce 6 checks");
}

#[test]
fn credit_suite_without_fcf_has_fewer() {
    let mut cm = credit_mapping();
    cm.fcf_node = None;
    let suite = credit_underwriting_checks(cm);
    // NonFinite, Leverage, Coverage, TrendEbitda, TrendDebt = 5
    assert_eq!(
        suite.len(),
        5,
        "without fcf_node suite should have 5 checks"
    );
}

// Newly-registered checks reach the suite when their inputs are supplied

/// Extend the full mapping with the inputs the reconciliation checks need.
fn reconciliation_ts_mapping() -> ThreeStatementMapping {
    ThreeStatementMapping {
        ppe_additions_node: Some(NodeId::new("ppe_additions")),
        intangible_additions_node: Some(NodeId::new("intangible_additions")),
        dividends_equity_node: Some(NodeId::new("dividends_equity")),
        debt_balance_nodes: vec![(NodeId::new("total_debt"), Some(NodeId::new("debt_rate")))],
        wc_change_cf_node: Some(NodeId::new("wc_change")),
        current_assets_nodes: vec![NodeId::new("receivables")],
        current_liabilities_nodes: vec![NodeId::new("payables")],
        ..full_ts_mapping()
    }
}

#[test]
fn reconciliation_checks_are_registered_when_inputs_present() {
    let suite = three_statement_checks(reconciliation_ts_mapping());
    let ids = suite.check_ids();

    for expected in [
        "capex_reconciliation",
        "dividend_reconciliation",
        "interest_expense_reconciliation",
        "working_capital_consistency",
        "effective_tax_rate",
        "growth_rate_consistency",
    ] {
        assert!(
            ids.contains(&expected),
            "{expected} should be registered in the three-statement suite; got {ids:?}"
        );
    }
}

#[test]
fn liquidity_runway_is_registered_when_cash_and_burn_present() {
    let mut cm = credit_mapping();
    cm.cash_node = Some(NodeId::new("cash"));
    cm.cash_burn_node = Some(NodeId::new("monthly_burn"));

    let suite = credit_underwriting_checks(cm);
    let ids = suite.check_ids();
    assert!(
        ids.contains(&"liquidity_runway"),
        "liquidity_runway should be registered when cash and burn nodes are mapped; got {ids:?}"
    );
}

#[test]
fn liquidity_runway_stays_off_without_a_burn_node() {
    let mut cm = credit_mapping();
    cm.cash_node = Some(NodeId::new("cash"));
    cm.cash_burn_node = None;

    let suite = credit_underwriting_checks(cm);
    let ids = suite.check_ids();
    assert!(
        !ids.contains(&"liquidity_runway"),
        "liquidity_runway needs a burn node; got {ids:?}"
    );
}

#[test]
fn lbo_suite_merges_both() {
    let ts = full_ts_mapping();
    let cr = credit_mapping();
    let suite = lbo_model_checks(ts, cr);
    // 8 (three-statement full) + 6 (credit) = 14
    assert_eq!(suite.len(), 14, "LBO suite should merge both suites");
}

// Suite runs correctly against a balanced model

#[test]
fn three_statement_suite_runs_against_balanced_model() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("total_assets", &[(q(1), s(1000.0)), (q(2), s(1100.0))])
        .value("total_liabilities", &[(q(1), s(600.0)), (q(2), s(650.0))])
        .value("total_equity", &[(q(1), s(400.0)), (q(2), s(450.0))])
        .value("cash", &[(q(1), s(200.0)), (q(2), s(220.0))])
        .value("retained_earnings", &[(q(1), s(300.0)), (q(2), s(350.0))])
        .value("net_income", &[(q(1), s(50.0)), (q(2), s(50.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let suite = three_statement_checks(minimal_ts_mapping());
    let report = suite.run(&model, &results).unwrap();

    // BS should pass (balanced). RE may flag if 300+50 != 350.
    // The key test is that it runs without error.
    assert!(report.summary.total_checks > 0);
}
