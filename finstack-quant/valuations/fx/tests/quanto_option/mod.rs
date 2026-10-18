//! Regression tests for QuantoOption greeks and metric calculators.
//!
//! These tests pin behaviour around recent fixes:
//! - FX vega uses an absolute (additive) bump, not a multiplicative one
//! - `Correlation01Calculator` is boundary-aware (rho near +/-1 still returns
//!   a finite, sensible value)
//! - `OptionGreeksProvider::option_greeks` errors on unsupported greek kinds
//!   instead of silently returning zero defaults

mod regression;
