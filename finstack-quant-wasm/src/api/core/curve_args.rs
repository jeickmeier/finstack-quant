//! Shared argument helpers of the term-structure bindings (`market_discount`,
//! `market_hazard`, `market_forward`, `market_curves`): curve query points and
//! flat `[x0, y0, x1, y1, …]` pillar arrays.

use crate::utils::{parse_iso_date, to_js_err};
use wasm_bindgen::prelude::*;

/// Curve query point: either a year fraction (`number`) or an ISO date (`string`).
pub(crate) enum TimePoint {
    Years(f64),
    Date(finstack_quant_core::dates::Date),
}

/// Parse a curve query point (`number` year fraction or ISO `"YYYY-MM-DD"` date string).
pub(crate) fn js_time_point(value: &JsValue, label: &str) -> Result<TimePoint, JsValue> {
    if let Some(t) = value.as_f64() {
        return Ok(TimePoint::Years(t));
    }
    if let Some(s) = value.as_string() {
        return parse_iso_date(&s).map(TimePoint::Date);
    }
    Err(crate::utils::input::invalid_type(
        label,
        "expected a number (years) or ISO date string",
    ))
}

/// Split a flat `[x0, y0, x1, y1, …]` array into `(x, y)` pairs.
///
/// # Errors
///
/// Returns a validation error naming `label` when the array has odd length.
pub(crate) fn flat_pairs(values: &[f64], label: &str) -> Result<Vec<(f64, f64)>, JsValue> {
    if !values.len().is_multiple_of(2) {
        return Err(to_js_err(format!(
            "{label} array must have even length (flat [x0, y0, x1, y1, …] pairs)"
        )));
    }
    Ok(values.chunks_exact(2).map(|c| (c[0], c[1])).collect())
}

/// Flatten `(x, y)` pairs into `[x0, y0, x1, y1, …]`.
pub(crate) fn flatten_pairs(points: impl Iterator<Item = (f64, f64)>) -> Box<[f64]> {
    points.flat_map(|(x, y)| [x, y]).collect()
}
