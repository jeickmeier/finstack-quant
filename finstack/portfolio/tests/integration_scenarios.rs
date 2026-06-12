//! Integration scenarios tests for portfolio.

mod common;

use common::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::Entity;
use finstack_portfolio::PortfolioBuilder;
use finstack_scenarios::spec::{CurveKind, OperationSpec, ScenarioSpec};
use finstack_valuations::instruments::rates::deposit::Deposit;
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
        .day_count(finstack_core::dates::DayCount::Act360)
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
    let base_valuation = finstack_portfolio::valuation::value_portfolio(
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

    let (shocked_valuation, report) =
        finstack_portfolio::scenarios::apply_and_revalue(&portfolio, &scenario, &market, &config)
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
