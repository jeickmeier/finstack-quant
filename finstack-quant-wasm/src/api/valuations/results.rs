//! `ValuationResult` methods as free functions.
//!
//! WASM valuation results are plain objects (with 64-bit fields as `BigInt`),
//! so each Rust `ValuationResult` method that Python exposes on its typed
//! wrapper is a function taking the result here.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value, to_js_value_with_bigints};
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

fn parse(result: &JsValue) -> Result<ValuationResult, JsValue> {
    from_js_json(result, "result")
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

pub(super) fn valuation_result_json(result: ValuationResult) -> Result<String, JsValue> {
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Convert a [`ValuationResult`] into a structured JavaScript object.
///
/// The `price*` entry points return this rather than a JSON string so JS
/// callers read `result.value.amount` / `result.measures.dv01` directly, the
/// same way Python callers read `PyValuationResult` getters. It uses
/// JSON-compatible map/object conventions while preserving every 64-bit
/// integer as JavaScript `BigInt`. In particular, Monte Carlo seeds span the
/// full `u64` range and cannot be narrowed to a JavaScript `number`.
pub(super) fn valuation_result_value(result: &ValuationResult) -> Result<JsValue, JsValue> {
    to_js_value_with_bigints(result)
}

/// Validate and serialize a `ValuationResult` object or JSON text to canonical JSON.
///
/// The inverse of the structured `priceInstrument*` return: it accepts the
/// plain object those entry points return, with 64-bit fields such as the
/// Monte Carlo `seed` as `BigInt`, and writes the same canonical JSON as
/// Python `ValuationResult.to_json()`, keeping every integer exact. Use it in
/// place of `JSON.stringify`, which throws on `BigInt`.
/// @param result - Canonical JSON text or a `ValuationResult` object returned by `priceInstrument`,
/// `priceInstrumentWithMarket`, a typed instrument's `price`, or a portfolio
/// valuation's `valuation_result` entry; 64-bit fields must be `BigInt` or
/// safe-integer numbers.
/// @returns Canonical `ValuationResult` JSON text.
///
/// # Errors
///
/// Throws a JavaScript exception if `result` does not match the
/// `ValuationResult` schema (for example a seed given as a string) or the
/// canonical result cannot be serialized.
#[wasm_bindgen(js_name = valuationResultToJson)]
pub fn valuation_result_to_json(result: JsValue) -> Result<String, JsValue> {
    let result: ValuationResult = from_js_json(&result, "result")?;
    valuation_result_json(result)
}

/// Decoded series of one composite base metric from a valuation result.
///
/// Free-function twin of Python `ValuationResult.metric_series` (Rust
/// `ValuationResult::metric_series`): every measure key that encodes a series
/// of `base` (e.g. `bucketed_dv01::USD-OIS::10y`) is decoded into its
/// components; scalar entries stored directly under `base` are excluded.
/// @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
/// @param base - Canonical base metric identifier (e.g. `"bucketed_dv01"`).
/// @returns `[components, value]` pairs in measure order.
///
/// # Errors
///
/// Throws with kind `validation` if `result` does not match the
/// `ValuationResult` schema or `base` is not a canonically encoded metric key.
#[wasm_bindgen(js_name = valuationResultMetricSeries)]
pub fn valuation_result_metric_series(result: JsValue, base: JsValue) -> Result<JsValue, JsValue> {
    let result: ValuationResult = from_js_json(&result, "result")?;
    let base: finstack_quant_valuations::metrics::MetricId =
        js_string(&base, "base")?.parse().map_err(to_js_err)?;
    to_js_value(&result.metric_series(&base))
}
