//! Latent-factor correlation model WASM handles.

//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::{js_f64, js_f64_seq, js_uint};
use crate::utils::to_js_err;
use finstack_quant_models::correlation::{
    LatentFactorKind, LatentFactorSpec, LatentMultiFactor, LatentSingleFactor, LatentTwoFactor,
};
use wasm_bindgen::prelude::*;

/// Latent-factor model specification for deferred construction.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const spec = models.correlation.LatentFactorSpec.twoFactor(0.2, 0.25, -0.3);
/// const kind = spec.build();
/// console.log(kind.modelName, kind.numFactors, kind.volatilities);
/// ```
#[wasm_bindgen(js_name = LatentFactorSpec)]
pub struct JsLatentFactorSpec {
    inner: LatentFactorSpec,
}

#[wasm_bindgen(js_class = LatentFactorSpec)]
impl JsLatentFactorSpec {
    /// One mean-reverting systematic factor.
    /// @param volatility - Annualized factor volatility as a decimal; non-negative.
    /// @param meanReversion - Mean-reversion speed per year; non-negative.
    /// @returns The single-factor specification.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, or a `validation`
    /// error if a model input is non-finite or outside its documented range.
    #[wasm_bindgen(js_name = singleFactor)]
    pub fn single_factor(
        #[wasm_bindgen(unchecked_param_type = "number")] volatility: JsValue,
        #[wasm_bindgen(js_name = "meanReversion", unchecked_param_type = "number")] mean_reversion: JsValue,
    ) -> Result<JsLatentFactorSpec, JsValue> {
        Ok(Self {
            inner: LatentFactorSpec::single_factor(
                js_f64(&volatility, "volatility")?,
                js_f64(&mean_reversion, "meanReversion")?,
            )
            .map_err(to_js_err)?,
        })
    }

    /// Correlated prepayment and credit factors.
    /// @param prepayVol - Annualized prepayment-factor volatility as a decimal; non-negative.
    /// @param creditVol - Annualized credit-factor volatility as a decimal; non-negative.
    /// @param correlation - Correlation between the two factors, from -1 through 1.
    /// @returns The two-factor specification.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, or a `validation`
    /// error if a model input is non-finite or outside its documented range.
    #[wasm_bindgen(js_name = twoFactor)]
    pub fn two_factor(
        #[wasm_bindgen(js_name = "prepayVol", unchecked_param_type = "number")] prepay_vol: JsValue,
        #[wasm_bindgen(js_name = "creditVol", unchecked_param_type = "number")] credit_vol: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<JsLatentFactorSpec, JsValue> {
        Ok(Self {
            inner: LatentFactorSpec::two_factor(
                js_f64(&prepay_vol, "prepayVol")?,
                js_f64(&credit_vol, "creditVol")?,
                js_f64(&correlation, "correlation")?,
            )
            .map_err(to_js_err)?,
        })
    }

    /// Number of systematic factors the specification describes.
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Build the concrete latent-factor model.
    /// @returns The `LatentFactorKind` handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the specification contains an invalid
    /// volatility, mean-reversion speed, correlation, count or matrix.
    pub fn build(&self) -> Result<JsLatentFactorKind, JsValue> {
        self.inner
            .build()
            .map(|inner| JsLatentFactorKind { inner })
            .map_err(to_js_err)
    }
}

/// Concrete latent-factor model built from a `LatentFactorSpec`.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const kind = models.correlation.LatentFactorSpec.twoFactor(0.2, 0.25, -0.3).build();
/// console.log(kind.modelName, kind.numFactors);
/// kind.free();
/// ```
#[wasm_bindgen(js_name = LatentFactorKind)]
pub struct JsLatentFactorKind {
    inner: LatentFactorKind,
}

wasm_getters!(JsLatentFactorKind, LatentFactorKind |s| {
    /// Number of systematic factors.
    num_factors as numFactors -> usize = s.inner.num_factors();
    /// Factor correlation matrix, flat row-major (`numFactors * numFactors`).
    correlation_matrix as correlationMatrix -> Box<[f64]> = s.inner.correlation_matrix().into();
    /// Annualized factor volatilities, one per factor.
    volatilities -> Box<[f64]> = s.inner.volatilities().into();
    /// Descriptive factor names, one per factor.
    factor_names as factorNames -> Vec<String> = s
        .inner
        .factor_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    /// Model name for diagnostics.
    model_name as modelName -> String = s.inner.model_name().to_string();
});

#[wasm_bindgen(js_class = LatentFactorKind)]
impl JsLatentFactorKind {
    /// Contribution of one factor's own (diagonal) shock to its value.
    /// @param factorIndex - Zero-based factor index; an index beyond the model returns 0.
    /// @param z - Independent standard normal draw for that factor.
    /// @returns The diagonal Cholesky loading times `z` times the factor volatility.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `factorIndex` is not a safe non-negative integer
    /// or `z` is not a number.
    #[wasm_bindgen(js_name = diagonalFactorContribution)]
    pub fn diagonal_factor_contribution(
        &self,
        #[wasm_bindgen(js_name = "factorIndex", unchecked_param_type = "number")]
        factor_index: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] z: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .diagonal_factor_contribution(js_uint(&factor_index, "factorIndex")?, js_f64(&z, "z")?))
    }
}

/// Single mean-reverting latent factor.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const factor = new models.correlation.LatentSingleFactor(0.25, 0.1);
/// console.log(factor.volatility, factor.meanReversion, factor.numFactors);
/// factor.free();
/// ```
#[wasm_bindgen(js_name = LatentSingleFactor)]
pub struct JsLatentSingleFactor {
    inner: LatentSingleFactor,
}

#[wasm_bindgen(js_class = LatentSingleFactor)]
impl JsLatentSingleFactor {
    /// Single-factor model; inputs are validated by Rust.
    /// @param volatility - Annualized factor volatility as a decimal; non-negative.
    /// @param meanReversion - Mean-reversion speed per year; non-negative.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, or a `validation`
    /// error if a model input is non-finite or outside its documented range.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] volatility: JsValue,
        #[wasm_bindgen(js_name = "meanReversion", unchecked_param_type = "number")] mean_reversion: JsValue,
    ) -> Result<JsLatentSingleFactor, JsValue> {
        Ok(Self {
            inner: LatentSingleFactor::new(
                js_f64(&volatility, "volatility")?,
                js_f64(&mean_reversion, "meanReversion")?,
            )
            .map_err(to_js_err)?,
        })
    }
}

wasm_getters!(JsLatentSingleFactor, LatentSingleFactor |s| {
    /// Annualized factor volatility, as a decimal.
    volatility -> f64 = s.inner.volatility();
    /// Mean-reversion speed per year.
    mean_reversion as meanReversion -> f64 = s.inner.mean_reversion();
    /// Number of systematic factors (always 1).
    num_factors as numFactors -> usize = s.inner.num_factors();
});

/// Correlated prepayment and credit latent factors.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const factors = models.correlation.LatentTwoFactor.rmbsStandard();
/// console.log(factors.prepayVol, factors.creditVol, factors.correlation, factors.choleskyL11);
/// factors.free();
/// ```
#[wasm_bindgen(js_name = LatentTwoFactor)]
pub struct JsLatentTwoFactor {
    inner: LatentTwoFactor,
}

#[wasm_bindgen(js_class = LatentTwoFactor)]
impl JsLatentTwoFactor {
    /// Two-factor model; inputs are validated by Rust.
    /// @param prepayVol - Annualized prepayment-factor volatility as a decimal; non-negative.
    /// @param creditVol - Annualized credit-factor volatility as a decimal; non-negative.
    /// @param correlation - Correlation between the two factors, from -1 through 1.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, or a `validation`
    /// error if a model input is non-finite or outside its documented range.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "prepayVol", unchecked_param_type = "number")] prepay_vol: JsValue,
        #[wasm_bindgen(js_name = "creditVol", unchecked_param_type = "number")] credit_vol: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<JsLatentTwoFactor, JsValue> {
        Ok(Self {
            inner: LatentTwoFactor::new(
                js_f64(&prepay_vol, "prepayVol")?,
                js_f64(&credit_vol, "creditVol")?,
                js_f64(&correlation, "correlation")?,
            )
            .map_err(to_js_err)?,
        })
    }

    /// Standard RMBS calibration of the two-factor model.
    /// @returns The RMBS-standard prepayment/credit factor model.
    #[wasm_bindgen(js_name = rmbsStandard)]
    pub fn rmbs_standard() -> Result<JsLatentTwoFactor, JsValue> {
        LatentTwoFactor::rmbs_standard()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Standard CLO calibration of the two-factor model.
    /// @returns The CLO-standard prepayment/credit factor model.
    #[wasm_bindgen(js_name = cloStandard)]
    pub fn clo_standard() -> Result<JsLatentTwoFactor, JsValue> {
        LatentTwoFactor::clo_standard()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }
}

wasm_getters!(JsLatentTwoFactor, LatentTwoFactor |s| {
    /// Annualized prepayment-factor volatility, as a decimal.
    prepay_vol as prepayVol -> f64 = s.inner.prepay_vol();
    /// Annualized credit-factor volatility, as a decimal.
    credit_vol as creditVol -> f64 = s.inner.credit_vol();
    /// Correlation between the prepayment and credit factors.
    correlation -> f64 = s.inner.correlation();
    /// Number of systematic factors (always 2).
    num_factors as numFactors -> usize = s.inner.num_factors();
    /// Off-diagonal Cholesky entry `L[1,0]`, equal to the correlation.
    cholesky_l10 as choleskyL10 -> f64 = s.inner.cholesky_l10();
    /// Diagonal Cholesky entry `L[1,1]`, equal to `sqrt(1 - correlation^2)`.
    cholesky_l11 as choleskyL11 -> f64 = s.inner.cholesky_l11();
});

/// General correlated latent-factor model with a Cholesky factorization.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const model = new models.correlation.LatentMultiFactor(2, [0.2, 0.3], [1, 0.5, 0.5, 1]);
/// console.log(model.generateCorrelatedFactors([1, -1]));
/// model.free();
/// ```
#[wasm_bindgen(js_name = LatentMultiFactor)]
pub struct JsLatentMultiFactor {
    inner: LatentMultiFactor,
}

wasm_getters!(JsLatentMultiFactor, LatentMultiFactor |s| {
    /// Number of systematic factors.
    num_factors as numFactors -> usize = s.inner.num_factors();
    /// Factor correlation matrix, flat row-major.
    correlation_matrix as correlationMatrix -> Box<[f64]> = s.inner.correlation_matrix().into();
    /// Annualized factor volatilities, one per factor.
    volatilities -> Box<[f64]> = s.inner.volatilities().into();
});

#[wasm_bindgen(js_class = LatentMultiFactor)]
impl JsLatentMultiFactor {
    /// Multi-factor model from volatilities and a correlation matrix.
    /// @param numFactors - Number of systematic factors; a positive safe integer.
    /// @param volatilities - Annualized factor volatilities, one per factor; non-negative.
    /// @param correlations - Flat row-major `numFactors * numFactors` correlation matrix; symmetric, unit diagonal, positive semidefinite.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the lengths disagree with `numFactors`,
    /// a volatility is negative or non-finite, or the matrix is not a valid
    /// correlation matrix.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "numFactors", unchecked_param_type = "number")]
        num_factors: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] volatilities: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] correlations: JsValue,
    ) -> Result<JsLatentMultiFactor, JsValue> {
        LatentMultiFactor::new(
            js_uint(&num_factors, "numFactors")?,
            js_f64_seq(&volatilities, "volatilities")?,
            js_f64_seq(&correlations, "correlations")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Model with independent factors (identity correlation).
    /// @param numFactors - Positive number of systematic factors; zero is rejected.
    /// @param volatilities - Annualized factor volatilities, one per factor; must be finite, non-negative and match the factor count.
    /// @returns The uncorrelated multi-factor model.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `numFactors` is not a safe non-negative integer
    /// or `volatilities` is not an array of numbers. Throws a validation error for a zero factor count, a length mismatch, or negative or non-finite volatility.
    pub fn uncorrelated(
        #[wasm_bindgen(js_name = "numFactors", unchecked_param_type = "number")]
        num_factors: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] volatilities: JsValue,
    ) -> Result<JsLatentMultiFactor, JsValue> {
        LatentMultiFactor::uncorrelated(
            js_uint(&num_factors, "numFactors")?,
            js_f64_seq(&volatilities, "volatilities")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Turn independent standard normal draws into correlated, volatility-scaled factors.
    /// @param independentZ - Independent standard normal draws, exactly one per factor.
    /// @returns The correlated factor values, one per factor.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `independentZ` does not hold exactly
    /// `numFactors` entries.
    #[wasm_bindgen(js_name = generateCorrelatedFactors)]
    pub fn generate_correlated_factors(
        &self,
        #[wasm_bindgen(js_name = "independentZ", unchecked_param_type = "NumericArray")]
        independent_z: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .generate_correlated_factors(&js_f64_seq(&independent_z, "independentZ")?)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }
}
