//! FX option metrics module.
//!
//! Defines FX option metric calculators per greek and registers
//! them with the `MetricRegistry`. Calculators reuse the pricing engine
//! helpers to ensure consistency between PV and greeks.

mod delta_conventions;
mod register;
#[cfg(test)]
mod tests;

pub(crate) use register::register_fx_option_metrics;
