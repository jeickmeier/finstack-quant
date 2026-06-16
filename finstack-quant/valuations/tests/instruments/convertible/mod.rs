//! Comprehensive test suite for convertible bond instruments.
//!
//! Tests are organized into focused modules:
//! - `fixtures`: Common test fixtures, market contexts, and helper functions
//! - `test_pricing_basic`: Basic valuation, parity, conversion value
//! - `test_pricing_trees`: Binomial/trinomial trees and convergence
//! - `test_greeks`: Greeks calculations and sensitivities
//! - `test_conversion_policies`: Voluntary, mandatory, window, event-triggered
//! - `test_embedded_options`: Calls, puts, and combinations
//! - `test_sensitivities`: Market parameter sensitivities
//! - `test_edge_cases`: Edge cases, error handling, currency safety
//! - `test_metrics`: Metric calculator framework integration
//! - `quantlib_parity`: QuantLib parity tests for convertible bonds

// Test modules
mod fixtures;
mod quantlib_parity;
mod test_conversion_policies;
mod test_credit_curve;
mod test_edge_cases;
mod test_embedded_options;
mod test_greeks;
mod test_metrics;
mod test_pricing_basic;
mod test_sensitivities;
mod test_tree_convergence;
