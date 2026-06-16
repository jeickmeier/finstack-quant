#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{
    BuiltinCheckSpec, CheckSuiteSpec, FormulaCheckSpec, PeriodScope, Severity,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

// ============================================================================
// JSON roundtrip: serialize → deserialize → resolve → check count
// ============================================================================

#[test]
fn suite_spec_json_roundtrip_and_resolve() {
    let spec = CheckSuiteSpec {
        name: "roundtrip_suite".into(),
        description: Some("Testing JSON roundtrip".into()),
        builtin_checks: vec![
            BuiltinCheckSpec::BalanceSheetArticulation {
                assets_nodes: vec![NodeId::new("total_assets")],
                liabilities_nodes: vec![NodeId::new("total_liabilities")],
                equity_nodes: vec![NodeId::new("total_equity")],
                tolerance: Some(0.5),
            },
            BuiltinCheckSpec::NonFinite {
                nodes: vec![NodeId::new("revenue")],
            },
            BuiltinCheckSpec::MissingValue {
                required_nodes: vec![NodeId::new("revenue"), NodeId::new("cogs")],
                scope: PeriodScope::AllPeriods,
            },
        ],
        formula_checks: vec![FormulaCheckSpec {
            id: "revenue_positive".into(),
            name: "Revenue must be positive".into(),
            category: finstack_quant_statements::checks::CheckCategory::InternalConsistency,
            severity: Severity::Error,
            formula: "revenue > 0".into(),
            message_template: "Revenue bad in {period}".into(),
            tolerance: None,
        }],
        config: Default::default(),
    };

    let json = serde_json::to_string_pretty(&spec).unwrap();
    let deserialized: CheckSuiteSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "roundtrip_suite");
    assert_eq!(
        deserialized.description.as_deref(),
        Some("Testing JSON roundtrip")
    );
    assert_eq!(deserialized.builtin_checks.len(), 3);
    assert_eq!(deserialized.formula_checks.len(), 1);

    let suite = deserialized.resolve().unwrap();
    assert_eq!(suite.len(), 3); // only builtins resolved
    assert_eq!(suite.name(), "roundtrip_suite");
    assert_eq!(suite.description(), Some("Testing JSON roundtrip"));
}

// ============================================================================
// Tagged serde: verify JSON tag format
// ============================================================================

#[test]
fn builtin_check_spec_tagged_serde() {
    let json = r#"{
        "type": "balance_sheet_articulation",
        "assets_nodes": ["total_assets"],
        "liabilities_nodes": ["total_liabilities"],
        "equity_nodes": ["total_equity"],
        "tolerance": 0.01
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::BalanceSheetArticulation {
            assets_nodes,
            liabilities_nodes,
            equity_nodes,
            tolerance,
        } => {
            assert_eq!(assets_nodes, &[NodeId::new("total_assets")]);
            assert_eq!(liabilities_nodes, &[NodeId::new("total_liabilities")]);
            assert_eq!(equity_nodes, &[NodeId::new("total_equity")]);
            assert_eq!(*tolerance, Some(0.01));
        }
        other => panic!("Expected BalanceSheetArticulation, got {other:?}"),
    }

    let roundtrip = serde_json::to_string(&spec).unwrap();
    assert!(roundtrip.contains("\"type\":\"balance_sheet_articulation\""));
}

#[test]
fn retained_earnings_spec_tagged_serde() {
    let json = r#"{
        "type": "retained_earnings_reconciliation",
        "retained_earnings_node": "re",
        "net_income_node": "ni",
        "dividends_node": "divs"
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::RetainedEarningsReconciliation {
            retained_earnings_node,
            net_income_node,
            dividends_node,
            ..
        } => {
            assert_eq!(retained_earnings_node, &NodeId::new("re"));
            assert_eq!(net_income_node, &NodeId::new("ni"));
            assert_eq!(dividends_node, &Some(NodeId::new("divs")));
        }
        other => panic!("Expected RetainedEarningsReconciliation, got {other:?}"),
    }
}

#[test]
fn cash_reconciliation_spec_tagged_serde() {
    let json = r#"{
        "type": "cash_reconciliation",
        "cash_balance_node": "cash",
        "total_cash_flow_node": "total_cf",
        "cfo_node": "cfo",
        "cfi_node": "cfi",
        "cff_node": "cff"
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::CashReconciliation {
            cash_balance_node,
            total_cash_flow_node,
            cfo_node,
            ..
        } => {
            assert_eq!(cash_balance_node, &NodeId::new("cash"));
            assert_eq!(total_cash_flow_node, &NodeId::new("total_cf"));
            assert_eq!(cfo_node, &Some(NodeId::new("cfo")));
        }
        other => panic!("Expected CashReconciliation, got {other:?}"),
    }
}

#[test]
fn sign_convention_spec_tagged_serde() {
    let json = r#"{
        "type": "sign_convention",
        "positive_nodes": ["revenue"],
        "negative_nodes": ["cogs"]
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::SignConvention {
            positive_nodes,
            negative_nodes,
        } => {
            assert_eq!(positive_nodes, &[NodeId::new("revenue")]);
            assert_eq!(negative_nodes, &[NodeId::new("cogs")]);
        }
        other => panic!("Expected SignConvention, got {other:?}"),
    }
}

#[test]
fn formula_check_spec_serde() {
    let json = r#"{
        "id": "margin_check",
        "name": "Margin >= 20%",
        "category": "internal_consistency",
        "severity": "warning",
        "formula": "(revenue - cogs) / revenue >= 0.20",
        "message_template": "Margin too low in {period}",
        "tolerance": 0.001
    }"#;

    let spec: FormulaCheckSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.id, "margin_check");
    assert_eq!(spec.severity, Severity::Warning);
    assert_eq!(spec.tolerance, Some(0.001));
}

// ============================================================================
// Resolved suite runs against a model
// ============================================================================

#[test]
fn resolved_suite_runs_against_model() {
    let spec = CheckSuiteSpec {
        name: "bs_check".into(),
        description: None,
        builtin_checks: vec![BuiltinCheckSpec::BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: None,
        }],
        formula_checks: vec![],
        config: Default::default(),
    };

    let suite = spec.resolve().unwrap();

    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("total_assets", &[(q(1), s(1000.0)), (q(2), s(1100.0))])
        .value("total_liabilities", &[(q(1), s(600.0)), (q(2), s(700.0))])
        .value("total_equity", &[(q(1), s(400.0)), (q(2), s(400.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let report = suite.run(&model, &results).unwrap();
    assert!(!report.has_errors());
    assert_eq!(report.summary.passed, 1);
}
