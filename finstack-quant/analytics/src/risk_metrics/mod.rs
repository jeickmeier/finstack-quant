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

/// `numerator / 0.0` with signed infinity for nonzero finite numerators and
/// `NaN` for zero or non-finite numerators.
#[inline]
pub(crate) fn zero_denom_ratio(numerator: f64) -> f64 {
    if !numerator.is_finite() || numerator == 0.0 {
        f64::NAN
    } else if numerator > 0.0 {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_denom_ratio_signs_zero_denominators() {
        assert_eq!(zero_denom_ratio(0.15), f64::INFINITY);
        assert_eq!(zero_denom_ratio(-0.05), f64::NEG_INFINITY);
        assert!(zero_denom_ratio(0.0).is_nan());
        assert!(zero_denom_ratio(f64::NAN).is_nan());
        assert!(zero_denom_ratio(f64::INFINITY).is_nan());
    }
}
