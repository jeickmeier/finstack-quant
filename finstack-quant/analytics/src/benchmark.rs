//! Benchmark-relative statistics with exact positional pairing and scaled centered OLS.

use crate::exact::{exact_pair, rational, rational_output, rational_sqrt};
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::special_functions::student_t_inv_cdf;
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_core::wire::NonFiniteFields;
use finstack_quant_core::{Error, Result};
use num_traits::{Signed, ToPrimitive, Zero};

const ROLLING_GREEKS_RECOMPUTE_INTERVAL: usize = 64;

fn calculation(message: &str) -> Error {
    Error::Internal(format!("benchmark calculation: {message}"))
}

fn validate_pairs(portfolio: &[f64], benchmark: &[f64]) -> Result<()> {
    if portfolio.len() != benchmark.len() {
        return Err(Error::Validation(
            "benchmark inputs must have equal lengths".into(),
        ));
    }
    if portfolio.iter().chain(benchmark).any(|x| !x.is_finite()) {
        return Err(Error::Validation(
            "benchmark observations must be finite".into(),
        ));
    }
    Ok(())
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(calculation("requested result is not representable"))
    }
}

// Keep ordinary arithmetic exact where possible; a logarithmic rescaling is a
// recovery path for the same product/quotient, never an estimator substitute.
fn product_ratio(a: f64, b: f64, c: f64) -> Result<f64> {
    if a == 0.0 || b == 0.0 {
        return Ok(0.0);
    }
    let direct = a * (b / c);
    if direct.is_finite() && direct != 0.0 {
        return Ok(direct);
    }
    let alternate = (a / c) * b;
    if alternate.is_finite() && alternate != 0.0 {
        return Ok(alternate);
    }
    let logarithm = a.abs().ln() + b.abs().ln() - c.abs().ln();
    let value = logarithm.exp().copysign(a * b.signum() * c.signum());
    if value == 0.0 {
        return Err(calculation("nonzero requested result underflowed"));
    }
    finite(value)
}

struct Centered {
    mean: f64,
    scale: f64,
    values: Vec<f64>,
    sum_squares: f64,
}

fn centered(values: &[f64]) -> Result<Centered> {
    let origin = values.first().copied().unwrap_or(0.0);
    let raw: Vec<f64> = values.iter().map(|&x| x - origin).collect();
    let use_shift = raw.iter().all(|x| x.is_finite());
    let scale = if use_shift {
        raw.iter().map(|x| x.abs()).fold(0.0, f64::max)
    } else {
        values.iter().map(|x| x.abs()).fold(0.0, f64::max)
    };
    if scale == 0.0 {
        return Ok(Centered {
            mean: origin,
            scale: 0.0,
            values: vec![0.0; values.len()],
            sum_squares: 0.0,
        });
    }
    let normalized: Vec<f64> = if use_shift {
        raw.iter().map(|x| x / scale).collect()
    } else {
        values.iter().map(|x| x / scale).collect()
    };
    let mean_normalized = neumaier_sum(normalized.iter().map(|x| x / values.len() as f64));
    let mean = if use_shift {
        origin + scale * mean_normalized
    } else {
        scale * mean_normalized
    };
    let mean = if mean.is_finite() {
        mean
    } else {
        finite(neumaier_sum(values.iter().map(|x| x / values.len() as f64)))?
    };
    let deviations: Vec<f64> = normalized.iter().map(|x| x - mean_normalized).collect();
    let sum_squares = neumaier_sum(deviations.iter().map(|x| x * x));
    if sum_squares <= 0.0 || !sum_squares.is_finite() {
        return Err(calculation("nonconstant centered variation collapsed"));
    }
    Ok(Centered {
        mean,
        scale,
        values: deviations,
        sum_squares,
    })
}

fn centered_r_squared(p: &Centered, b: &Centered) -> Result<f64> {
    if p.scale == 0.0 || b.scale == 0.0 {
        return Ok(f64::NAN);
    }
    let cross = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| x * y));
    let correlation = (cross / b.sum_squares.sqrt()) / p.sum_squares.sqrt();
    let result = correlation * correlation;
    if !result.is_finite() || result > 1.0 + 32.0 * f64::EPSILON || (result == 0.0 && cross != 0.0)
    {
        return Err(calculation("invalid squared correlation"));
    }
    Ok(result.min(1.0))
}

struct Fit {
    beta: f64,
    mean_portfolio: f64,
    mean_benchmark: f64,
    r_squared: f64,
    se: f64,
}

fn fit(portfolio: &[f64], benchmark: &[f64], uncertainty: bool, fit_stats: bool) -> Result<Fit> {
    validate_pairs(portfolio, benchmark)?;
    let n = portfolio.len();
    let undefined = Fit {
        beta: f64::NAN,
        mean_portfolio: f64::NAN,
        mean_benchmark: f64::NAN,
        r_squared: f64::NAN,
        se: f64::NAN,
    };
    if n < 2 || benchmark.iter().all(|&x| x - benchmark[0] == 0.0) {
        return Ok(undefined);
    }
    let b = centered(benchmark)?;
    let p = centered(portfolio)?;
    let cross = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| x * y));
    let cross_size = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| (x * y).abs()));
    if cross.abs() <= f64::EPSILON.sqrt() * cross_size {
        return exact_fit(portfolio, benchmark, uncertainty, fit_stats);
    }
    let normalized_slope = cross / b.sum_squares;
    let beta = if p.scale == 0.0 {
        0.0
    } else {
        product_ratio(normalized_slope, p.scale, b.scale)?
    };
    let residual_ss = if uncertainty {
        neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| {
            let residual = (-normalized_slope).mul_add(*y, *x);
            residual * residual
        }))
    } else {
        0.0
    };
    if uncertainty && portfolio.len() >= 3 && residual_ss <= f64::EPSILON.sqrt() * p.sum_squares {
        return exact_fit(portfolio, benchmark, uncertainty, fit_stats);
    }
    if !residual_ss.is_finite() || residual_ss < 0.0 {
        return Err(calculation("invalid centered residual variation"));
    }
    let r_squared = if fit_stats {
        centered_r_squared(&p, &b)?
    } else {
        f64::NAN
    };
    let se = if !uncertainty || n < 3 {
        f64::NAN
    } else if p.scale == 0.0 || residual_ss == 0.0 {
        0.0
    } else {
        product_ratio(
            (residual_ss / ((n - 2) as f64 * b.sum_squares)).sqrt(),
            p.scale,
            b.scale,
        )?
    };
    Ok(Fit {
        beta,
        mean_portfolio: p.mean,
        mean_benchmark: b.mean,
        r_squared,
        se,
    })
}

fn exact_fit(
    portfolio: &[f64],
    benchmark: &[f64],
    uncertainty: bool,
    fit_stats: bool,
) -> Result<Fit> {
    let pair = exact_pair(portfolio, benchmark)?;
    let slope = &pair.sxy / &pair.sxx;
    let beta = rational_output(&slope)?;
    let r_squared = if !fit_stats || pair.syy.is_zero() {
        f64::NAN
    } else {
        rational_output(&((&pair.sxy * &pair.sxy) / (&pair.sxx * &pair.syy)))?
    };
    let se = if !uncertainty || portfolio.len() < 3 {
        f64::NAN
    } else {
        let sse = &pair.syy - (&pair.sxy * &pair.sxy) / &pair.sxx;
        rational_sqrt(&(sse / (rational((portfolio.len() - 2) as f64)? * &pair.sxx)))?
    };
    Ok(Fit {
        beta,
        mean_portfolio: pair
            .mean_portfolio
            .to_f64()
            .ok_or_else(|| calculation("mean reconstruction failed"))?,
        mean_benchmark: pair
            .mean_benchmark
            .to_f64()
            .ok_or_else(|| calculation("mean reconstruction failed"))?,
        r_squared,
        se,
    })
}

fn exact_jensen(portfolio: &[f64], benchmark: &[f64], ann_factor: f64, cash: f64) -> Result<f64> {
    let pair = exact_pair(portfolio, benchmark)?;
    let q = rational(cash)?;
    let intercept =
        (&pair.mean_portfolio - &q) - (&pair.sxy / &pair.sxx) * (&pair.mean_benchmark - &q);
    rational_output(&(rational(ann_factor)? * intercept))
}

fn stable_jensen(
    portfolio: &[f64],
    benchmark: &[f64],
    fit: &Fit,
    ann_factor: f64,
    cash: f64,
) -> Result<f64> {
    let alpha = jensen_alpha(
        fit.mean_portfolio,
        fit.mean_benchmark,
        fit.beta,
        ann_factor,
        cash,
    );
    let scale =
        (fit.mean_portfolio.abs() + (fit.beta * (fit.mean_benchmark - cash)).abs() + cash.abs())
            * ann_factor;
    if !alpha.is_finite() || alpha.abs() <= f64::EPSILON.sqrt() * scale {
        exact_jensen(portfolio, benchmark, ann_factor, cash)
    } else {
        Ok(alpha)
    }
}

fn exact_active(
    returns: &[f64],
    benchmark: &[f64],
    annualization: f64,
    ratio: bool,
) -> Result<f64> {
    let pair = exact_pair(returns, benchmark)?;
    let ss = &pair.syy + &pair.sxx - rational(2.0)? * &pair.sxy;
    let count = rational((returns.len() - 1) as f64)?;
    let a = rational(annualization)?;
    if !ratio {
        return rational_sqrt(&(ss * a / count));
    }
    let mean = &pair.mean_portfolio - &pair.mean_benchmark;
    if ss.is_zero() {
        return Ok(if mean.is_zero() {
            f64::NAN
        } else if mean.is_positive() {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        });
    }
    if mean.is_zero() {
        return Ok(0.0);
    }
    let magnitude = rational_sqrt(&((&mean * &mean) * a * count / ss))?;
    Ok(if mean.is_negative() {
        -magnitude
    } else {
        magnitude
    })
}

/// Annualized sample SD of exactly paired arithmetic active returns.
///
/// # Arguments
/// * `returns` - Finite portfolio observations, aligned by position.
/// * `benchmark` - Finite benchmark observations of exactly the same length.
/// * `ann_factor` - Explicit finite positive observations per year.
///
/// # Errors
/// Rejects invalid pairing/frequency and unrepresentable calculations.
pub(crate) fn tracking_error(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> Result<f64> {
    validate_pairs(returns, benchmark)?;
    crate::returns::validate_frequency(ann_factor)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let active: Vec<f64> = returns.iter().zip(benchmark).map(|(p, b)| p - b).collect();
    if active.iter().any(|x| !x.is_finite()) {
        return exact_active(returns, benchmark, ann_factor, false);
    }
    let Ok((_, sd)) = crate::risk_metrics::mean_sample_sd(&active) else {
        return exact_active(returns, benchmark, ann_factor, false);
    };
    if sd == 0.0
        && (returns.windows(2).any(|w| w[0] - w[1] != 0.0)
            || benchmark.windows(2).any(|w| w[0] - w[1] != 0.0))
    {
        return exact_active(returns, benchmark, ann_factor, false);
    }
    finite(sd * ann_factor.sqrt()).or_else(|_| exact_active(returns, benchmark, ann_factor, false))
}

/// Annual arithmetic active reward divided by annualized sample active risk.
///
/// # Arguments
/// * `returns` - Finite portfolio observations, aligned by position.
/// * `benchmark` - Finite benchmark observations of exactly the same length.
/// * `ann_factor` - Explicit finite positive observations per year.
pub(crate) fn information_ratio(
    returns: &[f64],
    benchmark: &[f64],
    ann_factor: f64,
) -> Result<f64> {
    validate_pairs(returns, benchmark)?;
    crate::returns::validate_frequency(ann_factor)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let active: Vec<f64> = returns.iter().zip(benchmark).map(|(p, b)| p - b).collect();
    if active.iter().any(|x| !x.is_finite()) {
        return exact_active(returns, benchmark, ann_factor, true);
    }
    let Ok((mean, sd)) = crate::risk_metrics::mean_sample_sd(&active) else {
        return exact_active(returns, benchmark, ann_factor, true);
    };
    if sd == 0.0 {
        return exact_active(returns, benchmark, ann_factor, true);
    }
    product_ratio(mean, ann_factor.sqrt(), sd)
        .or_else(|_| exact_active(returns, benchmark, ann_factor, true))
}

/// Squared Pearson correlation of exactly paired finite observations.
///
/// # Arguments
/// * `returns` - Portfolio observations, with no investment-domain restriction.
/// * `benchmark` - Equal-length position-paired benchmark observations.
pub(crate) fn r_squared(returns: &[f64], benchmark: &[f64]) -> Result<f64> {
    validate_pairs(returns, benchmark)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let p = centered(returns)?;
    let b = centered(benchmark)?;
    if p.scale == 0.0 || b.scale == 0.0 {
        return Ok(f64::NAN);
    }
    let cross = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| x * y));
    let size = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| (x * y).abs()));
    if cross.abs() <= f64::EPSILON.sqrt() * size {
        let pair = exact_pair(returns, benchmark)?;
        return rational_output(&((&pair.sxy * &pair.sxy) / (&pair.sxx * &pair.syy)));
    }
    centered_r_squared(&p, &b)
}

/// OLS beta result with standard error and 95% confidence interval.
///
/// Floating-point estimates are [`f64::NAN`] when fewer than two paired observations are
/// available or the benchmark is constant; uncertainty additionally needs three pairs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BetaResult {
    /// Estimated beta coefficient, or [`f64::NAN`] when fewer than two pairs or a constant benchmark makes it unidentified.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub beta: f64,
    /// Standard error of the beta estimate, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub std_err: f64,
    /// Lower bound of the 95% confidence interval, or [`f64::NAN`] when
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub ci_lower: f64,
    /// Upper bound of the 95% confidence interval, or [`f64::NAN`] when
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub ci_upper: f64,
    /// Number of paired observations the regression used: the exact equal-length position-paired sample. Reported even when the
    /// estimates are [`f64::NAN`]. Required in the serialized result.
    pub n_obs: usize,
    /// Two-sided confidence level of `ci_lower`/`ci_upper` as a decimal
    /// probability (`0.95` = 95%). The interval is
    /// `beta ± t_{n_obs − 2, (1 + confidence_level) / 2} × std_err`.
    pub confidence_level: f64,
}

/// Two-sided confidence level of the [`BetaResult`] interval (95%).
const BETA_CONFIDENCE_LEVEL: f64 = 0.95;

impl NonFiniteFields for BetaResult {
    const NON_FINITE_FIELDS: &'static [&'static str] = &["beta", "std_err", "ci_lower", "ci_upper"];
}

/// Fit equal-weight OLS with an intercept and a fixed classical 95% Student-t interval.
///
/// Beta is available with two pairs and a varying benchmark; SE and confidence
/// bounds require three pairs. Unavailable fields are NaN independently.
/// Classical interval coverage assumes independent homoskedastic Gaussian errors.
///
/// # Arguments
/// * `portfolio` - Finite response observations; caller supplies economic alignment.
/// * `benchmark` - Finite explanatory observations of exactly the same length.
///
/// # Errors
/// Rejects unequal lengths/nonfinite observations and unrepresentable numerical results.
///
/// # Examples
/// ```rust
/// let result = finstack_quant_analytics::beta(&[-0.01, 0.01, 0.03], &[-0.01, 0.0, 0.01])?;
/// assert!((result.beta - 2.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn beta(portfolio: &[f64], benchmark: &[f64]) -> Result<BetaResult> {
    let estimate = fit(portfolio, benchmark, true, false)?;
    let mut result = BetaResult {
        beta: estimate.beta,
        std_err: estimate.se,
        ci_lower: f64::NAN,
        ci_upper: f64::NAN,
        n_obs: portfolio.len(),
        confidence_level: BETA_CONFIDENCE_LEVEL,
    };
    if estimate.se.is_finite() {
        let critical = student_t_inv_cdf(0.975, (portfolio.len() - 2) as f64)?;
        let margin = finite(critical * estimate.se)?;
        result.ci_lower = finite(estimate.beta - margin)?;
        result.ci_upper = finite(estimate.beta + margin)?;
    }
    Ok(result)
}

fn jensen_alpha(
    mean_port: f64,
    mean_bench: f64,
    beta: f64,
    ann_factor: f64,
    rf_period: f64,
) -> f64 {
    (-beta).mul_add(mean_bench - rf_period, mean_port - rf_period) * ann_factor
}

/// Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression.
///
/// Fields that cannot be estimated are [`f64::NAN`]. In particular, all
/// fields are `NaN` with fewer than two paired observations or a
/// zero-variance benchmark. R-squared fields are also `NaN` for a
/// zero-variance portfolio, and adjusted R-squared is `NaN` with fewer than
/// three observations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GreeksResult {
    /// Annualized Jensen alpha, or [`f64::NAN`] when the regression slope is
    /// undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub alpha: f64,
    /// Beta (slope) of portfolio vs benchmark, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub beta: f64,
    /// R-squared of the regression, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub r_squared: f64,
    /// Adjusted R-squared of the regression, or [`f64::NAN`] when undefined.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub adjusted_r_squared: f64,
    /// Number of paired observations the regression used: the exact equal-length position-paired sample. Reported even when the
    /// estimates are [`f64::NAN`]. Required in the serialized result.
    pub n_obs: usize,
}

impl NonFiniteFields for GreeksResult {
    const NON_FINITE_FIELDS: &'static [&'static str] =
        &["alpha", "beta", "r_squared", "adjusted_r_squared"];
}

pub(crate) fn greeks(
    returns: &[f64],
    benchmark: &[f64],
    ann_factor: f64,
    risk_free_rate: f64,
) -> Result<GreeksResult> {
    let cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    let estimate = fit(returns, benchmark, false, true)?;
    let alpha = if estimate.beta.is_nan() {
        f64::NAN
    } else {
        stable_jensen(returns, benchmark, &estimate, ann_factor, cash)?
    };
    let n = returns.len();
    Ok(GreeksResult {
        alpha,
        beta: estimate.beta,
        r_squared: estimate.r_squared,
        adjusted_r_squared: if n > 2 && estimate.r_squared.is_finite() {
            finite(1.0 - (1.0 - estimate.r_squared) * (n - 1) as f64 / (n - 2) as f64)?
        } else {
            f64::NAN
        },
        n_obs: n,
    })
}

/// Complete-window OLS/Jensen point estimates, labelled by window end dates.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RollingGreeks {
    /// Strictly increasing return-ending dates, one per completed window.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Annual arithmetic Jensen alpha; undefined windows serialize as `"nan"`.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::FiniteOrNanWire>")
    )]
    pub alphas: Vec<f64>,
    /// Dimensionless OLS beta; unavailability matches alpha at every date.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::FiniteOrNanWire>")
    )]
    pub betas: Vec<f64>,
}

impl<'de> serde::Deserialize<'de> for RollingGreeks {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            dates: Vec<Date>,
            #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
            alphas: Vec<f64>,
            #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
            betas: Vec<f64>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.dates.len() != wire.alphas.len()
            || wire.dates.len() != wire.betas.len()
            || wire.dates.windows(2).any(|w| w[0] >= w[1])
            || wire
                .alphas
                .iter()
                .zip(&wire.betas)
                .any(|(a, b)| a.is_infinite() || b.is_infinite() || a.is_nan() != b.is_nan())
        {
            return Err(serde::de::Error::custom(
                "invalid rolling greeks dates, shape or coefficient domain",
            ));
        }
        Ok(Self {
            dates: wire.dates,
            alphas: wire.alphas,
            betas: wire.betas,
        })
    }
}

fn recompute_rolling_greeks_sums(
    returns: &[f64],
    benchmark: &[f64],
    shift_r: f64,
    shift_b: f64,
) -> (f64, f64, f64, f64) {
    (
        neumaier_sum(returns.iter().map(|r| r - shift_r)),
        neumaier_sum(benchmark.iter().map(|b| b - shift_b)),
        neumaier_sum(
            returns
                .iter()
                .zip(benchmark)
                .map(|(r, b)| (r - shift_r) * (b - shift_b)),
        ),
        neumaier_sum(benchmark.iter().map(|b| (b - shift_b) * (b - shift_b))),
    )
}

fn seeded_accumulator(value: f64) -> NeumaierAccumulator {
    let mut sum = NeumaierAccumulator::new();
    sum.add(value);
    sum
}

pub(crate) fn rolling_greeks(
    returns: &[f64],
    benchmark: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    risk_free_rate: f64,
) -> Result<RollingGreeks> {
    validate_pairs(returns, benchmark)?;
    let rf_period = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    if window == 0 || dates.len() != returns.len() || dates.windows(2).any(|w| w[0] >= w[1]) {
        return Err(Error::Validation(
            "rolling greeks require a positive window and aligned strictly increasing dates".into(),
        ));
    }
    let n = returns.len();
    if n < window {
        tracing::debug!(
            n,
            window,
            reason = "insufficient_window",
            "rolling greeks returning empty result"
        );
        return Ok(RollingGreeks {
            dates: vec![],
            alphas: vec![],
            betas: vec![],
        });
    }
    let count = n - window + 1;
    if count > ROLLING_GREEKS_RECOMPUTE_INTERVAL {
        tracing::debug!(
            n,
            window,
            count,
            recompute_interval = ROLLING_GREEKS_RECOMPUTE_INTERVAL,
            "rolling greeks using incremental updates with periodic window rebuilds"
        );
    }
    let mut out_dates = Vec::with_capacity(count);
    let mut alphas = Vec::with_capacity(count);
    let mut betas = Vec::with_capacity(count);

    // Constant-cost sliding-window OLS updates between full-window rebuilds.
    //
    // Recenter on rebuilds so that observations which have left the window
    // cannot set the scale of cancellation indefinitely.
    //
    // They accumulate differences (`new − old`) and pass near zero when returns
    // cancel, so the increment can exceed the running total in magnitude — the
    // case Kahan handles worse than Neumaier. Every read therefore goes through
    // `current()`, which returns `sum + compensation`; reading the bare sum
    // would discard the correction.
    let w = window as f64;
    let mut shift_r = returns[0];
    let mut shift_b = benchmark[0];
    let (seed_sr, seed_sb, seed_srb, seed_sb2) =
        recompute_rolling_greeks_sums(&returns[..window], &benchmark[..window], shift_r, shift_b);
    let mut acc_sr = seeded_accumulator(seed_sr);
    let mut acc_sb = seeded_accumulator(seed_sb);
    let mut acc_srb = seeded_accumulator(seed_srb);
    let mut acc_sb2 = seeded_accumulator(seed_sb2);
    let mut steps_since_recompute = 0usize;
    let mut variance_scale = 0.0_f64;
    let mut cov_scale = 0.0_f64;
    let mut r_scale = 0.0_f64;
    let mut b_scale = 0.0_f64;

    for i in window..=n {
        // Shifted sums: correct for beta (shift-invariant) as they stand.
        let (mut sr, mut sb, mut srb, mut sb2) = (
            acc_sr.total(),
            acc_sb.total(),
            acc_srb.total(),
            acc_sb2.total(),
        );
        let mut denom = w * sb2 - sb * sb;
        // Remember the largest power-sum scale since the last rebuild:
        // removing an outlier can leave rounding residue even when the
        // remaining sums themselves are tiny. This relative trigger rebuilds
        // the window; it never declares small but nonzero variance invalid.
        variance_scale = variance_scale.max((w * sb2).abs() + sb * sb);
        cov_scale = cov_scale.max((w * srb).abs() + (sb * sr).abs());
        r_scale = r_scale.max(sr.abs());
        b_scale = b_scale.max(sb.abs());
        let cancellation = (variance_scale > 0.0 && denom <= f64::EPSILON.sqrt() * variance_scale)
            || (cov_scale > 0.0 && (w * srb - sb * sr).abs() <= f64::EPSILON.sqrt() * cov_scale)
            || (r_scale > 0.0
                && (sr.abs() <= f64::EPSILON.sqrt() * r_scale
                    || (sr + w * shift_r).abs() <= f64::EPSILON.sqrt() * sr.abs()))
            || (b_scale > 0.0
                && (sb.abs() <= f64::EPSILON.sqrt() * b_scale
                    || (sb + w * shift_b).abs() <= f64::EPSILON.sqrt() * sb.abs()));
        let sums_non_finite =
            !(sr.is_finite() && sb.is_finite() && srb.is_finite() && sb2.is_finite());
        if cancellation
            || sums_non_finite
            || steps_since_recompute >= ROLLING_GREEKS_RECOMPUTE_INTERVAL
        {
            let start = i - window;
            shift_r = returns[start];
            shift_b = benchmark[start];
            (sr, sb, srb, sb2) = recompute_rolling_greeks_sums(
                &returns[start..i],
                &benchmark[start..i],
                shift_r,
                shift_b,
            );
            acc_sr = seeded_accumulator(sr);
            acc_sb = seeded_accumulator(sb);
            acc_srb = seeded_accumulator(srb);
            acc_sb2 = seeded_accumulator(sb2);
            steps_since_recompute = 0;
            variance_scale = (w * sb2).abs() + sb * sb;
            cov_scale = (w * srb).abs() + (sb * sr).abs();
            r_scale = sr.abs();
            b_scale = sb.abs();
            denom = w * sb2 - sb * sb;
        }
        // A cancelled power-sum state is rebuilt using bounded centered
        // coordinates before a field is classified as unavailable.
        let raw_beta = (w * srb - sb * sr) / denom;
        let raw_alpha = jensen_alpha(
            sr / w + shift_r,
            sb / w + shift_b,
            raw_beta,
            ann_factor,
            rf_period,
        );
        let (alpha, beta) = if window < 2 {
            (f64::NAN, f64::NAN)
        } else if cancellation
            || sums_non_finite
            || denom <= 0.0
            || raw_beta == 0.0
            || raw_alpha.abs()
                <= f64::EPSILON.sqrt()
                    * ann_factor
                    * ((sr / w + shift_r).abs()
                        + (raw_beta * (sb / w + shift_b)).abs()
                        + rf_period.abs())
            || !raw_beta.is_finite()
            || !raw_alpha.is_finite()
        {
            let estimate = fit(
                &returns[i - window..i],
                &benchmark[i - window..i],
                false,
                false,
            )?;
            if estimate.beta.is_nan() {
                (f64::NAN, f64::NAN)
            } else {
                (
                    stable_jensen(
                        &returns[i - window..i],
                        &benchmark[i - window..i],
                        &estimate,
                        ann_factor,
                        rf_period,
                    )?,
                    estimate.beta,
                )
            }
        } else {
            (raw_alpha, raw_beta)
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
        }
    }

    Ok(RollingGreeks {
        dates: out_dates,
        alphas,
        betas,
    })
}

/// Signed logarithm of a conditional annual geometric return. Large positive
/// annual log-growth is retained as a coefficient of A so equal explosive
/// growth can cancel before an unrequested component overflows.
struct GrowthLog {
    slope: f64,
    offset: f64,
    sign: f64,
}

fn growth_log(sum_log: f64, count: f64, terminal: bool, annualization: f64) -> GrowthLog {
    if terminal {
        return GrowthLog {
            slope: 0.0,
            offset: 0.0,
            sign: -1.0,
        };
    }
    let mean_log = sum_log / count;
    let annual_log = mean_log * annualization;
    if mean_log > 0.0 && mean_log > 40.0 / annualization {
        GrowthLog {
            slope: mean_log,
            offset: (-(-annual_log).exp()).ln_1p(),
            sign: 1.0,
        }
    } else if annual_log.abs() < 1e-8 {
        if sum_log == 0.0 {
            return GrowthLog {
                slope: 0.0,
                offset: f64::NEG_INFINITY,
                sign: 0.0,
            };
        }
        let correction = if annual_log == 0.0 {
            0.0
        } else {
            (annual_log.exp_m1() / annual_log).ln()
        };
        GrowthLog {
            slope: 0.0,
            offset: sum_log.abs().ln() + annualization.ln() - count.ln() + correction,
            sign: sum_log.signum(),
        }
    } else {
        let growth = annual_log.exp_m1();
        GrowthLog {
            slope: 0.0,
            offset: growth.abs().ln(),
            sign: if growth == 0.0 { 0.0 } else { growth.signum() },
        }
    }
}

fn capture_logs(
    returns: &[f64],
    benchmark: &[f64],
    ann_factor: f64,
    up: bool,
) -> Result<Option<(GrowthLog, GrowthLog)>> {
    validate_pairs(returns, benchmark)?;
    crate::returns::validate_frequency(ann_factor)?;
    crate::returns::validate_history(returns)?;
    crate::returns::validate_history(benchmark)?;
    let selected: Vec<(f64, f64)> = returns
        .iter()
        .copied()
        .zip(benchmark.iter().copied())
        .filter(|(_, b)| if up { *b > 0.0 } else { *b < 0.0 })
        .collect();
    if selected.is_empty() {
        return Ok(None);
    }
    let n = selected.len() as f64;
    let p_terminal = selected.iter().any(|(p, _)| *p <= -1.0);
    let b_terminal = selected.iter().any(|(_, b)| *b <= -1.0);
    let p_log = if p_terminal {
        0.0
    } else {
        neumaier_sum(selected.iter().map(|(p, _)| p.ln_1p()))
    };
    let b_log = if b_terminal {
        0.0
    } else {
        neumaier_sum(selected.iter().map(|(_, b)| b.ln_1p()))
    };
    Ok(Some((
        growth_log(p_log, n, p_terminal, ann_factor),
        growth_log(b_log, n, b_terminal, ann_factor),
    )))
}

fn log_quotient(numerator: &GrowthLog, denominator: &GrowthLog, ann_factor: f64) -> Result<f64> {
    if denominator.sign == 0.0 {
        return Err(calculation("strict-regime benchmark growth collapsed"));
    }
    if numerator.sign == 0.0 {
        return Ok(0.0);
    }
    let exponent = (numerator.slope - denominator.slope)
        .mul_add(ann_factor, numerator.offset - denominator.offset);
    let result = exponent.exp().copysign(numerator.sign * denominator.sign);
    if result == 0.0 {
        return Err(calculation("nonzero capture underflowed"));
    }
    finite(result)
}

/// Conditional annual geometric capture over strictly positive benchmark periods.
///
/// # Arguments
/// * `returns` - Valid investment history with at most one final exact wipeout.
/// * `benchmark` - Exactly paired valid investment history.
/// * `ann_factor` - Explicit finite positive observations per year.
pub(crate) fn up_capture(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> Result<f64> {
    match capture_logs(returns, benchmark, ann_factor, true)? {
        Some((p, b)) => log_quotient(&p, &b, ann_factor),
        None => Ok(f64::NAN),
    }
}

/// Conditional annual geometric capture over strictly negative benchmark periods.
///
/// # Arguments
/// * `returns` - Valid investment history with at most one final exact wipeout.
/// * `benchmark` - Exactly paired valid investment history.
/// * `ann_factor` - Explicit finite positive observations per year.
pub(crate) fn down_capture(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> Result<f64> {
    match capture_logs(returns, benchmark, ann_factor, false)? {
        Some((p, b)) => log_quotient(&p, &b, ann_factor),
        None => Ok(f64::NAN),
    }
}

/// Up capture divided by down capture, undefined when either regime is absent
/// or down-capture portfolio growth is exactly zero.
///
/// # Arguments
/// * `returns` - Valid investment history with at most one final exact wipeout.
/// * `benchmark` - Exactly paired valid investment history.
/// * `ann_factor` - Explicit finite positive observations per year.
pub(crate) fn capture_ratio(returns: &[f64], benchmark: &[f64], ann_factor: f64) -> Result<f64> {
    let up = capture_logs(returns, benchmark, ann_factor, true)?;
    let down = capture_logs(returns, benchmark, ann_factor, false)?;
    let (Some((up_p, up_b)), Some((down_p, down_b))) = (up, down) else {
        return Ok(f64::NAN);
    };
    if down_p.sign == 0.0 {
        return Ok(f64::NAN);
    }
    if up_p.sign == 0.0 {
        return Ok(0.0);
    }
    if up_b.sign == 0.0 || down_b.sign == 0.0 {
        return Err(calculation("strict-regime benchmark growth collapsed"));
    }
    let slope = neumaier_sum([up_p.slope, -up_b.slope, -down_p.slope, down_b.slope]);
    let offset = neumaier_sum([up_p.offset, -up_b.offset, -down_p.offset, down_b.offset]);
    let result = slope
        .mul_add(ann_factor, offset)
        .exp()
        .copysign(up_p.sign * up_b.sign * down_p.sign * down_b.sign);
    if result == 0.0 {
        return Err(calculation("nonzero combined capture underflowed"));
    }
    finite(result)
}

/// Strict outperformance fraction; ties count as nonwins and empty history is undefined.
///
/// # Arguments
/// * `returns` - Finite portfolio observations aligned by position.
/// * `benchmark` - Finite benchmark observations of exactly the same length.
pub(crate) fn batting_average(returns: &[f64], benchmark: &[f64]) -> Result<f64> {
    validate_pairs(returns, benchmark)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    Ok(returns.iter().zip(benchmark).filter(|(p, b)| p > b).count() as f64 / returns.len() as f64)
}

/// Absolute annual arithmetic M-squared; zero benchmark risk targets cash.
///
/// # Arguments
/// * `ann_return` - Arithmetic mean portfolio return multiplied by `ann_factor`.
/// * `ann_vol` - Portfolio sample volatility on the annualized frequency basis.
/// * `bench_vol` - Same-span benchmark sample volatility on the identical basis.
/// * `risk_free_rate` - Explicit annual effective decimal cash rate greater than -1.
/// * `ann_factor` - Explicit finite positive observations per year.
pub(crate) fn m_squared(
    ann_return: f64,
    ann_vol: f64,
    bench_vol: f64,
    risk_free_rate: f64,
    ann_factor: f64,
) -> Result<f64> {
    let cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)? * ann_factor;
    finite(cash)?;
    if ann_vol.is_nan() || bench_vol.is_nan() || ann_return.is_nan() || ann_vol == 0.0 {
        return Ok(f64::NAN);
    }
    if !ann_vol.is_finite() || !bench_vol.is_finite() || ann_vol < 0.0 || bench_vol < 0.0 {
        return Err(calculation("invalid sample risk"));
    }
    if bench_vol == 0.0 {
        return Ok(cash);
    }
    let adjustment = product_ratio(ann_return - cash, bench_vol, ann_vol)?;
    finite(cash + adjustment)
}

/// Treynor evaluated from paired raw returns so unrequested annual means need
/// not overflow before division by a large identified beta.
pub(crate) fn treynor_from_returns(
    returns: &[f64],
    benchmark: &[f64],
    risk_free_rate: f64,
    ann_factor: f64,
) -> Result<f64> {
    let cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    validate_pairs(returns, benchmark)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let pair = exact_pair(returns, benchmark)?;
    if pair.sxx.is_zero() {
        return Ok(f64::NAN);
    }
    let excess = &pair.mean_portfolio - rational(cash)?;
    if pair.sxy.is_zero() {
        return Ok(if excess.is_zero() {
            f64::NAN
        } else if excess.is_positive() {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        });
    }
    rational_output(&(excess * rational(ann_factor)? * &pair.sxx / &pair.sxy))
}

/// Absolute M-squared on paired raw returns, with requested-result rescaling
/// before potentially overflowing the unrequested annual mean or volatility.
pub(crate) fn m_squared_from_returns(
    returns: &[f64],
    benchmark: &[f64],
    risk_free_rate: f64,
    ann_factor: f64,
) -> Result<f64> {
    let period_cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    validate_pairs(returns, benchmark)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let p = centered(returns)?;
    let b = centered(benchmark)?;
    let cash = finite(period_cash * ann_factor)?;
    if p.scale == 0.0 {
        return Ok(f64::NAN);
    }
    if b.scale == 0.0 {
        return Ok(cash);
    }
    let n = (returns.len() - 1) as f64;
    let annual_mean = p.mean * ann_factor;
    let p_vol = p.scale * (p.sum_squares / n).sqrt() * ann_factor.sqrt();
    let b_vol = b.scale * (b.sum_squares / n).sqrt() * ann_factor.sqrt();
    if annual_mean.is_finite()
        && p_vol.is_finite()
        && b_vol.is_finite()
        && p_vol > 0.0
        && b_vol > 0.0
    {
        return m_squared(annual_mean, p_vol, b_vol, risk_free_rate, ann_factor);
    }
    let excess = finite(p.mean - period_cash)?;
    if excess == 0.0 {
        return Ok(cash);
    }
    let exponent = neumaier_sum([
        excess.abs().ln(),
        ann_factor.ln(),
        b.scale.ln(),
        -p.scale.ln(),
        0.5 * (b.sum_squares.ln() - p.sum_squares.ln()),
    ]);
    let adjustment = exponent.exp().copysign(excess);
    if adjustment == 0.0 {
        return Err(calculation("nonzero M-squared risk allocation underflowed"));
    }
    finite(cash + finite(adjustment)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_finite_field_lists_match_serde_sentinels() {
        use crate::test_support::assert_non_finite_fields;
        let beta = BetaResult {
            beta: f64::NAN,
            std_err: f64::NAN,
            ci_lower: f64::NAN,
            ci_upper: f64::NAN,
            n_obs: 0,
            confidence_level: BETA_CONFIDENCE_LEVEL,
        };
        assert_non_finite_fields(&beta);
        let greeks = GreeksResult {
            alpha: f64::NAN,
            beta: f64::NAN,
            r_squared: f64::NAN,
            adjusted_r_squared: f64::NAN,
            n_obs: 0,
        };
        assert_non_finite_fields(&greeks);
    }

    #[test]
    fn regression_results_report_the_aligned_sample_size() {
        let port = [0.020, 0.042, 0.058, 0.081, 0.099];
        let bench = [0.010, 0.020, 0.030, 0.040, 0.050];
        let fit = beta(&port, &bench).expect("fit");
        assert_eq!(fit.n_obs, 5);
        assert_eq!(fit.confidence_level, 0.95);
        let critical = student_t_inv_cdf(0.975, 3.0).expect("t quantile");
        assert!((fit.ci_lower - (fit.beta - critical * fit.std_err)).abs() < 1e-12);
        assert!((fit.ci_upper - (fit.beta + critical * fit.std_err)).abs() < 1e-12);
        let short = beta(&port[..2], &bench[..2]).expect("two pairs");
        assert_eq!(short.n_obs, 2);
        assert!(short.beta.is_finite());
        assert!(short.std_err.is_nan());
        assert_eq!(
            greeks(&port[..1], &bench[..1], 252.0, 0.0)
                .expect("one pair")
                .n_obs,
            1
        );
    }

    use super::*;
    fn treynor(annual_mean: f64, cash: f64, slope: f64, annualization: f64) -> Result<f64> {
        let mean = annual_mean / annualization;
        let b = [-0.01, 0.0, 0.01];
        let p = [mean - 0.01 * slope, mean, mean + 0.01 * slope];
        treynor_from_returns(&p, &b, cash, annualization)
    }

    use time::{Duration, Month};

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2025, Month::January, day).expect("valid date")
    }

    #[test]
    fn tracking_error_zero_when_identical() {
        let r = [0.01, 0.02, -0.01, 0.03];
        let te = tracking_error(&r, &r, 252.0).expect("valid benchmark request");
        assert!(te.abs() < 1e-12);
    }

    #[test]
    fn tracking_error_rejects_invalid_frequency() {
        for factor in [0.0, -1.0, f64::NAN] {
            assert!(tracking_error(&[0.01, 0.02], &[0.01, 0.01], factor).is_err());
        }
    }

    #[test]
    fn information_ratio_basic() {
        let r = [0.02, 0.03, 0.01, 0.04];
        let b = [0.01, 0.01, 0.01, 0.01];
        let ir = information_ratio(&r, &b, 252.0).expect("valid benchmark request");
        assert!(ir > 0.0);
    }

    #[test]
    fn information_ratio_rejects_invalid_frequency() {
        for factor in [0.0, f64::INFINITY] {
            assert!(information_ratio(&[0.01, 0.02], &[0.01, 0.01], factor).is_err());
        }
    }

    #[test]
    fn r_squared_perfect_correlation() {
        let r = [1.0, 2.0, 3.0, 4.0];
        let b = [2.0, 4.0, 6.0, 8.0];
        let r2 = r_squared(&r, &b).expect("valid benchmark request");
        assert!((r2 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn regression_statistics_are_nan_without_two_overlapping_observations() {
        for (returns, benchmark) in [(&[][..], &[][..]), (&[0.02][..], &[0.01][..])] {
            let beta_result = beta(returns, benchmark).expect("valid benchmark request");
            assert!(beta_result.beta.is_nan());
            assert!(beta_result.std_err.is_nan());
            assert!(beta_result.ci_lower.is_nan());
            assert!(beta_result.ci_upper.is_nan());

            assert!(
                beta(returns, benchmark)
                    .map(|fit| fit.beta)
                    .expect("valid benchmark request")
                    .is_nan()
            );
            assert!(
                r_squared(returns, benchmark)
                    .expect("valid benchmark request")
                    .is_nan()
            );

            let greeks_result =
                greeks(returns, benchmark, 252.0, 0.0).expect("valid benchmark request");
            assert!(greeks_result.alpha.is_nan());
            assert!(greeks_result.beta.is_nan());
            assert!(greeks_result.r_squared.is_nan());
            assert!(greeks_result.adjusted_r_squared.is_nan());
        }
    }

    #[test]
    fn degenerate_beta_and_greeks_json_round_trip_nan_fields() {
        let beta_result = beta(&[], &[]).expect("valid benchmark request");
        let beta_json = serde_json::to_string(&beta_result).expect("beta result should serialize");
        let beta_round_trip: BetaResult =
            serde_json::from_str(&beta_json).expect("beta result should deserialize");
        assert!(beta_round_trip.beta.is_nan());
        assert!(beta_round_trip.std_err.is_nan());
        assert!(beta_round_trip.ci_lower.is_nan());
        assert!(beta_round_trip.ci_upper.is_nan());

        let greeks_result = greeks(&[], &[], 252.0, 0.0).expect("valid benchmark request");
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
        let result = beta(&y, &x).expect("valid benchmark request");
        assert!((result.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn beta_point_estimate_supports_two_pairs_without_inference() {
        let returns = [0.03, 0.05];
        let benchmark = [0.01, 0.02];

        assert!(
            (beta(&returns, &benchmark)
                .map(|fit| fit.beta)
                .expect("valid benchmark request")
                - 2.0)
                .abs()
                < 1e-12
        );

        let result = beta(&returns, &benchmark).expect("valid benchmark request");
        assert!((result.beta - 2.0).abs() < 1e-12);
        assert!(result.std_err.is_nan());
        assert!(result.ci_lower.is_nan());
        assert!(result.ci_upper.is_nan());

        let greeks_result =
            greeks(&returns, &benchmark, 12.0, 0.0).expect("valid benchmark request");
        assert!((greeks_result.alpha - 0.12).abs() < 1e-12);
        assert!((greeks_result.beta - 2.0).abs() < 1e-12);
        assert!((greeks_result.r_squared - 1.0).abs() < 1e-12);
        assert!(greeks_result.adjusted_r_squared.is_nan());
    }

    #[test]
    fn batch_regression_rejects_mismatched_or_nonfinite_observations() {
        let returns = [0.01, 0.03, 0.05, 99.0];
        let benchmark = [0.0, 0.01, 0.02];
        assert!(beta(&returns, &benchmark).is_err());
        assert!(beta(&returns, &benchmark).map(|fit| fit.beta).is_err());
        assert!(r_squared(&returns, &benchmark).is_err());
        assert!(greeks(&returns, &benchmark, 12.0, 0.0).is_err());
        assert!(beta(&[0.0, f64::NAN], &[0.0, 1.0]).is_err());
    }

    #[test]
    fn beta_uses_t_critical_value_for_small_samples() {
        let y = [0.020, 0.041, 0.059, 0.082, 0.099];
        let x = [0.010, 0.020, 0.030, 0.040, 0.050];
        let result = beta(&y, &x).expect("valid benchmark request");
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
        let result = beta(&y, &x).expect("valid benchmark request");
        // n = 42 → df = 40: use the actual degrees of freedom, not a bucket.
        let expected_t_critical_df40 = 2.021_075_390_306_273_3_f64;
        let actual_half_width = result.ci_upper - result.beta;

        assert!(
            (actual_half_width - expected_t_critical_df40 * result.std_err).abs() < 1e-12,
            "beta CI should continue using Student-t beyond df=37"
        );
    }

    #[test]
    fn beta_intervals_cover_the_student_t_probability_at_bucket_boundaries() {
        use finstack_quant_core::math::special_functions::student_t_cdf;

        for df in [38, 59, 60, 119, 120, 239, 240, 1_000] {
            let x: Vec<f64> = (0..df + 2).map(|i| i as f64 * 0.001).collect();
            let y: Vec<f64> = x
                .iter()
                .enumerate()
                .map(|(i, &b)| 1.4 * b + if i % 2 == 0 { 0.002 } else { -0.0015 })
                .collect();
            let result = beta(&y, &x).expect("valid benchmark request");
            let critical = (result.ci_upper - result.beta) / result.std_err;
            let probability = student_t_cdf(critical, df as f64).expect("valid degrees of freedom");
            assert!(
                (probability - 0.975).abs() < 1e-9,
                "df={df}: upper confidence bound has CDF {probability}"
            );
        }
    }

    #[test]
    fn greeks_basic() {
        let r = [0.01, 0.02, 0.03, 0.04, 0.05];
        let b = [0.005, 0.01, 0.015, 0.02, 0.025];
        let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
        assert!((g.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn greeks_reports_adjusted_r_squared() {
        let r = [0.011, 0.018, 0.031, 0.039, 0.052, 0.061];
        let b = [0.005, 0.010, 0.015, 0.020, 0.025, 0.030];
        let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
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

        let zero_rf = greeks(&r, &b, ann_factor, 0.0).expect("valid benchmark request");
        let nonzero_rf =
            greeks(&r, &b, ann_factor, risk_free_rate).expect("valid benchmark request");
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
        let rg = rolling_greeks(&r, &b, &dates, 5, 252.0, 0.0).expect("valid benchmark request");
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

        let zero_rf =
            rolling_greeks(&r, &b, &dates, 6, ann_factor, 0.0).expect("valid benchmark request");
        let nonzero_rf = rolling_greeks(&r, &b, &dates, 6, ann_factor, risk_free_rate)
            .expect("valid benchmark request");
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
        let rg =
            rolling_greeks(&r, &b, &dates, window, 252.0, 0.0).expect("valid benchmark request");
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
    fn rolling_greeks_rejects_nonfinite_history_before_window_selection() {
        let mut portfolio = vec![0.01; 30];
        portfolio[7] = f64::NAN;
        let benchmark: Vec<f64> = (0..30).map(|i| i as f64 * 0.001).collect();
        let dates: Vec<Date> = (1..=30).map(jan).collect();
        assert!(rolling_greeks(&portfolio, &benchmark, &dates, 5, 252.0, 0.0).is_err());
    }

    #[test]
    fn batch_beta_functions_report_nan_for_zero_variance_benchmark() {
        let r: Vec<f64> = (0..10).map(|i| (i as f64 + 1.0) * 0.001).collect();
        let b = vec![0.5_f64; 10];

        assert!(
            beta(&r, &b)
                .map(|fit| fit.beta)
                .expect("valid benchmark request")
                .is_nan()
        );
        assert!(r_squared(&r, &b).expect("valid benchmark request").is_nan());

        let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
        assert!(g.beta.is_nan());
        assert!(g.alpha.is_nan());
        assert!(g.r_squared.is_nan());
        assert!(g.adjusted_r_squared.is_nan());

        let result = beta(&r, &b).expect("valid benchmark request");
        assert!(result.beta.is_nan());
        assert!(result.std_err.is_nan());
        assert!(result.ci_lower.is_nan());
        assert!(result.ci_upper.is_nan());

        assert!(treynor_from_returns(&r, &b, 0.02, 1.0).unwrap().is_nan());
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
                    let alpha = (mean_r - beta * mean_b) * ann_factor;
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

        let rolling = rolling_greeks(&r, &b, &dates, window, ann_factor, 0.0)
            .expect("valid benchmark request");
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

        // A departing portfolio-side outlier (while benchmark stays smooth)
        // must trigger an immediate rebuild rather than polluting beta/alpha
        // over the remaining recompute interval.
        let mut r_spike: Vec<f64> = b.iter().map(|&bi| 2.0 * (bi - 500_000.0) + 0.001).collect();
        let b_smooth: Vec<f64> = b.iter().map(|&bi| bi - 500_000.0).collect();
        r_spike[0] = 1.0e9;
        r_spike[3] = -1.0e9;
        let spike_rolling = rolling_greeks(&r_spike, &b_smooth, &dates, window, ann_factor, 0.0)
            .expect("valid benchmark request");
        let (expected_alphas, expected_spike_betas) =
            exact_rolling_greeks(&r_spike, &b_smooth, window, ann_factor);
        for (idx, ((&a, &ea), (&bt, &ebt))) in spike_rolling
            .alphas
            .iter()
            .zip(expected_alphas.iter())
            .zip(spike_rolling.betas.iter().zip(expected_spike_betas.iter()))
            .enumerate()
            .skip(4)
        {
            assert!(
                (bt - ebt).abs() < 1e-10,
                "beta mismatch at {idx}: {bt} vs {ebt}"
            );
            assert!(
                (a - ea).abs() < 1e-10,
                "alpha mismatch at {idx}: {a} vs {ea}"
            );
        }
    }

    #[test]
    fn up_capture_hand_calc() {
        // r = [0.10, −0.05], b = [0.05, −0.10]
        // Up periods (b≥0): index 0
        // port_prod = 1.10, bench_prod = 1.05
        // up_capture = (1.10−1) / (1.05−1) = 0.10/0.05 = 2.0
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
        assert!((uc - 2.0).abs() < 1e-12);
    }

    #[test]
    fn down_capture_hand_calc() {
        // Same data: down periods (b<0): index 1
        // port_prod = 0.95, bench_prod = 0.90
        // down_capture = (0.95−1) / (0.90−1) = −0.05/−0.10 = 0.5
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
        assert!((down - 0.5).abs() < 1e-12);
    }

    #[test]
    fn capture_ratio_hand_calc() {
        // up/down = 2.0/0.5 = 4.0
        let r = [0.10, -0.05];
        let b = [0.05, -0.10];
        let cr = capture_ratio(&r, &b, 1.0).expect("valid benchmark request");
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
        let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
        let expected = ((1.04_f64 * 1.06_f64).sqrt() - 1.0) / ((1.02_f64 * 1.03_f64).sqrt() - 1.0);
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_uses_empyrical_style_annualized_subset_return() {
        let r = vec![0.02; 36];
        let b = vec![0.015; 36];
        let uc = up_capture(&r, &b, 12.0).expect("valid benchmark request");
        let expected = ((1.02_f64).powf(12.0) - 1.0) / ((1.015_f64).powf(12.0) - 1.0);

        assert!((uc - expected).abs() < 1e-12);
        assert!((uc - 1.371_251_926_947_35).abs() < 1e-12);
    }

    #[test]
    fn up_capture_uses_geometric_subset_returns() {
        let r = [1.0, 0.0, -0.4];
        let b = [0.5, 0.5, -0.1];
        let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
        let expected_port = (2.0_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (1.5_f64 * 1.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((uc - expected).abs() < 1e-12);
    }

    #[test]
    fn down_capture_defensive_portfolio() {
        // Portfolio loses less than benchmark → down < 1.0 (desirable)
        let r = [0.04, -0.01, 0.06];
        let b = [0.02, -0.03, 0.03];
        let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
        // Down periods: index 1. port_prod=0.99, bench_prod=0.97
        let expected = (0.99 - 1.0) / (0.97 - 1.0);
        assert!((down - expected).abs() < 1e-12);
        assert!(down < 1.0);
    }

    #[test]
    fn down_capture_uses_geometric_subset_returns() {
        let r = [-0.25, 0.0, 0.1];
        let b = [-0.5, -0.5, 0.1];
        let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
        let expected_port = (0.75_f64 * 1.0_f64).sqrt() - 1.0;
        let expected_bench = (0.5_f64 * 0.5_f64).sqrt() - 1.0;
        let expected = expected_port / expected_bench;
        assert!((down - expected).abs() < 1e-12);
    }

    #[test]
    fn up_capture_no_up_periods() {
        assert!(
            up_capture(&[0.01, 0.02], &[-0.01, -0.02], 1.0)
                .unwrap()
                .is_nan()
        );
    }

    #[test]
    fn down_capture_no_down_periods() {
        assert!(
            down_capture(&[0.01, 0.02], &[0.01, 0.02], 1.0)
                .unwrap()
                .is_nan()
        );
    }

    #[test]
    fn capture_ratio_perfect_tracking() {
        // Portfolio = benchmark → up_capture=1, down_capture=1, ratio=1
        let r = [0.02, -0.03, 0.01, -0.01];
        let cr = capture_ratio(&r, &r, 1.0).expect("valid benchmark request");
        assert!((cr - 1.0).abs() < 1e-12);
    }

    #[test]
    fn batting_average_hand_calc() {
        // r = [0.02, 0.01, 0.03, -0.01], b = [0.01, 0.02, 0.01, 0.00]
        // Wins: r[0]>b[0] (0.02>0.01), r[2]>b[2] (0.03>0.01) → 2/4 = 0.5
        let r = [0.02, 0.01, 0.03, -0.01];
        let b = [0.01, 0.02, 0.01, 0.00];
        assert!((batting_average(&r, &b).expect("valid benchmark request") - 0.5).abs() < 1e-12);
    }

    #[test]
    fn batting_average_all_wins() {
        let r = [0.05, 0.03, 0.04];
        let b = [0.01, 0.01, 0.01];
        assert!((batting_average(&r, &b).expect("valid benchmark request") - 1.0).abs() < 1e-12);
    }

    #[test]
    fn batting_average_empty() {
        assert!(batting_average(&[], &[]).unwrap().is_nan());
    }

    #[test]
    fn treynor_hand_calc() {
        let t = treynor(0.10, 0.02, 1.2, 1.0).expect("valid benchmark request");
        assert!((t - 0.08 / 1.2).abs() < 1e-14);
    }

    #[test]
    fn treynor_zero_beta() {
        assert_eq!(treynor(0.1, 0.02, 0.0, 1.0).unwrap(), f64::INFINITY);
        assert_eq!(treynor(0.01, 0.02, 0.0, 1.0).unwrap(), f64::NEG_INFINITY);
        assert!(treynor(0.02, 0.02, 0.0, 1.0).unwrap().is_nan());
    }

    #[test]
    fn treynor_negative_beta() {
        let t = treynor(0.10, 0.02, -0.5, 1.0).expect("valid benchmark request");
        assert!((t - (0.08 / -0.5)).abs() < 1e-14);
    }

    #[test]
    fn m_squared_hand_calc() {
        let m2 = m_squared(0.12, 0.20, 0.15, 0.02, 1.0).expect("valid benchmark request");
        assert!((m2 - 0.095).abs() < 1e-12);
    }

    #[test]
    fn m_squared_zero_vol() {
        assert!(m_squared(0.1, 0.0, 0.15, 0.02, 1.0).unwrap().is_nan());
        assert!((m_squared(0.1, 0.2, 0.0, 0.02, 1.0).unwrap() - 0.02).abs() < 1e-14);
    }

    #[test]
    fn jensen_and_sharpe_agree_on_rf_when_beta_is_zero() {
        let mu = 0.0004_f64;
        let ann_factor = 252.0;
        let rf_annual = 0.02;
        let rf_period =
            crate::returns::periodic_risk_free_rate(rf_annual, ann_factor).expect("cash");
        let expected = (mu - rf_period) * ann_factor;
        let alpha = jensen_alpha(mu, 0.001, 0.0, ann_factor, rf_period);
        let sample_sd = 0.0002 * 2.0_f64.sqrt();
        let sharpe_excess =
            crate::risk_metrics::sharpe(&[mu - 0.0002, mu + 0.0002], rf_annual, ann_factor)
                .unwrap()
                * sample_sd
                * ann_factor.sqrt();
        assert!((alpha - expected).abs() < 1e-12);
        assert!((sharpe_excess - expected).abs() < 1e-12);
        assert!((alpha - sharpe_excess).abs() < 1e-12);
    }
    #[test]
    fn scaled_ols_and_rolling_keep_tiny_identifiable_variation() {
        let b = [0.0, 1e-200, 2e-200];
        let p = [0.0, 2e-200, 4e-200];
        let estimate = beta(&p, &b).expect("tiny fit");
        assert!((estimate.beta - 2.0).abs() < 1e-14);
        let rolling =
            rolling_greeks(&p, &b, &[jan(1), jan(2), jan(3)], 2, 12.0, 0.0).expect("rolling");
        assert!(rolling.betas.iter().all(|x| (*x - 2.0).abs() < 1e-14));
        assert!(rolling.alphas.iter().all(|x| x.abs() < 1e-212));
    }

    #[test]
    fn strict_capture_ignores_neutral_and_cancels_explosive_growth() {
        let p = [0.02, 0.75];
        let b = [0.01, 0.0];
        let capture = up_capture(&p, &b, 12.0).expect("strict capture");
        let expected = 1.02_f64.powi(12).mul_add(1.0, -1.0) / (1.01_f64.powi(12) - 1.0);
        assert!((capture - expected).abs() < 1e-12);
        let explosive = [100.0, 200.0];
        assert_eq!(
            up_capture(&explosive, &explosive, 1e300).expect("cancel equal growth"),
            1.0
        );
        assert!(
            capture_ratio(&[0.02, 0.0], &[0.01, -0.01], 12.0)
                .expect("zero down")
                .is_nan()
        );
    }

    #[test]
    fn sample_risk_and_zero_risk_are_distinct() {
        assert!(
            tracking_error(&[0.1], &[0.1], 12.0)
                .expect("singleton")
                .is_nan()
        );
        assert_eq!(
            tracking_error(&[0.1, 0.1], &[0.0, 0.0], 12.0).expect("constant"),
            0.0
        );
        assert!(
            information_ratio(&[0.0, 0.0], &[0.0, 0.0], 12.0)
                .expect("zero reward risk")
                .is_nan()
        );
        assert!(
            treynor_from_returns(&[1.0, 2.0, 3.0], &[1e-310, 2e-310, 3e-310], 0.0, 1.0)
                .expect("requested tiny quotient")
                > 0.0
        );
    }

    #[test]
    fn rolling_greeks_roundtrip_preserves_undefined_and_rejects_invalid_state() {
        let result = rolling_greeks(&[0.1, 0.2], &[0.1, 0.2], &[jan(1), jan(2)], 1, 12.0, 0.0)
            .expect("one point windows");
        let json = serde_json::to_value(&result).expect("wire");
        assert_eq!(json["alphas"][0], "nan");
        let restored: RollingGreeks = serde_json::from_value(json.clone()).expect("restore");
        assert!(restored.alphas.iter().all(|x| x.is_nan()));
        for key in ["alphas", "betas"] {
            let mut invalid = json.clone();
            invalid[key][0] = serde_json::json!(0.0);
            assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
        }
        let mut invalid = json.clone();
        invalid["alphas"] = serde_json::json!(["nan"]);
        assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
        let mut invalid = json;
        invalid["dates"] = serde_json::json!(["2025-01-02", "2025-01-01"]);
        assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
    }
    #[test]
    fn noisy_ols_matches_exact_rational_reference() {
        // Exact decimal rational arithmetic: beta=199/100, intercept=1/2000,
        // SSE=67/10^7, SST=9917/2500000 and SE(beta)^2=67/30000.
        let p = [0.020, 0.041, 0.059, 0.082, 0.099];
        let b = [0.010, 0.020, 0.030, 0.040, 0.050];
        let estimate = beta(&p, &b).expect("fit");
        assert!((estimate.beta - 1.99).abs() < 1e-14);
        assert!((estimate.std_err - (67.0_f64 / 30000.0).sqrt()).abs() < 1e-14);
        let point = greeks(&p, &b, 12.0, 0.0).expect("point fit");
        assert!((point.alpha - 0.006).abs() < 1e-14);
        assert!((point.r_squared - 39601.0 / 39668.0).abs() < 1e-14);
    }

    #[test]
    fn requested_r_squared_and_ratios_avoid_unrequested_overflows() {
        assert!(
            (r_squared(&[0.0, 1e300, 2e300], &[0.0, 1e-300, 2e-300]).expect("r squared") - 1.0)
                .abs()
                < 1e-14
        );
        let p = [0.0, 1e300, 2e300];
        let b = [0.0, 1.0, 2.0];
        assert!(
            (treynor_from_returns(&p, &b, 0.0, 1e100).expect("Treynor") / 1e100 - 1.0).abs()
                < 1e-12
        );
        assert_eq!(
            m_squared_from_returns(&p, &[0.0, 0.0, 0.0], 0.0, 1e100).expect("cash target"),
            0.0
        );
    }

    #[test]
    fn rolling_reconstructs_covariance_product_underflow() {
        let b = [0.0, 1e-100, 2e-100];
        let p = [0.0, 1e-250, 2e-250];
        let result =
            rolling_greeks(&p, &b, &[jan(1), jan(2), jan(3)], 3, 1.0, 0.0).expect("scaled rolling");
        assert!((result.betas[0] / 1e-150 - 1.0).abs() < 1e-12);
    }
    #[test]
    fn tiny_capture_ratios_do_not_require_representable_component_growth() {
        let tiny = f64::from_bits(1);
        assert_eq!(up_capture(&[tiny, tiny], &[tiny, tiny], 1.0).unwrap(), 1.0);
        assert_eq!(up_capture(&[1e-200], &[1e-200], 1e-200).unwrap(), 1.0);
        assert_eq!(
            up_capture(&[tiny, tiny, tiny], &[tiny, tiny, tiny], tiny).unwrap(),
            1.0
        );
    }

    #[test]
    fn exact_active_rebuild_retains_variation_beneath_large_differences() {
        let risk = tracking_error(&[1e308, 1e308], &[1.0, 2.0], 1.0).unwrap();
        assert!((risk - 0.5_f64.sqrt()).abs() < 1e-14);
    }

    #[test]
    fn exact_treynor_avoids_unrequested_unrepresentable_beta() {
        let ratio =
            treynor_from_returns(&[1.0, 2.0, 3.0], &[1e-310, 2e-310, 3e-310], 0.0, 1.0).unwrap();
        assert!((ratio / 2e-310 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn exact_slope_and_alpha_recover_cancelled_cross_scale_terms() {
        let p = [1e308, 0.0, 1.0];
        let b = [0.0, 1.0, -1.0];
        let result = fit(&p, &b, false, false).unwrap();
        assert_eq!(result.beta, -0.5);
        let point = greeks(&p, &b, 1.0, 0.0).unwrap_err();
        // The nonzero R squared is below binary64 range, an explicit failure.
        assert!(point.to_string().contains("range"));
    }
    #[test]
    fn exact_uncertainty_preserves_tiny_nonzero_residuals() {
        let p = [0.0, 1.0, 2.0, 1e-300];
        let b = [0.0, 1.0, 2.0, 0.0];
        let result = beta(&p, &b).unwrap();
        assert!((result.beta - 1.0).abs() < 1e-14);
        assert!(result.std_err > 0.0);
        assert!(result.std_err < 1e-299);
    }
}
