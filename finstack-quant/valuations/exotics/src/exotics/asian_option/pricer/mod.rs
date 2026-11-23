//! Asian option pricers (Monte Carlo and analytical).

pub(crate) mod analytic;
mod common;
mod mc;
#[cfg(test)]
mod tests;

pub(crate) use analytic::{
    AsianOptionAnalyticalGeometricPricer, AsianOptionSemiAnalyticalTwPricer,
};
pub(crate) use mc::AsianOptionMcPricer;

pub(crate) use common::resolve_spot;
pub(super) use common::{expired_value, map_fixings_to_steps};

// Test-only re-exports so `tests.rs` and `performance_accuracy` keep their
// `use super::*;` imports without touching external test files.
#[cfg(test)]
use crate::exotics::asian_option::types::AsianOption;
#[cfg(test)]
use analytic::seasoned_geometric_asian_control;
#[cfg(test)]
use common::{asian_drift_and_fixing_multipliers, paired_moments};
#[cfg(test)]
use finstack_quant_core::dates::{Date, DayCountContext};
#[cfg(test)]
use finstack_quant_core::market_data::context::MarketContext;
#[cfg(test)]
use finstack_quant_core::market_data::traits::Discounting;
#[cfg(test)]
use finstack_quant_models::monte_carlo::control_variate::apply_control_variate;
#[cfg(test)]
use finstack_quant_models::monte_carlo::engine::PathCaptureConfig;
#[cfg(test)]
use finstack_quant_models::monte_carlo::payoff::asian::{
    Asian, AsianFixingHistory, AveragingMethod,
};
#[cfg(test)]
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
#[cfg(test)]
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
#[cfg(test)]
use finstack_quant_models::types::{OptionMarketParams, OptionType};
#[cfg(test)]
use finstack_quant_valuations_core::pricer::Pricer;

#[cfg(test)]
#[path = "../../../../tests/performance/asian_option.rs"]
mod performance_accuracy;
