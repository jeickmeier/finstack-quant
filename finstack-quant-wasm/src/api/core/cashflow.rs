//! `finstack_quant_core::cashflow`: `CFKind` and `CashFlow` computations over wire values.
//!
//! `CFKind` crosses the boundary as its snake_case wire string (`"fixed"`,
//! `"pre_payment"`) and `CashFlow` as a plain object; the Rust methods are
//! free functions taking that value.

use crate::utils::input::js_string;
use crate::utils::wire::js_wire;
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
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

/// Date on which a cashflow changes the outstanding balance.
///
/// Mirrors Rust `CashFlow::get_balance_date`: the flow's `principal_date`
/// when set, otherwise its payment `date`.
///
/// @param flow - `CashFlow` wire object.
/// @returns ISO-8601 balance date.
/// @throws If `flow` is not a `CashFlow` (kind `validation`).
#[wasm_bindgen(js_name = cashFlowGetBalanceDate, unchecked_return_type = "generated.core.DateWire")]
pub fn cash_flow_get_balance_date(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
) -> Result<String, JsValue> {
    Ok(date_to_iso(
        js_wire::<CashFlow>(&flow, "flow")?.get_balance_date(),
    ))
}

/// Check a cashflow's invariants.
///
/// @param flow - `CashFlow` wire object.
/// @throws If `flow` is not a `CashFlow`, its accrual factor or rate is non-finite, its accrual factor is negative, its reset date is after its payment date, or its principal delta is in another currency (kind `validation`).
#[wasm_bindgen(js_name = cashFlowValidate)]
pub fn cash_flow_validate(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.CashFlow")] flow: JsValue,
) -> Result<(), JsValue> {
    js_wire::<CashFlow>(&flow, "flow")?
        .validate()
        .map_err(to_js_err)
}
