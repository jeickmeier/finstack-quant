//! Free-function twins of the Rust methods on portfolio result types.
//!
//! WASM results are plain objects, so a Rust method on a result type is a
//! function that takes the object (or its JSON) as the first argument.

use crate::utils::input::{from_js_json, js_f64, js_string};
use crate::utils::{to_js_value, to_js_value_with_bigints};
use finstack_quant_portfolio::attribution::PortfolioAttribution;
use finstack_quant_portfolio::metrics::PortfolioMetrics;
use finstack_quant_portfolio::optimization::PortfolioOptimizationResultWire;
use finstack_quant_portfolio::valuation::PortfolioValuation;
use wasm_bindgen::prelude::*;

/// Render a portfolio P&L attribution as an indented text tree.
///
/// Twin of Python `PortfolioAttribution.explain` (Rust
/// `PortfolioAttribution::explain`): the total followed by one line per factor
/// bucket with its share of the total.
/// @param attribution - `PortfolioAttribution` object or JSON from `attributePortfolioPnl`.
/// @returns The multi-line explanation.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `attribution` is not a JSON
/// string or plain object, and a `FinstackError` (kind `validation`) if it
/// does not match the `PortfolioAttribution` schema.
#[wasm_bindgen(js_name = portfolioAttributionExplainText)]
pub fn portfolio_attribution_explain_text(attribution: JsValue) -> Result<String, JsValue> {
    let attribution: PortfolioAttribution = from_js_json(&attribution, "attribution")?;
    Ok(attribution.explain())
}

/// Check that a portfolio attribution's factor buckets sum to its total P&L.
///
/// Twin of Python `PortfolioAttribution.reconciliation_check` (Rust
/// `PortfolioAttribution::reconciliation_check`).
/// @param attribution - `PortfolioAttribution` object or JSON from `attributePortfolioPnl`.
/// @param tolerance - Largest absolute residual, in base-currency units, still reported as reconciled.
/// @returns The `ReconciliationReport`: `total_residual`, `is_reconciled` and the `tolerance` applied.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `attribution` is not a JSON
/// string or plain object or `tolerance` is not a number, and a
/// `FinstackError` (kind `validation`) if `attribution` does not match the
/// `PortfolioAttribution` schema.
#[wasm_bindgen(js_name = portfolioAttributionReconciliationCheck)]
pub fn portfolio_attribution_reconciliation_check(
    attribution: JsValue,
    tolerance: JsValue,
) -> Result<JsValue, JsValue> {
    let attribution: PortfolioAttribution = from_js_json(&attribution, "attribution")?;
    let tolerance = js_f64(&tolerance, "tolerance")?;
    to_js_value(&attribution.reconciliation_check(tolerance))
}

/// Look up one position's value in a portfolio valuation.
///
/// Twin of Python `PortfolioValuation.get_position_value` (Rust
/// `PortfolioValuation::get_position_value`). The returned
/// `valuation_result` keeps 64-bit fields as `BigInt`, as `valuePortfolio`
/// does.
/// @param valuation - `PortfolioValuation` object or JSON from `valuePortfolio`.
/// @param position_id - Position identifier to look up.
/// @returns The `PositionValue`, or `undefined` when the valuation has no such position.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `valuation` is not a JSON
/// string or plain object or `positionId` is not a string, and a
/// `FinstackError` (kind `validation`) if `valuation` does not match the
/// `PortfolioValuation` schema.
#[wasm_bindgen(js_name = portfolioValuationGetPositionValue)]
pub fn portfolio_valuation_get_position_value(
    valuation: JsValue,
    position_id: JsValue,
) -> Result<JsValue, JsValue> {
    let valuation: PortfolioValuation = from_js_json(&valuation, "valuation")?;
    let position_id = js_string(&position_id, "positionId")?;
    match valuation.get_position_value(&position_id) {
        Some(value) => to_js_value_with_bigints(value),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Look up one entity's aggregated base-currency value in a portfolio valuation.
///
/// Twin of Python `PortfolioValuation.get_entity_value` (Rust
/// `PortfolioValuation::get_entity_value`).
/// @param valuation - `PortfolioValuation` object or JSON from `valuePortfolio`.
/// @param entity_id - Entity identifier to look up.
/// @returns The entity total as `Money`, or `undefined` when the valuation has no such entity.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `valuation` is not a JSON
/// string or plain object or `entityId` is not a string, and a
/// `FinstackError` (kind `validation`) if `valuation` does not match the
/// `PortfolioValuation` schema.
#[wasm_bindgen(js_name = portfolioValuationGetEntityValue)]
pub fn portfolio_valuation_get_entity_value(
    valuation: JsValue,
    entity_id: JsValue,
) -> Result<JsValue, JsValue> {
    let valuation: PortfolioValuation = from_js_json(&valuation, "valuation")?;
    let entity_id = js_string(&entity_id, "entityId")?;
    match valuation.get_entity_value(&entity_id) {
        Some(value) => to_js_value(value),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Look up one aggregated metric in portfolio metrics.
///
/// Twin of Python `PortfolioMetrics.get_metric` (Rust
/// `PortfolioMetrics::get_metric`).
/// @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
/// @param metric_id - Fully qualified metric key.
/// @returns The `AggregatedMetric` (`total` plus `by_entity`), or `undefined` when the metric was not aggregated.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON
/// string or plain object or `metricId` is not a string, and a
/// `FinstackError` (kind `validation`) if `metrics` does not match the
/// `PortfolioMetrics` schema.
#[wasm_bindgen(js_name = portfolioMetricsGetMetric)]
pub fn portfolio_metrics_get_metric(
    metrics: JsValue,
    metric_id: JsValue,
) -> Result<JsValue, JsValue> {
    let metrics: PortfolioMetrics = from_js_json(&metrics, "metrics")?;
    let metric_id = js_string(&metric_id, "metricId")?;
    match metrics.get_metric(&metric_id) {
        Some(metric) => to_js_value(metric),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Look up one position's raw metric values in portfolio metrics.
///
/// Twin of Python `PortfolioMetrics.get_position_metrics` (Rust
/// `PortfolioMetrics::get_position_metrics`).
/// @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
/// @param position_id - Position identifier to look up.
/// @returns The position's `{ metricId: value }` map, or `undefined` when the position has no metrics.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON
/// string or plain object or `positionId` is not a string, and a
/// `FinstackError` (kind `validation`) if `metrics` does not match the
/// `PortfolioMetrics` schema.
#[wasm_bindgen(js_name = portfolioMetricsGetPositionMetrics)]
pub fn portfolio_metrics_get_position_metrics(
    metrics: JsValue,
    position_id: JsValue,
) -> Result<JsValue, JsValue> {
    let metrics: PortfolioMetrics = from_js_json(&metrics, "metrics")?;
    let position_id = js_string(&position_id, "positionId")?;
    match metrics.get_position_metrics(&position_id) {
        Some(position) => to_js_value(position),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Look up one metric's portfolio-wide total in portfolio metrics.
///
/// Twin of Python `PortfolioMetrics.get_total` (Rust
/// `PortfolioMetrics::get_total`).
/// @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
/// @param metric_id - Fully qualified metric key.
/// @returns The total, or `undefined` when the metric was not aggregated.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON
/// string or plain object or `metricId` is not a string, and a
/// `FinstackError` (kind `validation`) if `metrics` does not match the
/// `PortfolioMetrics` schema.
#[wasm_bindgen(js_name = portfolioMetricsGetTotal)]
pub fn portfolio_metrics_get_total(
    metrics: JsValue,
    metric_id: JsValue,
) -> Result<Option<f64>, JsValue> {
    let metrics: PortfolioMetrics = from_js_json(&metrics, "metrics")?;
    let metric_id = js_string(&metric_id, "metricId")?;
    Ok(metrics.get_total(&metric_id))
}

/// Trades of an optimization result that open a new position.
///
/// Twin of Python `PortfolioOptimizationResult.new_position_trades` (Rust
/// `PortfolioOptimizationResultWire::new_position_trades`): the entries of
/// `trades` whose `trade_type` is `new_position`, in trade-list order.
/// @param result - `PortfolioOptimizationResult` object or JSON from `optimizePortfolio`.
/// @returns The matching `TradeSpec` rows; empty for an infeasible solve.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `result` is not a JSON string
/// or plain object, and a `FinstackError` (kind `validation`) if it does not
/// match the optimization-result schema.
#[wasm_bindgen(js_name = portfolioOptimizationResultNewPositionTrades)]
pub fn portfolio_optimization_result_new_position_trades(
    result: JsValue,
) -> Result<JsValue, JsValue> {
    let result: PortfolioOptimizationResultWire = from_js_json(&result, "result")?;
    to_js_value(&result.new_position_trades())
}

/// Approximately binding constraints of an optimization result.
///
/// Twin of Python `PortfolioOptimizationResult.binding_constraints` (Rust
/// `PortfolioOptimizationResultWire::binding_constraints`): the constraints
/// whose slack is zero within the solver tolerance.
/// @param result - `PortfolioOptimizationResult` object or JSON from `optimizePortfolio`.
/// @returns `[label, slack]` pairs in slack order; empty for an infeasible solve.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `result` is not a JSON string
/// or plain object, and a `FinstackError` (kind `validation`) if it does not
/// match the optimization-result schema.
#[wasm_bindgen(js_name = portfolioOptimizationResultBindingConstraints)]
pub fn portfolio_optimization_result_binding_constraints(
    result: JsValue,
) -> Result<JsValue, JsValue> {
    let result: PortfolioOptimizationResultWire = from_js_json(&result, "result")?;
    to_js_value(&result.binding_constraints())
}
