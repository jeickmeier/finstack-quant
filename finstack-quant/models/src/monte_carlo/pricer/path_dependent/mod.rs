//! Generic path-dependent option pricer with event scheduling.
//!
//! Handles payoffs that depend on the entire price path (Asians, barriers, lookbacks)
//! with flexible event scheduling.

mod config;
mod pricer;
mod sobol_bridge;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use config::MAX_LRM_CAPTURED_POINTS;
pub use config::PathDependentPricerConfig;
pub use pricer::PathDependentPricer;
