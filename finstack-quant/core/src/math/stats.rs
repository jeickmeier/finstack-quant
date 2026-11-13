//! Statistical functions for financial time series analysis.
//!
//! Provides numerically stable implementations of basic statistics using
//! Welford's online algorithm and Kahan summation. All functions are
//! deterministic and produce identical results across platforms.
//!
//! # Algorithms
//!
//! - **Mean**: Kahan compensated summation for numerical stability
//! - **Variance**: Welford's one-pass algorithm (avoids catastrophic cancellation)
//! - **Covariance/Correlation**: Chan's parallel algorithm for stability
//!
//! # Use Cases
//!
//! - Portfolio risk metrics (volatility, correlation matrices)
//! - Time series analysis (returns, volatility estimation)
//! - Factor model estimation (principal components)
//! - Monte Carlo variance reduction (control variates)
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::stats::{mean, variance, covariance, correlation};
//! let xs = [1.0, 2.0, 3.0, 4.0];
//! let ys = [2.0, 4.0, 6.0, 8.0];
//! assert!((mean(&xs) - 2.5).abs() < 1e-12);
//! assert!(variance(&xs) > 0.0);
//! assert!(covariance(&xs, &ys) > 0.0);
//! assert!((correlation(&xs, &ys) - 1.0).abs() < 1e-12);
//! ```
//!
//! # References
//!
//! - **Welford's Algorithm**: `docs/REFERENCES.md#welford-1962`
//! - Welford, B. P. (1962). "Note on a Method for Calculating Corrected Sums of
//!   Squares and Products." *Technometrics*, 4(3), 419-420. `docs/REFERENCES.md#welford-1962`
//!
//! - **Chan's Parallel Algorithm**:
//! - Chan, T. F., Golub, G. H., & LeVeque, R. J. (1983). "Algorithms for Computing
//!   the Sample Variance: Analysis and Recommendations." *The American Statistician*,
//!   37(3), 242-247. `docs/REFERENCES.md#golub-van-loan-matrix-computations`
//!
//! - **Kahan Summation**: `docs/REFERENCES.md#kahan-1965`
//! - Kahan, W. (1965). "Further Remarks on Reducing Truncation Errors."
//!   *Communications of the ACM*, 8(1), 40. `docs/REFERENCES.md#kahan-1965`

use super::special_functions::student_t_inv_cdf;
use super::summation::kahan_sum;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// Arithmetic mean.
///
/// # Arguments
///
/// * `xs` - Observations to average. Values are used as supplied; an empty
///   slice returns `0.0`.
pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    // Use Kahan summation for numerical stability by default
    let s = kahan_sum(xs.iter().copied());
    s / xs.len() as f64
}

/// Sample variance (unbiased, n-1 denominator) using a single-pass Welford algorithm.
///
/// Matches Bloomberg, QuantLib, and the `OnlineStats::variance()` convention.
/// Returns `0.0` for fewer than 2 observations.
///
/// # Arguments
///
/// * `xs` - Sample observations whose unbiased variance is required. Fewer
///   than two observations produce `0.0`.
pub fn variance(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    online_stats(xs).variance()
}

/// Population variance (n denominator) using a single-pass Welford algorithm.
///
/// Use this when computing moments for the full population rather than
/// estimating from a sample (e.g., moment-matching for Monte Carlo).
/// Returns `0.0` for an empty slice.
///
/// # Arguments
///
/// * `xs` - Complete population of observations. An empty slice produces
///   `0.0` rather than an undefined value.
pub fn population_variance(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    online_stats(xs).population_variance()
}

/// Return (mean, variance) pair in a single Welford pass.
///
/// # Arguments
///
/// * `xs` - Sample observations used for both statistics. An empty slice
///   returns the pair `(0.0, 0.0)`.
pub fn mean_var(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let s = online_stats(xs);
    (s.mean(), s.variance())
}

/// Welford fold of `xs`.
fn online_stats(xs: &[f64]) -> OnlineStats {
    let mut s = OnlineStats::new();
    for &x in xs {
        s.update(x);
    }
    s
}

/// Welford fold of the pairs `(x[i], y[i])`; the slices must have equal length.
fn online_covariance(x: &[f64], y: &[f64]) -> OnlineCovariance {
    let mut oc = OnlineCovariance::new();
    for (&a, &b) in x.iter().zip(y) {
        oc.update(a, b);
    }
    oc
}

/// Arithmetic mean, returning [`f64::NAN`] for empty input.
///
/// # Arguments
///
/// * `xs` - Observations to average. Supply an empty slice only when `NaN` is
///   the desired missing-statistic sentinel.
pub fn mean_or_nan(xs: &[f64]) -> f64 {
    if xs.is_empty() { f64::NAN } else { mean(xs) }
}

/// Sample variance, returning [`f64::NAN`] for fewer than 2 observations.
///
/// # Arguments
///
/// * `xs` - Sample observations. Fewer than two values return `NaN` instead
///   of the `0.0` fallback used by [`variance`].
pub fn sample_variance_or_nan(xs: &[f64]) -> f64 {
    if xs.len() < 2 { f64::NAN } else { variance(xs) }
}

/// Sample standard deviation, returning [`f64::NAN`] for fewer than 2 observations.
///
/// # Arguments
///
/// * `xs` - Sample observations. Fewer than two values return `NaN` rather
///   than a zero standard deviation.
pub fn sample_std_or_nan(xs: &[f64]) -> f64 {
    sample_variance_or_nan(xs).sqrt()
}

/// Median of the input values, returning [`f64::NAN`] for empty input or when
/// any observation is non-finite.
///
/// # Arguments
///
/// * `xs` - Finite observations whose median is required. The input is copied
///   into a scratch buffer for partial selection, and an empty slice or
///   non-finite value returns `NaN`.
pub fn median_or_nan(xs: &[f64]) -> f64 {
    let mut buf = xs.to_vec();
    quantile(&mut buf, 0.5)
}

/// Minimum finite value, ignoring NaN / infinite entries. Returns [`f64::NAN`] if none are finite.
///
/// # Arguments
///
/// * `xs` - Observations to inspect. Non-finite values are ignored, and `NaN`
///   is returned when no finite observation remains.
pub fn finite_min_or_nan(xs: &[f64]) -> f64 {
    let mut min = f64::NAN;
    for &x in xs {
        if x.is_finite() && (min.is_nan() || x < min) {
            min = x;
        }
    }
    min
}

/// Maximum finite value, ignoring NaN / infinite entries. Returns [`f64::NAN`] if none are finite.
///
/// # Arguments
///
/// * `xs` - Observations to inspect. Non-finite values are ignored, and `NaN`
///   is returned when no finite observation remains.
pub fn finite_max_or_nan(xs: &[f64]) -> f64 {
    let mut max = f64::NAN;
    for &x in xs {
        if x.is_finite() && (max.is_nan() || x > max) {
            max = x;
        }
    }
    max
}

/// Count finite observations, excluding NaN / infinite entries.
///
/// # Arguments
///
/// * `xs` - Observations to inspect. Only finite floating-point values are
///   counted.
pub fn finite_count(xs: &[f64]) -> usize {
    xs.iter().copied().filter(|x| x.is_finite()).count()
}

/// Sample covariance (unbiased, n-1 denominator) between two equal-length slices.
///
/// Matches `OnlineCovariance::covariance()` convention. Returns `0.0` for
/// fewer than 2 observations.
///
/// Returns [`f64::NAN`] if slice lengths differ (pairing is undefined).
///
/// # Arguments
///
/// * `x` - First observation series. Each element is paired by index with the
///   corresponding element in `y`.
/// * `y` - Second observation series. It must have the same length as `x`;
///   otherwise the result is `NaN`.
pub fn covariance(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::NAN;
    }
    if x.len() < 2 {
        return 0.0;
    }
    online_covariance(x, y).covariance()
}

/// Pearson correlation in a single Welford pass via `OnlineCovariance`.
///
/// Returns [`f64::NAN`] if slice lengths differ (pairing is undefined).
///
/// # Arguments
///
/// * `x` - First observation series, paired element-by-element with `y`.
/// * `y` - Second observation series. It must have the same length as `x`;
///   unequal lengths return `NaN`.
pub fn correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::NAN;
    }
    if x.len() < 2 {
        return 0.0;
    }
    online_covariance(x, y).correlation()
}

/// Methods for calculating realized variance from price series.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RealizedVarMethod {
    /// Standard close-to-close returns (market standard default)
    #[default]
    CloseToClose,
    /// Parkinson (1980) high-low estimator
    Parkinson,
    /// Garman-Klass (1980) OHLC estimator
    GarmanKlass,
    /// Rogers-Satchell (1991) drift-independent OHLC estimator
    RogersSatchell,
    /// Yang-Zhang (2000) drift and opening gaps estimator
    YangZhang,
}

/// Displays the serde name (`close_to_close`, `yang_zhang`, ...).
impl std::fmt::Display for RealizedVarMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl RealizedVarMethod {
    /// Returns `true` if this method requires OHLC data.
    pub fn requires_ohlc(self) -> bool {
        !matches!(self, Self::CloseToClose)
    }

    /// Estimator [`realized_variance_ohlc`] uses when no method is given.
    ///
    /// Yang-Zhang (2000) is the minimum-variance OHLC estimator that is robust
    /// to both drift and opening gaps, so it is the natural choice once all
    /// four bar fields are supplied. The close-series entry point
    /// [`realized_variance`] instead defaults to [`RealizedVarMethod::default`]
    /// (`CloseToClose`), the only method it supports.
    pub const OHLC_DEFAULT: Self = Self::YangZhang;
}

/// Annualization factor applied when a realized-variance caller passes none:
/// [`PeriodKind::Daily`](crate::dates::PeriodKind::Daily) observations, i.e.
/// 252 trading days per year.
fn default_annualization_factor() -> f64 {
    f64::from(crate::dates::PeriodKind::Daily.annualization_factor())
}

/// Calculate log returns from a price series.
///
/// Each element is `ln(p_t / p_{t-1})` when both prices are finite and strictly
/// positive. Windows with a non-positive prior price, non-finite values, or a
/// non-positive ratio (including total wipeout to a zero price) return
/// [`f64::NAN`] so downstream variance estimates do not silently absorb
/// infinities.
///
/// # Arguments
///
/// * `prices` - Chronologically ordered price levels. Adjacent prices must be
///   finite and strictly positive to produce a finite return.
pub fn log_returns(prices: &[f64]) -> Vec<f64> {
    if prices.len() < 2 {
        return vec![];
    }
    prices
        .windows(2)
        .map(|w| {
            let p0 = w[0];
            let p1 = w[1];
            if p0 <= 0.0 || !p0.is_finite() || !p1.is_finite() {
                return f64::NAN;
            }
            let ratio = p1 / p0;
            if !ratio.is_finite() || ratio <= 0.0 {
                f64::NAN
            } else {
                ratio.ln()
            }
        })
        .collect()
}

/// Empirical quantile via partial sort.
///
/// # Interpolation method
///
/// Returns the `p`-th quantile (0 ≤ p ≤ 1) using **linear interpolation
/// between adjacent order statistics** — specifically R's `type = 7` /
/// NumPy's default `linear` mode, which computes
///
/// ```text
/// h = (n - 1) · p
/// lo = floor(h), hi = lo + 1
/// result = data[lo] + (h - lo) · (data[hi] - data[lo])
/// ```
///
/// This is **not** the SAS default (R's `type = 3`), the Excel default
/// (`type = 7` for `PERCENTILE.INC`, `type = 6` for `PERCENTILE.EXC`),
/// nor "nearest" or "lower" quantile. Backtest reproducibility across
/// tools requires picking the same method — compare against NumPy's
/// `np.quantile(..., method='linear')` or R's `quantile(..., type = 7)`.
///
/// # Mutation and cost
///
/// The input slice is **mutated** (partially sorted via
/// [`slice::select_nth_unstable_by`]) to avoid allocation. Cost is
/// expected O(n). Returns [`f64::NAN`] if the slice is empty, `p` is
/// outside `[0, 1]`, or any input value is non-finite.
///
/// # Arguments
///
/// * `data` - Finite observations to partially sort in place. The function
///   mutates their order to avoid allocating a sorted copy.
/// * `p` - Quantile probability in the inclusive `0.0..=1.0` range, using the
///   R-7/NumPy linear interpolation convention.
pub fn quantile(data: &mut [f64], p: f64) -> f64 {
    let n = data.len();
    if n == 0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if data.iter().any(|x| !x.is_finite()) {
        return f64::NAN;
    }
    let Ok((lo, frac)) = r7_rank(n, p) else {
        return f64::NAN;
    };
    data.select_nth_unstable_by(lo, f64::total_cmp);
    let v_lo = data[lo];
    if frac.is_zero() {
        return v_lo;
    }
    data[lo + 1..].select_nth_unstable_by(0, f64::total_cmp);
    interpolate_r7(v_lo, data[lo + 1], &frac)
        .and_then(|value| rational_to_f64(&value, false))
        .unwrap_or(f64::NAN)
}

/// Empirical R-7 quantile of data that is already sorted ascending.
///
/// Same estimator as [`quantile`] (`h = (n - 1) · p`, linear interpolation
/// between the bracketing order statistics) for callers that sort once and
/// read several quantiles; the slice is not modified and finiteness is not
/// re-checked.
///
/// # Arguments
///
/// * `sorted` - Observations sorted ascending (for example with
///   [`f64::total_cmp`]).
/// * `p` - Quantile probability in the inclusive `0.0..=1.0` range.
///
/// # Returns
///
/// The interpolated quantile, or [`f64::NAN`] when `sorted` is empty or `p`
/// is outside `[0, 1]`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::quantile_sorted;
///
/// let sorted = [1.0, 2.0, 3.0, 4.0];
/// assert_eq!(quantile_sorted(&sorted, 0.5), 2.5);
/// assert_eq!(quantile_sorted(&sorted, 1.0), 4.0);
/// assert!(quantile_sorted(&[], 0.5).is_nan());
/// ```
pub fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    quantile_rational(sorted, p)
        .and_then(|value| rational_to_f64(&value, false))
        .unwrap_or(f64::NAN)
}

/// Magnitude ratio of two R-7 quantiles, rounded only after division.
///
/// Both quantiles retain their exact binary-rational rank weights and
/// interpolation values until their magnitude ratio is formed. Individually
/// unrepresentable component quantiles therefore do not prevent a
/// representable requested ratio. This also preserves consequential residuals
/// at the final ratio's rounding boundary. A genuinely zero denominator gives
/// positive infinity for a nonzero numerator, or NaN for two zero quantiles.
///
/// # Arguments
///
/// * `sorted` - Nonempty finite observations in ascending order. Their
///   order is validated, and the slice is not modified.
/// * `numerator_p` - Numerator quantile probability in the inclusive
///   `0.0..=1.0` range, interpreted as its supplied binary64 value.
/// * `denominator_p` - Denominator quantile probability in the inclusive
///   `0.0..=1.0` range, interpreted as its supplied binary64 value.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for empty, nonfinite or unordered
/// observations, or an invalid probability. Returns
/// [`crate::Error::Internal`] for an unrepresentable requested nonzero finite
/// ratio; arithmetic underflow never creates an observed zero ratio.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::quantile_ratio;
/// let tiny = f64::from_bits(1);
/// assert_eq!(quantile_ratio(&[0.0, tiny], 0.75, 0.25)?, 3.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn quantile_ratio(sorted: &[f64], numerator_p: f64, denominator_p: f64) -> crate::Result<f64> {
    validate_sorted_sample(sorted)?;
    let numerator = quantile_rational(sorted, numerator_p)?.abs();
    let denominator = quantile_rational(sorted, denominator_p)?.abs();
    if denominator.is_zero() {
        return Ok(if numerator.is_zero() {
            f64::NAN
        } else {
            f64::INFINITY
        });
    }
    rational_to_f64(&(numerator / denominator), true)
}

/// Checked R-7 quantile with exact rank weights and one final output rounding.
///
/// This evaluates the same estimator as [`quantile_sorted`] while rejecting
/// a nonzero mathematical result that would round to zero. Use
/// [`quantile_ratio`] when a ratio is requested jointly instead of
/// independently requested scalar quantiles.
///
/// # Arguments
///
/// * `sorted` - Nonempty finite observations in ascending order. The slice
///   and its ordering are validated and are not modified.
/// * `p` - Probability in the inclusive `0.0..=1.0` range, interpreted as
///   its supplied binary64 value when forming the R-7 rank and weights.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for invalid observations or
/// probability, or [`crate::Error::Internal`] when a nonzero quantile
/// cannot be represented as a nonzero finite binary64 result.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::quantile_checked;
/// assert_eq!(quantile_checked(&[1.0, 2.0, 3.0, 4.0], 0.5)?, 2.5);
/// assert!(quantile_checked(&[0.0, f64::from_bits(1)], 0.25).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn quantile_checked(sorted: &[f64], p: f64) -> crate::Result<f64> {
    validate_sorted_sample(sorted)?;
    rational_to_f64(&quantile_rational(sorted, p)?, true)
}

/// Mean of exactly the lowest `p` mass of an equal-weight empirical sample.
///
/// With `mass = p * sorted.len()`, the calculation includes every observation
/// below `floor(mass)` and exactly the required fraction of the next one.
/// Ties do not expand the tail. Integer counts and supplied binary64 values
/// are evaluated exactly before the final rounding, preserving consequential
/// near-integer mass residuals, cancellation and subnormal observations.
/// This is an empirical tail mean, not an integral of the R-7 quantile curve.
///
/// # Arguments
///
/// * `sorted` - Nonempty finite observations in ascending order. The slice
///   is validated and is not modified; values can be positive or negative.
/// * `p` - Strictly positive selected probability mass, at most `1.0`.
///   A mass smaller than one observation returns the smallest observation.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for an empty, nonfinite or unordered
/// sample or a probability outside `(0, 1]`. Returns
/// [`crate::Error::Internal`] for an unrepresentable nonzero requested mean.
/// Genuine zero means remain valid; no floor or clipping is applied.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::empirical_tail_mean;
/// assert!((empirical_tail_mean(&[-0.2, -0.1, 0.0], 0.5)? + 1.0 / 6.0).abs() < 1e-15);
/// let tiny = f64::from_bits(1);
/// assert_eq!(empirical_tail_mean(&[tiny; 5], 0.4)?, tiny);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn empirical_tail_mean(sorted: &[f64], p: f64) -> crate::Result<f64> {
    validate_sorted_sample(sorted)?;
    if !(p > 0.0 && p <= 1.0) {
        return Err(crate::Error::Validation(
            "Empirical tail probability must be in (0, 1]".into(),
        ));
    }
    let (whole, fraction) = split_mass(sorted.len(), p)?;
    if whole == 0 {
        return Ok(sorted[0]);
    }
    let mut total = BigRational::zero();
    for &value in &sorted[..whole] {
        total += binary_rational(value)?;
    }
    if !fraction.is_zero() {
        total += binary_rational(sorted[whole])? * &fraction;
    }
    let mass = BigRational::from_integer(BigInt::from(whole)) + fraction;
    rational_to_f64(&(total / mass), true)
}

fn validate_sorted_sample(sorted: &[f64]) -> crate::Result<()> {
    if sorted.is_empty()
        || sorted.iter().any(|value| !value.is_finite())
        || sorted.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(crate::Error::Validation(
            "A nonempty finite ascending sample is required".into(),
        ));
    }
    Ok(())
}

fn binary_rational(value: f64) -> crate::Result<BigRational> {
    BigRational::from_float(value).ok_or_else(|| {
        crate::Error::Validation("Exact statistical weights require finite values".into())
    })
}

/// Keep the integer rank and its fractional remainder separate and exact.
fn r7_rank(n: usize, p: f64) -> crate::Result<(usize, BigRational)> {
    if n == 0 || !(0.0..=1.0).contains(&p) {
        return Err(crate::Error::Validation(
            "R-7 requires observations and a probability in [0, 1]".into(),
        ));
    }
    split_mass(n - 1, p)
}

fn split_mass(count: usize, p: f64) -> crate::Result<(usize, BigRational)> {
    let mass = binary_rational(p)? * BigInt::from(count);
    let whole = mass.to_integer().to_usize().ok_or_else(|| {
        crate::Error::Internal("Statistical rank does not fit an observation index".into())
    })?;
    let fraction = mass - BigRational::from_integer(BigInt::from(whole));
    Ok((whole, fraction))
}

fn interpolate_r7(lower: f64, upper: f64, fraction: &BigRational) -> crate::Result<BigRational> {
    if lower.to_bits() == upper.to_bits() || fraction.is_zero() {
        return binary_rational(lower);
    }
    Ok(binary_rational(lower)? * (BigRational::one() - fraction)
        + binary_rational(upper)? * fraction)
}

fn quantile_rational(sorted: &[f64], p: f64) -> crate::Result<BigRational> {
    let (lo, fraction) = r7_rank(sorted.len(), p)?;
    let lower = sorted[lo];
    if fraction.is_zero() {
        return binary_rational(lower);
    }
    interpolate_r7(lower, sorted[lo + 1], &fraction)
}

fn rational_to_f64(value: &BigRational, reject_zero_underflow: bool) -> crate::Result<f64> {
    let rounded = value.to_f64().ok_or_else(|| {
        crate::Error::Internal("Statistical result could not be reconstructed".into())
    })?;
    if !rounded.is_finite()
        || (reject_zero_underflow
            && rounded.classify() == std::num::FpCategory::Zero
            && !value.is_zero())
    {
        return Err(crate::Error::Internal(
            "Statistical result is outside the nonzero finite binary64 range".into(),
        ));
    }
    Ok(rounded)
}

/// Calculate annualized close-to-close realized variance from a close price series.
///
/// Only the `CloseToClose` method is supported; OHLC-based estimators
/// (`Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`) require
/// intraday high/low/open data and must be called via
/// [`realized_variance_ohlc`].
///
/// For `CloseToClose`, this function returns the annualized mean of squared log
/// returns. That is a close-to-close second moment estimator, not a de-meaned
/// sample variance. The name follows common market terminology, but callers
/// comparing against statistical variance formulas should account for that
/// convention explicitly.
///
/// # Arguments
/// * `prices` - Close price series ordered in time; every price must be finite
///   and strictly positive.
/// * `annualization_factor` - Observations per year used to annualize the
///   per-period variance (for example `252.0` for daily closes); must be
///   finite and positive. `None` selects the daily convention,
///   `PeriodKind::Daily.annualization_factor()` (252).
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if the annualization
/// factor is not finite and positive, or a price is not finite and positive.
///
/// # Returns
/// Annualized realized variance
pub fn realized_variance(prices: &[f64], annualization_factor: Option<f64>) -> crate::Result<f64> {
    let annualization_factor = annualization_factor.unwrap_or_else(default_annualization_factor);
    if !annualization_factor.is_finite() || annualization_factor <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "realized_variance: annualization_factor must be positive and finite, got {annualization_factor}"
        )));
    }
    if prices
        .iter()
        .any(|price| !price.is_finite() || *price <= 0.0)
    {
        return Err(crate::Error::Validation(
            "realized_variance: prices must be finite and positive".into(),
        ));
    }
    if prices.len() < 2 {
        return Ok(0.0);
    }

    let returns = log_returns(prices);
    let var =
        returns.iter().map(|r| r * r).sum::<f64>() / returns.len() as f64 * annualization_factor;
    if !var.is_finite() || var < 0.0 {
        return Err(crate::Error::Validation(
            "realized_variance: non-positive or non-finite prices produce undefined log returns"
                .into(),
        ));
    }
    Ok(var)
}

/// Calculate realized variance from OHLC data using advanced estimators.
///
/// All five methods are supported. For `CloseToClose`, only the `close` slice
/// is used; the other three legs may be the same slice as `close` or empty.
///
/// # Arguments
/// * `open` - Opening prices (required for `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `high` - High prices (required for `Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `low` - Low prices (required for `Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `close` - Closing prices (required for all methods)
/// * `method` - Estimator to use. `None` selects
///   [`RealizedVarMethod::OHLC_DEFAULT`] (Yang-Zhang).
/// * `annualization_factor` - Bars per year used to annualize the per-bar
///   variance (for example `252.0` for daily bars); must be finite and
///   positive. `None` selects the daily convention,
///   `PeriodKind::Daily.annualization_factor()` (252).
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if the four slices
/// have different lengths, the annualization factor is not finite and
/// positive, or a bar is not finite, positive and internally consistent.
///
/// # Returns
/// Annualized realized variance. Parkinson, Garman-Klass, and Rogers-Satchell
/// use each complete OHLC bar, including a single bar. Close-to-close requires
/// two closing levels, and Yang-Zhang requires three bars; shorter samples
/// return zero.
pub fn realized_variance_ohlc(
    open: &[f64],
    high: &[f64],
    low: &[f64],
    close: &[f64],
    method: Option<RealizedVarMethod>,
    annualization_factor: Option<f64>,
) -> crate::Result<f64> {
    let method = method.unwrap_or(RealizedVarMethod::OHLC_DEFAULT);
    let annualization_factor = annualization_factor.unwrap_or_else(default_annualization_factor);
    let n = close.len();
    if open.len() != n || high.len() != n || low.len() != n {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc: open ({}), high ({}), low ({}) and close ({}) \
             series must have the same length",
            open.len(),
            high.len(),
            low.len(),
            n
        )));
    }
    if !annualization_factor.is_finite() || annualization_factor <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc: annualization_factor must be positive and finite, got {annualization_factor}"
        )));
    }
    for i in 0..n {
        let (o, h, l, c) = (open[i], high[i], low[i], close[i]);
        if !o.is_finite()
            || !h.is_finite()
            || !l.is_finite()
            || !c.is_finite()
            || o <= 0.0
            || h <= 0.0
            || l <= 0.0
            || c <= 0.0
        {
            return Err(crate::Error::Validation(format!(
                "realized_variance_ohlc: bar {i} must contain finite positive OHLC values"
            )));
        }
        if l > o.min(c) || h < o.max(c) || l > h {
            return Err(crate::Error::Validation(format!(
                "realized_variance_ohlc: bar {i} violates low <= min(open, close) <= max(open, close) <= high"
            )));
        }
    }
    if n == 0 {
        return Ok(0.0);
    }

    let result = match method {
        RealizedVarMethod::CloseToClose => realized_variance(close, Some(annualization_factor)),
        RealizedVarMethod::Parkinson => {
            // Parkinson (1980) high-low range estimator
            // More efficient than close-to-close, using intraday range information
            //
            // Formula: σ² = [1/(4·ln(2))] · (1/n) · Σ[ln(H/L)]²
            //
            // Reference: Parkinson, M. (1980). "The Extreme Value Method for
            // Estimating the Variance of the Rate of Return."
            // Journal of Business, 53(1), 61-65.
            let sum: f64 = (0..n)
                .map(|i| {
                    let hl_ratio = high[i] / low[i];
                    (hl_ratio.ln()).powi(2)
                })
                .sum();
            let factor = 1.0 / (4.0 * std::f64::consts::LN_2);
            Ok((sum / n as f64) * factor * annualization_factor)
        }
        RealizedVarMethod::GarmanKlass => {
            // Garman-Klass (1980) OHLC estimator
            // Extends Parkinson by incorporating open and close prices for improved efficiency
            //
            // Formula: σ² = (1/n) · Σ[0.5·[ln(H/L)]² - (2·ln(2) - 1)·[ln(C/O)]²]
            //
            // The coefficient (2·ln(2) - 1) ≈ 0.386 is the optimal weight for the
            // close-open component under the assumption of Brownian motion.
            //
            // Reference: Garman, M. B., & Klass, M. J. (1980). "On the Estimation of
            // Security Price Volatilities from Historical Data."
            // Journal of Business, 53(1), 67-78.
            let gk_coeff = 2.0 * std::f64::consts::LN_2 - 1.0;
            let sum: f64 = (0..n)
                .map(|i| {
                    let hl = (high[i] / low[i]).ln();
                    let co = (close[i] / open[i]).ln();
                    0.5 * hl.powi(2) - gk_coeff * co.powi(2)
                })
                .sum();
            Ok((sum / n as f64) * annualization_factor)
        }
        RealizedVarMethod::RogersSatchell => {
            // Rogers-Satchell (1991) drift-independent OHLC estimator
            // Allows for non-zero drift, making it more robust than Parkinson or Garman-Klass
            // when the underlying asset has significant directional movement.
            //
            // Formula: σ² = (1/n) · Σ[ln(H/C)·ln(H/O) + ln(L/C)·ln(L/O)]
            //
            // Reference: Rogers, L. C. G., & Satchell, S. E. (1991). "Estimating Variance
            // From High, Low and Closing Prices." Annals of Applied Probability, 1(4), 504-512.
            let sum: f64 = (0..n)
                .map(|i| {
                    let hc = (high[i] / close[i]).ln();
                    let ho = (high[i] / open[i]).ln();
                    let lc = (low[i] / close[i]).ln();
                    let lo = (low[i] / open[i]).ln();
                    hc * ho + lc * lo
                })
                .sum();
            Ok((sum / n as f64) * annualization_factor)
        }
        RealizedVarMethod::YangZhang => {
            // Yang-Zhang needs at least 3 OHLC bars (2 overnight returns for variance)
            if n < 3 {
                return Ok(0.0);
            }
            // Yang-Zhang (2000) estimator: includes overnight jumps and opening gaps
            // Combines overnight variance with Rogers-Satchell intraday variance
            // using optimal weighting to minimize bias and variance of the estimator.
            //
            // Reference: Yang, D., & Zhang, Q. (2000). "Drift-Independent Volatility
            // Estimation Based on High, Low, Open, and Close Prices."
            // Journal of Business, 73(3), 477-491.

            /// Yang-Zhang optimal weight numerator for combining variance components
            const YANG_ZHANG_K_NUMERATOR: f64 = 0.34;
            /// Yang-Zhang optimal weight denominator base adjustment
            const YANG_ZHANG_K_DENOMINATOR_BASE: f64 = 1.34;

            let mut overnight_returns = Vec::with_capacity(n.saturating_sub(1));
            let mut open_close_returns = Vec::with_capacity(n.saturating_sub(1));
            let mut sum_rs = 0.0;

            for i in 1..n {
                // Overnight component
                let overnight = (open[i] / close[i - 1]).ln();
                overnight_returns.push(overnight);

                // Open-to-close component
                let open_close = (close[i] / open[i]).ln();
                open_close_returns.push(open_close);

                // Rogers-Satchell component for intraday
                let hc = (high[i] / close[i]).ln();
                let ho = (high[i] / open[i]).ln();
                let lc = (low[i] / close[i]).ln();
                let lo = (low[i] / open[i]).ln();
                sum_rs += hc * ho + lc * lo;
            }

            // Number of return observations (periods): n bars yield n−1
            // overnight/open-close/RS observations (bar 0 has no previous
            // close). Yang & Zhang (2000), Eq. (10), define
            // k = 0.34 / (1.34 + (n+1)/(n−1)) in terms of the number of
            // *periods* in the estimator, so we use m = n−1 here.
            let m = n - 1;
            let k = YANG_ZHANG_K_NUMERATOR
                / (YANG_ZHANG_K_DENOMINATOR_BASE + (m + 1) as f64 / (m - 1) as f64);
            let var_overnight = variance(&overnight_returns);
            let var_open_close = variance(&open_close_returns);
            // RS component averages over the same m = n−1 periods that
            // contribute overnight and open-close returns (Yang & Zhang 2000,
            // Eq. (4)); dividing by the bar count n would bias it down by
            // (n−1)/n.
            let var_rs = sum_rs / m as f64;

            Ok((var_overnight + k * var_open_close + (1.0 - k) * var_rs) * annualization_factor)
        }
    }?;
    if !result.is_finite() || result < -1.0e-14 {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc produced invalid variance {result}"
        )));
    }
    Ok(result.max(0.0))
}

/// Online statistics accumulator using Welford's algorithm.
///
/// This provides numerically stable computation of mean and variance
/// in a single pass, which is critical for Monte Carlo where we
/// process millions of samples.
#[derive(Clone, Debug, Default)]
pub struct OnlineStats {
    count: usize,
    mean: f64,
    m2: f64, // Sum of squared differences from current mean
}

impl OnlineStats {
    /// Create a new empty statistics accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with a new sample.
    ///
    /// Uses Welford's algorithm for numerical stability.
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Merge with another statistics accumulator.
    ///
    /// Uses Chan, Golub & LeVeque (1979) parallel algorithm:
    /// `combined_m2 = m2_a + m2_b + δ² · n_a · n_b / (n_a + n_b)`
    /// where `δ = mean_b − mean_a`. This is numerically stable even
    /// when the two sub-populations have very different means.
    pub fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other.clone();
            return;
        }

        let total_count = self.count + other.count;
        let delta = other.mean - self.mean;
        let combined_m2 = self.m2
            + other.m2
            + delta * delta * (self.count as f64) * (other.count as f64) / (total_count as f64);

        self.mean = (self.count as f64 * self.mean + other.count as f64 * other.mean)
            / (total_count as f64);
        self.m2 = combined_m2;
        self.count = total_count;
    }

    /// Number of samples.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Sample mean.
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Sample variance (unbiased estimator with n-1 denominator).
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / (self.count - 1) as f64
    }

    /// Population variance (n denominator).
    pub fn population_variance(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.m2 / self.count as f64
    }

    /// Sample standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Standard error of the mean (σ / √n).
    ///
    /// Returns [`f64::NAN`] for 0 or 1 samples because the standard error
    /// requires an estimate of σ, which is undefined with fewer than 2
    /// observations.
    pub fn stderr(&self) -> f64 {
        if self.count <= 1 {
            return f64::NAN;
        }
        self.std_dev() / (self.count as f64).sqrt()
    }

    /// Student-t confidence interval for the population mean.
    ///
    /// Uses the sample standard deviation and `count - 1` degrees of freedom.
    /// Coverage is exact for independent Gaussian observations and approximate
    /// for other distributions when their sample means are approximately normal.
    /// For randomized quasi-Monte Carlo, update this accumulator with independent
    /// replicate means, not the dependent paths within each replicate.
    ///
    /// Returns `(NaN, NaN)` when fewer than two samples are available or `alpha`
    /// is non-finite or outside `(0, 1)`, because no interval can be estimated.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Finite significance probability strictly between zero and
    ///   one; for example, `0.05` requests a two-sided 95% interval.
    ///
    /// # Returns
    ///
    /// Lower and upper bounds in the observations' units, or two `NaN` values
    /// when the sample size or significance probability is invalid.
    pub fn confidence_interval(&self, alpha: f64) -> (f64, f64) {
        if self.count <= 1 || !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return (f64::NAN, f64::NAN);
        }
        // Use the lower tail and symmetry to avoid rounding 1 - alpha/2 to 1.
        let critical = match student_t_inv_cdf(alpha / 2.0, (self.count - 1) as f64) {
            Ok(value) => -value,
            Err(_) => return (f64::NAN, f64::NAN),
        };
        let margin = critical * self.stderr();
        (self.mean - margin, self.mean + margin)
    }

    /// Half-width of the 95% confidence interval.
    pub fn ci_half_width(&self) -> f64 {
        let (lower, upper) = self.confidence_interval(0.05);
        (upper - lower) / 2.0
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }
}

/// Online covariance accumulator using Welford's algorithm.
///
/// Computes mean, variance, and covariance for two variables in a single pass.
/// This is essential for control variate estimation where we need covariance
/// between MC samples and control variate samples without storing all values.
///
/// Uses the parallel formula from Chan et al. (1979):
/// C_{n+1} = C_n + (x - mean_x) * (y - mean_y') where mean_y' is the new mean
#[derive(Clone, Debug, Default)]
pub struct OnlineCovariance {
    count: usize,
    mean_x: f64,
    mean_y: f64,
    m2_x: f64, // Sum of squared differences for x
    m2_y: f64, // Sum of squared differences for y
    c: f64,    // Co-moment sum: Σ(x_i - mean_x)(y_i - mean_y)
}

impl OnlineCovariance {
    /// Create a new empty covariance accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with a new sample pair (x, y).
    ///
    /// Uses Welford's algorithm extended to covariance.
    pub fn update(&mut self, x: f64, y: f64) {
        self.count += 1;
        let n = self.count as f64;

        let dx = x - self.mean_x;
        let dy = y - self.mean_y;
        self.mean_x += dx / n;
        self.mean_y += dy / n;

        self.m2_x += dx * (x - self.mean_x);
        self.m2_y += dy * (y - self.mean_y);
        self.c += dx * (y - self.mean_y);
    }

    /// Merge with another covariance accumulator.
    pub fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other.clone();
            return;
        }

        let total_count = self.count + other.count;
        let delta_x = other.mean_x - self.mean_x;
        let delta_y = other.mean_y - self.mean_y;

        self.c += other.c
            + delta_x * delta_y * (self.count as f64) * (other.count as f64) / (total_count as f64);
        self.m2_x += other.m2_x
            + delta_x * delta_x * (self.count as f64) * (other.count as f64) / (total_count as f64);
        self.m2_y += other.m2_y
            + delta_y * delta_y * (self.count as f64) * (other.count as f64) / (total_count as f64);
        self.mean_x = (self.count as f64 * self.mean_x + other.count as f64 * other.mean_x)
            / (total_count as f64);
        self.mean_y = (self.count as f64 * self.mean_y + other.count as f64 * other.mean_y)
            / (total_count as f64);
        self.count = total_count;
    }

    /// Number of samples.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Sample mean of x.
    pub fn mean_x(&self) -> f64 {
        self.mean_x
    }

    /// Sample mean of y.
    pub fn mean_y(&self) -> f64 {
        self.mean_y
    }

    /// Sample variance of x (unbiased).
    pub fn variance_x(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2_x / (self.count - 1) as f64
    }

    /// Sample variance of y (unbiased).
    pub fn variance_y(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2_y / (self.count - 1) as f64
    }

    /// Sample covariance (unbiased).
    pub fn covariance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.c / (self.count - 1) as f64
    }

    /// Sample correlation.
    pub fn correlation(&self) -> f64 {
        let var_x = self.variance_x();
        let var_y = self.variance_y();
        let denom = (var_x * var_y).sqrt();

        // Return 0 only when at least one series is genuinely constant (exact
        // IEEE zero), rather than using a hard-coded absolute threshold that
        // misfires for data with a very small natural scale.
        if denom == 0.0 {
            return 0.0;
        }

        (self.covariance() / denom).clamp(-1.0, 1.0)
    }

    /// Optimal beta coefficient for control variate.
    ///
    /// Returns Cov(X, Y) / Var(Y), the coefficient that minimizes
    /// the variance of `X - beta(Y - E[Y])`.
    pub fn optimal_beta(&self) -> f64 {
        let var_y = self.variance_y();
        if var_y == 0.0 {
            return 0.0;
        }
        self.covariance() / var_y
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean_x = 0.0;
        self.mean_y = 0.0;
        self.m2_x = 0.0;
        self.m2_y = 0.0;
        self.c = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::special_functions::standard_normal_inv_cdf;

    #[test]
    fn range_estimators_use_each_observed_bar_from_the_first() {
        let open: [f64; 3] = [100.0, 101.0, 99.0];
        let high: [f64; 3] = [103.0, 104.0, 102.0];
        let low: [f64; 3] = [98.0, 97.0, 96.0];
        let close: [f64; 3] = [102.0, 99.0, 100.0];
        for method in [
            RealizedVarMethod::Parkinson,
            RealizedVarMethod::GarmanKlass,
            RealizedVarMethod::RogersSatchell,
        ] {
            assert_eq!(
                realized_variance_ohlc(&[], &[], &[], &[], Some(method), Some(252.0)).unwrap(),
                0.0
            );
            for n in 1..=3 {
                let sum: f64 = (0..n)
                    .map(|i| {
                        let hl = (high[i] / low[i]).ln();
                        let co = (close[i] / open[i]).ln();
                        match method {
                            RealizedVarMethod::Parkinson => {
                                hl * hl / (4.0 * std::f64::consts::LN_2)
                            }
                            RealizedVarMethod::GarmanKlass => {
                                0.5 * hl * hl - (2.0 * std::f64::consts::LN_2 - 1.0) * co * co
                            }
                            RealizedVarMethod::RogersSatchell => {
                                (high[i] / close[i]).ln() * (high[i] / open[i]).ln()
                                    + (low[i] / close[i]).ln() * (low[i] / open[i]).ln()
                            }
                            _ => unreachable!(),
                        }
                    })
                    .sum();
                let actual = realized_variance_ohlc(
                    &open[..n],
                    &high[..n],
                    &low[..n],
                    &close[..n],
                    Some(method),
                    Some(252.0),
                )
                .unwrap();
                assert!((actual - 252.0 * sum / n as f64).abs() < 1e-12);
                assert!(actual > 0.0);
            }
        }
        for n in 1..=2 {
            assert_eq!(
                realized_variance_ohlc(
                    &open[..n],
                    &high[..n],
                    &low[..n],
                    &close[..n],
                    Some(RealizedVarMethod::YangZhang),
                    Some(252.0),
                )
                .unwrap(),
                0.0
            );
        }
        assert_eq!(
            realized_variance_ohlc(
                &open[..1],
                &high[..1],
                &low[..1],
                &close[..1],
                Some(RealizedVarMethod::CloseToClose),
                Some(252.0),
            )
            .unwrap(),
            0.0
        );
    }

    #[test]
    fn test_online_stats_basic() {
        let mut stats = OnlineStats::new();
        stats.update(1.0);
        stats.update(2.0);
        stats.update(3.0);

        assert_eq!(stats.count(), 3);
        assert_eq!(stats.mean(), 2.0);
        assert_eq!(stats.variance(), 1.0);
        assert_eq!(stats.std_dev(), 1.0);
    }

    #[test]
    fn test_online_stats_merge() {
        let mut stats1 = OnlineStats::new();
        stats1.update(1.0);
        stats1.update(2.0);

        let mut stats2 = OnlineStats::new();
        stats2.update(3.0);
        stats2.update(4.0);

        stats1.merge(&stats2);
        assert_eq!(stats1.count(), 4);
        assert_eq!(stats1.mean(), 2.5);
    }

    #[test]
    fn test_online_stats_merge_empty_accumulators() {
        let mut stats = OnlineStats::new();
        stats.update(1.0);
        stats.update(3.0);
        let before = stats.clone();

        stats.merge(&OnlineStats::new());
        assert_eq!(stats.count(), before.count());
        assert_eq!(stats.mean(), before.mean());
        assert_eq!(stats.variance(), before.variance());

        let mut empty = OnlineStats::new();
        empty.merge(&stats);
        assert_eq!(empty.count(), stats.count());
        assert_eq!(empty.mean(), stats.mean());
        assert_eq!(empty.variance(), stats.variance());
    }

    #[test]
    fn test_confidence_intervals() {
        let mut stats = OnlineStats::new();
        for i in 1..=100 {
            stats.update(i as f64);
        }

        let (lower, upper) = stats.confidence_interval(0.05);
        assert!(lower < stats.mean());
        assert!(upper > stats.mean());
        assert!(lower < 50.5 && upper > 50.5);
    }

    #[test]
    fn confidence_intervals_account_for_estimated_small_sample_variance() {
        let mut pair = OnlineStats::new();
        pair.update(0.0);
        pair.update(2.0);
        // With one degree of freedom Student-t is Cauchy, giving an
        // independent analytic critical value cot(pi * alpha / 2).
        let critical = 1.0 / (std::f64::consts::PI * 0.025).tan();
        let (lower, upper) = pair.confidence_interval(0.05);
        assert!((lower - (1.0 - critical)).abs() < 1e-10);
        assert!((upper - (1.0 + critical)).abs() < 1e-10);

        let mut replicates = OnlineStats::new();
        for value in 1..=16 {
            replicates.update(f64::from(value));
        }
        // Tabulated 97.5th percentile for 15 degrees of freedom, appropriate
        // to the 16 independent replicate means used by the RQMC pricer.
        let expected_half_width = 2.131_449_545_559_323 * replicates.stderr();
        assert!((replicates.ci_half_width() - expected_half_width).abs() < 1e-10);
    }

    #[test]
    fn confidence_intervals_reject_insufficient_samples_and_invalid_alpha() {
        let mut stats = OnlineStats::new();
        for count in 0..=1 {
            if count == 1 {
                stats.update(42.0);
            }
            let (lower, upper) = stats.confidence_interval(0.05);
            assert!(lower.is_nan() && upper.is_nan());
            assert!(stats.ci_half_width().is_nan());
        }
        stats.update(43.0);
        for alpha in [0.0, 1.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
            let (lower, upper) = stats.confidence_interval(alpha);
            assert!(lower.is_nan() && upper.is_nan());
        }
    }

    #[test]
    fn test_standard_normal_inv_cdf() {
        let z_95 = standard_normal_inv_cdf(0.975);
        assert!((z_95 - 1.96).abs() < 0.01);

        let z_99 = standard_normal_inv_cdf(0.995);
        assert!((z_99 - 2.576).abs() < 0.01);

        assert!((standard_normal_inv_cdf(0.5)).abs() < 0.01);
        assert!((standard_normal_inv_cdf(0.25) + standard_normal_inv_cdf(0.75)).abs() < 0.01);
    }

    #[test]
    fn covariance_rejects_mismatched_lengths_with_nan() {
        let x = [1.0, 2.0, 3.0];
        let y = [1.0, 2.0];
        assert!(covariance(&x, &y).is_nan());
    }

    #[test]
    fn correlation_rejects_mismatched_lengths_with_nan() {
        let x = [1.0, 2.0, 3.0];
        let y = [1.0, 2.0];
        assert!(correlation(&x, &y).is_nan());
    }

    #[test]
    fn variance_matches_online_stats() {
        let datasets: &[&[f64]] = &[
            &[1.0, 2.0, 3.0, 4.0, 5.0],
            &[0.0, 0.0, 0.0, 1.0],
            &[-3.0, -1.0, 0.0, 1.0, 3.0],
            &[1e6, 1e6 + 1.0, 1e6 + 2.0],
            &[1e-12, 2e-12, 3e-12],
        ];
        for xs in datasets {
            let mut s = OnlineStats::new();
            for &x in *xs {
                s.update(x);
            }
            assert!(
                (variance(xs) - s.variance()).abs() < 1e-12,
                "variance mismatch for {:?}",
                xs
            );
            assert!(
                (population_variance(xs) - s.population_variance()).abs() < 1e-12,
                "population_variance mismatch for {:?}",
                xs
            );
            let (m, v) = mean_var(xs);
            assert!(
                (m - s.mean()).abs() < 1e-12,
                "mean_var mean mismatch for {:?}",
                xs
            );
            assert!(
                (v - s.variance()).abs() < 1e-12,
                "mean_var variance mismatch for {:?}",
                xs
            );
        }
    }

    #[test]
    fn test_online_covariance_basic() {
        let mut cov = OnlineCovariance::new();
        cov.update(1.0, 2.0);
        cov.update(2.0, 4.0);
        cov.update(3.0, 6.0);

        assert_eq!(cov.count(), 3);
        assert_eq!(cov.mean_x(), 2.0);
        assert_eq!(cov.mean_y(), 4.0);
        assert!((cov.correlation() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_online_covariance_negative() {
        let mut cov = OnlineCovariance::new();
        cov.update(1.0, 6.0);
        cov.update(2.0, 4.0);
        cov.update(3.0, 2.0);

        assert!((cov.correlation() + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_online_covariance_uncorrelated() {
        let mut cov = OnlineCovariance::new();
        cov.update(1.0, 1.0);
        cov.update(1.0, -1.0);
        cov.update(-1.0, 1.0);
        cov.update(-1.0, -1.0);

        assert!(cov.correlation().abs() < 1e-10);
    }

    #[test]
    fn test_online_covariance_merge() {
        let mut cov1 = OnlineCovariance::new();
        cov1.update(1.0, 2.0);
        cov1.update(2.0, 4.0);

        let mut cov2 = OnlineCovariance::new();
        cov2.update(3.0, 6.0);
        cov2.update(4.0, 8.0);

        cov1.merge(&cov2);

        assert_eq!(cov1.count(), 4);
        assert_eq!(cov1.mean_x(), 2.5);
        assert_eq!(cov1.mean_y(), 5.0);
        assert!((cov1.correlation() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_online_covariance_matches_batch() {
        let data: Vec<(f64, f64)> =
            vec![(1.0, 2.1), (2.0, 3.9), (3.0, 6.2), (4.0, 7.8), (5.0, 10.1)];

        let mut online = OnlineCovariance::new();
        for &(x, y) in &data {
            online.update(x, y);
        }

        let n = data.len() as f64;
        let mean_x: f64 = data.iter().map(|(x, _)| x).sum::<f64>() / n;
        let mean_y: f64 = data.iter().map(|(_, y)| y).sum::<f64>() / n;
        let cov_batch: f64 = data
            .iter()
            .map(|(x, y)| (x - mean_x) * (y - mean_y))
            .sum::<f64>()
            / (n - 1.0);

        assert!((online.mean_x() - mean_x).abs() < 1e-10);
        assert!((online.mean_y() - mean_y).abs() < 1e-10);
        assert!((online.covariance() - cov_batch).abs() < 1e-10);
    }

    #[test]
    fn test_online_covariance_optimal_beta() {
        let mut cov = OnlineCovariance::new();
        cov.update(1.0, 2.0);
        cov.update(2.0, 4.0);
        cov.update(3.0, 6.0);

        assert!((cov.optimal_beta() - 0.5).abs() < 1e-10);
    }

    // ── H8 regression: tiny-scale perfectly correlated series ──
    //
    // With data at scale 1e-12 the old `var < 1e-20` guard fires incorrectly
    // because (1e-12)^2 = 1e-24 < 1e-20. The new implementation should detect
    // the true correlation = 1.0.
    #[test]
    fn test_online_covariance_tiny_scale_perfect_correlation() {
        let scale = 1.0e-12_f64;
        let mut cov = OnlineCovariance::new();
        cov.update(1.0 * scale, 2.0 * scale);
        cov.update(2.0 * scale, 4.0 * scale);
        cov.update(3.0 * scale, 6.0 * scale);
        let r = cov.correlation();
        assert!(
            (r - 1.0).abs() < 1e-10,
            "Tiny-scale perfect correlation should be 1.0, got {r}"
        );
    }

    // ── H8 regression: genuinely constant series returns 0 ──
    //
    // A series where all X values are identical has zero variance; the
    // correlation is undefined and we return 0 as the safe sentinel.
    #[test]
    fn test_online_covariance_constant_series_returns_zero() {
        let mut cov = OnlineCovariance::new();
        cov.update(5.0, 1.0);
        cov.update(5.0, 2.0);
        cov.update(5.0, 3.0);
        assert_eq!(
            cov.correlation(),
            0.0,
            "Constant X series must return correlation 0.0"
        );
    }

    // ── log_returns tests ──

    #[test]
    fn log_returns_empty_and_single_yield_empty() {
        assert!(super::log_returns(&[]).is_empty());
        assert!(super::log_returns(&[100.0]).is_empty());
    }

    #[test]
    fn log_returns_matches_ln_ratio_for_positive_prices() {
        let lr = super::log_returns(&[100.0, 110.0, 99.0]);
        assert_eq!(lr.len(), 2);
        assert!((lr[0] - 1.1_f64.ln()).abs() < 1e-12);
        assert!((lr[1] - (99.0_f64 / 110.0).ln()).abs() < 1e-12);
    }

    #[test]
    fn log_returns_non_positive_or_non_finite_prices_yield_nan() {
        assert!(super::log_returns(&[100.0, 0.0])[0].is_nan());
        assert!(super::log_returns(&[0.0, 100.0])[0].is_nan());
        assert!(super::log_returns(&[100.0, f64::NAN])[0].is_nan());
        assert!(super::log_returns(&[f64::NAN, 100.0])[0].is_nan());
    }

    // ── quantile tests ──

    #[test]
    fn quantile_empty_returns_nan() {
        let mut data: Vec<f64> = vec![];
        assert!(super::quantile(&mut data, 0.5).is_nan());
    }

    #[test]
    fn quantile_single_element() {
        let mut data = vec![42.0];
        assert!((super::quantile(&mut data, 0.0) - 42.0).abs() < 1e-12);
        assert!((super::quantile(&mut data, 0.5) - 42.0).abs() < 1e-12);
        assert!((super::quantile(&mut data, 1.0) - 42.0).abs() < 1e-12);
    }

    #[test]
    fn quantile_p0_returns_min() {
        let mut data = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        assert!((super::quantile(&mut data, 0.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn quantile_p1_returns_max() {
        let mut data = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        assert!((super::quantile(&mut data, 1.0) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn quantile_median_odd() {
        let mut data = vec![3.0, 1.0, 2.0, 5.0, 4.0];
        assert!((super::quantile(&mut data, 0.5) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn quantile_median_even_interpolates() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0];
        // R-7: h = 3 * 0.5 = 1.5 → lerp(data[1], data[2], 0.5) = 2.5
        assert!((super::quantile(&mut data, 0.5) - 2.5).abs() < 1e-12);
    }

    #[test]
    fn quantile_out_of_range_returns_nan() {
        let mut data = vec![1.0, 2.0, 3.0];
        assert!(super::quantile(&mut data, -0.1).is_nan());
        assert!(super::quantile(&mut data, 1.1).is_nan());
    }

    #[test]
    fn quantile_non_finite_inputs_returns_nan() {
        let mut data = vec![1.0, f64::NAN, 3.0];
        assert!(super::quantile(&mut data, 0.5).is_nan());
        let mut data_inf = vec![1.0, f64::INFINITY, 3.0];
        assert!(super::quantile(&mut data_inf, 0.5).is_nan());
    }

    #[test]
    fn quantile_all_equal() {
        let mut data = vec![7.0; 10];
        assert!((super::quantile(&mut data, 0.25) - 7.0).abs() < 1e-12);
        assert!((super::quantile(&mut data, 0.75) - 7.0).abs() < 1e-12);
    }

    #[test]
    fn helper_kernels_match_expected_nan_contracts() {
        assert!(super::mean_or_nan(&[]).is_nan());
        assert!(super::sample_variance_or_nan(&[42.0]).is_nan());
        assert!(super::sample_std_or_nan(&[42.0]).is_nan());
        assert!(super::median_or_nan(&[]).is_nan());
    }

    #[test]
    fn helper_kernels_skip_non_finite_for_extrema_and_count() {
        let values = [3.0, f64::NAN, -1.0, f64::INFINITY, 5.0];
        assert_eq!(super::finite_min_or_nan(&values), -1.0);
        assert_eq!(super::finite_max_or_nan(&values), 5.0);
        assert_eq!(super::finite_count(&values), 3);
    }

    #[test]
    fn quantile_matches_r7_interpolation() {
        let mut values = [1.0, 2.0, 3.0, 4.0];
        assert!((super::quantile(&mut values, 0.5) - 2.5).abs() < 1e-12);
    }

    #[test]
    fn quantile_preserves_a_rank_residual_rounded_away_by_multiplication() {
        // Binary64 1/3 is below its mathematical value: 3*p = 1 - 2^-54.
        // The small remaining weight on -2^53 contributes exactly -1/2.
        let sorted = [-2.0_f64.powi(53), 0.0, 1.0, 2.0];
        assert_eq!(super::quantile_sorted(&sorted, 1.0 / 3.0), -0.5);
        let mut data = sorted;
        assert_eq!(super::quantile(&mut data, 1.0 / 3.0), -0.5);
    }

    #[test]
    fn quantile_interpolates_opposite_extremes_without_overflow() {
        let sorted = [-f64::MAX, f64::MAX];
        assert_eq!(super::quantile_sorted(&sorted, 0.5), 0.0);
        let p = 0.5 + f64::EPSILON;
        assert_eq!(
            super::quantile_sorted(&sorted, p),
            f64::MAX * (2.0 * p - 1.0)
        );
    }

    #[test]
    fn quantile_ratio_preserves_unrepresentable_components_and_rounding_residuals() {
        let tiny = f64::from_bits(1);
        assert_eq!(
            super::quantile_ratio(&[0.0, tiny], 0.75, 0.25).expect("joint ratio"),
            3.0
        );
        // The lower magnitude is .75 - 2^-57, while the upper is .375*tiny.
        // The exact ratio is just above tiny/2 and rounds to tiny; rounding
        // the lower quantile first would give the tie tiny/2 and erase it.
        assert_eq!(
            super::quantile_ratio(&[-1.0, -1.0 / 3.0, -tiny, 0.0], 0.875, 0.125)
                .expect("ratio above underflow midpoint"),
            tiny
        );
        assert!(super::quantile_checked(&[0.0, tiny], 0.25).is_err());
        assert_eq!(
            super::quantile_checked(&[0.0, tiny], 0.75).expect("rounded nonzero"),
            tiny
        );
    }

    #[test]
    fn empirical_tail_mean_preserves_fractional_mass_and_subnormal_constants() {
        // Binary64 .1 is above 1/10: 10*p = 1 + 2^-54. The boundary
        // weight on 2^53 contributes 1/2, even though ordinary 10*p is 1.
        let mut sorted = [2.0_f64.powi(53); 10];
        sorted[0] = 0.0;
        assert_eq!(super::empirical_tail_mean(&sorted, 0.1).expect("tail"), 0.5);
        let tiny = f64::from_bits(1);
        assert_eq!(
            super::empirical_tail_mean(&[tiny; 5], 0.4).expect("tiny constant"),
            tiny
        );
        assert!(super::empirical_tail_mean(&[0.0, tiny], 1.0).is_err());
    }

    #[test]
    fn checked_quantiles_and_tail_means_reject_invalid_requests_and_samples() {
        for sorted in [&[][..], &[1.0, 0.0], &[0.0, f64::INFINITY]] {
            assert!(super::quantile_ratio(sorted, 0.75, 0.25).is_err());
            assert!(super::quantile_checked(sorted, 0.5).is_err());
            assert!(super::empirical_tail_mean(sorted, 0.5).is_err());
        }
        for p in [f64::NAN, -0.1, 1.1] {
            assert!(super::quantile_ratio(&[0.0, 1.0], p, 0.25).is_err());
            assert!(super::quantile_ratio(&[0.0, 1.0], 0.75, p).is_err());
            assert!(super::quantile_checked(&[0.0, 1.0], p).is_err());
            assert!(super::empirical_tail_mean(&[0.0, 1.0], p).is_err());
        }
        assert!(super::empirical_tail_mean(&[0.0, 1.0], 0.0).is_err());
    }

    // ── RealizedVarMethod FromStr / Display roundtrip ──

    #[test]
    fn realized_var_method_label_is_the_serde_name() {
        use super::RealizedVarMethod;

        for method in [
            RealizedVarMethod::CloseToClose,
            RealizedVarMethod::Parkinson,
            RealizedVarMethod::GarmanKlass,
            RealizedVarMethod::RogersSatchell,
            RealizedVarMethod::YangZhang,
        ] {
            let label = crate::wire::serde_label(&method).expect("label");
            assert_eq!(label, method.to_string());
            let parsed: RealizedVarMethod = crate::wire::serde_parse(&label).expect("parse");
            assert_eq!(parsed, method);
        }
        assert!(crate::wire::serde_parse::<RealizedVarMethod>("YangZhang").is_err());
        assert!(crate::wire::serde_parse::<RealizedVarMethod>("garbage").is_err());
    }

    #[test]
    fn realized_variance_rejects_invalid_ohlc_and_annualization() {
        use super::{RealizedVarMethod, realized_variance, realized_variance_ohlc};

        let open = [100.0, 101.0];
        let high = [102.0, 103.0];
        let low = [99.0, 100.0];
        let close = [101.0, 102.0];
        assert!(
            realized_variance_ohlc(
                &open,
                &high,
                &[0.0, 100.0],
                &close,
                Some(RealizedVarMethod::Parkinson),
                Some(252.0),
            )
            .is_err()
        );
        assert!(
            realized_variance_ohlc(
                &[f64::NAN, 101.0],
                &high,
                &low,
                &close,
                Some(RealizedVarMethod::GarmanKlass),
                Some(252.0),
            )
            .is_err()
        );
        assert!(
            realized_variance_ohlc(
                &open,
                &[98.0, 103.0],
                &low,
                &close,
                Some(RealizedVarMethod::RogersSatchell),
                Some(252.0),
            )
            .is_err()
        );
        assert!(
            realized_variance_ohlc(
                &open,
                &high,
                &low,
                &close,
                Some(RealizedVarMethod::YangZhang),
                Some(f64::INFINITY),
            )
            .is_err()
        );
        assert!(realized_variance(&close, Some(-1.0)).is_err());
    }

    #[test]
    fn realized_variance_defaults_are_rust_owned() {
        use super::{RealizedVarMethod, realized_variance, realized_variance_ohlc};

        let open = [100.0, 101.5, 100.8, 102.0];
        let high = [102.0, 103.0, 102.5, 103.5];
        let low = [99.0, 100.2, 99.9, 101.0];
        let close = [101.0, 102.0, 101.5, 103.0];
        let daily = f64::from(crate::dates::PeriodKind::Daily.annualization_factor());
        assert_eq!(daily, 252.0);

        assert_eq!(
            realized_variance(&close, None).expect("defaults"),
            realized_variance(&close, Some(daily)).expect("explicit"),
        );
        assert_eq!(
            RealizedVarMethod::OHLC_DEFAULT,
            RealizedVarMethod::YangZhang
        );
        assert_eq!(
            realized_variance_ohlc(&open, &high, &low, &close, None, None).expect("defaults"),
            realized_variance_ohlc(
                &open,
                &high,
                &low,
                &close,
                Some(RealizedVarMethod::YangZhang),
                Some(daily),
            )
            .expect("explicit"),
        );
    }
}
