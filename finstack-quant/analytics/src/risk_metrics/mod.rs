//! Return-based, tail-risk, and rolling analytics on simple return series.
//!
//! [`sharpe`], [`sortino`], [`volatility`], [`max_drawdown`], [`CagrDayCount`],
//! [`DatedSeries`], and [`RollingMetric`] are re-exported at the crate root;
//! the remaining functions are crate-internal building blocks for
//! [`crate::performance::Performance`].

pub(crate) mod return_based;
pub(crate) mod rolling;
mod tail_risk;

pub(crate) use return_based::CagrDayCount;
#[cfg(test)]
pub(super) use return_based::sharpe;
pub(crate) use return_based::{
    cagr, downside_deviation, gain_to_pain, geometric_mean, mean_return, mean_sample_sd,
    modified_sharpe, omega_ratio, validate_cagr_context, zero_denom_ratio,
};
pub(crate) use rolling::DatedSeries;
pub(crate) use rolling::{rolling_returns, rolling_sharpe, rolling_sortino, rolling_volatility};
pub(crate) use tail_risk::{
    cornish_fisher_var, expected_shortfall, kurtosis, parametric_var, skewness, tail_ratio,
    value_at_risk, value_at_risk_and_es,
};

#[cfg(test)]
mod tests;
