//! Benchmark-relative statistics with exact positional pairing and scaled centered OLS.

mod capture;
pub(crate) mod regression;
#[cfg(test)]
mod tests;

pub(crate) use capture::{
    batting_average, capture_ratio, down_capture, m_squared_from_returns, treynor_from_returns,
    up_capture,
};
pub(crate) use regression::{BetaResult, GreeksResult, RollingGreeks, beta};
pub(crate) use regression::{greeks, information_ratio, r_squared, rolling_greeks, tracking_error};
