//! WASM binding for portfolio-wide primitive exposure look-through.

use super::JsPortfolio;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::metrics::MetricId;
use wasm_bindgen::prelude::*;

/// Decompose every portfolio position into primitive economic exposures.
///
/// Published as `portfolio.primitiveExposures` (raw export
/// `portfolioPrimitiveExposures`; `valuations.composite.primitiveExposures` is
/// the per-composite variant).
///
/// Direct instruments yield one primitive path; composite positions recurse
/// through their frozen resolved leg quantities. The position quantity scales
/// every primitive quantity, value and additive risk amount. Returns the
/// `PortfolioPrimitiveExposureReport`: `base_currency`, position-aware
/// `paths` and per-instrument net/gross `aggregates`, all in the portfolio
/// base currency.
/// @param portfolio - Built portfolio whose direct and composite positions are decomposed.
/// @param market - `core.MarketContext` handle with the complete valuation and FX market.
/// @param metrics - Array of additive metric ids (e.g. `"dv01"`); pass `[]` for quantity and value only.
/// @returns The `PortfolioPrimitiveExposureReport`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `metrics` is not an array or
/// JSON string, and a `FinstackError` if a metric id is not canonical or is
/// non-additive, primitive definitions conflict across positions, a composite
/// is invalid, or market data or FX is missing.
#[wasm_bindgen(js_name = portfolioPrimitiveExposures)]
pub fn primitive_exposures(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    metrics: JsValue,
) -> Result<JsValue, JsValue> {
    let metrics: Vec<MetricId> = from_js_json(&metrics, "metrics")?;
    let report =
        finstack_quant_portfolio::primitive_exposures(&portfolio.inner, market.inner(), &metrics)
            .map_err(to_js_err)?;
    to_js_value(&report)
}
