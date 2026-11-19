//! Retained-earnings reconciliation (STATEMENTS-CONTRACTS-012).
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::RetainedEarningsReconciliation;
use finstack_quant_statements::checks::{
    Check, CheckContext, CheckOutcome, CheckResult, DatedOpening, SignConventionPolicy,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeId};
use time::macros::date;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn model(series: &[(&str, [Option<f64>; 2])]) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("retained earnings");
    builder.periods("2025Q1..Q2", None).unwrap();
    for (node, values) in series {
        let cells: Vec<(PeriodId, f64)> = values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.map(|v| (q(index as u8 + 1), v)))
            .collect();
        builder.value_scalar(*node, &cells);
    }
    builder.build().unwrap()
}

fn check() -> RetainedEarningsReconciliation {
    RetainedEarningsReconciliation {
        retained_earnings_node: NodeId::new("re"),
        net_income_node: NodeId::new("ni"),
        opening_balance: DatedOpening {
            date: date!(2025 - 01 - 01),
            value: AmountOrScalar::scalar(500.0),
        },
        dividends_node: Some(NodeId::new("div")),
        other_adjustments: vec![],
        tolerance: None,
        dividends_sign_convention: SignConventionPolicy::MagnitudePositive,
    }
}

fn run(
    model: &FinancialModelSpec,
    check: &RetainedEarningsReconciliation,
) -> finstack_quant_statements::Result<CheckResult> {
    let results = Evaluator::new().evaluate(model).unwrap();
    check.execute(&CheckContext::new(model, &results))
}

/// Every period reconciles, the first from the explicit opening:
/// 500 + 60 - 10 = 550, then 550 + 80 - 30 = 600.
#[test]
fn every_period_reconciles_from_the_explicit_opening() {
    let model = model(&[
        ("re", [Some(550.0), Some(600.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
        ("div", [Some(10.0), Some(30.0)]),
    ]);
    let result = run(&model, &check()).unwrap();
    assert_eq!(result.outcome, CheckOutcome::Pass);
    assert_eq!(result.comparisons.len(), 2);
    assert_eq!(result.comparisons[0].period, Some(q(1)));
    assert_eq!(
        result.comparisons[0].identity,
        "retained_earnings_roll_forward"
    );
    // A wrong opening fails the first period; the reference is |opening|.
    let wrong = RetainedEarningsReconciliation {
        opening_balance: DatedOpening {
            date: date!(2025 - 01 - 01),
            value: AmountOrScalar::scalar(400.0),
        },
        ..check()
    };
    let result = run(&model, &wrong).unwrap();
    assert_eq!(result.outcome, CheckOutcome::Fail);
    let materiality = result.findings[0].materiality.as_ref().unwrap();
    assert_eq!(
        (
            materiality.absolute,
            materiality.reference_value,
            materiality.relative_pct
        ),
        (100.0, 400.0, Some(25.0))
    );
}

/// A dividend magnitude is subtracted and cannot be negative; a signed
/// dividend under the explicitly selected inflow convention is added.
#[test]
fn dividend_signs_follow_the_selected_convention() {
    let negative = model(&[
        ("re", [Some(550.0), Some(600.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
        ("div", [Some(-10.0), Some(-30.0)]),
    ]);
    let error = run(&negative, &check()).unwrap_err();
    assert!(error.to_string().contains("cannot be negative"), "{error}");
    let signed = RetainedEarningsReconciliation {
        dividends_sign_convention: SignConventionPolicy::InflowPositive,
        ..check()
    };
    assert_eq!(run(&negative, &signed).unwrap().outcome, CheckOutcome::Pass);
    // A typed zero is a known zero dividend.
    let none_paid = model(&[
        ("re", [Some(560.0), Some(640.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
        ("div", [Some(0.0), Some(0.0)]),
    ]);
    assert_eq!(
        run(&none_paid, &check()).unwrap().outcome,
        CheckOutcome::Pass
    );
}

/// Direct entries are signed, distinct and included once.
#[test]
fn direct_adjustments_are_signed_and_distinct() {
    let model = model(&[
        ("re", [Some(545.0), Some(602.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
        ("div", [Some(10.0), Some(30.0)]),
        ("buyback", [Some(-5.0), Some(0.0)]),
        ("restatement", [Some(0.0), Some(7.0)]),
    ]);
    let adjusted = RetainedEarningsReconciliation {
        other_adjustments: vec![NodeId::new("buyback"), NodeId::new("restatement")],
        ..check()
    };
    assert_eq!(run(&model, &adjusted).unwrap().outcome, CheckOutcome::Pass);
    assert_eq!(run(&model, &check()).unwrap().outcome, CheckOutcome::Fail);
    let duplicated = RetainedEarningsReconciliation {
        other_adjustments: vec![NodeId::new("buyback"), NodeId::new("buyback")],
        ..check()
    };
    let error = run(&model, &duplicated).unwrap_err();
    assert!(error.to_string().contains("appears twice"), "{error}");
    // A movement that is also the dividend series would be counted twice.
    let overlapping = RetainedEarningsReconciliation {
        other_adjustments: vec![NodeId::new("div")],
        ..check()
    };
    assert!(run(&model, &overlapping).is_err());
}

/// A configured series without a value makes the period incomplete; it is
/// not read as zero.
#[test]
fn a_configured_missing_amount_is_incomplete() {
    let model = model(&[
        ("re", [Some(550.0), Some(630.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
        ("div", [Some(10.0), None]),
    ]);
    let result = run(&model, &check()).unwrap();
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    let finding = &result.findings[0];
    assert!(finding.coverage);
    assert_eq!(finding.period, Some(q(2)));
    assert_eq!(finding.nodes, [NodeId::new("div")]);
    // An unknown node is a configuration error, not a missing amount.
    let unknown = RetainedEarningsReconciliation {
        dividends_node: Some(NodeId::new("no_such_node")),
        ..check()
    };
    let error = run(&model, &unknown).unwrap_err();
    assert!(
        error.to_string().contains("not a node of the model"),
        "{error}"
    );
    // Omitting an inapplicable series is fine: 500 + 60 = 560.
    let no_dividends = RetainedEarningsReconciliation {
        dividends_node: None,
        ..check()
    };
    let model = self::model(&[
        ("re", [Some(560.0), Some(640.0)]),
        ("ni", [Some(60.0), Some(80.0)]),
    ]);
    assert_eq!(
        run(&model, &no_dividends).unwrap().outcome,
        CheckOutcome::Pass
    );
}
