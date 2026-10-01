//! Free-function twins of the Python scenario enum classmethods and of
//! `RateBindingSpec.validate`.
//!
//! WASM scenario enums are their serde wire values (the generated `CurveKind`,
//! `TenorMatchMode`, `TimeRollMode` and `Compounding` types), so each Python
//! `Enum.variant()` classmethod is a function returning that wire value.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_scenarios::{
    Compounding, CurveKind, RateBindingSpec, TenorMatchMode, TimeRollMode,
};
use wasm_bindgen::prelude::*;

/// Wire label of the discount-factor curve kind.
///
/// Free-function twin of Python `CurveKind.discount` (Rust `CurveKind::Discount`).
/// @returns `"discount"`, for operations that target a discount curve.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = curveKindDiscount)]
pub fn curve_kind_discount() -> Result<JsValue, JsValue> {
    to_js_value(&CurveKind::Discount)
}

/// Wire label of the forward-rate curve kind.
///
/// Free-function twin of Python `CurveKind.forward` (Rust `CurveKind::Forward`).
/// @returns `"forward"`, for operations that target a forward curve.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = curveKindForward)]
pub fn curve_kind_forward() -> Result<JsValue, JsValue> {
    to_js_value(&CurveKind::Forward)
}

/// Wire label of the par CDS spread curve kind.
///
/// Free-function twin of Python `CurveKind.par_cds` (Rust `CurveKind::ParCDS`).
/// @returns `"par_cds"`, for operations that shock par CDS spreads.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = curveKindParCds)]
pub fn curve_kind_par_cds() -> Result<JsValue, JsValue> {
    to_js_value(&CurveKind::ParCDS)
}

/// Wire label of the inflation index curve kind.
///
/// Free-function twin of Python `CurveKind.inflation` (Rust `CurveKind::Inflation`).
/// @returns `"inflation"`, for operations that target an inflation curve.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = curveKindInflation)]
pub fn curve_kind_inflation() -> Result<JsValue, JsValue> {
    to_js_value(&CurveKind::Inflation)
}

/// Wire label of the commodity forward price curve kind.
///
/// Free-function twin of Python `CurveKind.commodity` (Rust `CurveKind::Commodity`).
/// @returns `"commodity"`; basis-point shocks on this kind are percent of the forward, not additive bp.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = curveKindCommodity)]
pub fn curve_kind_commodity() -> Result<JsValue, JsValue> {
    to_js_value(&CurveKind::Commodity)
}

/// Wire label of exact tenor-pillar matching.
///
/// Free-function twin of Python `TenorMatchMode.exact` (Rust `TenorMatchMode::Exact`).
/// @returns `"exact"`: the requested tenor must be an existing pillar or the operation fails.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = tenorMatchModeExact)]
pub fn tenor_match_mode_exact() -> Result<JsValue, JsValue> {
    to_js_value(&TenorMatchMode::Exact)
}

/// Wire label of interpolated tenor-pillar matching.
///
/// Free-function twin of Python `TenorMatchMode.interpolate` (Rust `TenorMatchMode::Interpolate`).
/// @returns `"interpolate"`: the bump is spread across the adjacent pillars (the Rust default).
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = tenorMatchModeInterpolate)]
pub fn tenor_match_mode_interpolate() -> Result<JsValue, JsValue> {
    to_js_value(&TenorMatchMode::Interpolate)
}

/// Wire label of the business-day-adjusted time roll.
///
/// Free-function twin of Python `TimeRollMode.business_days` (Rust `TimeRollMode::BusinessDays`).
/// @returns `"business_days"`: the roll target is adjusted ModifiedFollowing (the Rust default).
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = timeRollModeBusinessDays)]
pub fn time_roll_mode_business_days() -> Result<JsValue, JsValue> {
    to_js_value(&TimeRollMode::BusinessDays)
}

/// Wire label of the pure calendar-day time roll.
///
/// Free-function twin of Python `TimeRollMode.calendar_days` (Rust `TimeRollMode::CalendarDays`).
/// @returns `"calendar_days"`: the tenor is added with no business-day adjustment.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = timeRollModeCalendarDays)]
pub fn time_roll_mode_calendar_days() -> Result<JsValue, JsValue> {
    to_js_value(&TimeRollMode::CalendarDays)
}

/// Wire label of the approximate fixed-day-count time roll.
///
/// Free-function twin of Python `TimeRollMode.approximate` (Rust `TimeRollMode::Approximate`).
/// @returns `"approximate"`: fixed day counts, not additive across successive rolls.
///
/// # Errors
///
/// Throws only if the label cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = timeRollModeApproximate)]
pub fn time_roll_mode_approximate() -> Result<JsValue, JsValue> {
    to_js_value(&TimeRollMode::Approximate)
}

/// Wire value of simple (non-compounded) interest.
///
/// Free-function twin of Python `Compounding.simple` (Rust `Compounding::Simple`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `"simple"`.
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingSimple)]
pub fn compounding_simple() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::Simple)
}

/// Wire value of continuous compounding.
///
/// Free-function twin of Python `Compounding.continuous` (Rust `Compounding::Continuous`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `"continuous"`, the `RateBindingSpec` default.
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingContinuous)]
pub fn compounding_continuous() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::Continuous)
}

/// Wire value of annual compounding.
///
/// Free-function twin of Python `Compounding.annual` (Rust `Compounding::Annual`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `"annual"`.
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingAnnual)]
pub fn compounding_annual() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::Annual)
}

/// Wire value of semi-annual compounding.
///
/// Free-function twin of Python `Compounding.semi_annual` (Rust `Compounding::SEMI_ANNUAL`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `{periodic: 2}` (two compounding periods per year).
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingSemiAnnual)]
pub fn compounding_semi_annual() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::SEMI_ANNUAL)
}

/// Wire value of quarterly compounding.
///
/// Free-function twin of Python `Compounding.quarterly` (Rust `Compounding::QUARTERLY`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `{periodic: 4}` (four compounding periods per year).
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingQuarterly)]
pub fn compounding_quarterly() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::QUARTERLY)
}

/// Wire value of monthly compounding.
///
/// Free-function twin of Python `Compounding.monthly` (Rust `Compounding::MONTHLY`). The
/// result is the serde value a `RateBindingSpec.compounding` field takes.
/// @returns `{periodic: 12}` (twelve compounding periods per year).
///
/// # Errors
///
/// Throws only if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = compoundingMonthly)]
pub fn compounding_monthly() -> Result<JsValue, JsValue> {
    to_js_value(&Compounding::MONTHLY)
}

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
