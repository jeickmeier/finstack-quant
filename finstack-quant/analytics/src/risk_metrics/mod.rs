//! Return-based, tail-risk, and rolling analytics on simple return series.
//!
//! [`sharpe`], [`sortino`], [`volatility`], [`max_drawdown`], [`CagrDayCount`],
//! [`DatedSeries`], and [`RollingMetric`] are re-exported at the crate root;
//! the remaining functions are crate-internal building blocks for
//! [`crate::performance::Performance`].

mod return_based;
mod rolling;
mod tail_risk;

pub use return_based::{CagrDayCount, max_drawdown, sharpe, sortino, volatility};
pub(crate) use return_based::{
    cagr, downside_deviation, gain_to_pain, geometric_mean, mean_return, mean_sample_sd,
    modified_sharpe, omega_ratio, validate_cagr_context,
};
pub use rolling::{DatedSeries, RollingMetric};
pub(crate) use rolling::{rolling_returns, rolling_sharpe, rolling_sortino, rolling_volatility};
pub(crate) use tail_risk::{
    cornish_fisher_var, expected_shortfall, kurtosis, parametric_var, skewness, tail_ratio,
    value_at_risk, value_at_risk_and_es,
};

/// `numerator / denominator` with signed infinity when `denominator == 0.0`
/// and `NaN` when both are zero. Non-finite inputs return `NaN`.
#[inline]
pub(crate) fn ratio_or_sign_infinity(numerator: f64, denominator: f64) -> f64 {
    if !numerator.is_finite() || !denominator.is_finite() {
        return f64::NAN;
    }
    if denominator == 0.0 {
        if numerator > 0.0 {
            f64::INFINITY
        } else if numerator < 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::NAN
        }
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_or_sign_infinity_divides_and_signs_zero_denominators() {
        assert!((ratio_or_sign_infinity(0.15, 0.30) - 0.5).abs() < 1e-12);
        assert!((ratio_or_sign_infinity(-0.10, 0.30) - (-1.0 / 3.0)).abs() < 1e-12);
        assert_eq!(ratio_or_sign_infinity(0.15, 0.0), f64::INFINITY);
        assert_eq!(ratio_or_sign_infinity(-0.05, 0.0), f64::NEG_INFINITY);
        assert!(ratio_or_sign_infinity(0.0, 0.0).is_nan());
        assert!(ratio_or_sign_infinity(f64::NAN, 1.0).is_nan());
    }
}
