//! Free-function twins of the Python result-object accessors.
//!
//! WASM statement results are plain objects, so each Rust accessor on
//! `StatementResult`, `CapitalStructureCashflows`, `CheckReport` and
//! `FinancialModelSpec` is a function taking the object (or its JSON).

use crate::utils::input::{from_js_json, js_f64, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::capital_structure::CapitalStructureCashflows;
use finstack_quant_statements::checks::{CheckReport, Severity};
use finstack_quant_statements::evaluator::{RollupSpec, StatementResult};
use finstack_quant_statements::types::FinancialModelSpec;
use wasm_bindgen::prelude::*;

fn period_id(period: &JsValue) -> Result<PeriodId, JsValue> {
    js_string(period, "period")?.parse().map_err(to_js_err)
}

/// Value of one node in one period.
///
/// Free-function twin of Python `StatementResult.get` (Rust
/// `StatementResult::get`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns The node value in its own units, or `undefined` when the node or period is absent.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or `period`
/// is not a valid period identifier.
#[wasm_bindgen(js_name = statementResultGet)]
pub fn statement_result_get(
    result_json: JsValue,
    node_id: JsValue,
    period: JsValue,
) -> Result<Option<f64>, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    Ok(result.get(&js_string(&node_id, "nodeId")?, &period_id(&period)?))
}

/// Roll a statement result up to a coarser frequency, such as quarterly to annual.
///
/// Free-function twin of Python `StatementResult.rollup` (Rust
/// `StatementResult::rollup`). Each target period combines its source
/// periods node by node: summed by default (flows), the last source value
/// for `period_end` nodes (balances, LTM metrics), and the node's formula
/// re-evaluated on the rolled-up values for `formula` nodes (ratios). Every
/// target period must be complete.
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param model - The `FinancialModelSpec` the result was evaluated from (object or JSON); it supplies the period dates and the formulas of `formula` nodes.
/// @param spec - `RollupSpec`: target `frequency` (`"quarterly"`, `"semi_annual"` or `"annual"`) and per-node `methods` (`"sum"` default, `"period_end"`, `"formula"`) (object or JSON).
/// @returns `StatementResult` plain object keyed by the target periods (`"2025"`, `"2025H1"`, `"2025Q1"`), without `node_sources`, capital-structure cashflows or a check report.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the target
/// frequency is not a whole multiple of the model's, a target period is
/// incomplete, a method names an unknown node, a `formula` node has no
/// formula or reads `cs.*` values, or a re-evaluated formula fails.
#[wasm_bindgen(js_name = statementResultRollup)]
pub fn statement_result_rollup(
    result_json: JsValue,
    model: JsValue,
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    let model = FinancialModelSpec::from_json(&json_text(&model, "model")?).map_err(to_js_err)?;
    let spec: RollupSpec = from_js_json(&spec, "spec")?;
    to_js_value(&result.rollup(&model, &spec).map_err(to_js_err)?)
}

/// Monetary value of one node in one period.
///
/// Free-function twin of Python `StatementResult.get_money` (Rust
/// `StatementResult::get_money`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns `Money` wire object (`{amount, currency}`), or `undefined` when the node is not monetary or has no value in the period.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or `period`
/// is not a valid period identifier.
#[wasm_bindgen(js_name = statementResultGetMoney)]
pub fn statement_result_get_money(
    result_json: JsValue,
    node_id: JsValue,
    period: JsValue,
) -> Result<JsValue, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    match result.get_money(&js_string(&node_id, "nodeId")?, &period_id(&period)?) {
        Some(money) => to_js_value(&money),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Scalar (non-monetary) value of one node in one period.
///
/// Free-function twin of Python `StatementResult.get_scalar` (Rust
/// `StatementResult::get_scalar`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns The scalar value, or `undefined` when the node is monetary or has no value in the period.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or `period`
/// is not a valid period identifier.
#[wasm_bindgen(js_name = statementResultGetScalar)]
pub fn statement_result_get_scalar(
    result_json: JsValue,
    node_id: JsValue,
    period: JsValue,
) -> Result<Option<f64>, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    Ok(result.get_scalar(&js_string(&node_id, "nodeId")?, &period_id(&period)?))
}

/// Value of one node in one period, or a fallback when it is absent.
///
/// Free-function twin of Python `StatementResult.get_or` (Rust
/// `StatementResult::get_or`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @param default_value - Value returned when the node or period is absent, in the node's own units.
/// @returns The node value, or `defaultValue`.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or `period`
/// is not a valid period identifier, and kind `invalid_type` if
/// `defaultValue` is not a number.
#[wasm_bindgen(js_name = statementResultGetOr)]
pub fn statement_result_get_or(
    result_json: JsValue,
    node_id: JsValue,
    period: JsValue,
    default_value: JsValue,
) -> Result<f64, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    Ok(result.get_or(
        &js_string(&node_id, "nodeId")?,
        &period_id(&period)?,
        js_f64(&default_value, "defaultValue")?,
    ))
}

/// Every `(period, value)` pair of one node.
///
/// Free-function twin of Python `StatementResult.all_periods` (Rust
/// `StatementResult::all_periods`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @returns `[periodId, value]` pairs in the result's period order; empty when the node is absent.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed.
#[wasm_bindgen(js_name = statementResultAllPeriods)]
pub fn statement_result_all_periods(
    result_json: JsValue,
    node_id: JsValue,
) -> Result<JsValue, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    let rows: Vec<(String, f64)> = result
        .all_periods(&js_string(&node_id, "nodeId")?)
        .map(|(period, value)| (period.to_string(), value))
        .collect();
    to_js_value(&rows)
}

/// Period-to-value map of one node.
///
/// Free-function twin of Python `StatementResult.get_node` (Rust
/// `StatementResult::get_node`).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to read.
/// @returns Object mapping period id to value in the node's own units, or `undefined` when the node is absent.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed.
#[wasm_bindgen(js_name = statementResultGetNode)]
pub fn statement_result_get_node(
    result_json: JsValue,
    node_id: JsValue,
) -> Result<JsValue, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    match result.get_node(&js_string(&node_id, "nodeId")?) {
        Some(values) => to_js_value(values),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Node identifiers held by a statement result.
///
/// Free-function twin of Python `StatementResult.node_ids`.
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @returns Node identifiers in evaluation (declaration) order.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed.
#[wasm_bindgen(js_name = statementResultNodeIds)]
pub fn statement_result_node_ids(result_json: JsValue) -> Result<JsValue, JsValue> {
    let result: StatementResult = from_js_json(&result_json, "resultJson")?;
    let ids: Vec<&str> = result.nodes.keys().map(String::as_str).collect();
    to_js_value(&ids)
}

/// Whether a model declares a node.
///
/// Free-function twin of Python `FinancialModelSpec.has_node` (Rust
/// `FinancialModelSpec::has_node`).
/// @param model_json - `FinancialModelSpec` (object or JSON).
/// @param node_id - Node identifier to look up.
/// @returns `true` when the model declares `nodeId`.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed or fails semantic
/// validation.
#[wasm_bindgen(js_name = financialModelHasNode)]
pub fn financial_model_has_node(model_json: JsValue, node_id: JsValue) -> Result<bool, JsValue> {
    let model =
        FinancialModelSpec::from_json(&json_text(&model_json, "modelJson")?).map_err(to_js_err)?;
    Ok(model.has_node(&js_string(&node_id, "nodeId")?))
}

/// One node specification of a model.
///
/// Free-function twin of Python `FinancialModelSpec.get_node` (Rust
/// `FinancialModelSpec::get_node`).
/// @param model_json - `FinancialModelSpec` (object or JSON).
/// @param node_id - Node identifier to look up.
/// @returns The `NodeSpec` plain object, or `undefined` when the model has no such node.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed or fails semantic
/// validation.
#[wasm_bindgen(js_name = financialModelGetNode)]
pub fn financial_model_get_node(model_json: JsValue, node_id: JsValue) -> Result<JsValue, JsValue> {
    let model =
        FinancialModelSpec::from_json(&json_text(&model_json, "modelJson")?).map_err(to_js_err)?;
    match model.get_node(&js_string(&node_id, "nodeId")?) {
        Some(node) => to_js_value(node),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Whether a check report holds at least one error-severity finding.
///
/// Free-function twin of Python `CheckReport.has_errors` (Rust
/// `CheckReport::has_errors`).
/// @param report_json - `CheckReport` returned by `runChecks` or carried on a statement result (object or JSON).
/// @returns `true` when the summary counts at least one error.
///
/// # Errors
///
/// Throws with kind `validation` if the report input is malformed.
#[wasm_bindgen(js_name = checkReportHasErrors)]
pub fn check_report_has_errors(report_json: JsValue) -> Result<bool, JsValue> {
    let report: CheckReport = from_js_json(&report_json, "reportJson")?;
    Ok(report.has_errors())
}

/// Whether a check report holds at least one warning-severity finding.
///
/// Free-function twin of Python `CheckReport.has_warnings` (Rust
/// `CheckReport::has_warnings`).
/// @param report_json - `CheckReport` returned by `runChecks` or carried on a statement result (object or JSON).
/// @returns `true` when the summary counts at least one warning.
///
/// # Errors
///
/// Throws with kind `validation` if the report input is malformed.
#[wasm_bindgen(js_name = checkReportHasWarnings)]
pub fn check_report_has_warnings(report_json: JsValue) -> Result<bool, JsValue> {
    let report: CheckReport = from_js_json(&report_json, "reportJson")?;
    Ok(report.has_warnings())
}

/// Retained findings of one severity.
///
/// Free-function twin of Python `CheckReport.findings_by_severity` (Rust
/// `CheckReport::findings_by_severity`).
/// @param report_json - `CheckReport` returned by `runChecks` or carried on a statement result (object or JSON).
/// @param severity - `"info"`, `"warning"` or `"error"`.
/// @returns `CheckFinding` objects of that severity, in check order.
///
/// # Errors
///
/// Throws with kind `validation` if the report input is malformed or
/// `severity` is not a severity name.
#[wasm_bindgen(js_name = checkReportFindingsBySeverity)]
pub fn check_report_findings_by_severity(
    report_json: JsValue,
    severity: JsValue,
) -> Result<JsValue, JsValue> {
    let report: CheckReport = from_js_json(&report_json, "reportJson")?;
    let severity: Severity = crate::utils::wire::js_wire(&severity, "severity")?;
    to_js_value(&report.findings_by_severity(severity))
}

/// Read one capital-structure component for an instrument or reporting total.
///
/// # Arguments
///
/// * `cashflows_json` - Capital-structure cashflows object or its serialized JSON.
/// * `component` - One of `interest_expense` (cash plus PIK), `interest_expense_cash`,
///   `interest_expense_pik`, `interest_income`, `principal_payment`, `debt_balance`,
///   `fees`, or `accrued_interest`. Flows are positive payment/receipt magnitudes;
///   debt balance is closing principal. Rate hedges have zero debt balance and
///   debt accrued interest.
/// * `instrument_or_total` - Instrument identifier for native-currency amounts,
///   or `total` for reporting-currency aggregates. This lookup performs no FX.
/// * `period_id` - Reporting-period label such as `2025Q1` present in the result.
///
/// @returns Requested amount in major currency units; absent optional interest income is zero.
///
/// # Errors
///
/// Throws with kind `validation` for malformed inputs or period labels, and
/// kind `computation` for unknown components, missing instrument/period data,
/// unavailable multi-currency totals, or incompatible cash/PIK currencies.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetComponent)]
pub fn capital_structure_cashflows_get_component(
    cashflows_json: JsValue,
    component: JsValue,
    instrument_or_total: JsValue,
    period_id: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_component(
            &js_string(&component, "component")?,
            &js_string(&instrument_or_total, "instrumentOrTotal")?,
            &self::period_id(&period_id)?,
        )
        .map_err(to_js_err)
}
