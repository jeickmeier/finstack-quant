//! Shared valuation market convention constants.
//!
//! For rate and spread conversions in non-hot-path code, prefer the typed
//! wrappers [`finstack_quant_core::types::Rate`] and
//! [`finstack_quant_core::types::Bps`].

/// Standard number of equity/FX trading days per year (252).
///
/// Conventional denominator for annualizing daily realized variance,
/// volatility, Sharpe ratio, and theta decay.
pub const TRADING_DAYS_PER_YEAR: f64 = 252.0;
