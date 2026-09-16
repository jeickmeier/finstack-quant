//! wasm-bindgen-test suite for the typed `Bond` / `TermLoan` classes in
//! `finstack_quant_wasm::api::valuations::fixed_income`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::dates::{JsDayCount, JsTenor};
use finstack_quant_wasm::api::core::money::JsMoney;
use finstack_quant_wasm::api::core::types::{JsBps, JsRate};
use finstack_quant_wasm::api::valuations::fixed_income::{JsBond, JsRevolvingCredit, JsTermLoan};
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
        "short_front",
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

fn credit_market_context_json() -> String {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};

    let base = time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)])
        .build()
        .unwrap();
    let hazard = HazardCurve::builder("ACME-HZD")
        .base_date(base)
        .recovery_rate(0.4)
        .knots([(1.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();
    serde_json::to_string(&MarketContext::new().insert(disc).insert(hazard)).unwrap()
}

fn explicit_credit_bond_json(
    id: &str,
    call_price_pct: Option<f64>,
    put_price_pct: Option<f64>,
) -> String {
    let mut envelope: serde_json::Value =
        serde_json::from_str(&fixed_bond().to_json().unwrap()).unwrap();
    let spec = &mut envelope["instrument"]["spec"];
    spec["id"] = serde_json::json!(id);
    spec["credit_curve_id"] = serde_json::json!("ACME-HZD");
    spec["settlement_days"] = serde_json::json!(0);

    if call_price_pct.is_some() || put_price_pct.is_some() {
        let calls = call_price_pct
            .map(|price_pct_of_par| {
                serde_json::json!({
                    "start_date": "2024-06-30",
                    "end_date": "2024-06-30",
                    "price_pct_of_par": price_pct_of_par,
                })
            })
            .into_iter()
            .collect::<Vec<_>>();
        let puts = put_price_pct
            .map(|price_pct_of_par| {
                serde_json::json!({
                    "start_date": "2024-06-30",
                    "end_date": "2024-06-30",
                    "price_pct_of_par": price_pct_of_par,
                })
            })
            .into_iter()
            .collect::<Vec<_>>();
        spec["call_put"] = serde_json::json!({ "calls": calls, "puts": puts });
    }

    serde_json::to_string(&envelope).unwrap()
}

/// Strip the wall-clock `meta.timestamp` before comparing two results.
/// Decode a `priceInstrument` return (a structured JS object, not a JSON
/// string) and drop the wall-clock stamp so two runs are comparable.
///
/// Round-tripping via `JSON.stringify` also catches an ES2015 `Map`
/// regression: a `Map` stringifies to `{}` and would lose every field.
fn without_timestamp(result: &wasm_bindgen::JsValue) -> serde_json::Value {
    let text: String = js_sys::JSON::stringify(result)
        .expect("valuation result must be JSON.stringify-able")
        .into();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        value.get("instrument_id").is_some(),
        "valuation object must retain its fields: {text}"
    );
    if let Some(meta) = value.get_mut("meta").and_then(|m| m.as_object_mut()) {
        meta.remove("timestamp");
    }
    value
}

/// Fractional basis-point input must be rejected, not silently rounded:
/// a 62.5bp FRN margin rounded to whole bp would price differently from
/// the JSON path for the same instrument.
#[wasm_bindgen_test]
fn bp_rejects_fractional_input() {
    assert!(JsBps::new(62.5).is_err());
    assert!(JsRate::from_bp(12.4).is_err());
    // Whole values still construct.
    assert!(JsBps::new(200.0).is_ok());
    assert!(JsRate::from_bp(250.0).is_ok());
}

#[wasm_bindgen_test]
fn bond_fixed_to_json_is_tagged_and_matches_rust() {
    let bond = fixed_bond();
    assert_eq!(bond.id(), "BOND-1");
    let json = bond.to_json().expect("toJson");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["schema"], "finstack_quant.instrument/1");
    assert_eq!(value["instrument"]["type"], "bond");
    assert_eq!(value["instrument"]["spec"]["id"], "BOND-1");

    // Same constructor called directly in Rust serializes identically.
    let rust_bond = finstack_quant_valuations::instruments::Bond::fixed(
        "BOND-1",
        finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
        time::Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    let rust_json = serde_json::to_string(
        &finstack_quant_valuations::instruments::InstrumentEnvelope::new(
            finstack_quant_valuations::instruments::InstrumentJson::Bond(rust_bond),
        ),
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
    assert_eq!(value["instrument"]["type"], "bond");
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
    let typed = price_instrument(
        &bond.to_json().unwrap(),
        &market,
        "2024-06-30",
        Some("default".to_string()),
        None,
        None,
        None,
    )
    .expect("price typed");
    let via_json = price_instrument(
        &JsBond::from_json(&bond.to_json().unwrap())
            .unwrap()
            .to_json()
            .unwrap(),
        &market,
        "2024-06-30",
        Some("default".to_string()),
        None,
        None,
        None,
    )
    .expect("price via json");
    assert_eq!(without_timestamp(&typed), without_timestamp(&via_json));
}

#[wasm_bindgen_test]
fn rates_credit_values_bond_call_and_put_rights() {
    fn price(instrument_json: &str, market_json: &str, model: &str) -> f64 {
        let result = price_instrument(
            instrument_json,
            market_json,
            "2024-06-30",
            Some(model.to_string()),
            None,
            None,
            None,
        )
        .expect("selected bond model must price");
        let parsed = without_timestamp(&result);
        parsed["value"]["amount"]
            .as_f64()
            .or_else(|| parsed["value"]["amount"].as_str()?.parse().ok())
            .expect("numeric valuation amount")
    }

    let market = credit_market_context_json();
    let bullet_json = explicit_credit_bond_json("WASM-RATES-CREDIT-BULLET", None, None);
    let callable_json = explicit_credit_bond_json("WASM-RATES-CREDIT-CALL", Some(80.0), None);
    let puttable_json = explicit_credit_bond_json("WASM-RATES-CREDIT-PUT", None, Some(120.0));
    let bullet = price(&bullet_json, &market, "rates_credit");
    let callable = price(&callable_json, &market, "rates_credit");
    let puttable = price(&puttable_json, &market, "rates_credit");
    let hazard = price(&bullet_json, &market, "hazard_rate");
    let tree = price(&callable_json, &market, "tree");

    assert!(
        0.0 < callable && callable < bullet && bullet < puttable,
        "expected callable < bullet < puttable, got {callable} < {bullet} < {puttable}"
    );
    assert!(0.0 < hazard && hazard < 1_500_000.0);
    assert!(0.0 < tree && tree < 1_500_000.0);

    let callable_json = explicit_credit_bond_json("WASM-HAZARD-REJECTS-CALL", Some(80.0), None);
    let error = price_instrument(
        &callable_json,
        &market,
        "2024-06-30",
        Some("hazard_rate".to_string()),
        None,
        None,
        None,
    )
    .expect_err("hazard_rate must reject embedded exercise rights");
    let message = js_sys::Reflect::get(&error, &wasm_bindgen::JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_default();
    assert!(
        message.contains("non-callable"),
        "unexpected hazard-rate rejection: {message}"
    );
}

#[wasm_bindgen_test]
fn stochastic_rates_credit_result_exports_full_width_seed_as_bigint() {
    // Keep this ID stable: its derived seed is intentionally above
    // Number.MAX_SAFE_INTEGER and forms part of the lossless-export fixture.
    let id = "WASM-HAZARD-BIGINT";
    let mut instrument: serde_json::Value =
        serde_json::from_str(&explicit_credit_bond_json(id, None, None)).unwrap();
    instrument["instrument"]["spec"]["instrument_pricing_overrides"] = serde_json::json!({
        "model_config": {
            "hazard_volatility": 0.01,
            "mc_paths": 2,
            "tree_steps": 4
        }
    });

    let result = price_instrument(
        &serde_json::to_string(&instrument).unwrap(),
        &credit_market_context_json(),
        "2024-06-30",
        Some("rates_credit".to_string()),
        None,
        None,
        None,
    )
    .expect("stochastic rates-credit bond price must serialize");

    let get = |target: &wasm_bindgen::JsValue, key: &str| {
        js_sys::Reflect::get(target, &wasm_bindgen::JsValue::from_str(key))
            .unwrap_or_else(|_| panic!("missing JavaScript property {key}"))
    };
    let details = get(&result, "details");
    let data = get(&details, "data");
    let seed = get(&data, "seed");

    assert_eq!(
        get(&data, "model_key").as_string().as_deref(),
        Some("rates_credit")
    );
    assert_eq!(get(&data, "make_whole_training_paths").as_f64(), Some(0.0));
    assert_eq!(
        get(&data, "make_whole_training_simulated_paths").as_f64(),
        Some(0.0)
    );

    assert_eq!(seed.js_typeof().as_string().as_deref(), Some("bigint"));
    let expected = finstack_quant_models::monte_carlo::seed::derive_seed(
        &finstack_quant_core::types::InstrumentId::from(id),
        "bond_hazard_lsmc",
    );
    assert!(
        expected > 9_007_199_254_740_991,
        "fixture seed must exceed Number.MAX_SAFE_INTEGER"
    );
    let actual = js_sys::BigInt::new(&seed)
        .expect("seed must be a BigInt")
        .to_string(10)
        .expect("base-10 BigInt conversion");
    assert_eq!(String::from(actual), expected.to_string());
}

#[wasm_bindgen_test]
fn term_loan_example_round_trips_and_prices() {
    let loan = JsTermLoan::example().unwrap();
    assert_eq!(loan.id(), "TERM-LOAN-USD-5Y");
    let json = loan.to_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["instrument"]["type"], "term_loan");

    let round_tripped = JsTermLoan::from_json(&json).unwrap().to_json().unwrap();
    assert_eq!(json, round_tripped);

    let market = market_context_json();
    let priced = price_instrument(
        &json,
        &market,
        "2024-06-30",
        Some("default".to_string()),
        None,
        None,
        None,
    )
    .expect("price loan");
    let result = without_timestamp(&priced);
    assert_eq!(result["instrument_id"], "TERM-LOAN-USD-5Y");
}

#[wasm_bindgen_test]
fn term_loan_from_json_rejects_invalid_json_and_wrong_type() {
    assert!(JsTermLoan::from_json("[1, 2").is_err());
    let bond_json = fixed_bond().to_json().unwrap();
    assert!(JsTermLoan::from_json(&bond_json).is_err());
}

#[wasm_bindgen_test]
fn revolving_credit_example_round_trips_through_the_envelope() {
    let facility = JsRevolvingCredit::example().expect("example facility");
    assert_eq!(facility.id(), "RCF-USD-3Y");
    let json = facility.to_json().expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["instrument"]["type"], "revolving_credit");
    let back = JsRevolvingCredit::from_json(&json).expect("parse");
    assert_eq!(back.to_json().expect("serialize again"), json);
}

#[wasm_bindgen_test]
fn revolving_credit_from_json_rejects_other_instrument_types() {
    assert!(JsRevolvingCredit::from_json("{not valid json").is_err());
    let loan_json = JsTermLoan::example().unwrap().to_json().unwrap();
    assert!(JsRevolvingCredit::from_json(&loan_json).is_err());
}
