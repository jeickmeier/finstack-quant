//! WASM bindings for the `finstack-quant-cashflows` crate.
//!
//! This module holds the JSON-first bridge (`*Json` functions and the
//! schedule-JSON analytics). The typed surface lives in the submodules: the
//! `CashFlowSchedule` / `CashFlowBuilder` / `AccrualIndex` handles, and free
//! functions over wire-shaped spec, primitive and aggregation values.

pub mod accrual;
pub mod aggregation;
pub mod fixings;
pub mod primitives;
pub mod schedule;
pub mod specs;

use crate::utils::input::{js_f64, js_f64_seq, js_string, js_uint, json_text, opt_json_text};
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
    spec_json: JsValue,
    market_json: Option<JsValue>,
) -> Result<String, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    let market_json = opt_json_text(market_json.as_ref(), "marketJson")?;
    finstack_quant_cashflows::build_cashflow_schedule_json(spec_json, market_json.as_deref())
        .map_err(to_js_err)
}

/// Validate a cashflow schedule JSON string and return it canonicalized.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @returns Canonicalized JSON-encoded `CashFlowSchedule`.
/// @throws If the schedule JSON is malformed or fails validation.
#[wasm_bindgen(js_name = validateCashflowScheduleJson)]
pub fn validate_cashflow_schedule_json(schedule_json: JsValue) -> Result<String, JsValue> {
    let schedule_json: &str = &json_text(&schedule_json, "scheduleJson")?;
    finstack_quant_cashflows::validate_cashflow_schedule_json(schedule_json).map_err(to_js_err)
}

/// Extract dated flows from a cashflow schedule JSON string.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @returns JSON array of settlement cash entries. PIK and
///   `DefaultedNotional` state rows are omitted; parse the full schedule JSON
///   when flow classification is required.
/// @throws If the schedule JSON is malformed or the schedule fails
///   `CashFlowSchedule` validation (kind `"validation"`).
#[wasm_bindgen(js_name = datedFlowsJson)]
pub fn dated_flows_json(schedule_json: JsValue) -> Result<String, JsValue> {
    let schedule_json: &str = &json_text(&schedule_json, "scheduleJson")?;
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
    schedule_json: JsValue,
    as_of: JsValue,
    config_json: Option<JsValue>,
) -> Result<f64, JsValue> {
    let schedule_json: &str = &json_text(&schedule_json, "scheduleJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let config_json = opt_json_text(config_json.as_ref(), "configJson")?;
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
pub fn cpr_to_smm(cpr: JsValue) -> Result<f64, JsValue> {
    let cpr = js_f64(&cpr, "cpr")?;
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
pub fn smm_to_cpr(smm: JsValue) -> Result<f64, JsValue> {
    let smm = js_f64(&smm, "smm")?;
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
pub fn cdr_to_mdr(cdr: JsValue) -> Result<f64, JsValue> {
    let cdr = js_f64(&cdr, "cdr")?;
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
pub fn mdr_to_cdr(mdr: JsValue) -> Result<f64, JsValue> {
    let mdr = js_f64(&mdr, "mdr")?;
    finstack_quant_cashflows::builder::mdr_to_cdr(mdr).map_err(to_js_err)
}

/// Convert an ABS speed to the single-month mortality for a seasoning month.
///
/// Mirrors Rust `abs_to_smm`: the ABS convention (auto-loan and consumer
/// ABS) prepays a constant share of the *original* balance each month, so
/// `SMM_t = speed / (1 − speed · (t − 1))`. Month 0 is treated as month 1;
/// once the original balance is exhausted the result is capped at 1.0.
///
/// @param speed - Monthly prepayment as a decimal fraction of the original balance (`0.015` = 1.5% ABS), in `[0, 1]`.
/// @param month - Seasoning month counted from origination (non-negative integer).
/// @returns Single-month mortality as a decimal in `[0, 1]`.
/// @throws If `speed` is non-finite or outside `[0, 1]` (kind `validation`), or `month` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = absToSmm)]
pub fn abs_to_smm(speed: JsValue, month: JsValue) -> Result<f64, JsValue> {
    let speed = js_f64(&speed, "speed")?;
    let month: u32 = js_uint(&month, "month")?;
    finstack_quant_cashflows::abs_to_smm(speed, month).map_err(to_js_err)
}

/// Weighted average life of a schedule, in years from `asOf`.
///
/// JSON-first twin of Python `CashFlowSchedule.wal` (Rust
/// `schedule_wal`): WAL over the positive principal flows (amortization,
/// notional and prepayment) dated after `asOf`.
///
/// @param schedule_json - `CashFlowSchedule` (object or JSON).
/// @param as_of - ISO-8601 measurement date; only principal flows strictly after it count.
/// @returns WAL in years; `0` when no principal flow falls after `asOf`.
/// @throws If the schedule or date is malformed or the schedule fails validation (kind `validation`).
#[wasm_bindgen(js_name = scheduleWal)]
pub fn schedule_wal(schedule_json: JsValue, as_of: JsValue) -> Result<f64, JsValue> {
    finstack_quant_cashflows::schedule_wal(
        &json_text(&schedule_json, "scheduleJson")?,
        &js_string(&as_of, "asOf")?,
    )
    .map_err(to_js_err)
}

/// Outstanding principal balance after each unique date of a schedule.
///
/// JSON-first twin of Python `CashFlowSchedule.outstanding_by_date` (Rust
/// `schedule_outstanding_by_date`): principal flows (amortization, PIK,
/// draws and repayments) replayed from the initial notional.
///
/// @param schedule_json - `CashFlowSchedule` (object or JSON) with `meta.issue_date` set.
/// @returns `{ date, amount }` entries in date order; `amount` is the outstanding balance after that date's flows.
/// @throws If the schedule is malformed or fails validation, `meta.issue_date` is unset, or principal flows mix currencies (kind `validation`).
#[wasm_bindgen(js_name = scheduleOutstandingByDate)]
pub fn schedule_outstanding_by_date(schedule_json: JsValue) -> Result<JsValue, JsValue> {
    let rows = finstack_quant_cashflows::schedule_outstanding_by_date(&json_text(
        &schedule_json,
        "scheduleJson",
    )?)
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&rows)
}

/// Calendar-year non-principal / principal / PV ladder of a schedule.
///
/// JSON-first twin of Python `CashFlowSchedule.calendar_year_ladder` (Rust
/// `schedule_calendar_year_ladder`).
///
/// @param schedule_json - `CashFlowSchedule` (object or JSON).
/// @param pvs - Present value of each schedule flow, one per flow in schedule order, in flow-amount units.
/// @returns `{ year, non_principal, principal, pv }` rows in ascending year order.
/// @throws If the schedule is malformed or fails validation, `pvs` does not have one entry per flow, or a value is non-finite (kind `validation`).
#[wasm_bindgen(js_name = scheduleCalendarYearLadder)]
pub fn schedule_calendar_year_ladder(
    schedule_json: JsValue,
    pvs: JsValue,
) -> Result<JsValue, JsValue> {
    let rows = finstack_quant_cashflows::schedule_calendar_year_ladder(
        &json_text(&schedule_json, "scheduleJson")?,
        &js_f64_seq(&pvs, "pvs")?,
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&rows)
}
