//! Barrier option test suite.
//!
//! This module organizes tests for barrier option pricing:
//! - Day count basis: Validates correct separation of vol vs discount bases
//! - Pricing: Analytical and Monte Carlo convergence
//! - Barrier types: Up/Down, In/Out combinations

mod helpers;
mod test_day_count_basis;
