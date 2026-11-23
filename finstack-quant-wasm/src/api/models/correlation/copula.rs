//! Copula specification and copula handle WASM bindings.

//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::{js_f64, js_f64_seq};
use crate::utils::to_js_err;
use finstack_quant_models::correlation::{Copula, CopulaSpec};
use wasm_bindgen::prelude::*;

/// Copula model specification for configuration and deferred construction.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const copula = models.correlation.CopulaSpec.gaussian().build();
/// console.log(copula.modelName, copula.numFactors);
/// ```
#[wasm_bindgen(js_name = CopulaSpec)]
pub struct JsCopulaSpec {
    inner: CopulaSpec,
}

json_round_trip!(JsCopulaSpec, CopulaSpec);

#[wasm_bindgen(js_class = CopulaSpec)]
impl JsCopulaSpec {
    /// One-factor Gaussian copula (market standard).
    /// @returns A `CopulaSpec` handle for deferred construction.
    #[wasm_bindgen(js_name = gaussian)]
    pub fn gaussian() -> Self {
        Self {
            inner: CopulaSpec::gaussian(),
        }
    }

    /// Student-t copula with specified degrees of freedom (must be > 2).
    /// @param degreesOfFreedom - Student-t degrees of freedom controlling tail thickness; finite and strictly greater than two.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `degreesOfFreedom` is not finite and
    /// strictly greater than two.
    /// @returns A `CopulaSpec` handle for deferred construction.
    #[wasm_bindgen(js_name = studentT)]
    pub fn student_t(
        #[wasm_bindgen(js_name = "degreesOfFreedom", unchecked_param_type = "number")]
        degrees_of_freedom: JsValue,
    ) -> Result<JsCopulaSpec, JsValue> {
        let degrees_of_freedom = js_f64(&degrees_of_freedom, "degreesOfFreedom")?;
        CopulaSpec::student_t(degrees_of_freedom)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Random Factor Loading copula with stochastic correlation.
    /// @param loadingVol - Factor-loading volatility (standard deviation of the loading); clamped to `[0, 0.5]`, typically 0.05 to 0.20.
    /// @returns A `CopulaSpec` handle for deferred construction.
    #[wasm_bindgen(js_name = randomFactorLoading)]
    pub fn random_factor_loading(
        #[wasm_bindgen(js_name = "loadingVol", unchecked_param_type = "number")]
        loading_vol: JsValue,
    ) -> Result<Self, JsValue> {
        let loading_vol = js_f64(&loading_vol, "loadingVol")?;
        Ok(Self {
            inner: CopulaSpec::random_factor_loading(loading_vol),
        })
    }

    /// Global-plus-sector two-factor Gaussian copula.
    /// @returns A `CopulaSpec` handle for deferred construction.
    #[wasm_bindgen(js_name = multiFactor)]
    pub fn multi_factor() -> Self {
        Self {
            inner: CopulaSpec::multi_factor(),
        }
    }

    /// Build a concrete copula from this specification.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if a Student-t specification contains
    /// non-finite degrees of freedom or a value at most two, or random-loading
    /// volatility is non-finite or outside `[0, 0.5]`.
    /// @returns A concrete `Copula` handle.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsCopula, JsValue> {
        self.inner
            .build()
            .map(|inner| JsCopula { inner })
            .map_err(to_js_err)
    }
}

wasm_getters!(JsCopulaSpec, CopulaSpec |s| {
    /// True if this is a Gaussian spec.
    is_gaussian as isGaussian -> bool = s.inner.is_gaussian();
    /// True if this is a Student-t spec.
    is_student_t as isStudentT -> bool = s.inner.is_student_t();
    /// True if this is a Random Factor Loading spec.
    is_rfl as isRfl -> bool = s.inner.is_rfl();
    /// True if this is a Multi-factor spec.
    is_multi_factor as isMultiFactor -> bool = s.inner.is_multi_factor();
});

/// Concrete copula model for portfolio default correlation.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const copula = models.correlation.CopulaSpec.gaussian().build();
/// console.log(copula.modelName, copula.numFactors);
/// copula.free();
/// ```
#[wasm_bindgen(js_name = Copula)]
pub struct JsCopula {
    inner: Box<dyn Copula + Send + Sync>,
}

wasm_getters!(JsCopula, Copula |s| {
    /// Number of systematic factors in the model.
    num_factors as numFactors -> usize = s.inner.num_factors();
    /// Model name for diagnostics.
    model_name as modelName -> String = s.inner.model_name().to_string();
});

#[wasm_bindgen(js_class = Copula)]
impl JsCopula {
    /// Conditional default probability given factor realization(s).
    /// @param defaultThreshold - Latent-variable default threshold corresponding to the marginal default probability.
    /// @param factorRealization - Realized systematic-factor value conditioning the default probability.
    /// @param correlation - Asset correlation as a decimal in `[0, 1]`; values outside this range throw.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the factor count does not match the
    /// copula, any input is non-finite, `correlation` is outside `[0, 1]`, or
    /// the model produces a probability outside `[0, 1]`.
    /// @returns Conditional default probability in `[0, 1]`.
    #[wasm_bindgen(js_name = conditionalDefaultProb)]
    pub fn conditional_default_prob(
        &self,
        #[wasm_bindgen(js_name = "defaultThreshold", unchecked_param_type = "number")]
        default_threshold: JsValue,
        #[wasm_bindgen(js_name = "factorRealization", unchecked_param_type = "number[]")]
        factor_realization: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<f64, JsValue> {
        let default_threshold = js_f64(&default_threshold, "defaultThreshold")?;
        let factor_realization: &[f64] = &js_f64_seq(&factor_realization, "factorRealization")?;
        let correlation = js_f64(&correlation, "correlation")?;
        self.inner
            .conditional_default_prob_checked(default_threshold, factor_realization, correlation)
            .map_err(to_js_err)
    }

    /// Strict lower-tail dependence coefficient `λ_L` at the given
    /// correlation.
    ///
    /// Random Factor Loading returns the mass at unit loading under its
    /// calibrated clipped-normal loading distribution. Gaussian models have
    /// zero tail dependence except at perfect correlation.
    /// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
    /// @returns Lower-tail dependence `λ_L` in `[0, 1]` for valid inputs; a non-finite `correlation` yields `NaN`.
    #[wasm_bindgen(js_name = tailDependence)]
    pub fn tail_dependence(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<f64, JsValue> {
        let correlation = js_f64(&correlation, "correlation")?;
        Ok(self.inner.tail_dependence(correlation))
    }

    /// Heuristic stress-correlation proxy for the Random Factor Loading
    /// copula.
    ///
    /// This is **not** the strict copula lower-tail-dependence coefficient
    /// `λ_L` returned by `tailDependence`. It gauges the extra correlation mass in the high-loading
    /// tail and vanishes in the Gaussian (`loadingVol = 0`) limit.
    /// @param correlation - Base asset correlation as a decimal in `[0, 1]`; out-of-range values are clamped to `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if this copula is not a Random Factor
    /// Loading model.
    /// @returns Heuristic extra correlation mass in the high-loading tail; 0 in the Gaussian limit.
    #[wasm_bindgen(js_name = stressCorrelationProxy)]
    pub fn stress_correlation_proxy(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<f64, JsValue> {
        let correlation = js_f64(&correlation, "correlation")?;
        self.inner
            .stress_correlation_proxy(correlation)
            .map_err(to_js_err)
    }
}
