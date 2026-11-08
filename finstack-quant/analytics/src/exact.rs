//! Exact binary-rational reconstruction for cancellation-sensitive statistics.
//!
//! Ordinary kernels remain scaled floating-point calculations. These helpers
//! retain the supplied binary64 observations exactly when reconstructing a
//! near-cancelled covariance or a requested quotient before final rounding.

use finstack_quant_core::{Error, Result};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};

pub(crate) struct ExactPair {
    pub mean_portfolio: BigRational,
    pub mean_benchmark: BigRational,
    pub sxx: BigRational,
    pub sxy: BigRational,
    pub syy: BigRational,
}

pub(crate) fn rational(value: f64) -> Result<BigRational> {
    BigRational::from_float(value)
        .ok_or_else(|| Error::Validation("exact statistical observations must be finite".into()))
}

pub(crate) fn exact_pair(portfolio: &[f64], benchmark: &[f64]) -> Result<ExactPair> {
    if portfolio.len() != benchmark.len() || portfolio.is_empty() {
        return Err(Error::Validation(
            "exact paired statistics require nonempty equal-length arrays".into(),
        ));
    }
    let portfolio: Vec<_> = portfolio
        .iter()
        .map(|&value| rational(value))
        .collect::<Result<_>>()?;
    let benchmark: Vec<_> = benchmark
        .iter()
        .map(|&value| rational(value))
        .collect::<Result<_>>()?;
    let count = BigRational::from_integer(portfolio.len().into());
    let mean_portfolio = portfolio.iter().cloned().sum::<BigRational>() / &count;
    let mean_benchmark = benchmark.iter().cloned().sum::<BigRational>() / count;
    let mut sxx = BigRational::zero();
    let mut sxy = BigRational::zero();
    let mut syy = BigRational::zero();
    for (p, b) in portfolio.iter().zip(&benchmark) {
        let x = b - &mean_benchmark;
        let y = p - &mean_portfolio;
        sxx += &x * &x;
        sxy += &x * &y;
        syy += &y * &y;
    }
    Ok(ExactPair {
        mean_portfolio,
        mean_benchmark,
        sxx,
        sxy,
        syy,
    })
}

pub(crate) fn rational_output(value: &BigRational) -> Result<f64> {
    let output = value.to_f64().ok_or_else(|| {
        Error::Internal("exact statistical result could not be reconstructed".into())
    })?;
    if !output.is_finite() || (output.abs() <= 0.0 && !value.is_zero()) {
        return Err(Error::Internal(
            "requested statistical result is outside the nonzero finite binary64 range".into(),
        ));
    }
    Ok(output)
}

pub(crate) fn rational_sqrt(value: &BigRational) -> Result<f64> {
    if value.is_negative() {
        return Err(Error::Internal(
            "negative reconstructed statistical square".into(),
        ));
    }
    if value.is_zero() {
        return Ok(0.0);
    }
    let exponent = value.numer().bits() as i32 - value.denom().bits() as i32;
    let half_exponent = exponent.div_euclid(2);
    let power = BigRational::from_integer(2.into()).pow(half_exponent * 2);
    let normalized = rational_output(&(value / power))?;
    let root =
        rational(normalized.sqrt())? * BigRational::from_integer(2.into()).pow(half_exponent);
    rational_output(&root)
}

pub(crate) fn pearson(portfolio: &[f64], benchmark: &[f64]) -> Result<f64> {
    let pair = exact_pair(portfolio, benchmark)?;
    if pair.sxx.is_zero() || pair.syy.is_zero() {
        return Err(Error::Validation(
            "correlation is undefined for a constant common-sample column".into(),
        ));
    }
    if pair.sxy.is_zero() {
        return Ok(0.0);
    }
    let square = (&pair.sxy * &pair.sxy) / (&pair.sxx * &pair.syy);
    let coefficient = rational_sqrt(&square)?;
    Ok(if pair.sxy.is_negative() {
        -coefficient
    } else {
        coefficient
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covariance_cancellation_keeps_a_representable_subnormal_correlation() {
        let rho = pearson(&[1e308, 0.0, 1.0], &[0.0, 1.0, -1.0]).unwrap();
        // Sxy=-1 and sqrt(Sxx*Syy) is asymptotically 2e308/sqrt(3).
        let expected = -(3.0_f64.sqrt() / 2.0) / 1e308;
        assert!(rho < 0.0);
        assert!((rho / expected - 1.0).abs() < 1e-14);
    }
}
