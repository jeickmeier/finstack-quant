//! WASM bindings for equality-constrained least-squares regression.
//!
//! Binds `finstack_quant_analytics::regression::constrained_least_squares`
//! (Jeet & Partani 2023, Appendix A). Unlike the JSON-shim pattern used
//! elsewhere in this crate's bindings, this is a plain numeric API: no
//! portfolio, sector, or attribution concepts are involved, so numeric
//! vectors travel as `Float64Array` rather than through a JSON envelope.

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
/// @param nFactors - Number of factor columns in `exposures`; must be a positive integer no greater than `4294967295`.
/// @param returns - Realized asset returns, length `n_assets` (defines `n_assets`).
/// @param weights - Holding weights whose weighted return `w'r` must be fully reproduced by `w'Xf` (e.g. benchmark weights for a benchmark-return attribution).
/// @returns Constrained factor returns `f`, one per factor, satisfying `w'Xf = w'r` to numerical precision.
/// @throws Error - If `nFactors` is non-finite, fractional, zero, negative, or exceeds the WebAssembly `usize` range; if vector dimensions are inconsistent (including an overflowing `n_assets * n_factors`); if any vector value is non-finite; if the design matrix is rank-deficient; if coefficient rescaling or the constraint correction produces a non-finite value; or if the correction direction is degenerate and OLS does not already satisfy the constraint.
#[wasm_bindgen(js_name = constrainedLeastSquares)]
pub fn constrained_least_squares(
    exposures: JsValue,
    n_factors: JsValue,
    returns: JsValue,
    weights: JsValue,
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
