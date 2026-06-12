//! Tests for engine edge cases and error handling.

use finstack_core::currency::Currency;
use finstack_core::dates::{build_periods, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_scenarios::{
    Compounding, ExecutionContext, OperationSpec, RateBindingSpec, ScenarioEngine, ScenarioSpec,
};
use finstack_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_statements::FinancialModelSpec;
use indexmap::{indexmap, IndexMap};
use time::Month;

#[test]
fn test_empty_operations_list() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "empty".into(),
        name: None,
        description: None,
        operations: vec![], // Empty
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 0);
    assert!(report.warnings.is_empty());
}

#[test]
fn test_multiple_operations_same_target_last_wins() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Apply two shocks to the same equity
    let scenario = ScenarioSpec {
        id: "last_wins".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -10.0, // First: 100 -> 90
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: 20.0, // Second: 90 -> 108
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 2);

    // Both operations applied sequentially
    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            let expected = 100.0 * 0.9 * 1.2; // 108
            assert!((money.amount() - expected).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price scalar"),
    }
}

#[test]
fn test_scenario_composition_same_priority() {
    let s1 = ScenarioSpec {
        id: "s1".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let s2 = ScenarioSpec {
        id: "s2".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["QQQ".into()],
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let composed = engine
        .try_compose(vec![s1, s2])
        .expect("compose should succeed");

    assert_eq!(composed.operations.len(), 2);
    assert_eq!(composed.id, "s1+s2");
}

#[test]
fn test_scenario_composition_different_priorities() {
    let high_priority = ScenarioSpec {
        id: "high".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -5.0,
        }],
        priority: -10, // Lower value = higher priority
        resolution_mode: Default::default(),
    };

    let low_priority = ScenarioSpec {
        id: "low".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["QQQ".into()],
            pct: -10.0,
        }],
        priority: 10,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let composed = engine
        .try_compose(vec![low_priority, high_priority])
        .expect("compose should succeed");

    // High priority should come first
    assert_eq!(composed.operations.len(), 2);
    match &composed.operations[0] {
        OperationSpec::EquityPricePct { ids, pct } => {
            assert_eq!(ids[0], "SPY");
            assert_eq!(*pct, -5.0);
        }
        _ => panic!("Expected EquityPricePct"),
    }
}

#[test]
fn test_warnings_missing_equity() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "missing".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["NONEXISTENT".into()],
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 0);
    assert!(!report.warnings.is_empty());
    assert!(report.warnings[0].to_string().contains("NONEXISTENT"));
}

#[test]
fn test_warnings_attribute_based_operations() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut attrs = IndexMap::new();
    attrs.insert("sector".into(), "Energy".into());

    let scenario = ScenarioSpec {
        id: "attrs".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::InstrumentPricePctByAttr {
                attrs: attrs.clone(),
                pct: -5.0,
            },
            OperationSpec::InstrumentSpreadBpByAttr { attrs, bp: 50.0 },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.warnings.len(), 2);
    assert!(report.warnings[0]
        .to_string()
        .contains("no instruments provided"));
    assert!(report.warnings[1]
        .to_string()
        .contains("no instruments provided"));
}

#[test]
fn test_stmt_forecast_percent_missing_node_warns() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "stmt_missing".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "NOPE".into(),
            pct: 5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 0);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.to_string().contains("NOPE")),
        "expected warning about missing node, got {:?}",
        report.warnings
    );
}

#[test]
fn test_rate_binding_missing_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();

    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test", periods.clone());

    let mut values = IndexMap::new();
    for period in &periods {
        values.insert(period.id, AmountOrScalar::Scalar(0.05));
    }
    let node = NodeSpec::new("InterestRate", NodeType::Value).with_values(values);
    model.add_node(node);

    let rate_bindings = Some(indexmap! {
        "InterestRate".into() => RateBindingSpec {
            node_id: "InterestRate".into(),
            curve_id: "NONEXISTENT_CURVE".into(),
            tenor: "1Y".to_string(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    });

    let scenario = ScenarioSpec {
        id: "test".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert!(!report.warnings.is_empty());
    assert!(report.warnings[0].to_string().contains("NONEXISTENT_CURVE"));
}

#[test]
fn test_rate_binding_missing_node() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let rate_bindings = Some(indexmap! {
        "NONEXISTENT_NODE".into() => RateBindingSpec {
            node_id: "NONEXISTENT_NODE".into(),
            curve_id: "USD_SOFR".into(),
            tenor: "1Y".to_string(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    });

    let scenario = ScenarioSpec {
        id: "test".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert!(!report.warnings.is_empty());
    assert!(report.warnings[0].to_string().contains("NONEXISTENT_NODE"));
}

#[test]
fn test_time_roll_with_apply_shocks_false() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_only".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false, // Should stop after roll
                roll_mode: finstack_scenarios::TimeRollMode::BusinessDays,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -10.0, // Should NOT be applied
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 1, "Only time roll should apply");

    // Date should be rolled
    let expected = base_date + time::Duration::days(31);
    assert_eq!(ctx.as_of, expected);
}

#[test]
fn test_time_roll_with_apply_shocks_true() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_and_shock".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::TimeRollForward {
                period: "1W".into(),
                apply_shocks: true, // Should continue to other ops
                roll_mode: finstack_scenarios::TimeRollMode::BusinessDays,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -10.0, // Should be applied
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

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
    assert_eq!(report.operations_applied, 2, "Both ops should apply");

    // Verify price was shocked
    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!((money.amount() - 90.0).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price scalar"),
    }
}
