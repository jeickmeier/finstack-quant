//! Integration tests for statement shock functionality.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{build_periods, Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_scenarios::{
    Compounding, ExecutionContext, OperationSpec, RateBindingSpec, ScenarioEngine, ScenarioSpec,
    Warning,
};
use finstack_quant_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_quant_statements::FinancialModelSpec;
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
        hazard_bump_mode: Default::default(),
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
        hazard_bump_mode: Default::default(),
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

#[test]
fn forecast_assignment_preserves_currency_and_actual_values() {
    let periods = build_periods("2025Q1..Q2", Some("2025Q1"))
        .expect("periods")
        .periods;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    for currency in [Currency::USD, Currency::EUR] {
        let mut market = MarketContext::new();
        let mut model = FinancialModelSpec::new("revenue", periods.clone());
        model.add_node(
            NodeSpec::new("Revenue", NodeType::Value).with_values(IndexMap::from([
                (
                    periods[0].id,
                    AmountOrScalar::amount(100.0, currency).expect("actual amount"),
                ),
                (
                    periods[1].id,
                    AmountOrScalar::amount(200.0, currency).expect("forecast amount"),
                ),
            ])),
        );
        model.validate_semantics().expect("valid original model");

        let scenario = ScenarioSpec {
            id: "assign_revenue".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::StmtForecastAssign {
                node_id: "Revenue".into(),
                value: 300.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let report = ScenarioEngine::new()
            .apply(&scenario, &mut ctx)
            .expect("apply forecast assignment");
        assert_eq!(report.operations_applied, 1);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let values = model
            .get_node("Revenue")
            .expect("revenue node")
            .values
            .as_ref()
            .expect("explicit values");
        assert_eq!(
            values[&periods[0].id],
            AmountOrScalar::amount(100.0, currency).expect("actual amount")
        );
        assert_eq!(
            values[&periods[1].id],
            AmountOrScalar::amount(300.0, currency).expect("forecast amount")
        );
        model.validate_semantics().expect("valid shocked model");
    }
}

#[test]
fn invalid_rate_conversion_reports_failure_without_mutating_forecasts() {
    let periods = build_periods("2025Q1..Q2", Some("2025Q1"))
        .expect("periods")
        .periods;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    // The negative case has a non-positive accumulation factor. The positive
    // case has a valid quarterly factor but overflows the annual output rate.
    for (simple_rate, compounding) in [
        (-5.0, Compounding::Continuous),
        (1e100, Compounding::Annual),
    ] {
        let forward = ForwardCurve::builder("FWD", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, simple_rate), (2.0, simple_rate)])
            .build()
            .expect("finite forward curve");
        let mut market = MarketContext::new().insert(forward);
        let initial_values = IndexMap::from([
            (periods[0].id, AmountOrScalar::Scalar(0.02)),
            (periods[1].id, AmountOrScalar::Scalar(0.03)),
        ]);
        let mut model = FinancialModelSpec::new("rates", periods.clone());
        model.add_node(NodeSpec::new("Rate", NodeType::Value).with_values(initial_values.clone()));
        model.validate_semantics().expect("valid original model");

        let scenario = ScenarioSpec {
            id: "invalid_rate_binding".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::RateBinding {
                binding: RateBindingSpec {
                    node_id: "Rate".into(),
                    curve_id: "FWD".into(),
                    tenor: "1Y".into(),
                    compounding,
                    day_count: Some(DayCount::Act360),
                },
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let report = ScenarioEngine::new()
            .apply(&scenario, &mut ctx)
            .expect("binding failure is reported as a warning");
        assert_eq!(report.operations_applied, 0);
        assert!(matches!(
            report.warnings.as_slice(),
            [Warning::RateBindingFailed { node_id, curve_id, .. }]
                if node_id == "Rate" && curve_id == "FWD"
        ));
        assert_eq!(
            model.get_node("Rate").expect("rate node").values.as_ref(),
            Some(&initial_values)
        );
        model.validate_semantics().expect("unchanged valid model");
    }
}

#[test]
fn rate_binding_rejects_monetary_nodes_without_mutating_values() {
    let periods = build_periods("2025Q1..Q2", Some("2025Q1"))
        .expect("periods")
        .periods;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let initial_values = IndexMap::from([
        (
            periods[0].id,
            AmountOrScalar::amount(100.0, Currency::USD).expect("actual amount"),
        ),
        (
            periods[1].id,
            AmountOrScalar::amount(200.0, Currency::USD).expect("forecast amount"),
        ),
    ]);

    for validated in [false, true] {
        let forward = ForwardCurve::builder("FWD", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .build()
            .expect("forward curve");
        let mut market = MarketContext::new().insert(forward);
        let mut model = FinancialModelSpec::new("expenses", periods.clone());
        model.add_node(
            NodeSpec::new("InterestExpense", NodeType::Value).with_values(initial_values.clone()),
        );
        if validated {
            model.validate_semantics().expect("valid original model");
        }
        let scenario = ScenarioSpec {
            id: "monetary_rate_binding".into(),
            operations: vec![OperationSpec::RateBinding {
                binding: RateBindingSpec {
                    node_id: "InterestExpense".into(),
                    curve_id: "FWD".into(),
                    tenor: "1Y".into(),
                    compounding: Compounding::Continuous,
                    day_count: None,
                },
            }],
            ..Default::default()
        };
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let report = ScenarioEngine::new()
            .apply(&scenario, &mut ctx)
            .expect("binding failure is reported as a warning");
        assert_eq!(report.operations_applied, 0);
        assert!(matches!(
            report.warnings.as_slice(),
            [Warning::RateBindingFailed { node_id, curve_id, reason }]
                if node_id == "InterestExpense"
                    && curve_id == "FWD"
                    && reason.contains("dimensionless scalar rates")
        ));
        assert_eq!(
            model
                .get_node("InterestExpense")
                .expect("expense node")
                .values
                .as_ref(),
            Some(&initial_values)
        );
        model.validate_semantics().expect("unchanged valid model");
    }
}
