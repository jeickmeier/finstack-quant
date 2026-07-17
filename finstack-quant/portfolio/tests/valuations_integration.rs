//! Integration tests for portfolio ↔ valuations public API.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentJson;
use std::sync::Arc;
use time::macros::date;

#[test]
fn bond_position_from_json_spec_matches_typed_pricing() {
    let as_of = base_date();
    let market = market_with_usd();
    let bond = Bond::fixed(
        "BOND_JSON",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        "USD",
    )
    .expect("bond");

    let pv_typed = bond.value(&market, as_of).expect("typed pv").amount();

    let spec_bond = InstrumentJson::Bond(bond);
    let pos = Position::from_spec(finstack_quant_portfolio::position::PositionSpec {
        position_id: "P1".into(),
        entity_id: "E1".into(),
        instrument_id: "BOND_JSON".into(),
        instrument_spec: Some(spec_bond),
        quantity: 1.0,
        unit: PositionUnit::Units,
        book_id: None,
        attributes: Default::default(),
        meta: Default::default(),
    })
    .expect("position from spec");

    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(finstack_quant_core::currency::Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(pos)
        .build()
        .expect("portfolio");

    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &FinstackConfig::default(),
        &Default::default(),
    )
    .expect("value portfolio");

    let pv_portfolio = valuation
        .get_position_value("P1")
        .expect("position pv")
        .value_native
        .amount();

    assert!(
        (pv_typed - pv_portfolio).abs() < 1e-6,
        "JSON spec position should match typed bond PV: typed={pv_typed}, portfolio={pv_portfolio}"
    );
}

#[test]
fn portfolio_valuation_stamps_caller_config() {
    let as_of = base_date();
    let market = market_with_usd();
    let bond = Bond::fixed(
        "BOND_CONFIG",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        "USD",
    )
    .expect("bond");
    let position = Position::new(
        "P_CONFIG",
        "E_CONFIG",
        "BOND_CONFIG",
        Arc::new(bond),
        1.0,
        PositionUnit::Units,
    )
    .expect("position");
    let portfolio = PortfolioBuilder::new("PF_CONFIG")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E_CONFIG"))
        .position(position)
        .build()
        .expect("portfolio");
    let mut config = FinstackConfig::default();
    config
        .rounding
        .output_scale
        .overrides
        .insert(Currency::USD, 4);

    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .expect("value portfolio");
    let result = valuation
        .get_position_value("P_CONFIG")
        .and_then(|value| value.valuation_result.as_ref())
        .expect("position valuation result");

    assert_eq!(
        result
            .meta
            .rounding
            .output_scale_by_ccy
            .get(&Currency::USD)
            .copied(),
        Some(4),
        "portfolio pricing must propagate the caller's FinstackConfig",
    );
}
