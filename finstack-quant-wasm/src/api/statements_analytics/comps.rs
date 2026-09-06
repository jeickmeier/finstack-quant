//! Comparable-company analysis bindings.
//!
//! Exposes peer statistics, percentile rank, z-score, OLS fair-value regression,
//! canonical valuation multiples, and composite rich/cheap scoring.

use crate::utils::to_js_err;
use finstack_quant_statements_analytics::analysis as fc;
use wasm_bindgen::prelude::*;

/// Percentile rank of `value` within `data` on a 0-1 scale (Rust `percentile_rank(values, value)` argument order).
///
/// Returns `undefined` when `data` is empty rather than a synthetic 0.5.
///
/// # Errors
///
/// Rejects when `data` is not a numeric JavaScript array or the finite rank
/// cannot be serialized. Empty/non-finite peer data or a non-finite `value`
/// return `undefined` rather than rejecting.
/// @param data - Non-empty numeric observation array used by the requested statistic.
/// @param value - Subject-company metric value to rank against the peer sample.
#[wasm_bindgen(js_name = percentileRank)]
pub fn percentile_rank(data: JsValue, value: f64) -> Result<Option<JsValue>, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::percentile_rank(&d, value) {
        Some(rank) => crate::utils::to_js_value(&rank).map(Some),
        None => Ok(None),
    }
}

/// Z-score of `value` within `data` (Rust `z_score(values, value)` argument order).
///
/// Returns `undefined` when fewer than two observations are provided or the
/// peer variance is zero, instead of a synthetic zero.
///
/// # Errors
///
/// Rejects when `data` is not a numeric JavaScript array or the computed score
/// cannot be serialized. Insufficient data, zero variance, or a non-finite
/// `value` return `undefined` rather than rejecting.
/// @param data - Non-empty numeric observation array used by the requested statistic.
/// @param value - Subject-company metric value to standardize against the peer sample.
#[wasm_bindgen(js_name = zScore)]
pub fn z_score(data: JsValue, value: f64) -> Result<Option<JsValue>, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::z_score(&d, value) {
        Some(z) => crate::utils::to_js_value(&z).map(Some),
        None => Ok(None),
    }
}

/// Descriptive statistics over a peer distribution.
///
/// Returns `undefined` (matching the other comps helpers) when `data` is empty.
///
/// # Errors
///
/// Rejects when `data` is not a numeric JavaScript array or the statistics
/// cannot be serialized. No finite observations return `undefined`.
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = peerStats)]
pub fn peer_stats(data: JsValue) -> Result<Option<JsValue>, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::peer_stats(&d) {
        Some(stats) => crate::utils::to_js_value(&stats).map(Some),
        None => Ok(None),
    }
}

/// Single-factor OLS fit of `y` on `x` evaluated at the subject observation.
///
/// # Errors
///
/// Rejects when `x_values` or `y_values` is not a numeric JavaScript array, or
/// the regression result cannot be serialized. Fewer than three paired values
/// or an unidentifiable fit returns `undefined`, as do unequal lengths and
/// non-finite numeric inputs or outputs.
/// @param x_values - Comparable-company independent-variable values aligned with y_values.
/// @param y_values - Comparable-company dependent-variable values aligned with x_values.
/// @param subject_x - Subject company's independent-variable value for the fitted regression.
/// @param subject_y - Subject company's observed dependent-variable value for relative-value comparison.
#[wasm_bindgen(js_name = regressionFairValue)]
pub fn regression_fair_value(
    x_values: JsValue,
    y_values: JsValue,
    subject_x: f64,
    subject_y: f64,
) -> Result<Option<JsValue>, JsValue> {
    let x: Vec<f64> = serde_wasm_bindgen::from_value(x_values).map_err(to_js_err)?;
    let y: Vec<f64> = serde_wasm_bindgen::from_value(y_values).map_err(to_js_err)?;
    match fc::regression_fair_value(&x, &y, subject_x, subject_y) {
        Some(result) => crate::utils::to_js_value(&result).map(Some),
        None => Ok(None),
    }
}

/// Compute a canonical valuation multiple for a company-metric bag.
///
/// # Errors
///
/// Rejects when `company_metrics` is not a string-to-number JavaScript object,
/// `multiple` is not a supported canonical identifier, or the computed value
/// cannot be serialized. Missing or non-finite inputs and non-positive
/// denominators return `undefined`.
/// @param company_metrics - Company financial-metric object supplying numerator and denominator inputs.
/// @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
#[wasm_bindgen(js_name = computeMultiple)]
pub fn compute_multiple(
    company_metrics: JsValue,
    multiple: &str,
) -> Result<Option<JsValue>, JsValue> {
    let metrics_map: std::collections::BTreeMap<String, f64> =
        serde_wasm_bindgen::from_value(company_metrics).map_err(to_js_err)?;
    let metrics = fc::CompanyMetrics::from_flat_metrics("subject", metrics_map);
    let multiple = multiple.parse::<fc::Multiple>().map_err(to_js_err)?;
    match fc::compute_multiple(&metrics, multiple) {
        Some(result) => crate::utils::to_js_value(&result).map(Some),
        None => Ok(None),
    }
}

/// Composite rich/cheap scoring across multiple dimensions.
///
/// # Errors
///
/// Rejects when `peer_set` or `dimensions` cannot be decoded into its declared
/// schema, when no scoring dimensions are supplied, or when the result cannot
/// be serialized to JavaScript.
/// @param peer_set - Comparable-company metric records used to score relative value.
/// @param dimensions - Metric dimensions and weights; each has one optional `x_extractor` for single-factor regression, or null for distribution scoring.
#[wasm_bindgen(js_name = scoreRelativeValue)]
pub fn score_relative_value(peer_set: JsValue, dimensions: JsValue) -> Result<JsValue, JsValue> {
    let ps: fc::PeerSet = serde_wasm_bindgen::from_value(peer_set).map_err(to_js_err)?;
    // Decode the full object before the typed contract: serde-wasm-bindgen's
    // struct visitor reads declared properties and can omit unknown fields.
    let dimensions: serde_json::Value =
        serde_wasm_bindgen::from_value(dimensions).map_err(to_js_err)?;
    let dims: Vec<fc::ScoringDimension> = serde_json::from_value(dimensions).map_err(to_js_err)?;
    let result = fc::score_relative_value(&ps, &dims).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}
