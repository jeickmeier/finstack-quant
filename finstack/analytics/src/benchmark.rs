//! Benchmark-relative metrics: tracking error, information ratio, beta, greeks.
//!
//! [`beta`] and the result types ([`BetaResult`], [`GreeksResult`],
//! [`RollingGreeks`], [`MultiFactorResult`]) are re-exported at the crate
//! root. Everything else is crate-internal; `///` doc examples target crate
//! developers and are marked `ignore`.
//!
//! Delegates to `math::stats` for core statistics (correlation, covariance,
//! variance, OnlineCovariance).

use crate::dates::Date;
use crate::math::stats::{correlation, mean, OnlineCovariance, OnlineStats};
use finstack_core::math::neumaier_sum;

// Recompute the four sliding-window sums (sr, sb, srb, sb²) every 64 steps to
// bound drift from incremental add/remove updates without turning the whole
// calculation into O(n * window). The greeks kernel maintains four cross-term
// sums that compound floating-point error roughly proportionally to the number
// of running quantities, so we recompute roughly 16× more often than the
// single-mean rolling kernels in `risk_metrics::rolling`
// (`ROLLING_KERNEL_RECOMPUTE_INTERVAL = 1024`). The 64-step interval is
// validated by `rolling_greeks_stays_close_to_exact_recomputation_on_long_series`,
// which compares against full re-computation across a long synthetic series.
const ROLLING_GREEKS_RECOMPUTE_INTERVAL: usize = 64;

#[inline]
fn recompute_rolling_greeks_sums(returns: &[f64], benchmark: &[f64]) -> (f64, f64, f64, f64) {
    (
        neumaier_sum(returns.iter().copied()),
        neumaier_sum(benchmark.iter().copied()),
        neumaier_sum(returns.iter().zip(benchmark.iter()).map(|(&r, &b)| r * b)),
        neumaier_sum(benchmark.iter().map(|&b| b * b)),
    )
}

#[inline]
fn compensated_add(sum: &mut f64, compensation: &mut f64, value: f64) {
    let y = value - *compensation;
    let t = *sum + y;
    *compensation = (t - *sum) - y;
    *sum = t;
}

/// Tracking error: annualized volatility of active (excess) returns.
///
/// Measures how consistently a portfolio follows its benchmark:
///
/// ```text
/// TE = σ(r_portfolio − r_benchmark) × sqrt(ann_factor)   [if annualized]
/// ```ignore
///
/// A lower tracking error indicates tighter benchmark replication.
///
/// # Arguments
///
/// * `returns` - Portfolio return series.
/// * `benchmark` - Benchmark return series. Lengths are matched to the
///   shorter of the two.
/// * `annualize` - Whether to scale by `sqrt(ann_factor)`.
/// * `ann_factor` - Number of periods per year.
///
/// # Returns
///
/// Tracking error (non-negative). Returns `0.0` for empty or mismatched series.
/// When `annualize` is `true`, returns [`f64::NAN`] if `ann_factor` is not finite
/// or is `<= 0`.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::tracking_error;
///
/// // Identical series → zero tracking error.
/// let r = [0.01, 0.02, -0.01, 0.03];
/// assert!(tracking_error(&r, &r, false, 252.0).abs() < 1e-12);
/// ```
///
/// # References
///
/// - Grinold & Kahn (1999): see docs/REFERENCES.md#grinoldKahn1999ActivePortfolio
#[must_use]
pub(crate) fn tracking_error(
    returns: &[f64],
    benchmark: &[f64],
    annualize: bool,
    ann_factor: f64,
) -> f64 {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }
    if crate::risk_metrics::invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    let mut os = OnlineStats::new();
    for i in 0..n {
        os.update(returns[i] - benchmark[i]);
    }
    let te = os.std_dev();
    if annualize {
        te * ann_factor.sqrt()
    } else {
        te
    }
}

/// Information ratio: annualized active return divided by tracking error.
///
/// Quantifies the consistency of alpha generation per unit of active risk:
///
/// ```text
/// IR = (mean active return × ann_factor) / (σ active return × sqrt(ann_factor))
///    = mean active return × sqrt(ann_factor) / σ active return
/// ```ignore
///
/// A higher IR indicates more reliable outperformance relative to the
/// benchmark. The IR is related to the Sharpe ratio but uses active
/// (excess) returns rather than returns in excess of the risk-free rate.
///
/// # Arguments
///
/// * `returns`    - Portfolio return series.
/// * `benchmark`  - Benchmark return series.
/// * `annualize`  - Whether to annualize numerator and denominator.
/// * `ann_factor` - Number of periods per year.
///
/// # Returns
///
/// The Information Ratio. Returns `0.0` when the series are empty, or when
/// tracking error is zero and mean active return is also zero. When the
/// tracking error is zero but mean active return is nonzero, returns
/// `+∞` or `-∞` matching the sign of the excess (consistent with
/// [`crate::risk_metrics::sharpe`]). When `annualize` is
/// `true`, returns [`f64::NAN`] if `ann_factor` is not finite or is `<= 0`.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::information_ratio;
///
/// let r = [0.02, 0.03, 0.01, 0.04];
/// let b = [0.01, 0.01, 0.01, 0.01];
/// let ir = information_ratio(&r, &b, false, 252.0);
/// assert!(ir > 0.0);
/// ```
///
/// # References
///
/// - Grinold & Kahn (1999): see docs/REFERENCES.md#grinoldKahn1999ActivePortfolio
#[must_use]
pub(crate) fn information_ratio(
    returns: &[f64],
    benchmark: &[f64],
    annualize: bool,
    ann_factor: f64,
) -> f64 {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }
    if crate::risk_metrics::invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    let mut os = OnlineStats::new();
    for i in 0..n {
        os.update(returns[i] - benchmark[i]);
    }
    let er = os.mean();
    let te = os.std_dev();
    if te == 0.0 {
        return if er > 0.0 {
            f64::INFINITY
        } else if er < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
    }
    if annualize {
        (er * ann_factor) / (te * ann_factor.sqrt())
    } else {
        er / te
    }
}

/// R-squared: proportion of portfolio variance explained by the benchmark.
///
/// Computed as the square of the Pearson correlation coefficient:
///
/// ```text
/// R² = corr(r_portfolio, r_benchmark)²
/// ```ignore
///
/// A value of 1.0 means the portfolio moves perfectly in line with the
/// benchmark; 0.0 means the two are uncorrelated.
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
///
/// # Returns
///
/// R-squared in `[0, 1]`. Returns `0.0` for empty or zero-variance series.
/// Mismatched lengths are truncated to the shorter series, matching the
/// convention of [`tracking_error`], [`beta`], and [`greeks`].
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::r_squared;
///
/// // Perfect linear relationship → R² = 1.
/// let r = [1.0, 2.0, 3.0, 4.0];
/// let b = [2.0, 4.0, 6.0, 8.0];
/// assert!((r_squared(&r, &b) - 1.0).abs() < 1e-10);
/// ```
#[must_use]
pub(crate) fn r_squared(returns: &[f64], benchmark: &[f64]) -> f64 {
    let n = returns.len().min(benchmark.len());
    let c = correlation(&returns[..n], &benchmark[..n]);
    c * c
}

/// OLS beta result with optional standard error and confidence interval.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BetaResult {
    /// Estimated beta coefficient.
    pub beta: f64,
    /// Standard error of the beta estimate.
    pub std_err: f64,
    /// Lower bound of the 95% confidence interval.
    pub ci_lower: f64,
    /// Upper bound of the 95% confidence interval.
    pub ci_upper: f64,
}

// Two-sided 95% critical value: Student's t with `n−2` degrees of freedom.
// Use exact tabulated values for small samples, then conservative step-down
// anchors at df = 40, 60, and 120 before the asymptotic normal limit.
fn beta_ci_critical_value(sample_size: usize) -> f64 {
    match sample_size.saturating_sub(2) {
        0 => f64::NAN,
        1 => 12.706_204_736_432_095,
        2 => 4.302_652_729_696_142,
        3 => 3.182_446_305_284_263,
        4 => 2.776_445_105_197_798_7,
        5 => 2.570_581_835_636_305,
        6 => 2.446_911_851_144_969_2,
        7 => 2.364_624_251_592_784_4,
        8 => 2.306_004_135_204_166,
        9 => 2.262_157_162_854_099_3,
        10 => 2.228_138_851_964_938_5,
        11 => 2.200_985_160_082_949,
        12 => 2.178_812_829_663_417_7,
        13 => 2.160_368_656_461_013,
        14 => 2.144_786_687_916_927_7,
        15 => 2.131_449_545_559_323,
        16 => 2.119_905_299_221_011_2,
        17 => 2.109_815_577_833_180_6,
        18 => 2.100_922_040_240_96,
        19 => 2.093_024_054_408_263,
        20 => 2.085_963_447_265_837,
        21 => 2.079_613_844_727_662,
        22 => 2.073_873_067_904_015,
        23 => 2.068_657_610_419_041,
        24 => 2.063_898_561_628_021,
        25 => 2.059_538_552_753_294,
        26 => 2.055_529_438_642_872,
        27 => 2.051_830_516_480_283_3,
        28 => 2.048_407_141_795_244,
        29 => 2.045_229_642_132_703,
        30 => 2.042_272_456_301_238,
        31 => 2.039_513_446_396_408_5,
        32 => 2.036_933_343_460_101_6,
        33 => 2.034_515_297_449_338_3,
        34 => 2.032_244_509_317_719,
        35 => 2.030_107_928_250_343,
        36 => 2.028_094_000_980_451,
        37 => 2.026_192_463_029_109_3,
        // Anchor at df = 38 (not 40) so the whole bucket is conservative:
        // t is decreasing in df, so the bucket's smallest df needs the
        // largest critical value.
        38..=59 => 2.024_394_164_575_136,
        60..=119 => 2.000_297_821_058_262,
        120..=239 => 1.979_930_405_052_777,
        _ => 1.959_963_984_540_054,
    }
}

/// OLS beta of portfolio vs benchmark, with standard error and 95% CI.
///
/// Estimates the slope of the single-factor linear regression
/// `r_portfolio = α + β × r_benchmark + ε` via:
///
/// ```text
/// β = Cov(r_portfolio, r_benchmark) / Var(r_benchmark)
/// ```
///
/// Standard error uses the OLS formula with `(n - 2)` degrees of freedom.
///
/// The **95% two-sided** interval uses `β ± t_{n−2, 0.975} × SE(β)`, where
/// `t_{n−2, 0.975}` is the **Student's t** critical value for `n − 2` degrees
/// of freedom. Exact tabulated values are used for `n − 2 ≤ 37`; for larger
/// samples the implementation steps down through conservative anchors before
/// reaching the asymptotic normal limit:
///
/// | `n − 2`     | Critical value                                |
/// |-------------|-----------------------------------------------|
/// | `1..=37`    | exact Student's t at that df                  |
/// | `38..=59`   | `2.024` (t at df = 38, used as a step-down)   |
/// | `60..=119`  | `2.000` (t at df = 60)                        |
/// | `120..=239` | `1.980` (t at df = 120)                       |
/// | `≥ 240`     | `1.96`  (asymptotic normal)                   |
///
/// Each bucket is anchored at its *smallest* df (the largest t value in the
/// bucket), so intervals are conservative — never narrower than the exact t
/// critical — for `38 ≤ n − 2 < 240`. The `≥ 240` regime uses the asymptotic
/// normal value, which is up to ~0.5% narrower than the exact t at df = 240.
///
/// Requires at least 3 observations; returns `NaN` for standard error and
/// CI bounds when `n < 3`.
///
/// # Arguments
///
/// * `portfolio`  - Portfolio return series.
/// * `benchmark`  - Benchmark return series.
///
/// # Returns
///
/// A [`BetaResult`] with `beta`, `std_err`, `ci_lower`, and `ci_upper`.
/// All fields are `NAN` when the series are too short (`n < 3`) or the
/// benchmark has zero variance (the slope is unidentifiable).
///
/// # Examples
///
/// ```rust
/// use finstack_analytics::beta;
///
/// // Portfolio returns are approximately 2× the benchmark with noise.
/// let port  = [0.020, 0.042, 0.058, 0.081, 0.099];
/// let bench = [0.010, 0.020, 0.030, 0.040, 0.050];
/// let result = beta(&port, &bench);
/// assert!((result.beta - 2.0).abs() < 0.1);
/// assert!(result.ci_lower <= result.ci_upper);
/// assert!(result.std_err.is_finite());
/// ```
pub fn beta(portfolio: &[f64], benchmark: &[f64]) -> BetaResult {
    let n = portfolio.len().min(benchmark.len());
    if n < 3 {
        return BetaResult {
            beta: f64::NAN,
            std_err: f64::NAN,
            ci_lower: f64::NAN,
            ci_upper: f64::NAN,
        };
    }
    let mut oc = OnlineCovariance::new();
    for i in 0..n {
        oc.update(portfolio[i], benchmark[i]);
    }
    // A zero-variance benchmark cannot identify a slope: surface NaN
    // (matching `beta_only` / `greeks` / `rolling_greeks`) rather than the
    // plausible-looking 0.0 the raw estimator would report.
    let beta = if oc.variance_y() == 0.0 {
        f64::NAN
    } else {
        oc.optimal_beta()
    };

    // oc.mean_x() = mean(portfolio), oc.mean_y() = mean(benchmark)
    let mean_port = oc.mean_x();
    let mean_bench = oc.mean_y();
    let alpha = mean_port - beta * mean_bench;

    let mut residual_stats = OnlineStats::new();
    for i in 0..n {
        let residual = portfolio[i] - alpha - beta * benchmark[i];
        residual_stats.update(residual);
    }

    let var_bench = oc.variance_y();
    let resid_var = residual_stats.variance();
    let se = if var_bench > 0.0 {
        (resid_var / ((n - 2) as f64 * var_bench)).sqrt()
    } else {
        f64::NAN
    };

    let critical_value = beta_ci_critical_value(n);
    BetaResult {
        beta,
        std_err: se,
        ci_lower: beta - critical_value * se,
        ci_upper: beta + critical_value * se,
    }
}

/// OLS beta only — slope of `r_portfolio` on `r_benchmark`.
///
/// Equivalent to `greeks(returns, benchmark, _).beta` but skips the alpha,
/// correlation, and adjusted-R² arithmetic. Used by callers (e.g. Treynor)
/// that need only the slope and otherwise pay for unused outputs.
///
/// Returns `0.0` for empty inputs (matching the [`greeks`] sentinel) and
/// [`f64::NAN`] for a zero-variance benchmark, where the regression cannot
/// identify a slope — matching the [`rolling_greeks`] degenerate-window
/// sentinel. A `NaN` beta propagates through downstream ratios (e.g.
/// Treynor) instead of producing a plausible-looking `0.0` or `±∞`.
#[must_use]
pub(crate) fn beta_only(returns: &[f64], benchmark: &[f64]) -> f64 {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }
    let mut oc = OnlineCovariance::new();
    for i in 0..n {
        oc.update(returns[i], benchmark[i]);
    }
    if oc.variance_y() == 0.0 {
        return f64::NAN;
    }
    oc.optimal_beta()
}

/// Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GreeksResult {
    /// Annualized alpha (intercept).
    pub alpha: f64,
    /// Beta (slope) of portfolio vs benchmark.
    pub beta: f64,
    /// R-squared of the regression.
    pub r_squared: f64,
    /// Adjusted R-squared of the regression.
    pub adjusted_r_squared: f64,
}

/// Single-factor greeks for portfolio vs benchmark.
///
/// Runs a simple OLS regression `r_portfolio = α + β × r_benchmark` and
/// returns the annualized alpha, beta, R², and adjusted R² from that fit.
///
/// Unlike [`beta`], this function does not compute standard errors
/// and is lighter-weight for scenarios where the point estimates are
/// sufficient (e.g., rolling window computations).
///
/// # Arguments
///
/// * `returns`    - Portfolio return series.
/// * `benchmark`  - Benchmark return series.
/// * `ann_factor` - Number of periods per year. Used to annualize alpha.
///
/// # Returns
///
/// A [`GreeksResult`] with `alpha` (annualized), `beta`, `r_squared`, and
/// `adjusted_r_squared`. Returns zeros for empty inputs. For a zero-variance
/// benchmark the regression cannot identify a slope, so `alpha` and `beta`
/// are [`f64::NAN`] (matching the [`rolling_greeks`] degenerate-window
/// sentinel) while `r_squared` / `adjusted_r_squared` remain `0.0`.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::greeks;
///
/// let r = [0.01, 0.02, 0.03, 0.04, 0.05];
/// let b = [0.005, 0.01, 0.015, 0.02, 0.025];
/// let g = greeks(&r, &b, 252.0);
/// assert!((g.beta - 2.0).abs() < 1e-10);
/// assert!((g.r_squared - 1.0).abs() < 1e-10);
/// ```
pub(crate) fn greeks(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> GreeksResult {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return GreeksResult {
            alpha: 0.0,
            beta: 0.0,
            r_squared: 0.0,
            adjusted_r_squared: 0.0,
        };
    }
    let mut oc = OnlineCovariance::new();
    for i in 0..n {
        oc.update(returns[i], benchmark[i]);
    }
    let beta = if oc.variance_y() == 0.0 {
        f64::NAN
    } else {
        oc.optimal_beta()
    };
    let alpha = (oc.mean_x() - beta * oc.mean_y()) * ann_factor;
    let c = oc.correlation();
    let r_squared = c * c;
    let adjusted_r_squared = if n > 2 {
        1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / (n as f64 - 2.0)
    } else {
        0.0
    };
    GreeksResult {
        alpha,
        beta,
        r_squared,
        adjusted_r_squared,
    }
}

/// Rolling greeks output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollingGreeks {
    /// End dates for each rolling window.
    pub dates: Vec<Date>,
    /// Rolling alpha values.
    pub alphas: Vec<f64>,
    /// Rolling beta values.
    pub betas: Vec<f64>,
}

/// Rolling single-factor greeks (alpha, beta) over a sliding window.
///
/// Computes [`greeks`] independently for each `window`-length sub-slice,
/// advancing one period at a time. Produces `n - window + 1` values where
/// `n = min(returns.len(), benchmark.len(), dates.len())`.
///
/// # Arguments
///
/// * `returns` - Portfolio return series.
/// * `benchmark` - Benchmark return series.
/// * `dates` - Date vector aligned with `returns`. Used to label window
///   end dates in the output.
/// * `window` - Look-back window length in periods.
/// * `ann_factor` - Number of periods per year for alpha annualization.
///
/// # Returns
///
/// A [`RollingGreeks`] with `dates`, `alphas`, and `betas` of equal length.
/// Returns empty vectors if `window` is zero or exceeds the series length.
/// Windows containing a non-finite input emit sentinel `NaN` values; windows
/// consisting only of finite data are unaffected (the kernel recomputes its
/// running sums as soon as a non-finite value exits the window). Use
/// [`multi_factor_greeks`] when strict regression input validation is
/// required.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::rolling_greeks;
/// use finstack_core::dates::{Date, Month};
///
/// let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
/// let b: Vec<f64> = (0..20).map(|i| i as f64 * 0.0005).collect();
/// let dates: Vec<Date> = (1..=20)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
///     .collect();
/// let rg = rolling_greeks(&r, &b, &dates, 5, 252.0);
/// assert_eq!(rg.betas.len(), 16); // 20 − 5 + 1
/// ```
pub(crate) fn rolling_greeks(
    returns: &[f64],
    benchmark: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
) -> RollingGreeks {
    let n = returns.len().min(benchmark.len()).min(dates.len());
    if n < window || window == 0 {
        tracing::debug!(
            n,
            window,
            reason = "insufficient_window",
            "rolling greeks returning empty result"
        );
        return RollingGreeks {
            dates: vec![],
            alphas: vec![],
            betas: vec![],
        };
    }
    let count = n - window + 1;
    if count > ROLLING_GREEKS_RECOMPUTE_INTERVAL {
        tracing::debug!(
            n,
            window,
            count,
            recompute_interval = ROLLING_GREEKS_RECOMPUTE_INTERVAL,
            "rolling greeks using incremental O(n) path"
        );
    }
    let mut out_dates = Vec::with_capacity(count);
    let mut alphas = Vec::with_capacity(count);
    let mut betas = Vec::with_capacity(count);

    // Incremental O(n) sliding-window OLS via running sums.
    let w = window as f64;
    let (mut sr, mut sb, mut srb, mut sb2) =
        recompute_rolling_greeks_sums(&returns[..window], &benchmark[..window]);
    let (mut csr, mut csb, mut csrb, mut csb2) = (0.0, 0.0, 0.0, 0.0);
    let mut steps_since_recompute = 0usize;

    for i in window..=n {
        let denom = w * sb2 - sb * sb;
        // Degenerate window (e.g. constant benchmark or NaN inputs) cannot
        // identify a beta; emit a sentinel `NaN` for both greeks so callers
        // see "do not use this point" instead of a plausible-looking 0.
        let (alpha, beta) = if denom.abs() < 1e-30 {
            (f64::NAN, f64::NAN)
        } else {
            let beta = (w * srb - sb * sr) / denom;
            let alpha = (sr / w - beta * sb / w) * ann_factor;
            (alpha, beta)
        };
        out_dates.push(dates[i - 1]);
        alphas.push(alpha);
        betas.push(beta);

        if i < n {
            let old_r = returns[i - window];
            let old_b = benchmark[i - window];
            let new_r = returns[i];
            let new_b = benchmark[i];
            compensated_add(&mut sr, &mut csr, new_r - old_r);
            compensated_add(&mut sb, &mut csb, new_b - old_b);
            compensated_add(&mut srb, &mut csrb, new_r * new_b - old_r * old_b);
            compensated_add(&mut sb2, &mut csb2, new_b * new_b - old_b * old_b);
            steps_since_recompute += 1;
            // A non-finite value entering or leaving the window poisons the
            // running sums permanently (`x − NaN = NaN`), so recompute
            // immediately whenever any sum is non-finite. Windows that still
            // contain the non-finite value recompute to NaN (and emit NaN);
            // the first all-finite window after it exits recovers exact sums
            // instead of staying NaN until the next scheduled recompute.
            let sums_non_finite =
                !(sr.is_finite() && sb.is_finite() && srb.is_finite() && sb2.is_finite());
            if sums_non_finite || steps_since_recompute >= ROLLING_GREEKS_RECOMPUTE_INTERVAL {
                let start = i + 1 - window;
                (sr, sb, srb, sb2) =
                    recompute_rolling_greeks_sums(&returns[start..=i], &benchmark[start..=i]);
                (csr, csb, csrb, csb2) = (0.0, 0.0, 0.0, 0.0);
                steps_since_recompute = 0;
            }
        }
    }

    RollingGreeks {
        dates: out_dates,
        alphas,
        betas,
    }
}

/// Up-market capture ratio: portfolio performance during benchmark up-periods.
///
/// Computes the ratio of the portfolio's geometric mean return to the benchmark's
/// geometric mean return over periods where the benchmark return is non-negative.
/// A value > 1.0 means the portfolio amplifies benchmark gains on a per-period
/// geometric basis within the benchmark-up subset.
///
/// **Convention:** Zero-return benchmark days (`r_bench = 0.0`) are classified
/// as "up" periods (using `>=`). Some vendors (e.g., Morningstar) use strict
/// `> 0.0` which would exclude flat days. This choice can produce small
/// differences in capture ratios on daily series with frequent zero-return
/// observations.
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
///
/// # Returns
///
/// Up capture ratio. Returns `0.0` if there are no up-benchmark periods
/// or the benchmark's compounded up-period return is negligible.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::up_capture;
///
/// // Portfolio doubles the benchmark in up periods.
/// let r = [0.04, -0.01, 0.06];
/// let b = [0.02, -0.03, 0.03];
/// let uc = up_capture(&r, &b);
/// assert!(uc > 1.0);
/// ```
#[must_use]
pub(crate) fn up_capture(returns: &[f64], benchmark: &[f64]) -> f64 {
    geometric_capture(returns, benchmark, |bench_return| bench_return >= 0.0)
}

/// Down-market capture ratio: portfolio performance during benchmark down-periods.
///
/// Computes the ratio of the portfolio's geometric mean return to the benchmark's
/// geometric mean return over periods where the benchmark return is negative.
/// A value < 1.0 means the portfolio loses less than the benchmark during
/// downturns on a per-period geometric basis (desirable).
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
///
/// # Returns
///
/// Down capture ratio. Returns `0.0` if there are no down-benchmark periods
/// or the benchmark's compounded down-period return is negligible.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::down_capture;
///
/// // Portfolio loses less than benchmark in down periods (defensive).
/// let r = [0.04, -0.01, 0.06];
/// let b = [0.02, -0.03, 0.03];
/// let dc = down_capture(&r, &b);
/// assert!(dc < 1.0);
/// ```
#[must_use]
pub(crate) fn down_capture(returns: &[f64], benchmark: &[f64]) -> f64 {
    geometric_capture(returns, benchmark, |bench_return| bench_return < 0.0)
}

/// Capture ratio = up capture / down capture.
///
/// A value > 1.0 indicates the portfolio captures more upside than downside
/// relative to the benchmark -- the hallmark of a skillful active manager.
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
///
/// # Returns
///
/// The capture ratio. Returns `0.0` if either capture component is zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::capture_ratio;
///
/// let r = [0.04, -0.01, 0.06];
/// let b = [0.02, -0.03, 0.03];
/// let cr = capture_ratio(&r, &b);
/// assert!(cr > 1.0);
/// ```
#[must_use]
pub(crate) fn capture_ratio(returns: &[f64], benchmark: &[f64]) -> f64 {
    let dc = down_capture(returns, benchmark);
    if dc.is_nan() {
        return f64::NAN;
    }
    if dc == 0.0 {
        return 0.0;
    }
    let uc = up_capture(returns, benchmark);
    if uc.is_nan() {
        return f64::NAN;
    }
    uc / dc
}

/// Batting average: fraction of periods where portfolio outperforms benchmark.
///
/// ```text
/// BA = count(r_portfolio > r_benchmark) / n
/// ```ignore
///
/// A value above 0.5 indicates the portfolio beats the benchmark more often
/// than not, though it says nothing about the magnitude of wins vs losses.
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
///
/// # Returns
///
/// Fraction in `[0, 1]`. Returns `0.0` for empty series.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::batting_average;
///
/// let r = [0.02, 0.01, 0.03, -0.01];
/// let b = [0.01, 0.02, 0.01, 0.00];
/// let ba = batting_average(&r, &b);
/// // Beats benchmark in periods 0, 2 → 2/4 = 0.5
/// // Period 3: -0.01 < 0.00 → loss
/// assert!((ba - 0.5).abs() < 1e-12);
/// ```
#[must_use]
pub(crate) fn batting_average(returns: &[f64], benchmark: &[f64]) -> f64 {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }
    let wins = (0..n).filter(|&i| returns[i] > benchmark[i]).count();
    wins as f64 / n as f64
}

/// Result of a multi-factor regression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiFactorResult {
    /// Annualized intercept (alpha).
    pub alpha: f64,
    /// Regression coefficients, one per factor.
    pub betas: Vec<f64>,
    /// R-squared: fraction of variance explained by the factors.
    pub r_squared: f64,
    /// Adjusted R-squared: penalizes additional regressors.
    ///
    /// ```text
    /// adj_R² = 1 − (1 − R²) × (n − 1) / (n − k − 1)
    /// ```
    pub adjusted_r_squared: f64,
    /// Annualized residual volatility.
    pub residual_vol: f64,
}

/// OLS via SVD of the design matrix `[1 | f₁ | … | f_k]` (leading intercept
/// column), solving for `[α, β₁, …, β_k]`.
///
/// The design matrix is built directly from the borrowed factor slices: the
/// intercept column is synthesized inline as `1.0` rather than materialized,
/// avoiding the `k + 1` intermediate owned column vectors.
fn svd_least_squares(factors: &[&[f64]], y: &[f64]) -> crate::Result<Vec<f64>> {
    use nalgebra::{DMatrix, DVector};

    let k = factors.len();
    let p = k + 1; // intercept + k factors
    let n = y.len();
    if n == 0 || factors.iter().any(|column| column.len() != n) {
        return Err(crate::error::InputError::Invalid.into());
    }

    // Normalize columns before the SVD so factor scale alone does not trigger
    // false "singular" classifications in otherwise full-rank regressions.
    // The intercept column of ones has Euclidean norm sqrt(n).
    let mut scales = Vec::with_capacity(p);
    scales.push((n as f64).sqrt());
    for column in factors {
        let norm = neumaier_sum(column.iter().map(|value| value * value)).sqrt();
        if !norm.is_finite() || norm <= 0.0 {
            return Err(crate::error::InputError::Invalid.into());
        }
        scales.push(norm);
    }

    let mut design = Vec::with_capacity(n * p);
    for row in 0..n {
        design.push(1.0 / scales[0]);
        for (col_idx, column) in factors.iter().enumerate() {
            design.push(column[row] / scales[col_idx + 1]);
        }
    }

    let x_matrix = DMatrix::from_row_slice(n, p, &design);
    let y_vector = DVector::from_column_slice(y);
    let svd = x_matrix.svd(true, true);
    let max_singular = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let tolerance = 1.0e-10 * max_singular.max(1.0);
    // A column-norm guard above already rejected zero / non-finite columns,
    // so any remaining failure is rank-deficiency relative to `p`.
    let rank = svd
        .singular_values
        .iter()
        .filter(|&&value| value > tolerance)
        .count();
    if rank < p {
        return Err(crate::error::InputError::Invalid.into());
    }

    let beta_scaled = svd
        .solve(&y_vector, tolerance)
        .map_err(|_| crate::error::InputError::Invalid)?;

    Ok(beta_scaled
        .iter()
        .zip(scales.iter())
        .map(|(beta, scale)| beta / scale)
        .collect())
}

fn geometric_capture<F>(returns: &[f64], benchmark: &[f64], include: F) -> f64
where
    F: Fn(f64) -> bool,
{
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }

    let mut port_logs = Vec::with_capacity(n);
    let mut bench_logs = Vec::with_capacity(n);
    for i in 0..n {
        if include(benchmark[i]) {
            let port_growth = 1.0 + returns[i];
            let bench_growth = 1.0 + benchmark[i];
            if !port_growth.is_finite()
                || !bench_growth.is_finite()
                || port_growth <= 0.0
                || bench_growth <= 0.0
            {
                return f64::NAN;
            }
            port_logs.push(port_growth.ln());
            bench_logs.push(bench_growth.ln());
        }
    }

    if port_logs.is_empty() {
        return 0.0;
    }

    let count = port_logs.len() as f64;
    let port_geom = (neumaier_sum(port_logs) / count).exp() - 1.0;
    let bench_geom = (neumaier_sum(bench_logs) / count).exp() - 1.0;
    if bench_geom.abs() < 1e-18 {
        return 0.0;
    }
    port_geom / bench_geom
}

/// Multi-factor OLS regression of portfolio returns on factor returns.
///
/// Estimates the linear model
///
/// ```text
/// r_portfolio = α + β₁f₁ + β₂f₂ + ... + βₖfₖ + ε
/// ```ignore
///
/// by solving the least-squares system with an SVD of the (column-normalized)
/// design matrix. SVD avoids explicitly forming the normal equations and is
/// numerically robust to correlated factors and rank deficiency.
///
/// # Arguments
///
/// * `returns`    - Portfolio simple-return series in decimal form (for example,
///   `0.01` for 1%).
/// * `factors`    - Slice of factor return series (each inner slice is one
///   factor's return series, all the same length as `returns`). Factor returns
///   use the same decimal convention as `returns`.
/// * `ann_factor` - Number of observation periods per year used to annualize
///   the intercept and residual volatility. For example, use `252.0` for daily
///   data or `12.0` for monthly data.
///
/// # Returns
///
/// A [`MultiFactorResult`] containing:
///
/// - `alpha`: annualized intercept
/// - `betas`: one loading per factor
/// - `r_squared` and `adjusted_r_squared`: goodness-of-fit measures
/// - `residual_vol`: annualized residual volatility
///
/// Coefficients are estimated on the overlapping sample implied by the input
/// slices. The function does not truncate mismatched factor lengths; it rejects
/// them as invalid input instead.
///
/// # Errors
///
/// Returns an error when:
///
/// - `ann_factor` is not finite or is `<= 0`
/// - no factors are supplied
/// - there are too few observations for the requested number of factors
/// - any portfolio or factor return is non-finite
/// - any factor length differs from `returns.len()`
/// - the factor matrix is singular or numerically rank deficient
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::multi_factor_greeks;
///
/// // y ≈ 2*f1 (single effective factor).
/// let y = [0.02, 0.04, 0.06, 0.08, 0.10];
/// let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
/// let result = multi_factor_greeks(&y, &[&f1], 252.0).unwrap();
/// assert!(result.r_squared > 0.99);
/// ```
///
/// # References
///
/// - Fama & French (1993): see docs/REFERENCES.md#fama-french-1993
/// - Higham: see docs/REFERENCES.md#higham-accuracy-and-stability
#[tracing::instrument(level = "debug", skip(returns, factors), fields(n = returns.len(), k = factors.len(), ann_factor = ann_factor))]
pub(crate) fn multi_factor_greeks(
    returns: &[f64],
    factors: &[&[f64]],
    ann_factor: f64,
) -> crate::Result<MultiFactorResult> {
    if !ann_factor.is_finite() || ann_factor <= 0.0 {
        tracing::debug!(
            ann_factor,
            reason = "invalid_annualization_factor",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }

    let n = returns.len();
    let k = factors.len();
    let p = k + 1; // intercept + k factors

    if n < p + 1 || k == 0 {
        tracing::debug!(
            n,
            k,
            min_observations = p + 1,
            reason = "insufficient_observations",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }
    if returns.iter().any(|r| !r.is_finite()) {
        tracing::debug!(
            n,
            reason = "non_finite_returns",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }
    if factors
        .iter()
        .any(|factor| factor.iter().any(|v| !v.is_finite()))
    {
        tracing::debug!(
            n,
            k,
            reason = "non_finite_factors",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }
    if factors.iter().any(|factor| factor.len() != n) {
        tracing::debug!(
            n,
            k,
            reason = "factor_length_mismatch",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::DimensionMismatch.into());
    }
    let beta = svd_least_squares(factors, returns)?;

    let alpha_per_period = beta[0];
    let factor_betas: Vec<f64> = beta[1..].to_vec();

    // Compute residuals and R²
    let y_mean = mean(returns);
    let mut ss_res = 0.0_f64;
    let mut ss_tot = 0.0_f64;
    for (t, &r) in returns.iter().enumerate().take(n) {
        let mut y_hat = alpha_per_period;
        for j in 0..k {
            let fj = factors[j][t];
            y_hat += factor_betas[j] * fj;
        }
        let residual = r - y_hat;
        ss_res += residual * residual;
        ss_tot += (r - y_mean) * (r - y_mean);
    }

    let r_sq = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };
    let dof = n as f64 - k as f64 - 1.0;
    let residual_var = if dof > 0.0 { ss_res / dof } else { 0.0 };
    let residual_vol = residual_var.sqrt() * ann_factor.sqrt();
    let alpha = alpha_per_period * ann_factor;

    let adjusted_r_squared = if dof > 0.0 {
        1.0 - (1.0 - r_sq) * (n as f64 - 1.0) / dof
    } else {
        0.0
    };

    Ok(MultiFactorResult {
        alpha,
        betas: factor_betas,
        r_squared: r_sq,
        adjusted_r_squared,
        residual_vol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dates::{Duration, Month};

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2025, Month::January, day).expect("valid date")
    }

    #[test]
    fn tracking_error_zero_when_identical() {
        let r = [0.01, 0.02, -0.01, 0.03];
        let te = tracking_error(&r, &r, false, 252.0);
        assert!(te.abs() < 1e-12);
    }

    #[test]
    fn tracking_error_nan_when_annualized_with_invalid_ann_factor() {
        let r = [0.01, 0.02];
        let b = [0.01, 0.01];
        assert!(tracking_error(&r, &b, true, 0.0).is_nan());
        assert!(tracking_error(&r, &b, true, -1.0).is_nan());
        assert!(tracking_error(&r, &b, true, f64::NAN).is_nan());
    }

    #[test]
    fn information_ratio_basic() {
        let r = [0.02, 0.03, 0.01, 0.04];
        let b = [0.01, 0.01, 0.01, 0.01];
        let ir = information_ratio(&r, &b, false, 252.0);
        assert!(ir > 0.0);
    }

    #[test]
    fn information_ratio_nan_when_annualized_with_invalid_ann_factor() {
        let r = [0.02, 0.03, 0.01, 0.04];
        let b = [0.01, 0.01, 0.01, 0.01];
        assert!(information_ratio(&r, &b, true, 0.0).is_nan());
        assert!(information_ratio(&r, &b, true, f64::INFINITY).is_nan());
    }

    #[test]
    fn r_squared_perfect_correlation() {
        let r = [1.0, 2.0, 3.0, 4.0];
        let b = [2.0, 4.0, 6.0, 8.0];
        let r2 = r_squared(&r, &b);
        assert!((r2 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn beta_basic() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let x = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = beta(&y, &x);
        assert!((result.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn beta_uses_t_critical_value_for_small_samples() {
        let y = [0.020, 0.041, 0.059, 0.082, 0.099];
        let x = [0.010, 0.020, 0.030, 0.040, 0.050];
        let result = beta(&y, &x);
        let expected_t_critical_df3 = 3.182_446_305_284_263_f64;
        let expected_half_width = expected_t_critical_df3 * result.std_err;
        let actual_half_width = result.ci_upper - result.beta;

        assert!(
            (actual_half_width - expected_half_width).abs() < 1e-12,
            "small-sample beta CI should use Student-t critical value: expected {}, got {}",
            expected_half_width,
            actual_half_width
        );
    }

    #[test]
    fn beta_uses_t_critical_value_beyond_df_37() {
        let x: Vec<f64> = (0..42).map(|i| -0.02 + i as f64 * 0.001).collect();
        let y: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, &b)| 1.4 * b + if i % 2 == 0 { 0.002 } else { -0.0015 })
            .collect();
        let result = beta(&y, &x);
        // n = 42 → df = 40; bucket is anchored at its smallest df (38) so the
        // interval is conservative for every df in 38..=59.
        let expected_t_critical_df38 = 2.024_394_164_575_136_f64;
        let actual_half_width = result.ci_upper - result.beta;

        assert!(
            (actual_half_width - expected_t_critical_df38 * result.std_err).abs() < 1e-12,
            "beta CI should continue using Student-t beyond df=37"
        );
    }

    #[test]
    fn greeks_basic() {
        let r = [0.01, 0.02, 0.03, 0.04, 0.05];
        let b = [0.005, 0.01, 0.015, 0.02, 0.025];
        let g = greeks(&r, &b, 252.0);
        assert!((g.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn greeks_reports_adjusted_r_squared() {
        let r = [0.011, 0.018, 0.031, 0.039, 0.052, 0.061];
        let b = [0.005, 0.010, 0.015, 0.020, 0.025, 0.030];
        let g = greeks(&r, &b, 252.0);
        let n = r.len() as f64;
        let expected = 1.0 - (1.0 - g.r_squared) * (n - 1.0) / (n - 2.0);

        assert!((g.adjusted_r_squared - expected).abs() < 1e-12);
        assert!(g.adjusted_r_squared <= g.r_squared);
    }

    #[test]
    fn rolling_greeks_basic() {
        let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b: Vec<f64> = (0..20).map(|i| i as f64 * 0.0005).collect();
        let dates: Vec<Date> = (1..=20).map(jan).collect();
        let rg = rolling_greeks(&r, &b, &dates, 5, 252.0);
        assert_eq!(rg.betas.len(), 16);
    }

    #[test]
    fn rolling_greeks_emits_nan_when_benchmark_window_is_constant() {
        // Use exactly-representable benchmark values (0.5) so the OLS denominator
        // collapses to a true zero rather than floating-point noise.
        let window = 5;
        let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b: Vec<f64> = vec![0.5_f64; 20];
        let dates: Vec<Date> = (1..=20).map(jan).collect();
        let rg = rolling_greeks(&r, &b, &dates, window, 252.0);
        assert_eq!(rg.alphas.len(), rg.dates.len());
        assert_eq!(rg.betas.len(), rg.dates.len());
        assert!(
            rg.alphas.iter().all(|a| a.is_nan()),
            "constant benchmark window must surface as NaN alpha rather than a plausible 0"
        );
        assert!(
            rg.betas.iter().all(|b| b.is_nan()),
            "constant benchmark window must surface as NaN beta rather than a plausible 0"
        );
    }

    #[test]
    fn rolling_greeks_recovers_immediately_after_nan_exits_window() {
        let window = 5;
        let mut r: Vec<f64> = (0..30).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b: Vec<f64> = (0..30).map(|i| i as f64 * 0.0005 + 0.001).collect();
        r[7] = f64::NAN;
        let dates: Vec<Date> = (1..=30).map(jan).collect();
        let rg = rolling_greeks(&r, &b, &dates, window, 252.0);

        for (k, beta) in rg.betas.iter().enumerate() {
            // Window k covers return indices [k, k + window).
            let contains_nan = k <= 7 && 7 < k + window;
            if contains_nan {
                assert!(beta.is_nan(), "window {k} contains the NaN: beta={beta}");
            } else {
                assert!(
                    beta.is_finite(),
                    "all-finite window {k} must not stay poisoned: beta={beta}"
                );
            }
        }
    }

    #[test]
    fn batch_beta_functions_report_nan_for_zero_variance_benchmark() {
        let r: Vec<f64> = (0..10).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b = vec![0.5_f64; 10];

        assert!(beta_only(&r, &b).is_nan());

        let g = greeks(&r, &b, 252.0);
        assert!(g.beta.is_nan());
        assert!(g.alpha.is_nan());

        let result = beta(&r, &b);
        assert!(result.beta.is_nan());
        assert!(result.std_err.is_nan());

        // Treynor on an unidentifiable beta propagates NaN instead of ±∞.
        assert!(treynor(0.10, 0.02, beta_only(&r, &b)).is_nan());
    }

    #[test]
    fn rolling_greeks_stays_close_to_exact_recomputation_on_long_series() {
        fn exact_rolling_greeks(
            returns: &[f64],
            benchmark: &[f64],
            window: usize,
            ann_factor: f64,
        ) -> (Vec<f64>, Vec<f64>) {
            let n = returns.len().min(benchmark.len());
            let w = window as f64;
            let mut alphas = Vec::with_capacity(n - window + 1);
            let mut betas = Vec::with_capacity(n - window + 1);
            for end in window..=n {
                let rs = &returns[end - window..end];
                let bs = &benchmark[end - window..end];
                let sr: f64 = rs.iter().sum();
                let sb: f64 = bs.iter().sum();
                let srb: f64 = rs.iter().zip(bs.iter()).map(|(&r, &b)| r * b).sum();
                let sb2: f64 = bs.iter().map(|&b| b * b).sum();
                let denom = w * sb2 - sb * sb;
                let (alpha, beta) = if denom.abs() < 1e-30 {
                    (f64::NAN, f64::NAN)
                } else {
                    let beta = (w * srb - sb * sr) / denom;
                    let alpha = (sr / w - beta * sb / w) * ann_factor;
                    (alpha, beta)
                };
                alphas.push(alpha);
                betas.push(beta);
            }
            (alphas, betas)
        }

        let window = 64;
        let ann_factor = 252.0;
        let n = ROLLING_GREEKS_RECOMPUTE_INTERVAL * 4 + window + 33;
        let r: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64;
                1_000_000.0 + x * 0.125 + (x / 9.0).sin() * 0.01
            })
            .collect();
        let b: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64;
                500_000.0 + x * 0.0625 + (x / 7.0).cos() * 0.01
            })
            .collect();
        let dates: Vec<Date> = (0..n).map(|i| jan(1) + Duration::days(i as i64)).collect();

        let rolling = rolling_greeks(&r, &b, &dates, window, ann_factor);
        let (_, expected_betas) = exact_rolling_greeks(&r, &b, window, ann_factor);

        let max_beta_diff = rolling
            .betas
            .iter()
            .zip(expected_betas.iter())
            .map(|(&actual, &expected)| (actual - expected).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_beta_diff < 4.5e-4,
            "max beta diff too large: {max_beta_diff}"
        );
        assert!(rolling.alphas.iter().all(|alpha| alpha.is_finite()));
    }

    #[test]
    fn up_capture_hand_calc() {
        // r = [0.10, −0.05], b = [0.05, −0.10]
        // Up periods (b≥0): index 0
        // port_prod = 1.10, bench_prod = 1.05
        // up_capture = (1.10−1) / (1.05−1) = 0.10/0.05 = 2.0
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let uc = up_capture(&r, &b);
        assert!((uc - 2.0).abs() < 1e-12);
    }

    #[test]
    fn down_capture_hand_calc() {
        // Same data: down periods (b<0): index 1
        // port_prod = 0.95, bench_prod = 0.90
        // down_capture = (0.95−1) / (0.90−1) = −0.05/−0.10 = 0.5
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let dc = down_capture(&r, &b);
        assert!((dc - 0.5).abs() < 1e-12);
    }

    #[test]
    fn capture_ratio_hand_calc() {
        // up/down = 2.0/0.5 = 4.0
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let cr = capture_ratio(&r, &b);
        assert!((cr - 4.0).abs() < 1e-12);
    }

    #[test]
    fn up_capture_multiple_periods() {
        // r = [0.04, −0.01, 0.06], b = [0.02, −0.03, 0.03]
        // Up periods: indices 0, 2 (b[0]=0.02≥0, b[2]=0.03≥0)
        // port geometric return = sqrt((1.04)(1.06)) − 1
        // bench geometric return = sqrt((1.02)(1.03)) − 1
        let r = [0.04, -0.01, 0.06];
        let b = [0.02, -0.03, 0.03];
        let uc = up_capture(&r, &b);
        let expected = ((1.04_f64 * 1.06_f64).sqrt() - 1.0) / ((1.02_f64 * 1.03_f64).sqrt() - 1.0);
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_uses_geometric_subset_returns() {
        let r = [1.0, 0.0, -0.4];
        let b = [0.5, 0.5, -0.1];
        let uc = up_capture(&r, &b);
        let expected_port = (2.0_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (1.5_f64 * 1.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn down_capture_defensive_portfolio() {
        // Portfolio loses less than benchmark → dc < 1.0 (desirable)
        let r = [0.04, -0.01, 0.06];
        let b = [0.02, -0.03, 0.03];
        let dc = down_capture(&r, &b);
        // Down periods: index 1. port_prod=0.99, bench_prod=0.97
        let expected = (0.99 - 1.0) / (0.97 - 1.0);
        assert!((dc - expected).abs() < 1e-12);
        assert!(dc < 1.0);
    }

    #[test]
    fn down_capture_uses_geometric_subset_returns() {
        let r = [-0.25, 0.0, 0.1];
        let b = [-0.5, -0.5, 0.1];
        let dc = down_capture(&r, &b);
        let expected_port = (0.75_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (0.5_f64 * 0.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((dc - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_no_up_periods() {
        let r = [0.01, 0.02];
        let b = [-0.01, -0.02];
        assert_eq!(up_capture(&r, &b), 0.0);
    }

    #[test]
    fn down_capture_no_down_periods() {
        let r = [0.01, 0.02];
        let b = [0.01, 0.02];
        assert_eq!(down_capture(&r, &b), 0.0);
    }

    #[test]
    fn capture_ratio_perfect_tracking() {
        // Portfolio = benchmark → up_capture=1, down_capture=1, ratio=1
        let r = [0.02, -0.03, 0.01, -0.01];
        let cr = capture_ratio(&r, &r);
        assert!((cr - 1.0).abs() < 1e-12);
    }

    #[test]
    fn multi_factor_single_factor() {
        // y = 2*x → alpha ≈ 0, beta ≈ 2, R² ≈ 1.
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0).expect("single-factor regression");
        assert!((result.betas[0] - 2.0).abs() < 1e-8);
        assert!(result.r_squared > 0.999);
    }

    #[test]
    fn multi_factor_two_factors() {
        // y ≈ 1.5*f1 + 0.5*f2 (non-collinear factors).
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let f2 = [0.03, -0.01, 0.02, 0.01, -0.02];
        let y: Vec<f64> = (0..5).map(|i| 1.5 * f1[i] + 0.5 * f2[i]).collect();
        let result = multi_factor_greeks(&y, &[&f1, &f2], 252.0).expect("two-factor regression");
        assert!(result.r_squared > 0.99);
        assert_eq!(result.betas.len(), 2);
        assert!((result.betas[0] - 1.5).abs() < 1e-6);
        assert!((result.betas[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn multi_factor_empty_errors() {
        let result = multi_factor_greeks(&[], &[&[]], 252.0);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_mismatched_factor_lengths_error() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03];
        let result = multi_factor_greeks(&y, &[&f1], 252.0);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_adjusted_r_squared() {
        // y = 2*x → R²≈1, adj_R² should also be close to 1
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0).expect("adjusted r-squared regression");
        assert!(result.adjusted_r_squared > 0.99);
        assert!(result.adjusted_r_squared <= result.r_squared);
    }

    #[test]
    fn batting_average_hand_calc() {
        // r = [0.02, 0.01, 0.03, -0.01], b = [0.01, 0.02, 0.01, 0.00]
        // Wins: r[0]>b[0] (0.02>0.01), r[2]>b[2] (0.03>0.01) → 2/4 = 0.5
        let r = [0.02, 0.01, 0.03, -0.01];
        let b = [0.01, 0.02, 0.01, 0.00];
        assert!((batting_average(&r, &b) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn batting_average_all_wins() {
        let r = [0.05, 0.03, 0.04];
        let b = [0.01, 0.01, 0.01];
        assert!((batting_average(&r, &b) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn batting_average_empty() {
        assert_eq!(batting_average(&[], &[]), 0.0);
    }
}

// ── Benchmark-relative risk ratios ──

/// Treynor ratio: excess return per unit of systematic risk.
///
/// ```text
/// Treynor = (R_p − R_f) / β
/// ```ignore
///
/// Complements the Sharpe ratio by using beta (systematic risk) rather
/// than total volatility as the risk denominator.
///
/// # Arguments
///
/// * `ann_return`     - Annualized portfolio return.
/// * `risk_free_rate` - Annualized risk-free rate.
/// * `beta`           - Portfolio beta vs benchmark.
///
/// # Returns
///
/// The Treynor ratio. Returns `0.0` if beta is zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::treynor;
///
/// // 10% return, 2% risk-free, beta = 1.2 → Treynor ≈ 0.0667.
/// let t = treynor(0.10, 0.02, 1.2);
/// assert!((t - 0.0667).abs() < 0.001);
/// ```
///
/// # References
///
/// - Treynor (1965): see docs/REFERENCES.md#treynor1965
#[must_use]
pub(crate) fn treynor(ann_return: f64, risk_free_rate: f64, beta: f64) -> f64 {
    if beta.abs() < 1e-10 {
        let excess = ann_return - risk_free_rate;
        return if excess > 0.0 {
            f64::INFINITY
        } else if excess < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
    }
    (ann_return - risk_free_rate) / beta
}

/// M-squared (Modigliani-Modigliani): risk-adjusted return on the benchmark's scale.
///
/// Leverages or deleverages the portfolio to match the benchmark's volatility,
/// then reports the resulting return. The difference `M² − R_bench` is a
/// direct measure of value added at the same risk level.
///
/// ```text
/// M² = R_f + (R_p − R_f) × (σ_bench / σ_portfolio)
/// ```ignore
///
/// # Arguments
///
/// * `ann_return`     - Annualized portfolio return.
/// * `ann_vol`        - Annualized portfolio volatility.
/// * `bench_vol`      - Annualized benchmark volatility.
/// * `risk_free_rate` - Annualized risk-free rate.
///
/// # Returns
///
/// The M-squared return. Returns the risk-free rate if portfolio volatility is zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::benchmark::m_squared;
///
/// // Portfolio: 12% return, 20% vol; Benchmark: 15% vol; Rf: 2%
/// // M² = 0.02 + (0.12 − 0.02) × (0.15 / 0.20) = 0.02 + 0.075 = 0.095
/// let m2 = m_squared(0.12, 0.20, 0.15, 0.02);
/// assert!((m2 - 0.095).abs() < 1e-12);
/// ```
///
/// # References
///
/// - Modigliani & Modigliani (1997): see docs/REFERENCES.md#modigliani1997
#[must_use]
pub(crate) fn m_squared(ann_return: f64, ann_vol: f64, bench_vol: f64, risk_free_rate: f64) -> f64 {
    if ann_vol.abs() < 1e-10 {
        return risk_free_rate;
    }
    risk_free_rate + (ann_return - risk_free_rate) * (bench_vol / ann_vol)
}

#[cfg(test)]
mod benchmark_ratio_tests {
    use super::*;

    #[test]
    fn treynor_hand_calc() {
        let t = treynor(0.10, 0.02, 1.2);
        assert!((t - 0.08 / 1.2).abs() < 1e-14);
    }

    #[test]
    fn treynor_zero_beta() {
        assert_eq!(treynor(0.10, 0.02, 0.0), f64::INFINITY);
        assert_eq!(treynor(0.01, 0.02, 0.0), f64::NEG_INFINITY);
        assert_eq!(treynor(0.02, 0.02, 0.0), 0.0);
    }

    #[test]
    fn treynor_negative_beta() {
        let t = treynor(0.10, 0.02, -0.5);
        assert!((t - (0.08 / -0.5)).abs() < 1e-14);
    }

    #[test]
    fn m_squared_hand_calc() {
        let m2 = m_squared(0.12, 0.20, 0.15, 0.02);
        assert!((m2 - 0.095).abs() < 1e-12);
    }

    #[test]
    fn m_squared_zero_vol() {
        assert_eq!(m_squared(0.10, 0.0, 0.15, 0.02), 0.02);
    }
}

#[cfg(test)]
mod multi_factor_error_regression_tests {
    use super::*;
    use crate::dates::{Month, PeriodKind};
    use crate::performance::Performance;

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2024, Month::January, day).expect("valid date")
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_singular_factor_matrix() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10];
        let factor_a = [0.01, 0.02, 0.03, 0.04, 0.05];
        let factor_b = [0.02, 0.04, 0.06, 0.08, 0.10];

        let result = multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0);
        assert!(result.is_err());
    }

    #[test]
    fn performance_multi_factor_greeks_errors_on_invalid_factor_input() {
        let dates = vec![jan(1), jan(2), jan(3), jan(4), jan(5), jan(6)];
        let prices = vec![
            vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
            vec![100.0, 100.5, 101.0, 101.5, 102.0, 102.5],
        ];
        let perf = Performance::new(
            dates,
            prices,
            vec!["BENCH".to_string(), "PORT".to_string()],
            Some("BENCH"),
            PeriodKind::Daily,
        )
        .expect("performance should build");

        let invalid_factor = [0.01, 0.02];
        let result = perf.multi_factor_greeks(1, &[&invalid_factor]);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_near_singular_factor_matrix() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10, 0.12];
        let factor_a = [0.01, 0.02, 0.03, 0.04, 0.05, 0.06];
        let factor_b = [
            0.010_000_000_001,
            0.020_000_000_002,
            0.029_999_999_999,
            0.040_000_000_001,
            0.050_000_000_003,
            0.060_000_000_000,
        ];

        let result = multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_non_positive_ann_factor() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10];
        let factor = [0.01, 0.02, 0.03, 0.04, 0.05];

        assert!(multi_factor_greeks(&returns, &[&factor], 0.0).is_err());
        assert!(multi_factor_greeks(&returns, &[&factor], -252.0).is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_hidden_multicollinearity() {
        let returns = [0.04, 0.01, 0.03, 0.02, 0.05, 0.06];
        let factor_a = [0.01, -0.02, 0.03, -0.01, 0.02, 0.01];
        let factor_b = [0.02, 0.01, -0.01, 0.03, -0.02, 0.04];
        let factor_c: Vec<f64> = factor_a
            .iter()
            .zip(factor_b.iter())
            .map(|(a, b)| a + b)
            .collect();

        let result = multi_factor_greeks(&returns, &[&factor_a, &factor_b, &factor_c], 252.0);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_handles_full_rank_scaled_factors() {
        let factor_a = [1.0e8, 2.0e8, 3.0e8, 4.0e8, 5.0e8, 6.0e8];
        let factor_b = [1.0e-4, -2.0e-4, 3.0e-4, -4.0e-4, 5.0e-4, -6.0e-4];
        let returns: Vec<f64> = factor_a
            .iter()
            .zip(factor_b.iter())
            .map(|(a, b)| 2.0 * a - 3.0 * b)
            .collect();

        let result = multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0)
            .expect("scaled factors should solve successfully");
        assert!((result.betas[0] - 2.0).abs() < 1e-10);
        assert!((result.betas[1] + 3.0).abs() < 5e-5);
    }
}
