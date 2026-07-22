//! wasm-bindgen-test suite for `finstack_quant_wasm::api::valuations`.
//!
//! Covers list_standard_metrics and price_instrument_with_metrics
//! which use JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::valuations::pricing::{
    instrument_cashflows_json, list_standard_metrics, price_instrument,
    price_instrument_with_metrics,
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

fn bond_instrument_json() -> String {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::InstrumentJson;

    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
        time::Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    serde_json::to_string(&InstrumentJson::Bond(bond)).unwrap()
}

fn term_loan_instrument_json() -> String {
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    use finstack_quant_valuations::instruments::InstrumentJson;

    let loan = TermLoan::example().expect("term loan example");
    serde_json::to_string(&InstrumentJson::TermLoan(loan)).unwrap()
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

fn structured_credit_instrument_json() -> String {
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, PoolAsset, StochasticDefaultSpec, StochasticPrepaySpec,
        StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
    use time::Month;

    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(1_000_000.0, Currency::USD),
        0.06,
        maturity,
        DayCount::Thirty360,
    ));
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(800_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(200_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .unwrap(),
    ])
    .unwrap();
    let mut sc =
        StructuredCredit::new_abs("ABS-STOCH-PV", pool, tranches, closing, maturity, "USD-OIS")
            .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    sc.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::deterministic(
        sc.credit_model.prepayment_spec.clone(),
    ));
    sc.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::deterministic(
        sc.credit_model.default_spec.clone(),
    ));
    sc.instrument_pricing_overrides = InstrumentPricingOverrides::default().with_mc_paths(1);

    serde_json::to_string(&InstrumentJson::StructuredCredit(Box::new(sc))).unwrap()
}

fn invalid_structured_credit_instrument_json() -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&structured_credit_instrument_json()).unwrap();
    value["spec"]["cleanup_call_pct"] = serde_json::json!(-0.5);
    serde_json::to_string(&value).unwrap()
}

fn fx_option_instrument_json() -> String {
    use finstack_quant_valuations::instruments::fx::FxOption;
    use finstack_quant_valuations::instruments::InstrumentJson;

    let option = FxOption::example().expect("fx option example");
    serde_json::to_string(&InstrumentJson::FxOption(option)).unwrap()
}

fn error_message(error: JsValue) -> String {
    error
        .dyn_into::<js_sys::Error>()
        .expect("binding errors should be JavaScript Error objects")
        .message()
        .into()
}

#[wasm_bindgen_test]
fn list_standard_metrics_returns_non_empty_array() {
    let result = list_standard_metrics().unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(!ids.is_empty());
}

#[wasm_bindgen_test]
fn price_instrument_with_metrics_returns_result() {
    let inst = bond_instrument_json();
    let mkt = market_context_json();
    let metrics = serde_wasm_bindgen::to_value(&vec!["dirty_price".to_string()]).unwrap();
    let result = price_instrument_with_metrics(
        &inst,
        &mkt,
        "2024-01-01",
        "discounting",
        metrics,
        None,
        None,
    )
    .unwrap();
    assert!(!result.is_empty());
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.is_object());
}

#[wasm_bindgen_test]
fn price_instrument_with_metrics_accepts_pricing_options() {
    let inst = bond_instrument_json();
    let mkt = market_context_json();
    let metrics = serde_wasm_bindgen::to_value(&vec!["dirty_price".to_string()]).unwrap();
    let result = price_instrument_with_metrics(
        &inst,
        &mkt,
        "2024-01-01",
        "discounting",
        metrics,
        Some(r#"{"theta_period":"1D"}"#.to_string()),
        None,
    )
    .unwrap();
    assert!(!result.is_empty());
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.is_object());
}

#[wasm_bindgen_test]
fn registered_term_loan_metrics_cross_wasm_json_boundary() {
    let metrics =
        serde_wasm_bindgen::to_value(&vec!["all_in_rate".to_string(), "yt2y".to_string()]).unwrap();
    let result = price_instrument_with_metrics(
        &term_loan_instrument_json(),
        &market_context_json(),
        "2024-01-01",
        "discounting",
        metrics,
        None,
        None,
    )
    .expect("registered custom metrics");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed["measures"].get("all_in_rate").is_some());
    assert!(parsed["measures"].get("yt2y").is_some());
}

#[wasm_bindgen_test]
fn public_json_routes_validate_instrument_before_malformed_market() {
    use finstack_quant_wasm::api::valuations::structured_credit::{
        structured_credit_tranche_breakeven_cdr, structured_credit_tranche_discount_margin,
        structured_credit_tranche_metrics, structured_credit_tranche_oas,
        structured_credit_tranche_scenario_table,
    };

    let instrument = invalid_structured_credit_instrument_json();
    let market = "not-market-json";
    let metrics = serde_wasm_bindgen::to_value(&vec!["not-a-metric".to_string()]).unwrap();

    let errors = [
        price_instrument(&instrument, market, "not-a-date", "not-a-model").unwrap_err(),
        price_instrument_with_metrics(
            &instrument,
            market,
            "not-a-date",
            "not-a-model",
            metrics,
            None,
            None,
        )
        .unwrap_err(),
        instrument_cashflows_json(&instrument, market, "not-a-date", "not-a-model").unwrap_err(),
        structured_credit_tranche_discount_margin(
            &instrument,
            "missing",
            market,
            "not-a-date",
            f64::NAN,
        )
        .unwrap_err(),
        structured_credit_tranche_breakeven_cdr(&instrument, "missing", market, "not-a-date")
            .unwrap_err(),
        structured_credit_tranche_oas(
            &instrument,
            "missing",
            f64::NAN,
            market,
            "not-a-date",
            Some("not-json".to_string()),
        )
        .unwrap_err(),
        structured_credit_tranche_scenario_table(
            &instrument,
            "missing",
            market,
            "not-a-date",
            "not-json",
        )
        .unwrap_err(),
        structured_credit_tranche_metrics(
            &instrument,
            "missing",
            market,
            "not-a-date",
            Some(f64::NAN),
        )
        .unwrap_err(),
    ];

    for error in errors {
        let message = error_message(error);
        assert!(
            message.contains("cleanup_call_pct"),
            "instrument validation should win over malformed market input: {message}"
        );
    }
}

#[wasm_bindgen_test]
fn fx_price_with_metrics_validates_merged_overrides_before_market() {
    use finstack_quant_wasm::api::valuations::fx::JsFxOption;

    let option = JsFxOption::from_json(&fx_option_instrument_json()).unwrap();
    let metrics = serde_wasm_bindgen::to_value(&Vec::<String>::new()).unwrap();
    let error = option
        .price_with_metrics(
            "not-market-json",
            "not-a-date",
            metrics,
            Some("not-a-model".to_string()),
            Some(r#"{"vol_bump_pct":-0.20}"#.to_string()),
            None,
        )
        .unwrap_err();
    let message = error_message(error);

    assert!(
        message.contains("NegativeValue") || message.contains("negative"),
        "merged pricing-option validation should win over malformed market input: {message}"
    );
}

#[wasm_bindgen_test]
fn price_instrument_structured_credit_stochastic_returns_details() {
    let inst = structured_credit_instrument_json();
    let mkt = market_context_json();
    let result =
        price_instrument(&inst, &mkt, "2024-01-01", "structured_credit_stochastic").expect("price");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["details"]["type"], "structured_credit_stochastic");
    let tranches = parsed["details"]["data"]["tranche_results"]
        .as_array()
        .expect("tranche_results array");
    assert_eq!(tranches.len(), 2);
}

#[wasm_bindgen_test]
fn price_instrument_structured_credit_waterfall_rules() {
    // `waterfall_rules` is an additive serde-default field on the deal; the
    // rebuilt wasm binding must accept and price a deal that configures it (an
    // available-funds cap on the senior), reachable through the existing
    // price_instrument entry point with no binding code change.
    let mut value: serde_json::Value =
        serde_json::from_str(&structured_credit_instrument_json()).unwrap();
    value["spec"]["waterfall_rules"] = serde_json::json!({ "afc": { "capped_tranches": ["SR"] } });
    let inst = serde_json::to_string(&value).unwrap();
    let mkt = market_context_json();
    let result =
        price_instrument(&inst, &mkt, "2024-01-01", "structured_credit_stochastic").expect("price");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["details"]["type"], "structured_credit_stochastic");
}

#[wasm_bindgen_test]
fn structured_credit_tranche_metrics_through_json() {
    use finstack_quant_wasm::api::valuations::structured_credit::{
        structured_credit_tranche_breakeven_cdr, structured_credit_tranche_discount_margin,
        structured_credit_tranche_metrics, structured_credit_tranche_oas,
        structured_credit_tranche_scenario_table,
    };

    let inst = structured_credit_instrument_json();
    let mkt = market_context_json();

    let breakeven = structured_credit_tranche_breakeven_cdr(&inst, "SR", &mkt, "2024-01-01")
        .expect("breakeven cdr");
    assert!(breakeven >= 0.0);

    // Discount margin is only defined for floating-rate tranches; "SR" is
    // fixed-rate, so the binding must surface the validation error rather than
    // silently returning a value (parity with the Python negative test).
    let dm_err =
        structured_credit_tranche_discount_margin(&inst, "SR", &mkt, "2024-01-01", 1_000.0)
            .expect_err("discount margin on a fixed-rate tranche should error");
    assert!(format!("{dm_err:?}").to_lowercase().contains("floating"));

    let oas = structured_credit_tranche_oas(&inst, "SR", 99.0, &mkt, "2024-01-01", None)
        .expect("tranche oas");
    let oas_parsed: serde_json::Value = serde_json::from_str(&oas).unwrap();
    assert!(oas_parsed["model_price"].as_f64().expect("model_price") > 0.0);

    let grid = r#"{"cprs":[0.10,0.20],"cdrs":[0.02],"severities":[0.40]}"#;
    let table = structured_credit_tranche_scenario_table(&inst, "SR", &mkt, "2024-01-01", grid)
        .expect("scenario table");
    let table_parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
    assert_eq!(table_parsed["cells"].as_array().expect("cells").len(), 2);

    // Per-tranche metrics bundle: model-price z-spread ~ 0, widening at a cheaper price.
    let tm = structured_credit_tranche_metrics(&inst, "SR", &mkt, "2024-01-01", None)
        .expect("tranche metrics");
    let tm_parsed: serde_json::Value = serde_json::from_str(&tm).unwrap();
    assert_eq!(tm_parsed["tranche_id"], "SR");
    assert!(tm_parsed["pv"].as_f64().expect("pv") > 0.0);
    let tm_cheap = structured_credit_tranche_metrics(&inst, "SR", &mkt, "2024-01-01", Some(95.0))
        .expect("tranche metrics @95");
    let cheap_parsed: serde_json::Value = serde_json::from_str(&tm_cheap).unwrap();
    assert!(
        cheap_parsed["z_spread_bp"].as_f64().expect("z")
            > tm_parsed["z_spread_bp"].as_f64().expect("z")
    );
}

#[wasm_bindgen_test]
fn price_instrument_structured_credit_stochastic_missing_market_data_errors() {
    let inst = structured_credit_instrument_json();
    let empty_market =
        serde_json::to_string(&finstack_quant_core::market_data::context::MarketContext::new())
            .unwrap();
    let err = price_instrument(
        &inst,
        &empty_market,
        "2024-01-01",
        "structured_credit_stochastic",
    )
    .expect_err("missing discount curve should error");
    assert!(format!("{err:?}").contains("USD-OIS"));
}
