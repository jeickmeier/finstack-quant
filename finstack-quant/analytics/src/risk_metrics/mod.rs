//! Return-based, tail-risk, and rolling analytics on simple return series.
//!
//! [`sharpe`], [`sortino`], [`volatility`], [`max_drawdown`], [`CagrDayCount`],
//! [`DatedSeries`], and [`RollingMetric`] are re-exported at the crate root;
//! the remaining functions are crate-internal building blocks for
//! [`crate::performance::Performance`].

mod return_based;
mod rolling;
mod tail_risk;

pub(crate) use return_based::{
    cagr, downside_deviation, gain_to_pain, geometric_mean, invalid_annualization_factor,
    mean_return, mean_vol_annualized, modified_sharpe, omega_ratio, volatility_with_annualize,
};
pub use return_based::{max_drawdown, sharpe, sortino, volatility, CagrDayCount};
pub(crate) use rolling::{rolling_returns, rolling_sharpe, rolling_sortino, rolling_volatility};
pub use rolling::{DatedSeries, RollingMetric};
pub(crate) use tail_risk::{
    cornish_fisher_var, expected_shortfall, kurtosis, parametric_var, skew_kurt, skewness,
    tail_ratio, value_at_risk, value_at_risk_and_es,
};

/// `numerator / denominator` with signed infinity when `denominator == 0.0`
/// and `0.0` when both are zero. Non-finite inputs return `NaN`.
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
            0.0
        }
    } else {
        numerator / denominator
    }
}

/// Annualized ratio `(period_excess * ann_factor) / (period_risk * sqrt(ann_factor))`
/// that checks finiteness and zero-risk sentinels on the unscaled per-period
/// statistics before multiplying by `ann_factor`.
#[inline]
pub(crate) fn annualized_ratio_or_sign_infinity(
    period_excess: f64,
    period_risk: f64,
    ann_factor: f64,
) -> f64 {
    if !period_excess.is_finite() || !period_risk.is_finite() {
        return f64::NAN;
    }
    if period_risk == 0.0 {
        ratio_or_sign_infinity(period_excess, 0.0)
    } else {
        (period_excess * ann_factor) / (period_risk * ann_factor.sqrt())
    }
}
