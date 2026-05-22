//! Tests for complex multi-operation integration scenarios.

use finstack_core::currency::Currency;
use finstack_core::dates::{build_periods, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::fx::FxMatrix;
use finstack_core::money::fx::SimpleFxProvider;
use finstack_core::money::Money;
use finstack_scenarios::{
    Compounding, CurveKind, ExecutionContext, OperationSpec, RateBindingSpec, ScenarioEngine,
    ScenarioSpec, TimeRollMode,
};
use finstack_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_statements::FinancialModelSpec;
use indexmap::{indexmap, IndexMap};
use std::sync::Arc;
use time::Month;

#[test]
fn test_fx_equity_curve_combo() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Setup FX
    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quote(Currency::EUR, Currency::USD, 1.1)
        .expect("valid rate");
    let fx_matrix = FxMatrix::new(fx_provider);

    // Setup curve
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .insert_fx(fx_matrix)
        .insert(curve)
        .insert_price("SPY", MarketScalar::Price(Money::new(400.0, Currency::USD)));

    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "combo".into(),
        name: Some("FX + Equity + Curve Combo".into()),
        description: None,
        operations: vec![
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 5.0,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -15.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 75.0,
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
    assert_eq!(report.operations_applied, 3);

    // Verify all shocks applied
    let fx = market.fx().unwrap();
    let query = finstack_core::money::fx::FxQuery::new(Currency::EUR, Currency::USD, base_date);
    let rate = fx.rate(query).unwrap().rate;
    assert!((rate - 1.155).abs() < 1e-6, "FX should be shocked");

    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!(
                (money.amount() - 340.0).abs() < 1e-6,
                "Equity should be shocked"
            );
        }
        _ => panic!("Expected Price"),
    }

    let curve = market.get_discount("USD_SOFR").unwrap();
    assert!(curve.df(1.0) < 0.98, "Curve should be shocked");
}

#[test]
fn test_statements_rate_bindings_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);

    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test", periods.clone());

    // Add revenue and rate nodes
    let mut revenue_values = IndexMap::new();
    let mut rate_values = IndexMap::new();
    for period in &periods {
        revenue_values.insert(period.id, AmountOrScalar::Scalar(1000.0));
        rate_values.insert(period.id, AmountOrScalar::Scalar(0.02));
    }

    model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(revenue_values));
    model.add_node(NodeSpec::new("InterestRate", NodeType::Value).with_values(rate_values));

    let rate_bindings = Some(indexmap! {
        "InterestRate".into() => RateBindingSpec {
            node_id: "InterestRate".into(),
            curve_id: "USD_SOFR".into(),
            tenor: "1Y".to_string(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    });

    let scenario = ScenarioSpec {
        id: "stmt_curve".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 100.0,
            },
            OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: 10.0,
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
        rate_bindings,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 2);

    // Verify revenue was shocked
    let revenue = model.get_node("Revenue").unwrap();
    let first_val = revenue.values.as_ref().unwrap().values().next().unwrap();
    match first_val {
        AmountOrScalar::Scalar(s) => {
            assert!((s - 1100.0).abs() < 1e-6);
        }
        _ => panic!("Expected scalar"),
    }

    // Verify rate was updated from curve
    let rate = model.get_node("InterestRate").unwrap();
    let first_rate = rate.values.as_ref().unwrap().values().next().unwrap();
    match first_rate {
        AmountOrScalar::Scalar(s) => {
            assert!(*s > 0.02, "Rate should be updated from shocked curve");
        }
        _ => panic!("Expected scalar"),
    }
}

#[test]
fn test_time_roll_with_market_shocks() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(450.0, Currency::USD)));
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_and_shock".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::BusinessDays,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -20.0,
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

    // Date rolled
    let expected_date = base_date + time::Duration::days(31);
    assert_eq!(ctx.as_of, expected_date);

    // Price shocked
    let price = market.get_price("SPY").unwrap();
    match price {
        MarketScalar::Price(money) => {
            assert!((money.amount() - 360.0).abs() < 1e-6);
        }
        _ => panic!("Expected Price"),
    }
}

#[test]
fn test_conflicting_operations_last_wins() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "conflicting".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 25.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 50.0, // This one wins (sequential application)
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

    // Both shocks applied sequentially: +25bp then +50bp = equivalent to +75bp
    let curve = market.get_discount("USD_SOFR").unwrap();
    let df = curve.df(1.0);
    // Original DF(1Y) = 0.98
    // Solve-to-par with settlement_days=0 (for synthetic curve re-calibration)
    let expected_df = 0.972180;
    assert!(
        (df - expected_df).abs() < 1e-4,
        "Expected DF ≈ {:.6} after sequential +25bp and +50bp shocks, got {:.6}",
        expected_df,
        df
    );
}

#[test]
fn test_three_scenario_composition() {
    let s1 = ScenarioSpec {
        id: "high_priority".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -5.0,
        }],
        priority: -10,
        resolution_mode: Default::default(),
    };

    let s2 = ScenarioSpec {
        id: "mid_priority".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["QQQ".into()],
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let s3 = ScenarioSpec {
        id: "low_priority".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["IWM".into()],
            pct: -15.0,
        }],
        priority: 10,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let composed = engine
        .try_compose(vec![s3, s1, s2])
        .expect("compose should succeed"); // Intentionally out of order

    assert_eq!(composed.operations.len(), 3);

    // Verify priority ordering
    match &composed.operations[0] {
        OperationSpec::EquityPricePct { ids, pct } => {
            assert_eq!(ids[0], "SPY");
            assert_eq!(*pct, -5.0);
        }
        _ => panic!("Expected SPY first"),
    }

    match &composed.operations[2] {
        OperationSpec::EquityPricePct { ids, pct } => {
            assert_eq!(ids[0], "IWM");
            assert_eq!(*pct, -15.0);
        }
        _ => panic!("Expected IWM last"),
    }
}

#[test]
fn test_multiple_statement_operations() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();

    let period_plan = build_periods("2025Q1..Q2", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test", periods.clone());

    let mut revenue_values = IndexMap::new();
    let mut cost_values = IndexMap::new();
    for period in &periods {
        revenue_values.insert(period.id, AmountOrScalar::Scalar(1000.0));
        cost_values.insert(period.id, AmountOrScalar::Scalar(600.0));
    }

    model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(revenue_values));
    model.add_node(NodeSpec::new("Cost", NodeType::Value).with_values(cost_values));

    let scenario = ScenarioSpec {
        id: "multi_stmt".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: 15.0,
            },
            OperationSpec::StmtForecastPercent {
                node_id: "Cost".into(),
                pct: 8.0,
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

    // Verify both statements shocked
    let revenue = model.get_node("Revenue").unwrap();
    let rev_val = revenue.values.as_ref().unwrap().values().next().unwrap();
    match rev_val {
        AmountOrScalar::Scalar(s) => {
            assert!((s - 1150.0).abs() < 1e-6);
        }
        _ => panic!("Expected scalar"),
    }

    let cost = model.get_node("Cost").unwrap();
    let cost_val = cost.values.as_ref().unwrap().values().next().unwrap();
    match cost_val {
        AmountOrScalar::Scalar(s) => {
            assert!((s - 648.0).abs() < 1e-6);
        }
        _ => panic!("Expected scalar"),
    }
}
