//! WASM bindings for portfolio metric aggregation.

use crate::utils::input::{js_string, json_text};

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Aggregate portfolio metrics from a valuation JSON.
/// @param valuation_json - `PortfolioValuation` JSON, for example `JSON.stringify(valuePortfolio(...))`.
/// @param base_currency - ISO-4217 base currency in which aggregate portfolio values are reported.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed,
/// `baseCurrency` or `asOf` is invalid, valuation currency or date metadata is
/// inconsistent, a required FX conversion is unavailable or invalid, or the
/// metrics cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = aggregateMetrics)]
pub fn aggregate_metrics(
    valuation_json: JsValue,
    base_currency: JsValue,
    market_json: JsValue,
    as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let base_currency: &str = &js_string(&base_currency, "baseCurrency")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let valuation: finstack_quant_portfolio::valuation::PortfolioValuation =
        from_js_json(&valuation_json, "valuationJson")?;
    let ccy: finstack_quant_core::currency::Currency = base_currency.parse().map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        from_js_json(&market_json, "marketJson")?;
    let date = crate::utils::parse_iso_date(as_of)?;
    let metrics =
        finstack_quant_portfolio::metrics::aggregate_metrics(&valuation, ccy, &market, date)
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
    let entries: Vec<MetricSeriesEntry<'_>> = metrics
        .metric_series(&base)
        .into_iter()
        .map(|(components, aggregate)| MetricSeriesEntry {
            components,
            total: aggregate.total,
            by_entity: &aggregate.by_entity,
        })
        .collect();
    to_js_value(&entries)
}

/// One decoded series entry returned by `portfolioMetricsSeries`.
#[derive(serde::Serialize)]
struct MetricSeriesEntry<'a> {
    components: Vec<String>,
    total: f64,
    by_entity: &'a indexmap::IndexMap<finstack_quant_portfolio::types::EntityId, f64>,
}
