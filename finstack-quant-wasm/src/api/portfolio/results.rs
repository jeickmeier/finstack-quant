//! Free-function twins of the Rust methods on portfolio result types.
//!
//! WASM results are plain objects, so a Rust method on a result type is a
//! function that takes the object (or its JSON) as the first argument. The
//! validating `Constraint` constructors return the constraint's wire object.

use crate::utils::input::{from_js_json, js_f64, js_json_value, js_opt_string, js_string, js_uint};
use crate::utils::{to_js_err, to_js_value, to_js_value_with_bigints};
use finstack_quant_portfolio::attribution::PortfolioAttribution;
use finstack_quant_portfolio::metrics::PortfolioMetrics;
use finstack_quant_portfolio::optimization::{
    Constraint, PortfolioOptimizationResultWire, PositionFilter,
};
use finstack_quant_portfolio::sensitivity::{SensitivityMatrix, SensitivityMatrixJson};
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

/// Look up one metric's portfolio-wide total, failing when it was not aggregated.
///
/// Twin of Python `PortfolioResult.require_metric` (Rust
/// `PortfolioMetrics::require_total`, which `PortfolioResult::require_metric`
/// calls on its metrics).
/// @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
/// @param metric_id - Fully qualified metric key (for example `dv01` or `bucketed_dv01::USD-OIS::10y`).
/// @returns The aggregated total in base-currency metric units.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON
/// string or plain object or `metricId` is not a string, a `FinstackError`
/// (kind `validation`) if `metrics` does not match the `PortfolioMetrics`
/// schema, and a `FinstackError` (kind `not_found`) naming the metric when it
/// was not aggregated.
#[wasm_bindgen(js_name = portfolioMetricsRequireTotal)]
pub fn portfolio_metrics_require_total(
    metrics: JsValue,
    metric_id: JsValue,
) -> Result<f64, JsValue> {
    let metrics: PortfolioMetrics = from_js_json(&metrics, "metrics")?;
    let metric_id = js_string(&metric_id, "metricId")?;
    metrics.require_total(&metric_id).map_err(to_js_err)
}

/// Executable trade list of an optimization result.
///
/// Twin of Python `PortfolioOptimizationResult.to_trade_list` (Rust
/// `PortfolioOptimizationResultWire::to_trade_list`): the trades from current
/// to target quantities, largest absolute quantity change first.
/// @param result - `PortfolioOptimizationResult` object or JSON from `optimizePortfolio`.
/// @returns The `TradeSpec` rows; empty for an infeasible solve (check `is_feasible`).
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `result` is not a JSON string
/// or plain object, and a `FinstackError` (kind `validation`) if it does not
/// match the optimization-result schema.
#[wasm_bindgen(js_name = portfolioOptimizationResultToTradeList)]
pub fn portfolio_optimization_result_to_trade_list(result: JsValue) -> Result<JsValue, JsValue> {
    let result: PortfolioOptimizationResultWire = from_js_json(&result, "result")?;
    to_js_value(&result.to_trade_list())
}

/// Rebuild the dense matrix from its wire object, validating its dimensions.
fn sensitivity_matrix(matrix: &JsValue) -> Result<SensitivityMatrix, JsValue> {
    let wire: SensitivityMatrixJson = from_js_json(matrix, "matrix")?;
    SensitivityMatrix::try_from(wire).map_err(to_js_err)
}

/// Read one position-by-factor sensitivity.
///
/// Twin of Python `SensitivityMatrix.delta` (Rust `SensitivityMatrix::try_delta`).
/// @param matrix - Sensitivity-matrix object or JSON `{ base_currency, position_ids, factor_ids, data }` from `computeFactorSensitivities`.
/// @param position_idx - Zero-based row index into `position_ids`.
/// @param factor_idx - Zero-based column index into `factor_ids`.
/// @returns The sensitivity in base-currency PV change per factor bump.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `matrix` is not a JSON string
/// or plain object or an index is not a non-negative integer, and a
/// `FinstackError` (kind `validation`) if `matrix` does not match the schema,
/// its rows do not match its axes, or an index is out of bounds.
#[wasm_bindgen(js_name = sensitivityMatrixDelta)]
pub fn sensitivity_matrix_delta(
    matrix: JsValue,
    position_idx: JsValue,
    factor_idx: JsValue,
) -> Result<f64, JsValue> {
    let position_idx: usize = js_uint(&position_idx, "positionIdx")?;
    let factor_idx: usize = js_uint(&factor_idx, "factorIdx")?;
    sensitivity_matrix(&matrix)?
        .try_delta(position_idx, factor_idx)
        .map_err(to_js_err)
}

/// Sensitivities of one position to every factor.
///
/// Twin of Python `SensitivityMatrix.position_deltas` (Rust
/// `SensitivityMatrix::try_position_deltas`).
/// @param matrix - Sensitivity-matrix object or JSON from `computeFactorSensitivities`.
/// @param position_idx - Zero-based row index into `position_ids`.
/// @returns One value per factor, in `factor_ids` order.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `matrix` is not a JSON string
/// or plain object or `positionIdx` is not a non-negative integer, and a
/// `FinstackError` (kind `validation`) if `matrix` does not match the schema,
/// its rows do not match its axes, or `positionIdx` is out of bounds.
#[wasm_bindgen(js_name = sensitivityMatrixPositionDeltas)]
pub fn sensitivity_matrix_position_deltas(
    matrix: JsValue,
    position_idx: JsValue,
) -> Result<Vec<f64>, JsValue> {
    let position_idx: usize = js_uint(&position_idx, "positionIdx")?;
    sensitivity_matrix(&matrix)?
        .try_position_deltas(position_idx)
        .map(<[f64]>::to_vec)
        .map_err(to_js_err)
}

/// Sensitivities of every position to one factor.
///
/// Twin of Python `SensitivityMatrix.factor_deltas` (Rust
/// `SensitivityMatrix::try_factor_deltas`).
/// @param matrix - Sensitivity-matrix object or JSON from `computeFactorSensitivities`.
/// @param factor_idx - Zero-based column index into `factor_ids`.
/// @returns One value per position, in `position_ids` order.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `matrix` is not a JSON string
/// or plain object or `factorIdx` is not a non-negative integer, and a
/// `FinstackError` (kind `validation`) if `matrix` does not match the schema,
/// its rows do not match its axes, or `factorIdx` is out of bounds.
#[wasm_bindgen(js_name = sensitivityMatrixFactorDeltas)]
pub fn sensitivity_matrix_factor_deltas(
    matrix: JsValue,
    factor_idx: JsValue,
) -> Result<Vec<f64>, JsValue> {
    let factor_idx: usize = js_uint(&factor_idx, "factorIdx")?;
    sensitivity_matrix(&matrix)?
        .try_factor_deltas(factor_idx)
        .map_err(to_js_err)
}

/// Apply the optional label and convert a validated constraint to its wire object.
fn constraint_value(
    constraint: finstack_quant_portfolio::Result<Constraint>,
    label: Option<&JsValue>,
) -> Result<JsValue, JsValue> {
    let constraint = constraint.map_err(to_js_err)?;
    to_js_value(&match js_opt_string(label, "label")? {
        Some(label) => constraint.with_label(label),
        None => constraint,
    })
}

/// Build a budget (weight-sum) constraint.
///
/// Twin of Python `Constraint.budget` (Rust `Constraint::budget`).
/// @param rhs - Target sum of weights as a decimal (`1.0` is fully invested); finite and non-negative.
/// @returns The `Constraint` object `{ budget: { rhs } }`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `rhs` is not a number, and a
/// `FinstackError` (kind `validation`) if it is negative or not finite.
#[wasm_bindgen(js_name = constraintBudget)]
pub fn constraint_budget(rhs: JsValue) -> Result<JsValue, JsValue> {
    constraint_value(Constraint::budget(js_f64(&rhs, "rhs")?), None)
}

/// Build per-position weight bounds for the positions a filter selects.
///
/// Twin of Python `Constraint.weight_bounds` (Rust `Constraint::weight_bounds`).
/// @param filter - `PositionFilter` value selecting the bounded positions: `"all"` or a filter object such as `{ by_entity_id: "FUND" }`.
/// @param min - Inclusive minimum weight as a decimal fraction of portfolio value.
/// @param max - Inclusive maximum weight as a decimal fraction of portfolio value.
/// @param label - Optional label reported in constraint slacks.
/// @returns The `weight_bounds` `Constraint` object.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a wrong argument type, and a
/// `FinstackError` (kind `validation`) if `filter` does not match the schema
/// or `min > max`.
#[wasm_bindgen(js_name = constraintWeightBounds)]
pub fn constraint_weight_bounds(
    filter: JsValue,
    min: JsValue,
    max: JsValue,
    label: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    // A filter is `"all"` or an object, so the argument is the value itself
    // (a string is the wire label, not JSON text to parse).
    let filter: PositionFilter = serde_json::from_value(js_json_value(&filter, "filter")?)
        .map_err(|error| {
            to_js_err(finstack_quant_core::Error::Validation(format!(
                "filter: {error}"
            )))
        })?;
    let min = js_f64(&min, "min")?;
    let max = js_f64(&max, "max")?;
    constraint_value(Constraint::weight_bounds(filter, min, max), label.as_ref())
}

/// Build a maximum-turnover constraint: `sum |w_new - w_current| <= maxTurnover`.
///
/// Twin of Python `Constraint.max_turnover` (Rust `Constraint::max_turnover`).
/// @param max_turnover - Largest allowed gross turnover as a decimal fraction of portfolio value; non-negative.
/// @param label - Optional label reported in constraint slacks.
/// @returns The `max_turnover` `Constraint` object.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a wrong argument type, and a
/// `FinstackError` (kind `validation`) if `maxTurnover` is negative.
#[wasm_bindgen(js_name = constraintMaxTurnover)]
pub fn constraint_max_turnover(
    max_turnover: JsValue,
    label: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let max_turnover = js_f64(&max_turnover, "maxTurnover")?;
    constraint_value(Constraint::max_turnover(max_turnover), label.as_ref())
}

/// Build an attribute exposure cap: `sum w_i * I[attr == value] <= maxShare`.
///
/// Twin of Python `Constraint.exposure_limit` (Rust `Constraint::exposure_limit`).
/// @param key - Position attribute key (for example `"rating"`).
/// @param value - Text value the attribute must equal to count toward the exposure.
/// @param max_share - Largest allowed share of portfolio weight as a decimal in `[0, 1]`.
/// @param label - Optional label reported in constraint slacks.
/// @returns The `metric_bound` `Constraint` object with operator `le`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a wrong argument type, and a
/// `FinstackError` (kind `validation`) if `maxShare` is outside `[0, 1]`.
#[wasm_bindgen(js_name = constraintExposureLimit)]
pub fn constraint_exposure_limit(
    key: JsValue,
    value: JsValue,
    max_share: JsValue,
    label: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let key = js_string(&key, "key")?;
    let value = js_string(&value, "value")?;
    let max_share = js_f64(&max_share, "maxShare")?;
    constraint_value(
        Constraint::exposure_limit(key, value, max_share),
        label.as_ref(),
    )
}

/// Build an attribute exposure floor: `sum w_i * I[attr == value] >= minShare`.
///
/// Twin of Python `Constraint.exposure_minimum` (Rust `Constraint::exposure_minimum`).
/// @param key - Position attribute key (for example `"sector"`).
/// @param value - Text value the attribute must equal to count toward the exposure.
/// @param min_share - Smallest required share of portfolio weight as a decimal in `[0, 1]`.
/// @param label - Optional label reported in constraint slacks.
/// @returns The `metric_bound` `Constraint` object with operator `ge`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a wrong argument type, and a
/// `FinstackError` (kind `validation`) if `minShare` is outside `[0, 1]`.
#[wasm_bindgen(js_name = constraintExposureMinimum)]
pub fn constraint_exposure_minimum(
    key: JsValue,
    value: JsValue,
    min_share: JsValue,
    label: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let key = js_string(&key, "key")?;
    let value = js_string(&value, "value")?;
    let min_share = js_f64(&min_share, "minShare")?;
    constraint_value(
        Constraint::exposure_minimum(key, value, min_share),
        label.as_ref(),
    )
}
