//! WASM bindings for portfolio P&L attribution.

use super::JsPortfolio;
use crate::utils::input::js_string;

use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::opt_json_text;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Attribute portfolio P&L between two market snapshots.
///
/// Returns the `PortfolioAttribution`: base-currency `total_pnl`, one bucket
/// per factor (`carry`, `rates_curves_pnl`, `credit_curves_pnl`, `fx_pnl`,
/// `vol_pnl`, ..., `residual`) and the per-position `by_position` detail.
/// @param portfolio - Built portfolio whose positions are attributed.
/// @param market_t0 - `core.MarketContext` handle for the opening snapshot.
/// @param market_t1 - `core.MarketContext` handle for the closing snapshot.
/// @param as_of_t0 - ISO-8601 date of the opening snapshot.
/// @param as_of_t1 - ISO-8601 date of the closing snapshot.
/// @param method - `AttributionMethod` wire value: a unit variant name such as `"parallel"` or `"metrics_based"`, an object such as `{ waterfall: ["carry", "rates_curves"] }`, or the same value as JSON text.
/// @param config - Optional `FinstackConfig` object or JSON; omit for the Rust default configuration.
/// @returns The `PortfolioAttribution`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
/// `FinstackError` if a date is not an ISO date or `method` / `config` is
/// malformed (kind `validation`), a required FX rate or market datum is
/// missing (kind `not_found`), or a position valuation or attribution fails.
#[wasm_bindgen(js_name = attributePortfolioPnl)]
pub fn attribute_portfolio_pnl(
    portfolio: &JsPortfolio,
    market_t0: &JsMarketContext,
    market_t1: &JsMarketContext,
    as_of_t0: JsValue,
    as_of_t1: JsValue,
    method: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let as_of_t0 = crate::utils::parse_iso_date(&js_string(&as_of_t0, "asOfT0")?)?;
    let as_of_t1 = crate::utils::parse_iso_date(&js_string(&as_of_t1, "asOfT1")?)?;
    // `js_wire`, not `from_js_json`: the unit variants are bare wire strings
    // (`"parallel"`), which `from_js_json` would parse as JSON text.
    let method: finstack_quant_attribution::AttributionMethod =
        crate::utils::wire::js_wire(&method, "method")?;
    let config: finstack_quant_core::config::FinstackConfig =
        match opt_json_text(config.as_ref(), "config")? {
            Some(text) => serde_json::from_str(&text).map_err(to_js_err)?,
            None => finstack_quant_core::config::FinstackConfig::default(),
        };
    let attribution = finstack_quant_portfolio::attribution::attribute_portfolio_pnl(
        &portfolio.inner,
        market_t0.inner(),
        market_t1.inner(),
        as_of_t0,
        as_of_t1,
        &config,
        method,
    )
    .map_err(to_js_err)?;
    to_js_value(&attribution)
}
