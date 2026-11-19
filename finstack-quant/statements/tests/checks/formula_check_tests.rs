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
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(100.0)), (q(2), s(200.0))]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "revenue_positive".into(),
        name: "Revenue must be positive".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "revenue > 0".into(),
        message_template: "Revenue was non-positive in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
    assert!(result.findings.is_empty());
}

// Failing formula produces finding

#[test]
fn failing_formula_produces_finding() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(-50.0))]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "revenue_positive".into(),
        name: "Revenue must be positive".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "revenue > 0".into(),
        message_template: "Revenue was non-positive in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed());
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Error);
    assert!(result.findings[0].message.contains("non-positive"));
    assert_eq!(result.findings[0].period, Some(q(1)));
}

// Warning severity does not fail

#[test]
fn warning_severity_does_not_fail() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("margin", &[(q(1), s(0.0))]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "margin_nonzero".into(),
        name: "Margin should not be zero".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Warning,
        formula: "margin > 0".into(),
        message_template: "Margin is zero in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed()); // warning-only, still passes
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
}

// Arithmetic formula

#[test]
fn arithmetic_formula_evaluates() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("assets", &[(q(1), s(100.0))]);
        model_builder.value("liabilities", &[(q(1), s(60.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
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
        "mode": "residual",
        "scope": "all_periods",
        "formula": "revenue - 100",
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
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value_scalar("placeholder", &[]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FormulaCheckSpec {
        id: "check_missing".into(),
        name: "Missing node".into(),
        category: CheckCategory::DataQuality,
        severity: Severity::Error,
        formula: "nonexistent > 0".into(),
        message_template: "Missing in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let error = check.execute(&ctx).unwrap_err();

    assert!(error.to_string().contains("nonexistent"));

    // A reference read only through `lag` is validated the same way.
    let lagged = FormulaCheckSpec {
        formula: "placeholder - lag(nonexistent, 1) > 0".into(),
        ..check
    };
    let error = lagged.execute(&ctx).unwrap_err();
    assert!(error.to_string().contains("nonexistent"), "{error}");
}

// Complex nested expression with parentheses

#[test]
fn nested_expression_gross_margin() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(1000.0))]);
        model_builder.value("cogs", &[(q(1), s(700.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
    assert!(result.findings.is_empty());
}

#[test]
fn nested_expression_gross_margin_fails() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(1000.0))]);
        model_builder.value("cogs", &[(q(1), s(850.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed());
    assert_eq!(result.findings.len(), 1);
}

// abs() function

#[test]
fn abs_function_in_formula() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("total_assets", &[(q(1), s(1000.0))]);
        model_builder.value("total_liabilities", &[(q(1), s(600.0))]);
        model_builder.value("total_equity", &[(q(1), s(400.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
}

#[test]
fn abs_function_detects_imbalance() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("total_assets", &[(q(1), s(1000.0))]);
        model_builder.value("total_liabilities", &[(q(1), s(600.0))]);
        model_builder.value("total_equity", &[(q(1), s(300.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed());
    assert_eq!(result.findings.len(), 1);
}

// Multi-operator expressions

#[test]
fn multi_operator_addition() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("a", &[(q(1), s(10.0))]);
        model_builder.value("b", &[(q(1), s(20.0))]);
        model_builder.value("c", &[(q(1), s(30.0))]);
        model_builder.value("total", &[(q(1), s(60.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
}

// max() / min() functions

#[test]
fn max_min_functions() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("x", &[(q(1), s(5.0))]);
        model_builder.value("y", &[(q(1), s(10.0))]);
        model_builder.build().unwrap()
    };

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
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();
    assert!(result.passed());

    // min(5, 10) == 5 → pass
    let check_min = FormulaCheckSpec {
        id: "min_check".into(),
        name: "Min check".into(),
        category: CheckCategory::DataQuality,
        severity: Severity::Error,
        formula: "min(x, y) == 5".into(),
        message_template: "Min failed in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let result_min = check_min.execute(&ctx).unwrap();
    assert!(result_min.passed());
}

// if() conditional

#[test]
fn if_conditional_formula() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(1000.0))]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    // A predicate is Boolean: the branches are comparisons, not 1 and 0.
    let check = FormulaCheckSpec {
        id: "cond_check".into(),
        name: "Conditional check".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "if(revenue > 500, revenue > 0, revenue < 0)".into(),
        message_template: "Condition failed in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();
    assert!(result.passed());
}

#[test]
fn canonical_variadic_function_is_supported() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("a", &[(q(1), s(10.0))]);
        model_builder.value("b", &[(q(1), s(20.0))]);
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = FormulaCheckSpec {
        id: "sum_check".into(),
        name: "Canonical sum".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "sum(a, b) == 30".into(),
        message_template: "Sum failed in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed());
}

#[test]
fn canonical_time_series_function_uses_evaluated_history() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(100.0)), (q(2), s(110.0))]);
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let check = FormulaCheckSpec {
        id: "lag_check".into(),
        name: "Canonical lag".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: "revenue == 100 or lag(revenue, 1) == 100".into(),
        message_template: "Lag failed in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed());
}

#[test]
fn capital_structure_reference_uses_result_cashflows() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "expected_interest",
            &[(q(1), AmountOrScalar::amount(25.0, Currency::USD).unwrap())],
        );
        model_builder.build().unwrap()
    };
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
        formula: "cs.interest_expense.total == expected_interest".into(),
        message_template: "Interest failed in {period}".into(),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    };

    let result = check.execute(&CheckContext::new(&model, &results)).unwrap();

    assert!(result.passed());
}

fn predicate_check(formula: &str, tolerance: Option<f64>) -> FormulaCheckSpec {
    FormulaCheckSpec {
        id: "predicate".into(),
        name: "Predicate".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        formula: formula.into(),
        message_template: "Predicate failed in {period}".into(),
        mode: if tolerance.is_some() {
            finstack_quant_statements::checks::FormulaCheckMode::Residual
        } else {
            finstack_quant_statements::checks::FormulaCheckMode::Predicate
        },
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance,
    }
}

fn insufficient_periods(result: &finstack_quant_statements::checks::CheckResult) -> Vec<PeriodId> {
    result
        .findings
        .iter()
        .filter(|finding| finding.coverage)
        .filter_map(|finding| finding.period)
        .collect()
}

// Non-finite and missing inputs are insufficient data, not failures

#[test]
fn non_finite_input_reports_insufficient_data() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("leverage", &[(q(1), s(4.0)), (q(2), s(5.0))]);
        model_builder.build().unwrap()
    };
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.set_cell(
        "leverage",
        q(2),
        finstack_quant_statements::evaluator::ResultCell::unavailable(
            finstack_quant_statements::evaluator::UnavailableReason::MissingInput,
        ),
    );

    for tolerance in [None, Some(0.5)] {
        let formula = if tolerance.is_some() {
            "leverage - 4"
        } else {
            "leverage < 6"
        };
        let result = predicate_check(formula, tolerance)
            .execute(&CheckContext::new(&model, &results))
            .unwrap();

        // A period that could not be judged makes the check incomplete.
        assert_eq!(
            result.outcome,
            finstack_quant_statements::checks::CheckOutcome::Incomplete,
            "{formula}: {:?}",
            result.findings
        );
        assert_eq!(result.findings.len(), 1, "{formula}: {:?}", result.findings);
        assert!(result.findings[0].coverage);
        let finding = &result.findings[0];
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.period, Some(q(2)));
        assert!(
            finding.message.contains("incomplete"),
            "{}",
            finding.message
        );
        assert!(finding.comparison.is_none());
        // Only the judged period is recorded as a comparison.
        assert_eq!(result.comparisons.len(), usize::from(tolerance.is_some()));
    }
}

#[test]
fn missing_input_cell_reports_insufficient_data() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(100.0)), (q(2), s(110.0))]);
        model_builder.build().unwrap()
    };
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results
        .cells
        .get_mut("revenue")
        .unwrap()
        .shift_remove(&q(2));

    let result = predicate_check("revenue > 0", None)
        .execute(&CheckContext::new(&model, &results))
        .unwrap();

    assert_eq!(
        result.outcome,
        finstack_quant_statements::checks::CheckOutcome::Incomplete
    );
    assert_eq!(insufficient_periods(&result), vec![q(2)]);
}

#[test]
fn lag_before_first_period_reports_insufficient_data() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q3", None).unwrap();
        model_builder.value(
            "revenue",
            &[(q(1), s(100.0)), (q(2), s(110.0)), (q(3), s(500.0))],
        );
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();

    let result = predicate_check("revenue / lag(revenue, 1) - 1 < 0.5", None)
        .execute(&CheckContext::new(&model, &results))
        .unwrap();

    // Q1 has no prior quarter, Q2 grows 10% and passes, Q3 grows 355% and fails.
    assert!(!result.passed());
    assert_eq!(insufficient_periods(&result), vec![q(1)]);
    let failures: Vec<_> = result
        .findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].period, Some(q(3)));
}

#[test]
fn division_by_zero_over_finite_inputs_is_an_execution_error() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("debt", &[(q(1), s(300.0))]);
        model_builder.value("ebitda", &[(q(1), s(0.0))]);
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();

    let error = predicate_check("debt / ebitda < 6", None)
        .execute(&CheckContext::new(&model, &results))
        .expect_err("a division by zero is not a finding");

    assert!(error.to_string().contains("division by zero"), "{error}");
}

// Explicit modes, eligibility, scope and lazy evaluation (STATEMENTS-CONTRACTS-015)

use finstack_quant_statements::checks::{CheckOutcome, FormulaCheckMode, PeriodScope};

/// Two quarters (the first actual); `primary` has no cell in the second.
fn sparse_model() -> (
    finstack_quant_statements::types::FinancialModelSpec,
    finstack_quant_statements::evaluator::StatementResult,
) {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        model_builder.value("primary", &[(q(1), s(10.0)), (q(2), s(20.0))]);
        model_builder.value("backup", &[(q(1), s(1.0)), (q(2), s(2.0))]);
        model_builder.value("flag", &[(q(1), s(1.0)), (q(2), s(0.0))]);
        model_builder.build().unwrap()
    };
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results
        .cells
        .get_mut("primary")
        .unwrap()
        .shift_remove(&q(2));
    (model, results)
}

fn spec(mode: FormulaCheckMode, formula: &str) -> FormulaCheckSpec {
    FormulaCheckSpec {
        id: "check".into(),
        name: "Check".into(),
        category: CheckCategory::InternalConsistency,
        severity: Severity::Error,
        mode,
        scope: PeriodScope::AllPeriods,
        formula: formula.into(),
        eligibility: None,
        message_template: "Breach in {period}".into(),
        tolerance: None,
    }
}

#[test]
fn coalesce_over_a_missing_cell_uses_the_backup() {
    let (model, results) = sparse_model();
    let result = spec(FormulaCheckMode::Predicate, "coalesce(primary, backup) > 0")
        .execute(&CheckContext::new(&model, &results))
        .unwrap();
    assert_eq!(result.outcome, CheckOutcome::Pass, "{:?}", result.findings);
    assert_eq!(result.evaluated, 2);
}

#[test]
fn an_untaken_branch_does_not_consume_its_missing_input() {
    let (model, results) = sparse_model();
    // In Q2 the condition is false, so the missing `primary` is never read.
    let lazy = spec(
        FormulaCheckMode::Predicate,
        "if(flag > 0, primary > 0, backup > 0)",
    )
    .execute(&CheckContext::new(&model, &results))
    .unwrap();
    assert_eq!(lazy.outcome, CheckOutcome::Pass, "{:?}", lazy.findings);

    // The taken branch does consume it.
    let consumed = spec(
        FormulaCheckMode::Predicate,
        "if(flag > 0, backup > 0, primary > 0)",
    )
    .execute(&CheckContext::new(&model, &results))
    .unwrap();
    assert_eq!(consumed.outcome, CheckOutcome::Incomplete);
    assert_eq!(insufficient_periods(&consumed), vec![q(2)]);
}

#[test]
fn an_undefined_domain_is_an_execution_error() {
    let (model, results) = sparse_model();
    let error = spec(FormulaCheckMode::Residual, "sqrt(0 - backup)")
        .execute(&CheckContext::new(&model, &results))
        .expect_err("the square root of a negative number is not a finding");
    assert!(error.to_string().contains("sqrt"), "{error}");
}

#[test]
fn eligibility_selects_periods_and_an_unavailable_condition_is_incomplete() {
    let (model, results) = sparse_model();

    // False in Q2: the breach there is out of scope.
    let mut ineligible = spec(FormulaCheckMode::Predicate, "backup < 2");
    ineligible.eligibility = Some("flag > 0".into());
    let result = ineligible
        .execute(&CheckContext::new(&model, &results))
        .unwrap();
    assert_eq!(result.outcome, CheckOutcome::Pass, "{:?}", result.findings);
    assert_eq!(result.evaluated, 1);

    // No value in Q2: the period cannot be placed in or out of scope.
    let mut unknown = spec(FormulaCheckMode::Predicate, "backup > 0");
    unknown.eligibility = Some("primary > 0".into());
    let result = unknown
        .execute(&CheckContext::new(&model, &results))
        .unwrap();
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    assert_eq!(insufficient_periods(&result), vec![q(2)]);
    assert_eq!(result.evaluated, 2);

    // No eligible period at all leaves the check unevaluated.
    let mut never = spec(FormulaCheckMode::Predicate, "backup > 0");
    never.eligibility = Some("backup < 0".into());
    let result = never.execute(&CheckContext::new(&model, &results)).unwrap();
    assert_eq!(result.outcome, CheckOutcome::Unevaluated);

    let mut scalar = spec(FormulaCheckMode::Predicate, "backup > 0");
    scalar.eligibility = Some("flag".into());
    let error = scalar
        .execute(&CheckContext::new(&model, &results))
        .expect_err("a scalar eligibility condition");
    assert!(error.to_string().contains("eligibility"), "{error}");
}

#[test]
fn scope_selects_actual_or_forecast_periods() {
    let (model, results) = sparse_model();
    for (scope, breaches) in [
        (PeriodScope::AllPeriods, vec![q(1), q(2)]),
        (PeriodScope::ActualsOnly, vec![q(1)]),
        (PeriodScope::ForecastOnly, vec![q(2)]),
    ] {
        let mut check = spec(FormulaCheckMode::Predicate, "backup < 0");
        check.scope = scope;
        let result = check.execute(&CheckContext::new(&model, &results)).unwrap();
        let periods: Vec<_> = result.findings.iter().filter_map(|f| f.period).collect();
        assert_eq!(periods, breaches, "{scope:?}");
        assert_eq!(result.evaluated, breaches.len());
    }
}

#[test]
fn mode_and_tolerance_are_validated_against_the_formula() {
    let (model, results) = sparse_model();
    let context = CheckContext::new(&model, &results);
    let rejected = |check: FormulaCheckSpec, needle: &str| {
        let error = check.execute(&context).expect_err(needle).to_string();
        assert!(error.contains(needle), "{error}");
    };

    rejected(spec(FormulaCheckMode::Predicate, "backup - 1"), "predicate");
    rejected(spec(FormulaCheckMode::Residual, "backup > 0"), "residual");
    let mut tolerant = spec(FormulaCheckMode::Predicate, "backup > 0");
    tolerant.tolerance = Some(0.1);
    rejected(tolerant, "tolerance");
    for bad in [-0.1, f64::NAN, f64::INFINITY] {
        let mut residual = spec(FormulaCheckMode::Residual, "backup - 1");
        residual.tolerance = Some(bad);
        rejected(residual, "tolerance");
    }
    rejected(
        spec(FormulaCheckMode::Predicate, "unknown_node > 0"),
        "unknown_node",
    );

    // A residual without a tolerance is an exact comparison with zero.
    let exact = spec(FormulaCheckMode::Residual, "backup - 1")
        .execute(&context)
        .unwrap();
    assert_eq!(exact.outcome, CheckOutcome::Fail);
    assert_eq!(exact.findings.len(), 1);
    assert_eq!(exact.findings[0].period, Some(q(2)));
    assert_eq!(exact.comparisons.len(), 2);
}

#[test]
fn an_invalid_consumed_input_and_an_overflow_are_execution_errors() {
    let (model, mut results) = sparse_model();
    let huge = spec(FormulaCheckMode::Residual, "backup * 1e308 * 1e308")
        .execute(&CheckContext::new(&model, &results));
    assert!(huge.is_err(), "an overflow is not a finding: {huge:?}");

    // A NaN in a value cell is an invalid input, not a missing one.
    results
        .cells
        .get_mut("backup")
        .unwrap()
        .get_mut(&q(1))
        .unwrap()
        .value = Some(s(f64::NAN));
    for (mode, formula) in [
        (FormulaCheckMode::Predicate, "backup > 0"),
        (FormulaCheckMode::Residual, "backup - 1"),
    ] {
        let outcome = spec(mode, formula).execute(&CheckContext::new(&model, &results));
        assert!(outcome.is_err(), "{formula}: {outcome:?}");
    }
    // An advisory severity does not turn the error into a finding.
    let mut advisory = spec(FormulaCheckMode::Predicate, "backup > 0");
    advisory.severity = Severity::Info;
    assert!(
        advisory
            .execute(&CheckContext::new(&model, &results))
            .is_err()
    );
}
