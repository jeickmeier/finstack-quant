//! Benchmark OLS regression, tracking error, information ratio, and rolling Greeks.

use crate::exact::{exact_pair, rational, rational_output, rational_sqrt};
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::special_functions::student_t_inv_cdf;
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_core::wire::NonFiniteFields;
use finstack_quant_core::{Error, Result};
use num_traits::{Signed, ToPrimitive, Zero};

pub(super) const ROLLING_GREEKS_RECOMPUTE_INTERVAL: usize = 64;

pub(super) fn calculation(message: &str) -> Error {
    Error::Internal(format!("benchmark calculation: {message}"))
}

pub(super) fn validate_pairs(portfolio: &[f64], benchmark: &[f64]) -> Result<()> {
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

pub(super) fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(calculation("requested result is not representable"))
    }
}

// Keep ordinary arithmetic exact where possible; a logarithmic rescaling is a
// recovery path for the same product/quotient, never an estimator substitute.
pub(super) fn product_ratio(a: f64, b: f64, c: f64) -> Result<f64> {
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

pub(super) struct Centered {
    pub(super) mean: f64,
    pub(super) scale: f64,
    pub(super) values: Vec<f64>,
    pub(super) sum_squares: f64,
}

pub(super) fn centered(values: &[f64]) -> Result<Centered> {
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
    let mean_normalized = neumaier_sum(normalized.iter().map(|x| x / values.len() as f64))?;
    let mean = if use_shift {
        origin + scale * mean_normalized
    } else {
        scale * mean_normalized
    };
    let mean = if mean.is_finite() {
        mean
    } else {
        finite(neumaier_sum(
            values.iter().map(|x| x / values.len() as f64),
        )?)?
    };
    let deviations: Vec<f64> = normalized.iter().map(|x| x - mean_normalized).collect();
    let sum_squares = neumaier_sum(deviations.iter().map(|x| x * x))?;
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

pub(super) fn centered_r_squared(p: &Centered, b: &Centered, cross: f64) -> Result<f64> {
    if p.scale == 0.0 || b.scale == 0.0 {
        return Ok(f64::NAN);
    }
    let correlation = (cross / b.sum_squares.sqrt()) / p.sum_squares.sqrt();
    let result = correlation * correlation;
    if !result.is_finite() || result > 1.0 + 32.0 * f64::EPSILON || (result == 0.0 && cross != 0.0)
    {
        return Err(calculation("invalid squared correlation"));
    }
    Ok(result.min(1.0))
}

pub(super) struct Fit {
    pub(super) beta: f64,
    pub(super) mean_portfolio: f64,
    pub(super) mean_benchmark: f64,
    pub(super) r_squared: f64,
    pub(super) se: f64,
}

pub(super) fn fit(
    portfolio: &[f64],
    benchmark: &[f64],
    uncertainty: bool,
    fit_stats: bool,
) -> Result<Fit> {
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
    let cross = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| x * y))?;
    let cross_size = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| (x * y).abs()))?;
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
        }))?
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
        centered_r_squared(&p, &b, cross)?
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

pub(super) fn exact_fit(
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

pub(super) fn exact_jensen(
    portfolio: &[f64],
    benchmark: &[f64],
    ann_factor: f64,
    cash: f64,
) -> Result<f64> {
    let pair = exact_pair(portfolio, benchmark)?;
    let q = rational(cash)?;
    let intercept =
        (&pair.mean_portfolio - &q) - (&pair.sxy / &pair.sxx) * (&pair.mean_benchmark - &q);
    rational_output(&(rational(ann_factor)? * intercept))
}

pub(super) fn stable_jensen(
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

pub(super) fn exact_active(
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
    let cross = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| x * y))?;
    let size = neumaier_sum(p.values.iter().zip(&b.values).map(|(x, y)| (x * y).abs()))?;
    if cross.abs() <= f64::EPSILON.sqrt() * size {
        let pair = exact_pair(returns, benchmark)?;
        return rational_output(&((&pair.sxy * &pair.sxy) / (&pair.sxx * &pair.syy)));
    }
    centered_r_squared(&p, &b, cross)
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
pub(super) const BETA_CONFIDENCE_LEVEL: f64 = 0.95;

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

pub(super) fn jensen_alpha(
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

pub(super) fn recompute_rolling_greeks_sums(
    returns: &[f64],
    benchmark: &[f64],
    shift_r: f64,
    shift_b: f64,
) -> Option<(f64, f64, f64, f64)> {
    Some((
        neumaier_sum(returns.iter().map(|r| r - shift_r)).ok()?,
        neumaier_sum(benchmark.iter().map(|b| b - shift_b)).ok()?,
        neumaier_sum(
            returns
                .iter()
                .zip(benchmark)
                .map(|(r, b)| (r - shift_r) * (b - shift_b)),
        )
        .ok()?,
        neumaier_sum(benchmark.iter().map(|b| (b - shift_b) * (b - shift_b))).ok()?,
    ))
}

pub(super) fn seeded_accumulator(value: f64) -> Option<NeumaierAccumulator> {
    let mut sum = NeumaierAccumulator::new();
    sum.add(value).ok()?;
    Some(sum)
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
    let mut accs =
        recompute_rolling_greeks_sums(&returns[..window], &benchmark[..window], shift_r, shift_b)
            .and_then(|(sr, sb, srb, sb2)| {
                Some((
                    seeded_accumulator(sr)?,
                    seeded_accumulator(sb)?,
                    seeded_accumulator(srb)?,
                    seeded_accumulator(sb2)?,
                ))
            });
    let mut steps_since_recompute = 0usize;
    let mut variance_scale = 0.0_f64;
    let mut cov_scale = 0.0_f64;
    let mut r_scale = 0.0_f64;
    let mut b_scale = 0.0_f64;

    for i in window..=n {
        // Shifted sums: correct for beta (shift-invariant) as they stand.
        let (mut sr, mut sb, mut srb, mut sb2) = accs
            .as_ref()
            .and_then(|(a, b, c, d)| {
                Some((
                    a.get_total().ok()?,
                    b.get_total().ok()?,
                    c.get_total().ok()?,
                    d.get_total().ok()?,
                ))
            })
            .unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
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
            let rebuilt = recompute_rolling_greeks_sums(
                &returns[start..i],
                &benchmark[start..i],
                shift_r,
                shift_b,
            );
            (sr, sb, srb, sb2) = rebuilt.unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
            accs = rebuilt.and_then(|(r, b, rb, b2)| {
                Some((
                    seeded_accumulator(r)?,
                    seeded_accumulator(b)?,
                    seeded_accumulator(rb)?,
                    seeded_accumulator(b2)?,
                ))
            });
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
            let step_ok = if let Some((acc_sr, acc_sb, acc_srb, acc_sb2)) = accs.as_mut() {
                acc_sr.add(new_r - old_r).is_ok()
                    && acc_sb.add(new_b - old_b).is_ok()
                    && acc_srb.add(new_r * new_b - old_r * old_b).is_ok()
                    && acc_sb2.add(new_b * new_b - old_b * old_b).is_ok()
            } else {
                false
            };
            if !step_ok {
                accs = None;
            }
            steps_since_recompute += 1;
        }
    }

    Ok(RollingGreeks {
        dates: out_dates,
        alphas,
        betas,
    })
}
