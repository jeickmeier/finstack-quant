//! Comparable-company analysis bindings.
//!
//! Exposes peer statistics, percentile rank, z-score, OLS fair-value regression,
//! canonical valuation multiples, and composite rich/cheap scoring.

use crate::utils::to_js_err;
use finstack_quant_statements_analytics::analysis as fc;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

fn map_to_company_metrics(values: BTreeMap<String, f64>) -> fc::CompanyMetrics {
    let mut metrics = fc::CompanyMetrics::new("subject");
    for (name, value) in values {
        match name.as_str() {
            "enterprise_value" => metrics.enterprise_value = Some(value),
            "market_cap" => metrics.market_cap = Some(value),
            "share_price" => metrics.share_price = Some(value),
            "oas_bps" => metrics.oas_bps = Some(value),
            "yield_pct" => metrics.yield_pct = Some(value),
            "ebitda" => metrics.ebitda = Some(value),
            "revenue" => metrics.revenue = Some(value),
            "ebit" => metrics.ebit = Some(value),
            "ufcf" => metrics.ufcf = Some(value),
            "lfcf" => metrics.lfcf = Some(value),
            "net_income" => metrics.net_income = Some(value),
            "book_value" => metrics.book_value = Some(value),
            "tangible_book_value" => metrics.tangible_book_value = Some(value),
            "dividends_per_share" => metrics.dividends_per_share = Some(value),
            "leverage" => metrics.leverage = Some(value),
            "interest_coverage" => metrics.interest_coverage = Some(value),
            "revenue_growth" => metrics.revenue_growth = Some(value),
            "ebitda_margin" => metrics.ebitda_margin = Some(value),
            _ => {
                metrics.custom.insert(name, value);
            }
        }
    }
    metrics
}

/// Percentile rank of `value` within `data` on a 0-1 scale.
///
/// Returns `null` when `data` is empty rather than a synthetic 0.5.
#[wasm_bindgen(js_name = percentileRank)]
/// @param value - Subject-company metric value to rank against the peer sample.
/// @param data - Non-empty numeric observation array used by the requested statistic.
pub fn percentile_rank(value: f64, data: JsValue) -> Result<JsValue, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::percentile_rank(&d, value) {
        Some(rank) => serde_wasm_bindgen::to_value(&rank).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Z-score of `value` within `data`.
///
/// Returns `null` when fewer than two observations are provided or the
/// peer variance is zero, instead of a synthetic zero.
#[wasm_bindgen(js_name = zScore)]
/// @param value - Subject-company metric value to standardize against the peer sample.
/// @param data - Non-empty numeric observation array used by the requested statistic.
pub fn z_score(value: f64, data: JsValue) -> Result<JsValue, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::z_score(&d, value) {
        Some(z) => serde_wasm_bindgen::to_value(&z).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Descriptive statistics over a peer distribution.
///
/// Returns `null` (matching the other comps helpers) when `data` is empty.
#[wasm_bindgen(js_name = peerStats)]
/// @param data - Non-empty numeric observation array used by the requested statistic.
pub fn peer_stats(data: JsValue) -> Result<JsValue, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::peer_stats(&d) {
        Some(stats) => serde_wasm_bindgen::to_value(&stats).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Single-factor OLS fit of `y` on `x` evaluated at the subject observation.
#[wasm_bindgen(js_name = regressionFairValue)]
/// @param x_values - Comparable-company independent-variable values aligned with y_values.
/// @param y_values - Comparable-company dependent-variable values aligned with x_values.
/// @param subject_x - Subject company's independent-variable value for the fitted regression.
/// @param subject_y - Subject company's observed dependent-variable value for relative-value comparison.
pub fn regression_fair_value(
    x_values: JsValue,
    y_values: JsValue,
    subject_x: f64,
    subject_y: f64,
) -> Result<JsValue, JsValue> {
    let x: Vec<f64> = serde_wasm_bindgen::from_value(x_values).map_err(to_js_err)?;
    let y: Vec<f64> = serde_wasm_bindgen::from_value(y_values).map_err(to_js_err)?;
    match fc::regression_fair_value(&x, &y, subject_x, subject_y) {
        Some(result) => serde_wasm_bindgen::to_value(&result).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Compute a canonical valuation multiple for a company-metric bag.
#[wasm_bindgen(js_name = computeMultiple)]
/// @param company_metrics - Company financial-metric object supplying numerator and denominator inputs.
/// @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
pub fn compute_multiple(company_metrics: JsValue, multiple: &str) -> Result<JsValue, JsValue> {
    let metrics_map: BTreeMap<String, f64> =
        serde_wasm_bindgen::from_value(company_metrics).map_err(to_js_err)?;
    let metrics = map_to_company_metrics(metrics_map);
    let multiple = multiple.parse::<fc::Multiple>().map_err(to_js_err)?;
    match fc::compute_multiple(&metrics, multiple) {
        Some(result) => serde_wasm_bindgen::to_value(&result).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Composite rich/cheap scoring across multiple dimensions.
#[wasm_bindgen(js_name = scoreRelativeValue)]
/// @param peer_set - Comparable-company metric records used to score relative value.
/// @param dimensions - Metric dimensions and weights included in the relative-value score.
pub fn score_relative_value(peer_set: JsValue, dimensions: JsValue) -> Result<JsValue, JsValue> {
    let ps: fc::PeerSet = serde_wasm_bindgen::from_value(peer_set).map_err(to_js_err)?;
    let dims: Vec<fc::ScoringDimension> =
        serde_wasm_bindgen::from_value(dimensions).map_err(to_js_err)?;
    let result = fc::score_relative_value(&ps, &dims).map_err(to_js_err)?;
    serde_wasm_bindgen::to_value(&result).map_err(to_js_err)
}
