//! Real estate asset metrics module.
//!
//! Provides standard rate risk and deal-style metrics for unlevered real estate
//! valuations, plus shared helpers reused by levered real estate equity.

mod cap_rates;
mod register;
mod returns;
mod sensitivities;

pub use register::moic;
pub(crate) use register::register_real_estate_metrics;
pub use sensitivities::{bumped_cap_rate, ensure_cap_rate01_applicable};
