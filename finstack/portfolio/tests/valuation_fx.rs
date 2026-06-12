//! Valuation fx tests for portfolio.

mod common;

use common::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::{Entity, DUMMY_ENTITY_ID};
use finstack_portfolio::{Error, PortfolioBuilder};
use finstack_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::Duration;

#[test]
fn cross_currency_conversion_uses_fx_matrix() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    // EUR deposit valued with USD base; FX = 1.10 EUR→USD
    let dep = Deposit::builder()
        .id("DEP_EUR".into())
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("EUR".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_EUR",
        DUMMY_ENTITY_ID,
        "DEP_EUR",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .position(position)
        .build()
        .unwrap();

    // Market with EUR curve and FX
    let market = market_with_eur_and_fx(1.10);
    // Sanity: base currency is USD
    assert_eq!(portfolio.base_ccy, Currency::USD);

    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    // With zero-rate curve, PV ~= notional; FX applied to convert to USD
    let pos_val = valuation.get_position_value("POS_EUR").unwrap();
    assert_eq!(pos_val.value_native.currency(), Currency::EUR);
    assert_eq!(pos_val.value_base.currency(), Currency::USD);

    // Verify FX conversion is applied correctly
    // With flat curve, deposit PV ≈ notional (1M EUR)
    // At 1.10 EUR/USD, base value should be ~1.1M USD
    let expected_base = pos_val.value_native.amount() * 1.10;
    assert!(
        (pos_val.value_base.amount() - expected_base).abs() < 1.0,
        "Base value should be native ({}) × FX rate (1.10) = {}, got: {}",
        pos_val.value_native.amount(),
        expected_base,
        pos_val.value_base.amount()
    );
}

#[test]
fn missing_fx_matrix_errors_for_cross_currency() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    // EUR deposit, portfolio base USD, but no FX in market
    let dep = Deposit::builder()
        .id("DEP_EUR".into())
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("EUR".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_EUR",
        DUMMY_ENTITY_ID,
        "DEP_EUR",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .position(position)
        .build()
        .unwrap();

    // Market has only EUR curve, no FX
    let market = market_with_eur();
    let config = FinstackConfig::default();
    let err = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap_err();

    match err {
        Error::MissingMarketData(msg) => assert!(msg.contains("FX")),
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn quantity_scaling_and_entity_totals() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(90);

    let dep = Deposit::builder()
        .id("DEP_USD".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .quote_rate(rust_decimal::Decimal::try_from(0.05).expect("valid literal")) // Add a 5% rate so deposit has non-zero PV
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    // Short position (negative quantity)
    let position = Position::new(
        "POS_SHORT",
        "E1",
        "DEP_USD",
        Arc::new(dep),
        -2.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let pv = valuation.get_position_value("POS_SHORT").unwrap();
    // With a negative quantity and positive instrument PV, the position value should be negative
    assert!(
        pv.value_native.amount().is_sign_negative(),
        "Expected negative position value, got: {}",
        pv.value_native.amount()
    );
    assert!(valuation.get_entity_value("E1").is_some());
}
