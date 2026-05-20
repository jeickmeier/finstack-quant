//! Bond pricing engines and utilities.
//!
//! # Engines (`engine/`)
//!
//! Core pricing math + the thin `Simple*Pricer` registry adapters that route
//! `(InstrumentType::Bond, ModelKey::*)` to the appropriate engine:
//! - **Discount**: PV = sum(CF_i * DF_i) using discount curves
//! - **Hazard**: Survival-weighted PV + fractional recovery of par (FRP)
//! - **Tree**: Binomial tree for callable/putable bonds and OAS
//! - **Merton MC**: Structural credit Monte Carlo for PIK bonds (feature-gated)
//!
//! # Utilities
//!
//! - `quote_conversions`: Price/yield/spread conversion functions
//! - `ytm_solver`: Robust yield-to-maturity calculation
//! - `settlement`: Settlement date and accrued interest utilities

pub mod engine;
pub mod quote_conversions;
pub(crate) mod settlement;
pub(crate) mod time_basis;
pub mod ytm_solver;
