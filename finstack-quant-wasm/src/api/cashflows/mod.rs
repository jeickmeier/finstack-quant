//! WASM bindings for the `finstack-quant-cashflows` crate.
//!
//! This module holds the JSON-first bridge (`*Json` functions and the
//! schedule-JSON analytics) and the prepayment/default rate conversions. The
//! typed surface lives in the submodules: the `CashFlowSchedule` /
//! `CashFlowBuilder` / `AccrualIndex` handles, and free functions over
//! wire-shaped spec and aggregation values.

pub mod accrual;
pub mod aggregation;
pub mod fixings;
pub mod schedule;
pub mod specs;

use crate::utils::input::{js_f64, js_uint, json_text, opt_json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Build a cashflow schedule from a JSON spec and return canonical schedule JSON.
///
/// IMM/CDS IMM roll rules use quarterly accrual and short-back stubs, regardless
/// of the supplied frequency/stub. IMM retains contractual maturity through a
/// terminal stub. Both modes reject end-of-month rolling; ACT/ACT ICMA rejects
/// third-Wednesday IMM but supports CDS IMM. Plain ACT/ACT ICMA end-of-month
/// schedules require a month-end regular anchor (maturity for front stubs,
/// start otherwise); the opposite endpoint may be irregular. Negative payment
/// and reset lags are rejected.
///
/// Every floating coupon needs its forward curve, even when the supplied
/// fixing observations cover it, unless an explicit fallback applies. An
/// explicit fallback replaces the whole coupon when its projection fails
/// (missing curve, absent fixing series, a reset before the curve base date,
/// or a projection day-count error). Supplied historical gaps before a
/// resolved curve's base date always fail.
///
/// @param spec_json - JSON-encoded `CashflowScheduleBuildSpec`. Optional
///   `principal_exchange` is `"none"` or `"initial_and_final"` (default).
///   `principal_events` entries require both economic `date` and cash `payment_date`.
///   `linear_between` amortization uses coupon accrual boundaries in `(start, end]`;
///   `end` must be an actual boundary no later than the terminal accrual date.
///   Installments use outstanding after start-date PIK and principal movements,
///   and the final installment clears the live balance. Cash settles on the
///   corresponding adjusted, lagged payment dates.
///   Term legs observe one fixing per coupon period at its lagged accrual start;
///   `rate_spec.reset_frequency` and `index_tenor` do not add intraperiod resets
///   or override the named forward curve's index tenor. Overnight legs use daily
///   observations; `overnight_index_constraints` selects daily or final-period
///   index bounds. Final-period index and all-in rate adjustments are allocated
///   uniformly over contractual accrual time when principal changes.
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
///   `DefaultedNotional` state rows and zero-cash principal markers are omitted.
///   Native currencies are retained without conversion or netting; parse the
///   full schedule JSON when flow classification is required.
/// @throws If the schedule JSON, flow amounts, accrual metadata, row currencies,
///   or dates are invalid, or a single-currency principal path fails balance
///   reconciliation (kind `"validation"`). Composite principal paths in
///   multiple currencies receive structural validation without scalar balance
///   reconciliation.
#[wasm_bindgen(js_name = datedFlowsJson)]
pub fn dated_flows_json(schedule_json: JsValue) -> Result<String, JsValue> {
    let schedule_json: &str = &json_text(&schedule_json, "scheduleJson")?;
    finstack_quant_cashflows::dated_flows_json(schedule_json).map_err(to_js_err)
}

/// Convert annual conditional prepayment or default mortality to monthly mortality.
///
/// Uses `monthly = 1 - (1 - annual)^(1/12)` with twelve monthly periods.
///
/// @param annual - Finite annual mortality as a decimal in `[0, 1]` (CPR or CDR); no calendar day-count adjustment applies.
/// @returns Monthly mortality as a decimal; zero and one map to themselves.
/// @throws If `annual` is not a number (kind `invalid_type`), is non-finite, or lies outside `[0, 1]` (kind `validation`).
#[wasm_bindgen(js_name = annualToMonthly)]
pub fn annual_to_monthly(annual: JsValue) -> Result<f64, JsValue> {
    let annual = js_f64(&annual, "annual")?;
    finstack_quant_cashflows::builder::annual_to_monthly(annual).map_err(to_js_err)
}

/// Convert monthly conditional prepayment or default mortality to annual mortality.
///
/// Uses `annual = 1 - (1 - monthly)^12` with twelve monthly periods.
///
/// @param monthly - Finite monthly mortality as a decimal in `[0, 1]` (SMM or MDR); no calendar day-count adjustment applies.
/// @returns Annual mortality as a decimal; zero and one map to themselves.
/// @throws If `monthly` is not a number (kind `invalid_type`), is non-finite, or lies outside `[0, 1]` (kind `validation`).
#[wasm_bindgen(js_name = monthlyToAnnual)]
pub fn monthly_to_annual(monthly: JsValue) -> Result<f64, JsValue> {
    let monthly = js_f64(&monthly, "monthly")?;
    finstack_quant_cashflows::builder::monthly_to_annual(monthly).map_err(to_js_err)
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
    finstack_quant_cashflows::builder::abs_to_smm(speed, month).map_err(to_js_err)
}
