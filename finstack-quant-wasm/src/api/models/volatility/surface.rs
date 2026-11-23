//! Local volatility, arbitrage grid checks, and surface/cube materialization.

//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::api::core::surfaces::{
    JsFxDeltaVolSurface, JsVolCube, JsVolCubeExpirySlice, JsVolSurface,
};
use crate::utils::input::{from_js_json, js_f64, js_f64_matrix, js_f64_seq, js_opt_f64};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::arbitrage as model_arbitrage;
use finstack_quant_models::volatility::local_vol::LocalVolSurface;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Extract Dupire local volatility from an implied volatility surface.
///
/// Twin of the Rust and Python `LocalVolSurface.from_implied_vol`. The local
/// variance at each node of the implied grid is `(dw/dT) / g` in total
/// variance `w = sigma^2 T` and log-moneyness `k = ln(K / F_T)` (Gatheral
/// 2006, eq. 1.10), with the time derivative taken at fixed `k`.
/// @param surface - `core.VolSurface` handle of unshifted Black implied volatilities (decimals) on expiries in years and positive cash strikes, at least two expiries and three strikes.
/// @param forwards - Forward price of the underlying for each surface expiry, in strike units and in the order of `surface.expiries`; finite and positive. For a flat carry, `F(T) = S * exp((r - q) * T)`.
/// @returns The `LocalVolSurface` object: `expiries` in years, `strikes`, and `local_vols` as annualized decimals in row-major order (`local_vols[i * strikes.length + j]` for expiry `i`, strike `j`).
///
/// @example
/// ```typescript
/// import init, { core, models } from "finstack-quant-wasm";
/// await init();
/// const implied = new core.VolSurface(
///   "FLAT",
///   [0.5, 1.0],
///   [90, 100, 110],
///   [0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
///   { secondaryAxis: "strike", quoteType: "black_lognormal", interpolationMode: "vol" },
/// );
/// const local = models.volatility.localVolFromImpliedVol(implied, [100, 100]);
/// console.log(local.expiries, local.strikes); // [0.5, 1] [90, 100, 110]
/// console.log(local.local_vols.every((v) => Math.abs(v - 0.2) < 1e-12)); // true
/// ```
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if `surface` has fewer than two expiries or three
/// strikes, is not an unshifted Black strike surface, `forwards` has
/// the wrong length or a non-positive entry, or the surface has butterfly or
/// calendar arbitrage at a node (the message names the node).
#[wasm_bindgen(js_name = localVolFromImpliedVol, unchecked_return_type = "generated.models.LocalVolSurface")]
pub fn local_vol_from_implied_vol(
    surface: &JsVolSurface,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] forwards: JsValue,
) -> Result<JsValue, JsValue> {
    let forwards = js_f64_seq(&forwards, "forwards")?;
    let local = LocalVolSurface::from_implied_vol(&surface.inner, &forwards).map_err(to_js_err)?;
    to_js_value(&local)
}

/// Extract Dupire local volatility after Gaussian smoothing of the implied
/// volatilities along the strike axis.
///
/// Twin of the Rust and Python `LocalVolSurface.from_implied_vol_smoothed`.
/// Smoothing regularises the second strike derivative, the usual source of a
/// non-positive Dupire density on market-calibrated grids.
/// @param surface - `core.VolSurface` handle, as for `localVolFromImpliedVol`.
/// @param forwards - One forward per surface expiry, as for `localVolFromImpliedVol`.
/// @param sigmaStrikes - Standard deviation of the Gaussian kernel in strike (price) units, non-negative; zero disables smoothing.
/// @returns The `LocalVolSurface` object on the grid of `surface`.
///
/// @example
/// ```typescript
/// import init, { core, models } from "finstack-quant-wasm";
/// await init();
/// const implied = new core.VolSurface(
///   "FLAT",
///   [0.5, 1.0],
///   [90, 100, 110],
///   [0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
///   { secondaryAxis: "strike", quoteType: "black_lognormal", interpolationMode: "vol" },
/// );
/// const local = models.volatility.localVolFromImpliedVolSmoothed(implied, [100, 100], 5.0);
/// console.log(local.local_vols.length); // 6
/// ```
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if `sigmaStrikes` is negative or non-finite, or for any
/// error of `localVolFromImpliedVol` on the smoothed surface.
#[wasm_bindgen(js_name = localVolFromImpliedVolSmoothed, unchecked_return_type = "generated.models.LocalVolSurface")]
pub fn local_vol_from_implied_vol_smoothed(
    surface: &JsVolSurface,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] forwards: JsValue,
    #[wasm_bindgen(js_name = "sigmaStrikes", unchecked_param_type = "number")]
    sigma_strikes: JsValue,
) -> Result<JsValue, JsValue> {
    let forwards = js_f64_seq(&forwards, "forwards")?;
    let sigma_strikes = js_f64(&sigma_strikes, "sigmaStrikes")?;
    let local =
        LocalVolSurface::from_implied_vol_smoothed(&surface.inner, &forwards, sigma_strikes)
            .map_err(to_js_err)?;
    to_js_value(&local)
}

/// Evaluate a local volatility surface: bilinear inside its grid, flat
/// outside it.
///
/// Twin of the Rust and Python `LocalVolSurface.value`.
/// @param localVol - `LocalVolSurface` object or JSON (`expiries`, `strikes`, `local_vols`), validated on decode.
/// @param expiry - Time in years from the valuation date.
/// @param strike - Level of the underlying in price units.
/// @returns The local volatility as an annualized decimal.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const local = { expiries: [1, 2], strikes: [100, 200], local_vols: [0.1, 0.2, 0.3, 0.4] };
/// console.log(models.volatility.localVolValue(local, 1.5, 150)); // 0.25
/// console.log(models.volatility.localVolValue(local, 9, 50)); // 0.3
/// ```
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if `localVol` is malformed: an empty or unsorted axis,
/// a non-positive strike, a negative volatility or a value count that does
/// not match the grid.
#[wasm_bindgen(js_name = localVolValue)]
pub fn local_vol_value(
    #[wasm_bindgen(
        js_name = "localVol",
        unchecked_param_type = "generated.models.LocalVolSurface | string"
    )]
    local_vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
) -> Result<f64, JsValue> {
    let local_vol: LocalVolSurface = from_js_json(&local_vol, "localVol")?;
    Ok(local_vol.value(js_f64(&expiry, "expiry")?, js_f64(&strike, "strike")?))
}

fn grid_tolerance(tolerance: Option<&JsValue>) -> Result<f64, JsValue> {
    Ok(js_opt_f64(tolerance, "tolerance")?.unwrap_or(model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))
}

macro_rules! arbitrage_grid_check {
    (
        $(#[$doc:meta])*
        $name:ident as $js_name:ident -> $ret_ts:literal
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        $(#[$doc])*
        #[wasm_bindgen(js_name = $js_name, unchecked_return_type = $ret_ts)]
        pub fn $name(
            #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
            #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
            #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
            #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
            forward_prices: JsValue,
            #[wasm_bindgen(unchecked_optional_param_type = "number")] tolerance: Option<JsValue>,
        ) -> Result<JsValue, JsValue> {
            let result = model_arbitrage::$name(
                &js_f64_seq(&expiries, "expiries")?,
                &js_f64_seq(&strikes, "strikes")?,
                &js_f64_matrix(&vols, "vols")?,
                js_f64_seq(&forward_prices, "forwardPrices")?,
                grid_tolerance(tolerance.as_ref())?,
            )
            .map_err(to_js_err)?;
            to_js_value(&result)
        }
    };
}

arbitrage_grid_check!(
    /// Butterfly-arbitrage check on a strike by expiry volatility grid.
    /// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
    /// @param strikes - Strictly increasing strike grid shared by every row.
    /// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
    /// @param forwardPrices - One forward price to broadcast, or one per expiry.
    /// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
    /// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the grid or forwards are invalid or the
    /// tolerance is negative or non-finite.
    check_butterfly_grid as checkButterflyGrid -> "generated.models.ArbitrageViolation[]"
);

arbitrage_grid_check!(
    /// Calendar-spread arbitrage check on a strike by expiry volatility grid.
    /// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
    /// @param strikes - Strictly increasing strike grid shared by every row.
    /// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
    /// @param forwardPrices - One forward price to broadcast, or one per expiry.
    /// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
    /// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the grid or forwards are invalid or the
    /// tolerance is negative or non-finite.
    check_calendar_spread_grid as checkCalendarSpreadGrid -> "generated.models.ArbitrageViolation[]"
);

/// Local-volatility density check (Dupire denominator positivity) on a volatility grid.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forwardPrices - One forward price per expiry.
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid is invalid or the forwards do not
/// match the expiries.
#[wasm_bindgen(js_name = checkLocalVolDensityGrid, unchecked_return_type = "generated.models.ArbitrageViolation[]")]
pub fn check_local_vol_density_grid(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
    #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
    forward_prices: JsValue,
) -> Result<JsValue, JsValue> {
    let violations = model_arbitrage::check_local_vol_density_grid(
        &js_f64_seq(&expiries, "expiries")?,
        &js_f64_seq(&strikes, "strikes")?,
        &js_f64_matrix(&vols, "vols")?,
        js_f64_seq(&forward_prices, "forwardPrices")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&violations)
}

arbitrage_grid_check!(
    /// Run every static-arbitrage check on a strike by expiry volatility grid.
    /// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
    /// @param strikes - Strictly increasing strike grid shared by every row.
    /// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
    /// @param forwardPrices - One forward price to broadcast, or one per expiry.
    /// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
    /// @returns The `ArbitrageReport` object (`vol_surface_id`, `violations`, `passed`, `counts_by_type`, `counts_by_severity`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the grid or forwards are invalid or the
    /// tolerance is negative or non-finite.
    check_surface_grid as checkSurfaceGrid -> "generated.models.ArbitrageReport"
);

/// Run the configured butterfly, calendar-spread and local-vol density checks on a volatility surface.
/// @param surface - `core.VolSurface` handle of unshifted Black lognormal volatilities on a strike axis (expiries in years, strikes in price units); it is read, not modified.
/// @param config - `ArbitrageCheckConfig` as an object or JSON text. Every field is optional and an omitted one takes its Rust default: `check_butterfly`, `check_calendar_spread` and `check_local_vol_density` (default `true`); `forward_prices` (one finite positive forward to broadcast or one per expiry, required while any check is enabled); `tolerance` (non-negative, total-variance units, default `DEFAULT_ARBITRAGE_TOLERANCE` = 1e-10); `min_severity` (`"negligible"` default, `"minor"`, `"major"`, `"critical"`). Unknown fields are rejected.
/// @returns The `ArbitrageReport` object (`vol_surface_id`, `violations` sorted most severe first, `passed`, `counts_by_type`, `counts_by_severity`).
///
/// # Errors
///
/// Throws a `validation` error if `config` has an unknown or mistyped field,
/// the forwards are missing while a check is enabled, non-finite,
/// non-positive or of the wrong length, the tolerance is negative or
/// non-finite, or the surface is normal-quoted, shifted-Black or on a tenor
/// axis.
#[wasm_bindgen(js_name = checkSurface, unchecked_return_type = "generated.models.ArbitrageReport")]
pub fn check_surface(
    surface: &JsVolSurface,
    #[wasm_bindgen(unchecked_param_type = "generated.models.ArbitrageCheckConfig | string")] config: JsValue,
) -> Result<JsValue, JsValue> {
    let config = from_js_json::<model_arbitrage::ArbitrageCheckConfig>(&config, "config")?;
    let report = model_arbitrage::check_surface(&surface.inner, &config).map_err(to_js_err)?;
    to_js_value(&report)
}

/// Interpolate a volatility surface; coordinates outside the grid are rejected.
/// @param surface - `core.VolSurface` handle, such as a `materializeCubeTenorSlice` or `materializeFxDeltaSurface` result.
/// @param expiry - Option expiry in years, within the surface grid.
/// @param strike - Strike in the surface's own units, within the surface grid.
/// @returns The interpolated volatility, as a decimal.
///
/// # Errors
///
/// Throws a `validation` error if a coordinate is outside the grid.
#[wasm_bindgen(js_name = getSurfaceVol)]
pub fn get_surface_vol(
    surface: &JsVolSurface,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
) -> Result<f64, JsValue> {
    vol::get_surface_vol(
        &surface.inner,
        js_f64(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
    )
    .map_err(to_js_err)
}

/// Interpolate a volatility surface with flat clamping to the grid edges.
/// @param surface - `core.VolSurface` handle.
/// @param expiry - Option expiry in years; clamped to the surface grid.
/// @param strike - Strike in the surface's own units; clamped to the surface grid.
/// @returns The interpolated volatility, as a decimal; `NaN` for non-finite coordinates.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `expiry` or `strike`
/// is not a number.
#[wasm_bindgen(js_name = getSurfaceVolClamped)]
pub fn get_surface_vol_clamped(
    surface: &JsVolSurface,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
) -> Result<f64, JsValue> {
    Ok(vol::get_surface_vol_clamped(
        &surface.inner,
        js_f64(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
    ))
}

/// Materialize the volatility slice of a SABR cube at one underlying tenor.
/// @param cube - `core.VolCube` handle.
/// @param tenor - Underlying tenor in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @param convention - `"lognormal"` for Black volatilities or `"normal"` for
/// absolute Bachelier volatilities; shifted-lognormal is rejected.
/// @param interpolationMode - Output expiry interpolation on the sampled sigma grid.
/// @returns The expiry by strike `core.VolSurface` handle in `convention`.
///
/// # Errors
///
/// Throws a `validation` error if the tenor is outside the cube, the strikes
/// are invalid, a SABR evaluation fails, or `convention` is shifted-lognormal.
#[wasm_bindgen(js_name = materializeCubeTenorSlice)]
pub fn materialize_cube_tenor_slice(
    cube: &JsVolCube,
    #[wasm_bindgen(unchecked_param_type = "number")] tenor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "VolatilityConvention")] convention: JsValue,
    #[wasm_bindgen(
        js_name = "interpolationMode",
        unchecked_param_type = "\"vol\" | \"total_variance\""
    )]
    interpolation_mode: JsValue,
) -> Result<JsVolSurface, JsValue> {
    let surface = vol::materialize_cube_tenor_slice(
        &cube.inner,
        js_f64(&tenor, "tenor")?,
        &js_f64_seq(&strikes, "strikes")?,
        js_wire(&convention, "convention")?,
        js_wire(&interpolation_mode, "interpolationMode")?,
    )
    .map_err(to_js_err)?;
    Ok(JsVolSurface::from_inner(Arc::new(surface)))
}

/// Materialize the volatility slice of a SABR cube at one option expiry.
/// @param cube - `core.VolCube` handle.
/// @param expiry - Option expiry in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @param convention - `"lognormal"` for Black volatilities or `"normal"` for
/// absolute Bachelier volatilities; shifted-lognormal is rejected.
/// @returns The tenor by strike `core.VolCubeExpirySlice` handle in `convention`.
///
/// # Errors
///
/// Throws a `validation` error if the expiry is outside the cube, the strikes
/// are invalid, a SABR evaluation fails, or `convention` is shifted-lognormal.
#[wasm_bindgen(js_name = materializeCubeExpirySlice)]
pub fn materialize_cube_expiry_slice(
    cube: &JsVolCube,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "VolatilityConvention")] convention: JsValue,
) -> Result<JsVolCubeExpirySlice, JsValue> {
    let surface = vol::materialize_cube_expiry_slice(
        &cube.inner,
        js_f64(&expiry, "expiry")?,
        &js_f64_seq(&strikes, "strikes")?,
        js_wire(&convention, "convention")?,
    )
    .map_err(to_js_err)?;
    Ok(JsVolCubeExpirySlice {
        inner: Arc::new(surface),
    })
}

/// Interpolate a fixed-expiry cube slice; coordinates outside the grid are rejected.
/// @param slice - `core.VolCubeExpirySlice` handle, such as a `materializeCubeExpirySlice` result.
/// @param tenor - Underlying tenor in years, within the slice's tenor axis.
/// @param strike - Strike in forward-rate units, within the slice's strike axis.
/// @returns The bilinearly interpolated volatility in the slice's quote convention.
///
/// # Errors
///
/// Throws a `validation` error if a coordinate is non-finite or outside the
/// grid.
#[wasm_bindgen(js_name = getCubeExpirySliceVol)]
pub fn get_cube_expiry_slice_vol(
    slice: &JsVolCubeExpirySlice,
    #[wasm_bindgen(unchecked_param_type = "number")] tenor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
) -> Result<f64, JsValue> {
    vol::get_cube_expiry_slice_vol(
        &slice.inner,
        js_f64(&tenor, "tenor")?,
        js_f64(&strike, "strike")?,
    )
    .map_err(to_js_err)
}

/// Interpolate a fixed-expiry cube slice with flat clamping to the grid edges.
/// @param slice - `core.VolCubeExpirySlice` handle.
/// @param tenor - Underlying tenor in years; clamped to the slice's tenor axis.
/// @param strike - Strike in forward-rate units; clamped to the slice's strike axis.
/// @returns The interpolated volatility in the slice's quote convention; `NaN` for non-finite coordinates.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `tenor` or `strike`
/// is not a number.
#[wasm_bindgen(js_name = getCubeExpirySliceVolClamped)]
pub fn get_cube_expiry_slice_vol_clamped(
    slice: &JsVolCubeExpirySlice,
    #[wasm_bindgen(unchecked_param_type = "number")] tenor: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
) -> Result<f64, JsValue> {
    Ok(vol::get_cube_expiry_slice_vol_clamped(
        &slice.inner,
        js_f64(&tenor, "tenor")?,
        js_f64(&strike, "strike")?,
    ))
}

/// Materialize an FX delta-quoted surface as an expiry by strike volatility surface.
///
/// # Errors
///
/// Throws a `TypeError` for mistyped arguments or a `validation` error if
/// `delivery_forwards` length mismatches `surface.expiries`, any forward is
/// non-finite or non-positive, or delta-to-strike conversion fails.
/// @param surface - `core.FxDeltaVolSurface` handle.
/// @param deliveryForwards - Outright FX delivery forwards (quote per unit of base), one positive value per expiry.
/// @param interpolationMode - Output grid interpolation in implied volatility ("vol") or total variance ("total_variance").
/// @returns The strike-space `core.VolSurface` handle.
/// @throws Error - Throws `invalid_type` for mistyped arguments or `validation` when forwards mismatch length, are non-positive, wing conversion fails, or the interpolation mode is invalid.
#[wasm_bindgen(js_name = materializeFxDeltaSurface)]
pub fn materialize_fx_delta_surface(
    surface: &JsFxDeltaVolSurface,
    #[wasm_bindgen(js_name = "deliveryForwards", unchecked_param_type = "NumericArray")]
    delivery_forwards: JsValue,
    #[wasm_bindgen(
        js_name = "interpolationMode",
        unchecked_param_type = "'vol' | 'total_variance'"
    )]
    interpolation_mode: JsValue,
) -> Result<JsVolSurface, JsValue> {
    let delivery_forwards = js_f64_seq(&delivery_forwards, "deliveryForwards")?;
    let mode = crate::utils::wire::parse_label(&crate::utils::input::js_string(
        &interpolation_mode,
        "interpolationMode",
    )?)?;
    let materialized = vol::materialize_fx_delta_surface(&surface.inner, &delivery_forwards, mode)
        .map_err(to_js_err)?;
    Ok(JsVolSurface::from_inner(Arc::new(materialized)))
}
