//! wasm-bindgen-test suite for the typed `Bond` / `TermLoan` classes in
//! `finstack_quant_wasm::api::valuations::fixed_income`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::dates::{JsDayCount, JsTenor};
use finstack_quant_wasm::api::core::money::JsMoney;
use finstack_quant_wasm::api::core::types::{JsBps, JsRate};
use finstack_quant_wasm::api::valuations::fixed_income::{JsBond, JsTermLoan};
use finstack_quant_wasm::api::valuations::pricing::price_instrument;
use wasm_bindgen_test::*;

fn usd_money(amount: f64) -> JsMoney {
    let usd =
        finstack_quant_wasm::api::core::currency::JsCurrency::new("USD").expect("USD currency");
    JsMoney::new(amount, &usd).expect("money")
}

fn fixed_bond() -> JsBond {
    JsBond::fixed(
        "BOND-1",
        &usd_money(1_000_000.0),
        &JsRate::new(0.05).expect("rate"),
        "2024-01-01",
        "2034-01-01",
        "USD-OIS",
    )
    .expect("fixed bond")
}

fn market_context_json() -> String {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    let base = time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(disc);
    serde_json::to_string(&ctx).unwrap()
}

/// Strip the wall-clock `meta.timestamp` before comparing two results.
fn without_timestamp(result_json: &str) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_str(result_json).unwrap();
    if let Some(meta) = value.get_mut("meta").and_then(|m| m.as_object_mut()) {
        meta.remove("timestamp");
    }
    value
}

#[wasm_bindgen_test]
fn bond_fixed_to_json_is_tagged_and_matches_rust() {
    let bond = fixed_bond();
    assert_eq!(bond.id(), "BOND-1");
    let json = bond.to_json().expect("toJson");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["type"], "bond");
    assert_eq!(value["spec"]["id"], "BOND-1");

    // Same constructor called directly in Rust serializes identically.
    let rust_bond = finstack_quant_valuations::instruments::Bond::fixed(
        "BOND-1",
        finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        finstack_quant_core::types::Rate::from_decimal(0.05),
        time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
        time::Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    let rust_json = serde_json::to_string(
        &finstack_quant_valuations::instruments::InstrumentJson::Bond(rust_bond),
    )
    .unwrap();
    assert_eq!(json, rust_json);
}

#[wasm_bindgen_test]
fn bond_from_json_round_trip_preserves_fields() {
    let original = fixed_bond().to_json().unwrap();
    let round_tripped = JsBond::from_json(&original).unwrap().to_json().unwrap();
    assert_eq!(original, round_tripped);
}

#[wasm_bindgen_test]
fn bond_floating_constructor_builds_frn() {
    let frn = JsBond::floating(
        "FRN-1",
        &usd_money(1_000_000.0),
        "USD-SOFR-3M",
        &JsBps::new(200.0).unwrap(),
        "2024-01-01",
        "2030-01-01",
        &JsTenor::quarterly(),
        &JsDayCount::act360(),
        "USD-OIS",
    )
    .expect("floating bond");
    assert_eq!(frn.id(), "FRN-1");
    let value: serde_json::Value = serde_json::from_str(&frn.to_json().unwrap()).unwrap();
    assert_eq!(value["type"], "bond");
}

#[wasm_bindgen_test]
fn bond_from_json_rejects_invalid_json_and_wrong_type() {
    assert!(JsBond::from_json("{not valid json").is_err());
    let loan_json = JsTermLoan::example().unwrap().to_json().unwrap();
    assert!(JsBond::from_json(&loan_json).is_err());
}

#[wasm_bindgen_test]
fn bond_typed_to_json_prices_identically_to_handwritten_json() {
    let bond = fixed_bond();
    let market = market_context_json();
    let typed = price_instrument(&bond.to_json().unwrap(), &market, "2024-06-30", "default")
        .expect("price typed");
    let via_json = price_instrument(
        &JsBond::from_json(&bond.to_json().unwrap())
            .unwrap()
            .to_json()
            .unwrap(),
        &market,
        "2024-06-30",
        "default",
    )
    .expect("price via json");
    assert_eq!(without_timestamp(&typed), without_timestamp(&via_json));
}

#[wasm_bindgen_test]
fn term_loan_example_round_trips_and_prices() {
    let loan = JsTermLoan::example().unwrap();
    assert_eq!(loan.id(), "TERM-LOAN-USD-5Y");
    let json = loan.to_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["type"], "term_loan");

    let round_tripped = JsTermLoan::from_json(&json).unwrap().to_json().unwrap();
    assert_eq!(json, round_tripped);

    let market = market_context_json();
    let priced = price_instrument(&json, &market, "2024-06-30", "default").expect("price loan");
    let result: serde_json::Value = serde_json::from_str(&priced).unwrap();
    assert_eq!(result["instrument_id"], "TERM-LOAN-USD-5Y");
}

#[wasm_bindgen_test]
fn term_loan_from_json_rejects_invalid_json_and_wrong_type() {
    assert!(JsTermLoan::from_json("[1, 2").is_err());
    let bond_json = fixed_bond().to_json().unwrap();
    assert!(JsTermLoan::from_json(&bond_json).is_err());
}
