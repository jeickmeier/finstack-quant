//! Comparable-company analysis bindings.
//!
//! Exposes peer statistics, percentile rank, z-score, OLS fair-value regression,
//! canonical valuation multiples, and composite rich/cheap scoring.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_string};
use crate::utils::{to_js_err, to_js_value};
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
        Some(rank) => to_js_value(&rank).map(Some),
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
        Some(z) => to_js_value(&z).map(Some),
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
        Some(stats) => to_js_value(&stats).map(Some),
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
        Some(result) => to_js_value(&result).map(Some),
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
        Some(result) => to_js_value(&result).map(Some),
        None => Ok(None),
    }
}

/// Composite rich/cheap scoring across multiple dimensions.
///
/// # Errors
///
/// Rejects when `peer_set` or `dimensions` cannot be decoded into its declared
/// schema, when a weight is negative/non-finite, no positive-weight dimension
/// has usable data, the composite is non-finite, or the result cannot be
/// serialized to JavaScript. Unusable dimensions are excluded; regression
/// dimensions do not fall back to univariate scoring.
/// @param peer_set - Comparable-company metric records used to score relative value.
/// @param dimensions - Metric dimensions and finite non-negative relative weights (zero disables, positive usable weights are normalized); each has one optional `x_extractor` for single-factor regression, or null for distribution scoring.
#[wasm_bindgen(js_name = scoreRelativeValue)]
pub fn score_relative_value(peer_set: JsValue, dimensions: JsValue) -> Result<JsValue, JsValue> {
    let ps: fc::PeerSet = from_js_json(&peer_set, "peerSet")?;
    let dims: Vec<fc::ScoringDimension> = from_js_json(&dimensions, "dimensions")?;
    let result = fc::score_relative_value(&ps, &dims).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Compare one subject with its filtered peers (mirrors Rust `relative_value_report`).
///
/// Returns the comps table, peer statistics, regression scatter and composite
/// rich/cheap score in one result, so a caller has nothing to compute. A used
/// dimension and the composite are exactly what `scoreRelativeValue` returns on
/// the same peers and dimensions; where that function throws because no dimension
/// is usable, this one still returns the table.
///
/// @param subject - `CompanyMetrics` of the company being evaluated (object or JSON). Monetary metrics carry no currency and every company's must already be in one currency. It need not be in `universe`.
/// @param universe - Candidate peers as `CompanyMetrics` (array or JSON), in the order the report returns them. A member with the subject's id is never a peer and does not count as excluded.
/// @param config - `RelativeValueReportConfig` (object or JSON): `filter` (optional; a `PeerFilter` in which every field is optional, an omitted field meaning no constraint and an unknown one rejected; an omitted filter makes every other company a peer), `period_basis` (required; `"ltm"`, `"ntm"` or `{custom: "<label>"}`; it labels the report and the metrics are used as given), `dimensions` (required; `ScoringDimension` objects with finite non-negative weights, zero disabling one; may be empty when `multiples` is not) and `multiples` (required; multiple identifiers such as `"ev_ebitda"`, the columns in order; may be empty when `dimensions` is not).
/// @returns `RelativeValueReport` plain object: `subject_id`, `period_basis`, `peer_count`, `excluded_count` (universe members the filter dropped, not counting the subject), `companies` (subject first, then peers in universe order: `id`, `is_subject`, `values` with one number or null per configured multiple), `multiples` in config order (`multiple`, `subject_value`, `peer_stats`, `percentile`, `z_score`), `dimensions` in config order including excluded ones (`label`, `weight`, `direction`, `y`, `x`, `subject_y`, `subject_x`, `peer_stats`, `percentile`, `z_score`, `regression`, `signal` positive meaning cheap, `excluded_reason`, and `points` for the scatter with `company_id`, `x`, `y`, `fitted_y`), `composite` (the `scoreRelativeValue` result, or null) and `composite_excluded_reason` (`no_dimensions`, `no_usable_dimension`, `non_finite_score`, or null).
/// @throws If an argument is malformed or carries an unknown field (a misspelt `CompanyMetrics` or `PeerFilter` key included), `dimensions` and `multiples` are both empty, or a weight is negative or non-finite (kind `validation`). No usable dimension is not an error: the table is returned with a null `composite`.
#[wasm_bindgen(js_name = relativeValueReport)]
pub fn relative_value_report(
    subject: JsValue,
    universe: JsValue,
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let subject: fc::CompanyMetrics = from_js_json(&subject, "subject")?;
    let universe: Vec<fc::CompanyMetrics> = from_js_json(&universe, "universe")?;
    let config: fc::RelativeValueReportConfig = from_js_json(&config, "config")?;
    to_js_value(&fc::relative_value_report(&subject, &universe, &config).map_err(to_js_err)?)
}
