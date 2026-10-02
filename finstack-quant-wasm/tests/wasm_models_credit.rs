//! wasm-bindgen-test suite for `api::models::credit`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_models::credit::{
    DynamicRecoverySpec, EndogenousHazardSpec, MertonModel, ToggleExerciseModel,
};
use finstack_quant_wasm::api::models::credit::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn wasm_merton() -> JsMertonModel {
    JsMertonModel::new(
        JsValue::from(100.0),
        JsValue::from(0.20),
        JsValue::from(80.0),
        JsValue::from(0.05),
    )
    .expect("merton")
}

fn native_merton() -> MertonModel {
    MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton")
}

#[wasm_bindgen_test]
fn merton_distance_to_default_matches_native() {
    let dd_wasm = wasm_merton()
        .distance_to_default(JsValue::from(1.0))
        .expect("dd");
    let dd_native = native_merton().distance_to_default(1.0);
    assert!(
        (dd_wasm - dd_native).abs() < 1e-12,
        "WASM dd ({dd_wasm}) must match native ({dd_native})"
    );
}

#[wasm_bindgen_test]
fn merton_implied_spread_matches_native() {
    let spread_wasm = wasm_merton()
        .implied_spread(JsValue::from(5.0), JsValue::from(0.40))
        .expect("spread");
    let spread_native = native_merton().implied_spread(5.0, 0.40).expect("spread");
    assert!(
        (spread_wasm - spread_native).abs() < 1e-12,
        "WASM spread ({spread_wasm}) must match native ({spread_native})"
    );
}

#[wasm_bindgen_test]
fn merton_default_probabilities_match_the_scalar_method() {
    let model = wasm_merton();
    let grid = model
        .default_probabilities(JsValue::from(js_sys::Float64Array::from(
            [1.0, 5.0].as_slice(),
        )))
        .expect("grid");
    let native = native_merton();
    assert_eq!(grid.len(), 2);
    assert!((grid[0] - native.default_probability(1.0)).abs() < 1e-12);
    assert!((grid[1] - native.default_probability(5.0)).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn dynamic_recovery_at_notional_matches_native() {
    let r_wasm = JsDynamicRecoverySpec::constant(JsValue::from(0.40))
        .expect("spec")
        .recovery_at_notional(JsValue::from(100.0))
        .expect("r");
    let r_native = DynamicRecoverySpec::constant(0.40)
        .expect("spec")
        .recovery_at_notional(100.0);
    assert!((r_wasm - r_native).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn endogenous_hazard_matches_native() {
    let wasm = JsEndogenousHazardSpec::power_law(
        JsValue::from(0.10),
        JsValue::from(1.5),
        JsValue::from(2.5),
    )
    .expect("spec");
    let native = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
    let at_leverage = wasm.hazard_at_leverage(JsValue::from(2.0)).expect("h");
    assert!((at_leverage - native.hazard_at_leverage(2.0)).abs() < 1e-12);
    let after_pik = wasm
        .hazard_after_pik_accrual(JsValue::from(120.0), JsValue::from(66.67))
        .expect("h");
    assert!((after_pik - native.hazard_after_pik_accrual(120.0, 66.67)).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn toggle_exercise_optimal_keeps_a_reasonable_path_count() {
    let model = JsToggleExerciseModel::optimal(
        JsValue::from(10_000),
        JsValue::from(0.10),
        JsValue::from(0.25),
        JsValue::from(0.04),
        JsValue::from(5.0),
    )
    .expect("model");
    let json = model.to_json().expect("json");
    let parsed: ToggleExerciseModel =
        serde_json::from_str(&json).expect("payload must deserialize");
    match parsed {
        ToggleExerciseModel::OptimalExercise(o) => assert_eq!(o.nested_paths, 10_000),
        other => panic!("expected OptimalExercise, got {other:?}"),
    }
}

#[wasm_bindgen_test]
fn toggle_exercise_optimal_rejects_an_unsafe_path_count() {
    // A `nestedPaths` above Number.MAX_SAFE_INTEGER would round silently when
    // marshaled as an f64; the binding must reject it instead.
    let result = JsToggleExerciseModel::optimal(
        JsValue::from(9_007_199_254_740_993.0_f64),
        JsValue::from(0.10),
        JsValue::from(0.25),
        JsValue::from(0.04),
        JsValue::from(5.0),
    );
    assert!(
        result.is_err(),
        "nestedPaths above 2^53-1 must be rejected, not silently rounded"
    );
}

#[wasm_bindgen_test]
fn merton_from_equity_roundtrips() {
    let known = native_merton();
    let (equity, equity_vol) = known.try_implied_equity(1.0).expect("equity");
    let calibrated = JsMertonModel::from_equity(
        JsValue::from(equity),
        JsValue::from(equity_vol),
        JsValue::from(80.0),
        JsValue::from(0.05),
        JsValue::from(0.0),
        JsValue::from(1.0),
    )
    .expect("calibration");
    assert!(
        (calibrated.asset_value() - known.asset_value()).abs() < 1e-6,
        "asset value roundtrip"
    );
    assert!(
        (calibrated.asset_vol() - known.asset_vol()).abs() < 1e-6,
        "asset vol roundtrip"
    );
}

#[wasm_bindgen_test]
fn merton_try_implied_equity_matches_native() {
    let pair = wasm_merton()
        .try_implied_equity(JsValue::from(1.0))
        .expect("equity");
    let (equity_native, vol_native) = native_merton().try_implied_equity(1.0).expect("native");
    assert!((pair[0] - equity_native).abs() < 1e-12);
    assert!((pair[1] - vol_native).abs() < 1e-12);
}
