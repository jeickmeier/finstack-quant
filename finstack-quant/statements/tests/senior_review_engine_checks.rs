//! Regression coverage for evaluated input units and accounting-check policies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use finstack_quant_statements::checks::builtins::{
    BalanceSheetArticulation, CashReconciliation, RetainedEarningsReconciliation,
};
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckConfig, CheckContext, CheckSuite, CheckSuiteSpec, FormulaCheckSpec,
    Severity, SignConventionPolicy,
};
use finstack_quant_statements::evaluator::MonteCarloConfig;
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::NodeValueType;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).unwrap()
}

fn identities(tolerance: Option<f64>) -> Vec<Box<dyn Check>> {
    vec![
        Box::new(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("a")],
            liabilities_nodes: vec![NodeId::new("b")],
            equity_nodes: vec![NodeId::new("c")],
            tolerance,
        }),
        Box::new(CashReconciliation {
            cash_balance_node: NodeId::new("a"),
            total_cash_flow_node: NodeId::new("b"),
            cfo_node: None,
            cfi_node: None,
            cff_node: None,
            tolerance,
        }),
        Box::new(RetainedEarningsReconciliation {
            retained_earnings_node: NodeId::new("a"),
            net_income_node: NodeId::new("b"),
            dividends_node: None,
            other_adjustments: vec![],
            tolerance,
            dividends_sign_convention: SignConventionPolicy::MagnitudePositive,
        }),
    ]
}

fn money_model(currency: Currency) -> FinancialModelSpec {
    ModelBuilder::new("units")
        .periods("2025Q1..Q2", Some("2025Q1"))
        .unwrap()
        .value(
            "a",
            &[
                (q(1), AmountOrScalar::amount(100.0, Currency::USD).unwrap()),
                (q(2), AmountOrScalar::amount(110.0, Currency::USD).unwrap()),
            ],
        )
        .value(
            "b",
            &[
                (q(1), AmountOrScalar::amount(60.0, currency).unwrap()),
                (q(2), AmountOrScalar::amount(60.0, currency).unwrap()),
            ],
        )
        .value(
            "c",
            &[
                (q(1), AmountOrScalar::amount(40.0, currency).unwrap()),
                (q(2), AmountOrScalar::amount(50.0, currency).unwrap()),
            ],
        )
        .build()
        .unwrap()
}

#[test]
fn prepared_and_ordinary_evaluation_reject_changed_input_currency() {
    let mut model = money_model(Currency::USD);
    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&model).unwrap();
    model
        .get_node_mut("a")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .insert(q(1), AmountOrScalar::amount(2.0, Currency::EUR).unwrap());
    for error in [
        evaluator.evaluate_prepared(&model, &prepared).unwrap_err(),
        evaluator.evaluate(&model).unwrap_err(),
        evaluator
            .evaluate_monte_carlo(&model, &MonteCarloConfig::new(2, 42))
            .unwrap_err(),
    ] {
        assert!(
            error.to_string().to_lowercase().contains("currency")
                || error.to_string().contains("declares")
        );
    }
}

#[test]
fn prepared_evaluation_rejects_scalar_replacement_and_changed_declaration() {
    let mut model = money_model(Currency::USD);
    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&model).unwrap();
    let node = model.get_node_mut("a").unwrap();
    node.values = Some(
        [(q(1), 2.0.into()), (q(2), 3.0.into())]
            .into_iter()
            .collect(),
    );
    assert!(evaluator
        .evaluate_prepared(&model, &prepared)
        .unwrap_err()
        .to_string()
        .contains("declares"));
    model.get_node_mut("a").unwrap().value_type = Some(NodeValueType::Scalar);
    assert!(evaluator
        .evaluate_prepared(&model, &prepared)
        .unwrap_err()
        .to_string()
        .contains("declared value type"));
}

#[test]
fn prepared_evaluation_accepts_changed_amounts_in_the_original_currency() {
    let mut model = money_model(Currency::USD);
    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&model).unwrap();
    model
        .get_node_mut("a")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .insert(q(1), AmountOrScalar::amount(2.0, Currency::USD).unwrap());
    let result = evaluator.evaluate_prepared(&model, &prepared).unwrap();
    assert_eq!(
        result.get_money("a", &q(1)),
        Some(Money::from((2_i64, Currency::USD)))
    );
}

#[test]
fn accounting_identities_reject_foreign_currency_and_scalar_money_mix() {
    let foreign = money_model(Currency::EUR);
    let foreign_results = Evaluator::new().evaluate(&foreign).unwrap();
    for check in identities(None) {
        assert!(check
            .execute(&CheckContext::new(&foreign, &foreign_results))
            .unwrap_err()
            .to_string()
            .contains("incompatible units"));
    }
    let mut scalar = money_model(Currency::USD);
    let node = scalar.get_node_mut("b").unwrap();
    node.values = Some(
        [(q(1), 60.0.into()), (q(2), 60.0.into())]
            .into_iter()
            .collect(),
    );
    node.value_type = Some(NodeValueType::Scalar);
    let scalar_results = Evaluator::new().evaluate(&scalar).unwrap();
    for check in identities(None) {
        assert!(check
            .execute(&CheckContext::new(&scalar, &scalar_results))
            .unwrap_err()
            .to_string()
            .contains("incompatible units"));
    }
    let same = money_model(Currency::USD);
    let results = Evaluator::new().evaluate(&same).unwrap();
    for check in identities(None) {
        assert!(check.execute(&CheckContext::new(&same, &results)).is_ok());
    }
}

#[test]
fn precomputed_identity_checks_reject_units_that_disagree_with_the_model() {
    let mut model = money_model(Currency::EUR);
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    for value_type in results.node_value_types.values_mut() {
        *value_type = NodeValueType::Scalar;
    }
    let suite = CheckSuite::builder("precomputed units")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("a")],
            liabilities_nodes: vec![NodeId::new("b")],
            equity_nodes: vec![NodeId::new("c")],
            tolerance: None,
        })
        .build();
    // The numeric balance sheet articulates in both periods. Stale scalar
    // metadata must not hide that the operands use different currencies.
    for declarations_present in [true, false] {
        if !declarations_present {
            for node in model.nodes.values_mut() {
                node.value_type = None;
            }
        }
        let error = suite.run_model(&model, Some(&results)).unwrap_err();
        assert!(error.to_string().contains("result units"));
        for check in identities(None) {
            assert!(check
                .execute(&CheckContext::new(&model, &results))
                .unwrap_err()
                .to_string()
                .contains("result units"));
        }
    }
}

#[test]
fn precomputed_checks_reject_explicit_currency_changes_with_stale_declarations() {
    let mut model = money_model(Currency::USD);
    let results = Evaluator::new().evaluate(&model).unwrap();
    let node = model.get_node_mut("b").unwrap();
    for amount in node.values.as_mut().unwrap().values_mut() {
        *amount = AmountOrScalar::amount(60.0, Currency::EUR).unwrap();
    }
    let suite = CheckSuite::builder("changed explicit units")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("a")],
            liabilities_nodes: vec![NodeId::new("b")],
            equity_nodes: vec![NodeId::new("c")],
            tolerance: None,
        })
        .build();
    let error = suite.run_model(&model, Some(&results)).unwrap_err();
    assert!(error.to_string().contains("Node 'b' declares"));
    assert!(error.to_string().contains("EUR"));
}

#[test]
fn precomputed_inferred_money_units_keep_valid_results_and_missing_node_warnings() {
    let model = ModelBuilder::new("inferred units")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (q(1), AmountOrScalar::amount(100.0, Currency::USD).unwrap()),
                (q(2), AmountOrScalar::amount(200.0, Currency::USD).unwrap()),
            ],
        )
        .compute("b", "a * 0.6")
        .unwrap()
        .compute("c", "a * 0.4")
        .unwrap()
        .build()
        .unwrap();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    let suite = CheckSuite::builder("inferred precomputed units")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("a")],
            liabilities_nodes: vec![NodeId::new("b")],
            equity_nodes: vec![NodeId::new("c")],
            tolerance: None,
        })
        .build();
    assert!(suite.run_model(&model, Some(&results)).unwrap().results[0].passed);
    results.node_value_types.clear();
    assert!(suite.run_model(&model, Some(&results)).unwrap().results[0].passed);
    results.nodes.shift_remove("b");
    let report = suite.run_model(&model, Some(&results)).unwrap();
    let findings = &report.results[0].findings;
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|finding| {
        finding.severity == Severity::Warning && finding.message.contains("[b]")
    }));
}

#[test]
fn accounting_identities_reject_invalid_tolerance_overrides() {
    let model = money_model(Currency::USD);
    let results = Evaluator::new().evaluate(&model).unwrap();
    for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
        for check in identities(Some(tolerance)) {
            assert!(check
                .execute(&CheckContext::new(&model, &results))
                .unwrap_err()
                .to_string()
                .contains("finite and nonnegative"));
        }
    }
}

#[test]
fn check_configs_reject_invalid_numeric_policies_even_for_empty_suites() {
    let model = money_model(Currency::USD);
    let results = Evaluator::new().evaluate(&model).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        for config in [
            CheckConfig {
                default_tolerance: bad,
                ..Default::default()
            },
            CheckConfig {
                default_relative_tolerance: bad,
                ..Default::default()
            },
            CheckConfig {
                materiality_threshold: bad,
                ..Default::default()
            },
        ] {
            assert!(config.validate().is_err());
            assert!(CheckSuiteSpec {
                name: "empty".into(),
                description: None,
                builtin_checks: Vec::new(),
                formula_checks: Vec::new(),
                config: config.clone(),
            }
            .resolve()
            .is_err());
            let suite = CheckSuite::builder("empty").config(config).build();
            assert!(suite.run(&model, &results).is_err());
        }
    }
    assert!(CheckConfig {
        default_tolerance: 0.0,
        default_relative_tolerance: 0.0,
        materiality_threshold: 0.0,
        ..Default::default()
    }
    .validate()
    .is_ok());
}

#[test]
fn shared_formula_check_history_preserves_temporal_results_and_visibility() {
    let model = ModelBuilder::new("history")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "x",
            &[
                (q(1), 10.0.into()),
                (q(2), 20.0.into()),
                (q(3), 30.0.into()),
                (q(4), 40.0.into()),
            ],
        )
        .compute("expected", "coalesce(lag(x, 1), 0) + rolling_mean(x, 2, 1)")
        .unwrap()
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = FormulaCheckSpec {
        id: "temporal".into(),
        name: "Temporal parity".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "expected - (coalesce(lag(x, 1), 0) + rolling_mean(x, 2, 1))".into(),
        message_template: "bad {period}".into(),
        tolerance: Some(1e-10),
    };
    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();
    assert!(result.passed, "{:?}", result.findings);
    assert_eq!(results.get("expected", &q(1)), Some(10.0));
    assert_eq!(results.get("expected", &q(4)), Some(65.0));
}
