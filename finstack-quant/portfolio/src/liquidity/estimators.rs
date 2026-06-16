//! Spread and illiquidity estimators.
//!
//! Pure functions operating on return/volume slices. No state, no allocations
//! beyond the stack.
//!
//! # References
//!
//! - Roll, R. (1984). "A Simple Implicit Measure of the Effective Bid-Ask
//!   Spread in an Efficient Market." *Journal of Finance*, 39(4).
//! - Amihud, Y. (2002). "Illiquidity and Stock Returns: Cross-Section and
//!   Time-Series Effects." *Journal of Financial Markets*, 5(1).

/// Estimate the effective bid-ask spread from return serial covariance.
///
/// The Roll model assumes that observed returns are the sum of an
/// efficient-price innovation and a bid-ask bounce component. Under
/// this model:
///
/// ```text
/// effective_spread = 2 * sqrt(-Cov(r_t, r_{t-1}))
/// ```
///
/// When the serial covariance is positive (which violates the model
/// assumption), returns `None` rather than producing a complex number.
///
/// # Arguments
///
/// * `returns` - Slice of log or arithmetic returns, length >= 2.
///
/// # Returns
///
/// `Some(spread)` if the serial covariance is negative, `None` otherwise.
/// The spread is in the same units as the returns (relative to price).
///
/// # References
///
/// - Roll, R. (1984). `docs/REFERENCES.md#roll1984EffectiveSpread`
pub fn roll_effective_spread(returns: &[f64]) -> Option<f64> {
    if returns.len() < 2 {
        return None;
    }

    let n = returns.len();

    // Mean of all returns
    let mean: f64 = returns.iter().sum::<f64>() / n as f64;

    // Serial covariance: Cov(r_t, r_{t-1})
    // = (1/(n-1)) * sum_{t=1}^{n-1} (r_t - mean)(r_{t-1} - mean)
    let mut cov_sum = 0.0;
    for i in 1..n {
        cov_sum += (returns[i] - mean) * (returns[i - 1] - mean);
    }
    let serial_cov = cov_sum / (n - 1) as f64;

    if serial_cov >= 0.0 {
        // Positive serial covariance violates Roll model assumption
        return None;
    }

    Some(2.0 * (-serial_cov).sqrt())
}

/// Compute the Amihud illiquidity ratio from returns and volume.
///
/// ```text
/// ILLIQ = (1/T) * sum_{t=1}^{T} |r_t| / Volume_t
/// ```
///
/// Higher values indicate less liquid instruments (more price impact
/// per unit of volume). The ratio is typically averaged over a
/// rolling window (e.g., 20 or 60 trading days).
///
/// # Arguments
///
/// * `returns` - Slice of returns (absolute value taken internally).
/// * `volumes` - Slice of daily trading volumes (same length as `returns`).
///
/// # Returns
///
/// The average daily illiquidity ratio. Returns `None` if slices are
/// empty or mismatched in length, or if any volume entry is zero.
///
/// # References
///
/// - Amihud, Y. (2002). `docs/REFERENCES.md#amihud2002Illiquidity`
pub fn amihud_illiquidity(returns: &[f64], volumes: &[f64]) -> Option<f64> {
    if returns.is_empty() || returns.len() != volumes.len() {
        return None;
    }

    let mut sum = 0.0;
    for (r, v) in returns.iter().zip(volumes.iter()) {
        if *v <= 0.0 || !v.is_finite() {
            return None;
        }
        if !r.is_finite() {
            return None;
        }
        sum += r.abs() / v;
    }

    Some(sum / returns.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_with_negative_serial_covariance() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        // Synthetic series with bid-ask bounce: alternating +/- deviations
        // which produce negative serial covariance
        let returns = vec![0.01, -0.01, 0.01, -0.01, 0.01, -0.01, 0.01, -0.01];
        let s = roll_effective_spread(&returns)
            .ok_or_else(|| std::io::Error::other("expected negative serial covariance"))?;
        assert!(s > 0.0, "Roll spread should be positive");
        Ok(())
    }

    #[test]
    fn roll_positive_serial_covariance_returns_none() {
        // Trending returns => positive serial covariance
        let returns = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        assert!(roll_effective_spread(&returns).is_none());
    }

    #[test]
    fn roll_too_short_returns_none() {
        assert!(roll_effective_spread(&[]).is_none());
        assert!(roll_effective_spread(&[0.01]).is_none());
    }

    #[test]
    fn roll_known_value() -> std::result::Result<(), Box<dyn std::error::Error>> {
        // With a known bid-ask bounce of c, returns alternate as +c and -c.
        // Serial covariance = -c^2, so Roll spread = 2*c.
        let c = 0.005;
        let n = 1000;
        let mut returns = Vec::with_capacity(n);
        for i in 0..n {
            if i % 2 == 0 {
                returns.push(c);
            } else {
                returns.push(-c);
            }
        }
        let spread = roll_effective_spread(&returns)
            .ok_or_else(|| std::io::Error::other("expected Roll spread estimate"))?;
        // The formula gives 2 * sqrt(-cov). For large n, cov -> -c^2
        // so spread -> 2*c = 0.01
        assert!(
            (spread - 2.0 * c).abs() < 0.001,
            "expected ~{}, got {}",
            2.0 * c,
            spread
        );
        Ok(())
    }

    #[test]
    fn amihud_basic() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let returns = vec![0.01, -0.02, 0.005];
        let volumes = vec![1_000_000.0, 2_000_000.0, 500_000.0];
        let expected = (0.01 / 1_000_000.0 + 0.02 / 2_000_000.0 + 0.005 / 500_000.0) / 3.0;
        let r = amihud_illiquidity(&returns, &volumes)
            .ok_or_else(|| std::io::Error::other("expected Amihud illiquidity estimate"))?;
        assert!((r - expected).abs() < 1e-15, "expected {expected}, got {r}");
        Ok(())
    }

    #[test]
    fn amihud_mismatched_lengths() {
        assert!(amihud_illiquidity(&[0.01], &[100.0, 200.0]).is_none());
    }

    #[test]
    fn amihud_empty_input() {
        assert!(amihud_illiquidity(&[], &[]).is_none());
    }

    #[test]
    fn amihud_zero_volume() {
        assert!(amihud_illiquidity(&[0.01, 0.02], &[100.0, 0.0]).is_none());
    }

    #[test]
    fn amihud_nan_volume() {
        assert!(amihud_illiquidity(&[0.01], &[f64::NAN]).is_none());
    }

    #[test]
    fn amihud_nan_return() {
        assert!(amihud_illiquidity(&[f64::NAN], &[100.0]).is_none());
    }
}
