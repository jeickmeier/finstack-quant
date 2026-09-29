//! wasm-bindgen-test suite for `api::models::credit`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_models::credit::{
    DynamicRecoverySpec, EndogenousHazardSpec, MertonModel, ToggleExerciseModel,
};
use finstack_quant_wasm::api::models::credit::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn merton_distance_to_default_matches_native() {
    let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
    let dd_wasm = merton_distance_to_default(JsValue::from(&json), 1.0).expect("dd");
    let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let dd_native = model.distance_to_default(1.0);
    assert!(
        (dd_wasm - dd_native).abs() < 1e-12,
        "WASM dd ({dd_wasm}) must match native ({dd_native})"
    );
}

#[wasm_bindgen_test]
fn merton_implied_spread_matches_native() {
    let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
    let spread_wasm = merton_implied_spread(JsValue::from(&json), 5.0, 0.40).expect("spread");
    let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let spread_native = model.implied_spread(5.0, 0.40).expect("spread");
    assert!(
        (spread_wasm - spread_native).abs() < 1e-12,
        "WASM spread ({spread_wasm}) must match native ({spread_native})"
    );
}

#[wasm_bindgen_test]
fn dynamic_recovery_at_notional_matches_native() {
    let json = dynamic_recovery_constant_json(0.40).expect("spec json");
    let r_wasm = dynamic_recovery_at_notional(JsValue::from(&json), 100.0).expect("r");
    let spec = DynamicRecoverySpec::constant(0.40).expect("spec");
    let r_native = spec.recovery_at_notional(100.0);
    assert!((r_wasm - r_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn endogenous_hazard_at_leverage_matches_native() {
    let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
    let h_wasm = endogenous_hazard_at_leverage(JsValue::from(&json), 2.0).expect("h");
    let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
    let h_native = spec.hazard_at_leverage(2.0);
    assert!((h_wasm - h_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn endogenous_hazard_after_pik_accrual_matches_native() {
    let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
    let h_wasm =
        endogenous_hazard_after_pik_accrual(JsValue::from(&json), 120.0, 66.67).expect("h");
    let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
    let h_native = spec.hazard_after_pik_accrual(120.0, 66.67);
    assert!((h_wasm - h_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn toggle_exercise_optimal_json_accepts_reasonable_path_count() {
    // A normal nested-path count round-trips into a valid model payload.
    let json = toggle_exercise_optimal_json(JsValue::from(10_000), 0.10, 0.25, 0.04, 5.0)
        .expect("model json");
    let model: ToggleExerciseModel = serde_json::from_str(&json).expect("payload must deserialize");
    match model {
        ToggleExerciseModel::OptimalExercise(o) => assert_eq!(o.nested_paths, 10_000),
        other => panic!("expected OptimalExercise, got {other:?}"),
    }
}

#[wasm_bindgen_test]
#[cfg(target_pointer_width = "64")]
fn toggle_exercise_optimal_json_rejects_unsafe_path_count() {
    // A `nested_paths` above Number.MAX_SAFE_INTEGER would round silently
    // when marshaled as an f64; the binding must reject it instead.
    let unsafe_count = crate::utils::MAX_SAFE_JS_INTEGER as usize + 1;
    let result = toggle_exercise_optimal_json(JsValue::from(unsafe_count), 0.10, 0.25, 0.04, 5.0);
    assert!(
        result.is_err(),
        "nested_paths above 2^53-1 must be rejected, not silently rounded"
    );
}
