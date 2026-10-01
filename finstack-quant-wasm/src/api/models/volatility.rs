//! Product-independent volatility model and evaluator bindings for WASM.
//!
//! Exposes `SabrParameters`, `SabrModel`, `SabrSmile`, and `SabrCalibrator` to
//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::api::core::surfaces::{JsFxDeltaVolSurface, JsVolCube};
use crate::utils::input::{
    from_js_json, invalid_type, js_bool, js_f64, js_f64_matrix, js_f64_seq, js_opt_f64, js_uint,
};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::arbitrage as model_arbitrage;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrModel, SabrParameters, SabrShift, SabrSmile,
};
use finstack_quant_models::volatility::svi::SviParams;
use wasm_bindgen::prelude::*;

/// SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
#[wasm_bindgen(js_name = SabrParameters)]
pub struct JsSabrParameters {
    pub(crate) inner: SabrParameters,
}

#[wasm_bindgen(js_class = SabrParameters)]
impl JsSabrParameters {
    /// Create SABR parameters from alpha, beta, nu, rho, and optional shift.
    /// @param alpha - Positive SABR initial volatility scale parameter.
    /// @param beta - SABR CEV elasticity parameter from 0 through 1.
    /// @param nu - Positive SABR volatility-of-volatility parameter.
    /// @param rho - Instantaneous correlation between the asset and variance shocks.
    /// @param shift - Additive SABR rate shift applied to forward and strike before modelling.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `alpha` is not finite and positive,
    /// `beta` is outside `[0, 1]`, `nu` is negative or non-finite, `rho` is
    /// outside `[-1, 1]`, or a supplied `shift` is not finite and positive.
    #[wasm_bindgen(constructor)]
    pub fn new(
        alpha: JsValue,
        beta: JsValue,
        nu: JsValue,
        rho: JsValue,
        shift: Option<JsValue>,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let beta = js_f64(&beta, "beta")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        let shift = js_opt_f64(shift.as_ref(), "shift")?;
        let inner = match shift {
            Some(s) => SabrParameters::new_with_shift(alpha, beta, nu, rho, s),
            None => SabrParameters::new(alpha, beta, nu, rho),
        }
        .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Default SABR parameters for equity underlyings.
    #[wasm_bindgen(js_name = equityDefault)]
    pub fn equity_default() -> JsSabrParameters {
        Self {
            inner: SabrParameters::equity_default(),
        }
    }

    /// Default SABR parameters for rates underlyings.
    #[wasm_bindgen(js_name = ratesDefault)]
    pub fn rates_default() -> JsSabrParameters {
        Self {
            inner: SabrParameters::rates_default(),
        }
    }

    /// SABR `alpha` (ATM volatility level).
    #[wasm_bindgen(getter)]
    pub fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    /// SABR `beta` (backbone exponent).
    #[wasm_bindgen(getter)]
    pub fn beta(&self) -> f64 {
        self.inner.beta
    }

    /// SABR `nu` (vol-of-vol).
    #[wasm_bindgen(getter)]
    pub fn nu(&self) -> f64 {
        self.inner.nu
    }

    /// SABR `rho` (spot/vol correlation).
    #[wasm_bindgen(getter)]
    pub fn rho(&self) -> f64 {
        self.inner.rho
    }

    /// Displacement applied for shifted SABR, if any.
    #[wasm_bindgen(getter)]
    pub fn shift(&self) -> Option<f64> {
        self.inner.shift
    }

    /// Whether a displacement (shift) is configured.
    #[wasm_bindgen(js_name = isShifted)]
    pub fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
    }
}

impl JsSabrParameters {
    pub(crate) fn from_inner(inner: SabrParameters) -> Self {
        Self { inner }
    }

    fn clone_inner(&self) -> SabrParameters {
        self.inner.clone()
    }
}

/// Hagan-2002 SABR volatility model.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
#[wasm_bindgen(js_name = SabrModel)]
pub struct JsSabrModel {
    inner: SabrModel,
}

#[wasm_bindgen(js_class = SabrModel)]
impl JsSabrModel {
    /// Create a Hagan-2002 SABR model from the supplied parameters.
    /// @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
    #[wasm_bindgen(constructor)]
    pub fn new(params: &JsSabrParameters) -> JsSabrModel {
        Self {
            inner: SabrModel::new(params.clone_inner()),
        }
    }

    /// Implied volatility: normal decimal rate for beta=0, Black decimal volatility otherwise.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param strike - Option strike price in the same price units as the underlying.
    /// @param t - Time from the curve base date in years.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `t` is not positive, the forward or
    /// strike lies outside the selected shifted or unshifted SABR domain, or
    /// the Hagan expansion produces an undefined or non-finite volatility.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        forward: JsValue,
        strike: JsValue,
        t: JsValue,
    ) -> Result<f64, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strike = js_f64(&strike, "strike")?;
        let t = js_f64(&t, "t")?;
        self.inner
            .implied_volatility(forward, strike, t)
            .map_err(to_js_err)
    }

    /// Parameters used by this model.
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> JsSabrParameters {
        JsSabrParameters::from_inner(self.inner.parameters().clone())
    }

    /// Whether the parameterization admits negative forwards.
    #[wasm_bindgen(js_name = supportsNegativeRates)]
    pub fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }
}

/// Volatility smile generator for a fixed `(forward, t)` pair.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
#[wasm_bindgen(js_name = SabrSmile)]
pub struct JsSabrSmile {
    inner: SabrSmile,
}

#[wasm_bindgen(js_class = SabrSmile)]
impl JsSabrSmile {
    /// Create a SABR smile for a fixed forward and expiry.
    /// @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param t - Time from the curve base date in years.
    #[wasm_bindgen(constructor)]
    pub fn new(
        params: &JsSabrParameters,
        forward: JsValue,
        t: JsValue,
    ) -> Result<JsSabrSmile, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let t = js_f64(&t, "t")?;
        let model = SabrModel::new(params.clone_inner());
        Ok(Self {
            inner: SabrSmile::new(model, forward, t),
        })
    }

    /// At-the-money implied volatility.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry or effective forward
    /// is outside the model domain, or the ATM calculation produces an invalid
    /// volatility.
    #[wasm_bindgen(js_name = atmVol)]
    pub fn atm_vol(&self) -> Result<f64, JsValue> {
        self.inner.atm_vol().map_err(to_js_err)
    }

    /// Implied volatility: normal decimal rate for beta=0, Black decimal volatility otherwise.
    /// @param strike - Option strike price in the same price units as the underlying.
    /// @returns Normal (Bachelier) vol in absolute rate units when beta < 1e-4,
    /// Black decimal vol otherwise.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry, forward, or
    /// requested `strike` is outside the model domain or the Hagan expansion
    /// fails.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(&self, strike: JsValue) -> Result<f64, JsValue> {
        let strike = js_f64(&strike, "strike")?;
        self.inner.implied_vol(strike).map_err(to_js_err)
    }

    /// Implied volatilities for a strike grid.
    /// @param strikes - Option strikes at which to evaluate the SABR volatility smile.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry or forward, or any
    /// supplied strike, is outside the model domain, or the Hagan expansion
    /// produces an invalid volatility.
    #[wasm_bindgen(js_name = generateSmile)]
    pub fn generate_smile(&self, strikes: JsValue) -> Result<Box<[f64]>, JsValue> {
        let strikes = js_f64_seq(&strikes, "strikes")?;
        self.inner
            .generate_smile(&strikes)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Butterfly + strike-monotonicity static-arbitrage check of the smile.
    ///
    /// Returns the Rust `ArbitrageValidationResult` serde object:
    /// `arbitrage_free`, `butterfly_violations` and `monotonicity_violations`.
    /// @param strikes - Ascending option strikes used to test the smile for static arbitrage.
    /// @param r - Continuously compounded risk-free rate (decimal) that discounts the
    /// forward-based Black call prices compared against the 1e-6 tolerance.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if volatility generation fails for the
    /// stored smile and supplied strikes, or the result cannot be converted to
    /// a JavaScript value.
    #[wasm_bindgen(js_name = validateNoArbitrage)]
    pub fn validate_no_arbitrage(&self, strikes: JsValue, r: JsValue) -> Result<JsValue, JsValue> {
        let r = js_f64(&r, "r")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }
}

/// SABR calibrator (Levenberg-Marquardt with beta fixed).
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
#[wasm_bindgen(js_name = SabrCalibrator)]
pub struct JsSabrCalibrator {
    inner: SabrCalibrator,
}

#[wasm_bindgen(js_class = SabrCalibrator)]
impl JsSabrCalibrator {
    /// Create a Levenberg-Marquardt SABR calibrator with default tolerances.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsSabrCalibrator {
        Self {
            inner: SabrCalibrator::new(),
        }
    }

    /// Calibrator preset with tighter convergence tolerances.
    #[wasm_bindgen(js_name = highPrecision)]
    pub fn high_precision() -> JsSabrCalibrator {
        Self {
            inner: SabrCalibrator::high_precision(),
        }
    }

    /// Return a copy of this calibrator with an overridden maximum relative
    /// quote error, preserving all other settings (e.g. the iteration cap from
    /// `highPrecision`).
    /// @param tolerance - Positive finite maximum relative error of any final volatility quote; 1e-4 permits 0.01% of each quote. Invalid settings or a fit outside this budget throw during calibration.
    #[wasm_bindgen(js_name = withTolerance)]
    pub fn with_tolerance(&self, tolerance: JsValue) -> Result<JsSabrCalibrator, JsValue> {
        let tolerance = js_f64(&tolerance, "tolerance")?;
        Ok(Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        })
    }

    /// Return a copy of this calibrator with an overridden iteration cap,
    /// preserving all other settings.
    /// @param max_iterations - Positive cap on solver iterations before a
    /// non-convergence error; pair a tight tolerance with a larger budget.
    #[wasm_bindgen(js_name = withMaxIterations)]
    pub fn with_max_iterations(
        &self,
        max_iterations: JsValue,
    ) -> Result<JsSabrCalibrator, JsValue> {
        let max_iterations: usize = js_uint(&max_iterations, "maxIterations")?;
        Ok(Self {
            inner: self.inner.clone().with_max_iterations(max_iterations),
        })
    }

    /// Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param strikes - Option strikes aligned one-for-one with market_vols.
    /// @param market_vols - Market-implied annualized volatilities aligned one-for-one with strikes.
    /// @param t - Time from the curve base date in years.
    /// @param beta - SABR CEV elasticity parameter held fixed during calibration.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the strike and volatility lengths
    /// differ, the quote arrays are empty, the SABR inputs or fitted parameters
    /// are invalid, or the calibration solver does not converge.
    pub fn calibrate(
        &self,
        forward: JsValue,
        strikes: JsValue,
        market_vols: JsValue,
        t: JsValue,
        beta: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let market_vols = js_f64_seq(&market_vols, "marketVols")?;
        let t = js_f64(&t, "t")?;
        let beta = js_f64(&beta, "beta")?;
        self.inner
            .calibrate(forward, &strikes, &market_vols, t, beta)
            .map(JsSabrParameters::from_inner)
            .map_err(to_js_err)
    }

    /// Return a copy of this calibrator with an overridden displacement
    /// policy, preserving all other settings.
    /// @param shift - `null`/`undefined` fits the quotes as-is; a number is a
    /// fixed additive shift in the forward's units (decimal rate or price);
    /// `"auto"` picks the smallest standardized shift (1-4%) that leaves 10bp
    /// of headroom above the most negative forward or strike, or none when
    /// every input is non-negative. The shift used is stored on the fitted
    /// `SabrParameters`.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (`kind: "validation"`) for a string other than
    /// `"auto"` (parsed by the Rust `SabrShift`), and a `TypeError`
    /// (`kind: "invalid_type"`) for any other non-null, non-number value such
    /// as a boolean.
    #[wasm_bindgen(js_name = withShift)]
    pub fn with_shift(&self, shift: JsValue) -> Result<JsSabrCalibrator, JsValue> {
        // Host-union conversion only: null → None, number → Fixed, string →
        // the Rust `SabrShift` keyword parser.
        let policy = if shift.is_null() || shift.is_undefined() {
            SabrShift::None
        } else if let Some(value) = shift.as_f64() {
            SabrShift::Fixed(value)
        } else if let Some(text) = shift.as_string() {
            text.parse::<SabrShift>().map_err(to_js_err)?
        } else {
            return Err(invalid_type(
                "shift",
                "expected null, a number, or the string \"auto\"",
            ));
        };
        Ok(Self {
            inner: self.inner.clone().with_shift(policy),
        })
    }

    /// Return a copy of this calibrator with exact ATM pinning enabled or
    /// disabled, preserving all other settings.
    /// @param atm_pinning - When `true`, alpha is solved analytically so the
    /// model reproduces the ATM volatility interpolated from the quotes
    /// exactly, and only nu and rho are fitted to the smile.
    #[wasm_bindgen(js_name = withAtmPinning)]
    pub fn with_atm_pinning(&self, atm_pinning: JsValue) -> Result<JsSabrCalibrator, JsValue> {
        let atm_pinning = js_bool(&atm_pinning, "atmPinning")?;
        Ok(Self {
            inner: self.inner.clone().with_atm_pinning(atm_pinning),
        })
    }
}

impl Default for JsSabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.
///
/// Prices are equated at the money (strike = forward) and the target vol is
/// solved deterministically.
///
/// # Arguments
///
/// * `vol` - Input volatility in the source convention: decimal Black vol for
///   `"lognormal"` / shifted-lognormal, absolute vol in the forward's rate units
///   for `"normal"`. Must be positive.
/// * `from_convention` - `"normal"`, `"lognormal"`, or
///   `{"shifted_lognormal": {"shift": s}}` (serde form of `VolatilityConvention`).
/// * `to_convention` - Target convention in the same encoding.
/// * `forward_rate` - ATM forward rate or price; must satisfy the target
///   convention's domain.
/// * `time_to_expiry` - Time to expiry in years (non-negative).
///
/// # Errors
///
/// Throws a JavaScript exception if a convention cannot be decoded, an input
/// is outside its domain, or the price-matching solver fails to converge.
#[wasm_bindgen(js_name = convertAtmVolatility)]
pub fn convert_atm_volatility(
    vol: JsValue,
    from_convention: JsValue,
    to_convention: JsValue,
    forward_rate: JsValue,
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
/// # Arguments
///
/// * `strikes` - Positive strikes (at least five).
/// * `vols` - Black implied vols (decimal) aligned one-for-one with `strikes`.
/// * `forward` - Positive forward at `expiry`.
/// * `expiry` - Positive time to expiry in years.
///
/// # Errors
///
/// Throws a JavaScript exception if lengths differ, fewer than five quotes are
/// supplied, an input is outside its domain, the optimizer fails to converge,
/// or the fit violates the SVI no-arbitrage conditions.
#[wasm_bindgen(js_name = calibrateSvi)]
pub fn calibrate_svi(
    strikes: JsValue,
    vols: JsValue,
    forward: JsValue,
    expiry: JsValue,
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
/// # Arguments
///
/// * `params` - SVI parameter object `{a, b, rho, m, sigma}` (validated on decode).
/// * `k` - Log-moneyness `ln(K / F)`.
/// * `t` - Positive time to expiry in years.
///
/// # Errors
///
/// Throws a JavaScript exception if `params` fails validation, `t` is not
/// positive, or the total variance at `k` is negative.
#[wasm_bindgen(js_name = sviImpliedVol)]
pub fn svi_implied_vol(params: JsValue, k: JsValue, t: JsValue) -> Result<f64, JsValue> {
    let k = js_f64(&k, "k")?;
    let t = js_f64(&t, "t")?;
    let params: finstack_quant_models::volatility::svi::SviParams =
        from_js_json(&params, "params")?;
    params.implied_vol(k, t).map_err(to_js_err)
}

/// Evaluate Black/lognormal volatility from a core SABR cube.
///
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Positive option expiry in years within the cube grid.
/// * `tenor` - Positive underlying tenor in years within the cube grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
///
/// # Errors
///
/// Throws a JavaScript exception for out-of-grid coordinates or invalid SABR
/// model inputs.
#[wasm_bindgen(js_name = getCubeVol)]
pub fn get_cube_vol(
    cube: &JsVolCube,
    expiry: JsValue,
    tenor: JsValue,
    strike: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    vol::get_cube_vol(&cube.inner, expiry, tenor, strike).map_err(to_js_err)
}

/// Evaluate Black/lognormal cube volatility with flat coordinate clamping.
///
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Finite option expiry in years; clamped to the stored grid.
/// * `tenor` - Finite underlying tenor in years; clamped to the stored grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
#[wasm_bindgen(js_name = getCubeVolClamped)]
pub fn get_cube_vol_clamped(
    cube: &JsVolCube,
    expiry: JsValue,
    tenor: JsValue,
    strike: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    Ok(vol::get_cube_vol_clamped(
        &cube.inner,
        expiry,
        tenor,
        strike,
    ))
}

/// Evaluate normal/Bachelier volatility from a core SABR cube.
///
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Positive option expiry in years within the cube grid.
/// * `tenor` - Positive underlying tenor in years within the cube grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
///
/// # Errors
///
/// Throws a JavaScript exception for out-of-grid coordinates, an invalid
/// shifted-SABR domain, or a failed normal-volatility expansion.
#[wasm_bindgen(js_name = getCubeNormalVol)]
pub fn get_cube_normal_vol(
    cube: &JsVolCube,
    expiry: JsValue,
    tenor: JsValue,
    strike: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    vol::get_cube_normal_vol(&cube.inner, expiry, tenor, strike).map_err(to_js_err)
}

/// Evaluate normal/Bachelier cube volatility with coordinate clamping.
///
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Finite option expiry in years; clamped to the stored grid.
/// * `tenor` - Finite underlying tenor in years; clamped to the stored grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
#[wasm_bindgen(js_name = getCubeNormalVolClamped)]
pub fn get_cube_normal_vol_clamped(
    cube: &JsVolCube,
    expiry: JsValue,
    tenor: JsValue,
    strike: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let tenor = js_f64(&tenor, "tenor")?;
    let strike = js_f64(&strike, "strike")?;
    Ok(vol::get_cube_normal_vol_clamped(
        &cube.inner,
        expiry,
        tenor,
        strike,
    ))
}

/// Return ATM, 25-delta put, and 25-delta call vols at a stored FX expiry.
///
/// # Arguments
///
/// * `surface` - Structurally validated data-only FX delta surface.
/// * `expiry_index` - Zero-based stored expiry index.
///
/// # Errors
///
/// Throws a JavaScript exception when `expiry_index` is outside the surface.
#[wasm_bindgen(js_name = getFxDeltaPillarVols)]
pub fn get_fx_delta_pillar_vols(
    surface: &JsFxDeltaVolSurface,
    expiry_index: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let expiry_index: usize = js_uint(&expiry_index, "expiryIndex")?;
    vol::get_fx_delta_pillar_vols(&surface.inner, expiry_index)
        .map(|(atm, put, call)| Box::new([atm, put, call]) as Box<[f64]>)
        .map_err(to_js_err)
}

/// Evaluate an FX delta-quoted surface at an expiry, strike, and forward.
///
/// # Arguments
///
/// * `surface` - Structurally validated data-only FX delta surface.
/// * `expiry` - Positive option expiry in years.
/// * `strike` - Positive strike in the FX quote currency.
/// * `forward` - Positive FX forward in quote currency per base currency.
///
/// # Errors
///
/// Throws a JavaScript exception for invalid coordinates or a non-positive
/// reconstructed wing volatility.
#[wasm_bindgen(js_name = getFxDeltaVol)]
pub fn get_fx_delta_vol(
    surface: &JsFxDeltaVolSurface,
    expiry: JsValue,
    strike: JsValue,
    forward: JsValue,
) -> Result<f64, JsValue> {
    let expiry = js_f64(&expiry, "expiry")?;
    let strike = js_f64(&strike, "strike")?;
    let forward = js_f64(&forward, "forward")?;
    vol::get_fx_delta_vol(&surface.inner, expiry, strike, forward).map_err(to_js_err)
}

/// Convert premium-unadjusted forward delta to strike.
///
/// # Arguments
///
/// * `delta` - Forward call delta as a decimal probability in `(0, 1)`.
/// * `forward` - Positive forward in the same units as the returned strike.
/// * `vol` - Positive annualized Black volatility as a decimal.
/// * `expiry` - Positive option expiry in years.
#[wasm_bindgen(js_name = deltaToStrike)]
pub fn delta_to_strike(
    delta: JsValue,
    forward: JsValue,
    vol: JsValue,
    expiry: JsValue,
) -> Result<f64, JsValue> {
    let delta = js_f64(&delta, "delta")?;
    let forward = js_f64(&forward, "forward")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    Ok(vol::delta_to_strike(delta, forward, vol, expiry))
}

/// Convert strike to premium-unadjusted forward call delta.
///
/// # Arguments
///
/// * `strike` - Positive strike in the same units as `forward`.
/// * `forward` - Positive forward in the same units as `strike`.
/// * `vol` - Positive annualized Black volatility as a decimal.
/// * `expiry` - Positive option expiry in years.
#[wasm_bindgen(js_name = strikeToDelta)]
pub fn strike_to_delta(
    strike: JsValue,
    forward: JsValue,
    vol: JsValue,
    expiry: JsValue,
) -> Result<f64, JsValue> {
    let strike = js_f64(&strike, "strike")?;
    let forward = js_f64(&forward, "forward")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    Ok(vol::strike_to_delta(strike, forward, vol, expiry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabr_params_equity_default_roundtrip() {
        let p = JsSabrParameters::equity_default();
        assert!((p.alpha() - 0.20).abs() < 1e-12);
        assert!((p.beta() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn sabr_auto_shift_fits_negative_rate_smile() {
        // Native tests cannot construct `JsValue` strings or Debug a `JsValue`
        // error: both abort off wasm32. Drive the same `"auto"` policy through
        // the Rust keyword parser the binding calls and the domain calibrator.
        // Synthetic quotes use the documented 2% ladder rung
        // (`-min(strike)+10bp = 1.6%` rounds up to 2%), matching the JS facade
        // and Python bindings.
        let policy = "auto".parse::<SabrShift>().expect("auto shift policy");
        let p = SabrParameters::new_with_shift(0.05, 0.5, 0.4, -0.1, 0.02).expect("params");
        let forward = -0.005;
        let strikes = vec![-0.015, -0.01, -0.005, 0.0, 0.005];
        let vols = SabrSmile::new(SabrModel::new(p), forward, 1.0)
            .generate_smile(&strikes)
            .expect("smile");

        let fitted = SabrCalibrator::new()
            .with_shift(policy)
            .calibrate(forward, &strikes, &vols, 1.0, 0.5)
            .expect("auto-shift calibrate");
        let shift = fitted
            .shift()
            .expect("negative-rate fit must carry a shift");
        assert!(shift > 0.0);
        assert!(fitted.is_shifted());
        assert!((shift - 0.02).abs() < 1e-12);
    }
}

#[wasm_bindgen(js_class = SabrSmile)]
impl JsSabrSmile {
    /// Forward price or rate the smile is built around.
    #[wasm_bindgen(getter)]
    pub fn forward(&self) -> f64 {
        self.inner.forward()
    }

    /// Time to expiry of the smile, in years.
    #[wasm_bindgen(getter)]
    pub fn t(&self) -> f64 {
        self.inner.time_to_expiry()
    }
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
pub fn svi_total_variance(params: JsValue, k: JsValue) -> Result<f64, JsValue> {
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
pub fn svi_durrleman_g(params: JsValue, k: JsValue) -> Result<f64, JsValue> {
    let params: SviParams = from_js_json(&params, "params")?;
    Ok(params.durrleman_g(js_f64(&k, "k")?))
}

fn grid_tolerance(tolerance: Option<&JsValue>) -> Result<f64, JsValue> {
    Ok(js_opt_f64(tolerance, "tolerance")?.unwrap_or(model_arbitrage::DEFAULT_GRID_TOLERANCE))
}

/// Butterfly-arbitrage check on a strike by expiry volatility grid.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forward_prices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_GRID_TOLERANCE` (1e-6).
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkButterflyGrid)]
pub fn check_butterfly_grid(
    strikes: JsValue,
    expiries: JsValue,
    vols: JsValue,
    forward_prices: JsValue,
    tolerance: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let violations = model_arbitrage::check_butterfly_grid(
        &js_f64_seq(&strikes, "strikes")?,
        &js_f64_seq(&expiries, "expiries")?,
        &js_f64_matrix(&vols, "vols")?,
        js_f64_seq(&forward_prices, "forwardPrices")?,
        grid_tolerance(tolerance.as_ref())?,
    )
    .map_err(to_js_err)?;
    to_js_value(&violations)
}

/// Calendar-spread arbitrage check on a strike by expiry volatility grid.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forward_prices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_GRID_TOLERANCE` (1e-6).
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkCalendarSpreadGrid)]
pub fn check_calendar_spread_grid(
    strikes: JsValue,
    expiries: JsValue,
    vols: JsValue,
    forward_prices: JsValue,
    tolerance: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let violations = model_arbitrage::check_calendar_spread_grid(
        &js_f64_seq(&strikes, "strikes")?,
        &js_f64_seq(&expiries, "expiries")?,
        &js_f64_matrix(&vols, "vols")?,
        js_f64_seq(&forward_prices, "forwardPrices")?,
        grid_tolerance(tolerance.as_ref())?,
    )
    .map_err(to_js_err)?;
    to_js_value(&violations)
}

/// Local-volatility density check (Dupire denominator positivity) on a volatility grid.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forward_prices - One forward price per expiry.
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid is invalid or the forwards do not
/// match the expiries.
#[wasm_bindgen(js_name = checkLocalVolDensityGrid)]
pub fn check_local_vol_density_grid(
    strikes: JsValue,
    expiries: JsValue,
    vols: JsValue,
    forward_prices: JsValue,
) -> Result<JsValue, JsValue> {
    let violations = model_arbitrage::check_local_vol_density_grid(
        &js_f64_seq(&strikes, "strikes")?,
        &js_f64_seq(&expiries, "expiries")?,
        &js_f64_matrix(&vols, "vols")?,
        js_f64_seq(&forward_prices, "forwardPrices")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&violations)
}

/// Run every static-arbitrage check on a strike by expiry volatility grid.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forward_prices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_GRID_TOLERANCE` (1e-6).
/// @returns The `ArbitrageReport` object (`vol_surface_id`, `violations`, `passed`, `counts_by_type`, `counts_by_severity`).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkSurfaceGrid)]
pub fn check_surface_grid(
    strikes: JsValue,
    expiries: JsValue,
    vols: JsValue,
    forward_prices: JsValue,
    tolerance: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let report = model_arbitrage::check_surface_grid(
        &js_f64_seq(&strikes, "strikes")?,
        &js_f64_seq(&expiries, "expiries")?,
        &js_f64_matrix(&vols, "vols")?,
        js_f64_seq(&forward_prices, "forwardPrices")?,
        grid_tolerance(tolerance.as_ref())?,
    )
    .map_err(to_js_err)?;
    to_js_value(&report)
}

/// Interpolate a volatility surface; coordinates outside the grid are rejected.
/// @param surface - `VolSurface` object or JSON in the canonical wire form (`id`, `expiries`, `strikes`, `vols_row_major`, ...), such as a `materializeCube*` result.
/// @param expiry - Option expiry in years, within the surface grid.
/// @param strike - Strike in the surface's own units, within the surface grid.
/// @returns The interpolated volatility, as a decimal.
///
/// # Errors
///
/// Throws a `validation` error if `surface` is malformed or a coordinate is
/// outside the grid.
#[wasm_bindgen(js_name = getSurfaceVol)]
pub fn get_surface_vol(surface: JsValue, expiry: JsValue, strike: JsValue) -> Result<f64, JsValue> {
    let surface: VolSurface = from_js_json(&surface, "surface")?;
    vol::get_surface_vol(
        &surface,
        js_f64(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
    )
    .map_err(to_js_err)
}

/// Interpolate a volatility surface with flat clamping to the grid edges.
/// @param surface - `VolSurface` object or JSON in the canonical wire form.
/// @param expiry - Option expiry in years; clamped to the surface grid.
/// @param strike - Strike in the surface's own units; clamped to the surface grid.
/// @returns The interpolated volatility, as a decimal; `NaN` for non-finite coordinates.
///
/// # Errors
///
/// Throws a `validation` error if `surface` is malformed.
#[wasm_bindgen(js_name = getSurfaceVolClamped)]
pub fn get_surface_vol_clamped(
    surface: JsValue,
    expiry: JsValue,
    strike: JsValue,
) -> Result<f64, JsValue> {
    let surface: VolSurface = from_js_json(&surface, "surface")?;
    Ok(vol::get_surface_vol_clamped(
        &surface,
        js_f64(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
    ))
}

/// Materialize the Black volatility slice of a SABR cube at one underlying tenor.
/// @param cube - `core.VolCube` handle.
/// @param tenor - Underlying tenor in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @returns The expiry by strike `VolSurface` object in its canonical wire form.
///
/// # Errors
///
/// Throws a `validation` error if the tenor is outside the cube, the strikes
/// are invalid, or a SABR evaluation fails.
#[wasm_bindgen(js_name = materializeCubeTenorSlice)]
pub fn materialize_cube_tenor_slice(
    cube: &JsVolCube,
    tenor: JsValue,
    strikes: JsValue,
) -> Result<JsValue, JsValue> {
    let surface = vol::materialize_cube_tenor_slice(
        &cube.inner,
        js_f64(&tenor, "tenor")?,
        &js_f64_seq(&strikes, "strikes")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&surface)
}

/// Materialize the normal (Bachelier) volatility slice of a SABR cube at one underlying tenor.
/// @param cube - `core.VolCube` handle.
/// @param tenor - Underlying tenor in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @returns The expiry by strike `VolSurface` object of normal volatilities.
///
/// # Errors
///
/// Throws a `validation` error if the tenor is outside the cube, the strikes
/// are invalid, or a SABR evaluation fails.
#[wasm_bindgen(js_name = materializeCubeTenorSliceNormal)]
pub fn materialize_cube_tenor_slice_normal(
    cube: &JsVolCube,
    tenor: JsValue,
    strikes: JsValue,
) -> Result<JsValue, JsValue> {
    let surface = vol::materialize_cube_tenor_slice_normal(
        &cube.inner,
        js_f64(&tenor, "tenor")?,
        &js_f64_seq(&strikes, "strikes")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&surface)
}

/// Materialize the Black volatility slice of a SABR cube at one option expiry.
/// @param cube - `core.VolCube` handle.
/// @param expiry - Option expiry in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @returns The tenor by strike `VolSurface` object in its canonical wire form.
///
/// # Errors
///
/// Throws a `validation` error if the expiry is outside the cube, the strikes
/// are invalid, or a SABR evaluation fails.
#[wasm_bindgen(js_name = materializeCubeExpirySlice)]
pub fn materialize_cube_expiry_slice(
    cube: &JsVolCube,
    expiry: JsValue,
    strikes: JsValue,
) -> Result<JsValue, JsValue> {
    let surface = vol::materialize_cube_expiry_slice(
        &cube.inner,
        js_f64(&expiry, "expiry")?,
        &js_f64_seq(&strikes, "strikes")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&surface)
}

/// Materialize the normal (Bachelier) volatility slice of a SABR cube at one option expiry.
/// @param cube - `core.VolCube` handle.
/// @param expiry - Option expiry in years, within the cube grid.
/// @param strikes - Strictly increasing strikes of the output surface.
/// @returns The tenor by strike `VolSurface` object of normal volatilities.
///
/// # Errors
///
/// Throws a `validation` error if the expiry is outside the cube, the strikes
/// are invalid, or a SABR evaluation fails.
#[wasm_bindgen(js_name = materializeCubeExpirySliceNormal)]
pub fn materialize_cube_expiry_slice_normal(
    cube: &JsVolCube,
    expiry: JsValue,
    strikes: JsValue,
) -> Result<JsValue, JsValue> {
    let surface = vol::materialize_cube_expiry_slice_normal(
        &cube.inner,
        js_f64(&expiry, "expiry")?,
        &js_f64_seq(&strikes, "strikes")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&surface)
}

/// Materialize an FX delta-quoted surface as an expiry by strike volatility surface.
/// @param surface - `core.FxDeltaVolSurface` handle.
/// @param spot - FX spot rate (domestic per unit of foreign); positive.
/// @param domestic_rate - Continuously compounded domestic rate, as a decimal.
/// @param foreign_rate - Continuously compounded foreign rate, as a decimal.
/// @returns The strike-space `VolSurface` object in its canonical wire form.
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or a
/// delta-to-strike conversion fails.
#[wasm_bindgen(js_name = materializeFxDeltaSurface)]
pub fn materialize_fx_delta_surface(
    surface: &JsFxDeltaVolSurface,
    spot: JsValue,
    domestic_rate: JsValue,
    foreign_rate: JsValue,
) -> Result<JsValue, JsValue> {
    let surface = vol::materialize_fx_delta_surface(
        &surface.inner,
        js_f64(&spot, "spot")?,
        js_f64(&domestic_rate, "domesticRate")?,
        js_f64(&foreign_rate, "foreignRate")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&surface)
}
