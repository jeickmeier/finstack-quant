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
//! use finstack_core::math::{mean, variance, covariance, correlation};
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
//! - **Welford's Algorithm**:
//!   - Welford, B. P. (1962). "Note on a Method for Calculating Corrected Sums of
//!     Squares and Products." *Technometrics*, 4(3), 419-420.
//!
//! - **Chan's Parallel Algorithm**:
//!   - Chan, T. F., Golub, G. H., & LeVeque, R. J. (1983). "Algorithms for Computing
//!     the Sample Variance: Analysis and Recommendations." *The American Statistician*,
//!     37(3), 242-247.
//!
//! - **Kahan Summation**:
//!   - Kahan, W. (1965). "Further Remarks on Reducing Truncation Errors."
//!     *Communications of the ACM*, 8(1), 40.

use super::special_functions::standard_normal_inv_cdf;
use super::summation::kahan_sum;

/// Arithmetic mean.
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
pub fn variance(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let mut s = OnlineStats::new();
    for &x in xs {
        s.update(x);
    }
    s.variance()
}

/// Population variance (n denominator) using a single-pass Welford algorithm.
///
/// Use this when computing moments for the full population rather than
/// estimating from a sample (e.g., moment-matching for Monte Carlo).
/// Returns `0.0` for an empty slice.
pub fn population_variance(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = OnlineStats::new();
    for &x in xs {
        s.update(x);
    }
    s.population_variance()
}

/// Return (mean, variance) pair in a single Welford pass.
pub fn mean_var(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let mut s = OnlineStats::new();
    for &x in xs {
        s.update(x);
    }
    (s.mean(), s.variance())
}

/// Arithmetic mean, returning [`f64::NAN`] for empty input.
pub fn mean_or_nan(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        f64::NAN
    } else {
        mean(xs)
    }
}

/// Sample variance, returning [`f64::NAN`] for fewer than 2 observations.
pub fn sample_variance_or_nan(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        f64::NAN
    } else {
        variance(xs)
    }
}

/// Sample standard deviation, returning [`f64::NAN`] for fewer than 2 observations.
pub fn sample_std_or_nan(xs: &[f64]) -> f64 {
    sample_variance_or_nan(xs).sqrt()
}

/// Median of the input values, returning [`f64::NAN`] for empty input.
pub fn median_or_nan(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let len = sorted.len();
    if len.is_multiple_of(2) {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    }
}

/// Linear-interpolation quantile (R-7 / NumPy / Excel style), returning [`f64::NAN`] for
/// empty input or when any input value is non-finite (matching the mutating
/// [`quantile`] variant's NaN policy).
pub fn quantile_linear_or_nan(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    // Same non-finite policy as `quantile`: NaN/±inf entries have no
    // meaningful order statistics, so reject them instead of letting
    // total_cmp sort NaN to the top.
    if xs.iter().any(|x| !x.is_finite()) {
        return f64::NAN;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let q = q.clamp(0.0, 1.0);
    let index = q * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = index - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// Minimum finite value, ignoring NaN / infinite entries. Returns [`f64::NAN`] if none are finite.
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
pub fn finite_count(xs: &[f64]) -> usize {
    xs.iter().copied().filter(|x| x.is_finite()).count()
}

/// Sample covariance (unbiased, n-1 denominator) between two equal-length slices.
///
/// Matches `OnlineCovariance::covariance()` convention. Returns `0.0` for
/// fewer than 2 observations.
///
/// Returns [`f64::NAN`] if slice lengths differ (pairing is undefined).
pub fn covariance(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::NAN;
    }
    if x.len() < 2 {
        return 0.0;
    }
    let mut oc = OnlineCovariance::new();
    for i in 0..x.len() {
        oc.update(x[i], y[i]);
    }
    oc.covariance()
}

/// Pearson correlation in a single Welford pass via `OnlineCovariance`.
///
/// Returns [`f64::NAN`] if slice lengths differ (pairing is undefined).
pub fn correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::NAN;
    }
    let n = x.len();
    if n < 2 {
        return 0.0;
    }
    let mut oc = OnlineCovariance::new();
    for i in 0..n {
        oc.update(x[i], y[i]);
    }
    oc.correlation()
}

/// Moment matching: adjust samples to have exact mean and variance.
///
/// This variance reduction technique forces the sample to have
/// exactly the theoretical moments.
///
/// # Arguments
///
/// * `samples` - Mutable slice of samples to adjust
/// * `target_mean` - Target mean (default 0.0 for standard normal)
/// * `target_std` - Target standard deviation (default 1.0 for standard normal)
pub fn moment_match(samples: &mut [f64], target_mean: f64, target_std: f64) {
    if samples.is_empty() {
        return;
    }

    let current_mean = mean(samples);
    let current_var = population_variance(samples);
    let current_std = current_var.sqrt();

    // Degenerate-spread guard, relative to the sample scale: a flat 1e-10
    // would wrongly treat genuinely dispersed dollar-scale samples (std 1e-9
    // of a 1e6 mean is degenerate) and tiny-scale samples (std 1e-11 around
    // a 1e-10 mean is meaningful spread) the same way.
    let scale = samples.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let degenerate_threshold = 1e-10 * scale.max(1.0);

    // Adjust samples
    if current_std > degenerate_threshold {
        for x in samples.iter_mut() {
            *x = (*x - current_mean) * (target_std / current_std) + target_mean;
        }
    }
}

// ====== Realized Variance Calculations ======

/// Methods for calculating realized variance from price series.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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

impl RealizedVarMethod {
    /// Returns the canonical string label for this method.
    pub fn label(self) -> &'static str {
        match self {
            Self::CloseToClose => "close_to_close",
            Self::Parkinson => "parkinson",
            Self::GarmanKlass => "garman_klass",
            Self::RogersSatchell => "rogers_satchell",
            Self::YangZhang => "yang_zhang",
        }
    }

    /// Returns `true` if this method requires OHLC data.
    pub fn requires_ohlc(self) -> bool {
        !matches!(self, Self::CloseToClose)
    }
}

// ---------------------------------------------------------------------------
// Display + FromStr
// ---------------------------------------------------------------------------

impl std::fmt::Display for RealizedVarMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl crate::parse::NormalizedEnum for RealizedVarMethod {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("close_to_close", Self::CloseToClose),
        ("close", Self::CloseToClose),
        ("closetoclose", Self::CloseToClose),
        ("parkinson", Self::Parkinson),
        ("garman_klass", Self::GarmanKlass),
        ("garmanklass", Self::GarmanKlass),
        ("rogers_satchell", Self::RogersSatchell),
        ("rogerssatchell", Self::RogersSatchell),
        ("yang_zhang", Self::YangZhang),
        ("yangzhang", Self::YangZhang),
    ];
}

impl std::str::FromStr for RealizedVarMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
    }
}

/// Calculate log returns from a price series.
///
/// Each element is `ln(p_t / p_{t-1})` when both prices are finite and strictly
/// positive. Windows with a non-positive prior price, non-finite values, or a
/// non-positive ratio (including total wipeout to a zero price) return
/// [`f64::NAN`] so downstream variance estimates do not silently absorb
/// infinities.
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
pub fn quantile(data: &mut [f64], p: f64) -> f64 {
    let n = data.len();
    if n == 0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if data.iter().any(|x| !x.is_finite()) {
        return f64::NAN;
    }
    if n == 1 {
        return data[0];
    }
    let h = (n - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = lo + 1;
    let frac = h - lo as f64;
    data.select_nth_unstable_by(lo, |a, b| {
        a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal)
    });
    let v_lo = data[lo];
    if hi >= n || frac == 0.0 {
        return v_lo;
    }
    data[lo + 1..].select_nth_unstable_by(0, |a, b| {
        a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal)
    });
    let v_hi = data[lo + 1];
    v_lo + frac * (v_hi - v_lo)
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
/// * `prices` - Close price series ordered in time
/// * `method` - Must be `CloseToClose`; OHLC-only methods return an error
/// * `annualization_factor` - Factor to annualize variance (e.g., 252 for daily data)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if `method` requires OHLC data.
///
/// # Returns
/// Annualized realized variance
pub fn realized_variance(
    prices: &[f64],
    method: RealizedVarMethod,
    annualization_factor: f64,
) -> crate::Result<f64> {
    if prices.len() < 2 {
        return Ok(0.0);
    }

    match method {
        RealizedVarMethod::CloseToClose => {
            let returns = log_returns(prices);
            let var = returns.iter().map(|r| r * r).sum::<f64>() / returns.len() as f64
                * annualization_factor;
            if var.is_nan() {
                return Err(crate::Error::Validation(
                    "realized_variance: non-positive or non-finite prices produce undefined log returns".into(),
                ));
            }
            Ok(var)
        }
        RealizedVarMethod::Parkinson
        | RealizedVarMethod::GarmanKlass
        | RealizedVarMethod::RogersSatchell
        | RealizedVarMethod::YangZhang => Err(crate::Error::Validation(format!(
            "Realized variance method '{}' requires OHLC data. \
             Use realized_variance_ohlc() with open, high, low, and close price series.",
            method.label()
        ))),
    }
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
/// * `method` - Method to use for calculation
/// * `annualization_factor` - Factor to annualize variance
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if the four slices have different lengths.
///
/// # Returns
/// Annualized realized variance
pub fn realized_variance_ohlc(
    open: &[f64],
    high: &[f64],
    low: &[f64],
    close: &[f64],
    method: RealizedVarMethod,
    annualization_factor: f64,
) -> crate::Result<f64> {
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
    if n < 2 {
        return Ok(0.0);
    }

    match method {
        RealizedVarMethod::CloseToClose => realized_variance(close, method, annualization_factor),
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
            let mean_overnight =
                overnight_returns.iter().sum::<f64>() / overnight_returns.len() as f64;
            let mean_open_close =
                open_close_returns.iter().sum::<f64>() / open_close_returns.len() as f64;
            let var_overnight = overnight_returns
                .iter()
                .map(|r| {
                    let d = r - mean_overnight;
                    d * d
                })
                .sum::<f64>()
                / (overnight_returns.len() - 1) as f64;
            let var_open_close = open_close_returns
                .iter()
                .map(|r| {
                    let d = r - mean_open_close;
                    d * d
                })
                .sum::<f64>()
                / (open_close_returns.len() - 1) as f64;
            // RS component averages over the same m = n−1 periods that
            // contribute overnight and open-close returns (Yang & Zhang 2000,
            // Eq. (4)); dividing by the bar count n would bias it down by
            // (n−1)/n.
            let var_rs = sum_rs / m as f64;

            Ok((var_overnight + k * var_open_close + (1.0 - k) * var_rs) * annualization_factor)
        }
    }
}

// ====== Online (Streaming) Statistics ======

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

    /// Confidence interval at specified level.
    ///
    /// Returns `(mean, mean)` when fewer than 2 samples are available
    /// (standard error is undefined so no interval can be constructed).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Significance level (e.g., 0.05 for 95% CI)
    ///
    /// # Returns
    ///
    /// (lower, upper) bounds of the confidence interval.
    pub fn confidence_interval(&self, alpha: f64) -> (f64, f64) {
        if self.count <= 1 {
            return (self.mean, self.mean);
        }
        let z = standard_normal_inv_cdf(1.0 - alpha / 2.0);
        let margin = z * self.stderr();
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

/// Compute relative error bound for target confidence.
///
/// Returns the number of samples required to achieve a target
/// relative error (standard error / mean) at a given confidence level.
///
/// # Arguments
///
/// * `cv` - Coefficient of variation (σ / μ)
/// * `target_rel_error` - Target relative standard error
/// * `alpha` - Significance level (0.05 for 95% confidence)
///
/// # Returns
///
/// Minimum number of samples required.
pub fn required_samples(cv: f64, target_rel_error: f64, alpha: f64) -> usize {
    let z = standard_normal_inv_cdf(1.0 - alpha / 2.0);
    let n = (z * cv / target_rel_error).powi(2);
    n.ceil() as usize
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

        // Update means (Welford's algorithm)
        let dx = x - self.mean_x;
        let dy = y - self.mean_y;
        self.mean_x += dx / n;
        self.mean_y += dy / n;

        // Update variance and covariance sums
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

    fn assert_parses_to(label: &str, expected: RealizedVarMethod) {
        assert!(matches!(label.parse::<RealizedVarMethod>(), Ok(value) if value == expected));
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
    fn test_moment_match() {
        let mut samples = vec![-1.5, -0.5, 0.0, 0.5, 1.5];
        moment_match(&mut samples, 0.0, 1.0);

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var = samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;

        assert!(mean.abs() < 1e-10);
        assert!((var - 1.0).abs() < 1e-10);
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
    fn test_required_samples() {
        let n = required_samples(1.0, 0.01, 0.05);
        assert!(n > 38000);
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
        assert!(super::quantile_linear_or_nan(&[], 0.5).is_nan());
    }

    #[test]
    fn helper_kernels_skip_non_finite_for_extrema_and_count() {
        let values = [3.0, f64::NAN, -1.0, f64::INFINITY, 5.0];
        assert_eq!(super::finite_min_or_nan(&values), -1.0);
        assert_eq!(super::finite_max_or_nan(&values), 5.0);
        assert_eq!(super::finite_count(&values), 3);
    }

    #[test]
    fn quantile_linear_helper_matches_r7_interpolation() {
        let values = [1.0, 2.0, 3.0, 4.0];
        assert!((super::quantile_linear_or_nan(&values, 0.5) - 2.5).abs() < 1e-12);
    }

    // ── RealizedVarMethod FromStr / Display roundtrip ──

    #[test]
    fn realized_var_method_display_roundtrip() {
        use super::RealizedVarMethod;

        let all = [
            RealizedVarMethod::CloseToClose,
            RealizedVarMethod::Parkinson,
            RealizedVarMethod::GarmanKlass,
            RealizedVarMethod::RogersSatchell,
            RealizedVarMethod::YangZhang,
        ];

        for method in &all {
            let label = method.to_string();
            assert!(
                matches!(label.parse::<RealizedVarMethod>(), Ok(value) if value == *method),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn realized_var_method_from_str_aliases() {
        use super::RealizedVarMethod;

        assert_parses_to("close", RealizedVarMethod::CloseToClose);
        assert_parses_to("closetoclose", RealizedVarMethod::CloseToClose);
        assert_parses_to("garmanklass", RealizedVarMethod::GarmanKlass);
        assert_parses_to("rogerssatchell", RealizedVarMethod::RogersSatchell);
        assert_parses_to("yangzhang", RealizedVarMethod::YangZhang);
    }

    #[test]
    fn realized_var_method_from_str_unknown() {
        assert!("garbage".parse::<super::RealizedVarMethod>().is_err());
    }
}
