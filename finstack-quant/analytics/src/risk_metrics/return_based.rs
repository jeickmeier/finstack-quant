//! Validated return means, sample risk and explicitly selected reward ratios.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_core::{Error, Result};

use crate::returns::{
    finite_result, periodic_risk_free_rate, validate_cash, validate_frequency, validate_history,
};

/// Day-count convention applied to actual opening and closing valuations.
///
/// The default is Actual/365 Fixed. Explicit alternatives retain their core
/// calendar, coupon and termination context requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CagrDayCount {
    /// Actual elapsed calendar days divided by 365.25, selected explicitly.
    Act365_25,
    /// A core convention evaluated with the caller's complete context.
    DayCount(DayCount),
}

impl Default for CagrDayCount {
    fn default() -> Self {
        Self::DayCount(DayCount::Act365F)
    }
}

impl std::str::FromStr for CagrDayCount {
    type Err = Error;

    /// Parse an exact canonical CAGR clock label.
    ///
    /// # Arguments
    ///
    /// * `label` - `act365_25` or a canonical core day-count token, without aliases or padding.
    fn from_str(label: &str) -> Result<Self> {
        match label {
            "act365_25" => Ok(Self::Act365_25),
            other => finstack_quant_core::wire::serde_parse::<DayCount>(other).map(Self::DayCount),
        }
    }
}

impl std::fmt::Display for CagrDayCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Act365_25 => f.write_str("act365_25"),
            Self::DayCount(day_count) => write!(f, "{day_count}"),
        }
    }
}

/// Validate clock context even when a selected ticker has no observations.
///
/// Delegates to the core boundary: a core convention validates its own
/// requirements, and `act365_25`, which needs no context, still rejects
/// structurally invalid supplied fields through a context-free convention.
pub(crate) fn validate_cagr_context(
    day_count: CagrDayCount,
    context: DayCountContext<'_>,
) -> Result<()> {
    match day_count {
        CagrDayCount::Act365_25 => DayCount::Act365F.validate_context(context),
        CagrDayCount::DayCount(convention) => convention.validate_context(context),
    }
}

/// Annualize compounded log growth on an actual dated interval.
pub(crate) fn cagr(
    returns: &[f64],
    start: Date,
    end: Date,
    day_count: CagrDayCount,
    context: DayCountContext<'_>,
) -> Result<f64> {
    validate_cagr_context(day_count, context)?;
    validate_history(returns)?;
    if start >= end {
        return Err(Error::Validation(
            "CAGR opening must precede closing".into(),
        ));
    }
    let years = match day_count {
        CagrDayCount::Act365_25 => (end - start).whole_days() as f64 / 365.25,
        CagrDayCount::DayCount(convention) => convention.year_fraction(start, end, context)?,
    };
    if !years.is_finite() || years < 0.0 {
        return Err(Error::Internal(
            "invalid computed CAGR year fraction".into(),
        ));
    }
    if returns.is_empty() || years == 0.0 {
        return Ok(f64::NAN);
    }
    let mut engine = crate::returns::WealthEngine::new();
    for &r in returns {
        engine.step(r);
    }
    let log_growth = engine.log_growth();
    let result = finite_result((log_growth / years).exp_m1(), "CAGR")?;
    if result == 0.0 && log_growth != 0.0 {
        return Err(Error::Internal("nonzero CAGR underflowed".into()));
    }
    Ok(result)
}

/// Centered moments in scaled units; raw squares need never be representable.
struct ScaledMoments {
    scale: f64,
    sum: f64,
    count: f64,
    sd: f64,
}

impl ScaledMoments {
    fn new(values: &[f64]) -> Result<Self> {
        if values.iter().any(|x| !x.is_finite()) {
            return Err(Error::Validation(
                "statistical observations must be finite".into(),
            ));
        }
        let scale = values.iter().fold(0.0_f64, |s, x| s.max(x.abs()));
        let scale = if scale == 0.0 { 1.0 } else { scale };
        let Some(&origin) = values.first() else {
            return Ok(Self {
                scale,
                sum: f64::NAN,
                count: 0.0,
                sd: f64::NAN,
            });
        };
        let n = values.len() as f64;
        let offset = |x: f64| {
            let difference = x - origin;
            if difference.is_finite() {
                difference / scale
            } else {
                x / scale - origin / scale
            }
        };
        let mean_offset = neumaier_sum(values.iter().map(|&x| offset(x) / n));
        let sd = if values.len() < 2 {
            f64::NAN
        } else {
            (neumaier_sum(values.iter().map(|&x| {
                let d = offset(x) - mean_offset;
                d * d
            })) / (n - 1.0))
                .sqrt()
        };
        if values.len() >= 2
            && sd == 0.0
            && values
                .iter()
                .any(|&x| x.partial_cmp(&origin) != Some(core::cmp::Ordering::Equal))
        {
            return Err(Error::Internal(
                "positive sample dispersion was lost numerically".into(),
            ));
        }
        Ok(Self {
            scale,
            sum: neumaier_sum(values.iter().map(|&x| x / scale)),
            count: n,
            sd,
        })
    }

    fn mean(&self) -> Result<f64> {
        if self.sum.is_nan() {
            return Ok(f64::NAN);
        }
        scaled_product(self.scale, self.sum, 1.0 / self.count, "arithmetic mean")
    }
}

/// Stable mean and centered sample SD of arbitrary finite arithmetic observations.
pub(crate) fn mean_sample_sd(values: &[f64]) -> Result<(f64, f64)> {
    let moments = ScaledMoments::new(values)?;
    let sd = if values.len() < 2 {
        f64::NAN
    } else {
        scaled_product(moments.scale, moments.sd, 1.0, "sample standard deviation")?
    };
    Ok((moments.mean()?, sd))
}

/// Stable arithmetic mean, with linear annualization only when requested.
pub(crate) fn mean_return(returns: &[f64], annualize: bool, ann_factor: f64) -> Result<f64> {
    validate_history(returns)?;
    if annualize {
        validate_frequency(ann_factor)?;
    }
    let moments = ScaledMoments::new(returns)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    scaled_product(
        moments.scale,
        moments.sum,
        if annualize {
            ann_factor / moments.count
        } else {
            1.0 / moments.count
        },
        "arithmetic mean",
    )
}

/// Annualized centered sample standard deviation, using an n−1 denominator.
///
/// # Arguments
///
/// * `returns` - Finite chronological decimal investment returns greater than -1, allowing one final terminal -1; empty or singleton history is undefined.
/// * `periods_per_year` - Explicit positive finite observation count per year; sample SD is multiplied by its square root.
///
/// # Returns
///
/// Decimal annualized sample volatility; NaN when fewer than two observations exist.
///
/// # Errors
///
/// Returns an input error for invalid history/frequency and a calculation error
/// when a positive requested risk cannot be represented.
pub fn volatility(returns: &[f64], periods_per_year: f64) -> Result<f64> {
    validate_history(returns)?;
    validate_frequency(periods_per_year)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let moments = ScaledMoments::new(returns)?;
    scaled_product(
        moments.scale,
        moments.sd,
        periods_per_year.sqrt(),
        "volatility",
    )
}

/// Represent a mean difference without materializing a potentially overflowing sum.
fn scaled_mean_difference(values: &[f64], reference: f64) -> (f64, f64, f64) {
    let scale = values
        .iter()
        .map(|&x| (x - reference).abs())
        .fold(0.0_f64, f64::max);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        values
            .iter()
            .fold(reference.abs(), |s, x| s.max(x.abs()))
            .max(f64::MIN_POSITIVE)
    };
    let mean = neumaier_sum(values.iter().map(|&x| {
        let difference = x - reference;
        if difference.is_finite() {
            difference / scale
        } else {
            x / scale - reference / scale
        }
    }));
    (scale, mean, values.len() as f64)
}

fn scaled_product(scale: f64, value: f64, multiplier: f64, metric: &str) -> Result<f64> {
    if value == 0.0 {
        return Ok(0.0);
    }
    let first = value * multiplier;
    let second = scale * multiplier;
    let direct = if first.is_finite() && first.abs() >= f64::MIN_POSITIVE {
        scale * first
    } else if second.is_finite() && second >= f64::MIN_POSITIVE {
        value * second
    } else {
        f64::NAN
    };
    let output = if direct.is_finite() && direct != 0.0 {
        direct
    } else {
        (scale.ln() + value.abs().ln() + multiplier.ln())
            .exp()
            .copysign(value)
    };
    if output == 0.0 {
        return Err(Error::Internal(format!("{metric} underflowed")));
    }
    finite_result(output, metric)
}

fn scaled_ratio(ns: f64, n: f64, ds: f64, d: f64, multiplier: f64, metric: &str) -> Result<f64> {
    if d == 0.0 {
        return Ok(super::ratio_or_sign_infinity(n, 0.0));
    }
    if n == 0.0 {
        return Ok(0.0);
    }
    let reward = n * multiplier;
    let period_ratio = reward / d;
    let relative_scale = ns / ds;
    let direct = period_ratio * relative_scale;
    let value = if reward.is_finite()
        && reward.abs() >= f64::MIN_POSITIVE
        && period_ratio.is_finite()
        && period_ratio.abs() >= f64::MIN_POSITIVE
        && relative_scale.is_finite()
        && relative_scale >= f64::MIN_POSITIVE
        && direct.is_finite()
        && direct != 0.0
    {
        direct
    } else {
        (n.abs().ln() - d.ln() + ns.ln() - ds.ln() + multiplier.ln())
            .exp()
            .copysign(n)
    };
    if value == 0.0 {
        return Err(Error::Internal(format!("{metric} underflowed")));
    }
    finite_result(value, metric)
}

/// Annualized arithmetic excess return per unit of centered sample risk.
///
/// # Arguments
///
/// * `returns` - Chronological finite decimal investment returns, allowing a final terminal -1; at least two are needed for a ratio.
/// * `risk_free_rate` - Explicit finite annual-effective decimal cash return strictly greater than -1; converted geometrically to one observation period.
/// * `periods_per_year` - Explicit positive finite observations per year used for cash conversion and square-root scaling.
///
/// # Returns
///
/// Dimensionless Sharpe, NaN for insufficient history or zero-over-zero;
/// signed infinity for a genuine constant sample with nonzero excess reward.
///
/// # Errors
///
/// Invalid history, cash or frequency is rejected before sample availability;
/// unrepresentable finite calculations return a calculation error.
pub fn sharpe(returns: &[f64], risk_free_rate: f64, periods_per_year: f64) -> Result<f64> {
    validate_history(returns)?;
    let cash = periodic_risk_free_rate(risk_free_rate, periods_per_year)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let moments = ScaledMoments::new(returns)?;
    let (scale, excess, count) = scaled_mean_difference(returns, cash);
    scaled_ratio(
        scale,
        excess,
        moments.scale,
        moments.sd,
        periods_per_year.sqrt() / count,
        "Sharpe",
    )
}

/// Maximum loss from a running unit-wealth peak, including exact terminal zero.
///
/// # Arguments
///
/// * `returns` - Finite chronological decimal investment returns, with one -1 allowed only as the final observation.
///
/// # Returns
///
/// A nonpositive decimal depth, exactly -1 after a terminal loss; NaN for empty history.
///
/// # Errors
///
/// Rejects invalid financial histories and unresolved numerical drawdown paths.
pub fn max_drawdown(returns: &[f64]) -> Result<f64> {
    validate_history(returns)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    Ok(crate::drawdown::deepest(
        &crate::drawdown::to_drawdown_series(returns)?,
    ))
}

fn downside_scaled(returns: &[f64], mar: f64) -> Result<(f64, f64)> {
    if !mar.is_finite() {
        return Err(Error::Validation("MAR must be finite".into()));
    }
    let scale = returns
        .iter()
        .filter(|&&x| x < mar)
        .map(|&x| (mar - x).abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return Ok((1.0, 0.0));
    }
    let scale = if scale.is_finite() {
        scale
    } else {
        returns.iter().fold(mar.abs(), |s, x| s.max(x.abs()))
    };
    let ss = neumaier_sum(returns.iter().filter(|&&x| x < mar).map(|&x| {
        let difference = mar - x;
        let d = if difference.is_finite() {
            difference / scale
        } else {
            mar / scale - x / scale
        };
        d * d / returns.len() as f64
    }));
    let rms = ss.sqrt();
    if rms == 0.0 {
        return Err(Error::Internal(
            "positive downside risk was lost numerically".into(),
        ));
    }
    Ok((scale, rms))
}

/// All-observation shortfall RMS; explicit MAR is in observation-period units.
pub(crate) fn downside_deviation(
    returns: &[f64],
    mar: f64,
    annualize: bool,
    ann_factor: f64,
) -> Result<f64> {
    validate_history(returns)?;
    if annualize {
        validate_frequency(ann_factor)?;
    }
    let (scale, rms) = downside_scaled(returns, mar)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    scaled_product(
        scale,
        rms,
        if annualize { ann_factor.sqrt() } else { 1.0 },
        "downside deviation",
    )
}

/// Annualized mean reward above MAR per unit of full-sample shortfall RMS.
///
/// # Arguments
///
/// * `returns` - Chronological finite decimal investment returns, with one final -1 permitted; fewer than two observations produce an undefined ratio.
/// * `mar` - Explicit finite minimum acceptable return per observation, in decimal units; every finite real hurdle is allowed.
/// * `periods_per_year` - Positive finite observations per year used for square-root scaling.
///
/// # Returns
///
/// Dimensionless Sortino; NaN for insufficient history or all-at-MAR returns,
/// and positive infinity when reward is positive with genuinely no shortfall.
///
/// # Errors
///
/// Returns input errors for invalid history, MAR or frequency, and calculation
/// errors when positive finite risk or the requested finite ratio is unrepresentable.
pub fn sortino(returns: &[f64], mar: f64, periods_per_year: f64) -> Result<f64> {
    validate_history(returns)?;
    validate_frequency(periods_per_year)?;
    let (den_scale, denominator) = downside_scaled(returns, mar)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let (num_scale, numerator, count) = scaled_mean_difference(returns, mar);
    scaled_ratio(
        num_scale,
        numerator,
        den_scale,
        denominator,
        periods_per_year.sqrt() / count,
        "Sortino",
    )
}

/// Observation-period geometric mean on the validated investment domain.
pub(crate) fn geometric_mean(returns: &[f64]) -> Result<f64> {
    validate_history(returns)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    let mut engine = crate::returns::WealthEngine::new();
    for &r in returns {
        engine.step(r);
    }
    let log_growth = engine.log_growth();
    let value = finite_result(
        (log_growth / returns.len() as f64).exp_m1(),
        "geometric mean",
    )?;
    if value == 0.0 && log_growth != 0.0 {
        return Err(Error::Internal("nonzero geometric mean underflowed".into()));
    }
    if value <= -1.0 && returns.last().is_none_or(|&r| r > -1.0) {
        return Err(Error::Internal(
            "positive geometric growth collapsed to terminal zero".into(),
        ));
    }
    Ok(value)
}

/// Empirical unannualized gain/shortfall ratio above an explicit finite threshold.
pub(crate) fn omega_ratio(returns: &[f64], threshold: f64) -> Result<f64> {
    validate_history(returns)?;
    if !threshold.is_finite() {
        return Err(Error::Validation("Omega threshold must be finite".into()));
    }
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    let scale = returns
        .iter()
        .fold(threshold.abs(), |s, x| s.max(x.abs()))
        .max(f64::MIN_POSITIVE);
    let mut gains = NeumaierAccumulator::new();
    let mut losses = NeumaierAccumulator::new();
    for &r in returns {
        let d = r - threshold;
        let d = if d.is_finite() {
            d / scale
        } else {
            r / scale - threshold / scale
        };
        if r > threshold {
            gains.add(d);
        } else if r < threshold {
            losses.add(-d);
        }
    }
    let gains = gains.total();
    let losses = losses.total();
    if (gains == 0.0 && returns.iter().any(|&r| r > threshold))
        || (losses == 0.0 && returns.iter().any(|&r| r < threshold))
    {
        return Err(Error::Internal(
            "Omega gain or shortfall was lost numerically".into(),
        ));
    }
    if losses == 0.0 {
        return Ok(super::ratio_or_sign_infinity(gains, losses));
    }
    scaled_ratio(1.0, gains, 1.0, losses, 1.0, "Omega")
}

/// Arithmetic net-return sum per unit of negative-return magnitude, without annualization.
pub(crate) fn gain_to_pain(returns: &[f64]) -> Result<f64> {
    validate_history(returns)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    let scale = returns
        .iter()
        .fold(0.0_f64, |s, x| s.max(x.abs()))
        .max(f64::MIN_POSITIVE);
    let net = neumaier_sum(returns.iter().map(|r| r / scale));
    let losses = neumaier_sum(returns.iter().filter(|&&r| r < 0.0).map(|r| -r / scale));
    if losses == 0.0 {
        if returns.iter().any(|&r| r < 0.0) {
            return Err(Error::Internal(
                "positive loss magnitude was lost numerically".into(),
            ));
        }
        return Ok(super::ratio_or_sign_infinity(net, losses));
    }
    scaled_ratio(1.0, net, 1.0, losses, 1.0, "gain-to-pain")
}

/// Observation-period excess mean over the independently valid negative CF quantile.
pub(crate) fn modified_sharpe(
    returns: &[f64],
    risk_free_rate: f64,
    confidence: f64,
    ann_factor: f64,
) -> Result<f64> {
    validate_history(returns)?;
    validate_cash(risk_free_rate)?;
    let cash = periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    let cf = super::tail_risk::cornish_fisher_var(returns, confidence, None)?;
    if returns.len() < 4 || cf.is_nan() || cf >= 0.0 {
        return Ok(f64::NAN);
    }
    let (scale, numerator, count) = scaled_mean_difference(returns, cash);
    scaled_ratio(
        scale,
        numerator,
        cf.abs(),
        1.0,
        1.0 / count,
        "modified Sharpe",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::TenorUnit;
    use time::Month;

    #[test]
    fn request_errors_are_not_hidden_by_insufficient_samples() {
        assert!(volatility(&[], 0.0).is_err());
        assert!(sharpe(&[], f64::NAN, 12.0).is_err());
        assert!(sortino(&[], f64::INFINITY, 12.0).is_err());
        assert!(volatility(&[f64::NAN], 12.0).is_err());
        assert!(volatility(&[-1.0, 0.0], 12.0).is_err());
        assert!(volatility(&[0.1], 12.0).unwrap().is_nan());
        assert!(volatility(&[], 12.0).unwrap().is_nan());
    }

    #[test]
    fn independent_extreme_scale_reference_values() {
        let tiny = volatility(&[1e-200, -1e-200], 1.0).unwrap();
        assert!((tiny / 1e-200 - 2.0_f64.sqrt()).abs() < 1e-14);
        assert!((sharpe(&[0.0, 1e308], 0.0, 12.0).unwrap() - 6.0_f64.sqrt()).abs() < 1e-14);
        assert!(volatility(&[0.0, 1e308], 12.0).is_err());
        assert!(mean_return(&[0.0, 1e308], true, 12.0).is_err());
        assert_eq!(mean_return(&[1e308, 1e308], false, 1.0).unwrap(), 1e308);
        let smallest = f64::from_bits(1);
        let scaled_mean = mean_return(&[smallest, 0.0], true, 1e308).unwrap();
        assert!((scaled_mean / (smallest * 1e308) - 0.5).abs() < 1e-14);
    }

    #[test]
    fn signed_cancellation_preserves_a_resolved_nonzero_reward() {
        let returns = [1.0, 1e-20, -1.0];
        let expected_mean = 1e-20 / 3.0;
        assert!((mean_return(&returns, false, 1.0).unwrap() / expected_mean - 1.0).abs() < 1e-14);
        assert!((sharpe(&returns, 0.0, 1.0).unwrap() / expected_mean - 1.0).abs() < 1e-14);
        assert!((gain_to_pain(&returns).unwrap() / 1e-20 - 1.0).abs() < 1e-14);
    }

    #[test]
    fn cancelled_subnormal_reward_survives_requested_annual_scaling() {
        let smallest = f64::from_bits(1);
        let returns = [1.0, smallest, -1.0];
        assert!(mean_return(&returns, false, 1.0).is_err());
        let expected_mean = smallest * (1e308 / 3.0);
        let actual_mean = mean_return(&returns, true, 1e308).unwrap();
        assert!((actual_mean / expected_mean - 1.0).abs() < 1e-14);
        assert!(sharpe(&returns, 0.0, 1.0).is_err());
        let expected_sharpe = smallest * (1e154 / 3.0);
        let actual_sharpe = sharpe(&returns, 0.0, 1e308).unwrap();
        assert!((actual_sharpe / expected_sharpe - 1.0).abs() < 1e-14);
        let expected_sortino = smallest * (1e154 / 3.0_f64.sqrt());
        let actual_sortino = sortino(&returns, 0.0, 1e308).unwrap();
        assert!((actual_sortino / expected_sortino - 1.0).abs() < 1e-14);
    }

    #[test]
    fn accepted_zero_risk_and_unavailable_outcomes() {
        assert_eq!(volatility(&[0.1, 0.1], 12.0).unwrap(), 0.0);
        assert!(sharpe(&[0.0, 0.0], 0.0, 12.0).unwrap().is_nan());
        assert_eq!(sharpe(&[0.1, 0.1], 0.0, 12.0).unwrap(), f64::INFINITY);
        assert!(sortino(&[0.0, 0.0], 0.0, 12.0).unwrap().is_nan());
        assert!(omega_ratio(&[0.0, 0.0], 0.0).unwrap().is_nan());
        assert!(gain_to_pain(&[0.0, 0.0]).unwrap().is_nan());
        assert!(downside_deviation(&[], 0.0, true, 12.0).unwrap().is_nan());
    }

    #[test]
    fn downside_all_observation_denominator_and_any_finite_mar() {
        let returns = [0.02, 0.0, -0.01];
        assert!((downside_deviation(&returns, 0.0, true, 12.0).unwrap() - 0.02).abs() < 1e-15);
        assert!((sortino(&returns, 0.0, 12.0).unwrap() - 2.0).abs() < 1e-14);
        assert_eq!(downside_deviation(&returns, -2.0, true, 12.0).unwrap(), 0.0);
        assert_eq!(sortino(&returns, -2.0, 12.0).unwrap(), f64::INFINITY);
    }

    #[test]
    fn underflow_cannot_manufacture_zero_in_a_positive_ratio() {
        let smallest = f64::from_bits(1);
        assert!(omega_ratio(&[-0.75, -0.75, -0.75, smallest], 0.0).is_err());
    }

    #[test]
    fn empty_history_does_not_hide_invalid_coupon_context() {
        let context = DayCountContext {
            frequency: Some(finstack_quant_core::dates::Tenor::new(1, TenorUnit::Days).unwrap()),
            ..DayCountContext::default()
        };
        assert!(
            validate_cagr_context(CagrDayCount::DayCount(DayCount::ActActIsma), context).is_err()
        );
        assert!(geometric_mean(&[f64::from_bits(1), 0.0]).is_err());
    }

    #[test]
    fn dated_cagr_default_is_fixed_365_and_terminal_is_exact() {
        let start = Date::from_calendar_date(2024, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let c = cagr(
            &[0.1],
            start,
            end,
            CagrDayCount::default(),
            DayCountContext::default(),
        )
        .unwrap();
        assert!((c - 0.09971358593414137).abs() < 1e-14);
        assert_eq!(
            cagr(
                &[-1.0],
                start,
                end,
                CagrDayCount::default(),
                DayCountContext::default()
            )
            .unwrap(),
            -1.0
        );
        assert_eq!(geometric_mean(&[0.1, -1.0]).unwrap(), -1.0);
        assert!(
            cagr(
                &[],
                start,
                end,
                CagrDayCount::DayCount(DayCount::Bus252),
                DayCountContext::default()
            )
            .is_err()
        );
    }
}
