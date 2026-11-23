//! Correlated recovery specification and model WASM handles.

//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::js_f64;
use crate::utils::to_js_err;
use finstack_quant_models::correlation::{self as corr, RecoveryModel};
use wasm_bindgen::prelude::*;

/// Recovery model specification for configuration and deferred construction.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const recovery = models.correlation.RecoverySpec.constant(0.4).build();
/// console.log(recovery.expectedRecovery, recovery.lgd);
/// ```
#[wasm_bindgen(js_name = RecoverySpec)]
pub struct JsRecoverySpec {
    inner: corr::RecoverySpec,
}

json_round_trip!(JsRecoverySpec, RecoverySpec);

#[wasm_bindgen(js_class = RecoverySpec)]
impl JsRecoverySpec {
    /// Constant recovery rate.
    /// @param rate - Constant recovery rate expressed as a fraction from 0 through 1.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `rate` is not finite or lies outside
    /// `[0, 1]`.
    /// @returns A `RecoverySpec` handle for deferred construction.
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(
        #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    ) -> Result<JsRecoverySpec, JsValue> {
        let rate = js_f64(&rate, "rate")?;
        corr::RecoverySpec::constant(rate)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Market-correlated (Andersen-Sidenius) stochastic recovery.
    /// @param mean - Zero-total-shock recovery location in `[0, 1]`; this is not the unconditional mean.
    /// @param vol - Local shock scale, finite in `[0, 0.5]`; this is not the realized recovery standard deviation.
    /// @param correlation - Correlation between recovery and the systematic factor, from -1 through 1; finite values outside that range are rejected.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `mean` is not finite or lies outside
    /// `[0, 1]`, or if `vol` or `correlation` is non-finite or outside its supported range.
    /// @returns A `RecoverySpec` handle for deferred construction.
    #[wasm_bindgen(js_name = marketCorrelated)]
    pub fn market_correlated(
        #[wasm_bindgen(unchecked_param_type = "number")] mean: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<JsRecoverySpec, JsValue> {
        let mean = js_f64(&mean, "mean")?;
        let vol = js_f64(&vol, "vol")?;
        let correlation = js_f64(&correlation, "correlation")?;
        corr::RecoverySpec::market_correlated(mean, vol, correlation)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
    /// recovery falls in stress under the canonical low-factor-stress
    /// convention).
    /// @returns A `RecoverySpec` handle for deferred construction.
    #[wasm_bindgen(js_name = marketStandardStochastic)]
    pub fn market_standard_stochastic() -> Result<Self, JsValue> {
        corr::RecoverySpec::market_standard_stochastic()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Build a concrete recovery model from this specification.
    /// @returns The validated recovery-model handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for invalid recovery inputs, or a
    /// `computation` error if expected-recovery integration fails.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsRecoveryModel, JsValue> {
        self.inner
            .build()
            .map(|inner| JsRecoveryModel { inner })
            .map_err(to_js_err)
    }
}

/// Concrete recovery model for credit portfolio pricing.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const recovery = models.correlation.RecoverySpec.constant(0.4).build();
/// console.log(recovery.expectedRecovery, recovery.lgd); // 0.4 0.6
/// recovery.free();
/// ```
#[wasm_bindgen(js_name = RecoveryModel)]
pub struct JsRecoveryModel {
    inner: Box<dyn RecoveryModel + Send + Sync>,
}

#[wasm_bindgen(js_class = RecoveryModel)]
impl JsRecoveryModel {
    /// Expected recovery conditional on the systematic market factor, integrating the independent obligor shock.
    /// @returns Conditional mean recovery in `[0, 1]`.
    /// @throws Throws a `validation` error for a non-finite factor or a `computation` error if integration fails.
    /// @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
    #[wasm_bindgen(js_name = conditionalRecovery)]
    pub fn conditional_recovery(
        &self,
        #[wasm_bindgen(js_name = "marketFactor", unchecked_param_type = "number")]
        market_factor: JsValue,
    ) -> Result<f64, JsValue> {
        let market_factor = js_f64(&market_factor, "marketFactor")?;
        self.inner
            .conditional_recovery(market_factor)
            .map_err(to_js_err)
    }

    /// Conditional expected LGD given the systematic market factor.
    /// @returns One minus conditional mean recovery.
    /// @throws Throws a `validation` error for a non-finite factor or a `computation` error if integration fails.
    /// @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
    #[wasm_bindgen(js_name = conditionalLgd)]
    pub fn conditional_lgd(
        &self,
        #[wasm_bindgen(js_name = "marketFactor", unchecked_param_type = "number")]
        market_factor: JsValue,
    ) -> Result<f64, JsValue> {
        let market_factor = js_f64(&market_factor, "marketFactor")?;
        self.inner.conditional_lgd(market_factor).map_err(to_js_err)
    }

    /// Sample bounded recovery from the canonical logistic-normal law.
    /// @param marketFactor - Finite systematic standard-normal shock; low values represent credit stress.
    /// @param idiosyncraticNormal - Finite independent obligor standard-normal shock; ignored for constant or perfectly systematic models.
    /// @returns Sampled recovery as a decimal fraction of notional in `[0, 1]`.
    /// @throws Throws a `validation` error if either shock is non-finite.
    #[wasm_bindgen(js_name = sampleRecovery)]
    pub fn sample_recovery(
        &self,
        #[wasm_bindgen(js_name = "marketFactor", unchecked_param_type = "number")]
        market_factor: JsValue,
        #[wasm_bindgen(js_name = "idiosyncraticNormal", unchecked_param_type = "number")]
        idiosyncratic_normal: JsValue,
    ) -> Result<f64, JsValue> {
        let market_factor = js_f64(&market_factor, "marketFactor")?;
        let idiosyncratic_normal = js_f64(&idiosyncratic_normal, "idiosyncraticNormal")?;
        self.inner
            .sample_recovery(market_factor, idiosyncratic_normal)
            .map_err(to_js_err)
    }
}

wasm_getters!(JsRecoveryModel, RecoveryModel |s| {
    /// Expected (unconditional) recovery rate.
    expected_recovery as expectedRecovery -> f64 = s.inner.expected_recovery();
    /// Loss given default (1 − recovery).
    lgd -> f64 = s.inner.lgd();
    /// Whether sampling needs an independent obligor recovery draw.
    has_idiosyncratic_dispersion as hasIdiosyncraticDispersion -> bool = s.inner.has_idiosyncratic_dispersion();
    /// Local recovery shock scale in decimals, not realized recovery standard deviation.
    recovery_volatility as recoveryVolatility -> f64 = s.inner.recovery_volatility();
    /// Whether the recovery distribution is nonconstant, including obligor-only variation.
    is_stochastic as isStochastic -> bool = s.inner.is_stochastic();
    /// Model name for diagnostics.
    model_name as modelName -> String = s.inner.model_name().to_string();
});
