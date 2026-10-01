//! WASM bindings for `finstack_quant_core::math` — linear algebra, statistics,
//! special functions, and compensated summation.

use crate::utils::to_js_err;
use finstack_quant_core::math::{self, linalg, special_functions, stats, summation};
use wasm_bindgen::prelude::*;

/// Cholesky decomposition for a flat row-major matrix.
///
/// Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
/// and returns a flat lower-triangular factor.
/// Rust normalizes the matrix before factorization and restores the factor's
/// scale afterwards. Numerical singularity is measured relative to the square
/// root of the largest input diagonal magnitude. Uniformly scaling the matrix
/// by a positive constant scales the factor by its square root.
///
/// # Arguments
///
/// * `matrix` - Flat row-major finite entries of a symmetric positive-definite matrix.
/// * `n` - Positive integral dimension in `1..=4294967295`; the input must have exactly `n * n` entries.
///
/// # Returns
///
/// Lower-triangular factor L as a flat row-major `Float64Array`.
///
/// # Errors
///
/// Throws a JavaScript exception if `n` is non-finite, fractional, non-positive,
/// or outside the wasm32 integer range, `n * n` overflows, `matrix` does not contain
/// exactly `n * n` entries, or the matrix contains a non-finite value, is
/// numerically singular under that relative criterion, or is not positive definite.
#[wasm_bindgen(js_name = choleskyDecomposition)]
pub fn cholesky_decomposition(matrix: &[f64], n: f64) -> Result<Box<[f64]>, JsValue> {
    let n = parse_dimension(n)?;
    validate_flat_matrix_len(matrix, n)?;
    linalg::cholesky_decomposition(matrix, n)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Solve a symmetric positive-definite linear system from a flat Cholesky factor.
///
/// A factor diagonal is numerically singular when its magnitude is below
/// `1e-10` times the largest factor diagonal magnitude. Uniformly scaling the
/// matrix and right-hand side preserves the solution.
///
/// # Arguments
///
/// * `chol` - Cholesky factor stored as `n * n` row-major entries; consumed lower-triangular entries must be finite, and the upper triangle is ignored.
/// * `b` - Finite right-hand-side vector containing exactly `n` entries.
/// * `n` - Positive integral dimension in `1..=4294967295`, shared by the factor and right-hand side.
///
/// # Errors
///
/// Throws a JavaScript exception if `n` is non-finite, fractional, non-positive,
/// or outside the wasm32 integer range, `n * n` overflows, `chol` does not contain
/// exactly `n * n` entries, `b` does not contain `n` entries, a consumed lower-triangular
/// or right-hand-side entry is non-finite, a diagonal factor is numerically
/// singular under that relative criterion, or the
/// solution overflows to a non-finite value.
#[wasm_bindgen(js_name = choleskySolve)]
pub fn cholesky_solve(chol: &[f64], b: &[f64], n: f64) -> Result<Box<[f64]>, JsValue> {
    let n = parse_dimension(n)?;
    validate_flat_matrix_len(chol, n)?;
    if b.len() != n {
        return Err(to_js_err(format!(
            "Right-hand side has length {} but Cholesky factor is {n}x{n}",
            b.len()
        )));
    }
    let mut x = vec![0.0; n];
    linalg::cholesky_solve(chol, b, &mut x).map_err(to_js_err)?;
    Ok(x.into_boxed_slice())
}

/// Apply a lower-triangular factor L to a vector z, returning `L z`.
///
/// This is the Cholesky "apply" step that turns independent standard normals
/// into correlated normals: if `A = L L^T` and `z ~ N(0, I)`, then
/// `L z ~ N(0, A)`. Accepts L as `n * n` row-major entries; only the lower
/// triangle is read and the upper triangle is assumed zero.
///
/// # Arguments
///
/// * `l` - Lower-triangular factor stored as `n * n` row-major entries; the upper triangle is ignored.
/// * `n` - Positive integral dimension in `1..=4294967295`, shared by the factor and vector.
/// * `z` - Vector of exactly `n` observations to transform, typically independent standard normals.
///
/// # Errors
///
/// Throws a JavaScript exception if `n` is non-finite, fractional, non-positive,
/// or outside the wasm32 integer range, `n * n` overflows, `l` does not contain
/// exactly `n * n` entries, or `z` does not contain exactly `n` entries.
#[wasm_bindgen(js_name = applyLowerTriangular)]
pub fn apply_lower_triangular(l: &[f64], n: f64, z: &[f64]) -> Result<Box<[f64]>, JsValue> {
    let n = parse_dimension(n)?;
    validate_flat_matrix_len(l, n)?;
    linalg::apply_lower_triangular(l, n, z)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Arithmetic mean over a typed numeric array.
/// @param data - Numeric observations in input order; an empty series yields 0.0.
/// @returns Arithmetic mean of `data`, or 0.0 when `data` is empty.
#[wasm_bindgen(js_name = mean)]
pub fn mean(data: &[f64]) -> f64 {
    stats::mean(data)
}

/// Sample variance over a typed numeric array.
/// @param data - Sample observations in input order; fewer than two points yield 0.0.
/// @returns Unbiased sample variance, or 0.0 when `data` has fewer than two points.
#[wasm_bindgen(js_name = variance)]
pub fn variance(data: &[f64]) -> f64 {
    stats::variance(data)
}

/// Population variance over a typed numeric array.
/// @param data - Observations in input order; fewer than two points yield 0.0.
/// @returns Population variance, or 0.0 when `data` has fewer than two points.
#[wasm_bindgen(js_name = populationVariance)]
pub fn population_variance(data: &[f64]) -> f64 {
    stats::population_variance(data)
}

/// Pearson correlation over typed numeric arrays.
/// @param x - First numeric series; must have the same length as `y`.
/// @param y - Second numeric series, aligned one-for-one with `x`.
/// @returns Sample correlation in `[-1, 1]`; fewer than two paired points or a constant series yield 0.0. Unequal lengths yield NaN.
#[wasm_bindgen(js_name = correlation)]
pub fn correlation(x: &[f64], y: &[f64]) -> f64 {
    stats::correlation(x, y)
}

/// Sample covariance over typed numeric arrays.
/// @param x - First numeric series; must have the same length as `y`.
/// @param y - Second numeric series, aligned one-for-one with `x`.
/// @returns Unbiased sample covariance, or 0.0 when a series has fewer than two points.
#[wasm_bindgen(js_name = covariance)]
pub fn covariance(x: &[f64], y: &[f64]) -> f64 {
    stats::covariance(x, y)
}

/// Empirical quantile over a typed numeric array.
/// @param data - Sample observations in input order; empty or non-finite data yields NaN.
/// @param q - Quantile probability in `[0, 1]`; values outside that range yield NaN.
/// @returns R-7 interpolated quantile, or NaN when `data` is empty or non-finite.
#[wasm_bindgen(js_name = quantile)]
pub fn quantile(data: &[f64], q: f64) -> f64 {
    let mut v = data.to_vec();
    stats::quantile(&mut v, q)
}

/// Standard normal CDF Φ(x).
/// @param x - Real-valued point at which to evaluate Φ; any finite or infinite `x` is accepted.
/// @returns Probability in `(0, 1)` for finite `x`, with the usual ±∞ limits.
#[wasm_bindgen(js_name = normCdf)]
pub fn norm_cdf(x: f64) -> f64 {
    special_functions::norm_cdf(x)
}

/// Natural logarithm of the standard normal CDF, stable in the negative tail.
///
/// # Arguments
/// - `x`: Standard-normal threshold in standard-deviation units; accepts infinities and propagates NaN.
///
/// @param x - Standard-normal threshold in standard-deviation units; infinities are accepted.
/// @returns Natural log probability; negative infinity at negative infinity, zero at positive infinity, and NaN for NaN.
/// @throws Does not throw; special floating-point inputs follow the documented limits.
#[wasm_bindgen(js_name = logNormCdf)]
pub fn log_norm_cdf(x: f64) -> f64 {
    special_functions::log_norm_cdf(x)
}

/// Standard normal PDF φ(x).
/// @param x - Real-valued point at which to evaluate φ.
/// @returns Density at `x`; φ(0) is `1/sqrt(2π)`.
#[wasm_bindgen(js_name = normPdf)]
pub fn norm_pdf(x: f64) -> f64 {
    special_functions::norm_pdf(x)
}

/// Inverse standard normal CDF Φ⁻¹(p).
/// @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
/// @returns Standard-normal quantile for probability `p`.
#[wasm_bindgen(js_name = standardNormalInvCdf)]
pub fn standard_normal_inv_cdf(p: f64) -> f64 {
    special_functions::standard_normal_inv_cdf(p)
}

/// Error function erf(x).
/// @param x - Real-valued argument to erf; the function is odd, so erf(-x) = -erf(x).
/// @returns erf(x) in `(-1, 1)` for finite `x`.
#[wasm_bindgen(js_name = erf)]
pub fn erf(x: f64) -> f64 {
    special_functions::erf(x)
}

/// Natural logarithm of the Gamma function ln(Γ(x)).
/// @param x - Real argument; must be positive and away from the non-positive integers.
/// @returns ln(Γ(x)); ln(Γ(1)) is 0 and ln(Γ(n+1)) is ln(n!).
#[wasm_bindgen(js_name = lnGamma)]
pub fn ln_gamma(x: f64) -> f64 {
    special_functions::ln_gamma(x)
}

/// Kahan compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values` in input order.
#[wasm_bindgen(js_name = kahanSum)]
pub fn kahan_sum(values: &[f64]) -> f64 {
    summation::kahan_sum(values.iter().copied())
}

/// Neumaier compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values`, robust to mixed-sign cancellation.
#[wasm_bindgen(js_name = neumaierSum)]
pub fn neumaier_sum(values: &[f64]) -> f64 {
    summation::neumaier_sum(values.iter().copied())
}

/// Count the longest consecutive run of strictly positive values in a typed array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Length of the longest run of strictly positive observations.
#[wasm_bindgen(js_name = longestPositiveRun)]
pub fn longest_positive_run(values: &[f64]) -> usize {
    math::longest_positive_run(values)
}

fn parse_dimension(n: f64) -> Result<usize, JsValue> {
    if !n.is_finite() || n.fract() != 0.0 || n <= 0.0 || n > f64::from(u32::MAX) {
        return Err(to_js_err(
            "n must be a finite positive integer no greater than 4294967295",
        ));
    }
    Ok(n as usize)
}

fn validate_flat_matrix_len(matrix: &[f64], n: usize) -> Result<(), JsValue> {
    let expected = n
        .checked_mul(n)
        .ok_or_else(|| to_js_err("Matrix dimension is too large"))?;
    if matrix.len() != expected {
        return Err(to_js_err(format!(
            "Flat matrix has length {} but expected {expected} for {n}x{n}",
            matrix.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-4;

    #[test]
    fn matrix_dimensions_reject_lossy_conversion() {
        for n in [0.0, -1.0, 1.9, f64::NAN, f64::INFINITY, 4_294_967_297.0] {
            assert!(cholesky_decomposition(&[4.0], n).is_err());
            assert!(cholesky_solve(&[2.0], &[4.0], n).is_err());
            assert!(apply_lower_triangular(&[2.0], n, &[4.0]).is_err());
        }
        assert_eq!(
            cholesky_decomposition(&[4.0], 1.0)
                .expect("valid matrix")
                .as_ref(),
            &[2.0]
        );
    }

    #[test]
    fn norm_cdf_reference_values() {
        assert!((norm_cdf(0.0) - 0.5).abs() < TOL);
        assert!((norm_cdf(3.0) - 0.9987).abs() < 1e-3);
    }

    #[test]
    fn norm_pdf_at_zero() {
        assert!((norm_pdf(0.0) - 0.3989).abs() < TOL);
    }

    #[test]
    fn standard_normal_inv_cdf_reference_values() {
        assert!(standard_normal_inv_cdf(0.5).abs() < TOL);
        assert!((standard_normal_inv_cdf(0.975) - 1.96).abs() < 1e-2);
    }

    #[test]
    fn erf_reference_values() {
        assert_eq!(erf(0.0), 0.0);
        assert!((erf(1.0) - 0.8427).abs() < TOL);
    }

    #[test]
    fn ln_gamma_reference_values() {
        assert!(ln_gamma(1.0).abs() < TOL);
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < TOL);
    }

    #[test]
    fn norm_cdf_extremes() {
        assert!(norm_cdf(-10.0) < 1e-15);
        assert!((norm_cdf(10.0) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn erf_negative_symmetry() {
        let pos = erf(1.0);
        let neg = erf(-1.0);
        assert!((pos + neg).abs() < 1e-12, "erf is odd");
    }
}
