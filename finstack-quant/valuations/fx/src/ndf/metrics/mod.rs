//! NDF metrics module.
//!
//! Provides metric calculators specific to `Ndf`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_ndf_metrics`.
//!
//! Exposed metrics:
//! - DV01 (interest rate sensitivity for settlement curve)
//! - Theta (time decay)

mod register;
#[cfg(test)]
mod tests;

pub(crate) use register::register_ndf_metrics;
