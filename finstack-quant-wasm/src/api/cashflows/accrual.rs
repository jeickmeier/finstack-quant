//! Schedule-driven accrued interest: the `AccrualIndex` handle, the typed
//! one-shot accrual and the ex-coupon date rule.

use super::schedule::JsCashFlowSchedule;
use crate::utils::wire::{js_date, js_opt_wire, js_wire};
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_cashflows::accrual::{
    AccrualConfig, AccrualIndex, ExCouponRule,
    accrued_interest_amount as rust_accrued_interest_amount,
};
use wasm_bindgen::prelude::*;

/// Coupon record date for a coupon paid on a date.
///
/// Settlement on the returned date keeps the coupon; settlement strictly
/// after it and before the payment date trades ex-coupon, and accrued
/// interest is then negative.
///
/// @param rule - `ExCouponRule` wire object: `days_before_coupon` and an optional `calendar_id` (business days when set, calendar days otherwise).
/// @param paymentDate - ISO-8601 coupon payment date the window is counted back from.
/// @returns ISO-8601 record date.
/// @throws If `rule` is not an `ExCouponRule`, `days_before_coupon` exceeds 366, the calendar id cannot be resolved (kind `validation`), or `paymentDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = exCouponRuleExDate, unchecked_return_type = "generated.core.DateWire")]
pub fn ex_coupon_rule_ex_date(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.ExCouponRule")] rule: JsValue,
    #[wasm_bindgen(js_name = "paymentDate", unchecked_param_type = "string")] payment_date: JsValue,
) -> Result<String, JsValue> {
    js_wire::<ExCouponRule>(&rule, "rule")?
        .ex_date(js_date(&payment_date, "paymentDate")?)
        .map(date_to_iso)
        .map_err(to_js_err)
}

/// Whether a settlement date falls in the ex-coupon window of a coupon.
///
/// Free-function twin of Python `ExCouponRule.is_ex_coupon` (Rust
/// `ExCouponRule::is_ex_coupon`): a buyer settling in the window forfeits the
/// imminent coupon.
///
/// @param rule - `ExCouponRule` wire object: `days_before_coupon` and an optional `calendar_id` (business days when set, calendar days otherwise).
/// @param paymentDate - ISO-8601 coupon payment date the ex-coupon date is counted back from.
/// @param settlementDate - ISO-8601 ownership-transfer (settlement) date; convert a trade date to settlement first.
/// @returns `true` on or after the ex-coupon date and strictly before `paymentDate`; settlement strictly before the ex-coupon date keeps the coupon and returns `false`.
/// @throws If `rule` is not an `ExCouponRule`, `days_before_coupon` exceeds 366, the calendar id cannot be resolved, the ex-coupon date is out of range (kind `validation`), or a date is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = exCouponRuleIsExCoupon)]
pub fn ex_coupon_rule_is_ex_coupon(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.ExCouponRule")] rule: JsValue,
    #[wasm_bindgen(js_name = "paymentDate", unchecked_param_type = "string")] payment_date: JsValue,
    #[wasm_bindgen(js_name = "settlementDate", unchecked_param_type = "string")]
    settlement_date: JsValue,
) -> Result<bool, JsValue> {
    js_wire::<ExCouponRule>(&rule, "rule")?
        .is_ex_coupon(
            js_date(&payment_date, "paymentDate")?,
            js_date(&settlement_date, "settlementDate")?,
        )
        .map_err(to_js_err)
}

/// Accrued interest of a schedule as of a date.
///
/// @param schedule - `CashFlowSchedule` handle.
/// @param asOf - ISO-8601 accrual snapshot date; interest accrues through accrual end and stays accrued until payment.
/// @param config - Optional `AccrualConfig` wire object (`{ method, ex_coupon, include_pik, frequency }`); omitted means linear accrual with the Rust defaults (`include_pik` defaults to `false`).
/// @returns `Money` wire object in the indexed coupon currency (falling back to the schedule's notional currency when no matching coupon flows exist); negative inside an ex-coupon window.
/// @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, a configured calendar id cannot be resolved, or a day-count calculation fails (kind `validation`).
#[wasm_bindgen(js_name = accruedInterestAmount, unchecked_return_type = "MoneyValue")]
pub fn accrued_interest_amount(
    schedule: &JsCashFlowSchedule,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "generated.cashflows.AccrualConfig | null")]
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let config = js_opt_wire::<AccrualConfig>(config.as_ref(), "config")?.unwrap_or_default();
    let accrued = rust_accrued_interest_amount(&schedule.inner, js_date(&as_of, "asOf")?, &config)
        .map_err(to_js_err)?;
    to_js_value(&accrued)
}

/// Precomputed accrual state for repeated accrued-interest queries.
///
/// Builds the coupon periods and outstanding path once for a schedule and
/// accrual configuration; prefer it over `accruedInterestAmount` when accruing
/// the same schedule on many dates.
///
/// @example
/// ```typescript
/// import init, { cashflows } from "finstack-quant-wasm";
/// await init();
/// const schedule = cashflows.CashFlowSchedule.builder()
///   .principal({ amount: "1000000", currency: "USD" }, "2025-01-15", "2027-01-15")
///   .fixedCf({ rate: "0.05", coupon_type: "cash", ...cashflows.scheduleParamsSemiannual30360() })
///   .build();
/// const index = cashflows.AccrualIndex.build(schedule);
/// console.log(index.accruedAt("2025-04-15").amount);
/// index.free();
/// schedule.free();
/// ```
#[wasm_bindgen(js_name = AccrualIndex)]
pub struct JsAccrualIndex {
    inner: AccrualIndex,
}

#[wasm_bindgen(js_class = AccrualIndex)]
impl JsAccrualIndex {
    /// Build reusable accrual state for a schedule.
    ///
    /// @param schedule - `CashFlowSchedule` handle with coupon, PIK and notional flows.
    /// @param config - Optional `AccrualConfig` wire object bound into the index; omitted means linear accrual with the Rust defaults (`include_pik` defaults to `false`). Build another index to accrue under a different configuration.
    /// @returns An `AccrualIndex` handle; release it with free().
    /// @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, a configured calendar id cannot be resolved, or the outstanding path or a day-count calculation fails (kind `validation`).
    #[wasm_bindgen]
    pub fn build(
        schedule: &JsCashFlowSchedule,
        #[wasm_bindgen(unchecked_optional_param_type = "generated.cashflows.AccrualConfig | null")]
        config: Option<JsValue>,
    ) -> Result<JsAccrualIndex, JsValue> {
        let config = js_opt_wire::<AccrualConfig>(config.as_ref(), "config")?.unwrap_or_default();
        Ok(JsAccrualIndex {
            inner: AccrualIndex::build(&schedule.inner, &config).map_err(to_js_err)?,
        })
    }

    /// Currency of the accrued interest returned by `accruedAt`.
    ///
    /// @returns ISO-4217 `Currency` wire code of the indexed coupon flows (or the schedule notional when empty).
    /// @throws If the currency cannot be serialized.
    #[wasm_bindgen(js_name = getCurrency, unchecked_return_type = "generated.core.Currency")]
    pub fn get_currency(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_currency())
    }

    /// Accrued interest as of a date.
    ///
    /// @param asOf - ISO-8601 accrual snapshot date.
    /// @returns `Money` wire object in the indexed coupon currency (falling back to the schedule's notional currency when no matching coupon flows exist); negative inside an ex-coupon window.
    /// @throws If `asOf` is not an ISO-8601 string (kind `invalid_type` or `validation`), a day-count calculation fails, or the ex-coupon calendar cannot be resolved (kind `validation`).
    #[wasm_bindgen(js_name = accruedAt, unchecked_return_type = "MoneyValue")]
    pub fn accrued_at(
        &self,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let accrued = self
            .inner
            .accrued_at(js_date(&as_of, "asOf")?)
            .map_err(to_js_err)?;
        to_js_value(&accrued)
    }
}
