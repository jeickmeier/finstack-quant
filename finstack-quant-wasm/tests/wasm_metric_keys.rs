//! wasm-bindgen-test suite for canonical valuation metric-key validation.
#![cfg(target_arch = "wasm32")]

use finstack_quant_core::{currency::Currency, money::Money};
use finstack_quant_valuations::results::ValuationResult;
use finstack_quant_wasm::api::valuations::pricing::validate_valuation_result_json;
use time::macros::date;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn valuation_wire_keys_are_canonical_and_preserve_literal_escape_labels() {
    let result = ValuationResult::stamped(
        "BOND",
        date!(2025 - 01 - 15),
        Money::from((0_i64, Currency::USD)),
    );
    let mut payload = serde_json::to_value(result).expect("fixture");
    for key in ["pv01::USD_x2dOIS", "pv01::curve_xray", "pv01::"] {
        payload["measures"] = serde_json::json!({key: 1.0});
        assert!(validate_valuation_result_json(&payload.to_string()).is_err());
    }
    payload["measures"] = serde_json::json!({
        "pv01::USD-OIS": 1.0, "pv01::USD_x5fx2dOIS": 2.0,
        "pv01::USD_x3a_x3aOIS": 3.0,
    });
    let wire = validate_valuation_result_json(&payload.to_string()).expect("canonical keys");
    let restored: ValuationResult = serde_json::from_str(&wire).expect("result");
    let series = restored.metric_series(&finstack_quant_valuations::metrics::MetricId::Pv01);
    // The validator emits canonical JSON, which sorts object keys.
    assert_eq!(
        series,
        vec![
            (vec!["USD-OIS".to_string()], 1.0),
            (vec!["USD::OIS".to_string()], 3.0),
            (vec!["USD_x2dOIS".to_string()], 2.0),
        ]
    );
}
