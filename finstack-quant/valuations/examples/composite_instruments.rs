//! Runnable cross-asset composite definitions.
//!
//! Run with:
//! `cargo run -p finstack-quant-valuations --example composite_instruments`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward;
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFutureSpecs,
};
use finstack_quant_valuations::instruments::{
    CommodityUnderlyingParams, CompositeLegSpec, CompositeSpec, InstrumentEnvelope, InstrumentJson,
    RebalanceRule, WeightingMethod,
};
use time::macros::date;

fn treasury_future(
    id: &str,
    specs: BondFutureSpecs,
) -> finstack_quant_core::Result<InstrumentJson> {
    let mut future = BondFuture::example()?;
    future.id = InstrumentId::new(id);
    future.contract_specs = specs;
    Ok(InstrumentJson::BondFuture(Box::new(future)))
}

fn rates_examples() -> finstack_quant_core::Result<(CompositeSpec, CompositeSpec)> {
    let two_year = treasury_future("TU", BondFutureSpecs::ust_2y())?;
    let five_year = treasury_future("FV", BondFutureSpecs::ust_5y())?;
    let ten_year = treasury_future("TY", BondFutureSpecs::ust_10y())?;

    let steepener = CompositeSpec::new(
        "USD.2s10s",
        Currency::USD,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        vec![
            CompositeLegSpec::new("TU", two_year.clone(), 1.0),
            CompositeLegSpec::new("TY", ten_year.clone(), -1.0),
        ],
        WeightingMethod::dv01_neutral("TU", 1.0),
        RebalanceRule::Manual,
    );

    let butterfly = CompositeSpec::new(
        "USD.2s5s10s",
        Currency::USD,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        vec![
            CompositeLegSpec::new("TU", two_year, -1.0),
            CompositeLegSpec::new("FV", five_year, 1.0),
            CompositeLegSpec::new("TY", ten_year, -1.0),
        ],
        WeightingMethod::dv01_neutral("FV", 1.0),
        RebalanceRule::Manual,
    );

    steepener.validate()?;
    butterfly.validate()?;
    Ok((steepener, butterfly))
}

fn commodity_spread() -> finstack_quant_core::Result<InstrumentEnvelope> {
    let mut wti = CommodityForward::example();
    wti.id = InstrumentId::new("WTI");

    let mut brent = CommodityForward::example();
    brent.id = InstrumentId::new("BRENT");
    brent.underlying = CommodityUnderlyingParams::new("Energy", "CO", "BBL", Currency::USD);
    brent.forward_curve_id = "BRENT-FORWARD".into();

    let spread = CompositeSpec::new(
        "BRENT-WTI",
        Currency::USD,
        Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        vec![
            CompositeLegSpec::new("BRENT", InstrumentJson::CommodityForward(brent), 1.0),
            CompositeLegSpec::new("WTI", InstrumentJson::CommodityForward(wti), -1.0),
        ],
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    )
    .initialize_fixed(date!(2025 - 01 - 02))?
    .instrument;

    Ok(InstrumentEnvelope::new(InstrumentJson::Composite(
        Box::new(spread),
    )))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (steepener, butterfly) = rates_examples()?;
    let commodity = commodity_spread()?;

    println!(
        "2s10s specification:\n{}",
        serde_json::to_string_pretty(&steepener)?
    );
    println!(
        "2s5s10s specification:\n{}",
        serde_json::to_string_pretty(&butterfly)?
    );
    println!(
        "Brent-WTI resolved instrument:\n{}",
        serde_json::to_string_pretty(&commodity)?
    );
    Ok(())
}
