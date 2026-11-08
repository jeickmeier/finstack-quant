//! Validated total-investment returns, annual-effective cash and log wealth.

use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::{Error, Result};

/// Check chronological nonnegative-wealth investment returns, including a final wipeout.
pub(crate) fn validate_history(returns: &[f64]) -> Result<()> {
    for (i, &r) in returns.iter().enumerate() {
        if !r.is_finite() || r < -1.0 || (r <= -1.0 && i + 1 != returns.len()) {
            return Err(Error::Validation(format!(
                "return {i} must be finite and greater than -1, except a final terminal -1"
            )));
        }
    }
    Ok(())
}

/// Check an explicitly supplied observations-per-year convention.
pub(crate) fn validate_frequency(frequency: f64) -> Result<()> {
    if !frequency.is_finite() || frequency <= 0.0 {
        return Err(Error::Validation(
            "observation frequency must be positive and finite".into(),
        ));
    }
    Ok(())
}

/// Check an explicitly supplied annual-effective decimal cash rate.
pub(crate) fn validate_cash(cash: f64) -> Result<()> {
    if !cash.is_finite() || cash <= -1.0 {
        return Err(Error::Validation(
            "annual-effective cash must be finite and greater than -1".into(),
        ));
    }
    Ok(())
}

/// Reject an unrepresentable required statistic rather than manufacture a sentinel.
pub(crate) fn finite_result(value: f64, metric: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Internal(format!(
            "{metric} calculation is not representable"
        )))
    }
}

/// Derive one return per adjacent price interval, preserving a final zero price.
pub(crate) fn pairwise_returns(prices: &[f64]) -> Result<Vec<f64>> {
    if prices
        .iter()
        .enumerate()
        .any(|(i, &p)| !p.is_finite() || p < 0.0 || (p == 0.0 && i + 1 != prices.len()))
    {
        return Err(Error::Validation(
            "prices must be positive and finite, with only a final zero allowed".into(),
        ));
    }
    prices
        .windows(2)
        .map(|w| {
            if w[1] == 0.0 {
                return Ok(-1.0);
            }
            let r = finite_result((w[1] - w[0]) / w[0], "price return")?;
            if r <= -1.0 {
                return Err(Error::Internal(
                    "positive price ratio collapsed to a terminal return".into(),
                ));
            }
            Ok(r)
        })
        .collect()
}

/// Convert annual-effective decimal cash to one declared observation period.
pub(crate) fn periodic_risk_free_rate(risk_free_rate: f64, ann_factor: f64) -> Result<f64> {
    validate_cash(risk_free_rate)?;
    validate_frequency(ann_factor)?;
    if ann_factor.total_cmp(&1.0).is_eq() {
        return Ok(risk_free_rate);
    }
    let value = finite_result(
        (risk_free_rate.ln_1p() / ann_factor).exp_m1(),
        "period cash",
    )?;
    if value <= -1.0 || (value == 0.0 && risk_free_rate != 0.0) {
        return Err(Error::Internal(
            "period cash lost its positive-growth or nonzero identity".into(),
        ));
    }
    Ok(value)
}

/// Arithmetic investment-minus-cash observations on an exactly aligned grid.
pub(crate) fn excess_returns(returns: &[f64], rf: &[f64], nperiods: f64) -> Result<Vec<f64>> {
    validate_history(returns)?;
    validate_frequency(nperiods)?;
    if returns.len() != rf.len() {
        return Err(Error::Validation(
            "cash and investment histories must have identical lengths".into(),
        ));
    }
    rf.iter().try_for_each(|&cash| validate_cash(cash))?;
    returns
        .iter()
        .zip(rf)
        .map(|(&r, &cash)| {
            finite_result(
                r - periodic_risk_free_rate(cash, nperiods)?,
                "arithmetic excess return",
            )
        })
        .collect()
}

/// Compensated positive log wealth with a separate, exact terminal-zero state.
pub(crate) struct WealthEngine {
    acc: NeumaierAccumulator,
    invalid: bool,
    terminal: bool,
}

impl WealthEngine {
    pub(crate) fn new() -> Self {
        Self {
            acc: NeumaierAccumulator::new(),
            invalid: false,
            terminal: false,
        }
    }

    /// Apply a simple return; continuation after a terminal event is invalid.
    pub(crate) fn step(&mut self, r: f64) {
        if self.invalid || self.terminal || !r.is_finite() || r < -1.0 {
            self.invalid = true;
        } else if r <= -1.0 {
            self.terminal = true;
        } else {
            self.acc.add(r.ln_1p());
        }
    }

    /// Positive log wealth, negative infinity for explicit zero, or NaN for invalid history.
    pub(crate) fn log_growth(&self) -> f64 {
        if self.invalid {
            f64::NAN
        } else if self.terminal {
            f64::NEG_INFINITY
        } else {
            self.acc.total()
        }
    }
}

/// One cumulative return per selected return endpoint, without a baseline row.
pub(crate) fn comp_sum(returns: &[f64]) -> Result<Vec<f64>> {
    validate_history(returns)?;
    let mut engine = WealthEngine::new();
    returns
        .iter()
        .map(|&r| {
            engine.step(r);
            finite_result(engine.log_growth().exp_m1(), "cumulative return")
        })
        .collect()
}

/// Final compound return; empty observed histories are undefined.
pub(crate) fn comp_total(returns: &[f64]) -> Result<f64> {
    validate_history(returns)?;
    if returns.is_empty() {
        return Ok(f64::NAN);
    }
    let mut engine = WealthEngine::new();
    for &r in returns {
        engine.step(r);
    }
    finite_result(engine.log_growth().exp_m1(), "total return")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_loss_is_exact_and_cannot_continue() {
        assert_eq!(comp_total(&[0.05, -1.0]).unwrap(), -1.0);
        assert_eq!(comp_sum(&[0.05, -1.0]).unwrap()[1], -1.0);
        assert!(comp_total(&[-1.0, 0.0]).is_err());
        assert!(comp_total(&[-1.01]).is_err());
        assert!(comp_total(&[f64::NAN]).is_err());
    }

    #[test]
    fn cash_validation_precedes_empty_shortcuts() {
        assert!(excess_returns(&[], &[], 0.0).is_err());
        assert!(excess_returns(&[0.1], &[-1.0], 12.0).is_err());
        assert!(excess_returns(&[0.1], &[], 12.0).is_err());
        assert_eq!(periodic_risk_free_rate(0.05, 1.0).unwrap(), 0.05);
        assert!(
            (periodic_risk_free_rate(0.05, 12.0).unwrap() - 0.0040741237836483535).abs() < 1e-16
        );
    }

    #[test]
    fn final_only_growth_does_not_require_finite_prefix_wealth() {
        let r = [1e200, 1e200, -0.9999999999999999];
        assert!(comp_sum(&r).is_err());
        assert!(comp_total(&[1e200, -0.5]).unwrap().is_finite());
        assert!(comp_total(&[]).unwrap().is_nan());
    }

    #[test]
    fn price_returns_preserve_terminal_and_small_moves() {
        assert_eq!(pairwise_returns(&[100.0, 0.0]).unwrap(), vec![-1.0]);
        assert!(pairwise_returns(&[100.0, 0.0, 1.0]).is_err());
        assert!(pairwise_returns(&[1e308, 1e-308]).is_err());
        assert!(pairwise_returns(&[0.0]).unwrap().is_empty());
    }
}
