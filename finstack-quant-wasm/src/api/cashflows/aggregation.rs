//! Currency-preserving aggregation of dated cashflows.

use super::schedule::dated_flows_arg;
use crate::utils::input::{js_currency, js_f64_seq, js_string, js_string_seq};
use crate::utils::wire::js_wire;
use crate::utils::{parse_iso_dates, to_js_err, to_js_value};
use finstack_quant_cashflows::aggregation::{
    aggregate_by_period as rust_aggregate_by_period,
    aggregate_cashflows_checked as rust_aggregate_cashflows_checked,
    calendar_year_ladder as rust_calendar_year_ladder, PeriodAggregation,
};
use finstack_quant_core::dates::Period;
use wasm_bindgen::prelude::*;

/// Sum dated amounts into reporting periods, keeping currencies separate.
///
/// @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow. A flow falls in the period whose `[start, end)` contains its date; flows outside every period are dropped.
/// @param periods - `Period` wire objects (`{ id, start, end, is_actual }`), sorted by date and non-overlapping.
/// @returns `PeriodAggregation`: `{ periodId: { currency: Money } }`, with only periods that received a flow.
/// @throws If an argument does not match its wire type, the periods are unsorted, overlapping or repeat an id, or a period total is non-finite or out of range (kind `validation`).
#[wasm_bindgen(js_name = aggregateByPeriod)]
pub fn aggregate_by_period(flows: JsValue, periods: JsValue) -> Result<JsValue, JsValue> {
    let flows = dated_flows_arg(&flows, "flows")?;
    let periods = js_wire::<Vec<Period>>(&periods, "periods")?;
    to_js_value(&rust_aggregate_by_period(&flows, &periods).map_err(to_js_err)?)
}

/// Read one period's currency totals using the canonical Rust label lookup.
///
/// @param aggregation - Period-to-currency map with currency-tagged Money values; currency keys must agree with their amounts.
/// @param period - Exact case-sensitive PeriodId display label, such as `"2025Q1"`; whitespace is not trimmed and aliases are not parsed.
/// @returns An independent currency map, including an empty map for a stored empty bucket, or null for an absent or malformed label. No FX conversion is performed.
/// @throws If aggregation has an invalid wire shape or mismatched currencies (kind `validation`), or period is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = periodAggregationGetPeriod)]
pub fn period_aggregation_get_period(
    aggregation: JsValue,
    period: JsValue,
) -> Result<JsValue, JsValue> {
    let aggregation = js_wire::<PeriodAggregation>(&aggregation, "aggregation")?;
    let period = js_string(&period, "period")?;
    to_js_value(&aggregation.get_period(&period))
}

/// Read one period-and-currency amount using the canonical Rust lookup.
///
/// @param aggregation - Period-to-currency map with currency-tagged Money values; currency keys must agree with their amounts.
/// @param period - Exact case-sensitive PeriodId display label; whitespace is not trimmed and aliases are not parsed.
/// @param currency - ISO-4217 currency code of the requested total; no FX conversion is performed.
/// @returns The Money value or null if the period or currency is absent; a malformed period label is a miss.
/// @throws If aggregation or currency has an invalid wire shape, or currency keys disagree with amounts (kind `validation`), or period is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = periodAggregationGetAmount)]
pub fn period_aggregation_get_amount(
    aggregation: JsValue,
    period: JsValue,
    currency: JsValue,
) -> Result<JsValue, JsValue> {
    let aggregation = js_wire::<PeriodAggregation>(&aggregation, "aggregation")?;
    let period = js_string(&period, "period")?;
    let currency = js_currency(&currency, "currency")?;
    to_js_value(&aggregation.get_amount(&period, currency))
}

/// Sum dated amounts that must all be in one currency.
///
/// @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow.
/// @param target - ISO-4217 currency code every flow must be in; no FX conversion is applied.
/// @returns `Money` wire object: the compensated sum in `target`.
/// @throws If a flow is in another currency or the sum cannot be represented (kind `validation`), or an argument does not match its wire type.
#[wasm_bindgen(js_name = aggregateCashflowsChecked)]
pub fn aggregate_cashflows_checked(flows: JsValue, target: JsValue) -> Result<JsValue, JsValue> {
    let flows = dated_flows_arg(&flows, "flows")?;
    let target = js_currency(&target, "target")?;
    to_js_value(&rust_aggregate_cashflows_checked(&flows, target).map_err(to_js_err)?)
}

/// Group dated cashflows into a calendar-year non-principal / principal / PV ladder.
///
/// The four arrays are parallel: entry `i` of each describes flow `i`.
///
/// @param dates - ISO-8601 flow dates; the calendar year of each date is its bucket.
/// @param kinds - Canonical cashflow kind labels (`"fixed"`, `"notional"`, `"pre_payment"`), with surrounding whitespace and ASCII case ignored; principal-like kinds go to `principal`, all others to `non_principal`.
/// @param amounts - Flow amounts in one currency's units.
/// @param pvs - Present value of each flow, in the same units as `amounts`.
/// @returns `{ year, non_principal, principal, pv }` rows in ascending year order.
/// @throws If the arrays differ in length, a kind label is unknown, or an amount or PV is non-finite (kind `validation`), or an array has the wrong element type (kind `invalid_type`).
#[wasm_bindgen(js_name = calendarYearLadder)]
pub fn calendar_year_ladder(
    dates: JsValue,
    kinds: JsValue,
    amounts: JsValue,
    pvs: JsValue,
) -> Result<JsValue, JsValue> {
    let dates = parse_iso_dates(&js_string_seq(&dates, "dates")?)?;
    let kinds = js_string_seq(&kinds, "kinds")?;
    let kind_refs: Vec<&str> = kinds.iter().map(String::as_str).collect();
    to_js_value(
        &rust_calendar_year_ladder(
            &dates,
            &kind_refs,
            &js_f64_seq(&amounts, "amounts")?,
            &js_f64_seq(&pvs, "pvs")?,
        )
        .map_err(to_js_err)?,
    )
}
