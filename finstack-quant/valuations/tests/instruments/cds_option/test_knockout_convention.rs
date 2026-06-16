//! Tests for explicit CDS option knockout convention.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

#[test]
fn test_no_knockout_includes_pre_expiry_default_value() {
    let as_of = date!(2025 - 01 - 01);
    let discount = flat_discount("USD-OIS", as_of, 0.03);
    let hazard = flat_hazard("HZ-SN", as_of, 0.4, 0.10);
    let market = MarketContext::new().insert(discount).insert(hazard);

    let knockout = CDSOptionBuilder::new()
        .call()
        .strike(100.0)
        .notional(10_000_000.0, Currency::USD)
        .knockout(true)
        .build(as_of);
    let no_knockout = CDSOptionBuilder::new()
        .call()
        .strike(100.0)
        .notional(10_000_000.0, Currency::USD)
        .knockout(false)
        .build(as_of);

    let knockout_pv = knockout.value(&market, as_of).unwrap().amount();
    let no_knockout_pv = no_knockout.value(&market, as_of).unwrap().amount();

    assert!(
        no_knockout_pv > knockout_pv,
        "no-knockout option should be worth more under material pre-expiry default risk: no_knockout={no_knockout_pv}, knockout={knockout_pv}"
    );
}

#[test]
fn test_knockout_default_for_new_instruments_is_false() {
    assert!(!CDSOption::example().unwrap().knockout);
}

#[test]
fn test_existing_single_name_fixture_builder_pins_knockout_true() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().build(as_of);
    assert!(option.knockout);
}
