//! Tests for the .mixed() fluent builder

use finstack_quant_statements::prelude::*;
use indexmap::indexmap;

#[test]
fn test_mixed_builder_basic() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .mixed("revenue")
        .values(&[
            (
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(ForecastSpec {
            method: ForecastMethod::GrowthPct,
            params: indexmap! { "rate".into() => serde_json::json!(0.05) },
        })
        .formula("lag(revenue, 1) * 1.05")
        .unwrap()
        .build()
        .unwrap()
        .build()
        .unwrap();

    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.node_type, NodeType::Mixed);
    assert!(node.values.is_some());
    assert!(node.forecast.is_some());
    assert!(node.formula_text.is_some());
}

#[test]
fn test_mixed_builder_with_name() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .mixed("revenue")
        .name("Total Revenue")
        .values(&[(
            PeriodId::quarter(2025, 1),
            AmountOrScalar::scalar(100_000.0),
        )])
        .build()
        .unwrap()
        .build()
        .unwrap();

    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.name.as_ref().unwrap(), "Total Revenue");
}

#[test]
fn test_mixed_builder_evaluation() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .mixed("revenue")
        .values(&[
            (
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(ForecastSpec {
            method: ForecastMethod::GrowthPct,
            params: indexmap! { "rate".into() => serde_json::json!(0.05) },
        })
        .build()
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1-Q2: Should use explicit values (Value precedence)
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(110_000.0)
    );

    // Q3-Q4: Should use forecast (110k * 1.05 = 115.5k, then 121.275k)
    let q3 = results.get("revenue", &PeriodId::quarter(2025, 3)).unwrap();
    let q4 = results.get("revenue", &PeriodId::quarter(2025, 4)).unwrap();

    assert!((q3 - 115_500.0).abs() < 1.0);
    assert!((q4 - 121_275.0).abs() < 1.0);
}

#[test]
fn test_mixed_builder_formula_fallback() {
    // Test that formula is used when no value or forecast applies
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "base",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(130.0)),
            ],
        )
        .mixed("derived")
        .values(&[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50.0))])
        .formula("base * 0.5")
        .unwrap()
        .build()
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: Use explicit value
    assert_eq!(
        results.get("derived", &PeriodId::quarter(2025, 1)),
        Some(50.0)
    );

    // Q2-Q4: Use formula (base * 0.5)
    assert_eq!(
        results.get("derived", &PeriodId::quarter(2025, 2)),
        Some(55.0)
    );
    assert_eq!(
        results.get("derived", &PeriodId::quarter(2025, 3)),
        Some(60.0)
    );
    assert_eq!(
        results.get("derived", &PeriodId::quarter(2025, 4)),
        Some(65.0)
    );
}

#[test]
fn test_mixed_builder_empty_formula_error() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .mixed("revenue")
        .formula("");

    assert!(result.is_err());
}

#[test]
fn test_mixed_builder_minimal() {
    // Test that you can create a mixed node with just values
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .mixed("revenue")
        .values(&[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))])
        .build()
        .unwrap()
        .build()
        .unwrap();

    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.node_type, NodeType::Mixed);
    assert!(node.values.is_some());
    assert!(node.forecast.is_none());
    assert!(node.formula_text.is_none());
}
