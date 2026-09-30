//! WASM bindings for `finstack_quant_core::math` — linear algebra, statistics,
//! special functions, and compensated summation.

use crate::utils::input::{js_f64, js_f64_seq, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::math::{self, linalg, special_functions, stats, summation};
use wasm_bindgen::prelude::*;

/// Cholesky decomposition for a flat row-major matrix.
///
/// Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
/// and returns a flat lower-triangular factor.
/// @param matrix - Flat row-major `n * n` entries of a symmetric
///   positive-definite matrix.
/// @param n - Positive square-matrix dimension; `matrix` must contain exactly
///   `n * n` entries.
/// @returns Lower-triangular factor L as a flat row-major `Float64Array`.
///
/// # Errors
///
/// Throws a JavaScript exception if `matrix` does not contain exactly `n * n`
/// entries (including when `n * n` overflows), or the matrix contains a
/// non-finite value, is singular, or is not positive definite.
#[wasm_bindgen(js_name = choleskyDecomposition)]
pub fn cholesky_decomposition(matrix: JsValue, n: JsValue) -> Result<Box<[f64]>, JsValue> {
    let matrix: &[f64] = &js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    linalg::cholesky_decomposition(matrix, n)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Solve a symmetric positive-definite linear system from a flat Cholesky factor.
///
/// The system dimension is `b.length`, as in Rust `cholesky_solve` and Python
/// `cholesky_solve(chol, b)`.
/// @param chol - Lower-triangular Cholesky factor as a flat row-major array of
///   `b.length * b.length` entries.
/// @param b - Right-hand-side vector of the linear system; its length is the system dimension.
/// @returns Solution vector `x` of `L Lᵀ x = b`, with the same length as `b`.
///
/// # Errors
///
/// Throws a JavaScript exception if `chol` does not contain exactly
/// `b.length * b.length` entries or a diagonal factor is singular.
#[wasm_bindgen(js_name = choleskySolve)]
pub fn cholesky_solve(chol: JsValue, b: JsValue) -> Result<Box<[f64]>, JsValue> {
    let chol: &[f64] = &js_f64_seq(&chol, "chol")?;
    let b: &[f64] = &js_f64_seq(&b, "b")?;
    let mut x = vec![0.0; b.len()];
    linalg::cholesky_solve(chol, b, &mut x).map_err(to_js_err)?;
    Ok(x.into_boxed_slice())
}

/// Apply a lower-triangular factor L to a vector z, returning `L z`.
///
/// This is the Cholesky "apply" step that turns independent standard normals
/// into correlated normals: if `A = L L^T` and `z ~ N(0, I)`, then
/// `L z ~ N(0, A)`. Accepts L as `n * n` row-major entries; only the lower
/// triangle is read and the upper triangle is assumed zero.
/// @param l - Lower-triangular Cholesky factor as a flat row-major array of n × n entries.
/// @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
/// @param z - Vector of length n to transform, typically independent standard-normal draws.
///
/// # Errors
///
/// Throws a JavaScript exception if `l` does not contain exactly `n * n`
/// entries (including when `n * n` overflows) or `z` does not contain exactly
/// `n` entries.
#[wasm_bindgen(js_name = applyLowerTriangular)]
pub fn apply_lower_triangular(l: JsValue, n: JsValue, z: JsValue) -> Result<Box<[f64]>, JsValue> {
    let l: &[f64] = &js_f64_seq(&l, "l")?;
    let z: &[f64] = &js_f64_seq(&z, "z")?;
    let n: usize = js_uint(&n, "n")?;
    linalg::apply_lower_triangular(l, n, z)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Arithmetic mean over a typed numeric array.
/// @param data - Numeric observations in input order; an empty series yields 0.0.
/// @returns Arithmetic mean of `data`, or 0.0 when `data` is empty.
#[wasm_bindgen(js_name = mean)]
pub fn mean(data: JsValue) -> Result<f64, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    Ok(stats::mean(data))
}

/// Sample variance over a typed numeric array.
/// @param data - Sample observations in input order; fewer than two points yield 0.0.
/// @returns Unbiased sample variance, or 0.0 when `data` has fewer than two points.
#[wasm_bindgen(js_name = variance)]
pub fn variance(data: JsValue) -> Result<f64, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    Ok(stats::variance(data))
}

/// Population variance over a typed numeric array.
/// @param data - Observations in input order; fewer than two points yield 0.0.
/// @returns Population variance, or 0.0 when `data` has fewer than two points.
#[wasm_bindgen(js_name = populationVariance)]
pub fn population_variance(data: JsValue) -> Result<f64, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    Ok(stats::population_variance(data))
}

/// Pearson correlation over typed numeric arrays.
/// @param x - First numeric series; must have the same length as `y`.
/// @param y - Second numeric series, aligned one-for-one with `x`.
/// @returns Sample correlation in `[-1, 1]`, or NaN when a series has fewer than two points.
#[wasm_bindgen(js_name = correlation)]
pub fn correlation(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    Ok(stats::correlation(x, y))
}

/// Sample covariance over typed numeric arrays.
/// @param x - First numeric series; must have the same length as `y`.
/// @param y - Second numeric series, aligned one-for-one with `x`.
/// @returns Unbiased sample covariance, or 0.0 when a series has fewer than two points.
#[wasm_bindgen(js_name = covariance)]
pub fn covariance(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    Ok(stats::covariance(x, y))
}

/// Empirical quantile over a typed numeric array.
/// @param data - Sample observations in input order; empty or non-finite data yields NaN.
/// @param q - Quantile probability in `[0, 1]`; values outside that range yield NaN.
/// @returns R-7 interpolated quantile, or NaN when `data` is empty or non-finite.
#[wasm_bindgen(js_name = quantile)]
pub fn quantile(data: JsValue, q: JsValue) -> Result<f64, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    let q = js_f64(&q, "q")?;
    let mut v = data.to_vec();
    Ok(stats::quantile(&mut v, q))
}

/// Standard normal CDF Φ(x).
/// @param x - Real-valued point at which to evaluate Φ; any finite or infinite `x` is accepted.
/// @returns Probability in `(0, 1)` for finite `x`, with the usual ±∞ limits.
#[wasm_bindgen(js_name = normCdf)]
pub fn norm_cdf(x: JsValue) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    Ok(special_functions::norm_cdf(x))
}

/// Standard normal PDF φ(x).
/// @param x - Real-valued point at which to evaluate φ.
/// @returns Density at `x`; φ(0) is `1/sqrt(2π)`.
#[wasm_bindgen(js_name = normPdf)]
pub fn norm_pdf(x: JsValue) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    Ok(special_functions::norm_pdf(x))
}

/// Inverse standard normal CDF Φ⁻¹(p).
/// @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
/// @returns Standard-normal quantile for probability `p`.
#[wasm_bindgen(js_name = standardNormalInvCdf)]
pub fn standard_normal_inv_cdf(p: JsValue) -> Result<f64, JsValue> {
    let p = js_f64(&p, "p")?;
    Ok(special_functions::standard_normal_inv_cdf(p))
}

/// Error function erf(x).
/// @param x - Real-valued argument to erf; the function is odd, so erf(-x) = -erf(x).
/// @returns erf(x) in `(-1, 1)` for finite `x`.
#[wasm_bindgen(js_name = erf)]
pub fn erf(x: JsValue) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    Ok(special_functions::erf(x))
}

/// Natural logarithm of the Gamma function ln(Γ(x)).
/// @param x - Real argument; must be positive and away from the non-positive integers.
/// @returns ln(Γ(x)); ln(Γ(1)) is 0 and ln(Γ(n+1)) is ln(n!).
#[wasm_bindgen(js_name = lnGamma)]
pub fn ln_gamma(x: JsValue) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    Ok(special_functions::ln_gamma(x))
}

/// Kahan compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values` in input order.
#[wasm_bindgen(js_name = kahanSum)]
pub fn kahan_sum(values: JsValue) -> Result<f64, JsValue> {
    let values: &[f64] = &js_f64_seq(&values, "values")?;
    Ok(summation::kahan_sum(values.iter().copied()))
}

/// Neumaier compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values`, robust to mixed-sign cancellation.
#[wasm_bindgen(js_name = neumaierSum)]
pub fn neumaier_sum(values: JsValue) -> Result<f64, JsValue> {
    let values: &[f64] = &js_f64_seq(&values, "values")?;
    Ok(summation::neumaier_sum(values.iter().copied()))
}

/// Count the longest consecutive run of strictly positive values in a typed array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Length of the longest run of strictly positive observations.
#[wasm_bindgen(js_name = longestPositiveRun)]
pub fn longest_positive_run(values: JsValue) -> Result<usize, JsValue> {
    let values: &[f64] = &js_f64_seq(&values, "values")?;
    Ok(math::longest_positive_run(values))
}
