//! Comprehensive bond instrument test suite.
//!
//! # Test Organization
//!
//! - `cashflows`: Cashflow generation (fixed, floating, custom, amortizing)
//! - `pricing`: Basic pricing engine, settlement conventions, theta
//! - `metrics`: Individual metric calculator tests
//! - `validation`: Market benchmark validation (Fabozzi, Hull)
//! - `integration`: Complex bond structures (callable, putable, amortizing)

mod bond_accrued_interest;
mod cashflows;
mod friction_cost;
mod helpers_tests;
mod integration;
mod merton_mc_convergence;
mod metrics;
mod pricing;
mod test_bond_pricing;
mod test_ytm_edge_cases;
mod validation;
