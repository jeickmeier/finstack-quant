//! Return-based, tail-risk, and rolling analytics on simple return series.
//!
//! The metric functions in this module are crate-internal building blocks for
//! [`crate::performance::Performance`]. Library consumers should drive every
//! analytic through `Performance`; the result/config types ([`CagrBasis`],
//! [`AnnualizationConvention`], [`DatedSeries`]) are re-exported because
//! `Performance` returns them.

mod return_based;
mod rolling;
mod tail_risk;

pub(crate) use return_based::invalid_annualization_factor;
pub(crate) use return_based::{
    cagr, downside_deviation, gain_to_pain, geometric_mean, mean_return, mean_vol_annualized,
    modified_sharpe, omega_ratio, sharpe, sortino, volatility,
};
pub use return_based::{AnnualizationConvention, CagrBasis};
pub use rolling::DatedSeries;
pub(crate) use rolling::{rolling_sharpe, rolling_sortino, rolling_volatility};
pub(crate) use tail_risk::{
    cornish_fisher_var, expected_shortfall, kurtosis, parametric_var, skew_kurt, skewness,
    tail_ratio, value_at_risk, value_at_risk_and_es,
};
