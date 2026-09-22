//! WASM bindings for standalone structured-credit tranche analytics.
//!
//! Mirrors the Python `StructuredCredit` metric methods — discount margin, OAS,
//! break-even CDR and the scenario table — as free functions that parse the
//! canonical envelope and dispatch to the typed structured-credit domain API.
//! The OAS, metrics, and scenario-table entry points return structured
//! JavaScript objects with the same snake_case fields Python exposes through
//! its `OasResult` / `TrancheMetrics` / `ScenarioTable` wrappers. The exported
//! JS surface lives under `valuations.instruments`.

use super::pricing::parse_market_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    self as rust_structured_credit, StructuredCredit,
};
use finstack_quant_valuations::instruments::InstrumentJson;
use wasm_bindgen::prelude::*;

fn parse_structured_credit(instrument_json: &str) -> Result<StructuredCredit, JsValue> {
    match finstack_quant_valuations::pricer::json::parse_instrument_from_json(instrument_json)
        .map_err(to_js_err)?
    {
        InstrumentJson::StructuredCredit(deal) => Ok(*deal),
        other => Err(to_js_err(finstack_quant_core::Error::Validation(format!(
            "expected a structured_credit instrument, got {}",
            other.type_tag()
        )))),
    }
}

/// Z-spread-equivalent discount margin for a floating-rate tranche, returned in
/// decimal units (`0.015` = 150 bp).
///
/// Contractual cashflows are projected without changing coupon projection,
/// then a constant additive spread is applied to the discount curve. The result
/// is zero at model PV, negative for a richer (higher) `targetPv`, and positive
/// for a cheaper (lower) `targetPv`; it is not the contractual quoted margin.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param tranche_id - Identifier of the floating-rate tranche whose contractual cashflows are spread-discounted.
/// @param market_json - Canonical market-context JSON supplying the discount curve and any forward curves or historical fixings required for cashflow projection.
/// @param as_of - ISO-8601 valuation date used for projection and discounting.
/// @param target_pv - Positive dirty settlement amount in tranche currency, including accrued interest once; settlement uses the deal's quote_settlement_date or the valuation date.
/// @returns The z-spread-equivalent discount margin in decimal units.
/// @throws Error - Thrown if JSON or the date is malformed, the deal is invalid, the tranche is missing or fixed-rate, target_pv is non-finite, required market data is unavailable, or the spread solve fails or exceeds ±5000 bp.
#[wasm_bindgen(js_name = structuredCreditTrancheDiscountMargin)]
pub fn structured_credit_tranche_discount_margin(
    instrument_json: &str,
    tranche_id: &str,
    market_json: &str,
    as_of: &str,
    target_pv: f64,
) -> Result<f64, JsValue> {
    let deal = parse_structured_credit(instrument_json)?;
    let market = parse_market_json(market_json)?;
    rust_structured_credit::structured_credit_tranche_discount_margin(
        &deal, tranche_id, &market, as_of, target_pv,
    )
    .map_err(to_js_err)
}

/// Break-even constant default rate (CDR, decimal) for a tranche — the highest
/// CDR at which the tranche takes no principal writedown.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument or market JSON is
/// malformed; the instrument fails pricing validation or is not a
/// structured-credit deal; `as_of` is invalid; the tranche or required market
/// data is missing; or the break-even calculation fails.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param tranche_id - Stable tranche identifier used to select the required domain object.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
#[wasm_bindgen(js_name = structuredCreditTrancheBreakevenCdr)]
pub fn structured_credit_tranche_breakeven_cdr(
    instrument_json: &str,
    tranche_id: &str,
    market_json: &str,
    as_of: &str,
) -> Result<f64, JsValue> {
    let deal = parse_structured_credit(instrument_json)?;
    let market = parse_market_json(market_json)?;
    rust_structured_credit::structured_credit_tranche_breakeven_cdr(
        &deal, tranche_id, &market, as_of,
    )
    .map_err(to_js_err)
}

/// Option-adjusted spread for a tranche; returns a typed `OasResult` object.
///
/// The result is a plain JavaScript object with snake_case fields (`oas`,
/// `model_price`, `market_price`, `num_paths`, `price_std_error`) — the same
/// shape Python exposes through its typed `OasResult` wrapper. Pass it to
/// `JSON.stringify` if a wire string is needed.
///
/// `marketPricePct` is the clean settlement quote as a percentage of original balance.
/// The deal's `quote_settlement_date` defaults to valuation. Accrued interest
/// is added exactly once; payments on or before settlement belong to the seller.
/// `configJson`, when present, is a JSON `OasConfig`; the default is used
/// otherwise.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument, market, or optional
/// configuration JSON is malformed; the instrument fails pricing validation;
/// `as_of` is invalid; the tranche or discount curve is missing; the OAS solve
/// fails or produces a non-finite result; or the result cannot be converted to
/// a JavaScript value.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param tranche_id - Stable tranche identifier used to select the required domain object.
/// @param market_price_pct - Clean settlement quote as a percentage of original balance; accrued interest is added once.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param config_json - Optional OasConfig JSON; omit to use the default OAS solver configuration.
#[wasm_bindgen(js_name = structuredCreditTrancheOas)]
pub fn structured_credit_tranche_oas(
    instrument_json: &str,
    tranche_id: &str,
    market_price_pct: f64,
    market_json: &str,
    as_of: &str,
    config_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let deal = parse_structured_credit(instrument_json)?;
    let market = parse_market_json(market_json)?;
    let result = rust_structured_credit::structured_credit_tranche_oas(
        &deal,
        tranche_id,
        market_price_pct,
        &market,
        as_of,
        config_json.as_deref(),
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
/// Throws a JavaScript exception if the instrument, market, or scenario-grid
/// JSON is malformed; the instrument fails pricing validation; `as_of` is
/// invalid; the tranche or required market data is missing; a scenario fails
/// or produces a non-finite result; or the table cannot be converted to a
/// JavaScript value.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param tranche_id - Stable tranche identifier used to select the required domain object.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param grid_json - ScenarioGrid JSON containing the CPR, CDR, and severity axes for the table.
#[wasm_bindgen(js_name = structuredCreditTrancheScenarioTable)]
pub fn structured_credit_tranche_scenario_table(
    instrument_json: &str,
    tranche_id: &str,
    market_json: &str,
    as_of: &str,
    grid_json: &str,
) -> Result<JsValue, JsValue> {
    let deal = parse_structured_credit(instrument_json)?;
    let market = parse_market_json(market_json)?;
    let result = rust_structured_credit::structured_credit_tranche_scenario_table(
        &deal, tranche_id, &market, as_of, grid_json,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Per-tranche risk/spread metrics (PV, price, WAL, z-spread, CS01, spread/
/// modified duration, convexity) computed from one tranche's own cashflows.
///
/// `marketPricePct` is a clean settlement quote (% of original balance). The
/// deal's `quote_settlement_date` defaults to valuation; accrued interest is
/// added once and seller-owned payments are excluded. When omitted the deal
/// quote is used, or its model clean settlement price if no quote is supplied. Returns a typed `TrancheMetrics` object —
/// a plain JavaScript object with the same snake_case fields (`tranche_id`,
/// `currency`, `pv`, `price_pct`, `wal`, `z_spread_bp`, `cs01`,
/// `spread_duration`, `spread_convexity`, `modified_duration`, `convexity`,
/// `target_price_pct`, `dm_bp` for floaters)
/// Python exposes through its typed `TrancheMetrics` wrapper. Pass it to
/// `JSON.stringify` if a wire string is needed.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument or market JSON is
/// malformed; the instrument fails pricing validation; `as_of` is invalid;
/// the tranche or discount curve is missing; a metric fails or is non-finite;
/// or the result cannot be converted to a JavaScript value.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param tranche_id - Stable tranche identifier used to select the required domain object.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param market_price_pct - Optional clean settlement quote as a percentage of original balance; omit to use the deal quote, or its model clean price when no quote is supplied.
#[wasm_bindgen(js_name = structuredCreditTrancheMetrics)]
pub fn structured_credit_tranche_metrics(
    instrument_json: &str,
    tranche_id: &str,
    market_json: &str,
    as_of: &str,
    market_price_pct: Option<f64>,
) -> Result<JsValue, JsValue> {
    let deal = parse_structured_credit(instrument_json)?;
    let market = parse_market_json(market_json)?;
    let result = rust_structured_credit::structured_credit_tranche_metrics(
        &deal,
        tranche_id,
        &market,
        as_of,
        market_price_pct,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(structured_credit_tranche_discount_margin(
            "{}",
            "missing",
            "not-market-json",
            "not-a-date",
            f64::NAN,
        )
        .is_err());
        assert!(structured_credit_tranche_breakeven_cdr(
            "{}",
            "missing",
            "not-market-json",
            "not-a-date",
        )
        .is_err());
        assert!(structured_credit_tranche_oas(
            "{}",
            "missing",
            f64::NAN,
            "not-market-json",
            "not-a-date",
            Some("not-json".to_string()),
        )
        .is_err());
        assert!(structured_credit_tranche_scenario_table(
            "{}",
            "missing",
            "not-market-json",
            "not-a-date",
            "not-json",
        )
        .is_err());
        assert!(structured_credit_tranche_metrics(
            "{}",
            "missing",
            "not-market-json",
            "not-a-date",
            Some(f64::NAN),
        )
        .is_err());
    }
}
