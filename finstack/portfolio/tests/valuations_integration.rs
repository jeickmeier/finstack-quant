//! Integration tests for portfolio ↔ valuations public API.

mod common;

use common::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::Entity;
use finstack_portfolio::PortfolioBuilder;
use finstack_valuations::instruments::Bond;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::InstrumentJson;
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
    let pos = Position::from_spec(finstack_portfolio::position::PositionSpec {
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
        .base_ccy(finstack_core::currency::Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(pos)
        .build()
        .expect("portfolio");

    let valuation = finstack_portfolio::valuation::value_portfolio(
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
