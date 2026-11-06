//! Product-independent volatility model and evaluator bindings for WASM.
//!
//! Exposes `SabrParameters`, `SabrSmile`, and `SabrCalibrator` to
//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::api::core::surfaces::{
    JsFxDeltaVolSurface, JsVolCube, JsVolCubeExpirySlice, JsVolSurface,
};
use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_f64_matrix, js_f64_seq, js_json_value, js_opt_f64, js_uint,
};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::arbitrage as model_arbitrage;
use finstack_quant_models::volatility::local_vol::LocalVolSurface;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrParameters, SabrShift, SabrSmile,
};
use finstack_quant_models::volatility::svi::SviParams;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const params = new models.volatility.SabrParameters(0.2, 1.0, 0.3, -0.2);
/// console.log(params.alpha, params.rho, params.impliedVol(100, 105, 1));
/// params.free();
/// ```
#[wasm_bindgen(js_name = SabrParameters)]
pub struct JsSabrParameters {
    pub(crate) inner: SabrParameters,
}

json_round_trip!(JsSabrParameters, SabrParameters);

#[wasm_bindgen(js_class = SabrParameters)]
impl JsSabrParameters {
    /// Create SABR parameters from alpha, beta, nu, rho, and optional shift.
    /// @param alpha - Positive SABR initial volatility scale parameter.
    /// @param beta - SABR CEV elasticity parameter from 0 through 1.
    /// @param nu - Finite nonnegative SABR volatility-of-volatility parameter; zero keeps the volatility factor constant.
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
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] beta: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] shift: Option<JsValue>,
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
    /// @returns A `SabrParameters` handle.
    #[wasm_bindgen(js_name = equityDefault)]
    pub fn equity_default() -> JsSabrParameters {
        Self {
            inner: SabrParameters::equity_default(),
        }
    }

    /// Default SABR parameters for rates underlyings.
    /// @returns A `SabrParameters` handle.
    #[wasm_bindgen(js_name = ratesDefault)]
    pub fn rates_default() -> JsSabrParameters {
        Self {
            inner: SabrParameters::rates_default(),
        }
    }

    /// Rates market standard SABR parameters (`beta = 0.5`).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility on the `beta = 0.5` CEV backbone (decimal).
    /// * `nu` - Finite non-negative volatility of volatility per square-root year.
    /// * `rho` - Forward/volatility correlation in `[-1, 1]`.
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `[-1, 1]`.
    /// @param alpha - Positive initial volatility on the `beta = 0.5` CEV backbone (decimal).
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `[-1, 1]`.
    #[wasm_bindgen(js_name = ratesStandard)]
    pub fn rates_standard(
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        SabrParameters::rates_standard(alpha, nu, rho)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Normal SABR parameters (`beta = 0`; negative forwards allowed).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial normal volatility in absolute rate units.
    /// * `nu` - Finite non-negative volatility of volatility per square-root year.
    /// * `rho` - Forward/volatility correlation in `[-1, 1]`.
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `[-1, 1]`.
    /// @param alpha - Positive initial normal volatility in absolute rate units.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `[-1, 1]`.
    #[wasm_bindgen]
    pub fn normal(
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        SabrParameters::normal(alpha, nu, rho)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Lognormal SABR parameters (`beta = 1`).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial Black volatility as a decimal.
    /// * `nu` - Finite non-negative volatility of volatility per square-root year.
    /// * `rho` - Forward/volatility correlation in `[-1, 1]`.
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `[-1, 1]`.
    /// @param alpha - Positive initial Black volatility as a decimal.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `[-1, 1]`.
    #[wasm_bindgen]
    pub fn lognormal(
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        SabrParameters::lognormal(alpha, nu, rho)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Shifted normal SABR parameters (`beta = 0` with a displacement).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial normal volatility in absolute rate units.
    /// * `nu` - Finite non-negative volatility of volatility per square-root year.
    /// * `rho` - Forward/volatility correlation in `[-1, 1]`.
    /// * `shift` - Positive displacement added to forward and strike, in the forward's rate units.
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `[-1, 1]` or `shift` is not finite and positive.
    /// @param alpha - Positive initial normal volatility in absolute rate units.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `[-1, 1]`.
    /// @param shift - Positive displacement added to forward and strike, in the forward's rate units.
    #[wasm_bindgen(js_name = shiftedNormal)]
    pub fn shifted_normal(
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] shift: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        let shift = js_f64(&shift, "shift")?;
        SabrParameters::shifted_normal(alpha, nu, rho, shift)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Shifted lognormal SABR parameters (`beta = 1` with a displacement).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial Black volatility of the shifted forward `F + shift` (decimal).
    /// * `nu` - Finite non-negative volatility of volatility per square-root year.
    /// * `rho` - Forward/volatility correlation in `[-1, 1]`.
    /// * `shift` - Positive displacement added to forward and strike, in the forward's rate units.
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `[-1, 1]` or `shift` is not finite and positive.
    /// @param alpha - Positive initial Black volatility of the shifted forward `F + shift` (decimal).
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `[-1, 1]`.
    /// @param shift - Positive displacement added to forward and strike, in the forward's rate units.
    #[wasm_bindgen(js_name = shiftedLognormal)]
    pub fn shifted_lognormal(
        #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] shift: JsValue,
    ) -> Result<JsSabrParameters, JsValue> {
        let alpha = js_f64(&alpha, "alpha")?;
        let nu = js_f64(&nu, "nu")?;
        let rho = js_f64(&rho, "rho")?;
        let shift = js_f64(&shift, "shift")?;
        SabrParameters::shifted_lognormal(alpha, nu, rho, shift)
            .map(Self::from_inner)
            .map_err(to_js_err)
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
    /// @returns `true` when a SABR displacement shift is configured.
    #[wasm_bindgen(js_name = isShifted)]
    pub fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
    }

    /// Whether the parameterization admits negative forwards.
    /// @returns `true` when the SABR parameterization admits negative forwards.
    #[wasm_bindgen(js_name = supportsNegativeRates)]
    pub fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
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
    /// @returns Hagan-2002 implied volatility: normal vol in absolute rate units when beta < 1e-4, Black decimal volatility otherwise.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    ) -> Result<f64, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strike = js_f64(&strike, "strike")?;
        let t = js_f64(&t, "t")?;
        self.inner
            .implied_vol(forward, strike, t)
            .map_err(to_js_err)
    }
}

impl JsSabrParameters {
    pub(crate) fn from_inner(inner: SabrParameters) -> Self {
        Self { inner }
    }
}

/// Volatility smile generator for a fixed `(forward, t)` pair.
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const params = models.volatility.SabrParameters.ratesDefault();
/// const smile = new models.volatility.SabrSmile(params, 0.03, 2);
/// console.log(smile.generateSmile([0.02, 0.03, 0.04]));
/// smile.free();
/// params.free();
/// ```
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
        #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    ) -> Result<JsSabrSmile, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let t = js_f64(&t, "t")?;
        Ok(Self {
            inner: SabrSmile::new(params.inner.clone(), forward, t),
        })
    }

    /// At-the-money implied volatility.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry or effective forward
    /// is outside the model domain, or the ATM calculation produces an invalid
    /// volatility.
    /// @returns ATM implied volatility for this smile's `(forward, t)`: normal vol in absolute rate units when beta < 1e-4, Black decimal vol otherwise.
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
    pub fn implied_vol(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    ) -> Result<f64, JsValue> {
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
    /// @returns One implied vol per strike, in the same order as `strikes` (normal vol when beta < 1e-4, Black decimal vol otherwise).
    #[wasm_bindgen(js_name = generateSmile)]
    pub fn generate_smile(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number[]")] strikes: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
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
    /// @param strikes - Finite, strictly ascending strikes with arbitrary spacing; convexity uses the actual strike distances.
    /// @param r - Finite continuously compounded risk-free rate (decimal) that discounts the
    /// forward-based Black call prices compared against the 1e-6 tolerance.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if volatility generation fails for the
    /// stored smile and supplied strikes, strikes are non-finite or not strictly
    /// ascending, the rate is non-finite, call prices are non-finite, or the
    /// result cannot be converted to a JavaScript value.
    /// @returns The Rust `ArbitrageValidationResult` for the supplied strike grid.
    #[wasm_bindgen(js_name = validateNoArbitrage, unchecked_return_type = "ArbitrageValidationResult")]
    pub fn validate_no_arbitrage(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] r: JsValue,
    ) -> Result<JsValue, JsValue> {
        let r = js_f64(&r, "r")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }

    /// Strike for an absolute forward delta, using the smile's ATM volatility.
    ///
    /// Bachelier delta when `beta == 0`, Black delta on the (shifted) forward
    /// otherwise; an ATM-vol approximation, not a smile-consistent solve.
    ///
    /// # Arguments
    ///
    /// * `delta` - Absolute undiscounted forward delta strictly between 0 and 1.
    /// * `is_call` - `true` for call delta, `false` for absolute put delta.
    ///
    /// @returns Strike in the forward's units.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `delta` is outside
    /// `(0, 1)`, the smile coordinates are outside the model domain, or the
    /// strike overflows.
    /// @param delta - Absolute undiscounted forward delta strictly between 0 and 1.
    /// @param isCall - `true` for call delta, `false` for absolute put delta.
    #[wasm_bindgen(js_name = strikeFromDelta)]
    pub fn strike_from_delta(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] delta: JsValue,
        #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
    ) -> Result<f64, JsValue> {
        let delta = js_f64(&delta, "delta")?;
        let is_call = js_bool(&is_call, "isCall")?;
        self.inner
            .strike_from_delta(delta, is_call)
            .map_err(to_js_err)
    }

    /// Throw when the smile has static arbitrage on `strikes`.
    ///
    /// Same butterfly and monotonicity checks as `validateNoArbitrage`; the
    /// error message counts each kind of violation.
    ///
    /// # Arguments
    ///
    /// * `strikes` - Finite, strictly ascending strikes with arbitrary spacing.
    /// * `r` - Finite continuously compounded risk-free rate (decimal) that
    ///   discounts the forward-based Black call prices.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if any violation is found,
    /// the grid is not finite and strictly ascending, `r` is non-finite, or the
    /// smile cannot be evaluated on the grid.
    /// @param strikes - Finite, strictly ascending strikes with arbitrary spacing.
    /// @param r - Finite continuously compounded risk-free rate (decimal) that discounts the forward-based Black call prices.
    #[wasm_bindgen(js_name = checkNoArbitrage)]
    pub fn check_no_arbitrage(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] r: JsValue,
    ) -> Result<(), JsValue> {
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let r = js_f64(&r, "r")?;
        self.inner
            .check_no_arbitrage(&strikes, r)
            .map_err(to_js_err)
    }

    /// Smile vols on `strikes` after a greedy monotonicity/convexity repair
    /// (not a full Fengler QP); strikes without violations keep their SABR vol.
    ///
    /// # Arguments
    ///
    /// * `strikes` - Finite, strictly ascending strikes in the model domain.
    /// * `r` - Finite continuously compounded risk-free rate (decimal) that
    ///   discounts the forward-based Black call prices.
    /// * `max_iterations` - Maximum number of repair passes (non-negative integer).
    ///
    /// @returns Repaired vols aligned with `strikes`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for a non-integer `maxIterations`, and a
    /// `FinstackError` (kind `validation`) if the grid is not finite and
    /// strictly ascending, `r` is non-finite, or the smile or a price
    /// inversion fails.
    /// @param strikes - Finite, strictly ascending strikes in the model domain.
    /// @param r - Finite continuously compounded risk-free rate (decimal) that discounts the forward-based Black call prices.
    /// @param maxIterations - Maximum number of repair passes (non-negative integer).
    #[wasm_bindgen(js_name = repairArbitrage)]
    pub fn repair_arbitrage(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] r: JsValue,
        #[wasm_bindgen(js_name = "maxIterations", unchecked_param_type = "number")] max_iterations: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let r = js_f64(&r, "r")?;
        let max_iterations: usize = js_uint(&max_iterations, "maxIterations")?;
        self.inner
            .repair_arbitrage(&strikes, r, max_iterations)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Forward price or rate the smile is built around.
    #[wasm_bindgen(getter)]
    pub fn forward(&self) -> f64 {
        self.inner.forward()
    }

    /// Time to expiry of the smile, in years.
    #[wasm_bindgen(getter, js_name = timeToExpiry)]
    pub fn time_to_expiry(&self) -> f64 {
        self.inner.time_to_expiry()
    }
}

/// SABR calibrator (Levenberg-Marquardt with beta fixed).
///
/// Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const calibrator = models.volatility.SabrCalibrator.highPrecision();
/// const tighter = calibrator.withTolerance(1e-10);
/// tighter.free();
/// calibrator.free();
/// ```
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
    /// @returns A `SabrCalibrator` handle.
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
    /// @returns A `SabrCalibrator` handle.
    #[wasm_bindgen(js_name = withTolerance)]
    pub fn with_tolerance(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] tolerance: JsValue,
    ) -> Result<JsSabrCalibrator, JsValue> {
        let tolerance = js_f64(&tolerance, "tolerance")?;
        Ok(Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        })
    }

    /// Return a copy of this calibrator with an overridden iteration cap,
    /// preserving all other settings.
    /// @param maxIterations - Positive cap on solver iterations before a
    /// non-convergence error; a finite non-negative integer no greater than
    /// 4294967295. Zero is rejected by the canonical calibrator when used.
    ///
    /// # Errors
    ///
    /// Throws if `max_iterations` is non-finite, fractional, negative, or
    /// exceeds the wasm32 integer range.
    /// @returns A `SabrCalibrator` handle.
    #[wasm_bindgen(js_name = withMaxIterations)]
    pub fn with_max_iterations(
        &self,
        #[wasm_bindgen(js_name = "maxIterations", unchecked_param_type = "number")] max_iterations: JsValue,
    ) -> Result<JsSabrCalibrator, JsValue> {
        let max_iterations: usize = js_uint(&max_iterations, "maxIterations")?;
        Ok(Self {
            inner: self.inner.clone().with_max_iterations(max_iterations),
        })
    }

    /// Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param strikes - Option strikes aligned one-for-one with market_vols.
    /// @param marketVols - Positive quotes aligned with strikes: absolute decimal-rate normal volatilities for beta near zero, otherwise annual Black volatilities in relative decimals.
    /// @param t - Positive time to expiry in years.
    /// @param beta - SABR CEV elasticity in `[0, 1]`, held fixed during calibration.
    /// @returns Calibrated parameters, signed model-minus-market quote errors, and deterministic solver diagnostics.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the strike and volatility lengths
    /// differ, the quote arrays are empty, the SABR inputs or fitted parameters
    /// are invalid, or the calibration solver does not converge.
    #[wasm_bindgen(unchecked_return_type = "SabrCalibrationOutcome")]
    pub fn calibrate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number[]")] strikes: JsValue,
        #[wasm_bindgen(js_name = "marketVols", unchecked_param_type = "number[]")]
        market_vols: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] beta: JsValue,
    ) -> Result<JsValue, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let market_vols = js_f64_seq(&market_vols, "marketVols")?;
        let t = js_f64(&t, "t")?;
        let beta = js_f64(&beta, "beta")?;
        self.inner
            .calibrate(forward, &strikes, &market_vols, t, beta)
            .map_err(to_js_err)
            .and_then(|outcome| to_js_value(&outcome))
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
    /// Throws a `FinstackError` (`kind: "validation"`) for a boolean, a string
    /// other than `"auto"` or any other shape the Rust `SabrShift` wire form
    /// rejects, and a `TypeError` (`kind: "invalid_type"`) for a non-finite
    /// number or a value JSON cannot represent.
    /// @returns A `SabrCalibrator` handle.
    #[wasm_bindgen(js_name = withShift)]
    pub fn with_shift(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | 'auto' | null | undefined")] shift: JsValue,
    ) -> Result<JsSabrCalibrator, JsValue> {
        // `undefined` is the omitted argument: the same as `null`.
        let wire = if shift.is_undefined() {
            serde_json::Value::Null
        } else {
            js_json_value(&shift, "shift")?
        };
        let policy: SabrShift = serde_json::from_value(wire).map_err(|error| {
            to_js_err(finstack_quant_core::Error::Validation(format!(
                "shift: {error}"
            )))
        })?;
        Ok(Self {
            inner: self.inner.clone().with_shift(policy),
        })
    }

    /// Return a copy of this calibrator with exact ATM pinning enabled or
    /// disabled, preserving all other settings.
    /// @param atmPinning - When `true`, alpha is solved analytically so the
    /// model reproduces the ATM volatility interpolated from the quotes
    /// exactly, and only nu and rho are fitted to the smile.
    /// @returns A `SabrCalibrator` handle.
    #[wasm_bindgen(js_name = withAtmPinning)]
    pub fn with_atm_pinning(
        &self,
        #[wasm_bindgen(js_name = "atmPinning", unchecked_param_type = "boolean")]
        atm_pinning: JsValue,
    ) -> Result<JsSabrCalibrator, JsValue> {
        let atm_pinning = js_bool(&atm_pinning, "atmPinning")?;
        Ok(Self {
            inner: self.inner.clone().with_atm_pinning(atm_pinning),
        })
    }

    /// Maximum accepted relative error of every fitted volatility quote.
    #[wasm_bindgen(getter)]
    pub fn tolerance(&self) -> f64 {
        self.inner.tolerance()
    }

    /// Iteration cap before the solver reports non-convergence.
    #[wasm_bindgen(getter, js_name = maxIterations)]
    pub fn max_iterations(&self) -> usize {
        self.inner.max_iterations()
    }

    /// Displacement policy: `null` (no shift), a fixed numeric shift in the
    /// forward's units, or `"auto"` — the same forms `withShift` accepts.
    #[wasm_bindgen(getter, unchecked_return_type = "number | 'auto' | null")]
    pub fn shift(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.shift())
    }

    /// Whether alpha is pinned to the interpolated ATM volatility.
    #[wasm_bindgen(getter, js_name = atmPinning)]
    pub fn atm_pinning(&self) -> bool {
        self.inner.atm_pinning()
    }
}

impl Default for JsSabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Bachelier (normal) implied volatility from an undiscounted option price.
///
/// Inverts the Bachelier price per unit annuity (`df = 1`); forward and
/// strike may be zero or negative.
///
/// # Arguments
///
/// * `price` - Non-negative undiscounted option price (forward premium per
///   unit annuity) in the forward's units.
/// * `forward` - Finite forward rate or price; negative values are allowed.
/// * `strike` - Finite strike in the forward's units; negative values are allowed.
/// * `t` - Strictly positive time to expiry in years.
/// * `is_call` - `true` for a call, `false` for a put.
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
/// # Arguments
///
/// * `price` - Non-negative undiscounted option price (forward premium per
///   unit annuity) in the forward's units.
/// * `forward` - Strictly positive forward rate or price.
/// * `strike` - Strictly positive strike in the forward's units.
/// * `t` - Strictly positive time to expiry in years.
/// * `is_call` - `true` for a call, `false` for a put.
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
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Positive option expiry in years within the cube grid.
/// * `tenor` - Positive underlying tenor in years within the cube grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
/// * `convention` - Output quote convention: `"lognormal"` (decimal Black,
///   shifted by the cube's own SABR shifts where it stores them) or `"normal"`
///   (absolute Bachelier vol in the forward's rate units). The serde form
///   `{"shifted_lognormal": {"shift": s}}` is rejected because the cube already
///   carries its own shifts.
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
/// clamping; returns `NaN` where the volatility is undefined.
///
/// # Arguments
///
/// * `cube` - Structurally validated data-only volatility cube.
/// * `expiry` - Finite option expiry in years; clamped to the stored grid.
/// * `tenor` - Finite underlying tenor in years; clamped to the stored grid.
/// * `strike` - Finite strike in the same rate units as the stored forwards.
/// * `convention` - Output quote convention: `"lognormal"` (decimal Black,
///   shifted by the cube's own SABR shifts where it stores them) or `"normal"`
///   (absolute Bachelier vol in the forward's rate units). The serde form
///   `{"shifted_lognormal": {"shift": s}}` is rejected because the cube already
///   carries its own shifts.
///   A shifted-lognormal convention yields `NaN`.
///
/// # Errors
///
/// Throws a JavaScript exception for a non-numeric coordinate or an
/// undecodable `convention`.
/// @param cube - Structurally validated data-only volatility cube.
/// @param expiry - Finite option expiry in years; clamped to the stored grid.
/// @param tenor - Finite underlying tenor in years; clamped to the stored grid.
/// @param strike - Finite strike in the same rate units as the stored forwards.
/// @param convention - Output quote convention: `"lognormal"` (decimal Black, shifted by the cube's own SABR shifts where it stores them) or `"normal"` (absolute Bachelier vol in the forward's rate units). The serde form `{"shifted_lognormal": {"shift": s}}` is rejected because the cube already carries its own shifts. A shifted-lognormal convention yields `NaN`.
/// @returns Annualized volatility in `convention`, or `NaN` when undefined (including a shifted-lognormal `convention`).
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
    Ok(vol::get_cube_vol_clamped(
        &cube.inner,
        expiry,
        tenor,
        strike,
        convention,
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
/// Throws a JavaScript exception when `expiry_index` is non-finite, fractional,
/// negative, exceeds the wasm32 integer range, or is outside the surface.
/// @param surface - Structurally validated data-only FX delta surface.
/// @param expiryIndex - Zero-based stored expiry index.
/// @returns Three annualized decimal volatilities in ATM, put, call order.
#[wasm_bindgen(js_name = getFxDeltaPillarVols)]
pub fn get_fx_delta_pillar_vols(
    surface: &JsFxDeltaVolSurface,
    #[wasm_bindgen(js_name = "expiryIndex", unchecked_param_type = "number")] expiry_index: JsValue,
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

/// Convert premium-unadjusted forward delta to strike.
///
/// # Arguments
///
/// * `delta` - Forward call delta as a decimal probability in `(0, 1)`.
/// * `forward` - Positive forward in the same units as the returned strike.
/// * `vol` - Positive annualized Black volatility as a decimal.
/// * `expiry` - Positive option expiry in years.
/// @param delta - Forward call delta as a decimal probability in `(0, 1)`.
/// @param forward - Positive forward in the same units as the returned strike.
/// @param vol - Positive annualized Black volatility as a decimal.
/// @param expiry - Positive option expiry in years.
/// @returns Strike in the same units as `forward`.
#[wasm_bindgen(js_name = deltaToStrike)]
pub fn delta_to_strike(
    #[wasm_bindgen(unchecked_param_type = "number")] delta: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
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
/// @param strike - Positive strike in the same units as `forward`.
/// @param forward - Positive forward in the same units as `strike`.
/// @param vol - Positive annualized Black volatility as a decimal.
/// @param expiry - Positive option expiry in years.
/// @returns Forward call delta as a decimal probability.
#[wasm_bindgen(js_name = strikeToDelta)]
pub fn strike_to_delta(
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
) -> Result<f64, JsValue> {
    let strike = js_f64(&strike, "strike")?;
    let forward = js_f64(&forward, "forward")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    Ok(vol::strike_to_delta(strike, forward, vol, expiry))
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

/// Butterfly-arbitrage check on a strike by expiry volatility grid.
/// @param strikes - Strictly increasing strike grid shared by every row.
/// @param expiries - Strictly increasing expiries in years, one per row of `vols`.
/// @param vols - Implied volatilities as nested rows: one `number[]` per expiry, one decimal volatility per strike.
/// @param forwardPrices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkButterflyGrid, unchecked_return_type = "generated.models.ArbitrageViolation[]")]
pub fn check_butterfly_grid(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
    #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
    forward_prices: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number")] tolerance: Option<JsValue>,
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
/// @param forwardPrices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkCalendarSpreadGrid, unchecked_return_type = "generated.models.ArbitrageViolation[]")]
pub fn check_calendar_spread_grid(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
    #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
    forward_prices: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number")] tolerance: Option<JsValue>,
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
/// @param forwardPrices - One forward price per expiry.
/// @returns The `ArbitrageViolation` objects found (empty when the grid is clean).
///
/// # Errors
///
/// Throws a `validation` error if the grid is invalid or the forwards do not
/// match the expiries.
#[wasm_bindgen(js_name = checkLocalVolDensityGrid, unchecked_return_type = "generated.models.ArbitrageViolation[]")]
pub fn check_local_vol_density_grid(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
    #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
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
/// @param forwardPrices - One forward price to broadcast, or one per expiry.
/// @param tolerance - Optional non-negative violation tolerance; omitted uses the Rust `DEFAULT_ARBITRAGE_TOLERANCE` (1e-10).
/// @returns The `ArbitrageReport` object (`vol_surface_id`, `violations`, `passed`, `counts_by_type`, `counts_by_severity`).
///
/// # Errors
///
/// Throws a `validation` error if the grid or forwards are invalid or the
/// tolerance is negative or non-finite.
#[wasm_bindgen(js_name = checkSurfaceGrid, unchecked_return_type = "generated.models.ArbitrageReport")]
pub fn check_surface_grid(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray[]")] vols: JsValue,
    #[wasm_bindgen(js_name = "forwardPrices", unchecked_param_type = "NumericArray")]
    forward_prices: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number")] tolerance: Option<JsValue>,
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
) -> Result<JsVolSurface, JsValue> {
    let surface = vol::materialize_cube_tenor_slice(
        &cube.inner,
        js_f64(&tenor, "tenor")?,
        &js_f64_seq(&strikes, "strikes")?,
        js_wire(&convention, "convention")?,
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
/// @param surface - `core.FxDeltaVolSurface` handle.
/// @param spot - FX spot rate (domestic per unit of foreign); positive.
/// @param domesticRate - Continuously compounded domestic rate, as a decimal.
/// @param foreignRate - Continuously compounded foreign rate, as a decimal.
/// @returns The strike-space `core.VolSurface` handle.
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or a
/// delta-to-strike conversion fails.
#[wasm_bindgen(js_name = materializeFxDeltaSurface)]
pub fn materialize_fx_delta_surface(
    surface: &JsFxDeltaVolSurface,
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(js_name = "domesticRate", unchecked_param_type = "number")]
    domestic_rate: JsValue,
    #[wasm_bindgen(js_name = "foreignRate", unchecked_param_type = "number")] foreign_rate: JsValue,
) -> Result<JsVolSurface, JsValue> {
    let materialized = vol::materialize_fx_delta_surface(
        &surface.inner,
        js_f64(&spot, "spot")?,
        js_f64(&domestic_rate, "domesticRate")?,
        js_f64(&foreign_rate, "foreignRate")?,
    )
    .map_err(to_js_err)?;
    Ok(JsVolSurface::from_inner(Arc::new(materialized)))
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
        let vols = SabrSmile::new(p, forward, 1.0)
            .generate_smile(&strikes)
            .expect("smile");

        let fitted = SabrCalibrator::new()
            .with_shift(policy)
            .calibrate(forward, &strikes, &vols, 1.0, 0.5)
            .expect("auto-shift calibrate")
            .parameters;
        let shift = fitted
            .shift()
            .expect("negative-rate fit must carry a shift");
        assert!(shift > 0.0);
        assert!(fitted.is_shifted());
        assert!((shift - 0.02).abs() < 1e-12);
    }
}
