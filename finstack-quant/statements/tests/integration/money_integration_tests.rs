//! Tests for Money/AmountOrScalar dual-type support.
//!
//! This test suite validates:
//! - Money values preserve currency through evaluation
//! - Cross-currency operations produce appropriate errors
//! - Scalar operations work correctly
//! - Monetary vs scalar tracking is accurate
//! - Results provide both Money and f64 accessors

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_statements::prelude::*;

// ============================================================================
// Money Currency Preservation Tests
// ============================================================================

#[test]
fn test_money_preserves_currency_usd() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value_money(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    Money::new(100_000.0, Currency::USD),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    Money::new(110_000.0, Currency::USD),
                ),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check f64 accessor works
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );

    // Check Money accessor preserves currency
    let money = results
        .get_money("revenue", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(money.amount(), 100_000.0);
    assert_eq!(money.currency(), Currency::USD);

    // Check node value type is tracked
    assert_eq!(
        results.node_value_types.get("revenue"),
        Some(&NodeValueType::Monetary {
            currency: Currency::USD
        })
    );
}

#[test]
fn test_money_preserves_currency_eur() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_money(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(50_000.0, Currency::EUR),
            )],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let money = results
        .get_money("revenue", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(money.currency(), Currency::EUR);
}

#[test]
fn test_money_multi_currency_tracking() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_money(
            "usd_revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(100_000.0, Currency::USD),
            )],
        )
        .value_money(
            "eur_revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(50_000.0, Currency::EUR),
            )],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Both currencies should be preserved independently
    let usd_money = results
        .get_money("usd_revenue", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(usd_money.currency(), Currency::USD);

    let eur_money = results
        .get_money("eur_revenue", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(eur_money.currency(), Currency::EUR);
}

// ============================================================================
// Scalar Operations Tests
// ============================================================================

#[test]
fn test_scalar_values_work_correctly() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value_scalar(
            "gross_margin_pct",
            &[
                (PeriodId::quarter(2025, 1), 0.35),
                (PeriodId::quarter(2025, 2), 0.37),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check scalar accessor
    assert_eq!(
        results.get_scalar("gross_margin_pct", &PeriodId::quarter(2025, 1)),
        Some(0.35)
    );

    // Check node value type
    assert_eq!(
        results.node_value_types.get("gross_margin_pct"),
        Some(&NodeValueType::Scalar)
    );

    // Money accessor should return None for scalar nodes
    assert_eq!(
        results.get_money("gross_margin_pct", &PeriodId::quarter(2025, 1)),
        None
    );
}

#[test]
fn test_mixed_monetary_and_scalar_nodes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_money(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(100_000.0, Currency::USD),
            )],
        )
        .value_scalar("count", &[(PeriodId::quarter(2025, 1), 42.0)])
        .value_scalar("ratio", &[(PeriodId::quarter(2025, 1), 0.15)])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Monetary node
    assert_eq!(
        results.node_value_types.get("revenue"),
        Some(&NodeValueType::Monetary {
            currency: Currency::USD
        })
    );

    // Scalar nodes
    assert_eq!(
        results.node_value_types.get("count"),
        Some(&NodeValueType::Scalar)
    );
    assert_eq!(
        results.node_value_types.get("ratio"),
        Some(&NodeValueType::Scalar)
    );
}

// ============================================================================
// Forecast Determinism Tests
// ============================================================================

#[test]
fn test_forecast_determinism_with_seed() {
    let forecast_spec = ForecastSpec::normal(100_000.0, 15_000.0, 42);

    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value_scalar("revenue", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .forecast("revenue", forecast_spec)
        .build()
        .unwrap();

    // Evaluate twice
    let mut evaluator1 = Evaluator::new();
    let results1 = evaluator1.evaluate(&model).unwrap();

    let mut evaluator2 = Evaluator::new();
    let results2 = evaluator2.evaluate(&model).unwrap();

    // Results should be identical with same seed
    assert_eq!(
        results1.get("revenue", &PeriodId::quarter(2025, 2)),
        results2.get("revenue", &PeriodId::quarter(2025, 2))
    );
    assert_eq!(
        results1.get("revenue", &PeriodId::quarter(2025, 3)),
        results2.get("revenue", &PeriodId::quarter(2025, 3))
    );
    assert_eq!(
        results1.get("revenue", &PeriodId::quarter(2025, 4)),
        results2.get("revenue", &PeriodId::quarter(2025, 4))
    );
}

#[test]
fn test_forecast_different_seeds_produce_different_results() {
    let model1 = ModelBuilder::new("test")
        .periods("2025Q1..Q2", Some("2025Q1"))
        .unwrap()
        .value_scalar("revenue", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .forecast("revenue", ForecastSpec::normal(100_000.0, 15_000.0, 42))
        .build()
        .unwrap();

    let model2 = ModelBuilder::new("test")
        .periods("2025Q1..Q2", Some("2025Q1"))
        .unwrap()
        .value_scalar("revenue", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .forecast("revenue", ForecastSpec::normal(100_000.0, 15_000.0, 99))
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results1 = evaluator.evaluate(&model1).unwrap();
    let results2 = evaluator.evaluate(&model2).unwrap();

    // Different seeds should produce different forecasts
    assert_ne!(
        results1.get("revenue", &PeriodId::quarter(2025, 2)),
        results2.get("revenue", &PeriodId::quarter(2025, 2))
    );
}

#[test]
fn test_lognormal_forecast_determinism() {
    let forecast_spec = ForecastSpec::lognormal(11.5, 0.15, 42);

    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q3", Some("2025Q1"))
        .unwrap()
        .value_scalar("price", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .forecast("price", forecast_spec)
        .build()
        .unwrap();

    let mut evaluator1 = Evaluator::new();
    let results1 = evaluator1.evaluate(&model).unwrap();

    let mut evaluator2 = Evaluator::new();
    let results2 = evaluator2.evaluate(&model).unwrap();

    // Same seed should produce identical results
    for period in &[PeriodId::quarter(2025, 2), PeriodId::quarter(2025, 3)] {
        assert_eq!(
            results1.get("price", period),
            results2.get("price", period),
            "LogNormal forecast should be deterministic with same seed"
        );
    }
}

// ============================================================================
// Results Metadata Tests
// ============================================================================

#[test]
fn test_results_metadata_includes_numeric_mode() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_scalar("revenue", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(results.meta.numeric_mode, NumericMode::Float64);
    assert!(!results.meta.parallel);
    assert!(results.meta.num_nodes > 0);
    assert!(results.meta.num_periods > 0);
}

#[test]
fn test_results_metadata_timing() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value_scalar(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), 100_000.0),
                (PeriodId::quarter(2025, 2), 110_000.0),
                (PeriodId::quarter(2025, 3), 120_000.0),
                (PeriodId::quarter(2025, 4), 130_000.0),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Evaluation time should be recorded (except on WASM)
    #[cfg(not(target_arch = "wasm32"))]
    assert!(results.meta.eval_time_ms.is_some());
}

// ============================================================================
// Builder Method Tests
// ============================================================================

#[test]
fn test_value_method_with_amount_or_scalar() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
}

#[test]
fn test_compute_method_with_formula() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_scalar("revenue", &[(PeriodId::quarter(2025, 1), 100_000.0)])
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("cogs", &PeriodId::quarter(2025, 1)),
        Some(60_000.0)
    );
}

// ============================================================================
// Formula Operations with Money Tests
// ============================================================================

#[test]
fn test_formula_with_monetary_nodes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_money(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(100_000.0, Currency::USD),
            )],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Formulas should work with monetary inputs
    assert_eq!(
        results.get("gross_profit", &PeriodId::quarter(2025, 1)),
        Some(40_000.0)
    );

    // Check that revenue is tracked as monetary
    assert!(matches!(
        results.node_value_types.get("revenue"),
        Some(NodeValueType::Monetary { .. })
    ));
}

// ============================================================================
// Results Accessor Tests
// ============================================================================

#[test]
fn test_get_scalar_returns_none_for_monetary() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_money(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                Money::new(100_000.0, Currency::USD),
            )],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // get_scalar should return None for monetary nodes
    assert_eq!(
        results.get_scalar("revenue", &PeriodId::quarter(2025, 1)),
        None
    );

    // get_money should work
    assert!(results
        .get_money("revenue", &PeriodId::quarter(2025, 1))
        .is_some());
}

#[test]
fn test_get_money_returns_none_for_scalar() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_scalar("ratio", &[(PeriodId::quarter(2025, 1), 0.15)])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // get_money should return None for scalar nodes
    assert_eq!(
        results.get_money("ratio", &PeriodId::quarter(2025, 1)),
        None
    );

    // get_scalar should work
    assert_eq!(
        results.get_scalar("ratio", &PeriodId::quarter(2025, 1)),
        Some(0.15)
    );
}

// ============================================================================
// AmountOrScalar Tests
// ============================================================================

#[test]
fn test_amount_or_scalar_as_money() {
    let amount = AmountOrScalar::amount(1_000_000.0, Currency::USD);
    let money = amount.as_money().unwrap();
    assert_eq!(money.amount(), 1_000_000.0);
    assert_eq!(money.currency(), Currency::USD);

    let scalar = AmountOrScalar::scalar(42.0);
    assert!(scalar.as_money().is_none());
}

#[test]
fn test_amount_or_scalar_from_money() {
    let money = Money::new(500.0, Currency::EUR);
    let aos: AmountOrScalar = money.into();

    assert!(aos.is_amount());
    assert!(!aos.is_scalar());
    assert_eq!(aos.value(), 500.0);
    assert_eq!(aos.currency(), Some(Currency::EUR));
}

// ============================================================================
// NodeValueType Serialization Tests
// ============================================================================

#[test]
fn test_node_value_type_serialization() {
    let monetary = NodeValueType::Monetary {
        currency: Currency::USD,
    };
    let json = serde_json::to_string(&monetary).unwrap();
    let deserialized: NodeValueType = serde_json::from_str(&json).unwrap();
    assert_eq!(monetary, deserialized);

    let scalar = NodeValueType::Scalar;
    let json = serde_json::to_string(&scalar).unwrap();
    let deserialized: NodeValueType = serde_json::from_str(&json).unwrap();
    assert_eq!(scalar, deserialized);
}

// ============================================================================
// NumericMode Tests
// ============================================================================

#[test]
fn test_numeric_mode_serialization() {
    let mode = NumericMode::Float64;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, r#""float64""#);

    let mode2 = NumericMode::Decimal;
    let json2 = serde_json::to_string(&mode2).unwrap();
    assert_eq!(json2, r#""decimal""#);
}

// ============================================================================
// Builder API Tests
// ============================================================================

#[test]
fn test_value_money_builder_api() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value_money(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    Money::new(100_000.0, Currency::USD),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    Money::new(110_000.0, Currency::USD),
                ),
            ],
        )
        .build()
        .unwrap();

    assert!(model.has_node("revenue"));
    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.node_type, NodeType::Value);
    assert_eq!(
        node.value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::USD
        })
    );
}

#[test]
fn test_value_money_rejects_mixed_currencies() {
    let err = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value_money(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    Money::new(100_000.0, Currency::USD),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    Money::new(90_000.0, Currency::EUR),
                ),
            ],
        )
        .build()
        .expect_err("mixed-currency monetary nodes must not build");

    assert!(err.to_string().contains("Currency mismatch"));
}

#[test]
fn test_value_scalar_builder_api() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value_scalar("margin_pct", &[(PeriodId::quarter(2025, 1), 0.35)])
        .build()
        .unwrap();

    let node = model.get_node("margin_pct").unwrap();
    assert_eq!(node.node_type, NodeType::Value);
    assert_eq!(node.value_type, Some(NodeValueType::Scalar));
}
