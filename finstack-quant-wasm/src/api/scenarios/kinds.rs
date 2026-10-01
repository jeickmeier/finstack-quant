//! Free-function twin of Python `RateBindingSpec.validate`.
//!
//! WASM scenario enums are their serde wire values (the generated `CurveKind`,
//! `TenorMatchMode`, `TimeRollMode` and `Compounding` types). A Python
//! `Enum.variant()` classmethod has no function here: its twin is the
//! TypeScript literal (`"discount"`, `{ periodic: 4 }`).

use crate::utils::input::from_js_json;
use crate::utils::to_js_err;
use finstack_quant_scenarios::RateBindingSpec;
use wasm_bindgen::prelude::*;

/// Validate a rate binding's identifiers and tenor.
///
/// Free-function twin of Python `RateBindingSpec.validate` (Rust
/// `RateBindingSpec::validate`). Returns `undefined` when valid.
/// @param binding - `RateBindingSpec` object or JSON: `node_id`, `curve_id`, `tenor`, optional `compounding` and `day_count`.
///
/// # Errors
///
/// Throws a `TypeError` when `binding` is not an object or JSON string, and a
/// `validation` error when it does not match the `RateBindingSpec` contract,
/// `node_id` or `curve_id` is blank, or `tenor` is not a valid tenor string.
#[wasm_bindgen(js_name = rateBindingSpecValidate)]
pub fn rate_binding_spec_validate(binding: JsValue) -> Result<(), JsValue> {
    from_js_json::<RateBindingSpec>(&binding, "binding")?
        .validate()
        .map_err(to_js_err)
}
