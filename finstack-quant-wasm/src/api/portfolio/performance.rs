//! WASM bindings for TWRR / MWRR return linking.

use crate::utils::input::js_f64;

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute a Modified-Dietz TWRR sub-period return from period JSON.
/// @param period_json - `TwrrPeriod` JSON: `beginning_market_value`, `ending_market_value` and optional `cashflows: [{ amount, fraction_of_period_remaining }]` (omitted means no flows); a positive `amount` is a contribution into the portfolio and the fraction, in `[0, 1]`, is the share of the period remaining after the flow. Unknown keys are rejected.
///
/// # Errors
///
/// Throws a JavaScript exception if `periodJson` is malformed, has unknown
/// keys, or the return is undefined (non-positive adjusted denominator,
/// out-of-range cashflow weight, non-finite inputs).
#[wasm_bindgen(js_name = twrrModifiedDietz)]
pub fn twrr_modified_dietz(period_json: JsValue) -> Result<f64, JsValue> {
    let period: finstack_quant_portfolio::performance::TwrrPeriod =
        from_js_json(&period_json, "periodJson")?;
    finstack_quant_portfolio::performance::twrr_modified_dietz(&period).map_err(to_js_err)
}

/// Geometrically link TWRR sub-period returns from returns JSON.
/// @param returns_json - Numeric return-series JSON.
/// @param horizon_years - Return-linking horizon measured in years for annualization.
///
/// # Errors
///
/// Throws a JavaScript exception if `returnsJson` is malformed, the return
/// series is invalid (non-finite sub-period return or return at most -1, non-positive compounded
/// growth factor), or the linked result cannot be converted to a JavaScript
/// value.
#[wasm_bindgen(js_name = twrrLinked)]
pub fn twrr_linked(returns_json: JsValue, horizon_years: JsValue) -> Result<JsValue, JsValue> {
    let horizon_years = js_f64(&horizon_years, "horizonYears")?;
    let returns: Vec<f64> = from_js_json(&returns_json, "returnsJson")?;
    let result = finstack_quant_portfolio::performance::twrr_linked(&returns, horizon_years)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Compute money-weighted return via XIRR from dated cashflow JSON.
///
/// Binds Rust `mwr_xirr`. Returns the unique annualized decimal
/// return under Act/365F, finite and greater than -1. Nonconventional streams
/// are rejected rather than selecting one of their potentially multiple roots.
///
/// # Arguments
///
/// * `cashflows_json` - JSON array of `{ date, amount }` flows from the
///   investor's cash account (contributions negative, distributions and
///   terminal value positive). Dates are sorted and equal-date amounts netted;
///   remaining nonzero flows must change sign exactly once.
///
/// # Errors
///
/// Throws a JavaScript exception if `cashflowsJson` is malformed, contains an
/// invalid date or insufficient net cash flows, the nonzero net flows do not
/// change sign exactly once, or no sufficiently accurate finite return greater
/// than -1 can be found.
#[wasm_bindgen(js_name = mwrXirr)]
pub fn mwr_xirr(cashflows_json: JsValue) -> Result<f64, JsValue> {
    let cashflows: Vec<finstack_quant_portfolio::performance::DatedCashflow> =
        from_js_json(&cashflows_json, "cashflowsJson")?;
    finstack_quant_portfolio::performance::mwr_xirr(&cashflows).map_err(to_js_err)
}
