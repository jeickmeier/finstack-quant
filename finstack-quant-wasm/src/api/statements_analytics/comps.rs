//! Comparable-company analysis bindings.
//!
//! Exposes peer statistics, percentile rank, z-score, OLS fair-value regression,
//! canonical valuation multiples, and composite rich/cheap scoring.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_string};
use crate::utils::to_js_err;
use finstack_quant_statements_analytics::analysis as fc;
use wasm_bindgen::prelude::*;

/// Percentile rank of `value` within `values` on a 0-1 scale (Rust `percentile_rank(values, value)`).
///
/// Returns `undefined` when `values` is empty rather than a synthetic 0.5.
///
/// # Errors
///
/// Rejects when `values` is not a numeric JavaScript array or the finite rank
/// cannot be serialized. Empty/non-finite peer data or a non-finite `value`
/// return `undefined` rather than rejecting.
/// @param values - Peer observations forming the comparison universe; non-finite entries are ignored.
/// @param value - Subject-company metric value to rank against the peer sample.
#[wasm_bindgen(js_name = percentileRank)]
pub fn percentile_rank(values: JsValue, value: JsValue) -> Result<Option<JsValue>, JsValue> {
    let value = js_f64(&value, "value")?;
    let values = js_f64_seq(&values, "values")?;
    match fc::percentile_rank(&values, value) {
        Some(rank) => crate::utils::to_js_value(&rank).map(Some),
        None => Ok(None),
    }
}

/// Z-score of `value` within `values` (Rust `z_score(values, value)`).
///
/// Returns `undefined` when fewer than two observations are provided or the
/// peer variance is zero, instead of a synthetic zero.
///
/// # Errors
///
/// Rejects when `values` is not a numeric JavaScript array or the computed
/// score cannot be serialized. Insufficient data, zero variance, or a
/// non-finite `value` return `undefined` rather than rejecting.
/// @param values - Peer observations the subject is standardized against; non-finite entries are ignored.
/// @param value - Subject-company metric value to standardize against the peer sample.
#[wasm_bindgen(js_name = zScore)]
pub fn z_score(values: JsValue, value: JsValue) -> Result<Option<JsValue>, JsValue> {
    let value = js_f64(&value, "value")?;
    let values = js_f64_seq(&values, "values")?;
    match fc::z_score(&values, value) {
        Some(z) => crate::utils::to_js_value(&z).map(Some),
        None => Ok(None),
    }
}

/// Descriptive statistics over a peer distribution (Rust `peer_stats(values)`).
///
/// Returns `undefined` (matching the other comps helpers) when `values` is
/// empty.
///
/// # Errors
///
/// Rejects when `values` is not a numeric JavaScript array or the statistics
/// cannot be serialized. No finite observations return `undefined`.
/// @param values - Peer metric observations; non-finite entries are ignored.
#[wasm_bindgen(js_name = peerStats)]
pub fn peer_stats(values: JsValue) -> Result<Option<JsValue>, JsValue> {
    let values = js_f64_seq(&values, "values")?;
    match fc::peer_stats(&values) {
        Some(stats) => crate::utils::to_js_value(&stats).map(Some),
        None => Ok(None),
    }
}

/// Single-factor OLS fit of `y_values` on `x_values` evaluated at the subject
/// observation (Rust `regression_fair_value(x_values, y_values, subject_x, subject_y)`).
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
    subject_x: JsValue,
    subject_y: JsValue,
) -> Result<Option<JsValue>, JsValue> {
    let subject_x = js_f64(&subject_x, "subjectX")?;
    let subject_y = js_f64(&subject_y, "subjectY")?;
    let x = js_f64_seq(&x_values, "xValues")?;
    let y = js_f64_seq(&y_values, "yValues")?;
    match fc::regression_fair_value(&x, &y, subject_x, subject_y) {
        Some(result) => crate::utils::to_js_value(&result).map(Some),
        None => Ok(None),
    }
}

/// Compute a canonical valuation multiple for a company-metric bag.
///
/// # Errors
///
/// Rejects when `company_metrics` is not an object of numbers (or `null`),
/// `multiple` is not a supported canonical identifier, or the computed value
/// cannot be serialized. Missing, `null` or non-finite inputs and
/// non-positive denominators return `undefined`.
/// @param company_metrics - Flat snake_case metric object (`enterprise_value`, `ebitda`, ...) supplying numerator and denominator inputs; a `null` value means the metric is missing.
/// @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
#[wasm_bindgen(js_name = computeMultiple)]
pub fn compute_multiple(
    company_metrics: JsValue,
    multiple: JsValue,
) -> Result<Option<JsValue>, JsValue> {
    let multiple: &str = &js_string(&multiple, "multiple")?;
    let metrics_map: std::collections::BTreeMap<String, Option<f64>> =
        from_js_json(&company_metrics, "companyMetrics")?;
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
    let ps: fc::PeerSet = from_js_json(&peer_set, "peerSet")?;
    let dims: Vec<fc::ScoringDimension> = from_js_json(&dimensions, "dimensions")?;
    let result = fc::score_relative_value(&ps, &dims).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}
