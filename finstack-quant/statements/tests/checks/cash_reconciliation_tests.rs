//! Cash roll-forward and component reconciliation (STATEMENTS-CONTRACTS-011).
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::CashReconciliation;
use finstack_quant_statements::checks::{
    Check, CheckContext, CheckOutcome, CheckResult, DatedOpening,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeId};
use time::macros::date;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn opening(value: f64) -> DatedOpening {
    DatedOpening {
        date: date!(2025 - 01 - 01),
        value: AmountOrScalar::scalar(value),
    }
}

/// Two quarters of scalar series; a `None` cell is left without a value.
fn model(series: &[(&str, [Option<f64>; 2])]) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("cash");
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

fn check() -> CashReconciliation {
    CashReconciliation {
        cash_balance_node: NodeId::new("cash"),
        total_cash_flow_node: NodeId::new("total_cf"),
        opening_balance: opening(80.0),
        adjustment_nodes: vec![],
        cfo_node: None,
        cfi_node: None,
        cff_node: None,
        tolerance: None,
    }
}

fn run(model: &FinancialModelSpec, check: &CashReconciliation) -> CheckResult {
    let results = Evaluator::new().evaluate(model).unwrap();
    check.execute(&CheckContext::new(model, &results)).unwrap()
}

/// Every period is reconciled, the first from the explicit dated opening:
/// 80 + 20 = 100 in Q1 and 100 + 50 = 150 in Q2.
#[test]
fn every_period_reconciles_from_the_explicit_opening() {
    let model = model(&[
        ("cash", [Some(100.0), Some(150.0)]),
        ("total_cf", [Some(20.0), Some(50.0)]),
    ]);
    let result = run(&model, &check());
    assert_eq!(result.outcome, CheckOutcome::Pass);
    assert_eq!(result.comparisons.len(), 2);
    let first = &result.comparisons[0];
    assert_eq!(
        (
            first.identity.as_str(),
            first.period,
            first.actual.value(),
            first.expected.value(),
            first.tolerance.value()
        ),
        ("cash_roll_forward", Some(q(1)), 100.0, 100.0, 0.0)
    );
    // A wrong opening fails the first period, which used to be skipped.
    let wrong = CashReconciliation {
        opening_balance: opening(70.0),
        ..check()
    };
    let result = run(&model, &wrong);
    assert_eq!(result.outcome, CheckOutcome::Fail);
    let finding = &result.findings[0];
    assert_eq!(finding.period, Some(q(1)));
    let materiality = finding.materiality.as_ref().unwrap();
    // Residual 10 against the opening of 70, with no floor on the reference.
    assert_eq!(materiality.absolute, 10.0);
    assert_eq!(materiality.reference_value, 70.0);
    assert!((materiality.relative.unwrap() - 1.0 / 7.0).abs() < 1e-12);
    assert_eq!(materiality.reference_label, "opening_cash");
}

/// Signed FX and cash-scope adjustments enter the roll-forward once each:
/// 80 + 20 - 3 + 1 = 98.
#[test]
fn signed_adjustments_enter_the_roll_forward() {
    let model = model(&[
        ("cash", [Some(98.0), Some(148.0)]),
        ("total_cf", [Some(20.0), Some(50.0)]),
        ("fx", [Some(-3.0), Some(0.0)]),
        ("scope", [Some(1.0), Some(0.0)]),
    ]);
    let adjusted = CashReconciliation {
        adjustment_nodes: vec![NodeId::new("fx"), NodeId::new("scope")],
        ..check()
    };
    assert_eq!(run(&model, &adjusted).outcome, CheckOutcome::Pass);
    // Without the adjustments the first period is off by 2.
    assert_eq!(run(&model, &check()).outcome, CheckOutcome::Fail);
    // An adjustment listed twice is rejected.
    let duplicated = CashReconciliation {
        adjustment_nodes: vec![NodeId::new("fx"), NodeId::new("fx")],
        ..check()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let error = duplicated
        .execute(&CheckContext::new(&model, &results))
        .unwrap_err();
    assert!(error.to_string().contains("appears twice"), "{error}");
}

/// The component identity runs for every period, including the first, with
/// `|total cash flow|` as its reference.
#[test]
fn components_reconcile_in_every_period() {
    let components = |cff_q1: f64| {
        model(&[
            ("cash", [Some(100.0), Some(150.0)]),
            ("total_cf", [Some(20.0), Some(50.0)]),
            ("cfo", [Some(30.0), Some(60.0)]),
            ("cfi", [Some(-15.0), Some(-20.0)]),
            ("cff", [Some(cff_q1), Some(10.0)]),
        ])
    };
    let with_components = CashReconciliation {
        cfo_node: Some(NodeId::new("cfo")),
        cfi_node: Some(NodeId::new("cfi")),
        cff_node: Some(NodeId::new("cff")),
        ..check()
    };
    let result = run(&components(5.0), &with_components);
    assert_eq!(result.outcome, CheckOutcome::Pass);
    assert_eq!(result.evaluated, 4);
    assert_eq!(result.comparisons.len(), 4);
    // 30 - 15 + 6 = 21 against a total of 20 in the first period.
    let result = run(&components(6.0), &with_components);
    assert_eq!(result.outcome, CheckOutcome::Fail);
    let finding = &result.findings[0];
    assert_eq!(finding.period, Some(q(1)));
    let comparison = finding.comparison.as_ref().unwrap();
    assert_eq!(comparison.identity, "cash_flow_components");
    assert_eq!(
        (comparison.actual.value(), comparison.expected.value()),
        (20.0, 21.0)
    );
    assert_eq!(
        finding.materiality.as_ref().unwrap().reference_label,
        "total_cash_flow"
    );
}

/// CFO, CFI and CFF are configured together or not at all.
#[test]
fn a_partial_component_configuration_is_rejected() {
    let model = model(&[
        ("cash", [Some(100.0), Some(150.0)]),
        ("total_cf", [Some(20.0), Some(50.0)]),
        ("cfo", [Some(20.0), Some(50.0)]),
    ]);
    let partial = CashReconciliation {
        cfo_node: Some(NodeId::new("cfo")),
        ..check()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let error = partial
        .execute(&CheckContext::new(&model, &results))
        .unwrap_err();
    assert!(error.to_string().contains("all three or none"), "{error}");
    // The same node in two component roles is rejected too.
    let repeated = CashReconciliation {
        cfo_node: Some(NodeId::new("cfo")),
        cfi_node: Some(NodeId::new("cfo")),
        cff_node: Some(NodeId::new("total_cf")),
        ..check()
    };
    assert!(
        repeated
            .execute(&CheckContext::new(&model, &results))
            .is_err()
    );
}

/// A missing input makes the comparison incomplete and names the node; the
/// period is neither skipped nor read as zero.
#[test]
fn a_missing_input_is_incomplete_not_skipped() {
    let model = model(&[
        ("cash", [Some(100.0), Some(150.0)]),
        ("total_cf", [Some(20.0), None]),
    ]);
    let result = run(&model, &check());
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    assert_eq!(result.comparisons.len(), 1);
    let finding = &result.findings[0];
    assert!(finding.coverage);
    assert_eq!(finding.period, Some(q(2)));
    assert_eq!(finding.nodes, [NodeId::new("total_cf")]);
    // A missing prior close leaves the next period without an opening.
    let no_close = model_without_first_close();
    let result = run(&no_close, &check());
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    let periods: Vec<_> = result.findings.iter().map(|f| f.period).collect();
    assert_eq!(periods, [Some(q(1)), Some(q(2))]);
}

fn model_without_first_close() -> FinancialModelSpec {
    model(&[
        ("cash", [None, Some(150.0)]),
        ("total_cf", [Some(20.0), Some(50.0)]),
    ])
}

/// The opening is dated at the first period boundary, in the cash units.
#[test]
fn the_opening_is_dated_and_typed() {
    let model = model(&[
        ("cash", [Some(100.0), Some(150.0)]),
        ("total_cf", [Some(20.0), Some(50.0)]),
    ]);
    let results = Evaluator::new().evaluate(&model).unwrap();
    let context = CheckContext::new(&model, &results);
    let misdated = CashReconciliation {
        opening_balance: DatedOpening {
            date: date!(2024 - 12 - 31),
            value: AmountOrScalar::scalar(80.0),
        },
        ..check()
    };
    let error = misdated.execute(&context).unwrap_err();
    assert!(error.to_string().contains("first period starts"), "{error}");
    let monetary = CashReconciliation {
        opening_balance: DatedOpening {
            date: date!(2025 - 01 - 01),
            value: AmountOrScalar::amount(80.0, finstack_quant_core::currency::Currency::USD)
                .unwrap(),
        },
        ..check()
    };
    let error = monetary.execute(&context).unwrap_err();
    assert!(
        error.to_string().contains("mixes monetary and scalar"),
        "{error}"
    );
}

/// Monetary cash is reconciled exactly: 0.10 + 0.20 is 0.30 in money, where
/// binary floats give 0.30000000000000004.
#[test]
fn monetary_cash_is_reconciled_exactly() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    let usd = |amount: f64| Money::new(amount, Currency::USD).unwrap();
    let mut builder = ModelBuilder::new("money");
    builder.periods("2025Q1..Q1", None).unwrap();
    builder.value_money("cash", &[(q(1), usd(0.30))]);
    builder.value_money("total_cf", &[(q(1), usd(0.20))]);
    let model = builder.build().unwrap();
    let exact = CashReconciliation {
        opening_balance: DatedOpening {
            date: date!(2025 - 01 - 01),
            value: AmountOrScalar::amount(0.10, Currency::USD).unwrap(),
        },
        ..check()
    };
    let result = run(&model, &exact);
    assert_eq!(result.outcome, CheckOutcome::Pass, "{:?}", result.findings);
}
