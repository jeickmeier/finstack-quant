//! Builder tests for Phase 1.

use finstack_quant_statements::prelude::*;

// ============================================================================
// Crate Bootstrap Tests
// ============================================================================

#[test]
fn test_builder_creation() {
    let builder = ModelBuilder::new("test_model");
    // Type-state ensures we can't call .build() yet
    // This test just verifies construction works
    let _ = builder;
}

// ============================================================================
// Period Integration Tests
// ============================================================================

#[test]
fn test_periods_parsing() {
    let result = ModelBuilder::new("test").periods("2025Q1..Q4", None);
    assert!(result.is_ok());

    let builder = result.unwrap();
    let model = builder.build().unwrap();

    assert_eq!(model.periods.len(), 4);
    assert_eq!(model.periods[0].id, PeriodId::quarter(2025, 1));
    assert_eq!(model.periods[3].id, PeriodId::quarter(2025, 4));
}

#[test]
fn test_periods_with_actuals_cutoff() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.periods.len(), 4);

    // Q1 and Q2 should be actuals
    assert!(model.periods[0].is_actual);
    assert!(model.periods[1].is_actual);

    // Q3 and Q4 should be forecast
    assert!(!model.periods[2].is_actual);
    assert!(!model.periods[3].is_actual);
}

#[test]
fn test_periods_explicit() {
    let periods = build_periods("2025Q1..Q2", None).unwrap().periods;
    let model = ModelBuilder::new("test")
        .periods_explicit(periods)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.periods.len(), 2);
}

#[test]
fn test_empty_periods_error() {
    let result = ModelBuilder::new("test").periods_explicit(vec![]);
    assert!(result.is_err());
}

#[test]
fn test_model_serialization() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .build()
        .unwrap();

    // Test JSON serialization roundtrip
    let json = serde_json::to_string(&model).unwrap();
    let deserialized: FinancialModelSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(model.id, deserialized.id);
    assert_eq!(model.periods.len(), deserialized.periods.len());
}

// ============================================================================
// Value Node Tests
// ============================================================================

#[test]
fn test_value_node_single_period() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
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

    assert_eq!(model.nodes.len(), 1);
    assert!(model.has_node("revenue"));

    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.node_type, NodeType::Value);
    assert!(node.values.is_some());

    let values = node.values.as_ref().unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[&PeriodId::quarter(2025, 1)].value(), 100_000.0);
}

#[test]
fn test_value_node_multiple_periods() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(121_000.0),
                ),
            ],
        )
        .build()
        .unwrap();

    let node = model.get_node("revenue").unwrap();
    let values = node.values.as_ref().unwrap();

    assert_eq!(values.len(), 3);
    assert_eq!(values[&PeriodId::quarter(2025, 1)].value(), 100_000.0);
    assert_eq!(values[&PeriodId::quarter(2025, 2)].value(), 110_000.0);
    assert_eq!(values[&PeriodId::quarter(2025, 3)].value(), 121_000.0);
}

#[test]
fn test_value_node_with_currency() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::amount(1_000_000.0, Currency::USD),
            )],
        )
        .build()
        .unwrap();

    let node = model.get_node("revenue").unwrap();
    let values = node.values.as_ref().unwrap();
    let amount = &values[&PeriodId::quarter(2025, 1)];

    assert!(amount.is_amount());
    assert_eq!(amount.currency(), Some(Currency::USD));
    assert_eq!(amount.value(), 1_000_000.0);
}

#[test]
fn test_calculated_node() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.nodes.len(), 1);
    let node = model.get_node("gross_profit").unwrap();

    assert_eq!(node.node_type, NodeType::Calculated);
    assert_eq!(node.formula_text.as_ref().unwrap(), "revenue - cogs");
    assert!(node.values.is_none());
}

#[test]
fn test_empty_formula_error() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .compute("invalid", "");

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::FormulaParse(_)));
}

#[test]
fn test_whitespace_only_formula_error() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .compute("invalid", "   ");

    assert!(result.is_err());
}

#[test]
fn test_multiple_nodes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.nodes.len(), 3);
    assert!(model.has_node("revenue"));
    assert!(model.has_node("cogs"));
    assert!(model.has_node("gross_profit"));
}

#[test]
fn test_metadata() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .with_meta("author", serde_json::json!("Test User"))
        .with_meta("version", serde_json::json!("1.0.0"))
        .build()
        .unwrap();

    assert_eq!(model.meta.len(), 2);
    assert_eq!(model.meta["author"], "Test User");
    assert_eq!(model.meta["version"], "1.0.0");
}

#[test]
fn test_schema_version() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.schema_version, 2);
}

// ============================================================================
// Complex Integration Tests
// ============================================================================

#[test]
fn test_basic_pl_model() {
    let model = ModelBuilder::new("Simple P&L")
        .periods("2025Q1..2025Q4", Some("2025Q2"))
        .unwrap()
        // Revenue
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(10_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(11_000_000.0),
                ),
            ],
        )
        // COGS as 60% of revenue
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        // Operating expenses
        .value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(2_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(2_100_000.0),
                ),
            ],
        )
        // Derived metrics
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("operating_income", "gross_profit - opex")
        .unwrap()
        .compute("gross_margin", "gross_profit / revenue")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(model.id, "Simple P&L");
    assert_eq!(model.periods.len(), 4);
    assert_eq!(model.nodes.len(), 6);

    // Verify actuals periods
    assert!(model.periods[0].is_actual);
    assert!(model.periods[1].is_actual);
    assert!(!model.periods[2].is_actual);
    assert!(!model.periods[3].is_actual);

    // Verify node types
    assert_eq!(
        model.get_node("revenue").unwrap().node_type,
        NodeType::Value
    );
    assert_eq!(
        model.get_node("cogs").unwrap().node_type,
        NodeType::Calculated
    );
    assert_eq!(
        model.get_node("gross_profit").unwrap().node_type,
        NodeType::Calculated
    );
}

#[test]
fn test_model_with_multiple_currencies() {
    let model = ModelBuilder::new("Multi-currency")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "usd_revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::amount(1_000_000.0, Currency::USD),
            )],
        )
        .value(
            "eur_revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::amount(900_000.0, Currency::EUR),
            )],
        )
        .build()
        .unwrap();

    let usd_node = model.get_node("usd_revenue").unwrap();
    let eur_node = model.get_node("eur_revenue").unwrap();

    let usd_values = usd_node.values.as_ref().unwrap();
    let eur_values = eur_node.values.as_ref().unwrap();

    assert_eq!(
        usd_values[&PeriodId::quarter(2025, 1)].currency(),
        Some(Currency::USD)
    );
    assert_eq!(
        eur_values[&PeriodId::quarter(2025, 1)].currency(),
        Some(Currency::EUR)
    );
}

// ============================================================================
// NodeSpec Builder Method Tests
// ============================================================================

#[test]
fn test_node_spec_with_tags() {
    use finstack_quant_statements::types::NodeSpec;

    let node = NodeSpec::new("revenue", NodeType::Value)
        .with_tags(vec!["income_statement".into(), "top_line".into()]);

    assert_eq!(node.tags.len(), 2);
    assert!(node.tags.contains(&"income_statement".to_string()));
    assert!(node.tags.contains(&"top_line".to_string()));
}

#[test]
fn test_node_spec_with_forecast() {
    use finstack_quant_statements::types::{ForecastMethod, ForecastSpec, NodeSpec};
    use indexmap::indexmap;

    let forecast = ForecastSpec {
        method: ForecastMethod::GrowthPct,
        params: indexmap! { "rate".into() => serde_json::json!(0.05) },
    };

    let node = NodeSpec::new("revenue", NodeType::Mixed).with_forecast(forecast);

    assert!(node.forecast.is_some());
    assert_eq!(
        node.forecast.as_ref().unwrap().method,
        ForecastMethod::GrowthPct
    );
}

#[test]
fn test_node_spec_with_name() {
    use finstack_quant_statements::types::NodeSpec;

    let node = NodeSpec::new("revenue", NodeType::Value).with_name("Total Revenue");

    assert!(node.name.is_some());
    assert_eq!(node.name.as_ref().unwrap(), "Total Revenue");
}

#[test]
fn test_node_spec_with_formula() {
    use finstack_quant_statements::types::NodeSpec;

    let node = NodeSpec::new("gross_profit", NodeType::Calculated).with_formula("revenue - cogs");

    assert!(node.formula_text.is_some());
    assert_eq!(node.formula_text.as_ref().unwrap(), "revenue - cogs");
}

#[test]
fn test_node_spec_with_values() {
    use finstack_quant_statements::types::NodeSpec;
    use indexmap::IndexMap;

    let mut values = IndexMap::new();
    values.insert(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0));
    values.insert(PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0));

    let node = NodeSpec::new("revenue", NodeType::Value).with_values(values.clone());

    assert!(node.values.is_some());
    assert_eq!(node.values.as_ref().unwrap().len(), 2);
}

#[test]
fn test_forecast_spec_helpers() {
    use finstack_quant_statements::types::ForecastSpec;

    // Test forward_fill helper
    let ff = ForecastSpec::forward_fill();
    assert_eq!(ff.method, ForecastMethod::ForwardFill);
    assert!(ff.params.is_empty());

    // Test growth helper
    let growth = ForecastSpec::growth(0.05);
    assert_eq!(growth.method, ForecastMethod::GrowthPct);
    assert_eq!(growth.params.get("rate").unwrap(), &serde_json::json!(0.05));

    // Test curve helper
    let curve = ForecastSpec::curve(vec![0.05, 0.06, 0.07]);
    assert_eq!(curve.method, ForecastMethod::CurvePct);
    assert_eq!(
        curve.params.get("curve").unwrap(),
        &serde_json::json!([0.05, 0.06, 0.07])
    );

    // Test normal helper
    let normal = ForecastSpec::normal(100.0, 15.0, 42);
    assert_eq!(normal.method, ForecastMethod::Normal);
    assert_eq!(
        normal.params.get("mean").unwrap(),
        &serde_json::json!(100.0)
    );
    assert_eq!(
        normal.params.get("std_dev").unwrap(),
        &serde_json::json!(15.0)
    );
    assert_eq!(normal.params.get("seed").unwrap(), &serde_json::json!(42));

    // Test lognormal helper
    let lognormal = ForecastSpec::lognormal(11.5, 0.15, 42);
    assert_eq!(lognormal.method, ForecastMethod::LogNormal);
}

// ============================================================================
// AmountOrScalar Tests
// ============================================================================

#[test]
fn test_amount_or_scalar_from_conversions() {
    use finstack_quant_core::money::Money;

    // Test From<f64>
    let scalar: AmountOrScalar = 42.0.into();
    assert!(scalar.is_scalar());
    assert_eq!(scalar.value(), 42.0);

    // Test From<Money>
    let money = Money::new(1000.0, Currency::USD);
    let amount: AmountOrScalar = money.into();
    assert!(amount.is_amount());
    assert_eq!(amount.value(), 1000.0);
    assert_eq!(amount.currency(), Some(Currency::USD));
}

// ============================================================================
// Node ID Validation Tests
// ============================================================================

/// Test that node IDs containing the reserved __cs__ prefix are rejected.
/// This prefix is used internally for capital structure references.
#[test]
fn test_reserved_cs_prefix_rejected_in_value() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "__cs__bad_node",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("reserved") || err.contains("__cs__"),
        "Expected error about reserved prefix, got: {}",
        err
    );
}

/// Test that node IDs containing __cs__ anywhere are rejected.
#[test]
fn test_reserved_cs_prefix_in_middle_rejected() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue__cs__hidden",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build();

    assert!(result.is_err());
}

/// Test that node IDs starting with __ (double underscore) are rejected.
/// This prefix is reserved for internal use.
#[test]
fn test_double_underscore_prefix_rejected_in_compute() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .compute("__internal_node", "100");

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("reserved") || err.contains("__"),
        "Expected error about reserved prefix, got: {}",
        err
    );
}

/// Test that normal node IDs without reserved prefixes are accepted.
#[test]
fn test_normal_node_ids_accepted() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "_single_underscore",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50.0))],
        )
        .value(
            "node_with__double_inside",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(25.0))],
        )
        .compute("gross_profit", "revenue * 0.4")
        .unwrap()
        .build();

    assert!(result.is_ok());
}

/// Test that mixed node builder also validates node IDs.
#[test]
fn test_reserved_prefix_rejected_in_mixed_node() {
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .mixed("__bad_mixed")
        .values(&[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))])
        .build();

    assert!(result.is_err());
}
