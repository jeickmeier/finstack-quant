//! Comprehensive test suite for interest rate options (caps/floors/caplets/floorlets).
//!
//! This test suite follows market-standard testing practices for interest rate derivatives:
//!
//! ## Test Organization
//!
//! - **construction**: Instrument creation, parameter validation, builder patterns
//! - **pricing**: Present value calculation, Black model correctness
//! - **cashflows**: Schedule generation for caps/floors
//! - **metrics**: Greeks and risk measures (delta, gamma, vega, theta, rho, dv01)
//! - **integration**: Full pricing scenarios with realistic market data
//! - **validation**: Edge cases, boundary conditions, numerical accuracy
//!
//! ## Coverage Goals
//!
//! - Core pricing logic: >90%
//! - Metrics calculators: >85%
//! - Edge cases and error handling: >80%
//!
//! ## Market Standards
//!
//! Tests validate:
//! - Black-76 model implementation
//! - Cap-floor parity relationships
//! - Greeks accuracy and consistency
//! - ATM/ITM/OTM behavior
//! - Time decay and expiry handling

mod cashflows;
mod construction;
mod integration;
mod metrics;
mod pricing;
mod validation;
