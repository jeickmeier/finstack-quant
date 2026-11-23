//! Implied volatility solvers, SVI calibration/evaluation, and FX delta math.

//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::api::core::surfaces::{JsFxDeltaVolSurface, JsVolCube};
use crate::utils::input::{from_js_json, js_bool, js_f64, js_f64_seq, js_uint};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::svi::SviParams;
use wasm_bindgen::prelude::*;

/// Bachelier (normal) implied volatility from an undiscounted option price.
///
/// Inverts the Bachelier price per unit annuity (`df = 1`); forward and
/// strike may be zero or negative.
///
/// @returns Normal volatility in absolute forward units per square-root year;
/// `0` when `price` equals intrinsic value.
///
/// # Errors
///
/// Throws a `FinstackError` (kind `validation`) if `forward` or `strike` is
/// non-finite, `t` is not positive, `price` is negative, non-finite or below
/// intrinsic value, or the solver does not converge.
/// @param price - Non-negative undiscounted option price (forward premium per unit annuity) in the forward's units.
/// @param forward - Finite forward rate or price; negative values are allowed.
/// @param strike - Finite strike in the forward's units; negative values are allowed.
/// @param t - Strictly positive time to expiry in years.
/// @param isCall - `true` for a call, `false` for a put.
#[wasm_bindgen(js_name = impliedVolBachelier)]
pub fn implied_vol_bachelier(
    #[wasm_bindgen(unchecked_param_type = "number")] price: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let price = js_f64(&price, "price")?;
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let t = js_f64(&t, "t")?;
    let is_call = js_bool(&is_call, "isCall")?;
    vol::implied_vol_bachelier(price, forward, strike, t, is_call).map_err(to_js_err)
}

/// Black-76 (lognormal) implied volatility from an undiscounted option price.
///
/// Inverts the Black-76 price per unit annuity (`df = 1`); for a discounted
/// price use `models.black76ImpliedVol`.
///
/// @returns Black volatility as an annualized decimal; `0` when `price`
/// equals intrinsic value.
///
/// # Errors
///
/// Throws a `FinstackError` (kind `validation`) if `forward`, `strike` or `t`
/// is not positive and finite, `price` is negative or non-finite, `price` is
/// below intrinsic value or at/above the upper bound (forward for a call,
/// strike for a put), or the solver does not converge.
/// @param price - Non-negative undiscounted option price (forward premium per unit annuity) in the forward's units.
/// @param forward - Strictly positive forward rate or price.
/// @param strike - Strictly positive strike in the forward's units.
/// @param t - Strictly positive time to expiry in years.
/// @param isCall - `true` for a call, `false` for a put.
#[wasm_bindgen(js_name = impliedVolBlack)]
pub fn implied_vol_black(
    #[wasm_bindgen(unchecked_param_type = "number")] price: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let price = js_f64(&price, "price")?;
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let t = js_f64(&t, "t")?;
    let is_call = js_bool(&is_call, "isCall")?;
    vol::implied_vol_black(price, forward, strike, t, is_call).map_err(to_js_err)
}

/// Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.
///
/// Prices are equated at the money (strike = forward) and the target vol is
/// solved deterministically.
///
/// # Errors
///
/// Throws a JavaScript exception if a convention cannot be decoded, an input
/// is outside its domain, or the price-matching solver fails to converge.
/// @param vol - Input volatility in the source convention: decimal Black vol for `"lognormal"` / shifted-lognormal, absolute vol in the forward's rate units for `"normal"`. Must be positive.
/// @param fromConvention - `"normal"`, `"lognormal"`, or `{"shifted_lognormal": {"shift": s}}` (serde form of `VolatilityConvention`).
/// @param toConvention - Target convention in the same encoding.
/// @param forwardRate - ATM forward rate or price; must satisfy the target convention's domain.
/// @param timeToExpiry - Time to expiry in years (non-negative).
/// @returns Volatility in the target convention (decimal Black vol, or absolute normal vol).
#[wasm_bindgen(js_name = convertAtmVolatility)]
pub fn convert_atm_volatility(
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(
        js_name = "fromConvention",
        unchecked_param_type = "VolatilityConvention"
    )]
    from_convention: JsValue,
    #[wasm_bindgen(
        js_name = "toConvention",
        unchecked_param_type = "VolatilityConvention"
    )]
    to_convention: JsValue,
    #[wasm_bindgen(js_name = "forwardRate", unchecked_param_type = "number")] forward_rate: JsValue,
    #[wasm_bindgen(js_name = "timeToExpiry", unchecked_param_type = "number")]
    time_to_expiry: JsValue,
) -> Result<f64, JsValue> {
    let vol = js_f64(&vol, "vol")?;
    let forward_rate = js_f64(&forward_rate, "forwardRate")?;
    let time_to_expiry = js_f64(&time_to_expiry, "timeToExpiry")?;
    // `js_wire`, not `from_js_json`: the unit variants are bare wire strings
    // (`"normal"`), which `from_js_json` would parse as JSON text.
    let from: vol::VolatilityConvention = js_wire(&from_convention, "fromConvention")?;
    let to: vol::VolatilityConvention = js_wire(&to_convention, "toConvention")?;
    vol::convert_atm_volatility(vol, from, to, forward_rate, time_to_expiry).map_err(to_js_err)
}

/// Calibrate Gatheral SVI parameters `{a, b, rho, m, sigma}` to a market smile.
///
/// Gatheral (2004): see docs/REFERENCES.md#gatheral-2004-svi.
///
/// # Errors
///
/// Throws a JavaScript exception if lengths differ, fewer than five quotes are
/// supplied, an input is outside its domain, the optimizer fails to converge,
/// or the fit violates the SVI no-arbitrage conditions.
/// @param strikes - Positive strikes (at least five).
/// @param vols - Black implied vols (decimal) aligned one-for-one with `strikes`.
/// @param forward - Positive forward at `expiry`.
/// @param expiry - Positive time to expiry in years.
/// @returns Validated SVI parameters `{ a, b, rho, m, sigma }`.
#[wasm_bindgen(js_name = calibrateSvi, unchecked_return_type = "SviParams")]
pub fn calibrate_svi(
    #[wasm_bindgen(unchecked_param_type = "number[]")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number[]")] vols: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
) -> Result<JsValue, JsValue> {
    let strikes = js_f64_seq(&strikes, "strikes")?;
    let vols = js_f64_seq(&vols, "vols")?;
    let forward = js_f64(&forward, "forward")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let params =
        finstack_quant_models::volatility::svi::calibrate_svi(&strikes, &vols, forward, expiry)
            .map_err(to_js_err)?;
    to_js_value(&params)
}

/// Black implied volatility from SVI parameters at log-moneyness `k = ln(K / F)`.
///
/// # Errors
///
/// Throws a JavaScript exception if `params` fails validation, `t` is not
/// positive, or the total variance at `k` is negative.
/// @param params - SVI parameter object `{a, b, rho, m, sigma}` (validated on decode).
/// @param k - Log-moneyness `ln(K / F)`.
/// @param t - Positive time to expiry in years.
/// @returns Annualized Black volatility as a decimal.
#[wasm_bindgen(js_name = sviImpliedVol)]
pub fn svi_implied_vol(
    #[wasm_bindgen(unchecked_param_type = "SviParams")] params: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] k: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
) -> Result<f64, JsValue> {
    let k = js_f64(&k, "k")?;
    let t = js_f64(&t, "t")?;
    let params: finstack_quant_models::volatility::svi::SviParams =
        from_js_json(&params, "params")?;
    params.implied_vol(k, t).map_err(to_js_err)
}

/// Evaluate a core SABR cube in the requested quote convention.
///
/// # Errors
///
/// Throws a JavaScript exception for out-of-grid coordinates, invalid SABR
/// model inputs, or a shifted-lognormal or undecodable `convention`.
/// @param cube - Structurally validated data-only volatility cube.
/// @param expiry - Positive option expiry in years within the cube grid.
/// @param tenor - Positive underlying tenor in years within the cube grid.
/// @param strike - Finite strike in the same rate units as the stored forwards.
/// @param convention - Output quote convention: `"lognormal"` (decimal Black, shifted by the cube's own SABR shifts where it stores them) or `"normal"` (absolute Bachelier vol in the forward's rate units). The serde form `{"shifted_lognormal": {"shift": s}}` is rejected because the cube already carries its own shifts.
/// @returns Annualized volatility: decimal Black for `"lognormal"`, absolute rate units for `"normal"`.
#[wasm_bindgen(js_name = getCubeVol)]
pub fn get_cube_vol(
    cube: &JsVolCube,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] tenor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "VolatilityConvention")] convention: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    let convention: vol::VolatilityConvention = js_wire(&convention, "convention")?;
    vol::get_cube_vol(&cube.inner, expiry, tenor, strike, convention).map_err(to_js_err)
}

/// Evaluate a core SABR cube in the requested convention with flat coordinate
/// clamping; throws on undefined volatility.
///
/// # Errors
///
/// Throws a JavaScript exception for a non-numeric coordinate or an
/// unsupported `convention` or failed SABR expansion.
/// @param cube - Structurally validated data-only volatility cube.
/// @param expiry - Finite option expiry in years; clamped to the stored grid.
/// @param tenor - Finite underlying tenor in years; clamped to the stored grid.
/// @param strike - Finite strike in the same rate units as the stored forwards.
/// @param convention - Output quote convention: `"lognormal"` (decimal Black, shifted by the cube's own SABR shifts where it stores them) or `"normal"` (absolute Bachelier vol in the forward's rate units). The serde form `{"shifted_lognormal": {"shift": s}}` is rejected because the cube already carries its own shifts.
/// @returns Annualized volatility in the selected convention.
#[wasm_bindgen(js_name = getCubeVolClamped)]
pub fn get_cube_vol_clamped(
    cube: &JsVolCube,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] tenor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "VolatilityConvention")] convention: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    let convention: vol::VolatilityConvention = js_wire(&convention, "convention")?;
    vol::get_cube_vol_clamped(&cube.inner, expiry, tenor, strike, convention).map_err(to_js_err)
}

/// Return ATM, 25-delta put/call, and optional 10-delta put/call vols at a stored FX expiry.
///
/// # Errors
///
/// Throws a JavaScript exception when `expiry_index` is non-finite, fractional,
/// negative, exceeds the wasm32 integer range, or is outside the surface.
/// @param surface - Structurally validated data-only FX delta surface.
/// @param expiryIndex - Zero-based stored expiry index.
/// @returns Three `[atm, put25d, call25d]` annualized decimal volatilities, or five `[atm, put25d, call25d, put10d, call10d]` values when 10-delta wings are present.
/// @throws Error - Throws `invalid_type` for a non-integer `expiryIndex` or `validation` if out of range.
#[wasm_bindgen(js_name = getFxDeltaPillarVols)]
pub fn get_fx_delta_pillar_vols(
    surface: &JsFxDeltaVolSurface,
    #[wasm_bindgen(js_name = "expiryIndex", unchecked_param_type = "number")] expiry_index: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let expiry_index: usize = js_uint(&expiry_index, "expiryIndex")?;
    vol::get_fx_delta_pillar_vols(&surface.inner, expiry_index)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Evaluate an FX delta-quoted surface at an expiry, strike, and forward.
///
/// # Errors
///
/// Throws a JavaScript exception for invalid coordinates or a non-positive
/// reconstructed wing volatility.
/// @param surface - Structurally validated data-only FX delta surface.
/// @param expiry - Positive option expiry in years.
/// @param strike - Positive strike in the FX quote currency.
/// @param forward - Positive FX forward in quote currency per base currency.
/// @returns Annualized Black volatility as a decimal.
#[wasm_bindgen(js_name = getFxDeltaVol)]
pub fn get_fx_delta_vol(
    surface: &JsFxDeltaVolSurface,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let strike = js_f64(&strike, "strike")?;
    let forward = js_f64(&forward, "forward")?;
    vol::get_fx_delta_vol(&surface.inner, expiry, strike, forward).map_err(to_js_err)
}

/// Convert premium-unadjusted forward delta magnitude to strike.
///
/// # Errors
///
/// Throws a `TypeError` for mistyped arguments or a `FinstackError` (kind `validation`)
/// when `delta_abs` is outside `(0, 1)` or `forward`, `vol`, or `expiry` is not finite and positive.
/// @param isCall - `true` for a call option, `false` for a put option.
/// @param deltaAbs - Forward delta magnitude as a decimal probability strictly in `(0, 1)`.
/// @param forward - Positive forward in the same units as the returned strike.
/// @param vol - Positive annualized Black volatility as a decimal.
/// @param expiry - Positive option expiry in years.
/// @returns Strike in the same units as `forward`.
/// @throws Error - Throws `invalid_type` for mistyped arguments or `validation` when an input is out of domain.
#[wasm_bindgen(js_name = deltaToStrike)]
pub fn delta_to_strike(
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
    #[wasm_bindgen(js_name = "deltaAbs", unchecked_param_type = "number")] delta_abs: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
) -> Result<f64, JsValue> {
    let is_call = js_bool(&is_call, "isCall")?;
    let delta_abs = js_f64(&delta_abs, "deltaAbs")?;
    let forward = js_f64(&forward, "forward")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    vol::delta_to_strike(is_call, delta_abs, forward, vol, expiry).map_err(to_js_err)
}

/// Convert strike to premium-unadjusted forward delta magnitude.
///
/// # Errors
///
/// Throws a `TypeError` for mistyped arguments or a `FinstackError` (kind `validation`)
/// when any numeric input is non-finite or non-positive.
/// @param isCall - `true` for a call option, `false` for a put option.
/// @param strike - Positive strike in the same units as `forward`.
/// @param forward - Positive forward in the same units as `strike`.
/// @param vol - Positive annualized Black volatility as a decimal.
/// @param expiry - Positive option expiry in years.
/// @returns Unsigned forward delta magnitude in `(0, 1)`.
/// @throws Error - Throws `invalid_type` for mistyped arguments or `validation` when an input is non-finite or non-positive.
#[wasm_bindgen(js_name = strikeToDelta)]
pub fn strike_to_delta(
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
) -> Result<f64, JsValue> {
    let is_call = js_bool(&is_call, "isCall")?;
    let strike = js_f64(&strike, "strike")?;
    let forward = js_f64(&forward, "forward")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    vol::strike_to_delta(is_call, strike, forward, vol, expiry).map_err(to_js_err)
}

/// SVI total implied variance `w(k)` at a log-moneyness.
///
/// Twin of the Rust and Python `SviParams.total_variance`.
/// @param params - `SviParams` object or JSON (`a`, `b`, `rho`, `m`, `sigma`).
/// @param k - Log-moneyness `ln(K / F)`.
/// @returns Total variance `sigma^2 * T` at `k`.
///
/// # Errors
///
/// Throws a `validation` error if `params` is malformed or fails validation.
#[wasm_bindgen(js_name = sviTotalVariance)]
pub fn svi_total_variance(
    #[wasm_bindgen(unchecked_param_type = "SviParams")] params: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] k: JsValue,
) -> Result<f64, JsValue> {
    let params: SviParams = from_js_json(&params, "params")?;
    Ok(params.total_variance(js_f64(&k, "k")?))
}

/// Durrleman's function `g(k)` of an SVI slice; the slice is free of butterfly
/// arbitrage where `g(k) >= 0`.
///
/// Twin of the Rust and Python `SviParams.durrleman_g`.
/// @param params - `SviParams` object or JSON (`a`, `b`, `rho`, `m`, `sigma`).
/// @param k - Log-moneyness `ln(K / F)`.
/// @returns `g(k)`; `-Infinity` where the total variance is not positive.
///
/// # Errors
///
/// Throws a `validation` error if `params` is malformed or fails validation.
#[wasm_bindgen(js_name = sviDurrlemanG)]
pub fn svi_durrleman_g(
    #[wasm_bindgen(unchecked_param_type = "SviParams")] params: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] k: JsValue,
) -> Result<f64, JsValue> {
    let params: SviParams = from_js_json(&params, "params")?;
    Ok(params.durrleman_g(js_f64(&k, "k")?))
}
