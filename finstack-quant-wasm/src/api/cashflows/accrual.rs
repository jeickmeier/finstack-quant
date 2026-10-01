//! Schedule-driven accrued interest: the `AccrualIndex` handle, the typed
//! one-shot accrual and the ex-coupon date rule.

use super::schedule::JsCashFlowSchedule;
use crate::utils::wire::{js_date, js_opt_wire, js_wire};
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_cashflows::accrual::{
    accrued_interest_amount, AccrualConfig, AccrualIndex, ExCouponRule,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// Accrued interest as `Money` in the schedule currency.
fn accrued_money(amount: f64, currency: Currency) -> Result<JsValue, JsValue> {
    to_js_value(&Money::new(amount, currency).map_err(to_js_err)?)
}

/// Ex-coupon date for a coupon paid on a date.
///
/// From the returned date (inclusive) until the payment date (exclusive) the
/// bond trades ex-coupon and accrued interest is negative.
///
/// @param rule - `ExCouponRule` wire object: `days_before_coupon` and an optional `calendar_id` (business days when set, calendar days otherwise).
/// @param payment_date - ISO-8601 coupon payment date the window is counted back from.
/// @returns ISO-8601 ex-coupon date.
/// @throws If `rule` is not an `ExCouponRule`, `days_before_coupon` exceeds 366, the calendar id cannot be resolved (kind `validation`), or `paymentDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = exCouponRuleExDate)]
pub fn ex_coupon_rule_ex_date(rule: JsValue, payment_date: JsValue) -> Result<String, JsValue> {
    js_wire::<ExCouponRule>(&rule, "rule")?
        .ex_date(js_date(&payment_date, "paymentDate")?)
        .map(date_to_iso)
        .map_err(to_js_err)
}

/// Accrued interest of a schedule as of a date.
///
/// Typed twin of `accruedInterest`, which takes schedule JSON and returns a
/// number.
///
/// @param schedule - `CashFlowSchedule` handle.
/// @param as_of - ISO-8601 accrual snapshot date; interest accrues through accrual end and stays accrued until payment.
/// @param config - Optional `AccrualConfig` wire object (`{ method, ex_coupon, include_pik, frequency }`); omitted means linear accrual with the Rust defaults.
/// @returns `Money` wire object in the schedule currency; negative inside an ex-coupon window.
/// @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, or a day-count calculation fails (kind `validation`).
#[wasm_bindgen(js_name = accruedInterestAmount)]
pub fn accrued_interest_amount_js(
    schedule: &JsCashFlowSchedule,
    as_of: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let config = js_opt_wire::<AccrualConfig>(config.as_ref(), "config")?.unwrap_or_default();
    let amount = accrued_interest_amount(&schedule.inner, js_date(&as_of, "asOf")?, &config)
        .map_err(to_js_err)?;
    accrued_money(amount, schedule.inner.get_notional().currency())
}

/// Precomputed accrual state for repeated accrued-interest queries.
///
/// Builds the coupon periods and outstanding path once for a schedule and
/// accrual configuration; prefer it over `accruedInterestAmount` when accruing
/// the same schedule on many dates.
#[wasm_bindgen(js_name = AccrualIndex)]
pub struct JsAccrualIndex {
    inner: AccrualIndex,
    currency: Currency,
}

#[wasm_bindgen(js_class = AccrualIndex)]
impl JsAccrualIndex {
    /// Build reusable accrual state for a schedule.
    ///
    /// @param schedule - `CashFlowSchedule` handle with coupon, PIK and notional flows.
    /// @param config - Optional `AccrualConfig` wire object bound into the index; omitted means linear accrual with the Rust defaults. Build another index to accrue under a different configuration.
    /// @returns An `AccrualIndex` handle; release it with free().
    /// @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, or the outstanding path or a day-count calculation fails (kind `validation`).
    #[wasm_bindgen]
    pub fn build(
        schedule: &JsCashFlowSchedule,
        config: Option<JsValue>,
    ) -> Result<JsAccrualIndex, JsValue> {
        let config = js_opt_wire::<AccrualConfig>(config.as_ref(), "config")?.unwrap_or_default();
        Ok(JsAccrualIndex {
            inner: AccrualIndex::build(&schedule.inner, &config).map_err(to_js_err)?,
            currency: schedule.inner.get_notional().currency(),
        })
    }

    /// Accrued interest as of a date.
    ///
    /// @param as_of - ISO-8601 accrual snapshot date.
    /// @returns `Money` wire object in the schedule currency; negative inside an ex-coupon window.
    /// @throws If `asOf` is not an ISO-8601 string (kind `invalid_type` or `validation`), a day-count calculation fails, or the ex-coupon calendar cannot be resolved (kind `validation`).
    #[wasm_bindgen(js_name = accruedAt)]
    pub fn accrued_at(&self, as_of: JsValue) -> Result<JsValue, JsValue> {
        let amount = self
            .inner
            .accrued_at(js_date(&as_of, "asOf")?)
            .map_err(to_js_err)?;
        accrued_money(amount, self.currency)
    }
}
