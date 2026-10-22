//! Builder tests for Phase 1.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::build_periods;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{
    AmountOrScalar, FinancialModelSpec, ForecastMethod, NodeType,
};
use finstack_quant_statements::Error;

// Period Integration Tests

#[test]
fn test_periods_parsing() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        match model_builder.periods("2025Q1..Q4", None) {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };
    assert!(result.is_ok());

    let builder = result.unwrap();
    let model = builder.build().unwrap();

    assert_eq!(model.periods.len(), 4);
    assert_eq!(
        model.periods[0].id,
        PeriodId::quarter(2025, 1).expect("valid period fixture")
    );
    assert_eq!(
        model.periods[3].id,
        PeriodId::quarter(2025, 4).expect("valid period fixture")
    );
}

#[test]
fn test_periods_with_actuals_cutoff() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q4", Some("2025Q2")).unwrap();
        model_builder.build().unwrap()
    };

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
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods_explicit(periods).unwrap();
        model_builder.build().unwrap()
    };

    assert_eq!(model.periods.len(), 2);
}

#[test]
fn test_empty_periods_error() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        match model_builder.periods_explicit(vec![]) {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };
    assert!(result.is_err());
}

#[test]
fn test_model_serialization() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.build().unwrap()
    };

    // Test JSON serialization roundtrip
    let json = serde_json::to_string(&model).unwrap();
    let deserialized: FinancialModelSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(model.id, deserialized.id);
    assert_eq!(model.periods.len(), deserialized.periods.len());
}

// Value Node Tests

#[test]
fn test_value_node_single_period() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            )],
        );
        model_builder.build().unwrap()
    };

    assert_eq!(model.nodes.len(), 1);
    assert!(model.has_node("revenue"));

    let node = model.get_node("revenue").unwrap();
    assert_eq!(node.node_type, NodeType::Value);
    assert!(node.values.is_some());

    let values = node.values.as_ref().unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[&PeriodId::quarter(2025, 1).expect("valid period fixture")].value(),
        100_000.0
    );
}

#[test]
fn test_value_node_multiple_periods() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q4", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(121_000.0),
                ),
            ],
        );
        model_builder.build().unwrap()
    };

    let node = model.get_node("revenue").unwrap();
    let values = node.values.as_ref().unwrap();

    assert_eq!(values.len(), 3);
    assert_eq!(
        values[&PeriodId::quarter(2025, 1).expect("valid period fixture")].value(),
        100_000.0
    );
    assert_eq!(
        values[&PeriodId::quarter(2025, 2).expect("valid period fixture")].value(),
        110_000.0
    );
    assert_eq!(
        values[&PeriodId::quarter(2025, 3).expect("valid period fixture")].value(),
        121_000.0
    );
}

#[test]
fn test_value_node_with_currency() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::amount(1_000_000.0, Currency::USD).expect("valid amount fixture"),
            )],
        );
        model_builder.build().unwrap()
    };

    let node = model.get_node("revenue").unwrap();
    let values = node.values.as_ref().unwrap();
    let amount = &values[&PeriodId::quarter(2025, 1).expect("valid period fixture")];

    assert!(amount.is_amount());
    assert_eq!(amount.currency(), Some(Currency::USD));
    assert_eq!(amount.value(), 1_000_000.0);
}

#[test]
fn test_value_node_rejects_mixed_currencies_within_one_node() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::amount(100_000.0, Currency::USD).expect("valid amount fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::amount(90_000.0, Currency::EUR).expect("valid amount fixture"),
                ),
            ],
        );
        model_builder.build()
    };

    let message = result.unwrap_err().to_string();
    assert!(
        message.contains("Currency mismatch"),
        "expected currency mismatch error, got: {message}"
    );
}

#[test]
fn test_calculated_node() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder.build().unwrap()
    };

    assert_eq!(model.nodes.len(), 1);
    let node = model.get_node("gross_profit").unwrap();

    assert_eq!(node.node_type, NodeType::Calculated);
    assert_eq!(node.formula_text.as_ref().unwrap(), "revenue - cogs");
    assert!(node.values.is_none());
}

#[test]
fn test_empty_formula_error() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        match model_builder.compute("invalid", "") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::FormulaParse(_)));
}

#[test]
fn test_whitespace_only_formula_error() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        match model_builder.compute("invalid", "   ") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

    assert!(result.is_err());
}

#[test]
fn test_multiple_nodes() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q4", Some("2025Q2")).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.compute("cogs", "revenue * 0.6").unwrap();
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder.build().unwrap()
    };

    assert_eq!(model.nodes.len(), 3);
    assert!(model.has_node("revenue"));
    assert!(model.has_node("cogs"));
    assert!(model.has_node("gross_profit"));
}

#[test]
fn test_metadata() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.with_meta("author", serde_json::json!("Test User"));
        model_builder.with_meta("version", serde_json::json!("1.0.0"));
        model_builder.build().unwrap()
    };

    assert_eq!(model.meta.len(), 2);
    assert_eq!(model.meta["author"], "Test User");
    assert_eq!(model.meta["version"], "1.0.0");
}

#[test]
fn test_schema_version() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.build().unwrap()
    };

    assert_eq!(u32::from(model.schema_version), 1);
}

// Complex Integration Tests

#[test]
fn test_basic_pl_model() {
    let model = {
        let mut model_builder = ModelBuilder::new("Simple P&L");
        model_builder
            .periods("2025Q1..2025Q4", Some("2025Q2"))
            .unwrap();
        model_builder
            // Revenue
            .value(
                "revenue",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(10_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(11_000_000.0),
                    ),
                ],
            );
        model_builder
            // COGS as 60% of revenue
            .compute("cogs", "revenue * 0.6")
            .unwrap();
        model_builder
            // Operating expenses
            .value(
                "opex",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(2_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(2_100_000.0),
                    ),
                ],
            );
        model_builder
            // Derived metrics
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder
            .compute("operating_income", "gross_profit - opex")
            .unwrap();
        model_builder
            .compute("gross_margin", "gross_profit / revenue")
            .unwrap();
        model_builder.build().unwrap()
    };

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
    let model = {
        let mut model_builder = ModelBuilder::new("Multi-currency");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "usd_revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::amount(1_000_000.0, Currency::USD).expect("valid amount fixture"),
            )],
        );
        model_builder.value(
            "eur_revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::amount(900_000.0, Currency::EUR).expect("valid amount fixture"),
            )],
        );
        model_builder.build().unwrap()
    };

    let usd_node = model.get_node("usd_revenue").unwrap();
    let eur_node = model.get_node("eur_revenue").unwrap();

    let usd_values = usd_node.values.as_ref().unwrap();
    let eur_values = eur_node.values.as_ref().unwrap();

    assert_eq!(
        usd_values[&PeriodId::quarter(2025, 1).expect("valid period fixture")].currency(),
        Some(Currency::USD)
    );
    assert_eq!(
        eur_values[&PeriodId::quarter(2025, 1).expect("valid period fixture")].currency(),
        Some(Currency::EUR)
    );
}

// NodeSpec Builder Method Tests

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
    values.insert(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        AmountOrScalar::scalar(100.0),
    );
    values.insert(
        PeriodId::quarter(2025, 2).expect("valid period fixture"),
        AmountOrScalar::scalar(110.0),
    );

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
    let lognormal = ForecastSpec::log_normal(11.5, 0.15, 42);
    assert_eq!(lognormal.method, ForecastMethod::LogNormal);
}

// AmountOrScalar Tests

#[test]
fn test_amount_or_scalar_from_conversions() {
    use finstack_quant_core::money::Money;

    // Test From<f64>
    let scalar: AmountOrScalar = 42.0.into();
    assert!(scalar.is_scalar());
    assert_eq!(scalar.value(), 42.0);

    // Test From<Money>
    let money = Money::new(1000.0, Currency::USD).expect("valid money fixture");
    let amount: AmountOrScalar = money.into();
    assert!(amount.is_amount());
    assert_eq!(amount.value(), 1000.0);
    assert_eq!(amount.currency(), Some(Currency::USD));
}

// Node ID Validation Tests

/// Test that node IDs containing the reserved __cs__ prefix are rejected.
/// This prefix is used internally for capital structure references.
#[test]
fn test_reserved_cs_prefix_rejected_in_value() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "__cs__bad_node",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.build()
    };

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
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue__cs__hidden",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.build()
    };

    assert!(result.is_err());
}

/// Test that node IDs starting with __ (double underscore) are rejected.
/// This prefix is reserved for internal use.
#[test]
fn test_double_underscore_prefix_rejected_in_compute() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        match model_builder.compute("__internal_node", "100") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

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
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.value(
            "_single_underscore",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(50.0),
            )],
        );
        model_builder.value(
            "node_with__double_inside",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(25.0),
            )],
        );
        model_builder
            .compute("gross_profit", "revenue * 0.4")
            .unwrap();
        model_builder.build()
    };

    assert!(result.is_ok());
}

/// Test that mixed node builder also validates node IDs.
#[test]
fn test_reserved_prefix_rejected_in_mixed_node() {
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        let mut model_builder = model_builder.mixed("__bad_mixed");
        model_builder.values(&[(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            AmountOrScalar::scalar(100.0),
        )]);
        model_builder.build()
    };

    assert!(result.is_err());
}

#[test]
fn from_spec_roundtrip_preserves_complete_model_state() {
    use finstack_quant_statements::types::CapitalStructureSpec;
    use indexmap::indexmap;

    let mut spec = {
        let mut model_builder = ModelBuilder::new("roundtrip");
        model_builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.with_meta("source", serde_json::json!({"system": "test"}));
        model_builder.build().unwrap()
    };
    spec.capital_structure = Some(CapitalStructureSpec {
        debt_instruments: Vec::new(),
        meta: indexmap! { "desk".into() => serde_json::json!("credit") },
        reporting_currency: Some(Currency::USD),
        fx_policy: None,
        waterfall: None,
    });
    let expected = serde_json::to_value(&spec).unwrap();

    let rebuilt = {
        let model_builder = ModelBuilder::from_spec(spec).unwrap();
        model_builder.build().unwrap()
    };

    assert_eq!(serde_json::to_value(rebuilt).unwrap(), expected);
}

#[test]
fn from_spec_rejects_empty_period_plan() {
    let spec = FinancialModelSpec::new("empty", Vec::new());

    assert!(ModelBuilder::from_spec(spec).is_err());
}
