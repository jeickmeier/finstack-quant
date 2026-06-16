//! Comprehensive test suite for basis swap instruments.
//!
//! This module contains extensive tests covering:
//! - Metrics calculations (DV01, par spread, theta, annuity, bucketed DV01)
//! - Sensitivity analysis and risk measures
//! - Edge cases and boundary conditions
//! - Schedule generation with various conventions
//! - Mathematical accuracy and market standards compliance

mod test_basis_swap_edge_cases;
mod test_basis_swap_metrics;
mod test_basis_swap_par_spread;
mod test_basis_swap_sensitivities;
mod test_basis_swap_theta;
