//! Product-independent volatility model and evaluator bindings for WASM.
//!
//! Exposes `SabrParameters`, `SabrModel`, `SabrSmile`, and `SabrCalibrator` to
//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::api::core::market_data::{JsFxDeltaVolSurface, JsVolCube};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrModel, SabrParameters, SabrShift, SabrSmile,
};
use wasm_bindgen::prelude::*;

/// SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
#[wasm_bindgen(js_name = SabrParameters)]
pub struct JsSabrParameters {
    #[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub inner: SabrParameters,
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
        alpha: f64,
        beta: f64,
        nu: f64,
        rho: f64,
        shift: Option<f64>,
    ) -> Result<JsSabrParameters, JsValue> {
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
    pub fn implied_vol(&self, forward: f64, strike: f64, t: f64) -> Result<f64, JsValue> {
        self.inner
            .implied_volatility(forward, strike, t)
            .map_err(to_js_err)
    }

    /// Parameters used by this model.
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> JsSabrParameters {
        JsSabrParameters {
            inner: self.inner.parameters().clone(),
        }
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
    pub fn new(params: &JsSabrParameters, forward: f64, t: f64) -> JsSabrSmile {
        let model = SabrModel::new(params.clone_inner());
        Self {
            inner: SabrSmile::new(model, forward, t),
        }
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
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry, forward, or
    /// requested `strike` is outside the model domain, the Hagan expansion
    /// fails, or no volatility is returned for the strike.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(&self, strike: f64) -> Result<f64, JsValue> {
        self.inner
            .generate_smile(&[strike])
            .map_err(to_js_err)?
            .first()
            .copied()
            .ok_or_else(|| {
                JsValue::from_str("SABR smile returned no volatility for the requested strike")
            })
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
    pub fn generate_smile(&self, strikes: Vec<f64>) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .generate_smile(&strikes)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Butterfly + monotonicity arbitrage diagnostics.
    ///
    /// Returns a JSON object with `arbitrage_free`, `butterfly_violations`,
    /// and `monotonicity_violations` arrays (snake_case keys matching the Rust
    /// canonical fields and the Python binding).
    /// @param strikes - Ordered option strikes used to test the calibrated smile for static arbitrage.
    /// @param r - Continuously compounded risk-free rate, expressed as a decimal.
    /// @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if volatility generation fails for the
    /// stored smile and supplied strikes, or the diagnostics cannot be
    /// converted to a JavaScript value.
    #[wasm_bindgen(js_name = arbitrageDiagnostics)]
    pub fn arbitrage_diagnostics(
        &self,
        strikes: Vec<f64>,
        r: Option<f64>,
        q: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r.unwrap_or(0.0), q.unwrap_or(0.0))
            .map_err(to_js_err)?;
        // Violation rows are the serde form of the Rust types, so field
        // names stay identical to Python and to `ArbitrageValidationResult`.
        let out = serde_json::json!({
            "arbitrage_free": result.is_arbitrage_free(),
            "butterfly_violations": result.butterfly_violations,
            "monotonicity_violations": result.monotonicity_violations,
        });
        to_js_value(&out)
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
    pub fn with_tolerance(&self, tolerance: f64) -> JsSabrCalibrator {
        Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        }
    }

    /// Return a copy of this calibrator with an overridden iteration cap,
    /// preserving all other settings.
    /// @param max_iterations - Positive cap on solver iterations before a
    /// non-convergence error; pair a tight tolerance with a larger budget.
    #[wasm_bindgen(js_name = withMaxIterations)]
    pub fn with_max_iterations(&self, max_iterations: usize) -> JsSabrCalibrator {
        Self {
            inner: self.inner.clone().with_max_iterations(max_iterations),
        }
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
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> Result<JsSabrParameters, JsValue> {
        self.inner
            .calibrate(forward, &strikes, &market_vols, t, beta)
            .map(|inner| JsSabrParameters { inner })
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
    /// Throws a JavaScript exception if `shift` is neither `null`, a number,
    /// nor the string `"auto"`.
    #[wasm_bindgen(js_name = withShift)]
    pub fn with_shift(&self, shift: JsValue) -> Result<JsSabrCalibrator, JsValue> {
        parse_sabr_shift(
            shift.is_null() || shift.is_undefined(),
            shift.as_f64(),
            shift.as_string().as_deref(),
        )
        .map(|policy| Self {
            inner: self.inner.clone().with_shift(policy),
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Return a copy of this calibrator with exact ATM pinning enabled or
    /// disabled, preserving all other settings.
    /// @param atm_pinning - When `true`, alpha is solved analytically so the
    /// model reproduces the ATM volatility interpolated from the quotes
    /// exactly, and only nu and rho are fitted to the smile.
    #[wasm_bindgen(js_name = withAtmPinning)]
    pub fn with_atm_pinning(&self, atm_pinning: bool) -> JsSabrCalibrator {
        Self {
            inner: self.inner.clone().with_atm_pinning(atm_pinning),
        }
    }
}

impl Default for JsSabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode the JS `withShift` union without constructing a `JsValue`.
///
/// Native tests cannot call `JsValue::from_str`: wasm-bindgen's string
/// constructor is a `nounwind` stub off `wasm32` and aborts the process.
fn parse_sabr_shift(
    is_null_or_undefined: bool,
    number: Option<f64>,
    text: Option<&str>,
) -> Result<SabrShift, String> {
    if is_null_or_undefined {
        Ok(SabrShift::None)
    } else if let Some(value) = number {
        Ok(SabrShift::Fixed(value))
    } else if text == Some("auto") {
        Ok(SabrShift::Auto)
    } else {
        Err("shift must be null, a number, or the string \"auto\"".to_string())
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
    vol: f64,
    from_convention: JsValue,
    to_convention: JsValue,
    forward_rate: f64,
    time_to_expiry: f64,
) -> Result<f64, JsValue> {
    let from: vol::VolatilityConvention = serde_wasm_bindgen::from_value(from_convention)
        .map_err(|e| JsValue::from_str(&format!("invalid from_convention: {e}")))?;
    let to: vol::VolatilityConvention = serde_wasm_bindgen::from_value(to_convention)
        .map_err(|e| JsValue::from_str(&format!("invalid to_convention: {e}")))?;
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
    strikes: Vec<f64>,
    vols: Vec<f64>,
    forward: f64,
    expiry: f64,
) -> Result<JsValue, JsValue> {
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
pub fn svi_implied_vol(params: JsValue, k: f64, t: f64) -> Result<f64, JsValue> {
    let params: finstack_quant_models::volatility::svi::SviParams =
        serde_wasm_bindgen::from_value(params)
            .map_err(|e| JsValue::from_str(&format!("invalid SVI params: {e}")))?;
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
    expiry: f64,
    tenor: f64,
    strike: f64,
) -> Result<f64, JsValue> {
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
pub fn get_cube_vol_clamped(cube: &JsVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_vol_clamped(&cube.inner, expiry, tenor, strike)
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
    expiry: f64,
    tenor: f64,
    strike: f64,
) -> Result<f64, JsValue> {
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
pub fn get_cube_normal_vol_clamped(cube: &JsVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_normal_vol_clamped(&cube.inner, expiry, tenor, strike)
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
    expiry_index: usize,
) -> Result<Box<[f64]>, JsValue> {
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
    expiry: f64,
    strike: f64,
    forward: f64,
) -> Result<f64, JsValue> {
    vol::get_fx_delta_vol(&surface.inner, expiry, strike, forward).map_err(to_js_err)
}

/// Convert premium-unadjusted forward delta to strike.
///
/// # Arguments
///
/// * `delta` - Forward call delta as a decimal probability in `(0, 1)`.
/// * `forward` - Positive forward in the same units as the returned strike.
/// * `volatility` - Positive annualized Black volatility as a decimal.
/// * `expiry` - Positive option expiry in years.
#[wasm_bindgen(js_name = deltaToStrike)]
pub fn delta_to_strike(delta: f64, forward: f64, volatility: f64, expiry: f64) -> f64 {
    vol::delta_to_strike(delta, forward, volatility, expiry)
}

/// Convert strike to premium-unadjusted forward call delta.
///
/// # Arguments
///
/// * `strike` - Positive strike in the same units as `forward`.
/// * `forward` - Positive forward in the same units as `strike`.
/// * `volatility` - Positive annualized Black volatility as a decimal.
/// * `expiry` - Positive option expiry in years.
#[wasm_bindgen(js_name = strikeToDelta)]
pub fn strike_to_delta(strike: f64, forward: f64, volatility: f64, expiry: f64) -> f64 {
    vol::strike_to_delta(strike, forward, volatility, expiry)
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
    fn sabr_model_computes_atm_vol() {
        let p = JsSabrParameters::new(0.2, 1.0, 0.3, -0.2, None).expect("params");
        let smile = JsSabrSmile::new(&p, 100.0, 1.0);
        let atm = smile.atm_vol().expect("atm_vol");
        assert!(atm > 0.0 && atm < 1.0);
    }

    #[test]
    fn sabr_model_exposes_params_getter() {
        let p = JsSabrParameters::new(0.2, 0.5, 0.3, -0.2, None).expect("params");
        let model = JsSabrModel::new(&p);
        let roundtrip = model.params();
        assert!((roundtrip.alpha() - 0.2).abs() < 1e-12);
        assert!((roundtrip.beta() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn sabr_calibrator_with_tolerance_calibrates() {
        let p = JsSabrParameters::new(0.05, 0.5, 0.4, -0.1, None).expect("params");
        let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        let smile = JsSabrSmile::new(&p, 0.03, 1.0);
        let vols = smile.generate_smile(strikes.clone()).expect("smile");

        // 1e-6 on the vega-weighted SSE objective is attainable within the
        // default iteration budget; tighter tolerances fail loudly under the
        // strict non-convergence semantics of core `minimize` because rho is
        // weakly identified on this near-symmetric strike set.
        let calibrator = JsSabrCalibrator::new().with_tolerance(1e-6);
        let fitted = calibrator
            .calibrate(0.03, strikes, vols.into_vec(), 1.0, 0.5)
            .expect("calibrate");
        assert!((fitted.beta() - 0.5).abs() < 1e-12);
        assert!(fitted.alpha() > 0.0);
    }

    #[test]
    fn parse_sabr_shift_accepts_null_number_and_auto() {
        assert_eq!(
            parse_sabr_shift(true, None, None).expect("null"),
            SabrShift::None
        );
        assert_eq!(
            parse_sabr_shift(false, Some(0.03), None).expect("fixed"),
            SabrShift::Fixed(0.03)
        );
        assert_eq!(
            parse_sabr_shift(false, None, Some("auto")).expect("auto"),
            SabrShift::Auto
        );
        assert!(parse_sabr_shift(false, None, Some("later")).is_err());
    }

    #[test]
    fn sabr_auto_shift_fits_negative_rate_smile() {
        // Native tests cannot construct `JsValue` strings or Debug a `JsValue`
        // error: both abort off wasm32. Drive the same `"auto"` policy through
        // the native-testable parser and the domain calibrator. Synthetic
        // quotes use the documented 2% ladder rung (`-min(strike)+10bp = 1.6%`
        // rounds up to 2%), matching the JS facade and Python bindings.
        let policy = parse_sabr_shift(false, None, Some("auto")).expect("auto shift policy");
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
