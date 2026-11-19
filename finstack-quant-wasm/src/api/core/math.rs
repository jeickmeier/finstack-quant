//! WASM bindings for `finstack_quant_core::math` — linear algebra, statistics,
//! special functions, and compensated summation.

use crate::utils::input::{js_f64, js_f64_seq, js_opt_f64, js_opt_string, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::math::{linalg, special_functions, stats, summation};
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
/// Throws a `TypeError` if `n` is non-finite, fractional, or negative. Throws
/// a JavaScript exception if `matrix` does not contain exactly `n * n`
/// entries (including when `n * n` overflows), or the matrix contains a
/// non-finite value, is not symmetric, is numerically singular under that
/// relative criterion, or is not positive definite.
/// @param matrix - Flat row-major finite entries of a symmetric positive-definite matrix.
/// @param n - Positive integral dimension in `1..=4294967295`; the input must have exactly `n * n` entries.
/// @returns Lower-triangular factor L as a flat row-major `Float64Array`.
#[wasm_bindgen(js_name = choleskyDecomposition)]
pub fn cholesky_decomposition(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let matrix: &[f64] = &js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    linalg::cholesky_decomposition(matrix, n)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Solve a symmetric positive-definite linear system from a flat Cholesky factor.
///
/// The system dimension is `b.length`, as in Rust `cholesky_solve` and Python
/// `cholesky_solve(chol, b)`. A factor diagonal is numerically singular when
/// its magnitude is below `1e-10` times the largest factor diagonal magnitude.
/// Uniformly scaling the matrix and right-hand side preserves the solution.
///
/// # Arguments
///
/// * `chol` - Cholesky factor as a flat row-major array of
///   `b.length * b.length` entries; consumed lower-triangular entries must be
///   finite, and the upper triangle is ignored.
/// * `b` - Finite right-hand-side vector of the linear system; its length is
///   the system dimension.
///
/// # Returns
///
/// Solution vector `x` of `L Lᵀ x = b`, with the same length as `b`.
///
/// # Errors
///
/// Throws a JavaScript exception if `chol` does not contain exactly
/// `b.length * b.length` entries, a consumed lower-triangular or
/// right-hand-side entry is non-finite, a diagonal factor is numerically
/// singular under that relative criterion, or the solution overflows to a
/// non-finite value.
/// @param chol - Cholesky factor as a flat row-major array of `b.length * b.length` entries; consumed lower-triangular entries must be finite, and the upper triangle is ignored.
/// @param b - Finite right-hand-side vector of the linear system; its length is the system dimension.
/// @returns Solution vector `x` of `L Lᵀ x = b`, with the same length as `b`.
#[wasm_bindgen(js_name = choleskySolve)]
pub fn cholesky_solve(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] chol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] b: JsValue,
) -> Result<Box<[f64]>, JsValue> {
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
///
/// # Arguments
///
/// * `l` - Lower-triangular factor stored as `n * n` row-major entries; the upper triangle is ignored.
/// * `n` - Positive integral dimension in `1..=4294967295`, shared by the factor and vector.
/// * `z` - Vector of exactly `n` observations to transform, typically independent standard normals.
///
/// # Errors
///
/// Throws a `TypeError` if `n` is non-finite, fractional, or negative. Throws
/// a JavaScript exception if `l` does not contain exactly `n * n` entries
/// (including when `n * n` overflows) or `z` does not contain exactly `n`
/// entries.
/// @param l - Lower-triangular factor stored as `n * n` row-major entries; the upper triangle is ignored.
/// @param n - Positive integral dimension in `1..=4294967295`, shared by the factor and vector.
/// @param z - Vector of exactly `n` observations to transform, typically independent standard normals.
/// @returns Transformed vector `L z` as a `Float64Array` of length `n`.
#[wasm_bindgen(js_name = applyLowerTriangular)]
pub fn apply_lower_triangular(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] l: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] z: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let l: &[f64] = &js_f64_seq(&l, "l")?;
    let z: &[f64] = &js_f64_seq(&z, "z")?;
    let n: usize = js_uint(&n, "n")?;
    linalg::apply_lower_triangular(l, n, z)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Arithmetic mean over a typed numeric array.
/// @param data - Finite numeric observations in input order; empty series returns `undefined`.
/// @returns Arithmetic mean of `data`, or `undefined` when `data` is empty.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for non-finite values; `FinstackError` (kind `computation`) on overflow.
#[wasm_bindgen(js_name = mean)]
pub fn mean(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    stats::mean(data).map_err(to_js_err)
}

/// Sample variance over a typed numeric array.
/// @param data - Finite sample observations in input order; fewer than two points return `undefined`.
/// @returns Unbiased sample variance, or `undefined` when `data` has fewer than two points.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for non-finite values; `FinstackError` (kind `computation`) on overflow or underflow.
#[wasm_bindgen(js_name = variance)]
pub fn variance(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    stats::variance(data).map_err(to_js_err)
}

/// Population variance over a typed numeric array.
/// @param data - Finite observations in input order; empty series returns `undefined`.
/// @returns Population variance, or `undefined` when `data` is empty.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for non-finite values; `FinstackError` (kind `computation`) on overflow or underflow.
#[wasm_bindgen(js_name = populationVariance)]
pub fn population_variance(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    let data: &[f64] = &js_f64_seq(&data, "data")?;
    stats::population_variance(data).map_err(to_js_err)
}

/// Pearson correlation over typed numeric arrays.
/// @param x - First finite numeric series; must have the same length as `y`.
/// @param y - Second finite numeric series, aligned one-for-one with `x`.
/// @returns Sample correlation in `[-1, 1]`, or `undefined` when fewer than two points exist or either series is constant.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) when lengths differ or values are non-finite; `FinstackError` (kind `computation`) on numerical failure.
#[wasm_bindgen(js_name = correlation)]
pub fn correlation(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] y: JsValue,
) -> Result<Option<f64>, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    stats::correlation(x, y).map_err(to_js_err)
}

/// Sample covariance over typed numeric arrays.
/// @param x - First finite numeric series; must have the same length as `y`.
/// @param y - Second finite numeric series, aligned one-for-one with `x`.
/// @returns Unbiased sample covariance, or `undefined` when fewer than two points exist.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) when lengths differ or values are non-finite; `FinstackError` (kind `computation`) on overflow or underflow.
#[wasm_bindgen(js_name = covariance)]
pub fn covariance(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] y: JsValue,
) -> Result<Option<f64>, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    stats::covariance(x, y).map_err(to_js_err)
}

/// Annualized realized variance of a close price series (Rust
/// `stats::realized_variance`): the mean of squared log returns times the
/// annualization factor, with no mean subtraction.
///
/// # Arguments
///
/// * `prices` - Close prices in time order; each must be finite and positive.
/// * `annualization_factor` - Observations per year (for example `252` for
///   daily closes); omitted selects the Rust daily default (252).
///
/// @returns Annualized decimal variance, or `undefined` for fewer than two valid prices.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for a non-positive or non-finite price or a non-positive
/// annualization factor; kind `computation` if the annualized result is unrepresentable.
/// @param prices - Close prices in time order; each must be finite and positive.
/// @param annualizationFactor - Observations per year (for example `252` for daily closes); omitted selects the Rust daily default (252).
///
/// @example
/// ```typescript
/// core.realizedVariance([100, 101, 99.5, 100.2]);  // close-to-close, 252/yr
/// ```
#[wasm_bindgen(js_name = realizedVariance)]
pub fn realized_variance(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] prices: JsValue,
    #[wasm_bindgen(
        js_name = "annualizationFactor",
        unchecked_optional_param_type = "number | null"
    )]
    annualization_factor: Option<JsValue>,
) -> Result<Option<f64>, JsValue> {
    let prices: &[f64] = &js_f64_seq(&prices, "prices")?;
    let annualization_factor = js_opt_f64(annualization_factor.as_ref(), "annualizationFactor")?;
    stats::realized_variance(prices, annualization_factor).map_err(to_js_err)
}

/// Annualized realized variance from OHLC bars (Rust `stats::realized_variance_ohlc`).
///
/// # Arguments
///
/// * `open` - Opening prices, one per bar.
/// * `high` - High prices, one per bar.
/// * `low` - Low prices, one per bar.
/// * `close` - Closing prices, one per bar.
/// * `method` - Estimator name: `"close_to_close"`, `"parkinson"`,
///   `"garman_klass"`, `"rogers_satchell"` or `"yang_zhang"`. Omitted selects
///   the Rust OHLC default (`"yang_zhang"`).
/// * `annualization_factor` - Bars per year (for example `252` for daily
///   bars); omitted selects the Rust daily default (252).
///
/// * `previous_close` - Positive finite close immediately preceding the first bar for Yang-Zhang and close-to-close. Range estimators require omission.
///
/// @param previousClose - Closing price immediately before the first target bar, required for close-to-close and Yang-Zhang and omitted otherwise.
/// @returns Annualized variance in decimal units, or `undefined` for a valid empty window or fewer than two Yang-Zhang bars.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for an unknown method, series of different lengths, an
/// missing or inappropriate preceding close, invalid bar, or a non-positive annualization factor;
/// kind `computation` if the annualized result is unrepresentable.
/// @param open - Opening prices, one per bar.
/// @param high - High prices, one per bar.
/// @param low - Low prices, one per bar.
/// @param close - Closing prices, one per bar.
/// @param method - Estimator name: `"close_to_close"`, `"parkinson"`, `"garman_klass"`, `"rogers_satchell"` or `"yang_zhang"`. Omitted selects the Rust OHLC default (`"yang_zhang"`).
/// @param annualizationFactor - Bars per year (for example `252` for daily bars); omitted selects the Rust daily default (252).
#[wasm_bindgen(js_name = realizedVarianceOhlc)]
pub fn realized_variance_ohlc(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] open: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] high: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] low: JsValue,
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] close: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] method: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "annualizationFactor",
        unchecked_optional_param_type = "number | null"
    )]
    annualization_factor: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "previousClose",
        unchecked_optional_param_type = "number | null"
    )]
    previous_close: Option<JsValue>,
) -> Result<Option<f64>, JsValue> {
    let open: &[f64] = &js_f64_seq(&open, "open")?;
    let high: &[f64] = &js_f64_seq(&high, "high")?;
    let low: &[f64] = &js_f64_seq(&low, "low")?;
    let close: &[f64] = &js_f64_seq(&close, "close")?;
    let method = realized_var_method(method)?;
    let annualization_factor = js_opt_f64(annualization_factor.as_ref(), "annualizationFactor")?;
    let previous_close = js_opt_f64(previous_close.as_ref(), "previousClose")?;
    stats::realized_variance_ohlc(
        open,
        high,
        low,
        close,
        method,
        annualization_factor,
        previous_close,
    )
    .map_err(to_js_err)
}

/// Parse an optional `RealizedVarMethod` from its serde name.
fn realized_var_method(
    method: Option<JsValue>,
) -> Result<Option<stats::RealizedVarMethod>, JsValue> {
    js_opt_string(method.as_ref(), "method")?
        .map(|name| finstack_quant_core::wire::serde_parse(&name))
        .transpose()
        .map_err(to_js_err)
}

/// Empirical quantile over a typed numeric array.
/// @param data - Finite sample observations in input order; an empty series returns `undefined`.
/// @param q - Quantile probability in `[0, 1]`.
/// @returns R-7 interpolated quantile, or `undefined` when `data` is empty.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) when `q` is outside `[0, 1]` or any value is non-finite; `FinstackError` (kind `computation`) on underflow.
#[wasm_bindgen(js_name = quantile)]
pub fn quantile(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] q: JsValue,
) -> Result<Option<f64>, JsValue> {
    let mut data = js_f64_seq(&data, "data")?;
    let q = js_f64(&q, "q")?;
    stats::quantile(&mut data, q).map_err(to_js_err)
}

/// Fit a full-rank linear regression over a rectangular predictor panel.
/// @param x - Finite observations in rows and one predictor feature per column.
/// @param y - Finite response with one value per observation.
/// @param addIntercept - Include a constant term by default. False fits through the origin and uses uncentered R².
/// @returns Fitted slopes, optional intercept, residuals and fit diagnostics.
/// @throws TypeError for incompatible host types, or FinstackError for mismatched dimensions, non-finite or rank-deficient inputs, zero response variation or unrepresentable results.
#[wasm_bindgen(js_name = ols, unchecked_return_type = "generated.core.OlsResult")]
pub fn ols(
    #[wasm_bindgen(unchecked_param_type = "readonly (readonly number[] | Float64Array)[]")]
    x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "readonly number[] | Float64Array")] y: JsValue,
    #[wasm_bindgen(
        js_name = "addIntercept",
        unchecked_optional_param_type = "boolean | null"
    )]
    add_intercept: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let rows = crate::utils::input::js_f64_matrix(&x, "x")?;
    let n_features = rows.first().map_or(0, Vec::len);
    if rows.iter().any(|row| row.len() != n_features) {
        return Err(to_js_err(finstack_quant_core::Error::Validation(
            "OLS requires a rectangular predictor matrix".into(),
        )));
    }
    let n_obs = rows.len();
    let data: Vec<_> = rows.into_iter().flatten().collect();
    let responses = crate::utils::input::js_f64_seq(&y, "y")?;
    let add_intercept =
        crate::utils::input::js_opt_bool(add_intercept.as_ref(), "addIntercept")?.unwrap_or(true);
    let result =
        stats::ols(&data, n_obs, n_features, &responses, add_intercept).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Compute the signed mean of the worst fractional tail of a sample.
/// @param data - Nonempty finite observations in any order, such as signed simple returns.
/// @param confidence - Probability strictly between zero and one. The lower tail has mass `1 - confidence`.
/// @returns Signed lower-tail mean in the observations' units.
/// @throws `TypeError` for a mistyped argument or `FinstackError` for invalid confidence, empty or nonfinite data, or an unrepresentable result.
#[wasm_bindgen(js_name = expectedShortfall)]
pub fn expected_shortfall(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
) -> Result<f64, JsValue> {
    let data = js_f64_seq(&data, "data")?;
    let confidence = js_f64(&confidence, "confidence")?;
    stats::expected_shortfall(&data, confidence).map_err(to_js_err)
}

/// Standard normal CDF Φ(x).
/// @param x - Real-valued point at which to evaluate Φ; any finite or infinite `x` is accepted.
/// @returns Cumulative probability in `[0, 1]`; the two infinities map to zero and one.
/// @throws `TypeError` for a non-number, or a validation error if `x` is NaN.
#[wasm_bindgen(js_name = normCdf)]
pub fn norm_cdf(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    special_functions::norm_cdf(x).map_err(to_js_err)
}

/// Natural logarithm of the standard normal CDF, stable in the negative tail.
///
/// @param x - Standard-normal threshold in standard-deviation units; both infinities are accepted.
/// @returns Natural log probability; negative infinity at negative infinity, zero at positive infinity.
/// @throws `TypeError` for a non-number, a validation error for NaN, or a computation error for an unrepresentable finite-input log probability.
#[wasm_bindgen(js_name = logNormCdf)]
pub fn log_norm_cdf(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    special_functions::log_norm_cdf(x).map_err(to_js_err)
}

/// Standard normal PDF φ(x).
/// @param x - Real-valued point at which to evaluate φ.
/// @returns Density at `x`; φ(0) is `1/sqrt(2π)`.
/// @throws `TypeError` for a non-number, or a validation error if `x` is NaN.
#[wasm_bindgen(js_name = normPdf)]
pub fn norm_pdf(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    special_functions::norm_pdf(x).map_err(to_js_err)
}

/// Inverse standard normal CDF Φ⁻¹(p).
/// @param p - Probability in `[0, 1]` (`0` yields `-Infinity`, `1` yields `Infinity`, values outside `[0, 1]` are invalid).
/// @returns Standard-normal quantile for probability `p`.
/// @throws `TypeError` for a non-number, a validation error for NaN or an out-of-domain probability, or a computation error for an unresolved interior quantile.
#[wasm_bindgen(js_name = standardNormalInvCdf)]
pub fn standard_normal_inv_cdf(
    #[wasm_bindgen(unchecked_param_type = "number")] p: JsValue,
) -> Result<f64, JsValue> {
    let p = js_f64(&p, "p")?;
    special_functions::standard_normal_inv_cdf(p).map_err(to_js_err)
}

/// Error function erf(x).
/// @param x - Real-valued argument to erf; the function is odd, so erf(-x) = -erf(x).
/// @returns erf(x) in `[-1, 1]`.
/// @throws `TypeError` for a non-number, or a validation error if `x` is NaN.
#[wasm_bindgen(js_name = erf)]
pub fn erf(#[wasm_bindgen(unchecked_param_type = "number")] x: JsValue) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    special_functions::erf(x).map_err(to_js_err)
}

/// Natural logarithm of the Gamma function ln(Γ(x)).
/// @param x - Finite positive argument.
/// @returns ln(Γ(x)); ln(Γ(1)) is 0 and ln(Γ(n+1)) is ln(n!).
/// @throws `TypeError` for a non-number, a validation error for non-finite or nonpositive arguments, or a computation error if the logarithm overflows.
#[wasm_bindgen(js_name = lnGamma)]
pub fn ln_gamma(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
) -> Result<f64, JsValue> {
    let x = js_f64(&x, "x")?;
    special_functions::ln_gamma(x).map_err(to_js_err)
}

/// Kahan compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values` in input order.
/// @throws Error with `kind: "validation"` if any element is `NaN` or infinite, or `kind: "computation"` if intermediate accumulation overflows.
#[wasm_bindgen(js_name = kahanSum)]
pub fn kahan_sum(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] values: JsValue,
) -> Result<f64, JsValue> {
    let values: &[f64] = &js_f64_seq(&values, "values")?;
    summation::kahan_sum(values.iter().copied()).map_err(to_js_err)
}

/// Neumaier compensated summation over a typed numeric array.
/// @param values - Finite numeric terms in summation or scan order.
/// @returns Compensated sum of `values`, robust to mixed-sign cancellation.
/// @throws Error with `kind: "validation"` if any element is `NaN` or infinite, or `kind: "computation"` if intermediate or final accumulation overflows.
#[wasm_bindgen(js_name = neumaierSum)]
pub fn neumaier_sum(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] values: JsValue,
) -> Result<f64, JsValue> {
    let values: &[f64] = &js_f64_seq(&values, "values")?;
    summation::neumaier_sum(values.iter().copied()).map_err(to_js_err)
}

/// Symmetric eigendecomposition of a flat row-major matrix (Rust `symmetric_eigen`).
///
/// # Arguments
///
/// * `matrix` - Flat row-major `n * n` entries of a symmetric matrix; it need
///   not be positive definite.
/// * `n` - Positive matrix dimension; `matrix` must contain exactly `n * n`
///   entries.
///
/// @returns A two-element array `[eigenvalues, eigenvectors]`: `n` eigenvalues
/// (in the solver's order, not sorted) and a flat `n * n` `Float64Array` in
/// which `eigenvectors[i * n + k]` is component `i` of eigenvector `k`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `matrix` does not hold exactly `n * n` entries, is not
/// symmetric, or contains a non-finite value.
/// @param matrix - Flat row-major `n * n` entries of a symmetric matrix; it need not be positive definite.
/// @param n - Positive matrix dimension; `matrix` must contain exactly `n * n` entries.
#[wasm_bindgen(js_name = symmetricEigen, unchecked_return_type = "[Float64Array, Float64Array]")]
pub fn symmetric_eigen(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
) -> Result<js_sys::Array, JsValue> {
    let matrix: &[f64] = &js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    let (values, vectors) = linalg::symmetric_eigen(matrix, n).map_err(to_js_err)?;
    Ok(js_sys::Array::of2(
        &js_sys::Float64Array::from(values.as_slice()),
        &js_sys::Float64Array::from(vectors.as_slice()),
    ))
}

/// Ledoit-Wolf (2004) shrinkage of a sample covariance matrix toward a scaled
/// identity (Rust `ledoit_wolf_shrinkage`).
///
/// # Arguments
///
/// * `observations` - Flat row-major `t * n` observation matrix; each row is
///   one date and each column one variable.
/// * `t` - Number of observations (rows); at least `2`.
/// * `n` - Number of variables (columns); at least `1`.
///
/// @returns A two-element array `[covariance, shrinkage]`: the shrunk
/// covariance as a flat row-major `n * n` `Float64Array`, and the estimated
/// shrinkage intensity in `[0, 1]`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `t < 2`, `n == 0`, an entry is non-finite, or
/// `observations` does not hold exactly `t * n` entries, or kind `computation`
/// if the intensity or covariance cannot be resolved.
/// @param observations - Flat row-major `t * n` observation matrix; each row is one date and each column one variable.
/// @param t - Number of observations (rows); at least `2`.
/// @param n - Number of variables (columns); at least `1`.
#[wasm_bindgen(js_name = ledoitWolfShrinkage, unchecked_return_type = "[Float64Array, number]")]
pub fn ledoit_wolf_shrinkage(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] observations: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
) -> Result<js_sys::Array, JsValue> {
    let observations: &[f64] = &js_f64_seq(&observations, "observations")?;
    let t: usize = js_uint(&t, "t")?;
    let n: usize = js_uint(&n, "n")?;
    let result = linalg::ledoit_wolf_shrinkage(observations, t, n).map_err(to_js_err)?;
    Ok(js_sys::Array::of2(
        &js_sys::Float64Array::from(result.covariance.as_slice()),
        &JsValue::from_f64(result.shrinkage),
    ))
}

/// Mean and sample variance in one pass (Rust `stats::mean_var`).
///
/// # Arguments
///
/// * `data` - Finite sample observations in input order.
///
/// @returns A two-element tuple `[mean, variance]`; `[undefined, undefined]` when empty, `[mean, undefined]` for one observation.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for non-finite observations; `FinstackError` (kind `computation`) on overflow or underflow.
/// @param data - Finite sample observations in input order.
#[wasm_bindgen(
    js_name = meanVar,
    unchecked_return_type = "[number | undefined, number | undefined]"
)]
pub fn mean_var(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<js_sys::Array, JsValue> {
    let (mean, variance) = stats::mean_var(&js_f64_seq(&data, "data")?).map_err(to_js_err)?;
    Ok(js_sys::Array::of2(
        &mean.map_or(JsValue::UNDEFINED, JsValue::from_f64),
        &variance.map_or(JsValue::UNDEFINED, JsValue::from_f64),
    ))
}

/// Sample standard deviation with `n - 1` denominator (Rust `stats::std_dev`).
///
/// Validates all observations upfront. Returns `undefined` when `data` has
/// fewer than two observations, and `0` when `data.length >= 2` and all
/// observations are identical.
///
/// # Arguments
///
/// * `data` - Sample observations.
///
/// @returns The unbiased sample standard deviation, or `undefined` for fewer than two observations.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if any observation is `NaN` or infinite; `FinstackError`
/// (kind `computation`) if non-constant sample standard deviation overflows
/// or underflows out of the positive finite range.
/// @param data - Sample observations.
#[wasm_bindgen(js_name = stdDev)]
pub fn std_dev(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    stats::std_dev(&js_f64_seq(&data, "data")?).map_err(to_js_err)
}

/// Sample median using R-7 / NumPy quantile interpolation at `p = 0.5` (Rust `stats::median`).
///
/// Validates all observations upfront without modifying the input array.
/// Returns `undefined` for empty input.
///
/// # Arguments
///
/// * `data` - Observations whose median is required; the input is not modified.
///
/// @returns The median, or `undefined` for an empty series.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if any observation is `NaN` or infinite.
/// @param data - Observations whose median is required; the input is not modified.
#[wasm_bindgen(js_name = median)]
pub fn median(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    stats::median(&js_f64_seq(&data, "data")?).map_err(to_js_err)
}

/// Smallest finite value (Rust `stats::finite_min`).
///
/// Ignores `NaN` and infinite entries. Returns `undefined` when `data`
/// contains no finite values.
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are ignored.
///
/// @returns The minimum finite value, or `undefined` when none is finite.
/// @throws `TypeError` if `data` is not an array of numbers.
/// @param data - Observations to inspect; NaN and infinite entries are ignored.
#[wasm_bindgen(js_name = finiteMin)]
pub fn finite_min(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    Ok(stats::finite_min(&js_f64_seq(&data, "data")?))
}

/// Largest finite value (Rust `stats::finite_max`).
///
/// Ignores `NaN` and infinite entries. Returns `undefined` when `data`
/// contains no finite values.
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are ignored.
///
/// @returns The maximum finite value, or `undefined` when none is finite.
/// @throws `TypeError` if `data` is not an array of numbers.
/// @param data - Observations to inspect; NaN and infinite entries are ignored.
#[wasm_bindgen(js_name = finiteMax)]
pub fn finite_max(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<Option<f64>, JsValue> {
    Ok(stats::finite_max(&js_f64_seq(&data, "data")?))
}

/// Number of finite observations (Rust `stats::finite_count`).
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are not counted.
///
/// @returns The count of finite values.
/// @throws `TypeError` if `data` is not an array of numbers.
/// @param data - Observations to inspect; NaN and infinite entries are not counted.
#[wasm_bindgen(js_name = finiteCount)]
pub fn finite_count(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] data: JsValue,
) -> Result<usize, JsValue> {
    Ok(stats::finite_count(&js_f64_seq(&data, "data")?))
}

/// Log returns of a price series (Rust `stats::log_returns`).
///
/// # Arguments
///
/// * `prices` - Chronologically ordered price levels.
///
/// @returns Adjacent log returns, or an empty array for fewer than two valid prices.
/// @throws `TypeError` for a mistyped series or `FinstackError` for nonpositive or nonfinite prices.
/// @param prices - Chronologically ordered price levels.
#[wasm_bindgen(js_name = logReturns)]
pub fn log_returns(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] prices: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    Ok(stats::log_returns(&js_f64_seq(&prices, "prices")?)
        .map_err(to_js_err)?
        .into_boxed_slice())
}

/// Normal cumulative distribution function `Φ((x - mean) / stdDev)` (Rust
/// `norm_cdf_with_params`).
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the CDF.
/// * `mean` - Mean of the distribution, in the units of `x`.
/// * `std_dev` - Strictly positive standard deviation (not the variance), in
///   the units of `x`.
///
/// @returns The probability that a `N(mean, stdDev²)` variable is at most `x`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `stdDev` is non-finite or not strictly positive, `mean` is
/// non-finite, or `x` is `NaN`.
/// @param x - Point at which to evaluate the CDF (`±Infinity` accepted, `NaN` rejected).
/// @param mean - Finite mean of the distribution, in the units of `x`.
/// @param stdDev - Strictly positive standard deviation (not the variance), in the units of `x`.
#[wasm_bindgen(js_name = normCdfWithParams)]
pub fn norm_cdf_with_params(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] mean: JsValue,
    #[wasm_bindgen(js_name = "stdDev", unchecked_param_type = "number")] std_dev: JsValue,
) -> Result<f64, JsValue> {
    special_functions::norm_cdf_with_params(
        js_f64(&x, "x")?,
        js_f64(&mean, "mean")?,
        js_f64(&std_dev, "stdDev")?,
    )
    .map_err(to_js_err)
}

/// Normal probability density `φ((x - mean) / stdDev) / stdDev` (Rust
/// `norm_pdf_with_params`).
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the density.
/// * `mean` - Mean of the distribution, in the units of `x`.
/// * `std_dev` - Strictly positive standard deviation (not the variance), in
///   the units of `x`.
///
/// @returns The density of `N(mean, stdDev²)` at `x`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `stdDev` is non-finite or not strictly positive, `mean` is
/// non-finite, or `x` is `NaN`; kind `computation` if the density overflows.
/// @param x - Point at which to evaluate the density (`±Infinity` accepted, `NaN` rejected).
/// @param mean - Finite mean of the distribution, in the units of `x`.
/// @param stdDev - Strictly positive standard deviation (not the variance), in the units of `x`.
#[wasm_bindgen(js_name = normPdfWithParams)]
pub fn norm_pdf_with_params(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] mean: JsValue,
    #[wasm_bindgen(js_name = "stdDev", unchecked_param_type = "number")] std_dev: JsValue,
) -> Result<f64, JsValue> {
    special_functions::norm_pdf_with_params(
        js_f64(&x, "x")?,
        js_f64(&mean, "mean")?,
        js_f64(&std_dev, "stdDev")?,
    )
    .map_err(to_js_err)
}

/// Student-t cumulative distribution function (Rust `student_t_cdf`).
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the CDF; infinities are accepted.
/// * `df` - Degrees of freedom, strictly positive.
///
/// @returns The probability that a Student-t variable with `df` degrees of
/// freedom is at most `x`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `x` is NaN or `df` is non-finite or nonpositive, or kind
/// `computation` if the tail cannot be evaluated.
/// @param x - Point at which to evaluate the CDF; infinities are accepted.
/// @param df - Degrees of freedom, strictly positive.
#[wasm_bindgen(js_name = studentTCdf)]
pub fn student_t_cdf(
    #[wasm_bindgen(unchecked_param_type = "number")] x: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] df: JsValue,
) -> Result<f64, JsValue> {
    special_functions::student_t_cdf(js_f64(&x, "x")?, js_f64(&df, "df")?).map_err(to_js_err)
}

/// Student-t inverse cumulative distribution function (Rust `student_t_inv_cdf`).
///
/// # Arguments
///
/// * `p` - Probability in `[0, 1]` (`0` returns `-Infinity`, `1` returns `Infinity`).
/// * `df` - Degrees of freedom, finite and strictly positive.
///
/// @returns Student-t quantile at probability `p`, including infinite endpoint limits.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `df` is non-finite or not strictly positive, or `p` is
/// non-finite or outside `[0, 1]`, or kind `computation` if an interior
/// quantile cannot be resolved or represented.
/// @param p - Probability in `[0, 1]` (`0` returns `-Infinity`, `1` returns `Infinity`).
/// @param df - Degrees of freedom, finite and strictly positive.
#[wasm_bindgen(js_name = studentTInvCdf)]
pub fn student_t_inv_cdf(
    #[wasm_bindgen(unchecked_param_type = "number")] p: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] df: JsValue,
) -> Result<f64, JsValue> {
    special_functions::student_t_inv_cdf(js_f64(&p, "p")?, js_f64(&df, "df")?).map_err(to_js_err)
}
