#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::{
    BalanceSheetArticulation, NonFiniteCheck, SignConventionCheck,
};
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use proptest::prelude::*;

fn q1() -> PeriodId {
    PeriodId::quarter(2025, 1)
}

// ---------------------------------------------------------------------------
// Balance-sheet articulation: always passes when A = L + E
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn balance_sheet_always_passes_when_balanced(
        a in 1.0f64..1e9,
        l_frac in 0.0f64..1.0,
    ) {
        let l = a * l_frac;
        let e = a - l;

        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("total_assets", &[(q1(), AmountOrScalar::scalar(a))])
            .value("total_liabilities", &[(q1(), AmountOrScalar::scalar(l))])
            .value("total_equity", &[(q1(), AmountOrScalar::scalar(e))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let check = BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: Some(1e-6),
        };

        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(result.passed, "Expected pass for A={a}, L={l}, E={e}");
    }

    #[test]
    fn balance_sheet_always_fails_when_imbalanced(
        a in 1.0f64..1e12,
        l in 0.0f64..1e12,
        gap in 1.0f64..1e6,
    ) {
        let e = a - l + gap; // intentional imbalance of `gap`

        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("total_assets", &[(q1(), AmountOrScalar::scalar(a))])
            .value("total_liabilities", &[(q1(), AmountOrScalar::scalar(l))])
            .value("total_equity", &[(q1(), AmountOrScalar::scalar(e))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let check = BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: Some(0.5),
        };

        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(!result.passed, "Expected fail for gap={gap}");
    }
}

// ---------------------------------------------------------------------------
// NonFiniteCheck: NaN / Inf injection is always detected
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn non_finite_always_catches_nan(
        base_val in -1e12f64..1e12,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("good_node", &[(q1(), AmountOrScalar::scalar(base_val))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let mut results = ev.evaluate(&model).unwrap();

        results
            .nodes
            .entry("bad_node".to_string())
            .or_default()
            .insert(q1(), f64::NAN);

        let check = NonFiniteCheck { nodes: vec![] };
        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(!result.passed, "NonFiniteCheck should catch NaN");
    }

    #[test]
    fn non_finite_always_catches_inf(
        base_val in -1e12f64..1e12,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("good_node", &[(q1(), AmountOrScalar::scalar(base_val))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let mut results = ev.evaluate(&model).unwrap();

        results
            .nodes
            .entry("bad_node".to_string())
            .or_default()
            .insert(q1(), f64::INFINITY);

        let check = NonFiniteCheck { nodes: vec![] };
        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(!result.passed, "NonFiniteCheck should catch Inf");
    }

    #[test]
    fn non_finite_always_catches_neg_inf(
        base_val in -1e12f64..1e12,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("good_node", &[(q1(), AmountOrScalar::scalar(base_val))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let mut results = ev.evaluate(&model).unwrap();

        results
            .nodes
            .entry("bad_node".to_string())
            .or_default()
            .insert(q1(), f64::NEG_INFINITY);

        let check = NonFiniteCheck { nodes: vec![] };
        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(!result.passed, "NonFiniteCheck should catch -Inf");
    }
}

// ---------------------------------------------------------------------------
// SignConventionCheck: passes when all positive_nodes > 0
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn sign_convention_passes_when_positive(
        v1 in 0.01f64..1e12,
        v2 in 0.01f64..1e12,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("revenue", &[(q1(), AmountOrScalar::scalar(v1))])
            .value("other_income", &[(q1(), AmountOrScalar::scalar(v2))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let check = SignConventionCheck {
            positive_nodes: vec![NodeId::new("revenue"), NodeId::new("other_income")],
            negative_nodes: vec![],
        };

        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(result.passed, "Expected pass for v1={v1}, v2={v2}");
        prop_assert!(result.findings.is_empty(), "Expected no findings");
    }

    #[test]
    fn sign_convention_warns_when_negative(
        v in -1e12f64..-0.01,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("revenue", &[(q1(), AmountOrScalar::scalar(v))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let check = SignConventionCheck {
            positive_nodes: vec![NodeId::new("revenue")],
            negative_nodes: vec![],
        };

        let ctx = CheckContext::new(&model, &results);
        let result = check.execute(&ctx).unwrap();
        prop_assert!(!result.findings.is_empty(), "Expected warnings for v={v}");
    }
}
