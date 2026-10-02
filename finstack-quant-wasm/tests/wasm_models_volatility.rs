//! wasm-bindgen-test suite for `api::models::volatility`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::volatility::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn sabr(alpha: f64, beta: f64, nu: f64, rho: f64) -> JsSabrParameters {
    JsSabrParameters::new(
        JsValue::from(alpha),
        JsValue::from(beta),
        JsValue::from(nu),
        JsValue::from(rho),
        None,
    )
    .expect("params")
}

#[wasm_bindgen_test]
fn sabr_model_computes_atm_vol() {
    let p = sabr(0.2, 1.0, 0.3, -0.2);
    let smile = JsSabrSmile::new(&p, JsValue::from(100.0), JsValue::from(1.0)).expect("smile");
    let atm = smile.atm_vol().expect("atm_vol");
    assert!(atm > 0.0 && atm < 1.0);
}

#[wasm_bindgen_test]
fn sabr_model_exposes_params_getter() {
    let p = sabr(0.2, 0.5, 0.3, -0.2);
    let model = JsSabrModel::new(&p);
    let roundtrip = model.params();
    assert!((roundtrip.alpha() - 0.2).abs() < 1e-12);
    assert!((roundtrip.beta() - 0.5).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn sabr_calibrator_with_tolerance_calibrates() {
    let p = sabr(0.05, 0.5, 0.4, -0.1);
    let strikes = vec![0.01_f64, 0.02, 0.03, 0.04, 0.05];
    let smile = JsSabrSmile::new(&p, JsValue::from(0.03), JsValue::from(1.0)).expect("smile");
    let vols = smile
        .generate_smile(JsValue::from(strikes.clone()))
        .expect("smile");

    // 1e-6 on the vega-weighted SSE objective is attainable within the
    // default iteration budget; tighter tolerances fail loudly under the
    // strict non-convergence semantics of core `minimize` because rho is
    // weakly identified on this near-symmetric strike set.
    let calibrator = JsSabrCalibrator::new()
        .with_tolerance(JsValue::from(1e-6))
        .unwrap();
    let fitted = calibrator
        .calibrate(
            JsValue::from(0.03),
            JsValue::from(strikes),
            JsValue::from(vols.into_vec()),
            JsValue::from(1.0),
            JsValue::from(0.5),
        )
        .expect("calibrate");
    assert!((fitted.beta() - 0.5).abs() < 1e-12);
    assert!(fitted.alpha() > 0.0);
}

#[wasm_bindgen_test]
fn sabr_parameters_reject_non_number_arguments() {
    assert!(JsSabrParameters::new(
        JsValue::from("0.2"),
        JsValue::from(0.5),
        JsValue::from(0.3),
        JsValue::from(-0.2),
        None,
    )
    .is_err());
    assert!(JsSabrParameters::new(
        JsValue::NULL,
        JsValue::from(0.5),
        JsValue::from(0.3),
        JsValue::from(-0.2),
        None,
    )
    .is_err());
}
