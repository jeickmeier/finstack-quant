#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{Check, CheckContext, PeriodScope};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    EffectiveTaxRateCheck, GrowthRateConsistency, WorkingCapitalConsistency,
};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

// ============================================================================
// GrowthRateConsistency
// ============================================================================

#[test]
fn growth_within_bounds_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q3", None)
        .unwrap()
        .value(
            "revenue",
            &[(q(1), s(100.0)), (q(2), s(110.0)), (q(3), s(120.0))],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = GrowthRateConsistency {
        nodes: vec![NodeId::new("revenue")],
        max_period_growth_pct: 0.50,
        max_decline_pct: -0.30,
        scope: PeriodScope::AllPeriods,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

#[test]
fn growth_spike_flags_warning() {
    // Q2 = 200, Q1 = 100 → 100% growth, exceeds max_period_growth_pct = 50%
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("revenue", &[(q(1), s(100.0)), (q(2), s(200.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = GrowthRateConsistency {
        nodes: vec![NodeId::new("revenue")],
        max_period_growth_pct: 0.50,
        max_decline_pct: -0.30,
        scope: PeriodScope::AllPeriods,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].period, Some(q(2)));
}

#[test]
fn sharp_decline_flags_warning() {
    // Q2 = 50, Q1 = 100 → -50% decline, exceeds max_decline_pct = -30%
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("revenue", &[(q(1), s(100.0)), (q(2), s(50.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = GrowthRateConsistency {
        nodes: vec![NodeId::new("revenue")],
        max_period_growth_pct: 0.50,
        max_decline_pct: -0.30,
        scope: PeriodScope::AllPeriods,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
}

// ============================================================================
// EffectiveTaxRateCheck
// ============================================================================

#[test]
fn etr_within_range_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("tax", &[(q(1), s(25.0)), (q(2), s(30.0))])
        .value("pretax", &[(q(1), s(100.0)), (q(2), s(120.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = EffectiveTaxRateCheck {
        tax_expense_node: NodeId::new("tax"),
        pretax_income_node: NodeId::new("pretax"),
        expected_range: (0.15, 0.40),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

#[test]
fn etr_outside_range_flags_info() {
    // ETR = 5/100 = 5%, below 15% floor
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("tax", &[(q(1), s(5.0))])
        .value("pretax", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = EffectiveTaxRateCheck {
        tax_expense_node: NodeId::new("tax"),
        pretax_income_node: NodeId::new("pretax"),
        expected_range: (0.15, 0.40),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
}

#[test]
fn etr_skips_negative_pretax() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("tax", &[(q(1), s(-10.0))])
        .value("pretax", &[(q(1), s(-50.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = EffectiveTaxRateCheck {
        tax_expense_node: NodeId::new("tax"),
        pretax_income_node: NodeId::new("pretax"),
        expected_range: (0.15, 0.40),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

// ============================================================================
// WorkingCapitalConsistency
// ============================================================================

#[test]
fn wc_consistent_passes() {
    // NWC(Q1) = 200 - 100 = 100, NWC(Q2) = 220 - 110 = 110
    // ΔNWC = 110 - 100 = 10
    // Expected WC change on CFS = -10
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("wc_cf", &[(q(1), s(0.0)), (q(2), s(-10.0))])
        .value("ar", &[(q(1), s(200.0)), (q(2), s(220.0))])
        .value("ap", &[(q(1), s(100.0)), (q(2), s(110.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = WorkingCapitalConsistency {
        wc_change_cf_node: NodeId::new("wc_cf"),
        current_assets_nodes: vec![NodeId::new("ar")],
        current_liabilities_nodes: vec![NodeId::new("ap")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

#[test]
fn wc_mismatch_flags_warning() {
    // NWC(Q1) = 200 - 100 = 100, NWC(Q2) = 220 - 110 = 110
    // Expected WC change on CFS = -10, but CFS shows 0 → diff = 10
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("wc_cf", &[(q(1), s(0.0)), (q(2), s(0.0))])
        .value("ar", &[(q(1), s(200.0)), (q(2), s(220.0))])
        .value("ap", &[(q(1), s(100.0)), (q(2), s(110.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = WorkingCapitalConsistency {
        wc_change_cf_node: NodeId::new("wc_cf"),
        current_assets_nodes: vec![NodeId::new("ar")],
        current_liabilities_nodes: vec![NodeId::new("ap")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    let mat = result.findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 10.0).abs() < 0.01);
}
