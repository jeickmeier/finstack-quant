//! WASM bindings for historical portfolio replay.

use crate::utils::input::json_text;

use crate::utils::{to_js_err, to_js_value_with_bigints};
use wasm_bindgen::prelude::*;

/// Replay a portfolio through dated market snapshots.
///
/// Accepts a portfolio spec, an array of dated market snapshots, and a
/// replay configuration. Returns a structured `ReplayResult` object.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param snapshots_json - Market-snapshot JSON array.
/// @param config_json - `ReplayConfig` JSON with a required `mode` (`pv_only` | `pv_and_pnl` | `full_attribution`) and optional `attribution_method`, `valuation_options` and `on_error`; unknown keys are rejected.
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
/// Throws a JavaScript exception if any JSON input is malformed; the portfolio,
/// replay configuration, or snapshot dates and ordering are invalid; valuation,
/// attribution, or currency conversion fails; best-effort replay retains no
/// step; or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = replayPortfolio)]
pub fn replay_portfolio(
    spec_json: JsValue,
    snapshots_json: JsValue,
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    let snapshots_json: &str = &json_text(&snapshots_json, "snapshotsJson")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let spec: finstack_quant_portfolio::PortfolioSpec =
        serde_json::from_str(spec_json).map_err(to_js_err)?;
    let portfolio = finstack_quant_portfolio::Portfolio::from_spec(spec).map_err(to_js_err)?;
    let config: finstack_quant_portfolio::replay::ReplayConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let timeline =
        finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(snapshots_json)
            .map_err(to_js_err)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let result = finstack_quant_portfolio::replay::replay_portfolio(
        &portfolio,
        &timeline,
        &config,
        &finstack_config,
    )
    .map_err(to_js_err)?;
    to_js_value_with_bigints(&result)
}
