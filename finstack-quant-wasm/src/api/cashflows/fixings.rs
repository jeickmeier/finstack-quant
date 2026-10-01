//! Materialization of projected fixings across a market roll.

use super::schedule::schedules_arg;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::to_js_err;
use crate::utils::wire::js_date;
use wasm_bindgen::prelude::*;

/// Materialize the reset observations crossed by a realized-forward market roll.
///
/// Every projected fixing recorded in the schedules' metadata with a date in
/// `(oldDate, newDate]` is written into a copy of the market as an observed
/// fixing; existing observations take precedence and are kept unchanged.
///
/// @param market - `MarketContext` handle of the pre-roll market; it is not modified.
/// @param schedules - Schedules projected on `market` at `oldDate`: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
/// @param old_date - ISO-8601 exclusive start of the fixing window.
/// @param new_date - ISO-8601 inclusive end of the window; must be on or after `oldDate`.
/// @returns A new `MarketContext` handle with the crossed fixings materialized; release it with free().
/// @throws If the window runs backward, a fixing-series identifier is malformed, a crossed projection is unavailable or non-finite, or two schedules project the same index and date differently (kind `validation`).
#[wasm_bindgen(js_name = materializeFixings)]
pub fn materialize_fixings(
    market: &JsMarketContext,
    schedules: JsValue,
    old_date: JsValue,
    new_date: JsValue,
) -> Result<JsMarketContext, JsValue> {
    let schedules = schedules_arg(&schedules, "schedules")?;
    finstack_quant_cashflows::fixings::materialize_fixings(
        market.inner(),
        schedules.iter(),
        js_date(&old_date, "oldDate")?,
        js_date(&new_date, "newDate")?,
    )
    .map(JsMarketContext::from_inner)
    .map_err(to_js_err)
}
