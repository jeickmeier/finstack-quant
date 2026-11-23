//! SABR parameters, smile, and calibrator WASM handles.

//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

use crate::utils::input::{js_bool, js_f64, js_f64_seq, js_json_value, js_opt_f64, js_uint};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrParameters, SabrShift, SabrSmile,
};
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
/// console.log(params.alpha, params.rho, params.impliedVol(100, 105, 1, "black_lognormal"));
/// params.free();
/// ```
#[wasm_bindgen(js_name = SabrParameters)]
pub struct JsSabrParameters {
    pub(crate) inner: SabrParameters,
}

json_round_trip!(JsSabrParameters, SabrParameters);

wasm_getters!(JsSabrParameters, SabrParameters |s| {
    /// SABR `alpha` (ATM volatility level).
    alpha -> f64 = s.inner.alpha;
    /// SABR `beta` (backbone exponent).
    beta -> f64 = s.inner.beta;
    /// SABR `nu` (vol-of-vol).
    nu -> f64 = s.inner.nu;
    /// SABR `rho` (spot/vol correlation).
    rho -> f64 = s.inner.rho;
    /// Displacement applied for shifted SABR, if any.
    shift -> Option<f64> = s.inner.shift;
});

macro_rules! sabr_preset_wasm {
    (
        $(#[$doc:meta])*
        $name:ident as $js_name:ident ()
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = SabrParameters)]
        impl JsSabrParameters {
            $(#[$doc])*
            #[wasm_bindgen(js_name = $js_name)]
            pub fn $name() -> JsSabrParameters {
                Self::from_inner(SabrParameters::$name())
            }
        }
    };
    (
        $(#[$doc:meta])*
        $name:ident as $js_name:ident ($($arg:ident),+ $(,)?)
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = SabrParameters)]
        impl JsSabrParameters {
            $(#[$doc])*
            #[wasm_bindgen(js_name = $js_name)]
            pub fn $name(
                $(#[wasm_bindgen(unchecked_param_type = "number")] $arg: JsValue),+
            ) -> Result<JsSabrParameters, JsValue> {
                SabrParameters::$name($(js_f64(&$arg, stringify!($arg))?),+)
                    .map(Self::from_inner)
                    .map_err(to_js_err)
            }
        }
    };
}

sabr_preset_wasm!(
    /// Default SABR parameters for equity underlyings.
    /// @returns A `SabrParameters` handle.
    equity_default as equityDefault ()
);

sabr_preset_wasm!(
    /// Default SABR parameters for rates underlyings.
    /// @returns A `SabrParameters` handle.
    rates_default as ratesDefault ()
);

sabr_preset_wasm!(
    /// Rates market standard SABR parameters (`beta = 0.5`).
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `(-1, 1)`.
    /// @param alpha - Positive initial volatility on the `beta = 0.5` CEV backbone (decimal).
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `(-1, 1)`.
    rates_standard as ratesStandard (alpha, nu, rho)
);

sabr_preset_wasm!(
    /// Normal SABR parameters (`beta = 0`; negative forwards allowed).
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `(-1, 1)`.
    /// @param alpha - Positive initial normal volatility in absolute rate units.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `(-1, 1)`.
    normal as normal (alpha, nu, rho)
);

sabr_preset_wasm!(
    /// Lognormal SABR parameters (`beta = 1`).
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `(-1, 1)`.
    /// @param alpha - Positive initial Black volatility as a decimal.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `(-1, 1)`.
    lognormal as lognormal (alpha, nu, rho)
);

sabr_preset_wasm!(
    /// Shifted normal SABR parameters (`beta = 0` with a displacement).
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `(-1, 1)` or `shift` is non-finite.
    /// @param alpha - Positive initial normal volatility in absolute rate units.
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `(-1, 1)`.
    /// @param shift - Finite signed displacement added to forward and strike, in the forward's rate units.
    shifted_normal as shiftedNormal (alpha, nu, rho, shift)
);

sabr_preset_wasm!(
    /// Shifted lognormal SABR parameters (`beta = 1` with a displacement).
    ///
    /// @returns A validated `SabrParameters` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if `alpha` is not finite
    /// and positive, `nu` is negative or non-finite, `rho` is outside
    /// `(-1, 1)` or `shift` is non-finite.
    /// @param alpha - Positive initial Black volatility of the shifted forward `F + shift` (decimal).
    /// @param nu - Finite non-negative volatility of volatility per square-root year.
    /// @param rho - Forward/volatility correlation in `(-1, 1)`.
    /// @param shift - Finite signed displacement added to forward and strike, in the forward's rate units.
    shifted_lognormal as shiftedLognormal (alpha, nu, rho, shift)
);

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
    /// outside `(-1, 1)`, or a supplied `shift` is non-finite.
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

    /// Implied volatility in the explicitly selected Normal or Black quote type.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param strike - Option strike price in the same price units as the underlying.
    /// @param t - Time from the curve base date in years.
    /// @param quoteType - Annualized Normal, Black, or shifted Black quote type.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `t` is not positive, the forward or
    /// strike lies outside the selected shifted or unshifted SABR domain, or
    /// the Hagan expansion produces an undefined or non-finite volatility.
    /// @returns Annualized Normal volatility in rate units or relative Black volatility, as selected.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
        #[wasm_bindgen(
            js_name = "quoteType",
            unchecked_param_type = "\"normal\" | \"black_lognormal\" | \"shifted_black_lognormal\""
        )]
        quote_type: JsValue,
    ) -> Result<f64, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strike = js_f64(&strike, "strike")?;
        let t = js_f64(&t, "t")?;
        self.inner
            .implied_vol(forward, strike, t, js_wire(&quote_type, "quoteType")?)
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
/// const smile = new models.volatility.SabrSmile(params, 0.03, 2, "black_lognormal");
/// console.log(smile.generateSmile([0.02, 0.03, 0.04]));
/// smile.free();
/// params.free();
/// ```
#[wasm_bindgen(js_name = SabrSmile)]
pub struct JsSabrSmile {
    inner: SabrSmile,
}

wasm_getters!(JsSabrSmile, SabrSmile |s| {
    /// Forward price or rate the smile is built around.
    forward -> f64 = s.inner.forward();
    /// Time to expiry of the smile, in years.
    time_to_expiry as timeToExpiry -> f64 = s.inner.time_to_expiry();
});

#[wasm_bindgen(js_class = SabrSmile)]
impl JsSabrSmile {
    /// Create a SABR smile for a fixed forward and expiry.
    /// @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
    /// @param forward - Forward price or rate in the same quote convention as the strike.
    /// @param t - Time from the curve base date in years.
    /// @param quoteType - Annualized Normal, Black, or shifted Black quote type.
    #[wasm_bindgen(constructor)]
    pub fn new(
        params: &JsSabrParameters,
        #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
        #[wasm_bindgen(
            js_name = "quoteType",
            unchecked_param_type = "\"normal\" | \"black_lognormal\" | \"shifted_black_lognormal\""
        )]
        quote_type: JsValue,
    ) -> Result<JsSabrSmile, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let t = js_f64(&t, "t")?;
        Ok(Self {
            inner: SabrSmile::new(
                params.inner.clone(),
                forward,
                t,
                js_wire(&quote_type, "quoteType")?,
            ),
        })
    }

    /// At-the-money implied volatility.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the smile's expiry or effective forward
    /// is outside the model domain, or the ATM calculation produces an invalid
    /// volatility.
    /// @returns ATM implied volatility for this smile's `(forward, t)`: Normal vol in absolute rate units or Black decimal vol, as selected.
    #[wasm_bindgen(js_name = atmVol)]
    pub fn atm_vol(&self) -> Result<f64, JsValue> {
        self.inner.atm_vol().map_err(to_js_err)
    }

    /// Implied volatility in the smile’s selected quote convention.
    /// @param strike - Option strike price in the same price units as the underlying.
    /// @returns Normal volatility in absolute rate units or relative Black volatility, as selected.
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
    /// @returns One implied vol per strike, in the same order as `strikes` in the smile's selected annualized Normal, Black, or shifted Black units.
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
    /// forward-based call prices in the selected model compared against the 1e-6 tolerance.
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
    /// Uses Bachelier delta for Normal quotes and Black delta for Black quotes,
    /// applying the displacement for shifted Black. The inversion uses ATM volatility.
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
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if any violation is found,
    /// the grid is not finite and strictly ascending, `r` is non-finite, or the
    /// smile cannot be evaluated on the grid.
    /// @param strikes - Finite, strictly ascending strikes with arbitrary spacing.
    /// @param r - Finite continuously compounded risk-free rate (decimal) that discounts the forward-based call prices in the selected model.
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
    /// @returns Repaired vols aligned with `strikes`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for a non-integer `maxIterations`, and a
    /// `FinstackError` (kind `validation`) if the grid is not finite and
    /// strictly ascending, `r` is non-finite, or the smile or a price
    /// inversion fails.
    /// @param strikes - Finite, strictly ascending strikes in the model domain.
    /// @param r - Finite continuously compounded risk-free rate (decimal) that discounts the forward-based call prices in the selected model.
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

wasm_getters!(JsSabrCalibrator, SabrCalibrator |s| {
    /// Maximum accepted relative error of every fitted volatility quote.
    tolerance -> f64 = s.inner.tolerance();
    /// Iteration cap before the solver reports non-convergence.
    max_iterations as maxIterations -> usize = s.inner.max_iterations();
    /// Displacement policy: `null` (no shift), a fixed numeric shift in the
    /// forward's units, or `"auto"` — the same forms `withShift` accepts.
    shift returns "number | 'auto' | null" -> Result<JsValue, JsValue> = to_js_value(&s.inner.shift());
    /// Whether alpha is pinned to the interpolated ATM volatility.
    atm_pinning as atmPinning -> bool = s.inner.atm_pinning();
});

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
    /// @param marketVols - Positive quotes aligned with strikes: absolute decimal-rate Normal volatilities or annual Black volatilities in relative decimals, as selected.
    /// @param t - Positive time to expiry in years.
    /// @param beta - SABR CEV elasticity in `[0, 1]`, held fixed during calibration.
    /// @param quoteType - Input annualized Normal or Black quote type. Shifted Black requires a selected displacement.
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
        #[wasm_bindgen(
            js_name = "quoteType",
            unchecked_param_type = "\"normal\" | \"black_lognormal\" | \"shifted_black_lognormal\""
        )]
        quote_type: JsValue,
    ) -> Result<JsValue, JsValue> {
        let forward = js_f64(&forward, "forward")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let market_vols = js_f64_seq(&market_vols, "marketVols")?;
        let t = js_f64(&t, "t")?;
        let beta = js_f64(&beta, "beta")?;
        self.inner
            .calibrate(
                forward,
                &strikes,
                &market_vols,
                t,
                beta,
                js_wire(&quote_type, "quoteType")?,
            )
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
}

impl Default for JsSabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}
