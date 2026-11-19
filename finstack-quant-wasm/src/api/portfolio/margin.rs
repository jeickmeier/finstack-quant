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

/// Reprice ordinary FRTB GIRR yield delta at every exact regulated yield vertex.
/// @param portfolio - Trading-book positions and their reporting currency, priced on the portfolio's as-of date.
/// @param market - Common market and held currency observations with executed rate quote calibrations.
/// @param pricing - Common pricing and rounding configuration for the center and all reprices.
/// @param targets - Exact quote identifiers and executed pillars for each tenored curve. Every dependent curve must cover the complete ten-vertex grid.
/// @returns Position reprice evidence, source fingerprints and the ordinary FRTB GIRR aggregation inputs.
/// @throws FinstackError if bindings, curves, pricing or FX inputs are missing, unsupported or inconsistent.
/// @throws TypeError if `targets` does not match the schema.
///
/// @example
/// ```typescript
/// import init, { core, portfolio } from "finstack-quant-wasm";
/// await init();
/// const book = portfolio.Portfolio.builder("book", "USD", "2025-01-01").build();
/// const market = new core.MarketContext();
/// const config = new core.FinstackConfig();
/// const result = portfolio.girrQuoteDelta(book, market, config, []);
/// console.log(result.as_of, result.quote_risk.length);
/// config.free();
/// market.free();
/// book.free();
/// ```
#[wasm_bindgen(js_name = girrQuoteDelta, unchecked_return_type = "GirrQuoteDelta")]
pub fn girr_quote_delta(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    pricing: &crate::api::core::config::JsFinstackConfig,
    #[wasm_bindgen(unchecked_param_type = "GirrQuoteTarget[]")] targets: JsValue,
) -> Result<JsValue, JsValue> {
    let targets: Vec<finstack_quant_portfolio::margin::GirrQuoteTarget> =
        from_js_json(&targets, "targets")?;
    let result = finstack_quant_portfolio::margin::girr_quote_delta(
        &portfolio.inner,
        market.inner(),
        &pricing.inner,
        &targets,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Derive ordinary non-securitisation CSR deltas from exact single-name CDS quote reprices.
/// @param portfolio - Trading book with signed quantities, valuation date and reporting currency.
/// @param market - Held market with original issuer spread risk recipes and reporting FX.
/// @param pricing - Common pricing and conversion settings for held and shocked marks.
/// @param sources - Dated issuer classification and all five exact quote coordinates for each live issuer CDS curve.
/// @returns Source fingerprints, signed position changes per basis point, and CSR deltas scaled to a percentage point.
/// @throws TypeError if sources contain a non-JSON value. FinstackError for unsupported exposures, invalid mappings, missing inputs or failed replay and pricing.
/// @example
/// ```typescript
/// import init, { core, portfolio } from "finstack-quant-wasm";
/// await init();
/// const book = portfolio.Portfolio.builder("credit", "USD", "2025-01-02").build();
/// const market = new core.MarketContext();
/// const pricing = new core.FinstackConfig();
/// console.log(portfolio.csrQuoteDelta(book, market, pricing, []).quote_risk.length);
/// pricing.free();
/// market.free();
/// book.free();
/// ```
#[wasm_bindgen(js_name = csrQuoteDelta, unchecked_return_type = "generated.portfolio.CsrQuoteDelta")]
pub fn csr_quote_delta(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    pricing: &crate::api::core::config::JsFinstackConfig,
    #[wasm_bindgen(unchecked_param_type = "generated.portfolio.CsrQuoteSource[]")] sources: JsValue,
) -> Result<JsValue, JsValue> {
    let sources: Vec<finstack_quant_portfolio::margin::CsrQuoteSource> =
        from_js_json(&sources, "sources")?;
    let result = finstack_quant_portfolio::margin::csr_quote_delta(
        &portfolio.inner,
        market.inner(),
        &pricing.inner,
        &sources,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
