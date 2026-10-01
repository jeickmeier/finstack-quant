//! Direct Hull-White one-factor calibrators on curve handles.
//!
//! Twins of Python `finstack_quant.calibration.hull_white`: quotes, configs
//! and fitted parameters are plain objects (`SwaptionQuote`, `CapFloorQuote`,
//! `CapFloorCalibrationConfig`, `PiecewiseSigmaCalibrationConfig`,
//! `HullWhiteCalibrationParams`, `HullWhiteParams`), curves are
//! `core.DiscountCurve` handles, and each fit returns the Python tuple as a
//! two-element array `[params, report]`.
//!
//! wasm-bindgen cannot borrow an optional class handle, so the raw cap/floor
//! functions take `forward` as a required handle; `exports/calibration.js`
//! passes `discount` when the caller omits it, which is the Rust default
//! (`forward: None` projects on the discount curve).

use crate::api::core::market_data::JsDiscountCurve;
use crate::utils::input::{from_js_json, js_f64, js_opt_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::hull_white::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves,
    calibrate_hull_white_to_cap_floors_from_curves, calibrate_hull_white_to_swaptions_from_curve,
    CapFloorCalibrationConfig, CapFloorQuote, HullWhiteCalibrationParams, HullWhiteParams,
    PiecewiseSigmaCalibrationConfig, SwapFrequency, SwaptionQuote,
};
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
/// `calibrate_hull_white_to_swaptions_from_curve`).
/// @param discount - Discount curve the swap annuities and forward swap rates are read from; its base date is the valuation date.
/// @param quotes - Array (or JSON) of at least two `SwaptionQuote` objects: `expiry` and `tenor` in years, `volatility` as a decimal, `is_normal_vol`.
/// @param fit_tolerance - Required positive maximum reconstructed quote error: decimal rate volatility for normal quotes, relative volatility for Black quotes.
/// @param frequency - Fixed-leg payment frequency: `"annual"`, `"semi_annual"` or `"quarterly"`; defaults to `"semi_annual"`.
/// @param initial_guess - Optional `HullWhiteCalibrationParams` solver seed (`kappa`, `sigma`); omitted uses the built-in starting point.
/// @returns `[params, report]`: the fitted `HullWhiteCalibrationParams` and the `CalibrationReport` with per-quote residuals in volatility units.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` if fewer than two quotes are supplied, a quote or the
/// frequency is invalid (`kind: "validation"`), or the solver fails to converge.
#[wasm_bindgen(js_name = calibrateHullWhiteToSwaptions)]
pub fn calibrate_hull_white_to_swaptions(
    discount: &JsDiscountCurve,
    quotes: JsValue,
    fit_tolerance: JsValue,
    frequency: Option<JsValue>,
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
    let fitted = calibrate_hull_white_to_swaptions_from_curve(
        &discount.inner,
        &quotes,
        frequency,
        initial_guess,
        fit_tolerance,
    )
    .map_err(to_js_err)?;
    to_js_value(&fitted)
}

/// Fit scalar Hull-White `(kappa, sigma)` to cap/floor quotes.
///
/// Twin of Python `calibrate_hull_white_to_cap_floors` (Rust
/// `calibrate_hull_white_to_cap_floors_from_curves`).
/// @param discount - Discounting curve; its base date is the valuation date.
/// @param quotes - Array (or JSON) of `CapFloorQuote` objects: `maturity` in years, `strike` and `volatility` as decimals, `is_cap`, `is_normal_vol`. A single quote requires `config.fixed_kappa`.
/// @param config - `CapFloorCalibrationConfig` object: required `fit_tolerance` (normal-vol units), optional `frequency` (default `"semi_annual"`), `fixed_kappa` and `initial_guess`.
/// @param forward - Curve projecting the caplet forwards; the published function defaults it to `discount`.
/// @returns `[params, report]`: the fitted `HullWhiteCalibrationParams` and the `CalibrationReport`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` if no quotes are supplied, a single quote is given
/// without `fixed_kappa`, a quote or the config is invalid
/// (`kind: "validation"`), or the solver fails to converge.
#[wasm_bindgen(js_name = calibrateHullWhiteToCapFloors)]
pub fn calibrate_hull_white_to_cap_floors(
    discount: &JsDiscountCurve,
    quotes: JsValue,
    config: JsValue,
    forward: &JsDiscountCurve,
) -> Result<JsValue, JsValue> {
    let quotes: Vec<CapFloorQuote> = from_js_json(&quotes, "quotes")?;
    let config: CapFloorCalibrationConfig = from_js_json(&config, "config")?;
    let fitted = calibrate_hull_white_to_cap_floors_from_curves(
        &discount.inner,
        Some(&forward.inner),
        &quotes,
        config,
    )
    .map_err(to_js_err)?;
    to_js_value(&fitted)
}

/// Bootstrap a piecewise-constant Hull-White sigma schedule to cap/floor quotes.
///
/// Twin of Python `bootstrap_hull_white_sigma_schedule_to_cap_floors` (Rust
/// `bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves`).
/// @param discount - Discounting curve; its base date is the valuation date.
/// @param quotes - Array (or JSON) of `CapFloorQuote` objects with distinct maturities in years; each maturity adds one constant-sigma interval.
/// @param config - `PiecewiseSigmaCalibrationConfig` object: `fixed_kappa`, `sigma_min`, `sigma_max` (absolute rate volatility), `fit_tolerance` and optional `frequency` (default `"semi_annual"`).
/// @param forward - Curve projecting the caplet forwards; the published function defaults it to `discount`.
/// @returns `[params, report]`: the piecewise `HullWhiteParams` and the `CalibrationReport`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` if no quotes are supplied, maturities repeat, the
/// configuration bounds are invalid (`kind: "validation"`), or an interval's
/// sigma cannot be bracketed or solved.
#[wasm_bindgen(js_name = bootstrapHullWhiteSigmaScheduleToCapFloors)]
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors(
    discount: &JsDiscountCurve,
    quotes: JsValue,
    config: JsValue,
    forward: &JsDiscountCurve,
) -> Result<JsValue, JsValue> {
    let quotes: Vec<CapFloorQuote> = from_js_json(&quotes, "quotes")?;
    let config: PiecewiseSigmaCalibrationConfig = from_js_json(&config, "config")?;
    let fitted = bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves(
        &discount.inner,
        Some(&forward.inner),
        &quotes,
        config,
    )
    .map_err(to_js_err)?;
    to_js_value(&fitted)
}

/// Short-rate volatility of a piecewise Hull-White parameter set at a model time.
///
/// Free-function twin of Python `HullWhiteParams.sigma_at` (Rust
/// `HullWhiteParams::volatility` read with `PiecewiseConstantCurve::value_at`).
/// @param params - `HullWhiteParams` object (or JSON): `kappa` and the piecewise `volatility` schedule (`times`, `values`).
/// @param time - Year fraction at which the left-continuous piecewise sigma is read.
/// @returns Sigma applying at `time`, in absolute rate units per square-root year.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if `params` is not a valid
/// `HullWhiteParams` object.
#[wasm_bindgen(js_name = hullWhiteParamsSigmaAt)]
pub fn hull_white_params_sigma_at(params: JsValue, time: JsValue) -> Result<f64, JsValue> {
    let params: HullWhiteParams = from_js_json(&params, "params")?;
    let time = js_f64(&time, "time")?;
    Ok(params.volatility.value_at(time))
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
