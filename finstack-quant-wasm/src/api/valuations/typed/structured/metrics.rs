//! Structured-credit tranche analytics and scenario-table WASM entry points.

use super::deal::JsStructuredCredit;

use crate::api::valuations::pricing::parse_market;
use crate::utils::input::{from_js_json, js_f64, js_opt_f64, js_opt_string_seq, js_string};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    self as sc, OasConfig, ScenarioGrid,
};
use wasm_bindgen::prelude::*;

/// Z-spread-equivalent discount margin for a floating-rate tranche, returned in
/// decimal units (`0.015` = 150 bp).
///
/// Contractual cashflows are projected without changing coupon projection,
/// then a constant additive spread is applied to the discount curve. The result
/// is zero at the model price, negative for a richer (higher)
/// `marketPricePct`, and positive for a cheaper (lower) `marketPricePct`; it is
/// not the contractual quoted margin.
/// @param deal - Borrowed typed StructuredCredit handle; load persisted envelopes once with StructuredCredit.fromJson before calling. The handle is not consumed.
/// @param trancheId - Identifier of the floating-rate tranche whose contractual cashflows are spread-discounted.
/// @param marketJson - Canonical market-context JSON supplying the discount curve and any forward curves or historical fixings required for cashflow projection.
/// @param asOf - ISO-8601 valuation date used for projection and discounting.
/// @param marketPricePct - Clean settlement price as a percentage of the tranche's CURRENT balance (100.0 = par); accrued interest is added once at the deal's quote_settlement_date or the valuation date.
/// @returns The z-spread-equivalent discount margin in decimal units.
/// @throws Error - Thrown if JSON or the date is malformed, the deal is invalid, the tranche is missing or fixed-rate, market_price_pct is not finite and positive, required market data is unavailable, or the spread solve fails or exceeds ±5000 bp.
#[wasm_bindgen(js_name = trancheDiscountMargin)]
pub fn tranche_discount_margin(
    deal: &JsStructuredCredit,
    #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(js_name = "marketPricePct", unchecked_param_type = "number")]
    market_price_pct: JsValue,
) -> Result<f64, JsValue> {
    let market_price_pct = js_f64(&market_price_pct, "marketPricePct")?;
    let tranche_id: &str = &js_string(&tranche_id, "trancheId")?;
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    sc::tranche_discount_margin(&deal.inner, tranche_id, &market, as_of, market_price_pct)
        .map_err(to_js_err)
}

/// Break-even constant default rate (CDR, decimal) for a tranche — the highest
/// CDR at which the tranche takes no principal writedown.
///
/// # Errors
///
/// Throws a JavaScript exception if the market JSON is
/// malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or required market
/// data is missing; or the break-even calculation fails.
/// @param deal - Borrowed typed StructuredCredit handle; load persisted envelopes once with StructuredCredit.fromJson before calling. The handle is not consumed.
/// @param trancheId - Stable tranche identifier used to select the required domain object.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @returns Break-even constant default rate as a decimal, such as `0.02` for 2% CDR.
#[wasm_bindgen(js_name = trancheBreakevenCdr)]
pub fn tranche_breakeven_cdr(
    deal: &JsStructuredCredit,
    #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
) -> Result<f64, JsValue> {
    let tranche_id: &str = &js_string(&tranche_id, "trancheId")?;
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    sc::tranche_breakeven_cdr(&deal.inner, tranche_id, &market, as_of).map_err(to_js_err)
}

/// Option-adjusted spread for a tranche; returns a typed `OasResult` object.
///
/// The result is a plain JavaScript object with snake_case fields (`oas`,
/// `model_price`, `market_price`, `num_paths`, `price_std_error`) — the same
/// shape Python exposes through its typed `OasResult` wrapper. Pass it to
/// `JSON.stringify` if a wire string is needed.
///
/// `marketPricePct` is the clean settlement quote as a percentage of CURRENT balance.
/// The deal's `quote_settlement_date` defaults to valuation. Accrued interest
/// is added exactly once; payments on or before settlement belong to the seller.
/// `configJson`, when present, is a JSON `OasConfig`; the default is used
/// otherwise.
///
/// # Errors
///
/// Throws a JavaScript exception if the market or optional
/// configuration JSON is malformed; the instrument fails pricing validation;
/// `as_of` is invalid; the tranche or discount curve is missing; the OAS solve
/// fails or produces a non-finite result; or the result cannot be converted to
/// a JavaScript value.
/// @param deal - Borrowed typed StructuredCredit handle; load persisted envelopes once with StructuredCredit.fromJson before calling. The handle is not consumed.
/// @param trancheId - Stable tranche identifier used to select the required domain object.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param marketPricePct - Clean settlement quote as a percentage of CURRENT balance; accrued interest is added once.
/// @param configJson - Optional OasConfig JSON; omit to use the default OAS solver configuration.
/// @returns Typed `OasResult` object for the tranche.
#[wasm_bindgen(js_name = trancheOas, unchecked_return_type = "OasResult")]
pub fn tranche_oas(
    deal: &JsStructuredCredit,
    #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(js_name = "marketPricePct", unchecked_param_type = "number")]
    market_price_pct: JsValue,
    #[wasm_bindgen(
        js_name = "configJson",
        unchecked_optional_param_type = "string | null"
    )]
    config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let market_price_pct = js_f64(&market_price_pct, "marketPricePct")?;
    let tranche_id: &str = &js_string(&tranche_id, "trancheId")?;
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    let config: OasConfig = match config_json.as_ref() {
        Some(value) if !(value.is_null() || value.is_undefined()) => {
            from_js_json(value, "OAS config")?
        }
        _ => OasConfig::default(),
    };
    let result = sc::tranche_oas(
        &deal.inner,
        tranche_id,
        &market,
        as_of,
        market_price_pct,
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Scenario (CPR x CDR x severity) table for a tranche; returns a typed
/// `ScenarioTable` object. `gridJson` is a JSON `ScenarioGrid` (`cprs`,
/// `cdrs`, `severities`).
///
/// The result is a plain JavaScript object with snake_case fields
/// (`tranche_id`, `cells`; each cell carries `cpr`, `cdr`, `severity`,
/// `price`, `wal`, `writedown`) — the same shape Python exposes through its
/// typed `ScenarioTable` wrapper. Pass it to `JSON.stringify` if a wire
/// string is needed.
///
/// # Errors
///
/// Throws a JavaScript exception if the market or scenario-grid
/// JSON is malformed; the instrument fails pricing validation; `as_of` is
/// invalid; the tranche or required market data is missing; a scenario fails
/// or produces a non-finite result; or the table cannot be converted to a
/// JavaScript value.
/// @param deal - Borrowed typed StructuredCredit handle; load persisted envelopes once with StructuredCredit.fromJson before calling. The handle is not consumed.
/// @param trancheId - Stable tranche identifier used to select the required domain object.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param gridJson - ScenarioGrid JSON containing the CPR, CDR, and severity axes for the table.
/// @returns Typed `ScenarioTable` object over the CPR/CDR/severity grid.
#[wasm_bindgen(js_name = trancheScenarioTable, unchecked_return_type = "ScenarioTable")]
pub fn tranche_scenario_table(
    deal: &JsStructuredCredit,
    #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(js_name = "gridJson", unchecked_param_type = "string")] grid_json: JsValue,
) -> Result<JsValue, JsValue> {
    let tranche_id: &str = &js_string(&tranche_id, "trancheId")?;
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    let grid: ScenarioGrid = from_js_json(&grid_json, "scenario grid")?;
    let result = sc::tranche_scenario_table(&deal.inner, tranche_id, &market, as_of, &grid)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Per-tranche valuation, yield, spread and risk metrics computed from one
/// tranche's own cashflows — the one per-note analytics entry point.
///
/// `marketPricePct` is a clean settlement quote (% of CURRENT balance). The
/// deal's `quote_settlement_date` defaults to valuation; accrued interest is
/// added once and seller-owned payments are excluded. When omitted the deal
/// quote is used, or its model clean settlement price if no quote is supplied.
/// `metrics` lists registry metric ids (`"dv01"`, `"default01"`, …) valued on
/// this note and returned in `metrics`, so tranche sensitivities sum to the
/// deal's. Returns a typed `TrancheMetrics` object — a plain JavaScript object
/// with the same snake_case fields (`tranche_id`, `currency`, `pv`,
/// `clean_price_pct`, `dirty_price_pct`, `accrued`, `factor`, `wal`, `ytm`,
/// `z_spread_bp`, `cs01`, `spread_duration`, `spread_convexity`,
/// `effective_duration`, `effective_convexity`, `target_price_pct`, `dm_bp`
/// for floaters, `metrics`) Python exposes through its typed `TrancheMetrics`
/// wrapper. Pass it to `JSON.stringify` if a wire string is needed.
///
/// # Errors
///
/// Throws a JavaScript exception if the market JSON is
/// malformed; a metric id is unknown; the instrument fails pricing
/// validation; `as_of` is invalid; the tranche or discount curve is missing;
/// a metric fails or is non-finite; or the result cannot be converted to a
/// JavaScript value.
/// @param deal - Borrowed typed StructuredCredit handle; load persisted envelopes once with StructuredCredit.fromJson before calling. The handle is not consumed.
/// @param trancheId - Stable tranche identifier used to select the required domain object.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param marketPricePct - Optional clean settlement quote as a percentage of CURRENT balance; omit to use the deal quote, or its model clean price when no quote is supplied.
/// @param metrics - Optional registry metric ids to value on this note; omit or pass an empty array for none.
/// @returns Typed `TrancheMetrics` object (PV, clean/dirty price, accrued, WAL, yield, z-spread, CS01, effective duration and convexity, requested registry metrics).
#[wasm_bindgen(js_name = trancheMetrics, unchecked_return_type = "TrancheMetrics")]
pub fn tranche_metrics(
    deal: &JsStructuredCredit,
    #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(
        js_name = "marketPricePct",
        unchecked_optional_param_type = "number | null"
    )]
    market_price_pct: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "string[] | null")] metrics: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let market_price_pct = js_opt_f64(market_price_pct.as_ref(), "marketPricePct")?;
    let metric_ids = js_opt_string_seq(metrics.as_ref(), "metrics")?
        .unwrap_or_default()
        .iter()
        .map(|id| finstack_quant_valuations::metrics::MetricId::parse_strict(id))
        .collect::<finstack_quant_core::Result<Vec<_>>>()
        .map_err(to_js_err)?;
    let tranche_id: &str = &js_string(&tranche_id, "trancheId")?;
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    let result = sc::tranche_metrics(
        &deal.inner,
        tranche_id,
        &market,
        as_of,
        market_price_pct,
        &metric_ids,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
