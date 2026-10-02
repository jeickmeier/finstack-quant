//! Cashflow primitives: `CFKind` and `CashFlow` computations over wire values.
//!
//! `CFKind` crosses the boundary as its snake_case wire string (`"fixed"`,
//! `"pre_payment"`) and `CashFlow` as a plain object; the Rust methods are
//! free functions taking that value.

use crate::utils::input::js_string;
use crate::utils::wire::{js_date, js_wire};
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_cashflows::primitives::{
    is_cash_settlement_kind, CFKind, CashFlow, CashFlowAccrual,
};
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// Parse a cashflow kind label into its `CFKind` wire string.
///
/// The label is the kind as Rust displays it (Rust `CFKind::from_str`), which
/// is the wire string except for prepayments: the label is `"prepayment"`
/// and the wire string `"pre_payment"`.
///
/// @param name - Kind label such as `"fixed"`, `"float_reset"` or `"prepayment"`.
/// @returns The `CFKind` wire string, as used by the `kind` field of a `CashFlow`.
/// @throws If `name` is not a string (kind `invalid_type`) or names no cashflow kind (kind `validation`).
#[wasm_bindgen(js_name = cfKindParse)]
pub fn cf_kind_parse(name: JsValue) -> Result<JsValue, JsValue> {
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
pub fn cf_kind_is_interest_like(kind: JsValue) -> Result<bool, JsValue> {
    Ok(js_wire::<CFKind>(&kind, "kind")?.is_interest_like())
}

/// Whether a cashflow kind changes or returns principal.
///
/// @param kind - `CFKind` wire string.
/// @returns `true` for notional, PIK, amortization, prepayment, revolving draw/repayment and defaulted-notional kinds.
/// @throws If `kind` is not a `CFKind` wire string (kind `validation`).
#[wasm_bindgen(js_name = cfKindIsPrincipalLike)]
pub fn cf_kind_is_principal_like(kind: JsValue) -> Result<bool, JsValue> {
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
#[wasm_bindgen(js_name = isCashSettlementKind)]
pub fn is_cash_settlement_kind_js(kind: JsValue) -> Result<bool, JsValue> {
    Ok(is_cash_settlement_kind(js_wire::<CFKind>(&kind, "kind")?))
}

/// Date on which a cashflow changes the outstanding balance.
///
/// Mirrors Rust `CashFlow::get_balance_date`: the flow's `principal_date`
/// when set, otherwise its payment `date`.
///
/// @param flow - `CashFlow` wire object.
/// @returns ISO-8601 balance date.
/// @throws If `flow` is not a `CashFlow` (kind `validation`).
#[wasm_bindgen(js_name = cashFlowGetBalanceDate)]
pub fn cash_flow_get_balance_date(flow: JsValue) -> Result<String, JsValue> {
    Ok(date_to_iso(
        js_wire::<CashFlow>(&flow, "flow")?.get_balance_date(),
    ))
}

/// Copy of a cashflow with an explicit economic principal date.
///
/// @param flow - `CashFlow` wire object.
/// @param date - ISO-8601 date on which the outstanding balance changes, independent of the cash payment date.
/// @returns The updated `CashFlow`.
/// @throws If `flow` is not a `CashFlow` or `date` is not an ISO date (kind `validation`).
#[wasm_bindgen(js_name = cashFlowWithPrincipalDate)]
pub fn cash_flow_with_principal_date(flow: JsValue, date: JsValue) -> Result<JsValue, JsValue> {
    let flow = js_wire::<CashFlow>(&flow, "flow")?;
    to_js_value(&flow.with_principal_date(js_date(&date, "date")?))
}

/// Copy of a cashflow carrying its accrual-period metadata.
///
/// @param flow - `CashFlow` wire object.
/// @param accrual - `CashFlowAccrual` wire object: accrual start and end dates, day count and optional projected index rate.
/// @returns The updated `CashFlow`.
/// @throws If either argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = cashFlowWithAccrual)]
pub fn cash_flow_with_accrual(flow: JsValue, accrual: JsValue) -> Result<JsValue, JsValue> {
    let flow = js_wire::<CashFlow>(&flow, "flow")?;
    to_js_value(&flow.with_accrual(js_wire::<CashFlowAccrual>(&accrual, "accrual")?))
}

/// Copy of a cashflow with an explicit outstanding-balance change.
///
/// @param flow - `CashFlow` wire object.
/// @param delta - `Money` wire object: change in outstanding principal (positive increases the balance), which may differ from the cash amount.
/// @returns The updated `CashFlow`.
/// @throws If either argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = cashFlowWithPrincipalDelta)]
pub fn cash_flow_with_principal_delta(flow: JsValue, delta: JsValue) -> Result<JsValue, JsValue> {
    let flow = js_wire::<CashFlow>(&flow, "flow")?;
    to_js_value(&flow.with_principal_delta(js_wire::<Money>(&delta, "delta")?))
}

/// Check a cashflow's invariants.
///
/// @param flow - `CashFlow` wire object.
/// @throws If `flow` is not a `CashFlow`, its amount, accrual factor or rate is non-finite, its accrual factor is negative, its reset date is after its payment date, or its principal delta is in another currency (kind `validation`).
#[wasm_bindgen(js_name = cashFlowValidate)]
pub fn cash_flow_validate(flow: JsValue) -> Result<(), JsValue> {
    js_wire::<CashFlow>(&flow, "flow")?
        .validate()
        .map_err(to_js_err)
}
