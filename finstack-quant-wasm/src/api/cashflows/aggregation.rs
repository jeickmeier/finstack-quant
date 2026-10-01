//! Currency-preserving aggregation of dated cashflows.

use super::schedule::dated_flows_arg;
use crate::utils::input::{js_f64_seq, js_string, js_string_seq};
use crate::utils::wire::js_wire;
use crate::utils::{parse_iso_dates, to_js_err, to_js_value};
use finstack_quant_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked, calendar_year_ladder, PeriodAggregation,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Period;
use wasm_bindgen::prelude::*;

/// Sum dated amounts into reporting periods, keeping currencies separate.
///
/// @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow. A flow falls in the period whose `[start, end)` contains its date; flows outside every period are dropped.
/// @param periods - `Period` wire objects (`{ id, start, end, is_actual }`), sorted by date and non-overlapping.
/// @returns `PeriodAggregation`: `{ periodId: { currency: Money } }`, with only periods that received a flow.
/// @throws If an argument does not match its wire type, the periods are unsorted, overlapping or repeat an id, or a period total is non-finite or out of range (kind `validation`).
#[wasm_bindgen(js_name = aggregateByPeriod)]
pub fn aggregate_by_period_js(flows: JsValue, periods: JsValue) -> Result<JsValue, JsValue> {
    let flows = dated_flows_arg(&flows, "flows")?;
    let periods = js_wire::<Vec<Period>>(&periods, "periods")?;
    to_js_value(&aggregate_by_period(&flows, &periods).map_err(to_js_err)?)
}

/// Sum dated amounts that must all be in one currency.
///
/// @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow.
/// @param target - ISO-4217 currency code every flow must be in; no FX conversion is applied.
/// @returns `Money` wire object: the compensated sum in `target`.
/// @throws If a flow is in another currency or the sum cannot be represented (kind `validation`), or an argument does not match its wire type.
#[wasm_bindgen(js_name = aggregateCashflowsChecked)]
pub fn aggregate_cashflows_checked_js(flows: JsValue, target: JsValue) -> Result<JsValue, JsValue> {
    let flows = dated_flows_arg(&flows, "flows")?;
    let target = js_wire::<Currency>(&target, "target")?;
    to_js_value(&aggregate_cashflows_checked(&flows, target).map_err(to_js_err)?)
}

/// Group dated cashflows into a calendar-year non-principal / principal / PV ladder.
///
/// The four arrays are parallel: entry `i` of each describes flow `i`.
///
/// @param dates - ISO-8601 flow dates; the calendar year of each date is its bucket.
/// @param kinds - Cashflow kind labels as Rust displays them (`"fixed"`, `"notional"`, `"prepayment"`; also `"coupon"` and `"principal"`), ASCII case ignored; principal-like kinds go to `principal`, all others to `non_principal`.
/// @param amounts - Flow amounts in one currency's units.
/// @param pvs - Present value of each flow, in the same units as `amounts`.
/// @returns `{ year, non_principal, principal, pv }` rows in ascending year order.
/// @throws If the arrays differ in length, a kind label is unknown, or an amount or PV is non-finite (kind `validation`), or an array has the wrong element type (kind `invalid_type`).
#[wasm_bindgen(js_name = calendarYearLadder)]
pub fn calendar_year_ladder_js(
    dates: JsValue,
    kinds: JsValue,
    amounts: JsValue,
    pvs: JsValue,
) -> Result<JsValue, JsValue> {
    let dates = parse_iso_dates(&js_string_seq(&dates, "dates")?)?;
    let kinds = js_string_seq(&kinds, "kinds")?;
    let kind_refs: Vec<&str> = kinds.iter().map(String::as_str).collect();
    to_js_value(
        &calendar_year_ladder(
            &dates,
            &kind_refs,
            &js_f64_seq(&amounts, "amounts")?,
            &js_f64_seq(&pvs, "pvs")?,
        )
        .map_err(to_js_err)?,
    )
}

/// Amount aggregated for one period and currency.
///
/// Mirrors Rust `PeriodAggregation::get_amount`; on a plain object this is
/// `aggregation[period]?.[currency]`.
///
/// @param aggregation - `PeriodAggregation` wire object, as returned by `aggregateByPeriod` or `CashFlowSchedule.pvByPeriod`.
/// @param period - Period code exactly as the aggregation spells it, for example `"2025Q1"`.
/// @param currency - ISO-4217 currency code of the bucket to read.
/// @returns `Money` wire object, or `undefined` when the period has no flows in that currency.
/// @throws If `aggregation` is not a `PeriodAggregation` or `currency` is not an ISO-4217 code (kind `validation`), or `period` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = periodAggregationGetAmount)]
pub fn period_aggregation_get_amount(
    aggregation: JsValue,
    period: JsValue,
    currency: JsValue,
) -> Result<JsValue, JsValue> {
    let aggregation = js_wire::<PeriodAggregation>(&aggregation, "aggregation")?;
    let currency = js_wire::<Currency>(&currency, "currency")?;
    match aggregation.get_amount(&js_string(&period, "period")?, currency) {
        Some(amount) => to_js_value(&amount),
        None => Ok(JsValue::UNDEFINED),
    }
}
