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
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::FinancialModelSpec;
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
    let severity: Severity =
        finstack_quant_core::wire::serde_parse(&js_string(&severity, "severity")?)
            .map_err(to_js_err)?;
    to_js_value(&report.findings_by_severity(severity))
}

/// Total interest expense (cash plus PIK) of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_interest` (Rust
/// `CapitalStructureCashflows::get_interest`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Interest expense in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetInterest)]
pub fn capital_structure_cashflows_get_interest(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_interest(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Cash interest expense of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_interest_cash` (Rust
/// `CapitalStructureCashflows::get_interest_cash`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Cash interest in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetInterestCash)]
pub fn capital_structure_cashflows_get_interest_cash(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_interest_cash(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Payment-in-kind interest of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_interest_pik` (Rust
/// `CapitalStructureCashflows::get_interest_pik`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns PIK interest in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetInterestPik)]
pub fn capital_structure_cashflows_get_interest_pik(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_interest_pik(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Principal payment of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_principal` (Rust
/// `CapitalStructureCashflows::get_principal`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Principal paid in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetPrincipal)]
pub fn capital_structure_cashflows_get_principal(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_principal(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Closing debt balance of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_debt_balance` (Rust
/// `CapitalStructureCashflows::get_debt_balance`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Outstanding balance in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetDebtBalance)]
pub fn capital_structure_cashflows_get_debt_balance(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_debt_balance(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Fees of one instrument in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_fees` (Rust
/// `CapitalStructureCashflows::get_fees`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Fees in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetFees)]
pub fn capital_structure_cashflows_get_fees(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_fees(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Accrued interest liability of one instrument at the end of one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_accrued_interest` (Rust
/// `CapitalStructureCashflows::get_accrued_interest`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param instrument_id - Capital-structure instrument identifier.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Accrued interest in the instrument's currency.
///
/// # Errors
///
/// Throws with kind `computation` if the instrument or period has no
/// cashflows, and kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetAccruedInterest)]
pub fn capital_structure_cashflows_get_accrued_interest(
    cashflows_json: JsValue,
    instrument_id: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_accrued_interest(
            &js_string(&instrument_id, "instrumentId")?,
            &period_id(&period)?,
        )
        .map_err(to_js_err)
}

/// Total interest expense (cash plus PIK) across all instruments in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_total_interest` (Rust
/// `CapitalStructureCashflows::get_total_interest`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Total interest expense in the reporting currency.
///
/// # Errors
///
/// Throws with kind `computation` if the period has no totals or the
/// instruments span several currencies without a reporting currency, and kind
/// `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetTotalInterest)]
pub fn capital_structure_cashflows_get_total_interest(
    cashflows_json: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_total_interest(&period_id(&period)?)
        .map_err(to_js_err)
}

/// Total principal payments across all instruments in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_total_principal` (Rust
/// `CapitalStructureCashflows::get_total_principal`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Total principal paid in the reporting currency.
///
/// # Errors
///
/// Throws with kind `computation` if the period has no totals or the
/// instruments span several currencies without a reporting currency, and kind
/// `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetTotalPrincipal)]
pub fn capital_structure_cashflows_get_total_principal(
    cashflows_json: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_total_principal(&period_id(&period)?)
        .map_err(to_js_err)
}

/// Total closing debt balance across all instruments in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_total_debt_balance` (Rust
/// `CapitalStructureCashflows::get_total_debt_balance`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Total outstanding balance in the reporting currency.
///
/// # Errors
///
/// Throws with kind `computation` if the period has no totals or the
/// instruments span several currencies without a reporting currency, and kind
/// `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetTotalDebtBalance)]
pub fn capital_structure_cashflows_get_total_debt_balance(
    cashflows_json: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_total_debt_balance(&period_id(&period)?)
        .map_err(to_js_err)
}

/// Total fees across all instruments in one period.
///
/// Free-function twin of Python `CapitalStructureCashflows.get_total_fees` (Rust
/// `CapitalStructureCashflows::get_total_fees`).
/// @param cashflows_json - The `cs_cashflows` object of a statement result (object or JSON).
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns Total fees in the reporting currency.
///
/// # Errors
///
/// Throws with kind `computation` if the period has no totals or the
/// instruments span several currencies without a reporting currency, and kind
/// `validation` if an input is malformed.
#[wasm_bindgen(js_name = capitalStructureCashflowsGetTotalFees)]
pub fn capital_structure_cashflows_get_total_fees(
    cashflows_json: JsValue,
    period: JsValue,
) -> Result<f64, JsValue> {
    let cashflows: CapitalStructureCashflows = from_js_json(&cashflows_json, "cashflowsJson")?;
    cashflows
        .get_total_fees(&period_id(&period)?)
        .map_err(to_js_err)
}
