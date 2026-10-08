//! Portfolio historical VaR / expected shortfall by full revaluation.
//!
//! Mirrors `finstack-quant-py/src/bindings/valuations/var.rs`: binds
//! `finstack_quant_valuations::metrics::risk::calculate_var`,
//! returning the `VarResult` plain object.

use super::pricing::parse_market_json;
use crate::utils::input::{from_js_json, js_opt_string, js_string, json_text};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_valuations::metrics::risk::{MarketHistory, VarConfig};
use finstack_quant_valuations::pricer::PricingDispatch;
use wasm_bindgen::prelude::*;

/// Historical VaR and expected shortfall of a list of instruments.
///
/// Mirrors Rust `metrics::risk::calculate_var`: every instrument
/// is repriced under every `historyJson` scenario, the per-scenario P&Ls are
/// summed across instruments, and VaR / ES are read off that single portfolio
/// distribution (R type-7 linear-interpolated quantile), so offsetting
/// positions diversify. Quote-recalibrated shocks (credit spreads) use the
/// same recalibration provider as `priceInstrument`.
/// @param instruments_json - Array of plain `finstack_quant.instrument/1` envelope objects, or one JSON string holding that array; nested JSON strings are rejected. The whole inventory is capped at 16 MiB; an empty array returns zero VaR and ES.
/// @param market_json - Unshocked base market (JSON string or plain object) every scenario perturbs.
/// @param history_json - `MarketHistory` (JSON string or plain object); a non-empty portfolio needs at least one scenario.
/// @param as_of - ISO-8601 valuation date for the base and every scenario revaluation.
/// @param config - Optional `VarConfig` (JSON string or plain object): `confidence_level` (decimal in `(0, 1)`), `method` (`"full_revaluation"` or `"taylor_approximation"`) and `reporting_currency` (required for mixed-currency portfolios). Omit for the Rust default: 95%, full revaluation, natural currency.
/// @param model - Optional model key applied to every instrument; omit or `"default"` for each instrument's canonical pricing path.
/// @returns `VarResult` plain object: `var`, `expected_shortfall` (losses negative), the worst-first `pnl_distribution`, `num_scenarios`, `confidence_level`, `skipped_fx` and `skipped_vol`.
/// @throws Error - Throws with kind `validation` if an envelope, the market, history, config, `asOf` or `model` is invalid, the confidence level is outside `(0, 1)`, a non-empty portfolio has no scenarios, or a mixed-currency portfolio has no reporting currency; kind `not_found` if required market data is missing; kind `invalid_type` for a wrong argument type; and kind `computation` if a scenario revaluation fails.
#[wasm_bindgen(js_name = calculateVar)]
pub fn calculate_var(
    instruments_json: JsValue,
    market_json: JsValue,
    history_json: JsValue,
    as_of: JsValue,
    config: Option<JsValue>,
    model: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let parsed = finstack_quant_valuations::pricer::json::parse_boxed_instruments_from_json(
        &json_text(&instruments_json, "instrumentsJson")?,
    )
    .map_err(to_js_err)?;
    let market = parse_market_json(&json_text(&market_json, "marketJson")?)?;
    let history: MarketHistory = from_js_json(&history_json, "historyJson")?;
    let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let config: VarConfig = match config.as_ref() {
        Some(value) if !(value.is_null() || value.is_undefined()) => from_js_json(value, "config")?,
        _ => VarConfig::default(),
    };
    let model = js_opt_string(model.as_ref(), "model")?;
    let dispatch =
        PricingDispatch::from_model(model.as_deref().unwrap_or("default")).map_err(to_js_err)?;
    let refs: Vec<_> = parsed.iter().map(|p| p.as_ref()).collect();
    let result = finstack_quant_valuations::metrics::risk::calculate_var(
        &refs,
        &market,
        &history,
        as_of,
        &config,
        dispatch,
        finstack_quant_calibration::recalibration::pricing_options().recalibration_provider,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
