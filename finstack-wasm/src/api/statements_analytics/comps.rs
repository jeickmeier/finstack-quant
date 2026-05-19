//! Comparable-company analysis bindings.
//!
//! Exposes peer statistics, percentile rank, z-score, OLS fair-value regression,
//! canonical valuation multiples, and composite rich/cheap scoring.

use crate::utils::to_js_err;
use finstack_statements_analytics::analysis as fc;
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
pub fn z_score(value: f64, data: JsValue) -> Result<JsValue, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    match fc::z_score(&d, value) {
        Some(z) => serde_wasm_bindgen::to_value(&z).map_err(to_js_err),
        None => Ok(JsValue::NULL),
    }
}

/// Descriptive statistics over a peer distribution.
#[wasm_bindgen(js_name = peerStats)]
pub fn peer_stats(data: JsValue) -> Result<JsValue, JsValue> {
    let d: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    let stats = fc::peer_stats(&d);
    serde_wasm_bindgen::to_value(&stats).map_err(to_js_err)
}

/// Single-factor OLS fit of `y` on `x` evaluated at the subject observation.
#[wasm_bindgen(js_name = regressionFairValue)]
pub fn regression_fair_value(
    x_values: JsValue,
    y_values: JsValue,
    subject_x: f64,
    subject_y: f64,
) -> Result<JsValue, JsValue> {
    let x: Vec<f64> = serde_wasm_bindgen::from_value(x_values).map_err(to_js_err)?;
    let y: Vec<f64> = serde_wasm_bindgen::from_value(y_values).map_err(to_js_err)?;
    let result = fc::regression_fair_value(&x, &y, subject_x, subject_y);
    serde_wasm_bindgen::to_value(&result).map_err(to_js_err)
}

/// Compute a canonical valuation multiple for a company-metric bag.
#[wasm_bindgen(js_name = computeMultiple)]
pub fn compute_multiple(company_metrics: JsValue, multiple: &str) -> Result<JsValue, JsValue> {
    let metrics_map: BTreeMap<String, f64> =
        serde_wasm_bindgen::from_value(company_metrics).map_err(to_js_err)?;
    let metrics = map_to_company_metrics(metrics_map);
    let multiple = multiple.parse::<fc::Multiple>().map_err(to_js_err)?;
    let result = fc::compute_multiple(&metrics, multiple);
    serde_wasm_bindgen::to_value(&result).map_err(to_js_err)
}

/// Composite rich/cheap scoring across multiple dimensions.
#[wasm_bindgen(js_name = scoreRelativeValue)]
pub fn score_relative_value(peer_set: JsValue, dimensions: JsValue) -> Result<JsValue, JsValue> {
    let ps: fc::PeerSet = serde_wasm_bindgen::from_value(peer_set).map_err(to_js_err)?;
    let dims: Vec<fc::ScoringDimension> =
        serde_wasm_bindgen::from_value(dimensions).map_err(to_js_err)?;
    let result = fc::score_relative_value(&ps, &dims).map_err(to_js_err)?;
    serde_wasm_bindgen::to_value(&result).map_err(to_js_err)
}
