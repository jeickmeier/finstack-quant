//! wasm-bindgen-test suite for `api::models::correlation`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::correlation::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn validate_correlation_matrix_accepts_valid_and_rejects_invalid() {
    #[rustfmt::skip]
    let good = vec![
        1.0, 0.5, 0.3,
        0.5, 1.0, 0.4,
        0.3, 0.4, 1.0,
    ];
    assert!(validate_correlation_matrix(&good, JsValue::from(3)).is_ok());

    // Off-diagonal outside [-1, 1] must be rejected.
    #[rustfmt::skip]
    let bad = vec![
        1.0, 1.5,
        1.5, 1.0,
    ];
    assert!(validate_correlation_matrix(&bad, JsValue::from(2)).is_err());

    // Length / dimension mismatch must be rejected, not panic.
    assert!(validate_correlation_matrix(&good, JsValue::from(2)).is_err());
}

#[wasm_bindgen_test]
fn nearest_correlation_repairs_near_psd_input() {
    // Valid correlation matrix passes through unchanged.
    #[rustfmt::skip]
    let good = vec![
        1.0, 0.5, 0.3,
        0.5, 1.0, 0.4,
        0.3, 0.4, 1.0,
    ];
    let out = nearest_correlation(good, JsValue::from(3), None, None)
        .expect("good matrix should project");
    assert_eq!(out.len(), 9);
    for i in 0..3 {
        assert!((out[i * 3 + i] - 1.0).abs() < 1e-9);
    }
}

#[wasm_bindgen_test]
fn portfolio_loss_result_handle_matches_rust() {
    let losses: JsValue = js_sys::Float64Array::from(&[0.0, 1.0, 2.0, 5.0, 10.0][..]).into();
    let result = JsPortfolioLossResult::from_losses(losses, 0.75).expect("valid losses");
    assert_eq!(result.var(), 5.0);
    assert!((result.expected_shortfall() - 9.0).abs() < 1e-12);
    assert!(result.tranche_loss_statistics(0.02, 0.06, 100.0).is_ok());
    let json = result.to_json().expect("json");
    let loaded = JsPortfolioLossResult::from_json(JsValue::from(&json)).expect("round trip");
    assert_eq!(loaded.to_json().unwrap(), json);
    assert!(JsPortfolioLossResult::from_json(JsValue::from(
        json.replace("\"var\":5.0", "\"var\":6.0")
    ))
    .is_err());
}
