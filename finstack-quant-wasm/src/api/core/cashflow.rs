//! `finstack_quant_core::cashflow`: `CFKind` and `CashFlow` computations over wire values.
//!
//! `CFKind` crosses the boundary as its snake_case wire string (`"fixed"`,
//! `"pre_payment"`) and `CashFlow` as a plain object; the Rust methods are
//! free functions taking that value.

use crate::utils::input::js_string;
use crate::utils::wire::{js_date, js_wire};
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual};
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// Parse a cashflow kind label into its `CFKind` wire string.
///
/// Parsed by Rust `CFKind::from_str`: the canonical snake_case wire string,
/// ignoring surrounding whitespace and ASCII case.
///
/// @param name - Kind label such as `"fixed"`, `"floating"` or `"pre_payment"`.
/// @returns The `CFKind` wire string, as used by the `kind` field of a `CashFlow`.
/// @throws If `name` is not a string (kind `invalid_type`) or names no cashflow kind (kind `validation`).
#[wasm_bindgen(js_name = cfKindParse, unchecked_return_type = "generated.cashflows.CFKind")]
pub fn cf_kind_parse(
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
) -> Result<JsValue, JsValue> {
    let kind = js_string(&name, "name")?
        .parse::<CFKind>()
        .map_err(to_js_err)?;
    to_js_value(&kind)
}

/// Whether a cashflow kind is interest-like (fixed, floating, inflation or stub coupon).
///
/// @param kind - `CFKind` wire string.
/// @returns `true` for coupon-type kinds; fees, principal and margin kinds are `false`.
/// @throws If `kind` is not a `CFKind` wire string (kind `validation`).
#[wasm_bindgen(js_name = cfKindIsInterestLike)]
pub fn cf_kind_is_coupon(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CFKind")] kind: JsValue,
) -> Result<bool, JsValue> {
    Ok(js_wire::<CFKind>(&kind, "kind")?.is_coupon())
}

/// Whether a cashflow kind changes or returns principal.
///
/// @param kind - `CFKind` wire string.
/// @returns `true` for notional, PIK, amortization, prepayment, revolving draw/repayment and defaulted-notional kinds.
/// @throws If `kind` is not a `CFKind` wire string (kind `validation`).
#[wasm_bindgen(js_name = cfKindIsPrincipalLike)]
pub fn cf_kind_is_principal_like(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CFKind")] kind: JsValue,
) -> Result<bool, JsValue> {
    Ok(js_wire::<CFKind>(&kind, "kind")?.is_principal_like())
}

/// Whether a cashflow kind settles in cash.
///
/// Non-cash state rows (`pik`, `defaulted_notional`) are excluded from
/// settlement sums such as `datedFlows`.
///
/// @param kind - `CFKind` wire string.
/// @returns `true` when flows of this kind are paid or received in cash.
/// @throws If `kind` is not a `CFKind` wire string (kind `validation`).
#[wasm_bindgen(js_name = cfKindIsCashSettlement)]
pub fn cf_kind_is_cash_settlement(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CFKind")] kind: JsValue,
) -> Result<bool, JsValue> {
    Ok(js_wire::<CFKind>(&kind, "kind")?.is_cash_settlement())
}

/// Date on which a principal row changes the outstanding balance.
///
/// Mirrors Rust `CashFlow::get_balance_date`: the flow's explicit
/// `principal_date`; `undefined` for a row that moves no principal. There is
/// no payment-date fallback.
///
/// @param flow - `CashFlow` wire object.
/// @returns ISO-8601 balance date, or `undefined` for a non-principal row.
/// @throws If `flow` is not a `CashFlow` (kind `validation`).
#[wasm_bindgen(
    js_name = cashFlowGetBalanceDate,
    unchecked_return_type = "generated.core.DateWire | undefined"
)]
pub fn cash_flow_get_balance_date(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
) -> Result<Option<String>, JsValue> {
    Ok(js_wire::<CashFlow>(&flow, "flow")?
        .get_balance_date()
        .map(date_to_iso))
}

/// Check a cashflow's invariants.
///
/// Every `CashFlow` the library returns already satisfies them; this is an
/// idempotent check of a hand-written wire object.
///
/// @param flow - `CashFlow` wire object.
/// @throws If `flow` is not a valid `CashFlow`: a non-finite or negative accrual factor, a non-finite rate, a reset date after its payment date, a principal kind without its principal delta and date (or a non-principal kind with them), nonzero cash on a noncash kind, a principal delta in another currency, or an invalid accrual slice (kind `validation`).
#[wasm_bindgen(js_name = cashFlowValidate)]
pub fn cash_flow_validate(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
) -> Result<(), JsValue> {
    js_wire::<CashFlow>(&flow, "flow")?
        .validate()
        .map_err(to_js_err)
}

/// Return a copy of a cashflow carrying new contractual accrual metadata.
///
/// Mirrors Rust `CashFlow::with_accrual`: the updated row is validated, and an
/// invalid update throws without changing `flow`.
///
/// @param flow - `CashFlow` wire object.
/// @param accrual - `CashFlowAccrual` wire object: one actual accrual calculation slice and its day-count context.
/// @returns The updated `CashFlow` wire object.
/// @throws If `flow` or `accrual` is not a valid wire value, or the updated row fails `cashFlowValidate` (kind `validation`).
#[wasm_bindgen(
    js_name = cashFlowWithAccrual,
    unchecked_return_type = "generated.cashflows.CashFlow"
)]
pub fn cash_flow_with_accrual(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlowAccrual")] accrual: JsValue,
) -> Result<JsValue, JsValue> {
    let flow = js_wire::<CashFlow>(&flow, "flow")?;
    let accrual = js_wire::<CashFlowAccrual>(&accrual, "accrual")?;
    to_js_value(&flow.with_accrual(accrual).map_err(to_js_err)?)
}

/// Return a copy of a cashflow with a new principal change and balance date.
///
/// Mirrors Rust `CashFlow::with_principal`: the delta and date are updated
/// together and the result is validated; an invalid update throws without
/// changing `flow`.
///
/// @param flow - `CashFlow` wire object of a principal kind.
/// @param delta - Signed change in outstanding as a `Money` wire object in the cash currency; positive values increase principal.
/// @param date - ISO-8601 balance-effective date of the change; cash still settles on the flow's `date`.
/// @returns The updated `CashFlow` wire object.
/// @throws If an argument is not a valid wire value (kind `invalid_type` or `validation`), or the updated row fails `cashFlowValidate` (kind `validation`).
#[wasm_bindgen(
    js_name = cashFlowWithPrincipal,
    unchecked_return_type = "generated.cashflows.CashFlow"
)]
pub fn cash_flow_with_principal(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.Money")] delta: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
) -> Result<JsValue, JsValue> {
    let flow = js_wire::<CashFlow>(&flow, "flow")?;
    let delta = js_wire::<Money>(&delta, "delta")?;
    let date = js_date(&date, "date")?;
    to_js_value(&flow.with_principal(delta, date).map_err(to_js_err)?)
}
