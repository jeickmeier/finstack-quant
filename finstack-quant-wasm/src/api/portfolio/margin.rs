//! WASM binding for portfolio margin by netting set.

use super::JsPortfolio;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::from_js_json;
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// Calculate portfolio margin requirements by netting set (Rust
/// `margin::portfolio_margin`).
///
/// Positions are grouped by the netting set named on their instrument's
/// margin specification; positions without one are counted in
/// `positions_without_margin`. Returns the `PortfolioMarginResult`:
/// base-currency totals (`total_initial_margin`, signed
/// `total_variation_margin`, `total_margin`), the one-way IM accounts
/// `by_csa`, and `netting_sets` in ascending identifier order. Each netting
/// set carries the netted SIMM `sensitivities` its initial margin was computed
/// from and the per-risk-class `im_breakdown`.
/// @param portfolio - Built portfolio whose positions carrying margin metadata form the netting sets; its base currency is the reporting currency.
/// @param market - `core.MarketContext` handle supplying curves and quotes for VM and SIMM sensitivities, and the FX matrix for base-currency reporting.
/// @param asOf - ISO-8601 valuation date of the margin run.
/// @param currentImCollateral - Optional one-way IM balances already held, as an object (or JSON) keyed by CSA id with `Money` values (`{ amount, currency }`) in that CSA's currency. Omitted, `undefined`, `null` or a missing key means zero.
/// @returns The `PortfolioMarginResult` as a plain object.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `asOf` is not a string or
/// `currentImCollateral` is not a JSON string or plain object, and a
/// `FinstackError` if positions in one netting set carry conflicting margin
/// specifications (kind `validation`), `asOf` is not an ISO date,
/// `currentImCollateral` does not match the schema or names an unknown CSA
/// id, or a required FX rate is unavailable.
///
/// @example
/// ```typescript
/// import init, { core, portfolio } from "finstack-quant-wasm";
/// await init();
/// const book = portfolio.Portfolio.builder("book", "USD", "2025-01-01").build();
/// const market = new core.MarketContext();
/// const margin = portfolio.portfolioMargin(book, market, "2025-01-01");
/// console.log(margin.total_initial_margin.amount, margin.netting_sets.length);
/// market.free();
/// book.free();
/// ```
#[wasm_bindgen(js_name = portfolioMargin, unchecked_return_type = "PortfolioMarginResult")]
pub fn portfolio_margin(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(
        js_name = "currentImCollateral",
        unchecked_optional_param_type = "Record<string, MoneyValue> | string | null"
    )]
    current_im_collateral: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let as_of = js_date(&as_of, "asOf")?;
    let collateral: FxHashMap<String, Money> = match current_im_collateral {
        Some(value) if !(value.is_null() || value.is_undefined()) => {
            from_js_json(&value, "currentImCollateral")?
        }
        _ => FxHashMap::default(),
    };
    let result = finstack_quant_portfolio::margin::portfolio_margin(
        &portfolio.inner,
        market.inner(),
        as_of,
        &collateral,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
