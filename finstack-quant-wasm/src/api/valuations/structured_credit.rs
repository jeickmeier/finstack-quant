//! WASM bindings for standalone structured-credit tranche analytics.
//!
//! Mirrors the Python `StructuredCredit` metric methods — discount margin, OAS,
//! break-even CDR and the scenario table — as free functions that wrap the
//! `pricer::structured_credit_*_json` entry points: parse the market JSON,
//! dispatch, and return JSON (or a scalar). The exported JS surface lives under
//! `valuations.instruments`.

use super::pricing::parse_market_json;
use crate::utils::{to_js_err, to_js_error};
use wasm_bindgen::prelude::*;

/// Discount margin (decimal) for a floating-rate tranche.
///
/// `targetPv` is interpreted in the named tranche's currency.
#[wasm_bindgen(js_name = structuredCreditTrancheDiscountMargin)]
pub fn structured_credit_tranche_discount_margin(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    tranche_id: &str,
    target_pv: f64,
) -> Result<f64, JsValue> {
    let market = parse_market_json(market_json)?;
    finstack_quant_valuations::pricer::structured_credit_tranche_discount_margin_json(
        instrument_json,
        tranche_id,
        &market,
        as_of,
        target_pv,
    )
    .map_err(|e| to_js_error(&e))
}

/// Break-even constant default rate (CDR, decimal) for a tranche — the highest
/// CDR at which the tranche takes no principal writedown.
#[wasm_bindgen(js_name = structuredCreditTrancheBreakevenCdr)]
pub fn structured_credit_tranche_breakeven_cdr(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    tranche_id: &str,
) -> Result<f64, JsValue> {
    let market = parse_market_json(market_json)?;
    finstack_quant_valuations::pricer::structured_credit_tranche_breakeven_cdr_json(
        instrument_json,
        tranche_id,
        &market,
        as_of,
    )
    .map_err(|e| to_js_error(&e))
}

/// Option-adjusted spread for a tranche; returns a JSON `OasResult`.
///
/// `marketPricePct` is the quoted price as a percentage of original balance.
/// `config`, when present, is a JSON `OasConfig`; the default is used otherwise.
#[wasm_bindgen(js_name = structuredCreditTrancheOas)]
pub fn structured_credit_tranche_oas(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    tranche_id: &str,
    market_price_pct: f64,
    config: Option<String>,
) -> Result<String, JsValue> {
    let market = parse_market_json(market_json)?;
    let result = finstack_quant_valuations::pricer::structured_credit_tranche_oas_json(
        instrument_json,
        tranche_id,
        market_price_pct,
        &market,
        as_of,
        config.as_deref(),
    )
    .map_err(|e| to_js_error(&e))?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Scenario (CPR x CDR x severity) table for a tranche; returns a JSON
/// `ScenarioTable`. `grid` is a JSON `ScenarioGrid` (`cprs`, `cdrs`,
/// `severities`).
#[wasm_bindgen(js_name = structuredCreditTrancheScenarioTable)]
pub fn structured_credit_tranche_scenario_table(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    tranche_id: &str,
    grid: &str,
) -> Result<String, JsValue> {
    let market = parse_market_json(market_json)?;
    let result = finstack_quant_valuations::pricer::structured_credit_tranche_scenario_table_json(
        instrument_json,
        tranche_id,
        &market,
        as_of,
        grid,
    )
    .map_err(|e| to_js_error(&e))?;
    serde_json::to_string(&result).map_err(to_js_err)
}
