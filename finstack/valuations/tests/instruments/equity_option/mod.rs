//! Comprehensive equity option test suite.
//!
//! This module organizes tests by functional area following market standards:
//! - Pricing: Black-Scholes PV validation
//! - Greeks: Individual greek tests with bounds and relationships
//! - Implied Vol: Solver accuracy and edge cases
//! - Edge Cases: Expiry, extreme strikes, zero vol, etc.
//! - Parity: Put-call parity validation
//! - Moneyness: ITM/ATM/OTM behavior
//!
//! Note: QuantLib parity tests were removed in favor of invariant/property-based tests
//! that don't rely on external reference values with loose tolerances.
//! See: test_option_bounds.rs (property tests) and test_put_call_parity.rs (parity tests)

mod helpers;
mod test_alt_models;
mod test_constructors;
mod test_edge_cases;
mod test_greeks;
mod test_implied_vol;
mod test_moneyness;
mod test_near_expiry;
mod test_pricing;
mod test_put_call_parity;
mod test_rough_bergomi;

mod test_option_pricing;
