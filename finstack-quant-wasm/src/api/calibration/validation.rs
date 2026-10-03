//! Standalone no-arbitrage checks on a volatility surface.
//!
//! Free-function twins of the Python `finstack_quant.calibration.validate_*`
//! functions. Each converts its arguments and calls the matching
//! `finstack_quant_calibration::validation` function; a failed check throws a
//! `FinstackError` with `kind: "validation"`.

use crate::api::core::surfaces::JsVolSurface;
use crate::utils::input::{from_js_json, js_f64_seq};
use crate::utils::to_js_err;
use finstack_quant_calibration::validation as rust_validation;
use finstack_quant_calibration::ValidationConfig;
use wasm_bindgen::prelude::*;

fn parse_config(value: &JsValue) -> Result<ValidationConfig, JsValue> {
    from_js_json(value, "config")
}

/// Run the calendar-spread, butterfly-spread and volatility-bound checks on a surface.
///
/// Free-function twin of Python `calibration.validate_surface` (Rust `validation::validate_surface`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `check_arbitrage: false` skips the two arbitrage checks and `lenient_arbitrage: true` logs arbitrage violations instead of throwing (the volatility bounds are always checked).
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed or any check fails.
#[wasm_bindgen(js_name = validateSurface)]
pub fn validate_surface(surface: &JsVolSurface, config: JsValue) -> Result<(), JsValue> {
    rust_validation::validate_surface(&surface.inner, &parse_config(&config)?).map_err(to_js_err)
}

/// Run the forward-aware calendar-spread and call-convexity checks plus the volatility bounds.
///
/// Free-function twin of Python `calibration.validate_surface_with_forwards` (Rust `validation::validate_surface_with_forwards`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `check_arbitrage: false` skips the arbitrage checks and `lenient_arbitrage: true` logs arbitrage violations instead of throwing.
/// @param forwards - Forward price for each surface expiry, in expiry order (same units as the strikes).
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed, `forwards` does not have one finite positive entry per expiry, or any check fails.
#[wasm_bindgen(js_name = validateSurfaceWithForwards)]
pub fn validate_surface_with_forwards(
    surface: &JsVolSurface,
    config: JsValue,
    forwards: JsValue,
) -> Result<(), JsValue> {
    rust_validation::validate_surface_with_forwards(
        &surface.inner,
        &parse_config(&config)?,
        &js_f64_seq(&forwards, "forwards")?,
    )
    .map_err(to_js_err)
}

/// Check that total variance does not decrease with expiry at each strike.
///
/// Free-function twin of Python `calibration.validate_calendar_spread` (Rust `validation::validate_calendar_spread`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `check_arbitrage: false` skips the check and `lenient_arbitrage: true` logs violations instead of throwing.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed or total variance decreases between two expiries.
#[wasm_bindgen(js_name = validateCalendarSpread)]
pub fn validate_calendar_spread(surface: &JsVolSurface, config: JsValue) -> Result<(), JsValue> {
    rust_validation::validate_calendar_spread(&surface.inner, &parse_config(&config)?)
        .map_err(to_js_err)
}

/// Check that total variance does not decrease with expiry at each forward moneyness.
///
/// Free-function twin of Python `calibration.validate_calendar_spread_with_forwards` (Rust `validation::validate_calendar_spread_with_forwards`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `check_arbitrage: false` skips the check and `lenient_arbitrage: true` logs violations instead of throwing.
/// @param forwards - Forward price for each surface expiry, in expiry order.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed, `forwards` does not match the expiries, or total variance decreases at fixed forward moneyness.
#[wasm_bindgen(js_name = validateCalendarSpreadWithForwards)]
pub fn validate_calendar_spread_with_forwards(
    surface: &JsVolSurface,
    config: JsValue,
    forwards: JsValue,
) -> Result<(), JsValue> {
    rust_validation::validate_calendar_spread_with_forwards(
        &surface.inner,
        &parse_config(&config)?,
        &js_f64_seq(&forwards, "forwards")?,
    )
    .map_err(to_js_err)
}

/// Check that total variance is convex in strike at each expiry (butterfly spread).
///
/// Free-function twin of Python `calibration.validate_butterfly_spread` (Rust `validation::validate_butterfly_spread`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object (`butterfly_upper_ratio`, `butterfly_lower_ratio`; `lenient_arbitrage: true` logs violations instead of throwing); `check_arbitrage: false` skips the check.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed or a butterfly violation exceeds the tolerance.
#[wasm_bindgen(js_name = validateButterflySpread)]
pub fn validate_butterfly_spread(surface: &JsVolSurface, config: JsValue) -> Result<(), JsValue> {
    rust_validation::validate_butterfly_spread(&surface.inner, &parse_config(&config)?)
        .map_err(to_js_err)
}

/// Check that undiscounted Black call prices are convex in strike at each expiry.
///
/// Every adjacent vertical call spread must also cost between zero and its
/// strike width; the price tolerance is `config.tolerance` times the forward.
///
/// Free-function twin of Python `calibration.validate_butterfly_call_convexity` (Rust `validation::validate_butterfly_call_convexity`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `check_arbitrage: false` skips the check and `lenient_arbitrage: true` logs violations instead of throwing.
/// @param forwards - Forward price for each surface expiry, in expiry order.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed, `forwards` does not match the expiries, or call prices are not convex in strike.
#[wasm_bindgen(js_name = validateButterflyCallConvexity)]
pub fn validate_butterfly_call_convexity(
    surface: &JsVolSurface,
    config: JsValue,
    forwards: JsValue,
) -> Result<(), JsValue> {
    rust_validation::validate_butterfly_call_convexity(
        &surface.inner,
        &parse_config(&config)?,
        &js_f64_seq(&forwards, "forwards")?,
    )
    .map_err(to_js_err)
}

/// Check that every surface volatility is positive and at most `config.max_volatility`.
///
/// Free-function twin of Python `calibration.validate_vol_bounds` (Rust `validation::validate_vol_bounds`).
/// @param surface - Implied-volatility surface (expiries in years, absolute strikes).
/// @param config - `ValidationConfig` object; `max_volatility` is the upper bound (annualized decimal).
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `config` is malformed or a volatility is not positive or exceeds `max_volatility`.
#[wasm_bindgen(js_name = validateVolBounds)]
pub fn validate_vol_bounds(surface: &JsVolSurface, config: JsValue) -> Result<(), JsValue> {
    rust_validation::validate_vol_bounds(&surface.inner, &parse_config(&config)?).map_err(to_js_err)
}
