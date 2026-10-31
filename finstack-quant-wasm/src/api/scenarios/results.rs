//! Free-function twins of the Python `ScenarioSpec` and `HorizonResult`
//! methods.
//!
//! WASM scenario specs and horizon results are plain objects, so each Python
//! method is a function taking that object (or its JSON) first.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::to_js_err;
use finstack_quant_scenarios::HorizonResult;
use wasm_bindgen::prelude::*;

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
    Ok(super::scenario_spec(&spec, "spec")?.requires_instruments())
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
    Ok(super::scenario_spec(&spec, "spec")?.mutates_instruments())
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

/// The valuation date that holds after a scenario.
///
/// Free-function twin of Python `ApplicationReport.effective_as_of` (Rust
/// `ApplicationReport::effective_as_of`). A time roll rebases the market, so a
/// re-valuation of the shocked market must use the rolled date.
/// @param report - `ApplicationReport` returned by `applyScenario`, as an object or JSON.
/// @param as_of - ISO-8601 valuation date the scenario was applied at, returned unchanged without a roll.
/// @returns ISO-8601 date: the roll's new date, else `asOf`.
///
/// # Errors
///
/// Throws a `TypeError` when an argument has the wrong type, and a `validation`
/// error when `report` is not an `ApplicationReport` or `asOf` is not an ISO date.
#[wasm_bindgen(js_name = applicationReportEffectiveAsOf)]
pub fn application_report_effective_as_of(
    report: JsValue,
    as_of: JsValue,
) -> Result<String, JsValue> {
    let report: finstack_quant_scenarios::ApplicationReport = from_js_json(&report, "report")?;
    let as_of = crate::utils::wire::js_date(&as_of, "asOf")?;
    Ok(crate::utils::date::date_to_iso(
        report.effective_as_of(as_of),
    ))
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
