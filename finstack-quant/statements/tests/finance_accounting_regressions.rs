//! Regression coverage for unit-safe formula checks and normalization reports.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::dates::StubKind;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_statements::adjustments::{normalize, Adjustment, NormalizationConfig};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckSuite, FormulaCheckSpec, Severity,
};
use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeId};
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
    {
        let mut model_builder = ModelBuilder::new("check units");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "usd",
            &[(q1(), AmountOrScalar::amount(100.0, Currency::USD).unwrap())],
        );
        model_builder.value(
            "eur",
            &[(q1(), AmountOrScalar::amount(100.0, Currency::EUR).unwrap())],
        );
        model_builder.value("count", &[(q1(), 100.0.into())]);
        model_builder.build().unwrap()
    }
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
    model.capital_structure = {
        let mut model_builder = ModelBuilder::new("native currency declaration");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.add_debt("EUR_BOND", FinancialStatementInstrument::Bond(bond));
        model_builder.build().unwrap()
    }
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
    let model = {
        let mut model_builder = ModelBuilder::new("balance sheet");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("assets", &[(q1(), 100.0.into())]);
        model_builder.value("liabilities", &[(q1(), 60.0.into())]);
        model_builder.value("equity", &[(q1(), 40.0.into())]);
        model_builder.build().unwrap()
    };
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
    let normalized = normalize(&results, &config).unwrap();

    finstack_quant_statements::adjustments::merge_into_results(&mut results, &normalized, "assets")
        .unwrap();

    assert_eq!(results.get("assets", &q1()), Some(150.0));
    assert!(results.check_report.is_none());
    assert_eq!(suite.run(&model, &results).unwrap().summary.failed, 1);
}

#[test]
fn failed_and_empty_normalization_merges_preserve_the_original_report_and_values() {
    let (_, mut results, _) = checked_balance_sheet();
    let original_report = results.check_report.clone();
    let original_values = results.nodes.clone();
    finstack_quant_statements::adjustments::merge_into_results(&mut results, &[], "assets")
        .unwrap();
    assert_eq!(results.check_report, original_report);

    let normalized = normalize(&results, &NormalizationConfig::new("assets")).unwrap();
    let duplicated = vec![normalized[0].clone(), normalized[0].clone()];
    assert!(finstack_quant_statements::adjustments::merge_into_results(
        &mut results,
        &duplicated,
        "assets"
    )
    .is_err());
    assert_eq!(results.nodes, original_values);
    assert_eq!(results.check_report, original_report);
}

fn assert_arithmetic_failure(result: &finstack_quant_statements::checks::CheckResult) {
    assert!(!result.passed);
    assert!(result.findings.iter().any(|finding| {
        finding.severity == Severity::Error
            && finding.message.contains("Arithmetic overflow")
            && finding.materiality.is_none()
            && finding.comparison.is_none()
    }));
    assert!(result
        .comparisons
        .iter()
        .all(|c| c.actual.is_finite() && c.expected.is_finite() && c.tolerance.is_finite()));
    serde_json::to_string(result).expect("all published numbers are finite");
}

#[test]
fn balance_sheet_arithmetic_overflow_fails_without_publishing_nonfinite_numbers() {
    for (assets, liabilities, equity, relative_tolerance) in [
        ([1e308, 1e308], 1e308, 1e308, 1e-9),
        ([1e308, 0.0], -1e308, -1.0, 1e-9),
        ([1e308, 0.0], 5e307, 5e307, 10.0),
    ] {
        let p2 = PeriodId::quarter(2025, 2).unwrap();
        let mut builder = ModelBuilder::new("overflow");
        builder.periods("2025Q1..Q2", None).unwrap();
        for (name, first, second) in [
            ("a1", assets[0], 40.0),
            ("a2", assets[1], 60.0),
            ("l", liabilities, 70.0),
            ("e", equity, 30.0),
        ] {
            builder.value_scalar(name, &[(q1(), first), (p2, second)]);
        }
        let model = builder.build().unwrap();
        let results = Evaluator::new().evaluate(&model).unwrap();
        let mut context = CheckContext::new(&model, &results);
        context.config.default_relative_tolerance = relative_tolerance;
        context.config.materiality_threshold = f64::MAX;
        let check = BalanceSheetArticulation {
            assets_nodes: vec!["a1".into(), "a2".into()],
            liabilities_nodes: vec!["l".into()],
            equity_nodes: vec!["e".into()],
            tolerance: None,
        };
        let result = check.execute(&context).unwrap();
        assert_arithmetic_failure(&result);
        assert_eq!(result.comparisons.len(), 1);
        assert_eq!(result.comparisons[0].period, Some(p2));
        let suite = CheckSuite::builder("overflow")
            .config(context.config.clone())
            .add_check(check)
            .build();
        let report = suite.run(&model, &results).unwrap();
        assert!(report.has_errors());
        assert_eq!(report.summary.failed, 1);
    }
}

#[test]
fn cash_identities_reject_intermediate_overflow() {
    use finstack_quant_statements::checks::builtins::CashReconciliation;
    let p2 = PeriodId::quarter(2025, 2).unwrap();
    let mut builder = ModelBuilder::new("cash overflow");
    builder.periods("2025Q1..Q2", None).unwrap();
    for (name, values) in [
        ("cash", [1e308, 1e308]),
        ("flow", [0.0, 1e308]),
        ("cfo", [0.0, 1e308]),
        ("cfi", [0.0, 1e308]),
        ("cff", [0.0, -1e308]),
    ] {
        builder.value_scalar(name, &[(q1(), values[0]), (p2, values[1])]);
    }
    let model = builder.build().unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = CashReconciliation {
        cash_balance_node: "cash".into(),
        total_cash_flow_node: "flow".into(),
        cfo_node: Some("cfo".into()),
        cfi_node: Some("cfi".into()),
        cff_node: Some("cff".into()),
        tolerance: None,
    };
    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();
    assert_arithmetic_failure(&result);
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count(),
        2
    );
}

#[test]
fn retained_earnings_rejects_intermediate_overflow() {
    use finstack_quant_statements::checks::builtins::RetainedEarningsReconciliation;
    let p2 = PeriodId::quarter(2025, 2).unwrap();
    let mut builder = ModelBuilder::new("retained earnings overflow");
    builder.periods("2025Q1..Q2", None).unwrap();
    for (name, values) in [
        ("re", [1e308, 1e308]),
        ("ni", [0.0, 1e308]),
        ("div", [0.0, 1e308]),
    ] {
        builder.value_scalar(name, &[(q1(), values[0]), (p2, values[1])]);
    }
    let model = builder.build().unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = RetainedEarningsReconciliation {
        retained_earnings_node: "re".into(),
        net_income_node: "ni".into(),
        dividends_node: Some("div".into()),
        other_adjustments: Vec::new(),
        tolerance: None,
        dividends_sign_convention: Default::default(),
    };
    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();
    assert_arithmetic_failure(&result);
    assert!(result.comparisons.is_empty());
}
