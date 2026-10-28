//! WASM bindings for portfolio valuation.

use super::JsPortfolio;
use crate::utils::input::{js_opt_bool, js_opt_string_seq};

use crate::api::core::market_context::JsMarketContext;
use crate::utils::{to_js_err, to_js_value_with_bigints};
use wasm_bindgen::prelude::*;

/// Value a portfolio using constructed portfolio and market handles.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param market - Constructed `core.MarketContext` handle supplying curves, quotes, and FX data.
/// @param strict_risk - Optional; when omitted or `undefined`, uses the Rust
///   `PortfolioValuationOptions` default, `true` (fail closed when a requested
///   risk metric fails to compute). Pass `false` only for an intentional
///   PV-preserving fallback.
/// @param metrics - Optional risk-metric ids to offer every position. Omit for
///   the standard set (PV plus `dv01`; pricer-specific metrics such as `theta`
///   or `cs01` must be listed explicitly); an empty array performs PV-only
///   valuation. Names resolve exactly as in `priceInstrument`: every id
///   `listStandardMetrics()` returns is accepted and an unknown name throws.
///   The list is a menu, not a per-position request: one list is chosen for a
///   book of mixed instrument types, so each position is asked for exactly the
///   entries its own instrument can compute (a composite position: the additive
///   entries at least one leg supports), and the rest appear on that position's
///   `inapplicable_metrics`. Narrowing covers structural inapplicability only;
///   `strictRisk` still governs a metric an instrument supports but fails to
///   compute. `priceInstrument` keeps the opposite contract and throws on a
///   metric its instrument cannot produce. Mirrors the Python `metrics=`
///   keyword.
///
/// Each position's `valuation_result` matches what `priceInstrument` returns:
/// the 64-bit Monte Carlo `seed` is a `BigInt` and path counts are plain
/// numbers, so serialize one with `valuations.valuationResultToJson`.
/// `position_values` and `by_entity` are plain objects keyed by id: JavaScript
/// enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric
/// order before other keys, not in the Rust (and Python) insertion order; take
/// valuation order from `spec.positions` when it matters.
///
/// # Errors
///
/// Throws a JavaScript exception if a requested
/// metric name is unknown, portfolio valuation fails, strict risk calculation
/// cannot produce a requested metric, a required FX conversion is unavailable,
/// or the valuation cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = valuePortfolio)]
pub fn value_portfolio(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    strict_risk: Option<JsValue>,
    metrics: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let strict_risk = js_opt_bool(strict_risk.as_ref(), "strictRisk")?;
    let metrics = js_opt_string_seq(metrics.as_ref(), "metrics")?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    // Strict parsing via the canonical portfolio-crate helper (shared with
    // the Python binding): an unknown metric name throws instead of silently
    // degrading to PV-only valuation.
    let mut options = finstack_quant_portfolio::valuation::PortfolioValuationOptions {
        metrics: finstack_quant_portfolio::valuation::RequestedMetrics::try_from_metric_names(
            metrics,
        )
        .map_err(to_js_err)?,
        ..Default::default()
    };
    if let Some(strict_risk) = strict_risk {
        options.strict_risk = strict_risk;
    }
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio.inner,
        market.inner(),
        &config,
        &options,
    )
    .map_err(to_js_err)?;
    to_js_value_with_bigints(&valuation)
}
