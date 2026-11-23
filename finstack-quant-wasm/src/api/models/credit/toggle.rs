//! PIK toggle exercise model WASM handle.

use crate::utils::input::{from_js_json, js_f64, js_string, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::wire::{serde_parse, serde_tag};
use finstack_quant_models::credit::{
    CreditState, CreditStateVariable, ThresholdDirection, ToggleExerciseModel,
};
use wasm_bindgen::prelude::*;

/// PIK/cash toggle exercise rule: threshold, stochastic (logistic) or nested
/// optimal exercise.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const rule = models.credit.ToggleExerciseModel.threshold("leverage", 0.7, "above");
/// const state = { hazard_rate: 0.05, distance_to_default: null, leverage: 0.8, accreted_notional: 100, coupon_due: 4, asset_value: null };
/// console.log(rule.kind, rule.shouldPik(state, 0.5));
/// rule.free();
/// ```
#[wasm_bindgen(js_name = ToggleExerciseModel)]
pub struct JsToggleExerciseModel {
    pub(crate) inner: ToggleExerciseModel,
}

json_round_trip!(JsToggleExerciseModel, ToggleExerciseModel);

#[wasm_bindgen(js_class = ToggleExerciseModel)]
impl JsToggleExerciseModel {
    /// Deterministic rule: PIK when `variable` is `direction` the threshold.
    /// @param variable - Credit-state variable observed: `"hazard_rate"`, `"distance_to_default"` or `"leverage"`.
    /// @param threshold - Trigger level in the variable's own units (annualized decimal hazard, standard deviations, or leverage ratio).
    /// @param direction - `"above"` to PIK when the variable exceeds the threshold, `"below"` when it falls under it.
    /// @returns The threshold rule.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown variable or direction, or a
    /// non-finite threshold.
    pub fn threshold(
        #[wasm_bindgen(unchecked_param_type = "string")] variable: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] direction: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        let variable = serde_parse::<CreditStateVariable>(&js_string(&variable, "variable")?)
            .map_err(to_js_err)?;
        let threshold = js_f64(&threshold, "threshold")?;
        let direction = serde_parse::<ThresholdDirection>(&js_string(&direction, "direction")?)
            .map_err(to_js_err)?;
        ToggleExerciseModel::threshold(variable, threshold, direction)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Stochastic rule: PIK with probability `logistic(intercept + sensitivity * x)`.
    /// @param variable - Credit-state variable `x` observed: `"hazard_rate"`, `"distance_to_default"` or `"leverage"`.
    /// @param intercept - Logit intercept (dimensionless).
    /// @param sensitivity - Logit slope per unit of the variable.
    /// @returns The stochastic rule.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown variable or a non-finite
    /// intercept or sensitivity.
    pub fn stochastic(
        #[wasm_bindgen(unchecked_param_type = "string")] variable: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] intercept: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] sensitivity: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        let variable = serde_parse::<CreditStateVariable>(&js_string(&variable, "variable")?)
            .map_err(to_js_err)?;
        ToggleExerciseModel::stochastic(
            variable,
            js_f64(&intercept, "intercept")?,
            js_f64(&sensitivity, "sensitivity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Nested-Monte-Carlo optimal exercise rule.
    /// @param nestedPaths - Inner simulation paths per decision date; a positive safe integer.
    /// @param assetVol - Annualized asset volatility as a decimal; non-negative.
    /// @param riskFreeRate - Continuously compounded risk-free rate, as a decimal.
    /// @param horizon - Inner simulation horizon in years; positive and finite.
    /// @returns The optimal-exercise rule.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `nestedPaths` is not a safe non-negative integer,
    /// and a `validation` error if it is zero, a rate is non-finite, `assetVol`
    /// is negative, or `horizon` is not positive and finite.
    pub fn optimal(
        #[wasm_bindgen(js_name = "nestedPaths", unchecked_param_type = "number")]
        nested_paths: JsValue,
        #[wasm_bindgen(js_name = "assetVol", unchecked_param_type = "number")] asset_vol: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        ToggleExerciseModel::optimal(
            js_uint(&nested_paths, "nestedPaths")?,
            js_f64(&asset_vol, "assetVol")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&horizon, "horizon")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Whether the rule elects PIK for a credit state given one uniform draw.
    ///
    /// Threshold rules ignore `u`; stochastic rules elect PIK when `u` is
    /// below the logistic probability; optimal-exercise rules run their nested
    /// simulation with a seed derived from `u`, so equal draws give equal
    /// decisions.
    /// @param state - `CreditState` JSON or plain object (`hazard_rate`, `distance_to_default`, `leverage`, `accreted_notional`, `coupon_due`, `asset_value`).
    /// @param u - Uniform draw in `[0, 1)`.
    /// @returns `true` when the rule elects to pay in kind.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `state` is neither a string nor a plain object
    /// or `u` is not a number, a `validation` error if `state` is malformed,
    /// or a computation error if the nested model cannot produce a finite draw.
    #[wasm_bindgen(js_name = shouldPik)]
    pub fn should_pik(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.CreditState | string")]
        state: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] u: JsValue,
    ) -> Result<bool, JsValue> {
        let state: CreditState = from_js_json(&state, "state")?;
        self.inner
            .should_pik(&state, js_f64(&u, "u")?)
            .map_err(to_js_err)
    }
}

wasm_getters!(JsToggleExerciseModel, ToggleExerciseModel |s| {
    /// Which rule this model carries: `"threshold"`, `"stochastic"` or
    /// `"optimal_exercise"` (the canonical serde tag).
    kind -> Result<String, JsValue> = serde_tag(&s.inner).map_err(to_js_err);
    /// Parameters of the active rule as a plain object in canonical JSON form.
    params returns "Record<string, unknown>" -> Result<JsValue, JsValue> = match &s.inner {
        ToggleExerciseModel::Threshold(spec) => to_js_value(spec),
        ToggleExerciseModel::Stochastic(spec) => to_js_value(spec),
        ToggleExerciseModel::OptimalExercise(spec) => to_js_value(spec),
    };
});
