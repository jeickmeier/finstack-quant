//! Return-based, tail-risk, and rolling analytics on simple return series.
//!
//! The metric functions in this module are crate-internal building blocks for
//! [`crate::performance::Performance`]. [`DatedSeries`] and [`RollingMetric`] remain
//! public because rolling `Performance` methods return them.

mod return_based;
mod rolling;
mod tail_risk;

pub(crate) use return_based::invalid_annualization_factor;
pub use return_based::CagrDayCount;
pub(crate) use return_based::{
    cagr, downside_deviation, gain_to_pain, geometric_mean, mean_return, mean_vol_annualized,
    modified_sharpe, omega_ratio, sharpe, sortino, volatility,
};
pub(crate) use rolling::{
    rolling_sharpe, rolling_sortino, rolling_volatility, ROLLING_KERNEL_RECOMPUTE_INTERVAL,
};
pub use rolling::{DatedSeries, RollingMetric};
pub(crate) use tail_risk::{
    cornish_fisher_var, expected_shortfall, kurtosis, parametric_var, skew_kurt, skewness,
    tail_ratio, value_at_risk, value_at_risk_and_es,
};
