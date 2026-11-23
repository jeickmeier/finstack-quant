//! Up/down capture ratios, batting average, Treynor, and M-squared.

use super::regression::*;
use crate::exact::{exact_pair, rational, rational_output};
use finstack_quant_core::Result;
use finstack_quant_core::math::summation::neumaier_sum;
use num_traits::{Signed, Zero};

/// Signed logarithm of a conditional annual geometric return. Large positive
/// annual log-growth is retained as a coefficient of A so equal explosive
/// growth can cancel before an unrequested component overflows.
pub(super) struct GrowthLog {
    slope: f64,
    offset: f64,
    sign: f64,
}

pub(super) fn growth_log(
    sum_log: f64,
    count: f64,
    terminal: bool,
    annualization: f64,
) -> GrowthLog {
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

pub(super) fn capture_logs(
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
        neumaier_sum(selected.iter().map(|(p, _)| p.ln_1p()))?
    };
    let b_log = if b_terminal {
        0.0
    } else {
        neumaier_sum(selected.iter().map(|(_, b)| b.ln_1p()))?
    };
    Ok(Some((
        growth_log(p_log, n, p_terminal, ann_factor),
        growth_log(b_log, n, b_terminal, ann_factor),
    )))
}

pub(super) fn log_quotient(
    numerator: &GrowthLog,
    denominator: &GrowthLog,
    ann_factor: f64,
) -> Result<f64> {
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
    let slope = neumaier_sum([up_p.slope, -up_b.slope, -down_p.slope, down_b.slope])?;
    let offset = neumaier_sum([up_p.offset, -up_b.offset, -down_p.offset, down_b.offset])?;
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
        let adjustment = product_ratio(annual_mean - cash, b_vol, p_vol)?;
        return finite(cash + adjustment);
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
    ])?;
    let adjustment = exponent.exp().copysign(excess);
    if adjustment == 0.0 {
        return Err(calculation("nonzero M-squared risk allocation underflowed"));
    }
    finite(cash + finite(adjustment)?)
}
