//! WASM bindings for structural credit model specifications.
//!
//! Mirrors the layout of `finstack-py/src/bindings/valuations/credit.rs` so the
//! Rust-canonical → PyO3 → wasm-bindgen triplet keeps file parity. The exported
//! JS surface (under `valuations.credit.*`) is unchanged — wasm-bindgen
//! exports are flat by `js_name`, so this is a pure source reorganisation.

use crate::utils::{check_js_safe_count, to_js_err};
use finstack_valuations::models::credit::{
    CreditState, CreditStateVariable, DynamicRecoverySpec, EndogenousHazardSpec, MertonModel,
    OptimalToggle, ThresholdDirection, ToggleExerciseModel,
};
use wasm_bindgen::prelude::*;

/// Build a structural Merton model JSON payload.
#[wasm_bindgen(js_name = mertonModelJson)]
pub fn merton_model_json(
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    risk_free_rate: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::new(asset_value, asset_vol, debt_barrier, risk_free_rate)
        .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build a CreditGrades structural model JSON payload.
#[wasm_bindgen(js_name = creditGradesModelJson)]
pub fn credit_grades_model_json(
    equity_value: f64,
    equity_vol: f64,
    total_debt: f64,
    risk_free_rate: f64,
    barrier_uncertainty: f64,
    mean_recovery: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::credit_grades(
        equity_value,
        equity_vol,
        total_debt,
        risk_free_rate,
        barrier_uncertainty,
        mean_recovery,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Compute structural default probability from model JSON.
#[wasm_bindgen(js_name = mertonDefaultProbability)]
pub fn merton_default_probability(model_json: &str, horizon: f64) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    Ok(model.default_probability(horizon))
}

/// Compute distance-to-default from a Merton model JSON payload.
///
/// Distance-to-default is `ln(V/B)/(sigma*sqrt(T))` plus drift adjustments.
/// Lower values indicate higher default risk.
#[wasm_bindgen(js_name = mertonDistanceToDefault)]
pub fn merton_distance_to_default(model_json: &str, horizon: f64) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    Ok(model.distance_to_default(horizon))
}

/// Compute the implied credit spread (per year) from a Merton model JSON
/// payload, given a recovery rate. Matches the structural-model-implied
/// spread used to back into a hazard curve.
#[wasm_bindgen(js_name = mertonImpliedSpread)]
pub fn merton_implied_spread(
    model_json: &str,
    horizon: f64,
    recovery: f64,
) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    Ok(model.implied_spread(horizon, recovery))
}

/// Evaluate a `DynamicRecoverySpec` JSON payload at a given accreted
/// notional, returning the implied recovery rate. Result is clamped to
/// `[0, base_recovery]`.
#[wasm_bindgen(js_name = dynamicRecoveryAtNotional)]
pub fn dynamic_recovery_at_notional(spec_json: &str, notional: f64) -> Result<f64, JsValue> {
    let spec: DynamicRecoverySpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.recovery_at_notional(notional))
}

/// Evaluate an `EndogenousHazardSpec` JSON payload at a given leverage
/// level, returning the implied hazard rate. Floored at 0.
#[wasm_bindgen(js_name = endogenousHazardAtLeverage)]
pub fn endogenous_hazard_at_leverage(spec_json: &str, leverage: f64) -> Result<f64, JsValue> {
    let spec: EndogenousHazardSpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.hazard_at_leverage(leverage))
}

/// Convenience evaluator: hazard rate after a PIK accrual updates the
/// outstanding notional. Computes leverage = `accreted_notional / asset_value`
/// then evaluates the hazard mapping.
#[wasm_bindgen(js_name = endogenousHazardAfterPikAccrual)]
pub fn endogenous_hazard_after_pik_accrual(
    spec_json: &str,
    accreted_notional: f64,
    asset_value: f64,
) -> Result<f64, JsValue> {
    let spec: EndogenousHazardSpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.hazard_after_pik_accrual(accreted_notional, asset_value))
}

/// Build a constant dynamic-recovery spec JSON payload.
#[wasm_bindgen(js_name = dynamicRecoveryConstantJson)]
pub fn dynamic_recovery_constant_json(recovery: f64) -> Result<String, JsValue> {
    let spec = DynamicRecoverySpec::constant(recovery).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Build an endogenous hazard power-law spec JSON payload.
#[wasm_bindgen(js_name = endogenousHazardPowerLawJson)]
pub fn endogenous_hazard_power_law_json(
    base_hazard: f64,
    base_leverage: f64,
    exponent: f64,
) -> Result<String, JsValue> {
    let spec =
        EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Build a credit-state JSON payload for toggle-exercise decisions.
#[wasm_bindgen(js_name = creditStateJson)]
pub fn credit_state_json(
    hazard_rate: f64,
    leverage: f64,
    accreted_notional: f64,
    coupon_due: f64,
    distance_to_default: Option<f64>,
    asset_value: Option<f64>,
) -> Result<String, JsValue> {
    let state = CreditState {
        hazard_rate,
        distance_to_default,
        leverage,
        accreted_notional,
        coupon_due,
        asset_value,
    };
    serde_json::to_string(&state).map_err(to_js_err)
}

/// Build a threshold toggle-exercise model JSON payload.
#[wasm_bindgen(js_name = toggleExerciseThresholdJson)]
pub fn toggle_exercise_threshold_json(
    variable: &str,
    threshold: f64,
    direction: &str,
) -> Result<String, JsValue> {
    let variable = variable.parse::<CreditStateVariable>().map_err(to_js_err)?;
    let direction = direction.parse::<ThresholdDirection>().map_err(to_js_err)?;
    let model = ToggleExerciseModel::threshold(variable, threshold, direction);
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build an optimal toggle-exercise model JSON payload.
///
/// `nested_paths` is the Monte-Carlo path count for the nested optimal-exercise
/// simulation. It is rejected if it exceeds `Number.MAX_SAFE_INTEGER` (`2^53-1`):
/// `usize` counts marshal across the wasm boundary as IEEE-754 doubles, so a
/// larger value would round silently rather than fail loudly.
#[wasm_bindgen(js_name = toggleExerciseOptimalJson)]
pub fn toggle_exercise_optimal_json(
    nested_paths: usize,
    equity_discount_rate: f64,
    asset_vol: f64,
    risk_free_rate: f64,
    horizon: f64,
) -> Result<String, JsValue> {
    check_js_safe_count(nested_paths, "nested_paths")?;
    let model = ToggleExerciseModel::OptimalExercise(OptimalToggle {
        nested_paths,
        equity_discount_rate,
        asset_vol,
        risk_free_rate,
        horizon,
    });
    serde_json::to_string(&model).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Credit-model evaluator parity (mirrors finstack-py PyMertonModel etc.).

    #[test]
    fn merton_distance_to_default_matches_native() {
        let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
        let dd_wasm = merton_distance_to_default(&json, 1.0).expect("dd");
        let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let dd_native = model.distance_to_default(1.0);
        assert!(
            (dd_wasm - dd_native).abs() < 1e-12,
            "WASM dd ({dd_wasm}) must match native ({dd_native})"
        );
    }

    #[test]
    fn merton_implied_spread_matches_native() {
        let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
        let spread_wasm = merton_implied_spread(&json, 5.0, 0.40).expect("spread");
        let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let spread_native = model.implied_spread(5.0, 0.40);
        assert!(
            (spread_wasm - spread_native).abs() < 1e-12,
            "WASM spread ({spread_wasm}) must match native ({spread_native})"
        );
    }

    #[test]
    fn dynamic_recovery_at_notional_matches_native() {
        let json = dynamic_recovery_constant_json(0.40).expect("spec json");
        let r_wasm = dynamic_recovery_at_notional(&json, 100.0).expect("r");
        let spec = DynamicRecoverySpec::constant(0.40).expect("spec");
        let r_native = spec.recovery_at_notional(100.0);
        assert!((r_wasm - r_native).abs() < 1e-12);
    }

    #[test]
    fn endogenous_hazard_at_leverage_matches_native() {
        let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
        let h_wasm = endogenous_hazard_at_leverage(&json, 2.0).expect("h");
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
        let h_native = spec.hazard_at_leverage(2.0);
        assert!((h_wasm - h_native).abs() < 1e-12);
    }

    #[test]
    fn endogenous_hazard_after_pik_accrual_matches_native() {
        let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
        let h_wasm = endogenous_hazard_after_pik_accrual(&json, 120.0, 66.67).expect("h");
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
        let h_native = spec.hazard_after_pik_accrual(120.0, 66.67);
        assert!((h_wasm - h_native).abs() < 1e-12);
    }

    #[test]
    fn toggle_exercise_optimal_json_accepts_reasonable_path_count() {
        // A normal nested-path count round-trips into a valid model payload.
        let json = toggle_exercise_optimal_json(10_000, 0.10, 0.25, 0.04, 5.0).expect("model json");
        let model: ToggleExerciseModel =
            serde_json::from_str(&json).expect("payload must deserialize");
        match model {
            ToggleExerciseModel::OptimalExercise(o) => assert_eq!(o.nested_paths, 10_000),
            other => panic!("expected OptimalExercise, got {other:?}"),
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn toggle_exercise_optimal_json_rejects_unsafe_path_count() {
        // A `nested_paths` above Number.MAX_SAFE_INTEGER would round silently
        // when marshaled as an f64; the binding must reject it instead.
        let unsafe_count = crate::utils::MAX_SAFE_JS_INTEGER as usize + 1;
        let result = toggle_exercise_optimal_json(unsafe_count, 0.10, 0.25, 0.04, 5.0);
        assert!(
            result.is_err(),
            "nested_paths above 2^53-1 must be rejected, not silently rounded"
        );
    }
}
