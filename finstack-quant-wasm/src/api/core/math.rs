//! WASM bindings for `finstack_quant_core::math` — linear algebra, statistics,
//! special functions, and compensated summation.

use crate::utils::to_js_err;
use finstack_quant_core::math::{self, linalg, special_functions, stats, summation};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Linear algebra
// ---------------------------------------------------------------------------

/// Cholesky decomposition of a symmetric positive-definite matrix.
///
/// Accepts a square matrix as a nested JS array (`number[][]`, row-major)
/// and returns the lower-triangular factor L such that A = L L^T.
/// @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
#[wasm_bindgen(js_name = choleskyDecomposition)]
pub fn cholesky_decomposition(matrix: JsValue) -> Result<JsValue, JsValue> {
    let rows: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(matrix).map_err(to_js_err)?;
    let n = rows.len();
    let flat = flatten_matrix(&rows, n)?;
    let result = linalg::cholesky_decomposition(&flat, n).map_err(to_js_err)?;
    let nested = unflatten_matrix(&result, n);
    serde_wasm_bindgen::to_value(&nested).map_err(to_js_err)
}

/// Solve a symmetric positive-definite linear system A x = b given the
/// Cholesky factor L (where A = L L^T).
///
/// Accepts L as `number[][]` and b as `number[]`. Returns x as `number[]`.
/// @param chol - Lower-triangular Cholesky factor of the coefficient matrix, in the documented matrix shape.
/// @param b - Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.
#[wasm_bindgen(js_name = choleskySolve)]
pub fn cholesky_solve(chol: JsValue, b: JsValue) -> Result<JsValue, JsValue> {
    let rows: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(chol).map_err(to_js_err)?;
    let n = rows.len();
    let flat = flatten_matrix(&rows, n)?;
    let b_vec: Vec<f64> = serde_wasm_bindgen::from_value(b).map_err(to_js_err)?;
    if b_vec.len() != n {
        return Err(to_js_err(format!(
            "Right-hand side has length {} but Cholesky factor is {n}x{n}",
            b_vec.len()
        )));
    }
    let mut x = vec![0.0; n];
    linalg::cholesky_solve(&flat, &b_vec, &mut x).map_err(to_js_err)?;
    serde_wasm_bindgen::to_value(&x).map_err(to_js_err)
}

/// Cholesky decomposition for a flat row-major matrix.
///
/// Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
/// and returns a flat lower-triangular factor.
/// @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
/// @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
#[wasm_bindgen(js_name = choleskyDecompositionFlat)]
pub fn cholesky_decomposition_flat(matrix: &[f64], n: usize) -> Result<Box<[f64]>, JsValue> {
    validate_flat_matrix_len(matrix, n)?;
    linalg::cholesky_decomposition(matrix, n)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Solve a symmetric positive-definite linear system from a flat Cholesky factor.
/// @param chol - Lower-triangular Cholesky factor of the coefficient matrix, in the documented matrix shape.
/// @param b - Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.
/// @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
#[wasm_bindgen(js_name = choleskySolveFlat)]
pub fn cholesky_solve_flat(chol: &[f64], b: &[f64], n: usize) -> Result<Box<[f64]>, JsValue> {
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

/// Validate a flat row-major correlation matrix.
///
/// This is the only correlation-matrix validator on the `core` namespace.
/// Callers pass `n * n` row-major entries plus the matrix dimension `n`.
/// @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
/// @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
#[wasm_bindgen(js_name = validateCorrelationMatrixFlat)]
pub fn validate_correlation_matrix_flat(matrix: &[f64], n: usize) -> Result<(), JsValue> {
    validate_flat_matrix_len(matrix, n)?;
    linalg::validate_correlation_matrix(matrix, n).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Arithmetic mean.
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = mean)]
pub fn mean(data: JsValue) -> Result<f64, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    Ok(stats::mean(&v))
}

/// Sample variance (unbiased, n-1 denominator).
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = variance)]
pub fn variance(data: JsValue) -> Result<f64, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    Ok(stats::variance(&v))
}

/// Population variance (n denominator).
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = populationVariance)]
pub fn population_variance(data: JsValue) -> Result<f64, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    Ok(stats::population_variance(&v))
}

/// Pearson correlation coefficient.
/// @param x - Numeric observation series aligned one-for-one with the other series.
/// @param y - Numeric observation series aligned one-for-one with the other series.
#[wasm_bindgen(js_name = correlation)]
pub fn correlation(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let xv: Vec<f64> = serde_wasm_bindgen::from_value(x).map_err(to_js_err)?;
    let yv: Vec<f64> = serde_wasm_bindgen::from_value(y).map_err(to_js_err)?;
    Ok(stats::correlation(&xv, &yv))
}

/// Sample covariance (unbiased, n-1 denominator).
/// @param x - Numeric observation series aligned one-for-one with the other series.
/// @param y - Numeric observation series aligned one-for-one with the other series.
#[wasm_bindgen(js_name = covariance)]
pub fn covariance(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let xv: Vec<f64> = serde_wasm_bindgen::from_value(x).map_err(to_js_err)?;
    let yv: Vec<f64> = serde_wasm_bindgen::from_value(y).map_err(to_js_err)?;
    Ok(stats::covariance(&xv, &yv))
}

/// Empirical quantile (R-7 / NumPy default) with linear interpolation.
/// @param data - Non-empty numeric observation array used by the requested statistic.
/// @param q - Quantile probability from 0 through 1 used to select the order statistic.
#[wasm_bindgen(js_name = quantile)]
pub fn quantile(data: JsValue, q: f64) -> Result<f64, JsValue> {
    let mut v: Vec<f64> = serde_wasm_bindgen::from_value(data).map_err(to_js_err)?;
    Ok(stats::quantile(&mut v, q))
}

/// Arithmetic mean over a typed numeric array.
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = meanArray)]
pub fn mean_array(data: &[f64]) -> f64 {
    stats::mean(data)
}

/// Sample variance over a typed numeric array.
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = varianceArray)]
pub fn variance_array(data: &[f64]) -> f64 {
    stats::variance(data)
}

/// Population variance over a typed numeric array.
/// @param data - Non-empty numeric observation array used by the requested statistic.
#[wasm_bindgen(js_name = populationVarianceArray)]
pub fn population_variance_array(data: &[f64]) -> f64 {
    stats::population_variance(data)
}

/// Pearson correlation over typed numeric arrays.
/// @param x - Numeric observation series aligned one-for-one with the other series.
/// @param y - Numeric observation series aligned one-for-one with the other series.
#[wasm_bindgen(js_name = correlationArray)]
pub fn correlation_array(x: &[f64], y: &[f64]) -> f64 {
    stats::correlation(x, y)
}

/// Sample covariance over typed numeric arrays.
/// @param x - Numeric observation series aligned one-for-one with the other series.
/// @param y - Numeric observation series aligned one-for-one with the other series.
#[wasm_bindgen(js_name = covarianceArray)]
pub fn covariance_array(x: &[f64], y: &[f64]) -> f64 {
    stats::covariance(x, y)
}

/// Empirical quantile over a typed numeric array.
/// @param data - Non-empty numeric observation array used by the requested statistic.
/// @param q - Quantile probability from 0 through 1 used to select the order statistic.
#[wasm_bindgen(js_name = quantileArray)]
pub fn quantile_array(data: &[f64], q: f64) -> f64 {
    let mut v = data.to_vec();
    stats::quantile(&mut v, q)
}

// ---------------------------------------------------------------------------
// Special functions
// ---------------------------------------------------------------------------

/// Standard normal CDF Φ(x).
/// @param x - Real-valued input to the requested scalar mathematical function.
#[wasm_bindgen(js_name = normCdf)]
pub fn norm_cdf(x: f64) -> f64 {
    special_functions::norm_cdf(x)
}

/// Standard normal PDF φ(x).
/// @param x - Real-valued input to the requested scalar mathematical function.
#[wasm_bindgen(js_name = normPdf)]
pub fn norm_pdf(x: f64) -> f64 {
    special_functions::norm_pdf(x)
}

/// Inverse standard normal CDF Φ⁻¹(p).
/// @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
#[wasm_bindgen(js_name = standardNormalInvCdf)]
pub fn standard_normal_inv_cdf(p: f64) -> f64 {
    special_functions::standard_normal_inv_cdf(p)
}

/// Error function erf(x).
/// @param x - Real-valued input to the requested scalar mathematical function.
#[wasm_bindgen(js_name = erf)]
pub fn erf(x: f64) -> f64 {
    special_functions::erf(x)
}

/// Natural logarithm of the Gamma function ln(Γ(x)).
/// @param x - Real-valued input to the requested scalar mathematical function.
#[wasm_bindgen(js_name = lnGamma)]
pub fn ln_gamma(x: f64) -> f64 {
    special_functions::ln_gamma(x)
}

// ---------------------------------------------------------------------------
// Summation
// ---------------------------------------------------------------------------

/// Kahan compensated summation.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = kahanSum)]
pub fn kahan_sum(values: JsValue) -> Result<f64, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    Ok(summation::kahan_sum(v))
}

/// Neumaier compensated summation — handles mixed-sign values.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = neumaierSum)]
pub fn neumaier_sum(values: JsValue) -> Result<f64, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    Ok(summation::neumaier_sum(v))
}

/// Count the longest consecutive run of strictly positive values.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = countConsecutive)]
pub fn count_consecutive(values: JsValue) -> Result<usize, JsValue> {
    let v: Vec<f64> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    Ok(math::count_consecutive(&v, |x| x > 0.0))
}

/// Kahan compensated summation over a typed numeric array.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = kahanSumArray)]
pub fn kahan_sum_array(values: &[f64]) -> f64 {
    summation::kahan_sum(values.iter().copied())
}

/// Neumaier compensated summation over a typed numeric array.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = neumaierSumArray)]
pub fn neumaier_sum_array(values: &[f64]) -> f64 {
    summation::neumaier_sum(values.iter().copied())
}

/// Count the longest consecutive run of strictly positive values in a typed array.
/// @param values - Numeric values in the order used by the requested numerical operation.
#[wasm_bindgen(js_name = countConsecutiveArray)]
pub fn count_consecutive_array(values: &[f64]) -> usize {
    math::count_consecutive(values, |x| x > 0.0)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

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

/// Flatten nested rows into a row-major `Vec<f64>`, validating squareness.
fn flatten_matrix(rows: &[Vec<f64>], n: usize) -> Result<Vec<f64>, JsValue> {
    let mut flat = Vec::with_capacity(n * n);
    for (i, row) in rows.iter().enumerate() {
        if row.len() != n {
            return Err(to_js_err(format!(
                "Row {i} has length {} but expected {n} for a square matrix",
                row.len()
            )));
        }
        flat.extend_from_slice(row);
    }
    Ok(flat)
}

/// Unflatten a row-major `Vec<f64>` of length `n*n` into nested rows.
fn unflatten_matrix(flat: &[f64], n: usize) -> Vec<Vec<f64>> {
    flat.chunks(n).map(|c| c.to_vec()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-4;

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
    fn flatten_unflatten_matrix_identity() {
        let rows = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let flat = flatten_matrix(&rows, 2).expect("square 2x2 matrix");
        assert_eq!(flat, vec![1.0, 0.0, 0.0, 1.0]);
        let back = unflatten_matrix(&flat, 2);
        assert_eq!(back, rows);
    }

    // -- Boundary tests ------------------------------------------------
    // flatten_matrix returns Result<_, JsValue> which panics on native.
    // Test the validation logic directly.

    #[test]
    fn flatten_matrix_empty() {
        // 0x0 case: no rows, n=0 — the loop body never runs
        let rows: Vec<Vec<f64>> = vec![];
        let n = rows.len();
        let mut flat = Vec::with_capacity(n * n);
        for row in &rows {
            flat.extend_from_slice(row);
        }
        assert!(flat.is_empty());
    }

    #[test]
    fn non_square_row_detected() {
        // Verify the dimension check logic used by flatten_matrix
        let rows = [vec![1.0, 0.0], vec![0.0]];
        let n = rows.len(); // 2
        let bad_row = rows.iter().enumerate().find(|(_, r)| r.len() != n);
        assert!(bad_row.is_some(), "should detect row length mismatch");
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
