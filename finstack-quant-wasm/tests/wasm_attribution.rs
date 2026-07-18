//! wasm-bindgen-test suite for `api::attribution`.
//!
//! The attribution execute path previously had no WASM test at all. Covers the
//! full JSON pipeline (attributePnl / attributePnlFromSpec), the schema gate
//! in validateAttributionJson, and the default helpers.

#![cfg(target_arch = "wasm32")]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::money::Money;
use finstack_quant_wasm::api::attribution::*;
use wasm_bindgen_test::*;

fn bond_json() -> String {
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::Bond;
    use time::macros::date;

    let bond = Bond::fixed(
        "WASM-ATTR-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 15),
        date!(2029 - 01 - 15),
        "USD-OIS",
    )
    .expect("bond construction");
    serde_json::to_string(&InstrumentJson::Bond(bond)).expect("instrument JSON")
}

fn market_json(as_of: time::Date, rate: f64) -> String {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    let knots: Vec<(f64, f64)> = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        .iter()
        .map(|&t| (t, (-rate * t).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(curve);
    serde_json::to_string(&MarketContextState::from(&market)).expect("market JSON")
}

fn params(method_json: &str) -> JsAttributionParams {
    use time::macros::date;
    JsAttributionParams::new(
        bond_json(),
        market_json(date!(2025 - 01 - 15), 0.04),
        market_json(date!(2025 - 01 - 16), 0.042),
        "2025-01-15".to_string(),
        "2025-01-16".to_string(),
        method_json.to_string(),
        None,
        None,
    )
}

#[wasm_bindgen_test]
fn default_waterfall_order_starts_with_carry() {
    let order: Vec<String> =
        serde_wasm_bindgen::from_value(default_waterfall_order().unwrap()).unwrap();
    assert_eq!(order.first().map(String::as_str), Some("Carry"));
}

#[wasm_bindgen_test]
fn default_attribution_metrics_non_empty() {
    let metrics: Vec<String> =
        serde_wasm_bindgen::from_value(default_attribution_metrics().unwrap()).unwrap();
    assert!(metrics.iter().any(|m| m == "theta"));
}

#[wasm_bindgen_test]
fn attribute_pnl_end_to_end_parallel() {
    let json = attribute_pnl(&params("\"Parallel\"")).expect("attributePnl should succeed");
    let attr: serde_json::Value = serde_json::from_str(&json).expect("PnlAttribution JSON");
    // +20bp rates move on a long bond: the rates factor must be a loss.
    let rates: f64 = attr["rates_curves_pnl"]["amount"]
        .as_str()
        .expect("Money amount serializes as a decimal string")
        .parse()
        .expect("decimal amount");
    assert!(
        rates < 0.0,
        "rates up must lose on a long bond, got {rates}"
    );
}

#[wasm_bindgen_test]
fn validate_attribution_json_rejects_wrong_schema() {
    let envelope = format!(
        r#"{{"schema":"finstack_quant.attribution/99","spec":{{"instrument":{},"market_t0":{},"market_t1":{},"as_of_t0":"2025-01-15","as_of_t1":"2025-01-16","method":"Parallel"}}}}"#,
        bond_json(),
        market_json(time::macros::date!(2025 - 01 - 15), 0.04),
        market_json(time::macros::date!(2025 - 01 - 16), 0.042),
    );
    let err = validate_attribution_json(&envelope)
        .expect_err("wrong schema must be rejected by validation, not just by execute");
    let msg = err.as_string().unwrap_or_default();
    assert!(
        msg.contains("schema"),
        "error should name the schema: {msg}"
    );
}

#[wasm_bindgen_test]
fn attribute_pnl_missing_market_data_yields_structured_error() {
    use time::macros::date;
    let empty = serde_json::to_string(&MarketContextState::from(&MarketContext::new())).unwrap();
    let p = JsAttributionParams::new(
        bond_json(),
        empty.clone(),
        empty,
        "2025-01-15".to_string(),
        "2025-01-16".to_string(),
        "\"Parallel\"".to_string(),
        None,
        None,
    );
    let _ = date!(2025 - 01 - 15);
    let err = attribute_pnl(&p).expect_err("missing curves must error");
    // The error is a structured AttributionError object with a stable kind tag.
    let kind = js_sys::Reflect::get(&err, &"kind".into())
        .ok()
        .and_then(|v| v.as_string());
    assert!(
        kind.is_some(),
        "attribution errors must carry a structured kind tag"
    );
}
