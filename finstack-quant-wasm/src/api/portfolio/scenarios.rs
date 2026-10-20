//! WASM bindings for scenario application and scenario P&L.

use super::JsPortfolio;
use crate::utils::input::json_text;

use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value, to_js_value_with_bigints};
use wasm_bindgen::prelude::*;

/// Apply a scenario to an already-built `Portfolio` handle and revalue.
/// Returns a JS object with structured `valuation` and `report` values.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param scenario_json - Scenario specification JSON.
/// @param market - Constructed `core.MarketContext` handle supplying curves, quotes, and FX data.
///
/// Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo
/// `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns
/// them; serialize one with `valuations.valuationResultToJson`.
/// `position_values` and `by_entity` are plain objects keyed by id: JavaScript
/// enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric
/// order before other keys, not in the Rust (and Python) insertion order; take
/// valuation order from `spec.positions` when it matters.
///
/// # Errors
///
/// Throws a JavaScript exception if the scenario JSON is malformed or invalid,
/// scenario application or portfolio revaluation fails, or the structured
/// result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = applyScenarioAndRevalue)]
pub fn apply_scenario_and_revalue(
    portfolio: &JsPortfolio,
    scenario_json: JsValue,
    market: &JsMarketContext,
) -> Result<JsValue, JsValue> {
    let scenario_json: &str = &json_text(&scenario_json, "scenarioJson")?;
    let scenario =
        finstack_quant_scenarios::ScenarioSpec::from_json(scenario_json).map_err(to_js_err)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let out = finstack_quant_portfolio::scenarios::apply_and_revalue(
        &portfolio.inner,
        &scenario,
        market.inner(),
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value_with_bigints(&out)
}
/// Compute the profit and loss attributable to a scenario for an already-built
/// `Portfolio` handle.
///
/// Values the portfolio against the unshocked market and against the
/// scenario-shocked market, and returns a JS object with structured `pnl`
/// (base-currency `total` plus `by_position`) and `report` values. Positions
/// added or removed by the scenario are zero-filled against the missing side,
/// so the drill-down always sums to the total.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param scenario_json - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
/// @param market - Constructed `core.MarketContext` handle supplying the unshocked curves, quotes, and FX data used for the base leg.
///
/// # Errors
///
/// Throws a JavaScript exception if the scenario JSON is malformed or invalid,
/// scenario application or either valuation fails, valuation currencies are
/// inconsistent, or the structured result cannot be converted to JavaScript.
#[wasm_bindgen(js_name = scenarioPnl)]
pub fn scenario_pnl(
    portfolio: &JsPortfolio,
    scenario_json: JsValue,
    market: &JsMarketContext,
) -> Result<JsValue, JsValue> {
    let scenario_json: &str = &json_text(&scenario_json, "scenarioJson")?;
    let scenario =
        finstack_quant_scenarios::ScenarioSpec::from_json(scenario_json).map_err(to_js_err)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let out = finstack_quant_portfolio::scenarios::scenario_pnl(
        &portfolio.inner,
        &scenario,
        market.inner(),
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&out)
}
/// Compute ordered portfolio P&L for a batch of scenarios.
///
/// The Rust batch engine values the unstressed base leg once for the whole
/// request, then applies and revalues each scenario independently. Returns one
/// `ScenarioPnlBatchItem` (`scenario_id`, `pnl`, `report`) per input scenario,
/// in input order; an empty batch returns `[]` without a valuation.
/// @param portfolio - Built portfolio valued for the shared base and every scenario.
/// @param scenarios - Array of `ScenarioSpec` objects, or its JSON.
/// @param market - `core.MarketContext` handle holding the unshocked market snapshot.
/// @returns The ordered `ScenarioPnlBatchItem` array.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `scenarios` is not a JSON
/// string or array, and a `FinstackError` if a scenario is malformed or
/// inconsistent (kind `validation`), or scenario application, valuation or
/// base-currency differencing fails; the error is the earliest failing
/// scenario's.
#[wasm_bindgen(js_name = scenarioPnlBatch)]
pub fn scenario_pnl_batch(
    portfolio: &JsPortfolio,
    scenarios: JsValue,
    market: &JsMarketContext,
) -> Result<JsValue, JsValue> {
    let scenarios: Vec<finstack_quant_scenarios::ScenarioSpec> =
        from_js_json(&scenarios, "scenarios")?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let items = finstack_quant_portfolio::scenarios::scenario_pnl_batch(
        &portfolio.inner,
        &scenarios,
        market.inner(),
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&items)
}
