//! Free-function twins of the Python `ScenarioSpec` and `HorizonResult`
//! methods.
//!
//! WASM scenario specs and horizon results are plain objects, so each Python
//! method is a function taking that object (or its JSON) first.

use crate::utils::input::{from_js_json, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_scenarios::{HazardBumpMode, HorizonResult, ScenarioSpec};
use wasm_bindgen::prelude::*;

fn scenario_spec(value: &JsValue) -> Result<ScenarioSpec, JsValue> {
    ScenarioSpec::from_json(&json_text(value, "spec")?).map_err(to_js_err)
}

fn horizon_result(value: &JsValue) -> Result<HorizonResult, JsValue> {
    from_js_json(value, "result")
}

/// Report whether applying a scenario needs instruments in the context.
///
/// Free-function twin of Python `ScenarioSpec.requires_instruments` (Rust
/// `ScenarioSpec::requires_instruments`).
/// @param spec - `ScenarioSpec` object or JSON; it is validated first.
/// @returns `true` when the scenario holds an instrument-scoped shock or a `time_roll_forward`.
///
/// # Errors
///
/// Throws a `TypeError` when `spec` is not an object or JSON string, and a
/// `validation` error when it is not a valid `ScenarioSpec`.
#[wasm_bindgen(js_name = scenarioSpecRequiresInstruments)]
pub fn scenario_spec_requires_instruments(spec: JsValue) -> Result<bool, JsValue> {
    Ok(scenario_spec(&spec)?.requires_instruments())
}

/// Report whether applying a scenario can replace or mutate instruments.
///
/// Free-function twin of Python `ScenarioSpec.mutates_instruments` (Rust
/// `ScenarioSpec::mutates_instruments`).
/// @param spec - `ScenarioSpec` object or JSON; it is validated first.
/// @returns `true` for instrument price, spread or structured-credit correlation shocks; a time roll alone is `false`.
///
/// # Errors
///
/// Throws a `TypeError` when `spec` is not an object or JSON string, and a
/// `validation` error when it is not a valid `ScenarioSpec`.
#[wasm_bindgen(js_name = scenarioSpecMutatesInstruments)]
pub fn scenario_spec_mutates_instruments(spec: JsValue) -> Result<bool, JsValue> {
    Ok(scenario_spec(&spec)?.mutates_instruments())
}

/// Copy a scenario with a different ParCDS hazard delivery mode.
///
/// Free-function twin of Python `ScenarioSpec.with_hazard_bump_mode` (Rust
/// `ScenarioSpec::with_hazard_bump_mode`). The input is not modified.
/// @param spec - `ScenarioSpec` object or JSON; it is validated first.
/// @param mode - `"solve_to_par"` (re-bootstrap hazard from shocked par spreads) or `"first_order_shift"` (shift hazard knots in place).
/// @returns A new `ScenarioSpec` object with `hazard_bump_mode` replaced.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments, and a `validation` error
/// when `spec` is not a valid `ScenarioSpec` or `mode` is not an accepted
/// label.
#[wasm_bindgen(js_name = scenarioSpecWithHazardBumpMode)]
pub fn scenario_spec_with_hazard_bump_mode(
    spec: JsValue,
    mode: JsValue,
) -> Result<JsValue, JsValue> {
    let mode: HazardBumpMode =
        finstack_quant_core::wire::serde_parse(&js_string(&mode, "mode")?).map_err(to_js_err)?;
    to_js_value(&scenario_spec(&spec)?.with_hazard_bump_mode(mode))
}

/// Render a horizon result as a multi-line text summary.
///
/// Free-function twin of Python `HorizonResult.explain` (Rust
/// `Display for HorizonResult`): total and annualized return, horizon length,
/// initial and terminal values, and the carry / rates / credit / residual
/// legs of the attribution.
/// @param result - Object returned by `computeHorizonReturn` (its `summary` is ignored), or a `HorizonResult` object or JSON.
/// @returns Multi-line human-readable text.
///
/// # Errors
///
/// Throws a `TypeError` when `result` is not an object or JSON string, and a
/// `validation` error when it is not a `HorizonResult`.
#[wasm_bindgen(js_name = horizonResultExplainText)]
pub fn horizon_result_explain(result: JsValue) -> Result<String, JsValue> {
    Ok(horizon_result(&result)?.to_string())
}

/// One attribution factor's P&L as a fraction of the initial value.
///
/// Free-function twin of Python `HorizonResult.factor_contribution` (Rust
/// `HorizonResult::factor_contribution`).
/// @param result - Object returned by `computeHorizonReturn` (its `summary` is ignored), or a `HorizonResult` object or JSON.
/// @param factor - Attribution factor serde name: `"carry"`, `"rates_curves"`, `"credit_curves"`, `"inflation_curves"`, `"correlations"`, `"fx"`, `"volatility"`, `"market_scalars"` or `"model_parameters"`.
/// @returns Factor P&L divided by the initial value, as a decimal fraction (`0.01` = 1%); `NaN` when the initial value is zero or negative or the currencies differ.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments, and a `validation` error
/// when `result` is not a `HorizonResult` or `factor` is not an accepted name.
#[wasm_bindgen(js_name = horizonResultFactorContribution)]
pub fn horizon_result_factor_contribution(
    result: JsValue,
    factor: JsValue,
) -> Result<f64, JsValue> {
    let factor: finstack_quant_attribution::AttributionFactor =
        finstack_quant_core::wire::serde_parse(&js_string(&factor, "factor")?)
            .map_err(to_js_err)?;
    Ok(horizon_result(&result)?.factor_contribution(&factor))
}
