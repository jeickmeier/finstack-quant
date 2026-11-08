//! WASM binding for equality-constrained least-squares regression.
//!
//! Binds `finstack_quant_analytics::regression::constrained_least_squares`
//! (Jeet & Partani 2023, Appendix A) as `analytics.constrainedLeastSquares`:
//! numeric arrays in, a `Float64Array` of factor returns out.

use crate::utils::input::{js_f64_seq, js_uint};
use js_sys::Float64Array;
use wasm_bindgen::prelude::*;

use crate::utils::to_js_err;

/// Fit factor returns satisfying the equality constraint `w'Xf = w'r`.
///
/// Binds Rust `constrained_least_squares`: adds the minimal Lagrangian
/// correction to an unconstrained OLS fit so the corrected factor returns
/// exactly reproduce the weighted realized return `w'r`. Typically used to
/// fit the benchmark factor returns consumed by
/// `portfolio.factorBrinsonAttribution`, which requires factor returns
/// satisfying that same completeness condition.
/// @param exposures - Row-major factor exposure matrix, `n_assets x n_factors`: asset i's exposure to factor j is `exposures[i * n_factors + j]`.
/// @param nFactors - Number of factor columns in `exposures`; a non-negative whole number no greater than `4294967295` (Rust rejects `0` as invalid input).
/// @param returns - Realized asset returns, length `n_assets` (defines `n_assets`).
/// @param weights - Holding weights whose weighted return `w'r` must be fully reproduced by `w'Xf` (e.g. benchmark weights for a benchmark-return attribution).
/// @returns Constrained factor returns `f`, one per factor, satisfying `w'Xf = w'r` to numerical precision.
/// @throws Error - A `TypeError` (kind `invalid_type`) if `nFactors` is not a finite, non-negative whole number within the WebAssembly `usize` range. A validation error from Rust if `nFactors` is zero or `returns` is empty; if vector dimensions are inconsistent (including an overflowing `n_assets * n_factors`); if any vector value is non-finite; if the design matrix is rank-deficient; if coefficient rescaling or the constraint correction produces a non-finite value; or if the correction direction is degenerate and OLS does not already satisfy the constraint.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// console.log(analytics.constrainedLeastSquares([1, 2], 1, [0.01, 0.02], [0.5, 0.5])[0]); // 0.01
/// ```
#[wasm_bindgen(js_name = constrainedLeastSquares)]
pub fn constrained_least_squares(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] exposures: JsValue,
    #[wasm_bindgen(js_name = "nFactors", unchecked_param_type = "number")] n_factors: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] weights: JsValue,
) -> Result<Float64Array, JsValue> {
    let n_factors: usize = js_uint(&n_factors, "nFactors")?;
    let exposures = js_f64_seq(&exposures, "exposures")?;
    let returns = js_f64_seq(&returns, "returns")?;
    let weights = js_f64_seq(&weights, "weights")?;
    let f = finstack_quant_analytics::regression::constrained_least_squares(
        &exposures, n_factors, &returns, &weights,
    )
    .map_err(to_js_err)?;
    Ok(Float64Array::from(f.as_slice()))
}

/// Equal-weight raw multifactor intercept OLS with explicit response interpretation.
///
/// Factors remain caller-prepared and are never automatically cash-adjusted.
/// This raw API accepts finite statistical responses, including already-excess
/// returns below -1, whereas the Performance adapter owns total-investment panels.
/// @param returns - Finite response observations in exact caller-aligned order.
/// @param factorReturns - One finite row per already-prepared factor, each with exactly the response observation count.
/// @param periodsPerYear - Required finite positive observation frequency for annualized intercept and residual sample risk.
/// @param returnKind - Required exact label `"excess"` or `"total"`; excess leaves response unchanged and total subtracts geometric periodic cash from response only.
/// @param riskFreeRate - Annual-effective decimal cash rate greater than -1, required for total and forbidden for excess; omit for excess.
/// @returns MultiFactorResult with annualized alpha, raw factor slopes, R-squared, adjusted R-squared, residual sample volatility and observation count; undefined field-specific estimates are numeric NaN.
/// @throws Error - Rejects invalid labels/cash/frequency, misaligned or nonfinite samples, empty factors, deficient normalized numerical rank, or unrepresentable requested results.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// const result = analytics.multiFactorGreeks([0.03, 0.01, 0.05], [[0.01, 0, 0.02]], 12, "excess");
/// console.log(result.betas[0]); // 2
/// ```
#[wasm_bindgen(js_name = multiFactorGreeks, unchecked_return_type = "MultiFactorResult")]
pub fn multi_factor_greeks(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
    #[wasm_bindgen(js_name = "factorReturns", unchecked_param_type = "NumericMatrix")]
    factor_returns: JsValue,
    #[wasm_bindgen(js_name = "periodsPerYear", unchecked_param_type = "number")]
    periods_per_year: JsValue,
    #[wasm_bindgen(js_name = "returnKind", unchecked_param_type = "string")] return_kind: JsValue,
    #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
    risk_free_rate: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    use crate::utils::input::{js_f64, js_f64_matrix, js_opt_f64, js_string};

    let returns = js_f64_seq(&returns, "returns")?;
    let factors = js_f64_matrix(&factor_returns, "factorReturns")?;
    let refs: Vec<&[f64]> = factors.iter().map(Vec::as_slice).collect();
    let periods_per_year = js_f64(&periods_per_year, "periodsPerYear")?;
    let return_kind = js_string(&return_kind, "returnKind")?;
    let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
    let kind = finstack_quant_analytics::ReturnKind::from_label(&return_kind, risk_free_rate)
        .map_err(to_js_err)?;
    let result =
        finstack_quant_analytics::multi_factor_greeks(&returns, &refs, periods_per_year, kind)
            .map_err(to_js_err)?;
    crate::utils::to_js_value_numeric(&result)
}
