//! Term structure curve tests.
//!
//! This module groups tests for curve types:
//! - [`discount`] - Discount curves (DF interpolation, bumps, no-arbitrage validation)
//! - [`forward`] - Forward rate curves
//! - [`hazard`] - Credit hazard rate curves (survival probabilities)
//! - [`inflation`] - Inflation/CPI curves
//! - [`base_correlation`] - Credit index base correlation curves
//! - [`flat_tests`] - Flat curve tests

mod base_correlation;
mod discount;
mod flat_tests;
mod forward;
mod hazard;
mod inflation;
mod scalar_curves_invariants;
