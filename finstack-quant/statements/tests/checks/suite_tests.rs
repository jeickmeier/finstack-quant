//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
use finstack_quant_statements::checks::{
    BuiltinCheckSpec, CheckCategory, CheckSuite, CheckSuiteSpec, FormulaCheckSpec, PeriodScope,
    Severity,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, ForecastMethod, ForecastSpec, NodeId};
use indexmap::indexmap;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

fn positive_revenue_suite() -> CheckSuite {
    CheckSuite::builder("positive revenue")
        .add_check(FormulaCheckSpec {
            id: "revenue_positive".into(),
            name: "Revenue must be positive".into(),
            category: CheckCategory::InternalConsistency,
            severity: Severity::Error,
            formula: "revenue > 0".into(),
            message_template: "Revenue was non-positive in {period}".into(),
            tolerance: None,
        })
        .build()
}

// JSON roundtrip: serialize → deserialize → resolve → check count

#[test]
fn suite_spec_json_roundtrip_and_resolve() {
    let spec = CheckSuiteSpec {
        name: "roundtrip_suite".into(),
        description: Some("Testing JSON roundtrip".into()),
        builtin_checks: vec![
            BuiltinCheckSpec::BalanceSheetArticulation(BalanceSheetArticulation {
                assets_nodes: vec![NodeId::new("total_assets")],
                liabilities_nodes: vec![NodeId::new("total_liabilities")],
                equity_nodes: vec![NodeId::new("total_equity")],
                tolerance: Some(0.5),
            }),
            BuiltinCheckSpec::NonFinite(NonFiniteCheck {
                nodes: vec![NodeId::new("revenue")],
            }),
            BuiltinCheckSpec::MissingValue(MissingValueCheck {
                required_nodes: vec![NodeId::new("revenue"), NodeId::new("cogs")],
                scope: PeriodScope::AllPeriods,
            }),
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
    assert_eq!(suite.len(), 4);
    assert_eq!(suite.name(), "roundtrip_suite");
    assert_eq!(suite.description(), Some("Testing JSON roundtrip"));
}

// Tagged serde: verify JSON tag format

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
        BuiltinCheckSpec::BalanceSheetArticulation(BalanceSheetArticulation {
            assets_nodes,
            liabilities_nodes,
            equity_nodes,
            tolerance,
        }) => {
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
        BuiltinCheckSpec::RetainedEarningsReconciliation(RetainedEarningsReconciliation {
            retained_earnings_node,
            net_income_node,
            dividends_node,
            ..
        }) => {
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
        BuiltinCheckSpec::CashReconciliation(CashReconciliation {
            cash_balance_node,
            total_cash_flow_node,
            cfo_node,
            ..
        }) => {
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
        BuiltinCheckSpec::SignConvention(SignConventionCheck {
            positive_nodes,
            negative_nodes,
        }) => {
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

// Resolved suite runs against a model

#[test]
fn resolved_suite_runs_against_model() {
    let spec = CheckSuiteSpec {
        name: "bs_check".into(),
        description: None,
        builtin_checks: vec![BuiltinCheckSpec::BalanceSheetArticulation(
            BalanceSheetArticulation {
                assets_nodes: vec![NodeId::new("total_assets")],
                liabilities_nodes: vec![NodeId::new("total_liabilities")],
                equity_nodes: vec![NodeId::new("total_equity")],
                tolerance: None,
            },
        )],
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

#[test]
fn run_model_evaluates_with_value_forecast_formula_precedence() {
    let model = ModelBuilder::new("precedence")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .mixed("revenue")
        .values(&[(q(1), s(100.0)), (q(2), s(110.0))])
        .forecast(ForecastSpec {
            method: ForecastMethod::GrowthPct,
            params: indexmap! { "rate".into() => serde_json::json!(0.05) },
        })
        .formula("0")
        .unwrap()
        .build()
        .unwrap()
        .build()
        .unwrap();

    let report = positive_revenue_suite().run_model(&model, None).unwrap();

    assert!(!report.has_errors());
}

#[test]
fn run_model_uses_supplied_results_without_recomputation() {
    let model = ModelBuilder::new("supplied")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), s(100.0))])
        .build()
        .unwrap();
    let mut supplied = Evaluator::new().evaluate(&model).unwrap();
    supplied
        .nodes
        .get_mut("revenue")
        .unwrap()
        .insert(q(1), -1.0);

    let report = positive_revenue_suite()
        .run_model(&model, Some(&supplied))
        .unwrap();

    assert!(report.has_errors());
}

// Materiality is a reporting filter, not a verdict knob: an Error finding
// must never be suppressed in a way that flips a failing check to passed.

#[test]
fn materiality_threshold_does_not_flip_error_verdict() {
    use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
    use finstack_quant_statements::checks::{CheckConfig, CheckSuite};

    // Imbalance of 100 on a 1000 sheet: an unambiguous Error.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), s(1000.0))])
        .value("total_liabilities", &[(q(1), s(600.0))])
        .value("total_equity", &[(q(1), s(300.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let suite = CheckSuite::builder("materiality")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: None,
        })
        .config(CheckConfig {
            materiality_threshold: 1_000_000.0, // far above the 100 imbalance
            ..CheckConfig::default()
        })
        .build();

    let report = suite.run(&model, &results).unwrap();
    let result = &report.results[0];

    assert!(
        !result.passed,
        "a materiality threshold must not convert a failing identity into a pass"
    );
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error),
        "the Error finding must survive materiality filtering, got: {:?}",
        result.findings
    );
}
