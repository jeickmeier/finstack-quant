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

/// Build a stamped cashflow schedule envelope from a JSON spec.
///
/// @param spec_json - JSON-encoded `CashflowScheduleBuildSpec`.
/// @param market_json - Optional JSON-encoded market context for floating-rate lookups.
/// @returns JSON-encoded `CashflowScheduleEnvelope`.
/// @throws If the spec or market JSON is malformed, or schedule construction fails.
#[wasm_bindgen(js_name = buildCashflowScheduleEnvelopeJson)]
pub fn build_cashflow_schedule_envelope_json(
    spec_json: &str,
    market_json: Option<String>,
) -> Result<String, JsValue> {
    finstack_quant_cashflows::build_cashflow_schedule_envelope_json(
        spec_json,
        market_json.as_deref(),
    )
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

/// Validate a stamped cashflow schedule envelope and return it canonicalized.
///
/// @param envelope_json - JSON-encoded `CashflowScheduleEnvelope`.
/// @returns Canonicalized JSON-encoded `CashflowScheduleEnvelope`.
/// @throws If the envelope JSON is malformed, has an unsupported schema version,
///   or fails schedule validation.
#[wasm_bindgen(js_name = validateCashflowScheduleEnvelopeJson)]
pub fn validate_cashflow_schedule_envelope_json(envelope_json: &str) -> Result<String, JsValue> {
    finstack_quant_cashflows::validate_cashflow_schedule_envelope_json(envelope_json)
        .map_err(to_js_err)
}

/// Extract dated flows from a cashflow schedule JSON string.
///
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @returns JSON array of `{date, amount}` entries, where `amount` is itself
///   `{amount, currency}`. `CFKind` and accrual metadata are intentionally
///   omitted; parse the full schedule JSON if you need flow classification.
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

/// Create tagged Bond instrument JSON from a cashflow schedule JSON string.
///
/// Convenience wrapper that crosses crates: it materializes a
/// `finstack_quant_valuations::instruments::fixed_income::bond::Bond` from the
/// supplied schedule and wraps it in the tagged `InstrumentJson` envelope.
///
/// @param instrument_id - Identifier for the Bond instrument.
/// @param schedule_json - JSON-encoded `CashFlowSchedule`.
/// @param discount_curve_id - Identifier of the discount curve used for pricing.
/// @param quoted_clean - Optional clean quoted price used to calibrate yield on construction.
/// @returns JSON-encoded tagged `InstrumentJson::Bond`.
/// @throws If the schedule JSON is malformed or bond construction fails.
#[wasm_bindgen(js_name = bondFromCashflowsJson)]
pub fn bond_from_cashflows_json(
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> Result<String, JsValue> {
    finstack_quant_valuations::bond_from_cashflows_json(
        instrument_id,
        schedule_json,
        discount_curve_id,
        quoted_clean,
    )
    .map_err(to_js_err)
}
