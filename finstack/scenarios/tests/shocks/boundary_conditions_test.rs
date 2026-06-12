//! Tests for boundary conditions and edge values.

use finstack_core::currency::Currency;
use finstack_core::dates::{build_periods, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec,
};
use finstack_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_statements::FinancialModelSpec;
use indexmap::IndexMap;
use time::Month;

#[test]
fn test_zero_percent_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(400.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "zero_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: 0.0, // No change
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
    assert_eq!(report.operations_applied, 1);

    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!((money.amount() - 400.0).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price"),
    }
}

#[test]
fn test_negative_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "negative_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -50.0, // Large negative shock
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
    assert_eq!(report.operations_applied, 1);

    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!((money.amount() - 50.0).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price"),
    }
}

#[test]
fn test_very_large_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "large_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: 500.0, // 5x increase
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
    assert_eq!(report.operations_applied, 1);

    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!((money.amount() - 600.0).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price"),
    }
}

#[test]
fn test_negative_100_percent_shock_on_equity_is_accepted() {
    // Equity shocks at or below -100% are legitimate tail-risk stress (default
    // events, fair-value floors). The engine accepts them; only FX shocks are
    // rejected at -100% to prevent NaN propagation through triangulation.
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "full_loss_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -100.0,
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

    let report = engine
        .apply(&scenario, &mut ctx)
        .expect("equity -100% shock must be accepted (relaxed pct floor)");
    assert_eq!(report.operations_applied, 1);
}

#[test]
fn test_shock_beyond_negative_100_percent_on_equity_is_accepted() {
    // Equity stress can model a "wipeout to negative" (write-down beyond zero)
    // for risk-system testing; downstream pricers handle negative prices.
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "beyond_full_loss".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -150.0,
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

    let report = engine
        .apply(&scenario, &mut ctx)
        .expect("equity -150% shock must be accepted (relaxed pct floor)");
    assert_eq!(report.operations_applied, 1);
}

#[test]
fn test_negative_100_percent_fx_shock_is_rejected() {
    // FX shocks remain rejected at <= -100% because driving a spot rate to
    // zero would propagate NaNs through triangulation. This is the one
    // category where the strict `check_pct_floor` still applies.
    use finstack_core::currency::Currency;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "fx_full_loss".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::MarketFxPct {
            base: Currency::EUR,
            quote: Currency::USD,
            pct: -100.0,
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

    let err = engine
        .apply(&scenario, &mut ctx)
        .expect_err("FX -100% shock must still be rejected by validation");
    assert!(err.to_string().contains("greater than -100%"));
}

#[test]
fn test_shock_nonexistent_market_data() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "missing_data".into(),
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
}

#[test]
fn test_shock_nonexistent_statement_node() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "missing_node".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "NONEXISTENT".into(),
            pct: 10.0,
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

    let report = engine
        .apply(&scenario, &mut ctx)
        .expect("apply completes; missing statement nodes surface as warnings");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.to_string().contains("NONEXISTENT")),
        "expected warning for missing statement node, got {:?}",
        report.warnings
    );
}

#[test]
fn test_curve_parallel_shock_zero_bp() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "zero_bp".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 0.0,
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
    assert_eq!(report.operations_applied, 1);
}

#[test]
fn test_statement_shock_negative_percent() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();

    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test", periods.clone());

    let mut values = IndexMap::new();
    for period in &periods {
        values.insert(period.id, AmountOrScalar::Scalar(100.0));
    }

    model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(values));

    let scenario = ScenarioSpec {
        id: "negative_stmt".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: -30.0,
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
    assert_eq!(report.operations_applied, 1);

    let revenue = model.get_node("Revenue").unwrap();
    let val = revenue.values.as_ref().unwrap().values().next().unwrap();
    match val {
        AmountOrScalar::Scalar(s) => {
            assert!((s - 70.0).abs() < 1e-6);
        }
        AmountOrScalar::Amount(_) => panic!("Expected scalar"),
    }
}

#[test]
fn test_curve_shock_nonexistent_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "missing_curve".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "NONEXISTENT_CURVE".into(),
            discount_curve_id: None,
            bp: 50.0,
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

    let result = engine.apply(&scenario, &mut ctx);
    assert!(result.is_err(), "Should error on missing curve");
}

#[test]
fn test_statement_assign_extreme_value() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();

    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test", periods.clone());

    let mut values = IndexMap::new();
    for period in &periods {
        values.insert(period.id, AmountOrScalar::Scalar(100.0));
    }

    model.add_node(NodeSpec::new("TestNode", NodeType::Value).with_values(values));

    let scenario = ScenarioSpec {
        id: "extreme_assign".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastAssign {
            node_id: "TestNode".into(),
            value: 1_000_000_000.0, // Very large
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
    assert_eq!(report.operations_applied, 1);

    let node = model.get_node("TestNode").unwrap();
    let val = node.values.as_ref().unwrap().values().next().unwrap();
    match val {
        AmountOrScalar::Scalar(s) => {
            assert!((s - 1_000_000_000.0).abs() < 1e-6);
        }
        AmountOrScalar::Amount(_) => panic!("Expected scalar"),
    }
}
