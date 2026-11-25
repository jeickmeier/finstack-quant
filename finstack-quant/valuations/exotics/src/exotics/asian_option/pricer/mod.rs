//! Asian option pricers (Monte Carlo and analytical).

pub(crate) mod analytic;
pub(super) mod inputs;
mod mc;
pub(crate) mod moments;
#[cfg(test)]
mod tests;

pub(crate) use analytic::{
    AsianOptionAnalyticalGeometricPricer, AsianOptionSemiAnalyticalTwPricer,
};
pub(super) use inputs::{expired_value, map_fixings_to_steps};
pub(crate) use mc::AsianOptionMcPricer;

#[cfg(test)]
#[path = "../../../../tests/performance/asian_option.rs"]
mod performance_accuracy;
