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

/// Minimum swap/TRS annuity threshold guarded against divide-by-zero in par-rate
/// and par-spread solvers.
pub const ANNUITY_EPSILON: f64 = 1e-12;
