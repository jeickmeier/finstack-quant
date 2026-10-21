//! WASM bindings for portfolio metric aggregation.

use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::{from_js_json, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Aggregate portfolio metrics from a valuation.
///
/// Metrics are aggregated in the valuation's base currency, with FX spots
/// read at the valuation's `as_of`.
/// @param valuation_json - `PortfolioValuation` object or JSON, for example the result of `valuePortfolio`.
/// @param market - `core.MarketContext` handle supplying the FX spots that convert out-of-base metrics.
///
/// # Errors
///
/// Throws a JavaScript exception if the valuation input is malformed, a
/// required FX conversion is unavailable or invalid, or the metrics cannot be
/// converted to a JavaScript value.
#[wasm_bindgen(js_name = aggregateMetrics)]
pub fn aggregate_metrics(
    valuation_json: JsValue,
    market: &JsMarketContext,
) -> Result<JsValue, JsValue> {
    let valuation: finstack_quant_portfolio::valuation::PortfolioValuation =
        from_js_json(&valuation_json, "valuationJson")?;
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(&valuation, market.inner())
        .map_err(to_js_err)?;
    to_js_value(&metrics)
}

/// Decoded series of one base metric from aggregated portfolio metrics.
///
/// Free-function twin of Python `PortfolioMetrics.metric_series` (Rust
/// `PortfolioMetrics::metric_series`): every aggregated key that encodes a
/// series of `base` (e.g. `bucketed_dv01::USD-OIS::10y`) is decoded into its
/// components.
/// @param metrics_json - `PortfolioMetrics` from `aggregateMetrics` (object or JSON).
/// @param base - Canonical base metric identifier (e.g. `"bucketed_dv01"`).
/// @returns `{ components, total, by_entity }` entries in aggregation order.
///
/// # Errors
///
/// Throws with kind `validation` if the metrics input is malformed or `base`
/// is not a canonically encoded metric key.
#[wasm_bindgen(js_name = portfolioMetricsSeries)]
pub fn portfolio_metrics_series(metrics_json: JsValue, base: JsValue) -> Result<JsValue, JsValue> {
    let metrics: finstack_quant_portfolio::metrics::PortfolioMetrics =
        serde_json::from_str(&json_text(&metrics_json, "metricsJson")?).map_err(to_js_err)?;
    let base: finstack_quant_valuations::metrics::MetricId =
        js_string(&base, "base")?.parse().map_err(to_js_err)?;
    to_js_value(&metrics.metric_series(&base))
}
