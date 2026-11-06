//! Free-function twins of the Python `OperationSpec` members that compute in Rust.
//!
//! WASM operations are plain objects (the generated `OperationSpec` type). A
//! Python `OperationSpec.<variant>(...)` constructor only builds that object, so
//! its WASM twin is the TypeScript literal (`{ kind: "curve_parallel_bp", ... }`),
//! recorded as `literal` in the parity ledger. The functions here are the ones
//! that do work in Rust: `parallelBpMany` expands one shift into several
//! operations, `operationSpecTimeRollForward` applies the Rust defaults, and the
//! predicates and `operationSpecValidate` run the canonical Rust rules.

use crate::utils::input::{
    from_js_json, js_f64, js_opt_bool, js_opt_string, js_string, js_string_seq,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::{OperationSpec, TimeRollMode};
use wasm_bindgen::prelude::*;

/// Parse a serde string-enum argument from its wire label.
fn label<T: serde::de::DeserializeOwned>(value: &JsValue, name: &str) -> Result<T, JsValue> {
    finstack_quant_core::wire::serde_parse(&js_string(value, name)?).map_err(to_js_err)
}

/// Parse an optional serde string-enum argument from its wire label.
fn opt_label<T: serde::de::DeserializeOwned>(
    value: Option<&JsValue>,
    name: &str,
) -> Result<Option<T>, JsValue> {
    js_opt_string(value, name)?
        .as_deref()
        .map(finstack_quant_core::wire::serde_parse)
        .transpose()
        .map_err(to_js_err)
}

fn opt_curve_id(value: Option<&JsValue>, name: &str) -> Result<Option<CurveId>, JsValue> {
    Ok(js_opt_string(value, name)?.map(|id| CurveId::from(id.as_str())))
}

fn operation(value: &JsValue) -> Result<OperationSpec, JsValue> {
    from_js_json(value, "operation")
}

/// Expand a parallel shift into one operation per curve identifier.
/// @param curveKind - Canonical discount, forward, par_cds, inflation or commodity family label.
/// @param curveIds - Ordered array of curve identifiers; empty input returns an empty array.
/// @param bp - Additive basis points; percent of forward for commodity curves.
/// @param discountCurveId - Optional ParCDS discount curve copied onto every operation.
/// @returns An array of operations preserving identifier order.
///
/// # Errors
///
/// Throws TypeError for malformed identifiers or non-numeric bp, and a validation error for an unknown curve family.
#[wasm_bindgen(js_name = parallelBpMany, unchecked_return_type = "OperationSpec[]")]
pub fn parallel_bp_many(
    #[wasm_bindgen(js_name = "curveKind", unchecked_param_type = "CurveKind")] curve_kind: JsValue,
    #[wasm_bindgen(js_name = "curveIds", unchecked_param_type = "string[]")] curve_ids: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] bp: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_optional_param_type = "string")]
    discount_curve_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let ids = js_string_seq(&curve_ids, "curveIds")?;
    to_js_value(&OperationSpec::parallel_bp_many(
        label(&curve_kind, "curveKind")?,
        ids,
        js_f64(&bp, "bp")?,
        opt_curve_id(discount_curve_id.as_ref(), "discountCurveId")?,
    ))
}

/// Build a time-roll operation moving the valuation horizon forward.
///
/// Free-function twin of Python `OperationSpec.time_roll_forward` (Rust
/// `OperationSpec::time_roll_forward`).
/// @param period - Tenor-style roll period such as `"1D"`, `"1W"`, `"1M"` or `"1Y"`; parsed when the operation is validated.
/// @param applyShocks - Whether the remaining operations run after the roll; omit for the Rust default, `true`.
/// @param rollMode - Optional roll semantics label: `"business_days"`, `"calendar_days"` or `"approximate"`; omit for the Rust default, `"business_days"`.
/// @returns The `time_roll_forward` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown `rollMode` label.
#[wasm_bindgen(js_name = operationSpecTimeRollForward, unchecked_return_type = "OperationSpec")]
pub fn operation_spec_time_roll_forward(
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
    #[wasm_bindgen(js_name = "applyShocks", unchecked_optional_param_type = "boolean")]
    apply_shocks: Option<JsValue>,
    #[wasm_bindgen(js_name = "rollMode", unchecked_optional_param_type = "TimeRollMode")] roll_mode: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let period = js_string(&period, "period")?;
    let apply_shocks = js_opt_bool(apply_shocks.as_ref(), "applyShocks")?;
    let roll_mode: Option<TimeRollMode> = opt_label(roll_mode.as_ref(), "rollMode")?;
    to_js_value(&OperationSpec::time_roll_forward(
        period,
        apply_shocks,
        roll_mode,
    ))
}

/// Validate one scenario operation with the canonical Rust rules.
///
/// Free-function twin of Python `OperationSpec.validate` (Rust
/// `OperationSpec::validate`): identifiers, finite numbers, variant-specific
/// floors and tenor parsing. Returns `undefined` when valid.
/// @param operation - `OperationSpec` object or JSON to check.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant or fails
/// the variant's rules (blank identifier, non-finite number, FX `pct <= -100`,
/// unparseable tenor).
#[wasm_bindgen(js_name = operationSpecValidate)]
pub fn operation_spec_validate(
    #[wasm_bindgen(unchecked_param_type = "OperationSpec | string")] operation: JsValue,
) -> Result<(), JsValue> {
    self::operation(&operation)?.validate().map_err(to_js_err)
}

/// Report whether an operation needs instruments in the execution context.
///
/// Free-function twin of Python `OperationSpec.requires_instruments` (Rust
/// `OperationSpec::requires_instruments`).
/// @param operation - `OperationSpec` object or JSON to inspect.
/// @returns `true` for instrument-scoped shocks and time rolls.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant.
#[wasm_bindgen(js_name = operationSpecRequiresInstruments)]
pub fn operation_spec_requires_instruments(
    #[wasm_bindgen(unchecked_param_type = "OperationSpec | string")] operation: JsValue,
) -> Result<bool, JsValue> {
    Ok(self::operation(&operation)?.requires_instruments())
}

/// Report whether an operation can replace or mutate instruments.
///
/// Free-function twin of Python `OperationSpec.mutates_instruments` (Rust
/// `OperationSpec::mutates_instruments`).
/// @param operation - `OperationSpec` object or JSON to inspect.
/// @returns `true` for price, spread and structured-credit correlation shocks; `false` for a time roll.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant.
#[wasm_bindgen(js_name = operationSpecMutatesInstruments)]
pub fn operation_spec_mutates_instruments(
    #[wasm_bindgen(unchecked_param_type = "OperationSpec | string")] operation: JsValue,
) -> Result<bool, JsValue> {
    Ok(self::operation(&operation)?.mutates_instruments())
}
