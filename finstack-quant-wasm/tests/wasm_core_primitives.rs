//! wasm-bindgen-test suite for `api::core` currency, date and money bindings.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::currency::*;
use finstack_quant_wasm::api::core::dates::*;
use finstack_quant_wasm::api::core::money::*;
use finstack_quant_wasm::api::core::types::*;
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
    let m = JsMoney::new(JsValue::from(1234.56), &usd()).expect("valid");
    let json = m.to_json().expect("serialize");
    let back = JsMoney::from_json(JsValue::from(&json)).expect("deserialize");
    assert_eq!(back.amount_decimal(), m.amount_decimal());
    assert_eq!(back.currency().code(), "USD");
}

#[wasm_bindgen_test]
fn construct_and_getters() {
    let m = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    assert!((m.amount() - 10.0).abs() < 1e-10);
    assert_eq!(m.currency().code(), "USD");
}

#[wasm_bindgen_test]
fn add_same_currency() {
    let a = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let b = JsMoney::new(JsValue::from(5.0), &usd()).expect("valid");
    let c = a.add(&b).expect("add");
    assert!((c.amount() - 15.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn sub_same_currency() {
    let a = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let b = JsMoney::new(JsValue::from(3.0), &usd()).expect("valid");
    let c = a.sub(&b).expect("sub");
    assert!((c.amount() - 7.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn mul_scalar() {
    let m = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let scaled = m.mul_scalar(JsValue::from(2.5)).expect("finite factor");
    assert!((scaled.amount() - 25.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn div_scalar() {
    let m = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let half = m.div_scalar(JsValue::from(2.0)).expect("div");
    assert!((half.amount() - 5.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn negate() {
    let m = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let neg = m.negate();
    assert!((neg.amount() + 10.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn to_string_format() {
    let m = JsMoney::new(JsValue::from(10.0), &usd()).expect("valid");
    let s = m.to_string();
    assert!(s.contains("USD"), "expected USD in: {s}");
    assert!(s.contains("10"), "expected 10 in: {s}");
}

#[wasm_bindgen_test]
fn rate_new_roundtrip() {
    let r = JsRate::new(JsValue::from(0.05)).expect("valid");
    assert!((r.as_decimal() - 0.05).abs() < 1e-12);
    assert!((r.as_percent() - 5.0).abs() < 1e-10);
    assert_eq!(r.as_bp(), 500);
}

#[wasm_bindgen_test]
fn rate_from_percent() {
    let r = JsRate::from_percent(JsValue::from(5.0)).expect("valid");
    assert!((r.as_decimal() - 0.05).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn rate_from_bp() {
    let r = JsRate::from_bp(JsValue::from(250.0)).expect("valid");
    assert!((r.as_decimal() - 0.025).abs() < 1e-10);
    assert_eq!(r.as_bp(), 250);
}

#[wasm_bindgen_test]
fn bp_roundtrip() {
    let b = JsBps::new(JsValue::from(25.0)).expect("valid");
    assert!((b.as_decimal() - 0.0025).abs() < 1e-10);
    assert_eq!(b.as_bp(), 25);
}

#[wasm_bindgen_test]
fn percentage_roundtrip() {
    let p = JsPercentage::new(JsValue::from(5.0)).expect("valid");
    assert!((p.as_decimal() - 0.05).abs() < 1e-12);
    assert!((p.as_percent() - 5.0).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn rate_zero() {
    let r = JsRate::new(JsValue::from(0.0)).expect("valid");
    assert_eq!(r.as_decimal(), 0.0);
    assert_eq!(r.as_percent(), 0.0);
    assert_eq!(r.as_bp(), 0);
}

#[wasm_bindgen_test]
fn bp_large_value() {
    let b = JsBps::new(JsValue::from(10_000.0)).expect("valid");
    assert!((b.as_decimal() - 1.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn percentage_zero() {
    let p = JsPercentage::new(JsValue::from(0.0)).expect("valid");
    assert_eq!(p.as_decimal(), 0.0);
}

#[wasm_bindgen_test]
fn rate_negative() {
    let r = JsRate::new(JsValue::from(-0.01)).expect("valid");
    assert!((r.as_decimal() - (-0.01)).abs() < 1e-12);
}
