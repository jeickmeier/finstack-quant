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
    let json = merton_model_json(
        JsValue::from(100.0),
        JsValue::from(0.20),
        JsValue::from(80.0),
        JsValue::from(0.05),
    )
    .expect("merton json");
    let dd_wasm = merton_distance_to_default(JsValue::from(&json), JsValue::from(1.0)).expect("dd");
    let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let dd_native = model.distance_to_default(1.0);
    assert!(
        (dd_wasm - dd_native).abs() < 1e-12,
        "WASM dd ({dd_wasm}) must match native ({dd_native})"
    );
}

#[wasm_bindgen_test]
fn merton_implied_spread_matches_native() {
    let json = merton_model_json(
        JsValue::from(100.0),
        JsValue::from(0.20),
        JsValue::from(80.0),
        JsValue::from(0.05),
    )
    .expect("merton json");
    let spread_wasm = merton_implied_spread(
        JsValue::from(&json),
        JsValue::from(5.0),
        JsValue::from(0.40),
    )
    .expect("spread");
    let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let spread_native = model.implied_spread(5.0, 0.40).expect("spread");
    assert!(
        (spread_wasm - spread_native).abs() < 1e-12,
        "WASM spread ({spread_wasm}) must match native ({spread_native})"
    );
}

#[wasm_bindgen_test]
fn dynamic_recovery_at_notional_matches_native() {
    let json = dynamic_recovery_constant_json(JsValue::from(0.40)).expect("spec json");
    let r_wasm =
        dynamic_recovery_at_notional(JsValue::from(&json), JsValue::from(100.0)).expect("r");
    let spec = DynamicRecoverySpec::constant(0.40).expect("spec");
    let r_native = spec.recovery_at_notional(100.0);
    assert!((r_wasm - r_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn endogenous_hazard_at_leverage_matches_native() {
    let json = endogenous_hazard_power_law_json(
        JsValue::from(0.10),
        JsValue::from(1.5),
        JsValue::from(2.5),
    )
    .expect("spec json");
    let h_wasm =
        endogenous_hazard_at_leverage(JsValue::from(&json), JsValue::from(2.0)).expect("h");
    let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
    let h_native = spec.hazard_at_leverage(2.0);
    assert!((h_wasm - h_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn endogenous_hazard_after_pik_accrual_matches_native() {
    let json = endogenous_hazard_power_law_json(
        JsValue::from(0.10),
        JsValue::from(1.5),
        JsValue::from(2.5),
    )
    .expect("spec json");
    let h_wasm = endogenous_hazard_after_pik_accrual(
        JsValue::from(&json),
        JsValue::from(120.0),
        JsValue::from(66.67),
    )
    .expect("h");
    let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
    let h_native = spec.hazard_after_pik_accrual(120.0, 66.67);
    assert!((h_wasm - h_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn toggle_exercise_optimal_json_accepts_reasonable_path_count() {
    // A normal nested-path count round-trips into a valid model payload.
    let json = toggle_exercise_optimal_json(
        JsValue::from(10_000),
        JsValue::from(0.10),
        JsValue::from(0.25),
        JsValue::from(0.04),
        JsValue::from(5.0),
    )
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

#[wasm_bindgen_test]
fn merton_from_equity_roundtrips() {
    let m_known = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let (equity, equity_vol) = m_known.try_implied_equity(1.0).expect("equity");
    let json = merton_from_equity_json(
        JsValue::from(equity),
        JsValue::from(equity_vol),
        JsValue::from(80.0),
        JsValue::from(0.05),
        JsValue::from(0.0),
        JsValue::from(1.0),
    )
    .expect("json");
    let m_cal: MertonModel = serde_json::from_str(&json).expect("deserialize");
    assert!(
        (m_cal.asset_value() - m_known.asset_value()).abs() < 1e-6,
        "asset value roundtrip"
    );
    assert!(
        (m_cal.asset_vol() - m_known.asset_vol()).abs() < 1e-6,
        "asset vol roundtrip"
    );
}

#[wasm_bindgen_test]
fn merton_try_implied_equity_matches_native() {
    let json = merton_model_json(
        JsValue::from(100.0),
        JsValue::from(0.20),
        JsValue::from(80.0),
        JsValue::from(0.05),
    )
    .expect("merton json");
    let pair = merton_try_implied_equity(JsValue::from(json.as_str()), JsValue::from(1.0))
        .expect("equity");
    let (equity_wasm, vol_wasm) = (pair.get_index(0), pair.get_index(1));
    let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
    let (equity_native, vol_native) = model.try_implied_equity(1.0).expect("native");
    assert!(
        (equity_wasm - equity_native).abs() < 1e-12,
        "WASM equity ({equity_wasm}) must match native ({equity_native})"
    );
    assert!(
        (vol_wasm - vol_native).abs() < 1e-12,
        "WASM equity vol ({vol_wasm}) must match native ({vol_native})"
    );
}
