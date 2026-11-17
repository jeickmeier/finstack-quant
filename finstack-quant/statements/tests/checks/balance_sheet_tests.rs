//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

#[test]
fn balanced_passes() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(1000.0)),
                (q(2), AmountOrScalar::scalar(1100.0)),
            ],
        );
        model_builder.value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(600.0)),
                (q(2), AmountOrScalar::scalar(700.0)),
            ],
        );
        model_builder.value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(400.0)),
                (q(2), AmountOrScalar::scalar(400.0)),
            ],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
    assert!(result.findings.is_empty());

    // A passing check still reports what it compared in every period.
    assert_eq!(result.comparisons.len(), 2);
    let q1 = &result.comparisons[0];
    assert_eq!(q1.identity, "balance_sheet_articulation");
    assert_eq!(q1.period, Some(q(1)));
    assert_eq!(q1.actual, 1000.0);
    assert_eq!(q1.expected, 1000.0);
    // No tolerance is stated, so the identity must hold exactly.
    assert_eq!(q1.tolerance, 0.0);
    assert!(result.comparisons.iter().all(|c| c.within_tolerance()));
}

#[test]
fn imbalanced_fails_with_materiality() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(1000.0)),
                (q(2), AmountOrScalar::scalar(1100.0)),
            ],
        );
        model_builder.value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(600.0)),
                (q(2), AmountOrScalar::scalar(700.0)),
            ],
        );
        model_builder.value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(300.0)), // imbalance: 1000 - (600+300) = 100
                (q(2), AmountOrScalar::scalar(400.0)), // balanced
            ],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed());

    let q1_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.period == Some(q(1)))
        .collect();
    assert_eq!(q1_findings.len(), 1);

    let mat = q1_findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 100.0).abs() < 0.01);
    assert!((mat.relative_pct - 10.0).abs() < 0.01); // 100/1000 * 100 = 10%
    assert!((mat.reference_value - 1000.0).abs() < 0.01);
    assert_eq!(mat.reference_label, "total_assets");

    // The finding carries the numbers behind the verdict, and they reconcile
    // with the reported materiality: |actual - expected| == absolute > tolerance.
    let comparison = q1_findings[0].comparison.as_ref().unwrap();
    assert_eq!(comparison.period, Some(q(1)));
    assert_eq!(comparison.actual, 1000.0);
    assert_eq!(comparison.expected, 900.0);
    assert_eq!(comparison.tolerance, 0.0);
    assert_eq!(comparison.difference().abs(), mat.absolute);
    assert!(!comparison.within_tolerance());

    // Both periods are listed on the result: Q1 failed, Q2 passed.
    assert_eq!(result.comparisons.len(), 2);
    assert_eq!(&result.comparisons[0], comparison);
    assert_eq!(result.comparisons[1].period, Some(q(2)));
    assert_eq!(result.comparisons[1].actual, 1100.0);
    assert_eq!(result.comparisons[1].expected, 1100.0);
    assert!(result.comparisons[1].within_tolerance());

    // Old reports (no comparison fields) still deserialize.
    let mut json = serde_json::to_value(&result).unwrap();
    let object = json.as_object_mut().unwrap();
    object.remove("comparisons");
    for finding in object["findings"].as_array_mut().unwrap() {
        finding.as_object_mut().unwrap().remove("comparison");
    }
    let legacy: finstack_quant_statements::checks::CheckResult =
        serde_json::from_value(json).unwrap();
    assert!(legacy.comparisons.is_empty());
    assert!(legacy.findings.iter().all(|f| f.comparison.is_none()));
}

#[test]
fn within_tolerance_passes() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(100.0)),
            ],
        );
        model_builder.value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(60.0)),
                (q(2), AmountOrScalar::scalar(60.0)),
            ],
        );
        model_builder.value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(39.995)), // imbalance = 0.005
                (q(2), AmountOrScalar::scalar(39.995)),
            ],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: Some(0.01), // 0.005 < 0.01 => passes
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed());
    assert!(result.findings.is_empty());
}

// Fail-open guards: the identity must never pass because its operands are
// absent (missing nodes sum to zero on both sides → imbalance 0 → "pass").

#[test]
fn empty_node_group_is_rejected() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("total_assets", &[(q(1), AmountOrScalar::scalar(1000.0))]);
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![],
        equity_nodes: vec![],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let err = check
        .execute(&ctx)
        .expect_err("an empty node group makes the identity vacuous and must be rejected");
    assert!(
        err.to_string().contains("node group"),
        "error should name the empty node group, got: {err}"
    );
}

/// A node the model does not have is a configuration error, whether every
/// reference is misspelled or one member of a group is; a missing balance
/// of a known node makes its period incomplete and names the node.
#[test]
fn unknown_nodes_are_rejected_and_missing_balances_are_incomplete() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "current_assets",
            &[
                (q(1), AmountOrScalar::scalar(400.0)),
                (q(2), AmountOrScalar::scalar(400.0)),
            ],
        );
        // Fixed assets have no Q2 balance.
        model_builder.value("fixed_assets", &[(q(1), AmountOrScalar::scalar(600.0))]);
        model_builder.value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(600.0)),
                (q(2), AmountOrScalar::scalar(600.0)),
            ],
        );
        model_builder.value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(400.0)),
                (q(2), AmountOrScalar::scalar(400.0)),
            ],
        );
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let ctx = CheckContext::new(&model, &results);
    let check = |assets: &[&str]| BalanceSheetArticulation {
        assets_nodes: assets.iter().map(|node| NodeId::new(*node)).collect(),
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let error = check(&["current_assets", "fixed_assetz"])
        .execute(&ctx)
        .unwrap_err();
    assert!(
        error.to_string().contains("'fixed_assetz'")
            && error.to_string().contains("not a node of the model"),
        "{error}"
    );
    // An account listed twice, or on both sides, is rejected.
    assert!(
        check(&["current_assets", "current_assets"])
            .execute(&ctx)
            .unwrap_err()
            .to_string()
            .contains("appears twice")
    );
    assert!(
        check(&["current_assets", "total_equity"])
            .execute(&ctx)
            .unwrap_err()
            .to_string()
            .contains("appears in both")
    );

    let result = check(&["current_assets", "fixed_assets"])
        .execute(&ctx)
        .unwrap();
    use finstack_quant_statements::checks::CheckOutcome;
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    assert_eq!(result.comparisons.len(), 1, "Q1 is complete and compared");
    let finding = &result.findings[0];
    assert!(finding.coverage);
    assert_eq!(finding.period, Some(q(2)));
    assert_eq!(finding.nodes, [NodeId::new("fixed_assets")]);
}

#[test]
fn large_balance_sheet_within_relative_tolerance_passes() {
    // A $10B balance sheet with $0.05 of f64 accumulation drift must not fail
    // under the *default* config: the default tolerance is scale-aware
    // (relative component), not a flat absolute cent.
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "total_assets",
            &[(q(1), AmountOrScalar::scalar(10_000_000_000.0))],
        );
        model_builder.value(
            "total_liabilities",
            &[(q(1), AmountOrScalar::scalar(6_000_000_000.0))],
        );
        model_builder.value(
            "total_equity",
            &[(q(1), AmountOrScalar::scalar(3_999_999_999.95))],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    // Without a stated tolerance the 0.05 imbalance fails.
    let exact = check.execute(&CheckContext::new(&model, &results)).unwrap();
    assert!(!exact.passed());

    // An explicit relative tolerance of 1e-9 allows 10 on a $10B sheet.
    let mut ctx = CheckContext::new(&model, &results);
    ctx.config.default_relative_tolerance = 1e-9;
    let result = check.execute(&ctx).unwrap();

    assert!(
        result.passed(),
        "a 5e-12 relative imbalance is inside a stated 1e-9 relative tolerance: {:?}",
        result.findings
    );
}

#[test]
fn nan_operand_warns_instead_of_silently_passing() {
    // A NaN operand makes the imbalance NaN, and `NaN > tolerance` is false:
    // without a guard, a genuinely broken balance sheet silently passes. The
    // check must treat a non-finite operand like a missing one - warn and
    // skip the period.
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("total_assets", &[(q(1), AmountOrScalar::scalar(1000.0))]);
        model_builder.value(
            "total_liabilities",
            &[(q(1), AmountOrScalar::scalar(600.0))],
        );
        model_builder.value("total_equity", &[(q(1), AmountOrScalar::scalar(400.0))]);
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();
    results.set_cell(
        "total_assets",
        q(1),
        finstack_quant_statements::evaluator::ResultCell::unavailable(
            finstack_quant_statements::evaluator::UnavailableReason::MissingInput,
        ),
    );

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(
        result.findings.iter().any(|f| f.severity
            == finstack_quant_statements::checks::Severity::Warning
            && f.message.contains("total_assets")),
        "a NaN operand must produce a warning naming the node, not a silent pass: {:?}",
        result.findings
    );
}

// --- E2: accounting identities (STATEMENTS-CONTRACTS-010, -011, -012) ---

/// One hand-built three-statement model through all three identities.
///
/// Opening: cash 50, PP&E 200, debt 100, retained earnings 150.
/// Q1: net income 30, dividends 10, capex 25, borrowing 5.
///   CFO 30, CFI -25, CFF 5 - 10 = -5, total 0; cash 50.
///   PP&E 225, debt 105, retained earnings 170: 50 + 225 = 105 + 170.
/// Q2: net income 40, dividends 15, capex 10, repayment 20.
///   CFO 40, CFI -10, CFF -35, total -5; cash 45.
///   PP&E 235, debt 85, retained earnings 195: 45 + 235 = 85 + 195.
#[test]
fn a_three_statement_model_satisfies_all_three_identities() {
    use finstack_quant_statements::checks::builtins::{
        CashReconciliation, RetainedEarningsReconciliation,
    };
    use finstack_quant_statements::checks::{CheckOutcome, CheckSuite, DatedOpening};
    use time::macros::date;

    let build = |q2_cash: f64| {
        let mut builder = ModelBuilder::new("three-statement");
        builder.periods("2025Q1..Q2", None).unwrap();
        for (node, values) in [
            ("cash", [50.0, q2_cash]),
            ("ppe", [225.0, 235.0]),
            ("debt", [105.0, 85.0]),
            ("retained_earnings", [170.0, 195.0]),
            ("net_income", [30.0, 40.0]),
            ("dividends", [10.0, 15.0]),
            ("cfo", [30.0, 40.0]),
            ("cfi", [-25.0, -10.0]),
            ("cff", [-5.0, -35.0]),
            ("total_cf", [0.0, -5.0]),
        ] {
            builder.value_scalar(node, &[(q(1), values[0]), (q(2), values[1])]);
        }
        builder.build().unwrap()
    };
    let opening = |value: f64| DatedOpening {
        date: date!(2025 - 01 - 01),
        value: AmountOrScalar::scalar(value),
    };
    let suite = CheckSuite::builder("three statements")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("cash"), NodeId::new("ppe")],
            liabilities_nodes: vec![NodeId::new("debt")],
            equity_nodes: vec![NodeId::new("retained_earnings")],
            tolerance: None,
        })
        .add_check(CashReconciliation {
            cash_balance_node: NodeId::new("cash"),
            total_cash_flow_node: NodeId::new("total_cf"),
            opening_balance: opening(50.0),
            adjustment_nodes: vec![],
            cfo_node: Some(NodeId::new("cfo")),
            cfi_node: Some(NodeId::new("cfi")),
            cff_node: Some(NodeId::new("cff")),
            tolerance: None,
        })
        .add_check(RetainedEarningsReconciliation {
            retained_earnings_node: NodeId::new("retained_earnings"),
            net_income_node: NodeId::new("net_income"),
            opening_balance: opening(150.0),
            dividends_node: Some(NodeId::new("dividends")),
            other_adjustments: vec![],
            tolerance: None,
            dividends_sign_convention: Default::default(),
        })
        .build()
        .unwrap();
    let run = |q2_cash: f64| {
        let model = build(q2_cash);
        let results = Evaluator::new().evaluate(&model).unwrap();
        suite.run(&model, &results).unwrap()
    };

    let report = run(45.0);
    assert_eq!(report.outcome, CheckOutcome::Pass);
    // Two periods of articulation, two roll-forwards plus two component
    // comparisons of cash, and two retained-earnings roll-forwards.
    let comparisons: Vec<usize> = report.results.iter().map(|r| r.comparisons.len()).collect();
    assert_eq!(comparisons, [2, 4, 2]);
    assert!(
        report
            .results
            .iter()
            .flat_map(|r| &r.comparisons)
            .all(|c| c.tolerance == 0.0 && c.difference() == 0.0)
    );

    // One unit of unexplained Q2 cash breaks the balance sheet and the cash
    // roll-forward, and leaves retained earnings intact.
    let report = run(46.0);
    let outcomes: Vec<CheckOutcome> = report.results.iter().map(|r| r.outcome).collect();
    assert_eq!(
        outcomes,
        [CheckOutcome::Fail, CheckOutcome::Fail, CheckOutcome::Pass]
    );
    let sheet = report.results[0].findings[0].comparison.as_ref().unwrap();
    assert_eq!((sheet.actual, sheet.expected), (281.0, 280.0));
    let materiality = report.results[0].findings[0].materiality.as_ref().unwrap();
    assert_eq!(materiality.absolute, 1.0);
    assert!((materiality.relative_pct - 100.0 / 281.0).abs() < 1e-12);
}

/// Large balances that cancel are compared on one compensated sum, so a
/// small genuine difference is neither lost nor invented:
/// 1e16 + 3 - 1e16 against 3 balances exactly, and against 4 is off by 1.
#[test]
fn large_cancelling_balances_keep_small_differences() {
    let build = |equity: f64| {
        let mut builder = ModelBuilder::new("cancelling");
        builder.periods("2025Q1..Q1", None).unwrap();
        builder.value_scalar("gross_assets", &[(q(1), 1e16)]);
        builder.value_scalar("small_asset", &[(q(1), 3.0)]);
        builder.value_scalar("contra_asset", &[(q(1), -1e16)]);
        builder.value_scalar("liabilities", &[(q(1), 0.0)]);
        builder.value_scalar("equity", &[(q(1), equity)]);
        builder.build().unwrap()
    };
    let check = BalanceSheetArticulation {
        assets_nodes: vec![
            NodeId::new("gross_assets"),
            NodeId::new("small_asset"),
            NodeId::new("contra_asset"),
        ],
        liabilities_nodes: vec![NodeId::new("liabilities")],
        equity_nodes: vec![NodeId::new("equity")],
        tolerance: None,
    };
    let run = |equity: f64| {
        let model = build(equity);
        let results = Evaluator::new().evaluate(&model).unwrap();
        check.execute(&CheckContext::new(&model, &results)).unwrap()
    };
    let balanced = run(3.0);
    assert!(balanced.passed(), "{:?}", balanced.findings);
    assert_eq!(balanced.comparisons[0].actual, 3.0);
    let off = run(4.0);
    assert!(!off.passed());
    assert_eq!(off.findings[0].materiality.as_ref().unwrap().absolute, 1.0);
}
