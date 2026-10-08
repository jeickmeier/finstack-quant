//! WASM binding for the absolute-priority recovery waterfall.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/recovery_waterfall.rs`.
//! `RecoveryClaim`, `RecoveryAllocation` and `RecoveryWaterfallResult` cross
//! the boundary as plain objects in their canonical serde form.

use crate::utils::input::{from_js_json, js_f64};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::recovery_waterfall::{self as waterfall, RecoveryClaim};
use wasm_bindgen::prelude::*;

/// Allocate a bankruptcy estate across claims under the absolute priority rule.
///
/// Secured claims first recover from their own collateral (after the haircut);
/// the remaining estate is then distributed by `priority`, pro rata within a
/// priority level.
/// @param estate_value - Distributable estate value in monetary units; non-negative.
/// @param claims - Array of `RecoveryClaim` objects, or its JSON text. Each claim holds `id`, `seniority`, `priority` (lower is more senior), `principal`, `accrued`, `penalties`, `collateral_value` (or `null`) and `collateral_haircut` in `[0, 1]`.
/// @returns The `RecoveryWaterfallResult` object (`allocations`, `total_distributed`, `undistributed_estate`, `apr_satisfied`).
///
/// # Errors
///
/// Throws a `TypeError` if `claims` is neither a string nor a plain array, and
/// a `validation` error if the estate or a claim amount is negative or
/// non-finite, a haircut is out of range, or claim identifiers repeat.
#[wasm_bindgen(js_name = allocateRecovery)]
pub fn allocate_recovery(estate_value: JsValue, claims: JsValue) -> Result<JsValue, JsValue> {
    let estate_value = js_f64(&estate_value, "estateValue")?;
    let claims: Vec<RecoveryClaim> = from_js_json(&claims, "claims")?;
    let result = waterfall::allocate_recovery(estate_value, &claims).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Total allowed amount of a recovery claim: principal plus accrued amounts
/// and penalties.
///
/// Twin of the Rust `RecoveryClaim::total_claim` and the Python property
/// `RecoveryClaim.total_claim`.
/// @param claim - `RecoveryClaim` object or JSON, as passed to `allocateRecovery`.
/// @returns `principal + accrued + penalties`, in the claim's monetary units.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const claim = {
///   id: "TL-B",
///   seniority: "senior_secured",
///   priority: 1,
///   principal: 100,
///   accrued: 5,
///   penalties: 1,
///   collateral_value: null,
///   collateral_haircut: 0,
/// };
/// console.log(models.credit.recoveryClaimTotalClaim(claim)); // 106
/// ```
///
/// # Errors
///
/// Throws a `TypeError` if `claim` is neither a string nor a plain object,
/// and a `validation` error if it is malformed.
#[wasm_bindgen(js_name = recoveryClaimTotalClaim)]
pub fn recovery_claim_total_claim(claim: JsValue) -> Result<f64, JsValue> {
    let claim: RecoveryClaim = from_js_json(&claim, "claim")?;
    Ok(claim.total_claim())
}
