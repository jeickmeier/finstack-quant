//! Regression coverage for unit-safe formula checks and normalization reports.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use finstack_quant_core::dates::StubKind;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_statements::adjustments::engine::NormalizationEngine;
use finstack_quant_statements::adjustments::types::{Adjustment, NormalizationConfig};
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckSuite, FormulaCheckSpec, Severity,
};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::{FinancialStatementInstrument, NodeValueType};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use time::macros::date;

fn q1() -> PeriodId {
    PeriodId::quarter(2025, 1).unwrap()
}

fn formula_check(formula: &str, tolerance: Option<f64>) -> FormulaCheckSpec {
    FormulaCheckSpec {
        id: "accounting_units".into(),
        name: "Accounting units".into(),
        category: CheckCategory::AccountingIdentity,
        severity: Severity::Error,
        formula: formula.into(),
        message_template: "Identity failed in {period}".into(),
        tolerance,
    }
}

fn currency_model() -> FinancialModelSpec {
    ModelBuilder::new("check units")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "usd",
            &[(q1(), AmountOrScalar::amount(100.0, Currency::USD).unwrap())],
        )
        .value(
            "eur",
            &[(q1(), AmountOrScalar::amount(100.0, Currency::EUR).unwrap())],
        )
        .value("count", &[(q1(), 100.0.into())])
        .build()
        .unwrap()
}

#[test]
fn formula_checks_reject_incompatible_units_before_comparing_equal_numbers() {
    let model = currency_model();
    let results = Evaluator::new().evaluate(&model).unwrap();
    for (formula, tolerance) in [
        ("usd == eur", None),
        ("usd - eur", Some(0.01)),
        ("usd == count", None),
        ("usd - count", Some(0.01)),
        ("usd > 0", None),
    ] {
        let error = formula_check(formula, tolerance)
            .execute(&CheckContext::new(&model, &results))
            .expect_err("equal raw values must not hide incompatible units");
        assert!(
            error.to_string().contains("Dimensional mismatch"),
            "{formula}: {error}"
        );
    }
}

#[test]
fn formula_checks_reject_stale_result_units_instead_of_trusting_them() {
    let model = currency_model();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.node_value_types.insert(
        "eur".into(),
        NodeValueType::Monetary {
            currency: Currency::USD,
        },
    );
    let error = formula_check("usd - eur", Some(0.01))
        .execute(&CheckContext::new(&model, &results))
        .unwrap_err();
    assert!(error.to_string().contains("result units"));
}

#[test]
fn formula_checks_recover_missing_type_metadata_from_the_model() {
    let mut model = currency_model();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.node_value_types.clear();
    for node in model.nodes.values_mut() {
        node.value_type = None;
    }
    assert!(formula_check("usd - eur", Some(0.01))
        .execute(&CheckContext::new(&model, &results))
        .is_err());
    for (formula, tolerance) in [("usd - usd", Some(0.01)), ("usd / usd == 1", None)] {
        assert!(
            formula_check(formula, tolerance)
                .execute(&CheckContext::new(&model, &results))
                .unwrap()
                .passed
        );
    }
}

#[test]
fn formula_checks_validate_capital_structure_reporting_units() {
    let model = currency_model();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::new(100.0, Currency::USD).unwrap();
    let mut cashflows = CapitalStructureCashflows::new();
    cashflows.reporting_currency = Some(Currency::USD);
    cashflows.totals.insert(q1(), breakdown);
    results.cs_cashflows = Some(cashflows);

    assert!(
        formula_check("cs.interest_expense.total == usd", None)
            .execute(&CheckContext::new(&model, &results))
            .unwrap()
            .passed
    );
    assert!(formula_check("cs.interest_expense.total == eur", None)
        .execute(&CheckContext::new(&model, &results))
        .is_err());

    let mut usd_model = model;
    usd_model.nodes.shift_remove(&NodeId::new("eur"));
    results.nodes.shift_remove("eur");
    results.node_value_types.shift_remove("eur");
    results.cs_cashflows.as_mut().unwrap().reporting_currency = Some(Currency::EUR);
    let error = formula_check("cs.interest_expense.total == usd", None)
        .execute(&CheckContext::new(&usd_model, &results))
        .unwrap_err();
    assert!(error.to_string().contains("result currency"));
}

#[test]
fn formula_checks_keep_native_instrument_units_distinct_from_reporting_totals() {
    let mut model = currency_model();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    let bond = Bond::fixed(
        InstrumentId::new("EUR_BOND"),
        Money::from((1_000_i64, Currency::EUR)),
        Rate::from_decimal(0.1).unwrap(),
        date!(2024 - 01 - 01),
        date!(2027 - 01 - 01),
        StubKind::ShortFront,
        CurveId::new("EUR-OIS"),
    )
    .unwrap();
    model.capital_structure = ModelBuilder::new("native currency declaration")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .add_debt("EUR_BOND", FinancialStatementInstrument::Bond(bond))
        .build()
        .unwrap()
        .capital_structure;
    model.capital_structure.as_mut().unwrap().reporting_currency = Some(Currency::USD);

    let mut native = CashflowBreakdown::with_currency(Currency::EUR);
    native.interest_expense_cash = Money::from((100_i64, Currency::EUR));
    let mut total = CashflowBreakdown::with_currency(Currency::USD);
    total.interest_expense_cash = Money::from((100_i64, Currency::USD));
    let mut cashflows = CapitalStructureCashflows::new();
    cashflows.reporting_currency = Some(Currency::USD);
    cashflows.totals.insert(q1(), total.clone());
    cashflows
        .by_instrument
        .insert("EUR_BOND".into(), [(q1(), native)].into_iter().collect());
    results.cs_cashflows = Some(cashflows);

    let mut result_only_model = model.clone();
    result_only_model.capital_structure = None;
    for check_model in [&model, &result_only_model] {
        for formula in [
            "cs.interest_expense.EUR_BOND == eur",
            "cs.interest_expense.total == usd",
        ] {
            assert!(
                formula_check(formula, None)
                    .execute(&CheckContext::new(check_model, &results))
                    .unwrap()
                    .passed
            );
        }
        let error = formula_check(
            "cs.interest_expense.EUR_BOND == cs.interest_expense.total",
            None,
        )
        .execute(&CheckContext::new(check_model, &results))
        .unwrap_err();
        assert!(error.to_string().contains("Dimensional mismatch"));
    }

    results
        .cs_cashflows
        .as_mut()
        .unwrap()
        .by_instrument
        .get_mut("EUR_BOND")
        .unwrap()
        .insert(q1(), total);
    let error = formula_check("cs.interest_expense.EUR_BOND == usd", None)
        .execute(&CheckContext::new(&model, &results))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("instrument 'EUR_BOND' result currency"));
}

fn checked_balance_sheet() -> (FinancialModelSpec, StatementResult, CheckSuite) {
    let model = ModelBuilder::new("balance sheet")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("assets", &[(q1(), 100.0.into())])
        .value("liabilities", &[(q1(), 60.0.into())])
        .value("equity", &[(q1(), 40.0.into())])
        .build()
        .unwrap();
    let suite = CheckSuite::builder("balance sheet")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("assets")],
            liabilities_nodes: vec![NodeId::new("liabilities")],
            equity_nodes: vec![NodeId::new("equity")],
            tolerance: None,
        })
        .build();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.check_report = Some(suite.run(&model, &results).unwrap());
    (model, results, suite)
}

#[test]
fn normalization_merge_clears_the_report_for_the_previous_financial_values() {
    let (model, mut results, suite) = checked_balance_sheet();
    assert_eq!(results.check_report.as_ref().unwrap().summary.passed, 1);
    let config = NormalizationConfig::new("assets")
        .add_adjustment(Adjustment::fixed(
            "asset_adjustment",
            "Asset adjustment",
            [(q1(), 50.0)].into_iter().collect(),
        ))
        .unwrap();
    let normalized = NormalizationEngine::normalize(&results, &config).unwrap();

    NormalizationEngine::merge_into_results(&mut results, &normalized, "assets").unwrap();

    assert_eq!(results.get("assets", &q1()), Some(150.0));
    assert!(results.check_report.is_none());
    assert_eq!(suite.run(&model, &results).unwrap().summary.failed, 1);
}

#[test]
fn failed_and_empty_normalization_merges_preserve_the_original_report_and_values() {
    let (_, mut results, _) = checked_balance_sheet();
    let original_report = results.check_report.clone();
    let original_values = results.nodes.clone();
    NormalizationEngine::merge_into_results(&mut results, &[], "assets").unwrap();
    assert_eq!(results.check_report, original_report);

    let normalized =
        NormalizationEngine::normalize(&results, &NormalizationConfig::new("assets")).unwrap();
    let duplicated = vec![normalized[0].clone(), normalized[0].clone()];
    assert!(NormalizationEngine::merge_into_results(&mut results, &duplicated, "assets").is_err());
    assert_eq!(results.nodes, original_values);
    assert_eq!(results.check_report, original_report);
}
