//! WASM bindings for the portfolio cashflow ladder.

use super::JsPortfolio;
use crate::utils::input::{js_opt_bool, js_string, json_text};

use crate::api::core::market_context::JsMarketContext;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Aggregate the full classified cashflow ladder using reusable portfolio and market handles.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param market - Constructed `core.MarketContext` handle supplying curves, quotes, and FX data.
/// @param allow_partial - Optional; when omitted or `undefined`, uses the Rust
///   `CashflowAggregationOptions` default, `false` (fail closed if any
///   position fails schedule construction). Pass `true` to keep a partial
///   ladder with issues on the result.
///
/// # Errors
///
/// Throws a JavaScript exception if any position
/// fails schedule construction while `allowPartial` is not `true`, monetary
/// cash-flow aggregation overflows, or the aggregate cannot be converted to
/// a JavaScript value.
#[wasm_bindgen(js_name = aggregateFullCashflows)]
pub fn aggregate_full_cashflows(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    allow_partial: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let allow_partial = js_opt_bool(allow_partial.as_ref(), "allowPartial")?;
    let mut options = finstack_quant_portfolio::cashflows::CashflowAggregationOptions::default();
    if let Some(allow_partial) = allow_partial {
        options.allow_partial = allow_partial;
    }
    let cashflows = finstack_quant_portfolio::cashflows::aggregate_full_cashflows(
        &portfolio.inner,
        market.inner(),
        &options,
    )
    .map_err(to_js_err)?;
    to_js_value(&cashflows)
}

/// Net same-currency cashflow amounts per date from a cashflow ladder.
///
/// Mirrors Python `net_in_currency_by_date` (Rust
/// `cashflows::net_in_currency_by_date_json`).
/// @param cashflows_json - `PortfolioCashflows` from `aggregateFullCashflows` (object or JSON), or a bare `{date: {ccy: {kind: money}}}` map.
/// @param currency - ISO-4217 code selecting which per-date currency bucket to net.
/// @returns `[isoDate, netAmount]` pairs sorted by date; dates with no flows in `currency` are omitted.
///
/// # Errors
///
/// Throws with kind `validation` if the input is not JSON, `currency` is not
/// a known ISO code, or `by_date` is not an object.
#[wasm_bindgen(js_name = netInCurrencyByDate)]
pub fn net_in_currency_by_date(
    cashflows_json: JsValue,
    currency: JsValue,
) -> Result<JsValue, JsValue> {
    let rows = finstack_quant_portfolio::cashflows::net_in_currency_by_date_json(
        &json_text(&cashflows_json, "cashflowsJson")?,
        &js_string(&currency, "currency")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&rows)
}

/// Collapse a multi-currency cashflow ladder into the base currency per date and kind.
///
/// Free-function twin of Python `PortfolioCashflows.collapse_to_base_by_date_kind_json`
/// (Rust `PortfolioCashflows::collapse_to_base_by_date_kind`): each flow is
/// converted at the CIP forward `F(T) = S × DF_from(T) / DF_base(T)` from the
/// `asOf` spot.
/// @param cashflows_json - `PortfolioCashflows` from `aggregateFullCashflows` (object or JSON).
/// @param market_json - Canonical market-context JSON supplying the FX matrix and discount curves.
/// @param base_currency - ISO-4217 reporting currency.
/// @param as_of - ISO-8601 valuation date for spot FX and the start of each discount interval.
/// @param discount_curves - Optional `{ currency: curveId }` map; a missing entry uses the ISO code as the curve id.
/// @returns Nested `{ isoDate: { kind: Money } }` ladder in the base currency.
///
/// # Errors
///
/// Throws if an input is malformed, an FX rate or discount factor needed for a
/// conversion is missing or invalid, or monetary aggregation fails.
#[wasm_bindgen(js_name = collapseToBaseByDateKind)]
pub fn collapse_to_base_by_date_kind(
    cashflows_json: JsValue,
    market_json: JsValue,
    base_currency: JsValue,
    as_of: JsValue,
    discount_curves: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let cashflows: finstack_quant_portfolio::cashflows::PortfolioCashflows =
        serde_json::from_str(&json_text(&cashflows_json, "cashflowsJson")?).map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(&json_text(&market_json, "marketJson")?).map_err(to_js_err)?;
    let base_currency: finstack_quant_core::currency::Currency =
        js_string(&base_currency, "baseCurrency")?
            .parse()
            .map_err(to_js_err)?;
    let as_of = crate::utils::parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let discount_curves: Option<
        std::collections::HashMap<
            finstack_quant_core::currency::Currency,
            finstack_quant_core::types::CurveId,
        >,
    > = match discount_curves.as_ref() {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            Some(crate::utils::input::from_js_json(value, "discountCurves")?)
        }
        _ => None,
    };
    let collapsed = cashflows
        .collapse_to_base_by_date_kind(&market, base_currency, as_of, discount_curves.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&collapsed)
}
