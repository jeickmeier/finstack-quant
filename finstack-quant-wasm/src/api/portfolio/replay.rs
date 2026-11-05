//! WASM bindings for historical portfolio replay.

use super::JsPortfolio;
use crate::utils::input::{from_js_json, json_text};
use crate::utils::{to_js_err, to_js_value_with_bigints};
use wasm_bindgen::prelude::*;

/// Replay a portfolio through dated market snapshots.
///
/// Accepts a built portfolio, an array of dated market snapshots, and a
/// replay configuration. Returns a structured `ReplayResult` object.
/// @param portfolio - Built `Portfolio` handle (from `Portfolio.fromSpec` or `Portfolio.builder`) to revalue at every snapshot.
/// @param snapshotsJson - Market-snapshot JSON array.
/// @param configJson - `ReplayConfig` JSON with a required `mode` (`pv_only` | `pv_and_pnl` | `full_attribution`) and optional `attribution_method`, `valuation_options` and `on_error`; unknown keys are rejected.
///
/// Each position's `valuation_result` matches what `priceInstrument` returns:
/// the 64-bit Monte Carlo `seed` is a `BigInt` and path counts are plain
/// numbers, so serialize one with `valuations.valuationResultToJson`.
/// `position_values` and `by_entity` are plain objects keyed by id: JavaScript
/// enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric
/// order before other keys, not in the Rust (and Python) insertion order; take
/// valuation order from the portfolio's positions when it matters.
///
/// # Errors
///
/// Throws a JavaScript exception if any JSON input is malformed; the replay
/// configuration or snapshot dates and ordering are invalid; valuation,
/// attribution, or currency conversion fails; best-effort replay retains no
/// step; or the result cannot be converted to a JavaScript value.
/// @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
#[wasm_bindgen(js_name = replayPortfolio, unchecked_return_type = "ReplayResult")]
pub fn replay_portfolio(
    portfolio: &JsPortfolio,
    #[wasm_bindgen(js_name = "snapshotsJson", unchecked_param_type = "JsonInput")]
    snapshots_json: JsValue,
    #[wasm_bindgen(js_name = "configJson", unchecked_param_type = "JsonInput")]
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let snapshots_json: &str = &json_text(&snapshots_json, "snapshotsJson")?;
    let config: finstack_quant_portfolio::replay::ReplayConfig =
        from_js_json(&config_json, "configJson")?;
    let timeline =
        finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(snapshots_json)
            .map_err(to_js_err)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let result = finstack_quant_portfolio::replay::replay_portfolio(
        &portfolio.inner,
        &timeline,
        &config,
        &finstack_config,
    )
    .map_err(to_js_err)?;
    to_js_value_with_bigints(&result)
}
