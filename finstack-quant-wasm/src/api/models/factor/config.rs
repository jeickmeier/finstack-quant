//! WASM bindings for crate-root factor covariance and configuration helpers.
//!
//! Exposed on `models.factor`. Both `FactorCovarianceMatrix` and
//! `FactorModelConfig` cross the boundary as plain objects, so their Rust
//! methods are free functions here.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_factor::{FactorCovarianceMatrix, FactorId, FactorModelConfig};
use wasm_bindgen::prelude::*;

/// Variance of one factor in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param factorId - Factor whose diagonal variance is requested; an unknown identifier returns 0.
/// @returns The factor's variance.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorVariance)]
pub fn factor_variance(
    #[wasm_bindgen(unchecked_param_type = "FactorCovarianceMatrix | string")] matrix: JsValue,
    #[wasm_bindgen(js_name = "factorId", unchecked_param_type = "string")] factor_id: JsValue,
) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.variance(&FactorId::new(js_string(&factor_id, "factorId")?)))
}

/// Covariance between two factors in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param lhs - First factor identifier; an unknown identifier returns 0.
/// @param rhs - Second factor identifier; an unknown identifier returns 0.
/// @returns The covariance of the two factors.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCovariance)]
pub fn factor_covariance(
    #[wasm_bindgen(unchecked_param_type = "FactorCovarianceMatrix | string")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] lhs: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] rhs: JsValue,
) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.covariance(
        &FactorId::new(js_string(&lhs, "lhs")?),
        &FactorId::new(js_string(&rhs, "rhs")?),
    ))
}

/// Correlation between two factors in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param lhs - First factor identifier; an unknown identifier or a non-positive variance returns 0.
/// @param rhs - Second factor identifier; an unknown identifier or a non-positive variance returns 0.
/// @returns The correlation of the two factors, from -1 through 1.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCorrelation)]
pub fn factor_correlation(
    #[wasm_bindgen(unchecked_param_type = "FactorCovarianceMatrix | string")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] lhs: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] rhs: JsValue,
) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.correlation(
        &FactorId::new(js_string(&lhs, "lhs")?),
        &FactorId::new(js_string(&rhs, "rhs")?),
    ))
}

/// Factor covariance matrix as one row per factor (the twin of Python `to_numpy`).
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @returns An array of `number[]` rows, `nFactors` by `nFactors`, in `factor_ids` order.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCovarianceRows, unchecked_return_type = "number[][]")]
pub fn factor_covariance_rows(
    #[wasm_bindgen(unchecked_param_type = "FactorCovarianceMatrix | string")] matrix: JsValue,
) -> Result<JsValue, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    to_js_value(&matrix.to_rows())
}

/// Validate a factor-model configuration: factor ordering, matching rules and the risk measure.
/// @param config - `FactorModelConfig` object or JSON, as returned by the credit factor model `config` getter or `factorModelAt`.
///
/// # Errors
///
/// Throws a `validation` error naming the first inconsistency.
#[wasm_bindgen(js_name = validateFactorModelConfig)]
pub fn validate_factor_model_config(
    #[wasm_bindgen(unchecked_param_type = "FactorModelConfig | string")] config: JsValue,
) -> Result<(), JsValue> {
    let config: FactorModelConfig = from_js_json(&config, "config")?;
    config.validate().map_err(to_js_err)
}
