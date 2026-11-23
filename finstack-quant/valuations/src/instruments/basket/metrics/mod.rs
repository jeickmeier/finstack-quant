//! Basket metrics module.
//!
//! Provides metric calculators specific to `Basket`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_basket_metrics`.
//!
//! Exposed metrics:
//! - Constituent count
//! - One-percent relative mark shock
//!
//! Note: Present value is handled by the instrument's built-in value() method.

mod basket_mark_shock;
mod register;
mod weight_risk;

pub(crate) use register::register_basket_metrics;
pub(crate) use weight_risk::WeightRiskCalculator;
