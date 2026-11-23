//! ILB metrics module.
//!
//! Splits ILB-specific metric calculators into focused files and
//! registers them with the shared metrics framework.

mod inflation01;
mod register;
#[cfg(test)]
mod tests;

pub(crate) use inflation01::Inflation01Calculator;
pub(crate) use register::register_ilb_metrics;
