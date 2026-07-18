//! Integration scenarios tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_scenarios::spec::{CurveKind, OperationSpec, ScenarioSpec};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::Duration;

#[test]
fn apply_and_revalue_succeeds() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let pos = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();
    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(pos)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();

    // Get base valuation first
    let base_valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let scenario = ScenarioSpec {
        id: "s".to_string(),
        name: Some("s".to_string()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (shocked_valuation, report) = finstack_quant_portfolio::scenarios::apply_and_revalue(
        &portfolio, &scenario, &market, &config,
    )
    .unwrap();
    assert!(report.operations_applied > 0);

    // Verify the shocked valuation differs from base
    // +10bp shift should change deposit value slightly
    let base_total = base_valuation.total_base_ccy.amount();
    let shocked_total = shocked_valuation.total_base_ccy.amount();

    // For a 30-day deposit, +10bp should have a small but measurable impact
    // Don't assert sign as deposits may behave differently than bonds
    assert!(
        (shocked_total - base_total).abs() > 0.01,
        "Scenario should have measurable impact: base={}, shocked={}",
        base_total,
        shocked_total
    );
}

/// End-to-end scenario P&L across a two-entity book: the drill-down must foot
/// to the headline, and the headline must agree with an independently computed
/// stressed-minus-base total.
#[test]
fn scenario_pnl_reconciles_end_to_end() {
    let as_of = base_date();

    let build_deposit = |id: &str, notional: f64, days: i64| {
        Deposit::builder()
            .id(id.into())
            .notional(Money::new(notional, Currency::USD))
            .start_date(as_of)
            .maturity(as_of + Duration::days(days))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .unwrap()
    };

    let pos_a = Position::new(
        "P_A",
        "E_A",
        "D_A",
        Arc::new(build_deposit("D_A", 1_000_000.0, 30)),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let pos_b = Position::new(
        "P_B",
        "E_B",
        "D_B",
        Arc::new(build_deposit("D_B", 2_500_000.0, 180)),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E_A"))
        .entity(Entity::new("E_B"))
        .position(pos_a)
        .position(pos_b)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();

    let scenario = ScenarioSpec {
        id: "pnl".to_string(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (pnl, report) =
        finstack_quant_portfolio::scenarios::scenario_pnl(&portfolio, &scenario, &market, &config)
            .unwrap();

    assert!(report.operations_applied > 0);
    assert_eq!(pnl.by_position.len(), 2);
    assert_eq!(pnl.total.currency(), Currency::USD);

    // The drill-down foots to the headline.
    let drilldown: f64 = pnl.by_position.values().map(|m| m.amount()).sum();
    assert!(
        (drilldown - pnl.total.amount()).abs() < 1e-6,
        "drill-down {drilldown} must foot to total {}",
        pnl.total.amount()
    );

    // The headline agrees with an independently computed stressed-minus-base.
    let base = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let (stressed, _) = finstack_quant_portfolio::scenarios::apply_and_revalue(
        &portfolio, &scenario, &market, &config,
    )
    .unwrap();
    let expected = stressed.total_base_ccy.amount() - base.total_base_ccy.amount();
    assert!(
        (pnl.total.amount() - expected).abs() < 1e-6,
        "total {} must equal stressed-minus-base {expected}",
        pnl.total.amount()
    );

    // A 100bp shock on a real book must move something.
    assert!(pnl.total.amount().abs() > 0.01);
}

/// A scenario with no operations must produce exactly zero P&L in every
/// position and in the total — the deterministic-baseline check.
#[test]
fn scenario_pnl_no_op_scenario_is_flat() {
    let as_of = base_date();

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(as_of + Duration::days(90))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let pos = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();
    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(pos)
        .build()
        .unwrap();

    let scenario = ScenarioSpec {
        id: "noop".to_string(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (pnl, _report) = finstack_quant_portfolio::scenarios::scenario_pnl(
        &portfolio,
        &scenario,
        &market_with_usd(),
        &FinstackConfig::default(),
    )
    .unwrap();

    assert_eq!(pnl.total.amount(), 0.0);
    for (position_id, delta) in &pnl.by_position {
        assert_eq!(delta.amount(), 0.0, "position '{position_id}' must be flat");
    }
}
