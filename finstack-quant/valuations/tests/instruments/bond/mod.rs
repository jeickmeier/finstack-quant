//! Comprehensive bond instrument test suite.
//!
//! # Test Organization
//!
//! - `cashflows`: Cashflow generation (fixed, floating, custom, amortizing)
//! - `pricing`: Basic pricing engine, settlement conventions, theta
//! - `metrics`: Individual metric calculator tests
//! - `validation`: Market benchmark validation (Fabozzi, Hull)
//! - `integration`: Complex bond structures (callable, putable, amortizing)

mod metrics;
