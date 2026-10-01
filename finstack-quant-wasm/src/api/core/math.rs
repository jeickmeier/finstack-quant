//! WASM bindings for `finstack_quant_core::math` — linear algebra, statistics,
//! special functions, and compensated summation.

use crate::utils::input::{js_f64, js_f64_seq, js_opt_f64, js_opt_string, js_uint};
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
/// @returns Sample correlation in `[-1, 1]`; 0.0 when fewer than two points or either series is constant; NaN when `x` and `y` lengths differ.
#[wasm_bindgen(js_name = correlation)]
pub fn correlation(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    Ok(stats::correlation(x, y))
}

/// Sample covariance over typed numeric arrays.
/// @param x - First numeric series; must have the same length as `y`.
/// @param y - Second numeric series, aligned one-for-one with `x`.
/// @returns Unbiased sample covariance; 0.0 when fewer than two points; NaN when `x` and `y` lengths differ.
#[wasm_bindgen(js_name = covariance)]
pub fn covariance(x: JsValue, y: JsValue) -> Result<f64, JsValue> {
    let x: &[f64] = &js_f64_seq(&x, "x")?;
    let y: &[f64] = &js_f64_seq(&y, "y")?;
    Ok(stats::covariance(x, y))
}

/// Annualized realized variance of a close price series (Rust
/// `stats::realized_variance`): the mean of squared log returns times the
/// annualization factor, with no mean subtraction.
///
/// # Arguments
///
/// * `prices` - Close prices in time order; each must be finite and positive.
/// * `method` - Estimator name; only `"close_to_close"` applies to closes
///   (the OHLC estimators need `realizedVarianceOhlc`). Omitted selects the
///   Rust default (`"close_to_close"`).
/// * `annualization_factor` - Observations per year (for example `252` for
///   daily closes); omitted selects the Rust daily default (252).
///
/// @returns Annualized realized variance (decimal, not volatility).
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for an unknown or OHLC-only method, a non-positive or
/// non-finite price, or a non-positive annualization factor.
#[wasm_bindgen(js_name = realizedVariance)]
pub fn realized_variance(
    prices: JsValue,
    method: Option<JsValue>,
    annualization_factor: Option<JsValue>,
) -> Result<f64, JsValue> {
    let prices: &[f64] = &js_f64_seq(&prices, "prices")?;
    let method = realized_var_method(method)?;
    let annualization_factor = js_opt_f64(annualization_factor.as_ref(), "annualizationFactor")?;
    stats::realized_variance(prices, method, annualization_factor).map_err(to_js_err)
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
/// @returns Annualized realized variance (decimal, not volatility).
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for an unknown method, series of different lengths, an
/// invalid bar, or a non-positive annualization factor.
#[wasm_bindgen(js_name = realizedVarianceOhlc)]
pub fn realized_variance_ohlc(
    open: JsValue,
    high: JsValue,
    low: JsValue,
    close: JsValue,
    method: Option<JsValue>,
    annualization_factor: Option<JsValue>,
) -> Result<f64, JsValue> {
    let open: &[f64] = &js_f64_seq(&open, "open")?;
    let high: &[f64] = &js_f64_seq(&high, "high")?;
    let low: &[f64] = &js_f64_seq(&low, "low")?;
    let close: &[f64] = &js_f64_seq(&close, "close")?;
    let method = realized_var_method(method)?;
    let annualization_factor = js_opt_f64(annualization_factor.as_ref(), "annualizationFactor")?;
    stats::realized_variance_ohlc(open, high, low, close, method, annualization_factor)
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
/// `validation`) if `matrix` does not hold exactly `n * n` entries or contains
/// a non-finite value.
#[wasm_bindgen(js_name = symmetricEigen)]
pub fn symmetric_eigen(matrix: JsValue, n: JsValue) -> Result<js_sys::Array, JsValue> {
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
/// covariance as a flat row-major `n * n` `Float64Array`, and the optimal
/// shrinkage intensity in `[0, 1]`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `t < 2`, `n == 0`, an entry is non-finite, or
/// `observations` does not hold exactly `t * n` entries.
#[wasm_bindgen(js_name = ledoitWolfShrinkage)]
pub fn ledoit_wolf_shrinkage(
    observations: JsValue,
    t: JsValue,
    n: JsValue,
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

/// Pivot threshold below which Cholesky treats a matrix as singular (Rust
/// `linalg::SINGULAR_THRESHOLD`).
///
/// @returns The absolute pivot threshold.
#[wasm_bindgen(js_name = singularThreshold)]
pub fn singular_threshold() -> f64 {
    linalg::SINGULAR_THRESHOLD
}

/// Tolerance on the unit diagonal of a correlation matrix (Rust
/// `linalg::DIAGONAL_TOLERANCE`).
///
/// @returns The absolute tolerance on `|diagonal - 1|`.
#[wasm_bindgen(js_name = diagonalTolerance)]
pub fn diagonal_tolerance() -> f64 {
    linalg::DIAGONAL_TOLERANCE
}

/// Tolerance on the symmetry of a matrix (Rust `linalg::SYMMETRY_TOLERANCE`).
///
/// @returns The absolute tolerance on `|a[i][j] - a[j][i]|`.
#[wasm_bindgen(js_name = symmetryTolerance)]
pub fn symmetry_tolerance() -> f64 {
    linalg::SYMMETRY_TOLERANCE
}

/// Mean and sample variance in one pass (Rust `stats::mean_var`).
///
/// # Arguments
///
/// * `data` - Sample observations in input order.
///
/// @returns A two-element array `[mean, variance]`; `[0, 0]` for an empty series.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = meanVar)]
pub fn mean_var(data: JsValue) -> Result<Box<[f64]>, JsValue> {
    let (mean, variance) = stats::mean_var(&js_f64_seq(&data, "data")?);
    Ok(Box::new([mean, variance]))
}

/// Arithmetic mean with a NaN sentinel for missing data (Rust `stats::mean_or_nan`).
///
/// # Arguments
///
/// * `data` - Observations to average.
///
/// @returns The mean, or `NaN` for an empty series.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = meanOrNan)]
pub fn mean_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::mean_or_nan(&js_f64_seq(&data, "data")?))
}

/// Sample variance with a NaN sentinel (Rust `stats::sample_variance_or_nan`).
///
/// # Arguments
///
/// * `data` - Sample observations.
///
/// @returns The unbiased sample variance, or `NaN` for fewer than two observations.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = sampleVarianceOrNan)]
pub fn sample_variance_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::sample_variance_or_nan(&js_f64_seq(&data, "data")?))
}

/// Sample standard deviation with a NaN sentinel (Rust `stats::sample_std_or_nan`).
///
/// # Arguments
///
/// * `data` - Sample observations.
///
/// @returns The sample standard deviation, or `NaN` for fewer than two observations.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = sampleStdOrNan)]
pub fn sample_std_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::sample_std_or_nan(&js_f64_seq(&data, "data")?))
}

/// Median with a NaN sentinel (Rust `stats::median_or_nan`).
///
/// # Arguments
///
/// * `data` - Observations whose median is required; the input is not modified.
///
/// @returns The median, or `NaN` for an empty series.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = medianOrNan)]
pub fn median_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::median_or_nan(&js_f64_seq(&data, "data")?))
}

/// Linear-interpolation quantile (R-7, the NumPy and Excel default) with a
/// NaN sentinel (Rust `stats::quantile_linear_or_nan`).
///
/// # Arguments
///
/// * `data` - Finite observations; the input is not modified.
/// * `q` - Quantile probability; values outside `[0, 1]` are clamped to the
///   nearest endpoint.
///
/// @returns The quantile, or `NaN` for an empty series, a non-finite
/// observation or a NaN `q`.
/// @throws `TypeError` for a mistyped argument.
#[wasm_bindgen(js_name = quantileLinearOrNan)]
pub fn quantile_linear_or_nan(data: JsValue, q: JsValue) -> Result<f64, JsValue> {
    let q = js_f64(&q, "q")?;
    Ok(stats::quantile_linear_or_nan(
        &js_f64_seq(&data, "data")?,
        q,
    ))
}

/// Smallest finite value (Rust `stats::finite_min_or_nan`).
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are ignored.
///
/// @returns The minimum finite value, or `NaN` when none is finite.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = finiteMinOrNan)]
pub fn finite_min_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::finite_min_or_nan(&js_f64_seq(&data, "data")?))
}

/// Largest finite value (Rust `stats::finite_max_or_nan`).
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are ignored.
///
/// @returns The maximum finite value, or `NaN` when none is finite.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = finiteMaxOrNan)]
pub fn finite_max_or_nan(data: JsValue) -> Result<f64, JsValue> {
    Ok(stats::finite_max_or_nan(&js_f64_seq(&data, "data")?))
}

/// Number of finite observations (Rust `stats::finite_count`).
///
/// # Arguments
///
/// * `data` - Observations to inspect; NaN and infinite entries are not counted.
///
/// @returns The count of finite values.
/// @throws `TypeError` if `data` is not an array of numbers.
#[wasm_bindgen(js_name = finiteCount)]
pub fn finite_count(data: JsValue) -> Result<usize, JsValue> {
    Ok(stats::finite_count(&js_f64_seq(&data, "data")?))
}

/// Log returns of a price series (Rust `stats::log_returns`).
///
/// # Arguments
///
/// * `prices` - Chronologically ordered price levels.
///
/// @returns `prices.length - 1` values `ln(p[t] / p[t-1])`; a window with a
/// non-positive or non-finite price gives `NaN`. Empty for fewer than two prices.
/// @throws `TypeError` if `prices` is not an array of numbers.
#[wasm_bindgen(js_name = logReturns)]
pub fn log_returns(prices: JsValue) -> Result<Box<[f64]>, JsValue> {
    Ok(stats::log_returns(&js_f64_seq(&prices, "prices")?).into_boxed_slice())
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
/// `validation`) if `stdDev` is non-finite or not strictly positive.
#[wasm_bindgen(js_name = normCdfWithParams)]
pub fn norm_cdf_with_params(x: JsValue, mean: JsValue, std_dev: JsValue) -> Result<f64, JsValue> {
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
/// `validation`) if `stdDev` is non-finite or not strictly positive.
#[wasm_bindgen(js_name = normPdfWithParams)]
pub fn norm_pdf_with_params(x: JsValue, mean: JsValue, std_dev: JsValue) -> Result<f64, JsValue> {
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
/// * `x` - Point at which to evaluate the CDF; `NaN` returns `NaN`.
/// * `df` - Degrees of freedom, strictly positive.
///
/// @returns The probability that a Student-t variable with `df` degrees of
/// freedom is at most `x`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `df` is non-finite or not strictly positive.
#[wasm_bindgen(js_name = studentTCdf)]
pub fn student_t_cdf(x: JsValue, df: JsValue) -> Result<f64, JsValue> {
    special_functions::student_t_cdf(js_f64(&x, "x")?, js_f64(&df, "df")?).map_err(to_js_err)
}

/// Student-t inverse cumulative distribution function (Rust `student_t_inv_cdf`).
///
/// # Arguments
///
/// * `p` - Probability; `p <= 0` returns `-Infinity`, `p >= 1` returns
///   `Infinity` and `NaN` returns `NaN`.
/// * `df` - Degrees of freedom, strictly positive.
///
/// @returns The quantile `x` with `studentTCdf(x, df) === p`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `df` is non-finite or not strictly positive.
#[wasm_bindgen(js_name = studentTInvCdf)]
pub fn student_t_inv_cdf(p: JsValue, df: JsValue) -> Result<f64, JsValue> {
    special_functions::student_t_inv_cdf(js_f64(&p, "p")?, js_f64(&df, "df")?).map_err(to_js_err)
}
