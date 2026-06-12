//! Integration tests for statement shock functionality.

use finstack_core::dates::{build_periods, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_scenarios::{ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec};
use finstack_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_statements::FinancialModelSpec;
use indexmap::IndexMap;
use time::Month;

#[test]
fn test_statement_forecast_percent() {
    // Setup market
    let mut market = MarketContext::new();

    // Setup model with explicit values
    let period_plan = build_periods("2025Q1..Q4", None).unwrap();
    let periods = period_plan.periods;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut model = FinancialModelSpec::new("test", periods.clone());

    // Add a revenue node with explicit values
    let mut revenue_values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        revenue_values.insert(period.id, AmountOrScalar::Scalar(100.0 * (i as f64 + 1.0)));
    }

    let revenue_node = NodeSpec::new("Revenue", NodeType::Value).with_values(revenue_values);

    model.add_node(revenue_node);

    // Verify initial values
    let initial_values: Vec<f64> = model
        .get_node("Revenue")
        .unwrap()
        .values
        .as_ref()
        .unwrap()
        .values()
        .map(|v| match v {
            AmountOrScalar::Scalar(s) => *s,
            AmountOrScalar::Amount(_) => 0.0,
        })
        .collect();
    assert_eq!(initial_values, vec![100.0, 200.0, 300.0, 400.0]);

    // Create scenario with -10% revenue shock
    let scenario = ScenarioSpec {
        id: "revenue_shock".into(),
        name: Some("Revenue Shock".into()),
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    // Apply scenario
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Verify shocked values (-10%)
    let shocked_values: Vec<f64> = model
        .get_node("Revenue")
        .unwrap()
        .values
        .as_ref()
        .unwrap()
        .values()
        .map(|v| match v {
            AmountOrScalar::Scalar(s) => *s,
            AmountOrScalar::Amount(_) => 0.0,
        })
        .collect();

    assert_eq!(shocked_values, vec![90.0, 180.0, 270.0, 360.0]);
}

#[test]
fn test_statement_forecast_assign() {
    // Setup market
    let mut market = MarketContext::new();

    // Setup model with explicit values
    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut model = FinancialModelSpec::new("test", periods.clone());

    // Add a node with explicit values
    let mut values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        values.insert(period.id, AmountOrScalar::Scalar(100.0 * (i as f64 + 1.0)));
    }

    let node = NodeSpec::new("TestNode", NodeType::Value).with_values(values);
    model.add_node(node);

    // Create scenario to assign fixed value
    let scenario = ScenarioSpec {
        id: "assign_shock".into(),
        name: Some("Assign Shock".into()),
        description: None,
        operations: vec![OperationSpec::StmtForecastAssign {
            node_id: "TestNode".into(),
            value: 500.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    // Apply scenario
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Verify all values are now 500.0
    let shocked_values: Vec<f64> = model
        .get_node("TestNode")
        .unwrap()
        .values
        .as_ref()
        .unwrap()
        .values()
        .map(|v| match v {
            AmountOrScalar::Scalar(s) => *s,
            AmountOrScalar::Amount(_) => 0.0,
        })
        .collect();

    assert!(shocked_values.iter().all(|&v| (v - 500.0).abs() < 1e-6));
}
