//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, FormulaCheckSpec, Severity,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

// Basic passing formula

#[test]
fn revenue_positive_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("revenue", &[(q(1), s(100.0)), (q(2), s(200.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "revenue_positive".into(),
        name: "Revenue must be positive".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "revenue > 0".into(),
        message_template: "Revenue was non-positive in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

// Failing formula produces finding

#[test]
fn failing_formula_produces_finding() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), s(-50.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "revenue_positive".into(),
        name: "Revenue must be positive".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "revenue > 0".into(),
        message_template: "Revenue was non-positive in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Error);
    assert!(result.findings[0].message.contains("non-positive"));
    assert_eq!(result.findings[0].period, Some(q(1)));
}

// Warning severity does not fail

#[test]
fn warning_severity_does_not_fail() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("margin", &[(q(1), s(0.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "margin_nonzero".into(),
        name: "Margin should not be zero".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Warning,
        formula: "margin > 0".into(),
        message_template: "Margin is zero in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed); // warning-only, still passes
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
}

// Arithmetic formula

#[test]
fn arithmetic_formula_evaluates() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("assets", &[(q(1), s(100.0))])
        .value("liabilities", &[(q(1), s(60.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // assets - liabilities = 40 > 0  → result is 1.0 (truthy), passes.
    let check = FormulaCheckSpec {
        id: "equity_positive".into(),
        name: "Equity positive".into(),
        category: CheckCategory::AccountingIdentity,
        severity: Severity::Error,
        formula: "assets > liabilities".into(),
        message_template: "Equity negative in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

// JSON deserialization

#[test]
fn json_deserialization_works() {
    let json = r#"{
        "id": "test_check",
        "name": "Test Check",
        "category": "data_quality",
        "severity": "warning",
        "formula": "revenue > 0",
        "message_template": "Revenue bad in {period}",
        "tolerance": 0.01
    }"#;

    let check: FormulaCheckSpec = serde_json::from_str(json).unwrap();

    assert_eq!(check.id, "test_check");
    assert_eq!(check.category, CheckCategory::DataQuality);
    assert_eq!(check.severity, Severity::Warning);
    assert_eq!(check.tolerance, Some(0.01));
}

// Missing nodes are explicit evaluation errors

#[test]
fn missing_node_returns_error() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "check_missing".into(),
        name: "Missing node".into(),
        category: CheckCategory::DataQuality,
        severity: Severity::Error,
        formula: "nonexistent > 0".into(),
        message_template: "Missing in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let error = check.execute(&ctx).unwrap_err();

    assert!(error.to_string().contains("nonexistent"));
}

// Complex nested expression with parentheses

#[test]
fn nested_expression_gross_margin() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), s(1000.0))])
        .value("cogs", &[(q(1), s(700.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // (1000 - 700) / 1000 = 0.30 >= 0.20 → pass
    let check = FormulaCheckSpec {
        id: "gross_margin_floor".into(),
        name: "Gross margin >= 20%".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "(revenue - cogs) / revenue >= 0.20".into(),
        message_template: "Gross margin below 20% in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn nested_expression_gross_margin_fails() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), s(1000.0))])
        .value("cogs", &[(q(1), s(850.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // (1000 - 850) / 1000 = 0.15 < 0.20 → fail
    let check = FormulaCheckSpec {
        id: "gross_margin_floor".into(),
        name: "Gross margin >= 20%".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "(revenue - cogs) / revenue >= 0.20".into(),
        message_template: "Gross margin below 20% in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
}

// abs() function

#[test]
fn abs_function_in_formula() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), s(1000.0))])
        .value("total_liabilities", &[(q(1), s(600.0))])
        .value("total_equity", &[(q(1), s(400.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // abs(1000 - 600 - 400) = 0.0 < 0.01 → 1.0 (truthy) → pass
    let check = FormulaCheckSpec {
        id: "bs_identity".into(),
        name: "BS identity via abs".into(),
        category: CheckCategory::AccountingIdentity,
        severity: Severity::Error,
        formula: "abs(total_assets - total_liabilities - total_equity) < 0.01".into(),
        message_template: "BS does not balance in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
}

#[test]
fn abs_function_detects_imbalance() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), s(1000.0))])
        .value("total_liabilities", &[(q(1), s(600.0))])
        .value("total_equity", &[(q(1), s(300.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // abs(1000 - 600 - 300) = 100.0, NOT < 0.01 → 0.0 (falsy) → fail
    let check = FormulaCheckSpec {
        id: "bs_identity".into(),
        name: "BS identity via abs".into(),
        category: CheckCategory::AccountingIdentity,
        severity: Severity::Error,
        formula: "abs(total_assets - total_liabilities - total_equity) < 0.01".into(),
        message_template: "BS does not balance in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
}

// Multi-operator expressions

#[test]
fn multi_operator_addition() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("a", &[(q(1), s(10.0))])
        .value("b", &[(q(1), s(20.0))])
        .value("c", &[(q(1), s(30.0))])
        .value("total", &[(q(1), s(60.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // a + b + c == total → 60 == 60 → 1.0 → pass
    let check = FormulaCheckSpec {
        id: "sum_check".into(),
        name: "Sum matches total".into(),
        category: CheckCategory::AccountingIdentity,
        severity: Severity::Error,
        formula: "a + b + c == total".into(),
        message_template: "Sum mismatch in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
}

// max() / min() functions

#[test]
fn max_min_functions() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("x", &[(q(1), s(5.0))])
        .value("y", &[(q(1), s(10.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // max(5, 10) == 10 → pass
    let check = FormulaCheckSpec {
        id: "max_check".into(),
        name: "Max check".into(),
        category: CheckCategory::DataQuality,
        severity: Severity::Error,
        formula: "max(x, y) == 10".into(),
        message_template: "Max failed in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();
    assert!(result.passed);

    // min(5, 10) == 5 → pass
    let check_min = FormulaCheckSpec {
        id: "min_check".into(),
        name: "Min check".into(),
        category: CheckCategory::DataQuality,
        severity: Severity::Error,
        formula: "min(x, y) == 5".into(),
        message_template: "Min failed in {period}".into(),
        tolerance: None,
    };

    let result_min = check_min.execute(&ctx).unwrap();
    assert!(result_min.passed);
}

// if() conditional

#[test]
fn if_conditional_formula() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), s(1000.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // if(revenue > 500, 1, 0) → 1.0 → pass
    let check = FormulaCheckSpec {
        id: "cond_check".into(),
        name: "Conditional check".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "if(revenue > 500, 1, 0)".into(),
        message_template: "Condition failed in {period}".into(),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();
    assert!(result.passed);
}

#[test]
fn canonical_variadic_function_is_supported() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("a", &[(q(1), s(10.0))])
        .value("b", &[(q(1), s(20.0))])
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = FormulaCheckSpec {
        id: "sum_check".into(),
        name: "Canonical sum".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "sum(a, b) == 30".into(),
        message_template: "Sum failed in {period}".into(),
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed);
}

#[test]
fn canonical_time_series_function_uses_evaluated_history() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("revenue", &[(q(1), s(100.0)), (q(2), s(110.0))])
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = FormulaCheckSpec {
        id: "lag_check".into(),
        name: "Canonical lag".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "if(revenue == 100, 1, lag(revenue, 1) == 100)".into(),
        message_template: "Lag failed in {period}".into(),
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed);
}

#[test]
fn capital_structure_reference_uses_result_cashflows() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .build()
        .unwrap();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::new(25.0, Currency::USD).expect("valid money fixture");
    let mut cashflows = CapitalStructureCashflows::new();
    cashflows.totals.insert(q(1), breakdown);
    cashflows.reporting_currency = Some(Currency::USD);
    results.cs_cashflows = Some(cashflows);

    let check = FormulaCheckSpec {
        id: "interest_check".into(),
        name: "Canonical capital structure reference".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "cs.interest_expense.total == 25".into(),
        message_template: "Interest failed in {period}".into(),
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed);
}
