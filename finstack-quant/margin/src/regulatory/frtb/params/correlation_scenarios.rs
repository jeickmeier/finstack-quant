//! Correlation scenario scaling factors per BCBS d457.
//!
//! The three correlation scenarios are applied to the base (Medium)
//! prescribed correlations to produce Low and High stress correlations.
//!
//! Low:  rho_low  = max(2 * rho_medium - 1, -1.0)
//! High: rho_high = min(1.25 * rho_medium, 1.0)
//!
//! These scaling operations are implemented directly on the
//! `CorrelationScenario` enum in `types.rs`.
