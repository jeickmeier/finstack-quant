//! WASM bindings for the `finstack-quant-cashflows` crate.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Build a cashflow schedule from a JSON spec and return canonical schedule JSON.
///
/// @param spec_json - JSON-encoded `CashflowScheduleBuildSpec`. Optional
///   `principal_exchange` is `"none"` or `"initial_and_final"` (default).
///   `principal_events` entries require both economic `date` and cash `payment_date`.
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
#[wasm_bindgen(js_name = accruedInterest)]
pub fn accrued_interest(
    schedule_json: &str,
    as_of: &str,
    config_json: Option<String>,
) -> Result<f64, JsValue> {
    finstack_quant_cashflows::accrued_interest(schedule_json, as_of, config_json.as_deref())
        .map_err(to_js_err)
}

/// Convert an annual CPR (constant prepayment rate) to a monthly SMM.
///
/// Uses the standard relationship `SMM = 1 - (1 - CPR)^(1/12)` (Fabozzi's
/// MBS handbook).
///
/// @param cpr - Annualized CPR as a decimal in `[0, 1]` (0.06 means 6%).
/// @returns Monthly SMM as a decimal.
/// @throws If `cpr` is negative, non-finite, or above 1.0.
#[wasm_bindgen(js_name = cprToSmm)]
pub fn cpr_to_smm(cpr: f64) -> Result<f64, JsValue> {
    finstack_quant_cashflows::builder::cpr_to_smm(cpr).map_err(to_js_err)
}

/// Convert a monthly SMM (single monthly mortality) to an annual CPR.
///
/// Uses `CPR = 1 - (1 - SMM)^12`.
///
/// @param smm - Monthly SMM as a decimal in `[0, 1]`.
/// @returns Annualized CPR as a decimal.
/// @throws If `smm` is negative, non-finite, or above 1.0.
#[wasm_bindgen(js_name = smmToCpr)]
pub fn smm_to_cpr(smm: f64) -> Result<f64, JsValue> {
    finstack_quant_cashflows::builder::smm_to_cpr(smm).map_err(to_js_err)
}

/// Convert an annual CDR (constant default rate) to a monthly MDR.
///
/// Default and prepayment mortality rates share the same annual-to-monthly
/// conversion kernel: `MDR = 1 - (1 - CDR)^(1/12)`.
///
/// @param cdr - Constant annual default rate as a decimal in `[0, 1]`.
/// @returns Monthly MDR as a decimal.
/// @throws If `cdr` is negative, non-finite, or above 1.0.
#[wasm_bindgen(js_name = cdrToMdr)]
pub fn cdr_to_mdr(cdr: f64) -> Result<f64, JsValue> {
    finstack_quant_cashflows::builder::cdr_to_mdr(cdr).map_err(to_js_err)
}

/// Convert a monthly MDR (monthly default rate) to an annual CDR.
///
/// Uses `CDR = 1 - (1 - MDR)^12`.
///
/// @param mdr - Monthly default rate as a decimal in `[0, 1]`.
/// @returns Annualized CDR as a decimal.
/// @throws If `mdr` is negative, non-finite, or above 1.0.
#[wasm_bindgen(js_name = mdrToCdr)]
pub fn mdr_to_cdr(mdr: f64) -> Result<f64, JsValue> {
    finstack_quant_cashflows::builder::mdr_to_cdr(mdr).map_err(to_js_err)
}
