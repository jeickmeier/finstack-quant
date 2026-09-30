//! Risk metrics test suite.
//!
//! Tests for Greek calculations, convergence properties, invariants, and edge cases.
//!
//! ## Test Organization
//!
//! - `bump_config_routing` - Every finite-difference greek honours `bump_config`
//! - `convergence` - Analytical vs finite difference Greek convergence
//! - `determinism` - Deterministic results for identical inputs
//! - `edge_cases` - Boundary conditions and degenerate cases
//! - `graceful_metrics_test` - Graceful failure handling for metric computation
//! - `greek_relationships` - Mathematical relationships between Greeks
//! - `invariants` - Property-based tests for metric invariants
//! - `sign_conventions` - Correct sign conventions for all Greeks
//! - `theta_observed_fixings` - Theta records the held spot for fixings inside the roll
//! - `var_quantile` - Historical-simulation VaR/ES quantile-estimator tests
//!
//! ## Notes
//!
//! - `vanna_volga_pockets` exercises internal bumping helpers; keep it in this module so
//!   integration coverage includes vanna/volga paths.

mod bump_config_routing;
mod convergence;
mod determinism;
mod edge_cases;
mod graceful_metrics_test;
mod greek_relationships;
mod invariants;
mod option_provider_consolidation;
mod sign_conventions;
mod theta_observed_fixings;
mod vanna_volga_pockets;
mod var_quantile;
