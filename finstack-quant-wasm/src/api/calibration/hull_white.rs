//! Direct Hull-White one-factor calibrators on curve handles.
//!
//! Twins of Python `finstack_quant.calibration.hull_white`: quotes, configs
//! and scalar fitted parameters are plain objects (`SwaptionQuote`,
//! `CapFloorQuote`, `CapFloorCalibrationConfig`,
//! `PiecewiseSigmaCalibrationConfig`, `HullWhiteCalibrationParams`), curves
//! are `core.DiscountCurve` handles, and each fit returns the Python tuple as
//! a two-element array `[params, report]`. The piecewise bootstrap returns
//! the `models.rates.hullWhite.HullWhiteParams` handle, the one host type for
//! Rust `HullWhiteParams`.
//!
//! wasm-bindgen cannot borrow an optional class handle, so each cap/floor fit
//! has a raw export without and with `forward`; `exports/calibration.js`
//! picks one (`forward: None` projects on the discount curve).

use crate::api::core::market_discount::JsDiscountCurve;
use crate::api::models::rates::hull_white::JsHullWhiteParams;
use crate::utils::input::{from_js_json, js_f64, js_opt_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::hull_white::{
    self as rust_hw, CapFloorCalibrationConfig, CapFloorQuote, PiecewiseSigmaCalibrationConfig,
    SwapFrequency, SwaptionQuote,
};
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use wasm_bindgen::prelude::*;

/// An omitted frequency is the Rust default (`SwapFrequency::default()`).
fn parse_frequency(frequency: Option<&str>) -> finstack_quant_core::Result<SwapFrequency> {
    frequency
        .map(str::parse::<SwapFrequency>)
        .transpose()
        .map(Option::unwrap_or_default)
}

/// Fit scalar Hull-White `(kappa, sigma)` to at-the-money swaption quotes.
///
/// Twin of Python `calibrate_hull_white_to_swaptions` (Rust
/// `calibrate_hull_white_to_swaptions`).
/// @param discount - Discount curve the swap annuities and forward swap rates are read from; its base date is the valuation date.
/// @param quotes - Array (or JSON) of at least two `SwaptionQuote` objects: `expiry` and `tenor` in years, `volatility` as a decimal, `is_normal_vol`.
/// @param fitTolerance - Required positive maximum reconstructed quote error: decimal rate volatility for normal quotes, relative volatility for Black quotes.
/// @param frequency - Fixed-leg payment frequency: `"annual"`, `"semi_annual"` or `"quarterly"`; defaults to `"semi_annual"`.
/// @param initialGuess - Optional `HullWhiteCalibrationParams` solver seed (`kappa`, `sigma`); omitted uses the built-in starting point.
/// @returns `[params, report]`: the fitted `HullWhiteCalibrationParams` and the `CalibrationReport` with per-quote residuals in volatility units.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` if fewer than two quotes are supplied, a quote or the
/// frequency is invalid (`kind: "validation"`), or the solver fails to converge.
#[wasm_bindgen(js_name = calibrateHullWhiteToSwaptions, unchecked_return_type = "[generated.models.HullWhiteCalibrationParams, generated.calibration.CalibrationReport]")]
pub fn calibrate_hull_white_to_swaptions(
    discount: &JsDiscountCurve,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.calibration.SwaptionQuote[] | string"
    )]
    quotes: JsValue,
    #[wasm_bindgen(js_name = "fitTolerance", unchecked_param_type = "number")]
    fit_tolerance: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "generated.calibration.SwapFrequency | null")]
    frequency: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "initialGuess",
        unchecked_optional_param_type = "generated.models.HullWhiteCalibrationParams | string | null"
    )]
    initial_guess: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let quotes: Vec<SwaptionQuote> = from_js_json(&quotes, "quotes")?;
    let fit_tolerance = js_f64(&fit_tolerance, "fitTolerance")?;
    let frequency = js_opt_string(frequency.as_ref(), "frequency")?;
    let frequency = parse_frequency(frequency.as_deref()).map_err(to_js_err)?;
    let initial_guess: Option<HullWhiteCalibrationParams> = match initial_guess.as_ref() {
        Some(value) if !(value.is_null() || value.is_undefined()) => {
            Some(from_js_json(value, "initialGuess")?)
        }
        _ => None,
    };
    let fitted = rust_hw::calibrate_hull_white_to_swaptions(
        &discount.inner,
        &quotes,
        frequency,
        initial_guess,
        fit_tolerance,
    )
    .map_err(to_js_err)?;
    to_js_value(&fitted)
}

fn calibrate_to_cap_floors_inner(
    discount: &JsDiscountCurve,
    quotes: JsValue,
    config: JsValue,
    forward: Option<&JsDiscountCurve>,
) -> Result<JsValue, JsValue> {
    let quotes: Vec<CapFloorQuote> = from_js_json(&quotes, "quotes")?;
    let config: CapFloorCalibrationConfig = from_js_json(&config, "config")?;
    let fitted = rust_hw::calibrate_hull_white_to_cap_floors(
        &discount.inner,
        forward.map(|curve| curve.inner.as_ref()),
        &quotes,
        config,
    )
    .map_err(to_js_err)?;
    to_js_value(&fitted)
}

/// Raw export behind the published `calibration.calibrateHullWhiteToCapFloors` without a forward curve, projecting on `discount`; see that function.
/// @param discount - Discounting curve handle; its base date is the valuation date.
/// @param quotes - `CapFloorQuote` array or JSON, as the published function takes it.
/// @param config - Calibration config object or JSON, as the published function takes it.
/// @returns `[params, report]`: the calibrated Hull-White parameters and the calibration report, as the published function returns them.
#[wasm_bindgen(js_name = calibrateHullWhiteToCapFloorsWithoutForward, unchecked_return_type = "[generated.models.HullWhiteCalibrationParams, generated.calibration.CalibrationReport]")]
pub fn calibrate_hull_white_to_cap_floors_without_forward(
    discount: &JsDiscountCurve,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.calibration.CapFloorQuote[] | string"
    )]
    quotes: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "generated.calibration.CapFloorCalibrationConfig | string"
    )]
    config: JsValue,
) -> Result<JsValue, JsValue> {
    calibrate_to_cap_floors_inner(discount, quotes, config, None)
}

/// Raw export behind the published `calibration.calibrateHullWhiteToCapFloors` with a forward curve; see that function.
/// @param discount - Discounting curve handle; its base date is the valuation date.
/// @param quotes - `CapFloorQuote` array or JSON, as the published function takes it.
/// @param config - Calibration config object or JSON, as the published function takes it.
/// @param forward - Curve handle projecting the caplet forwards.
/// @returns `[params, report]`: the calibrated Hull-White parameters and the calibration report, as the published function returns them.
#[wasm_bindgen(js_name = calibrateHullWhiteToCapFloorsWithForward, unchecked_return_type = "[generated.models.HullWhiteCalibrationParams, generated.calibration.CalibrationReport]")]
pub fn calibrate_hull_white_to_cap_floors_with_forward(
    discount: &JsDiscountCurve,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.calibration.CapFloorQuote[] | string"
    )]
    quotes: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "generated.calibration.CapFloorCalibrationConfig | string"
    )]
    config: JsValue,
    forward: &JsDiscountCurve,
) -> Result<JsValue, JsValue> {
    calibrate_to_cap_floors_inner(discount, quotes, config, Some(forward))
}

fn bootstrap_sigma_schedule_inner(
    discount: &JsDiscountCurve,
    quotes: JsValue,
    config: JsValue,
    forward: Option<&JsDiscountCurve>,
) -> Result<js_sys::Array, JsValue> {
    let quotes: Vec<CapFloorQuote> = from_js_json(&quotes, "quotes")?;
    let config: PiecewiseSigmaCalibrationConfig = from_js_json(&config, "config")?;
    let (params, report) = rust_hw::bootstrap_hull_white_sigma_schedule_to_cap_floors(
        &discount.inner,
        forward.map(|curve| curve.inner.as_ref()),
        &quotes,
        config,
    )
    .map_err(to_js_err)?;
    Ok(js_sys::Array::of2(
        &JsHullWhiteParams { inner: params }.into(),
        &to_js_value(&report)?,
    ))
}

/// Raw export behind the published `calibration.bootstrapHullWhiteSigmaScheduleToCapFloors` without a forward curve, projecting on `discount`; see that function.
/// @param discount - Discounting curve handle; its base date is the valuation date.
/// @param quotes - `CapFloorQuote` array or JSON, as the published function takes it.
/// @param config - Calibration config object or JSON, as the published function takes it.
/// @returns `[params, report]`: the calibrated Hull-White parameters and the calibration report, as the published function returns them.
#[wasm_bindgen(js_name = bootstrapHullWhiteSigmaScheduleToCapFloorsWithoutForward, unchecked_return_type = "[HullWhiteParams, generated.calibration.CalibrationReport]")]
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors_without_forward(
    discount: &JsDiscountCurve,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.calibration.CapFloorQuote[] | string"
    )]
    quotes: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "generated.calibration.PiecewiseSigmaCalibrationConfig | string"
    )]
    config: JsValue,
) -> Result<js_sys::Array, JsValue> {
    bootstrap_sigma_schedule_inner(discount, quotes, config, None)
}

/// Raw export behind the published `calibration.bootstrapHullWhiteSigmaScheduleToCapFloors` with a forward curve; see that function.
/// @param discount - Discounting curve handle; its base date is the valuation date.
/// @param quotes - `CapFloorQuote` array or JSON, as the published function takes it.
/// @param config - Calibration config object or JSON, as the published function takes it.
/// @param forward - Curve handle projecting the caplet forwards.
/// @returns `[params, report]`: the calibrated Hull-White parameters and the calibration report, as the published function returns them.
#[wasm_bindgen(js_name = bootstrapHullWhiteSigmaScheduleToCapFloorsWithForward, unchecked_return_type = "[HullWhiteParams, generated.calibration.CalibrationReport]")]
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors_with_forward(
    discount: &JsDiscountCurve,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.calibration.CapFloorQuote[] | string"
    )]
    quotes: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "generated.calibration.PiecewiseSigmaCalibrationConfig | string"
    )]
    config: JsValue,
    forward: &JsDiscountCurve,
) -> Result<js_sys::Array, JsValue> {
    bootstrap_sigma_schedule_inner(discount, quotes, config, Some(forward))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_frequency_is_the_rust_default() {
        assert_eq!(parse_frequency(None).ok(), Some(SwapFrequency::default()));
        assert_eq!(
            parse_frequency(Some("quarterly")).ok(),
            Some(SwapFrequency::Quarterly)
        );
        assert!(parse_frequency(Some("monthly")).is_err());
    }
}
