#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::{
    MissingValueCheck, NonFiniteCheck, SignConventionCheck,
};
use finstack_quant_statements::checks::{Check, CheckContext, PeriodScope, Severity};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

// ---------------------------------------------------------------------------
// MissingValueCheck
// ---------------------------------------------------------------------------

#[test]
fn missing_value_actual_is_error() {
    // Q1,Q2 actual; Q3,Q4 forecast. Build with all values, then remove some.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(110.0)),
                (q(3), AmountOrScalar::scalar(200.0)),
                (q(4), AmountOrScalar::scalar(210.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();

    // Remove Q2 and Q4 to simulate missing values
    if let Some(period_map) = results.nodes.get_mut("revenue") {
        period_map.swap_remove(&q(2));
        period_map.swap_remove(&q(4));
    }

    let check = MissingValueCheck {
        required_nodes: vec![NodeId::new("revenue")],
        scope: PeriodScope::AllPeriods,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed); // Q2 missing in actuals → Error

    // Q2 is actual → Error
    let q2_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.period == Some(q(2)))
        .collect();
    assert_eq!(q2_findings.len(), 1);
    assert_eq!(q2_findings[0].severity, Severity::Error);

    // Q4 is forecast → Warning
    let q4_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.period == Some(q(4)))
        .collect();
    assert_eq!(q4_findings.len(), 1);
    assert_eq!(q4_findings[0].severity, Severity::Warning);
}

#[test]
fn missing_value_actuals_only_scope() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(110.0)),
                (q(3), AmountOrScalar::scalar(200.0)),
                (q(4), AmountOrScalar::scalar(210.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();

    // Remove Q2 (actual) and Q4 (forecast) to simulate missing values
    if let Some(period_map) = results.nodes.get_mut("revenue") {
        period_map.swap_remove(&q(2));
        period_map.swap_remove(&q(4));
    }

    let check = MissingValueCheck {
        required_nodes: vec![NodeId::new("revenue")],
        scope: PeriodScope::ActualsOnly,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    // Only Q2 flagged (actual, missing). Q4 is out of scope.
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].period, Some(q(2)));
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn missing_value_forecast_only_scope() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(110.0)),
                (q(3), AmountOrScalar::scalar(200.0)),
                (q(4), AmountOrScalar::scalar(210.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();

    // Remove Q2 (actual) and Q4 (forecast)
    if let Some(period_map) = results.nodes.get_mut("revenue") {
        period_map.swap_remove(&q(2));
        period_map.swap_remove(&q(4));
    }

    let check = MissingValueCheck {
        required_nodes: vec![NodeId::new("revenue")],
        scope: PeriodScope::ForecastOnly,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    // Only Q4 flagged (forecast, missing). Q2 is out of scope.
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].period, Some(q(4)));
    assert_eq!(result.findings[0].severity, Severity::Warning);
    assert!(result.passed); // warnings only → passed
}

// ---------------------------------------------------------------------------
// SignConventionCheck
// ---------------------------------------------------------------------------

#[test]
fn sign_convention_positive_violation() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(-50.0)), // unexpected negative
                (q(2), AmountOrScalar::scalar(200.0)), // fine
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = SignConventionCheck {
        positive_nodes: vec![NodeId::new("revenue")],
        negative_nodes: vec![],
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed); // warnings only → passed
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
    assert_eq!(result.findings[0].period, Some(q(1)));
}

#[test]
fn sign_convention_negative_violation() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "expense",
            &[
                (q(1), AmountOrScalar::scalar(-100.0)), // fine (expected negative)
                (q(2), AmountOrScalar::scalar(50.0)),   // unexpected positive
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = SignConventionCheck {
        positive_nodes: vec![],
        negative_nodes: vec![NodeId::new("expense")],
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed); // warnings only → passed
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
    assert_eq!(result.findings[0].period, Some(q(2)));
}

#[test]
fn sign_convention_clean() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(200.0)),
            ],
        )
        .value(
            "expense",
            &[
                (q(1), AmountOrScalar::scalar(-50.0)),
                (q(2), AmountOrScalar::scalar(-60.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = SignConventionCheck {
        positive_nodes: vec![NodeId::new("revenue")],
        negative_nodes: vec![NodeId::new("expense")],
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

// ---------------------------------------------------------------------------
// NonFiniteCheck
// ---------------------------------------------------------------------------

#[test]
fn non_finite_nan_detected() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "good_node",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(200.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();

    // Inject NaN into results
    results
        .nodes
        .entry("nan_node".to_string())
        .or_default()
        .insert(q(1), f64::NAN);

    let check = NonFiniteCheck { nodes: vec![] };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);

    let nan_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.nodes.iter().any(|n| n.as_str() == "nan_node"))
        .collect();
    assert_eq!(nan_findings.len(), 1);
    assert_eq!(nan_findings[0].severity, Severity::Error);
}

#[test]
fn non_finite_inf_detected() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "good_node",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(200.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();

    // Inject Inf into results
    results
        .nodes
        .entry("inf_node".to_string())
        .or_default()
        .insert(q(1), f64::INFINITY);

    let check = NonFiniteCheck {
        nodes: vec![NodeId::new("inf_node")],
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn non_finite_all_finite_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "good_node",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(200.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = NonFiniteCheck {
        nodes: vec![NodeId::new("good_node")],
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}
