//! wasm-bindgen-test suite for `api::core` currency, date and money bindings.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::currency::*;
use finstack_quant_wasm::api::core::dates::*;
use finstack_quant_wasm::api::core::money::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn currency(code: &str) -> JsCurrency {
    JsCurrency::new(JsValue::from(code)).expect("valid currency code")
}

#[wasm_bindgen_test]
fn json_roundtrip() {
    let c = currency("GBP");
    let json = c.to_json().expect("serialize");
    let c2 = JsCurrency::from_json(JsValue::from(&json)).expect("deserialize");
    assert_eq!(c2.code(), "GBP");
}

fn epoch(y: i32, m: u8, d: u8) -> i32 {
    let month = time::Month::try_from(m).expect("valid month");
    let date = finstack_quant_core::dates::create_date(y, month, d).expect("valid date");
    finstack_quant_core::dates::days_since_epoch(date)
}

fn jan15() -> i32 {
    epoch(2024, 1, 15)
}

fn jul15() -> i32 {
    epoch(2024, 7, 15)
}

#[wasm_bindgen_test]
fn thirty_e_360_isda_uses_termination_context() {
    let start = epoch(2025, 1, 31);
    let end = epoch(2025, 2, 28);
    let day_count = JsDayCount::thirty_e360_isda();
    let regular = day_count
        .year_fraction(
            JsValue::from(start),
            JsValue::from(end),
            &JsDayCountContext::new(),
        )
        .expect("regular period");
    let terminal_ctx = JsDayCountContext::new()
        .with_end_is_termination_date(JsValue::from(true))
        .expect("context");
    let terminal = day_count
        .year_fraction(JsValue::from(start), JsValue::from(end), &terminal_ctx)
        .expect("terminal period");

    assert!((regular - 30.0 / 360.0).abs() < 1e-12);
    assert!((terminal - 28.0 / 360.0).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn daycount_from_string() {
    let day_count = JsDayCount::new(JsValue::from("act_360")).expect("valid");
    assert_eq!(day_count.to_string(), "act_360");
}

#[wasm_bindgen_test]
fn calendar_days() {
    let days =
        JsDayCount::calendar_days(JsValue::from(jan15()), JsValue::from(jul15())).expect("valid");
    assert_eq!(days, (jul15() - jan15()) as i64);
}

#[wasm_bindgen_test]
fn tenor_parse() {
    let t = JsTenor::new(JsValue::from("3M")).expect("valid");
    assert_eq!(t.count(), 3);
    assert!(t.to_years() > 0.24 && t.to_years() < 0.26);
}

#[wasm_bindgen_test]
fn tenor_parse_year() {
    let t = JsTenor::new(JsValue::from("1Y")).expect("valid");
    assert!((t.to_years() - 1.0).abs() < 0.01);
}

#[wasm_bindgen_test]
fn create_date_valid() {
    let e = create_date(JsValue::from(2024), JsValue::from(1), JsValue::from(15)).expect("valid");
    assert_eq!(e, jan15());
}

#[wasm_bindgen_test]
fn date_from_epoch_days_roundtrip() {
    let parts = date_from_epoch_days(JsValue::from(jan15())).expect("valid");
    assert_eq!(parts, vec![2024, 1, 15]);
}

fn usd() -> JsCurrency {
    currency("USD")
}

#[wasm_bindgen_test]
fn json_roundtrip_binding() {
    let m = JsMoney::new(1234.56, &usd()).expect("valid");
    let json = m.to_json().expect("serialize");
    let back = JsMoney::from_json(JsValue::from(&json)).expect("deserialize");
    assert_eq!(back.amount_decimal(), m.amount_decimal());
    assert_eq!(back.currency().code(), "USD");
}
