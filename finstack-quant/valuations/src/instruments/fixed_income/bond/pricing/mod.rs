//! Bond pricing engines and utilities.
//!
//! # Engines (`engine/`)
//!
//! Core pricing math plus the bond registry adapter that preserves the
//! caller-selected model:
//! - **Discounting**: Non-callable PV from projected cashflows and discount
//!   curves; an attached credit curve is ignored
//! - **Hazard rate**: Non-callable survival-weighted PV plus fractional
//!   recovery of par; an explicit hazard curve is required
//! - **Tree**: Rates-only option rollback and OAS; an attached credit curve is
//!   ignored
//! - **Rates credit**: Joint rates-credit rollback and OAS, including call,
//!   put, and return-floor rights
//! - **Merton MC**: Structural credit Monte Carlo for PIK bonds (feature-gated)
//!
//! # Utilities
//!
//! - `quote_conversions`: Price/yield/spread conversion functions
//! - `ytm_solver`: Robust yield-to-maturity calculation
//! - `settlement`: Settlement date and accrued interest utilities

pub mod engine;
pub mod quote_conversions;
pub(crate) mod return_floor;
pub(crate) mod settlement;
pub(crate) mod time_basis;
pub mod ytm_solver;
