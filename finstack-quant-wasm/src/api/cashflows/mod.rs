//! WASM bindings for the `finstack-quant-cashflows` crate.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Build a cashflow schedule from a JSON spec and return canonical schedule JSON.
///
/// @param spec_json - JSON-encoded `CashflowScheduleBuildSpec`.
/// @param market_json - Optional JSON-encoded market context for floating-rate lookups.
/// @returns JSON-encoded `CashFlowSchedule`.
/// @throws If the spec or market JSON is malformed, or schedule construction fails.
#[wasm_bindgen(js_name = buildCashflowScheduleJson)]
pub fn build_cashflow_schedule_json(
    spec_json: &str,
    market_json: Option<String>,
) -> Result<String, JsValue> {
    finstack_quant_cashflows::build_cashflow_schedule_json(spec_json, market_json.as_deref())
        .map_err(to_js_err)
}

/// Validate a cashflow schedule JSON string and return it canonicalized.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @returns Canonicalized JSON-encoded `CashFlowSchedule`.
/// @throws If the schedule JSON is malformed or fails validation.
#[wasm_bindgen(js_name = validateCashflowScheduleJson)]
pub fn validate_cashflow_schedule_json(schedule_json: &str) -> Result<String, JsValue> {
    finstack_quant_cashflows::validate_cashflow_schedule_json(schedule_json).map_err(to_js_err)
}

/// Extract dated flows from a cashflow schedule JSON string.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @returns JSON array of settlement cash entries. PIK and
///   `DefaultedNotional` state rows are omitted; parse the full schedule JSON
///   when flow classification is required.
/// @throws If the schedule JSON is malformed.
#[wasm_bindgen(js_name = datedFlowsJson)]
pub fn dated_flows_json(schedule_json: &str) -> Result<String, JsValue> {
    finstack_quant_cashflows::dated_flows_json(schedule_json).map_err(to_js_err)
}

/// Compute accrued interest from a cashflow schedule JSON string as of a given date.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @param as_of - ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
/// @param config_json - Optional JSON-encoded `AccrualConfig` overriding defaults.
/// @returns Accrued interest in the schedule's settlement currency as a JS
///   number. The Rust engine computes from the canonical schedule and then
///   crosses the WASM boundary as `f64`; for large notionals, compare with an
///   absolute tolerance scaled to the schedule notional rather than expecting
///   decimal-string equality.
/// @throws If any JSON input is malformed or the accrual computation fails.
#[wasm_bindgen(js_name = accruedInterestJson)]
pub fn accrued_interest_json(
    schedule_json: &str,
    as_of: &str,
    config_json: Option<String>,
) -> Result<f64, JsValue> {
    finstack_quant_cashflows::accrued_interest_json(schedule_json, as_of, config_json.as_deref())
        .map_err(to_js_err)
}
