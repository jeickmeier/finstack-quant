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
use crate::math::stats::{OnlineCovariance, OnlineStats};
use crate::regression::normalized_svd_least_squares;
use finstack_quant_core::math::{neumaier_sum, NeumaierAccumulator};
use nalgebra::DMatrix;

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

/// Recompute the four sliding-window power sums about the shift origins
/// `(shift_r, shift_b)`.
///
/// # Why the shift
///
/// Beta is formed from `(w*srb - sb*sr) / (w*sb2 - sb*sb)` — the normal-equation
/// form. Both numerator and denominator are differences of large, nearly equal
/// products whenever the inputs carry a large constant level relative to their
/// variation: for a series around `5e5` with moves of `1e-2`, `w*sb2` and
/// `sb*sb` agree to roughly twelve significant digits, so the subtraction throws
/// away most of the mantissa before the division ever happens.
///
/// Because covariance and variance are invariant to a constant shift, computing
/// the sums about a fixed origin near the data leaves beta mathematically
/// unchanged while making the cancelled quantities `O(variation)` instead of
/// `O(level)`. This is the standard shifted-data formulation for one-pass
/// variance (Chan, Golub & LeVeque 1983, "Algorithms for Computing the Sample
/// Variance"); it costs one subtraction per element and preserves the O(n)
/// sliding-window update, which centring per window would not.
///
/// The mean-dependent parts of alpha need the unshifted sums; the caller
/// reconstructs them as `sr + w*shift_r`, which involves no cancellation.
#[inline]
fn recompute_rolling_greeks_sums(
    returns: &[f64],
    benchmark: &[f64],
    shift_r: f64,
    shift_b: f64,
) -> (f64, f64, f64, f64) {
    (
        neumaier_sum(returns.iter().map(|&r| r - shift_r)),
        neumaier_sum(benchmark.iter().map(|&b| b - shift_b)),
        neumaier_sum(
            returns
                .iter()
                .zip(benchmark.iter())
                .map(|(&r, &b)| (r - shift_r) * (b - shift_b)),
        ),
        neumaier_sum(benchmark.iter().map(|&b| (b - shift_b) * (b - shift_b))),
    )
}

/// Seed a [`NeumaierAccumulator`] with a starting total.
#[inline]
fn seeded_accumulator(value: f64) -> NeumaierAccumulator {
    let mut acc = NeumaierAccumulator::new();
    acc.add(value);
    acc
}

/// Accumulate the sufficient statistics for a single-factor regression over
/// the overlapping prefix of two series.
#[inline]
fn regression_covariance(returns: &[f64], benchmark: &[f64]) -> (usize, OnlineCovariance) {
    let n = returns.len().min(benchmark.len());
    let mut covariance = OnlineCovariance::new();
    for (&portfolio_return, &benchmark_return) in returns[..n].iter().zip(benchmark[..n].iter()) {
        covariance.update(portfolio_return, benchmark_return);
    }
    (n, covariance)
}

/// Tracking error: annualized volatility of active (excess) returns.
///
/// Measures how consistently a portfolio follows its benchmark:
///
/// ```text
/// TE = σ(r_portfolio − r_benchmark) × sqrt(ann_factor)   [if annualized]
/// ```
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
/// = mean active return × sqrt(ann_factor) / σ active return
/// ```
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
/// ```
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
/// R-squared in `[0, 1]` when both overlapping series have at least two
/// observations and nonzero variance. Returns [`f64::NAN`] when R-squared
/// cannot be estimated, including empty or one-observation overlap and
/// zero-variance portfolio or benchmark series. Mismatched lengths are
/// truncated to the shorter series, matching the convention of
/// [`tracking_error`], [`beta`], and [`greeks`].
#[must_use]
pub(crate) fn r_squared(returns: &[f64], benchmark: &[f64]) -> f64 {
    let (n, covariance) = regression_covariance(returns, benchmark);
    if n < 2 || covariance.variance_x() == 0.0 || covariance.variance_y() == 0.0 {
        return f64::NAN;
    }
    let c = covariance.correlation();
    c * c
}

/// OLS beta result with standard error and 95% confidence interval.
///
/// All fields are [`f64::NAN`] when fewer than three paired observations are
/// available or the benchmark variance is zero.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BetaResult {
    /// Estimated beta coefficient, or [`f64::NAN`] when it is not estimable
    /// together with the confidence interval.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub beta: f64,
    /// Standard error of the beta estimate, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub std_err: f64,
    /// Lower bound of the 95% confidence interval, or [`f64::NAN`] when
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub ci_lower: f64,
    /// Upper bound of the 95% confidence interval, or [`f64::NAN`] when
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
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
/// Requires at least 3 paired observations because the result includes a
/// residual standard error and confidence interval.
///
/// # Arguments
///
/// * `portfolio`  - Portfolio return series. Mismatched input lengths are
///   truncated to the shorter overlap.
/// * `benchmark`  - Benchmark return series. A zero-variance overlapping
///   benchmark makes the regression undefined.
///
/// # Returns
///
/// A [`BetaResult`] with `beta`, `std_err`, `ci_lower`, and `ci_upper`.
/// All fields are [`f64::NAN`] when the series are too short (`n < 3`) or the
/// benchmark has zero variance (the slope is unidentifiable).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_analytics::beta;
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
    let (n, oc) = regression_covariance(portfolio, benchmark);
    if n < 3 {
        return BetaResult {
            beta: f64::NAN,
            std_err: f64::NAN,
            ci_lower: f64::NAN,
            ci_upper: f64::NAN,
        };
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
    // SE(β) = s / √SXX with s² = SSR/(n−2). `resid_var` carries an (n−1)
    // denominator and `var_bench` = SXX/(n−1); the two (n−1) factors cancel,
    // so resid_var / ((n−2)·var_bench) = [SSR/(n−2)] / SXX exactly.
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
/// Equivalent to the beta component of [`greeks`] but skips the alpha,
/// correlation, and adjusted-R² arithmetic. Used by callers (e.g. Treynor)
/// that need only the slope and otherwise pay for unused outputs.
///
/// Returns [`f64::NAN`] when fewer than two paired observations are available
/// or the benchmark variance is zero, because the slope cannot be identified.
/// Mismatched lengths are truncated to the shorter overlap. A `NaN` beta
/// propagates through downstream ratios (e.g. Treynor) instead of producing a
/// plausible-looking `0.0` or `±∞`.
#[must_use]
pub(crate) fn beta_only(returns: &[f64], benchmark: &[f64]) -> f64 {
    let (n, covariance) = regression_covariance(returns, benchmark);
    if n < 2 || covariance.variance_y() == 0.0 {
        return f64::NAN;
    }
    covariance.optimal_beta()
}

fn jensen_alpha(
    mean_port: f64,
    mean_bench: f64,
    beta: f64,
    ann_factor: f64,
    risk_free_rate: f64,
) -> f64 {
    let rf_period = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor);
    (mean_port - rf_period - beta * (mean_bench - rf_period)) * ann_factor
}

/// Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression.
///
/// Fields that cannot be estimated are [`f64::NAN`]. In particular, all
/// fields are `NaN` with fewer than two paired observations or a
/// zero-variance benchmark. R-squared fields are also `NaN` for a
/// zero-variance portfolio, and adjusted R-squared is `NaN` with fewer than
/// three observations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GreeksResult {
    /// Annualized Jensen alpha, or [`f64::NAN`] when the regression slope is
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub alpha: f64,
    /// Beta (slope) of portfolio vs benchmark, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub beta: f64,
    /// R-squared of the regression, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub r_squared: f64,
    /// Adjusted R-squared of the regression, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub adjusted_r_squared: f64,
}

/// Single-factor greeks for portfolio vs benchmark.
///
/// Runs a simple OLS regression to estimate beta, then reports annualized
/// Jensen alpha:
///
/// ```text
/// α = ((mean(r_p) - rf_period) - β × (mean(r_b) - rf_period)) × ann_factor
/// ```
///
/// Unlike [`beta`], this function does not compute standard errors and is
/// lighter-weight when point estimates are sufficient.
///
/// # Arguments
///
/// * `returns`    - Portfolio return series. Mismatched input lengths are
///   truncated to the shorter overlap.
/// * `benchmark`  - Benchmark return series. A zero-variance overlapping
///   benchmark makes alpha and beta undefined.
/// * `ann_factor` - Number of periods per year. Used to annualize alpha.
/// * `risk_free_rate` - Annualized risk-free rate used for Jensen alpha.
///
/// # Returns
///
/// A [`GreeksResult`] with annualized Jensen `alpha`, `beta`, `r_squared`, and
/// `adjusted_r_squared`. Fields that cannot be estimated are [`f64::NAN`]:
/// all fields for fewer than two paired observations or a zero-variance
/// benchmark; both R-squared fields for a zero-variance portfolio; and
/// adjusted R-squared for exactly two observations.
pub(crate) fn greeks(
    returns: &[f64],
    benchmark: &[f64],
    ann_factor: f64,
    risk_free_rate: f64,
) -> GreeksResult {
    let (n, covariance) = regression_covariance(returns, benchmark);
    if n < 2 {
        return GreeksResult {
            alpha: f64::NAN,
            beta: f64::NAN,
            r_squared: f64::NAN,
            adjusted_r_squared: f64::NAN,
        };
    }
    let beta = if covariance.variance_y() == 0.0 {
        f64::NAN
    } else {
        covariance.optimal_beta()
    };
    let alpha = jensen_alpha(
        covariance.mean_x(),
        covariance.mean_y(),
        beta,
        ann_factor,
        risk_free_rate,
    );
    let r_squared = if covariance.variance_x() == 0.0 || covariance.variance_y() == 0.0 {
        f64::NAN
    } else {
        let correlation = covariance.correlation();
        correlation * correlation
    };
    let adjusted_r_squared = if n > 2 && r_squared.is_finite() {
        1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / (n as f64 - 2.0)
    } else {
        f64::NAN
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
/// * `risk_free_rate` - Annualized risk-free rate used for Jensen alpha.
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
pub(crate) fn rolling_greeks(
    returns: &[f64],
    benchmark: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    risk_free_rate: f64,
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
    let rf_period = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor);

    // Incremental O(n) sliding-window OLS via running sums.
    //
    // The sums are kept about fixed origins near the data (see
    // `recompute_rolling_greeks_sums`) so the normal-equation differences below
    // cancel at the scale of the *variation* rather than the *level*.
    //
    // They accumulate differences (`new − old`) and pass near zero when returns
    // cancel, so the increment can exceed the running total in magnitude — the
    // case Kahan handles worse than Neumaier. Every read therefore goes through
    // `current()`, which returns `sum + compensation`; reading the bare sum
    // would discard the correction.
    let w = window as f64;
    let shift_r = returns[0];
    let shift_b = benchmark[0];
    let (seed_sr, seed_sb, seed_srb, seed_sb2) =
        recompute_rolling_greeks_sums(&returns[..window], &benchmark[..window], shift_r, shift_b);
    let mut acc_sr = seeded_accumulator(seed_sr);
    let mut acc_sb = seeded_accumulator(seed_sb);
    let mut acc_srb = seeded_accumulator(seed_srb);
    let mut acc_sb2 = seeded_accumulator(seed_sb2);
    let mut steps_since_recompute = 0usize;

    for i in window..=n {
        // Shifted sums: correct for beta (shift-invariant) as they stand.
        let (sr, sb, srb, sb2) = (
            acc_sr.current(),
            acc_sb.current(),
            acc_srb.current(),
            acc_sb2.current(),
        );
        let denom = w * sb2 - sb * sb;
        // Degenerate window (e.g. constant benchmark or NaN inputs) cannot
        // identify a beta; emit a sentinel `NaN` for both greeks so callers
        // see "do not use this point" instead of a plausible-looking 0.
        let (alpha, beta) = if denom.abs() < 1e-30 {
            (f64::NAN, f64::NAN)
        } else {
            let beta = (w * srb - sb * sr) / denom;
            // Alpha needs the *unshifted* means; adding the origin back is a
            // plain addition of same-signed magnitudes, so it introduces no
            // cancellation of its own.
            let mean_r = sr / w + shift_r;
            let mean_b = sb / w + shift_b;
            let alpha = (mean_r - rf_period - beta * (mean_b - rf_period)) * ann_factor;
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
            // Increments are formed from shifted values so they stay on the
            // same footing as the seeded sums.
            let (old_r, old_b) = (old_r - shift_r, old_b - shift_b);
            let (new_r, new_b) = (new_r - shift_r, new_b - shift_b);
            acc_sr.add(new_r - old_r);
            acc_sb.add(new_b - old_b);
            acc_srb.add(new_r * new_b - old_r * old_b);
            acc_sb2.add(new_b * new_b - old_b * old_b);
            steps_since_recompute += 1;
            // A non-finite value entering or leaving the window poisons the
            // running sums permanently (`x − NaN = NaN`), so recompute
            // immediately whenever any sum is non-finite. Windows that still
            // contain the non-finite value recompute to NaN (and emit NaN);
            // the first all-finite window after it exits recovers exact sums
            // instead of staying NaN until the next scheduled recompute.
            // Checked on the compensated totals, since a non-finite value
            // corrupts the compensation term as well as the raw sum.
            let sums_non_finite = !(acc_sr.current().is_finite()
                && acc_sb.current().is_finite()
                && acc_srb.current().is_finite()
                && acc_sb2.current().is_finite());
            if sums_non_finite || steps_since_recompute >= ROLLING_GREEKS_RECOMPUTE_INTERVAL {
                let start = i + 1 - window;
                let (sr, sb, srb, sb2) = recompute_rolling_greeks_sums(
                    &returns[start..=i],
                    &benchmark[start..=i],
                    shift_r,
                    shift_b,
                );
                acc_sr = seeded_accumulator(sr);
                acc_sb = seeded_accumulator(sb);
                acc_srb = seeded_accumulator(srb);
                acc_sb2 = seeded_accumulator(sb2);
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
/// Computes the ratio of the portfolio's annualized geometric return to the
/// benchmark's annualized geometric return over periods where the benchmark
/// return is non-negative, matching empyrical's annualized capture convention.
/// A value > 1.0 means the portfolio amplifies benchmark gains on a per-period
/// annualized geometric basis within the benchmark-up subset.
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
/// * `ann_factor` - Annualization factor used for subset geometric returns.
///
/// # Returns
///
/// Up capture ratio. Returns `0.0` if there are no up-benchmark periods
/// or the benchmark's compounded up-period return is negligible.
#[must_use]
pub(crate) fn up_capture(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> f64 {
    geometric_capture(returns, benchmark, ann_factor, |bench_return| {
        bench_return >= 0.0
    })
}

/// Down-market capture ratio: portfolio performance during benchmark down-periods.
///
/// Computes the ratio of the portfolio's annualized geometric return to the
/// benchmark's annualized geometric return over periods where the benchmark
/// return is negative, matching empyrical's annualized capture convention.
/// A value < 1.0 means the portfolio loses less than the benchmark during
/// downturns on an annualized geometric basis (desirable).
///
/// # Arguments
///
/// * `returns`   - Portfolio return series.
/// * `benchmark` - Benchmark return series.
/// * `ann_factor` - Annualization factor used for subset geometric returns.
///
/// # Returns
///
/// Down capture ratio. Returns `0.0` if there are no down-benchmark periods
/// or the benchmark's compounded down-period return is negligible.
#[must_use]
pub(crate) fn down_capture(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> f64 {
    geometric_capture(returns, benchmark, ann_factor, |bench_return| {
        bench_return < 0.0
    })
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
/// * `ann_factor` - Annualization factor used for subset geometric returns.
///
/// # Returns
///
/// The capture ratio. Returns `0.0` if either capture component is zero.
#[must_use]
pub(crate) fn capture_ratio(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> f64 {
    let day_count = down_capture(returns, benchmark, ann_factor);
    if day_count.is_nan() {
        return f64::NAN;
    }
    if day_count == 0.0 {
        return 0.0;
    }
    let uc = up_capture(returns, benchmark, ann_factor);
    if uc.is_nan() {
        return f64::NAN;
    }
    uc / day_count
}

/// Batting average: fraction of periods where portfolio outperforms benchmark.
///
/// ```text
/// BA = count(r_portfolio > r_benchmark) / n
/// ```
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
#[must_use]
pub(crate) fn batting_average(returns: &[f64], benchmark: &[f64]) -> f64 {
    let n = returns.len().min(benchmark.len());
    if n == 0 {
        return 0.0;
    }
    let wins = (0..n).filter(|&i| returns[i] > benchmark[i]).count();
    wins as f64 / n as f64
}

/// How the dependent return series is interpreted in a multi-factor regression.
///
/// Factor series are always treated as already-excess (Fama–French style).
/// Only the dependent series is adjusted when [`ReturnKind::Total`] is used.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// `returns` are already excess returns. Alpha is the annualized OLS
    /// intercept of excess `y` on the supplied (already-excess) factors.
    Excess,
    /// `returns` are total returns. The geometrically decompounded period
    /// risk-free rate is subtracted from `y` only, then OLS is run.
    /// Alpha is the annualized intercept and is Jensen-style for the
    /// dependent variable.
    Total {
        /// Annualized risk-free rate in decimal form (e.g. `0.02` for 2%).
        risk_free_rate: f64,
    },
}

impl ReturnKind {
    /// Attach an annualized risk-free rate to a [`ReturnKind::Total`] kind.
    ///
    /// [`ReturnKind::Excess`] ignores the rate and is returned unchanged, so
    /// hosts can parse a label and then apply the caller's rate uniformly.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form
    ///   (`0.02` for 2%), geometrically decompounded to the observation
    ///   frequency before it is subtracted from the dependent series.
    #[must_use]
    pub fn with_risk_free_rate(self, risk_free_rate: f64) -> Self {
        match self {
            Self::Excess => Self::Excess,
            Self::Total { .. } => Self::Total { risk_free_rate },
        }
    }
}

impl std::str::FromStr for ReturnKind {
    type Err = crate::error::Error;

    /// Parse `"excess"` or `"total"`.
    ///
    /// `"total"` yields [`ReturnKind::Total`] with a zero risk-free rate;
    /// use [`ReturnKind::with_risk_free_rate`] to attach the caller's rate.
    fn from_str(label: &str) -> Result<Self, Self::Err> {
        match label {
            "excess" => Ok(Self::Excess),
            "total" => Ok(Self::Total {
                risk_free_rate: 0.0,
            }),
            other => Err(crate::error::Error::Validation(format!(
                "unknown return_kind {other:?}; expected \"excess\" or \"total\""
            ))),
        }
    }
}

/// Result of a multi-factor regression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiFactorResult {
    /// Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
    pub alpha: f64,
    /// Regression coefficients, one per factor.
    pub betas: Vec<f64>,
    /// Fraction of variance explained; NaN for a constant dependent series.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub r_squared: f64,
    /// Adjusted R-squared; NaN when R-squared or residual degrees of freedom are undefined.
    ///
    /// ```text
    /// adj_R² = 1 − (1 − R²) × (n − 1) / (n − k − 1)
    /// ```
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    pub adjusted_r_squared: f64,
    /// Annualized residual volatility.
    pub residual_vol: f64,
}

fn geometric_capture<F>(returns: &[f64], benchmark: &[f64], ann_factor: f64, include: F) -> f64
where
    F: Fn(f64) -> bool,
{
    if !ann_factor.is_finite() || ann_factor <= 0.0 {
        return f64::NAN;
    }

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
    let port_geom = (neumaier_sum(port_logs) * ann_factor / count).exp() - 1.0;
    let bench_geom = (neumaier_sum(bench_logs) * ann_factor / count).exp() - 1.0;
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
/// y = α + β₁f₁ + β₂f₂ + ... + βₖfₖ + ε
/// ```
///
/// by solving the least-squares system with an SVD of the (column-normalized)
/// design matrix. SVD avoids explicitly forming the normal equations and is
/// numerically robust to correlated factors and rank deficiency.
///
/// Factor series are treated as already-excess. [`ReturnKind::Total`]
/// subtracts the geometrically decompounded period risk-free rate from the
/// **dependent** series only. [`ReturnKind::Excess`] leaves `y` unchanged;
/// the annualized intercept is then the alpha of already-excess `y`, not a
/// Jensen adjustment of total returns.
///
/// # Arguments
///
/// * `returns`     - Portfolio simple-return series in decimal form (for
///   example, `0.01` for 1%). Interpreted as excess or total according to
///   `return_kind`.
/// * `factors`     - Slice of factor return series (each inner slice is one
///   factor's return series, all the same length as `returns`). Factors are
///   already-excess returns in the same decimal convention.
/// * `ann_factor`  - Number of observation periods per year used to
///   annualize the intercept and residual volatility (e.g. `252.0` daily).
/// * `return_kind` - Whether `returns` are already excess or total. Total
///   subtracts `(1 + rf)^{1/N} − 1` from `y` only.
///
/// # Returns
///
/// A [`MultiFactorResult`] containing:
///
/// - `alpha`: annualized OLS intercept of the (possibly rf-adjusted) `y`
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
/// - [`ReturnKind::Total`] has a non-finite risk-free rate
/// - no factors are supplied
/// - there are too few observations for the requested number of factors
/// - any portfolio or factor return is non-finite
/// - any factor length differs from `returns.len()`
/// - the factor matrix is singular or numerically rank deficient
///
/// # References
///
/// - Fama & French (1993): see docs/REFERENCES.md#fama-french-1993
/// - Higham: see docs/REFERENCES.md#higham-accuracy-and-stability `docs/REFERENCES.md#higham-accuracy-and-stability`
#[tracing::instrument(level = "debug", skip(returns, factors), fields(n = returns.len(), k = factors.len(), ann_factor = ann_factor))]
pub(crate) fn multi_factor_greeks(
    returns: &[f64],
    factors: &[&[f64]],
    ann_factor: f64,
    return_kind: ReturnKind,
) -> crate::Result<MultiFactorResult> {
    if !ann_factor.is_finite() || ann_factor <= 0.0 {
        tracing::debug!(
            ann_factor,
            reason = "invalid_annualization_factor",
            "multi-factor greeks rejected input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }

    let adjusted_y;
    let y = match return_kind {
        ReturnKind::Excess => returns,
        ReturnKind::Total { risk_free_rate } => {
            let rf_period = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor);
            if !rf_period.is_finite() {
                return Err(crate::error::InputError::Invalid.into());
            }
            adjusted_y = returns.iter().map(|r| r - rf_period).collect::<Vec<_>>();
            adjusted_y.as_slice()
        }
    };

    let n = y.len();
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
    let design = DMatrix::from_fn(
        n,
        p,
        |row, col| {
            if col == 0 {
                1.0
            } else {
                factors[col - 1][row]
            }
        },
    );
    let targets = DMatrix::from_column_slice(n, 1, y);
    let solutions = normalized_svd_least_squares(design, &targets)?;
    let beta = solutions.column(0).iter().copied().collect::<Vec<_>>();

    let alpha_per_period = beta[0];
    let factor_betas: Vec<f64> = beta[1..].to_vec();

    // Compute residuals and R²
    let mut response_stats = OnlineStats::new();
    let mut ss_res = 0.0_f64;
    for (t, &r) in y.iter().enumerate().take(n) {
        let mut y_hat = alpha_per_period;
        for j in 0..k {
            let fj = factors[j][t];
            y_hat += factor_betas[j] * fj;
        }
        let residual = r - y_hat;
        ss_res += residual * residual;
        response_stats.update(r);
    }

    // Online variance preserves exact zero for constant observations, even
    // when summing and dividing their level would round the mean.
    let ss_tot = response_stats.variance() * (n - 1) as f64;
    let r_sq = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        f64::NAN
    };
    let dof = n as f64 - k as f64 - 1.0;
    let residual_var = if dof > 0.0 { ss_res / dof } else { 0.0 };
    let residual_vol = residual_var.sqrt() * ann_factor.sqrt();
    let alpha = alpha_per_period * ann_factor;

    let adjusted_r_squared = if dof > 0.0 && r_sq.is_finite() {
        1.0 - (1.0 - r_sq) * (n as f64 - 1.0) / dof
    } else {
        f64::NAN
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
    fn regression_statistics_are_nan_without_two_overlapping_observations() {
        for (returns, benchmark) in [
            (&[][..], &[][..]),
            (&[0.01][..], &[][..]),
            (&[][..], &[0.01][..]),
            (&[0.02][..], &[0.01][..]),
        ] {
            let beta_result = beta(returns, benchmark);
            assert!(beta_result.beta.is_nan());
            assert!(beta_result.std_err.is_nan());
            assert!(beta_result.ci_lower.is_nan());
            assert!(beta_result.ci_upper.is_nan());

            assert!(beta_only(returns, benchmark).is_nan());
            assert!(r_squared(returns, benchmark).is_nan());

            let greeks_result = greeks(returns, benchmark, 252.0, 0.0);
            assert!(greeks_result.alpha.is_nan());
            assert!(greeks_result.beta.is_nan());
            assert!(greeks_result.r_squared.is_nan());
            assert!(greeks_result.adjusted_r_squared.is_nan());
        }
    }

    #[test]
    fn degenerate_beta_and_greeks_json_round_trip_nan_fields() {
        let beta_result = beta(&[], &[]);
        let beta_json = serde_json::to_string(&beta_result).expect("beta result should serialize");
        let beta_round_trip: BetaResult =
            serde_json::from_str(&beta_json).expect("beta result should deserialize");
        assert!(beta_round_trip.beta.is_nan());
        assert!(beta_round_trip.std_err.is_nan());
        assert!(beta_round_trip.ci_lower.is_nan());
        assert!(beta_round_trip.ci_upper.is_nan());

        let greeks_result = greeks(&[], &[], 252.0, 0.0);
        let greeks_json =
            serde_json::to_string(&greeks_result).expect("greeks result should serialize");
        let greeks_round_trip: GreeksResult =
            serde_json::from_str(&greeks_json).expect("greeks result should deserialize");
        assert!(greeks_round_trip.alpha.is_nan());
        assert!(greeks_round_trip.beta.is_nan());
        assert!(greeks_round_trip.r_squared.is_nan());
        assert!(greeks_round_trip.adjusted_r_squared.is_nan());
    }

    #[test]
    fn beta_basic() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let x = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = beta(&y, &x);
        assert!((result.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn slope_only_supports_two_points_while_beta_result_requires_three() {
        let returns = [0.03, 0.05];
        let benchmark = [0.01, 0.02];

        assert!((beta_only(&returns, &benchmark) - 2.0).abs() < 1e-12);

        let result = beta(&returns, &benchmark);
        assert!(result.beta.is_nan());
        assert!(result.std_err.is_nan());
        assert!(result.ci_lower.is_nan());
        assert!(result.ci_upper.is_nan());

        let greeks_result = greeks(&returns, &benchmark, 12.0, 0.0);
        assert!((greeks_result.alpha - 0.12).abs() < 1e-12);
        assert!((greeks_result.beta - 2.0).abs() < 1e-12);
        assert!((greeks_result.r_squared - 1.0).abs() < 1e-12);
        assert!(greeks_result.adjusted_r_squared.is_nan());
    }

    #[test]
    fn batch_regression_truncates_to_shorter_valid_linear_overlap() {
        let returns = [0.01, 0.03, 0.05, 99.0];
        let benchmark = [0.00, 0.01, 0.02];

        let result = beta(&returns, &benchmark);
        assert!((result.beta - 2.0).abs() < 1e-12);
        assert!(result.std_err.abs() < 1e-12);
        assert!((result.ci_lower - 2.0).abs() < 1e-12);
        assert!((result.ci_upper - 2.0).abs() < 1e-12);

        assert!((beta_only(&returns, &benchmark) - 2.0).abs() < 1e-12);
        assert!((r_squared(&returns, &benchmark) - 1.0).abs() < 1e-12);

        let greeks_result = greeks(&returns, &benchmark, 12.0, 0.0);
        assert!((greeks_result.alpha - 0.12).abs() < 1e-12);
        assert!((greeks_result.beta - 2.0).abs() < 1e-12);
        assert!((greeks_result.r_squared - 1.0).abs() < 1e-12);
        assert!((greeks_result.adjusted_r_squared - 1.0).abs() < 1e-12);
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
        let g = greeks(&r, &b, 252.0, 0.0);
        assert!((g.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn greeks_reports_adjusted_r_squared() {
        let r = [0.011, 0.018, 0.031, 0.039, 0.052, 0.061];
        let b = [0.005, 0.010, 0.015, 0.020, 0.025, 0.030];
        let g = greeks(&r, &b, 252.0, 0.0);
        let n = r.len() as f64;
        let expected = 1.0 - (1.0 - g.r_squared) * (n - 1.0) / (n - 2.0);

        assert!((g.adjusted_r_squared - expected).abs() < 1e-12);
        assert!(g.adjusted_r_squared <= g.r_squared);
    }

    #[test]
    fn greeks_alpha_is_annualized_jensen_alpha() {
        let ann_factor = 12.0;
        let risk_free_rate = 0.12;
        let beta_value = 1.6;
        let intercept = 0.001;
        let b = [-0.02, -0.01, 0.00, 0.01, 0.02, 0.03];
        let r: Vec<f64> = b
            .iter()
            .map(|&bench| intercept + beta_value * bench)
            .collect();

        let zero_rf = greeks(&r, &b, ann_factor, 0.0);
        let nonzero_rf = greeks(&r, &b, ann_factor, risk_free_rate);
        let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
        let expected_change = rf_period * (zero_rf.beta - 1.0) * ann_factor;

        assert!((zero_rf.beta - beta_value).abs() < 1e-12);
        assert!((zero_rf.alpha - intercept * ann_factor).abs() < 1e-12);
        assert!((nonzero_rf.alpha - zero_rf.alpha - expected_change).abs() < 1e-12);
    }

    #[test]
    fn rolling_greeks_basic() {
        let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b: Vec<f64> = (0..20).map(|i| i as f64 * 0.0005).collect();
        let dates: Vec<Date> = (1..=20).map(jan).collect();
        let rg = rolling_greeks(&r, &b, &dates, 5, 252.0, 0.0);
        assert_eq!(rg.betas.len(), 16);
    }

    #[test]
    fn rolling_greeks_alpha_is_annualized_jensen_alpha() {
        let ann_factor = 12.0;
        let risk_free_rate = 0.12;
        let beta_value = 1.4;
        let intercept = 0.002;
        let b: Vec<f64> = (0..12).map(|i| -0.03 + i as f64 * 0.006).collect();
        let r: Vec<f64> = b
            .iter()
            .map(|&bench| intercept + beta_value * bench)
            .collect();
        let dates: Vec<Date> = (1..=12).map(jan).collect();

        let zero_rf = rolling_greeks(&r, &b, &dates, 6, ann_factor, 0.0);
        let nonzero_rf = rolling_greeks(&r, &b, &dates, 6, ann_factor, risk_free_rate);
        let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
        let expected_change = rf_period * (beta_value - 1.0) * ann_factor;

        for ((zero_alpha, nonzero_alpha), beta) in zero_rf
            .alphas
            .iter()
            .zip(nonzero_rf.alphas.iter())
            .zip(nonzero_rf.betas.iter())
        {
            assert!((*beta - beta_value).abs() < 1e-12);
            assert!((*zero_alpha - intercept * ann_factor).abs() < 1e-12);
            assert!((*nonzero_alpha - *zero_alpha - expected_change).abs() < 1e-12);
        }
    }

    #[test]
    fn rolling_greeks_emits_nan_when_benchmark_window_is_constant() {
        // Use exactly-representable benchmark values (0.5) so the OLS denominator
        // collapses to a true zero rather than floating-point noise.
        let window = 5;
        let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b: Vec<f64> = vec![0.5_f64; 20];
        let dates: Vec<Date> = (1..=20).map(jan).collect();
        let rg = rolling_greeks(&r, &b, &dates, window, 252.0, 0.0);
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
        let rg = rolling_greeks(&r, &b, &dates, window, 252.0, 0.0);

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
        assert!(r_squared(&r, &b).is_nan());

        let g = greeks(&r, &b, 252.0, 0.0);
        assert!(g.beta.is_nan());
        assert!(g.alpha.is_nan());
        assert!(g.r_squared.is_nan());
        assert!(g.adjusted_r_squared.is_nan());

        let result = beta(&r, &b);
        assert!(result.beta.is_nan());
        assert!(result.std_err.is_nan());
        assert!(result.ci_lower.is_nan());
        assert!(result.ci_upper.is_nan());

        // Treynor on an unidentifiable beta propagates NaN instead of ±∞.
        assert!(treynor(0.10, 0.02, beta_only(&r, &b), 1.0).is_nan());
    }

    #[test]
    fn rolling_greeks_stays_close_to_exact_recomputation_on_long_series() {
        /// Reference implementation, computed per window about that window's
        /// own means.
        ///
        /// Centring is what makes this a *reference*: the naive power-sum form
        /// is itself accurate to only ~8e-5 on this data (verified against
        /// exact rational arithmetic), so comparing the kernel against an
        /// uncentred reference measures agreement between two ill-conditioned
        /// computations rather than accuracy.
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
                let mean_r = neumaier_sum(rs.iter().copied()) / w;
                let mean_b = neumaier_sum(bs.iter().copied()) / w;
                let sr: f64 = neumaier_sum(rs.iter().map(|&r| r - mean_r));
                let sb: f64 = neumaier_sum(bs.iter().map(|&b| b - mean_b));
                let srb: f64 = neumaier_sum(
                    rs.iter()
                        .zip(bs.iter())
                        .map(|(&r, &b)| (r - mean_r) * (b - mean_b)),
                );
                let sb2: f64 = neumaier_sum(bs.iter().map(|&b| (b - mean_b) * (b - mean_b)));
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

        let rolling = rolling_greeks(&r, &b, &dates, window, ann_factor, 0.0);
        let (_, expected_betas) = exact_rolling_greeks(&r, &b, window, ann_factor);

        let max_beta_diff = rolling
            .betas
            .iter()
            .zip(expected_betas.iter())
            .map(|(&actual, &expected)| (actual - expected).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            // Measured 1.13e-13 with shifted-origin sums. The bound here used
            // to be 4.5e-4, which reflected the *uncentred* reference this
            // test compared against rather than any real error: kernel and
            // reference were losing the same ~8e-5 to cancellation, so they
            // agreed while both being wrong. Against exact rational
            // arithmetic the uncentred form errs by 7.8e-5 on this series and
            // the shifted form by 1.1e-13.
            max_beta_diff < 1e-12,
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
        let uc = up_capture(&r, &b, 1.0);
        assert!((uc - 2.0).abs() < 1e-12);
    }

    #[test]
    fn down_capture_hand_calc() {
        // Same data: down periods (b<0): index 1
        // port_prod = 0.95, bench_prod = 0.90
        // down_capture = (0.95−1) / (0.90−1) = −0.05/−0.10 = 0.5
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let day_count = down_capture(&r, &b, 1.0);
        assert!((day_count - 0.5).abs() < 1e-12);
    }

    #[test]
    fn capture_ratio_hand_calc() {
        // up/down = 2.0/0.5 = 4.0
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let cr = capture_ratio(&r, &b, 1.0);
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
        let uc = up_capture(&r, &b, 1.0);
        let expected = ((1.04_f64 * 1.06_f64).sqrt() - 1.0) / ((1.02_f64 * 1.03_f64).sqrt() - 1.0);
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_uses_empyrical_style_annualized_subset_return() {
        let r = vec![0.02; 36];
        let b = vec![0.015; 36];
        let uc = up_capture(&r, &b, 12.0);
        let expected = ((1.02_f64).powf(12.0) - 1.0) / ((1.015_f64).powf(12.0) - 1.0);

        assert!((uc - expected).abs() < 1e-12);
        assert!((uc - 1.371_251_926_947_35).abs() < 1e-12);
    }

    #[test]
    fn up_capture_uses_geometric_subset_returns() {
        let r = [1.0, 0.0, -0.4];
        let b = [0.5, 0.5, -0.1];
        let uc = up_capture(&r, &b, 1.0);
        let expected_port = (2.0_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (1.5_f64 * 1.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn down_capture_defensive_portfolio() {
        // Portfolio loses less than benchmark → day_count < 1.0 (desirable)
        let r = [0.04, -0.01, 0.06];
        let b = [0.02, -0.03, 0.03];
        let day_count = down_capture(&r, &b, 1.0);
        // Down periods: index 1. port_prod=0.99, bench_prod=0.97
        let expected = (0.99 - 1.0) / (0.97 - 1.0);
        assert!((day_count - expected).abs() < 1e-12);
        assert!(day_count < 1.0);
    }

    #[test]
    fn down_capture_uses_geometric_subset_returns() {
        let r = [-0.25, 0.0, 0.1];
        let b = [-0.5, -0.5, 0.1];
        let day_count = down_capture(&r, &b, 1.0);
        let expected_port = (0.75_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (0.5_f64 * 0.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((day_count - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_no_up_periods() {
        let r = [0.01, 0.02];
        let b = [-0.01, -0.02];
        assert_eq!(up_capture(&r, &b, 1.0), 0.0);
    }

    #[test]
    fn down_capture_no_down_periods() {
        let r = [0.01, 0.02];
        let b = [0.01, 0.02];
        assert_eq!(down_capture(&r, &b, 1.0), 0.0);
    }

    #[test]
    fn capture_ratio_perfect_tracking() {
        // Portfolio = benchmark → up_capture=1, down_capture=1, ratio=1
        let r = [0.02, -0.03, 0.01, -0.01];
        let cr = capture_ratio(&r, &r, 1.0);
        assert!((cr - 1.0).abs() < 1e-12);
    }

    #[test]
    fn multi_factor_single_factor() {
        // y = 2*x → alpha ≈ 0, beta ≈ 2, R² ≈ 1.
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess)
            .expect("single-factor regression");
        assert!((result.betas[0] - 2.0).abs() < 1e-8);
        assert!(result.r_squared > 0.999);
    }

    #[test]
    fn multi_factor_two_factors() {
        // y ≈ 1.5*f1 + 0.5*f2 (non-collinear factors).
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let f2 = [0.03, -0.01, 0.02, 0.01, -0.02];
        let y: Vec<f64> = (0..5).map(|i| 1.5 * f1[i] + 0.5 * f2[i]).collect();
        let result = multi_factor_greeks(&y, &[&f1, &f2], 252.0, ReturnKind::Excess)
            .expect("two-factor regression");
        assert!(result.r_squared > 0.99);
        assert_eq!(result.betas.len(), 2);
        assert!((result.betas[0] - 1.5).abs() < 1e-6);
        assert!((result.betas[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn multi_factor_empty_errors() {
        let result = multi_factor_greeks(&[], &[&[]], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_mismatched_factor_lengths_error() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_adjusted_r_squared() {
        // y = 2*x → R²≈1, adj_R² should also be close to 1
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess)
            .expect("adjusted r-squared regression");
        assert!(result.adjusted_r_squared > 0.99);
        assert!(result.adjusted_r_squared <= result.r_squared);
    }

    #[test]
    fn multi_factor_total_zero_rf_matches_excess() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let excess =
            multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess).expect("excess regression");
        let total_zero = multi_factor_greeks(
            &y,
            &[&f1],
            252.0,
            ReturnKind::Total {
                risk_free_rate: 0.0,
            },
        )
        .expect("total zero-rf regression");
        assert!((excess.alpha - total_zero.alpha).abs() < 1e-12);
        assert!((excess.betas[0] - total_zero.betas[0]).abs() < 1e-12);
        assert!((excess.r_squared - total_zero.r_squared).abs() < 1e-12);
        assert!((excess.residual_vol - total_zero.residual_vol).abs() < 1e-12);
    }

    #[test]
    fn multi_factor_total_matches_manual_excess_subtraction() {
        let y = [0.02, 0.03, 0.01, 0.04, 0.00];
        let f1 = [0.01, 0.02, -0.01, 0.03, 0.00];
        let rf_annual = 0.12;
        let rf_period = crate::returns::periodic_risk_free_rate(rf_annual, 252.0);
        let y_ex: Vec<f64> = y.iter().map(|r| r - rf_period).collect();
        let total = multi_factor_greeks(
            &y,
            &[&f1],
            252.0,
            ReturnKind::Total {
                risk_free_rate: rf_annual,
            },
        )
        .expect("total rf regression");
        let excess = multi_factor_greeks(&y_ex, &[&f1], 252.0, ReturnKind::Excess)
            .expect("manual excess regression");
        assert!((total.alpha - excess.alpha).abs() < 1e-12);
        assert!((total.betas[0] - excess.betas[0]).abs() < 1e-12);
        assert!((total.r_squared - excess.r_squared).abs() < 1e-12);
        assert!((total.residual_vol - excess.residual_vol).abs() < 1e-12);
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

/// Treynor ratio: excess return per unit of systematic risk.
///
/// ```text
/// Treynor = (R_p − R_f) / β
/// ```
///
/// Complements the Sharpe ratio by using beta (systematic risk) rather
/// than total volatility as the risk denominator.
///
/// # Arguments
///
/// * `ann_return`     - Linearly annualized arithmetic mean (`μ × N`).
/// * `risk_free_rate` - Annualized risk-free rate in decimal form.
/// * `beta`           - Portfolio beta vs benchmark.
/// * `ann_factor`     - Periods per year `N` used to decompound
///   `risk_free_rate`. Pass `1.0` when `ann_return` and `risk_free_rate`
///   are already in the same (annual) units.
///
/// # Returns
///
/// The Treynor ratio. When `|beta| < 1e-10`: returns `+∞` if the excess
/// return is positive, `−∞` if negative, and `0.0` only when the excess
/// return is also zero (matching the [`sharpe`](crate::risk_metrics) /
/// `information_ratio` zero-denominator convention). A `NaN` beta (e.g.
/// from a zero-variance benchmark) propagates to a `NaN` ratio.
///
/// Excess return uses the same geometric rf decompounding as
/// [`crate::risk_metrics::sharpe`]:
///
/// ```text
/// excess_ann = (μ − rf_period) × N
/// ```
///
/// # References
///
/// - Treynor (1965): see docs/REFERENCES.md#treynor1965
#[must_use]
pub(crate) fn treynor(ann_return: f64, risk_free_rate: f64, beta: f64, ann_factor: f64) -> f64 {
    let excess = crate::returns::annualized_excess_return(ann_return, risk_free_rate, ann_factor);
    if beta.abs() < 1e-10 {
        return if excess > 0.0 {
            f64::INFINITY
        } else if excess < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
    }
    excess / beta
}

/// M-squared (Modigliani-Modigliani): risk-adjusted return on the benchmark's scale.
///
/// Leverages or deleverages the portfolio to match the benchmark's volatility,
/// then reports the resulting return. The difference `M² − R_bench` is a
/// direct measure of value added at the same risk level.
///
/// ```text
/// M² = rf_period × N + excess_ann × (σ_bench / σ_portfolio)
/// excess_ann = (μ − rf_period) × N
/// ```
///
/// # Arguments
///
/// * `ann_return`     - Linearly annualized arithmetic mean (`μ × N`).
/// * `ann_vol`        - Annualized portfolio volatility.
/// * `bench_vol`      - Annualized benchmark volatility.
/// * `risk_free_rate` - Annualized risk-free rate in decimal form.
/// * `ann_factor`     - Periods per year `N` used to decompound
///   `risk_free_rate`. Pass `1.0` when `ann_return` and `risk_free_rate`
///   are already in the same (annual) units.
///
/// # Returns
///
/// The linearly annualized M-squared return. For zero portfolio volatility,
/// returns the decompounded periodic cash rate multiplied by `ann_factor`.
///
/// # References
///
/// - Modigliani & Modigliani (1997): see docs/REFERENCES.md#modigliani1997
#[must_use]
pub(crate) fn m_squared(
    ann_return: f64,
    ann_vol: f64,
    bench_vol: f64,
    risk_free_rate: f64,
    ann_factor: f64,
) -> f64 {
    let cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor) * ann_factor;
    if !cash.is_finite()
        || !ann_return.is_finite()
        || !ann_vol.is_finite()
        || !bench_vol.is_finite()
    {
        return f64::NAN;
    }
    if ann_vol.abs() < 1e-10 {
        return cash;
    }
    cash + (ann_return - cash) * (bench_vol / ann_vol)
}

#[cfg(test)]
mod benchmark_ratio_tests {
    use super::*;

    #[test]
    fn treynor_hand_calc() {
        let t = treynor(0.10, 0.02, 1.2, 1.0);
        assert!((t - 0.08 / 1.2).abs() < 1e-14);
    }

    #[test]
    fn treynor_zero_beta() {
        assert_eq!(treynor(0.10, 0.02, 0.0, 1.0), f64::INFINITY);
        assert_eq!(treynor(0.01, 0.02, 0.0, 1.0), f64::NEG_INFINITY);
        assert_eq!(treynor(0.02, 0.02, 0.0, 1.0), 0.0);
    }

    #[test]
    fn treynor_negative_beta() {
        let t = treynor(0.10, 0.02, -0.5, 1.0);
        assert!((t - (0.08 / -0.5)).abs() < 1e-14);
    }

    #[test]
    fn m_squared_hand_calc() {
        let m2 = m_squared(0.12, 0.20, 0.15, 0.02, 1.0);
        assert!((m2 - 0.095).abs() < 1e-12);
    }

    #[test]
    fn m_squared_zero_vol() {
        assert_eq!(m_squared(0.10, 0.0, 0.15, 0.02, 1.0), 0.02);
    }

    #[test]
    fn jensen_and_sharpe_agree_on_rf_when_beta_is_zero() {
        let mu = 0.0004_f64;
        let ann_factor = 252.0;
        let rf_annual = 0.02;
        let rf_period = crate::returns::periodic_risk_free_rate(rf_annual, ann_factor);
        let expected = (mu - rf_period) * ann_factor;
        let alpha = jensen_alpha(mu, 0.001, 0.0, ann_factor, rf_annual);
        let sharpe_excess =
            crate::returns::annualized_excess_return(mu * ann_factor, rf_annual, ann_factor);
        assert!((alpha - expected).abs() < 1e-12);
        assert!((sharpe_excess - expected).abs() < 1e-12);
        assert!((alpha - sharpe_excess).abs() < 1e-12);
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

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess);
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
        let result = perf.multi_factor_greeks(1, &[&invalid_factor], ReturnKind::Excess);
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

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_non_positive_ann_factor() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10];
        let factor = [0.01, 0.02, 0.03, 0.04, 0.05];

        assert!(multi_factor_greeks(&returns, &[&factor], 0.0, ReturnKind::Excess).is_err());
        assert!(multi_factor_greeks(&returns, &[&factor], -252.0, ReturnKind::Excess).is_err());
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

        let result = multi_factor_greeks(
            &returns,
            &[&factor_a, &factor_b, &factor_c],
            252.0,
            ReturnKind::Excess,
        );
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

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess)
                .expect("scaled factors should solve successfully");
        assert!((result.betas[0] - 2.0).abs() < 1e-10);
        assert!((result.betas[1] + 3.0).abs() < 5e-5);
    }
}
