//! `ValuationResult` methods as free functions.
//!
//! WASM valuation results are plain objects (with 64-bit fields as `BigInt`),
//! so each Rust `ValuationResult` method that Python exposes on its typed
//! wrapper is a function taking the result here, next to
//! `valuationResultMetricSeries` and `valuationResultToJson` in
//! [`super::pricing`].

use crate::utils::input::{from_js_json, js_string};
use crate::utils::to_js_value;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

fn parse(result: &JsValue) -> Result<ValuationResult, JsValue> {
    from_js_json(result, "result")
}

/// Present value as an exact decimal string.
///
/// Twin of Python `ValuationResult.price_decimal`: the `Money` amount without
/// the binary rounding a JavaScript number would add.
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns The present value amount as a decimal string, e.g. `"1042315.67"`.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultPriceDecimal)]
pub fn valuation_result_price_decimal(result: JsValue) -> Result<String, JsValue> {
    Ok(parse(&result)?.value.amount_decimal().to_string())
}

/// One measure of a valuation result by its metric key.
///
/// Twin of Python `ValuationResult.get_metric` (Rust `ValuationResult::metric_str`).
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @param key - Fully qualified metric key, e.g. `"dv01"` or `"bucketed_dv01::USD-OIS::10y"`.
/// @returns The measure value, or `undefined` when the result has no such key.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema, and kind `invalid_type` if `key` is not a string.
#[wasm_bindgen(js_name = valuationResultGetMetric)]
pub fn valuation_result_get_metric(result: JsValue, key: JsValue) -> Result<Option<f64>, JsValue> {
    Ok(parse(&result)?.metric_str(&js_string(&key, "key")?))
}

/// Metric keys of a valuation result, in measure order.
///
/// Twin of Python `ValuationResult.metric_keys`.
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns Fully qualified metric keys in the order the measures were computed.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultMetricKeys)]
pub fn valuation_result_metric_keys(result: JsValue) -> Result<Vec<String>, JsValue> {
    Ok(parse(&result)?
        .measures
        .keys()
        .map(ToString::to_string)
        .collect())
}

/// Number of measures in a valuation result.
///
/// Twin of Python `ValuationResult.metric_count`.
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns Count of measure keys.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultMetricCount)]
pub fn valuation_result_metric_count(result: JsValue) -> Result<usize, JsValue> {
    Ok(parse(&result)?.measures.len())
}

/// Unit of every measure in a valuation result.
///
/// Twin of Python `ValuationResult.metric_units` (Rust `ValuationResult::metric_units`).
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns Map of metric key to its unit name (for example `"currency_per_bp"`), in measure order.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultMetricUnits)]
pub fn valuation_result_metric_units(result: JsValue) -> Result<JsValue, JsValue> {
    let units: IndexMap<String, &'static str> = parse(&result)?
        .metric_units()
        .into_iter()
        .map(|(key, unit)| (key, unit.as_str()))
        .collect();
    to_js_value(&units)
}

/// Whether every covenant in a valuation result passed.
///
/// Twin of Python `ValuationResult.all_covenants_passed` (Rust `ValuationResult::all_covenants_passed`).
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns `true` when all covenants passed or the result carries no covenant reports.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultAllCovenantsPassed)]
pub fn valuation_result_all_covenants_passed(result: JsValue) -> Result<bool, JsValue> {
    Ok(parse(&result)?.all_covenants_passed())
}

/// Identifiers of the covenants that failed in a valuation result.
///
/// Twin of Python `ValuationResult.failed_covenants` (Rust `ValuationResult::failed_covenants`).
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @returns Covenant identifiers whose report did not pass; empty when none failed.
/// @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema.
#[wasm_bindgen(js_name = valuationResultFailedCovenants)]
pub fn valuation_result_failed_covenants(result: JsValue) -> Result<Vec<String>, JsValue> {
    Ok(parse(&result)?
        .failed_covenants()
        .into_iter()
        .map(String::from)
        .collect())
}
