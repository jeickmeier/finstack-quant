//! Margin-registry loading and override helpers.
//!
//! This module exposes the resolved configuration surface that powers schedule
//! IM, SIMM parameters, collateral schedules, and CCP fallback settings.
//! Values are loaded from embedded JSON assets and may be overlaid through
//! [`finstack_quant_core::config::FinstackConfig`] using
//! `"margin.registry.v1"`.
//!
//! # Units And Conventions
//!
//! - Rates and haircuts are stored as decimal fractions, not basis points.
//! - Thresholds, MTAs, and independent amounts are stored as raw currency
//!   amounts before conversion into [`finstack_quant_core::money::Money`].
//! - `mpor_days` and VM settlement lags are stored in business days.
//! - Schedule maturities are stored as year fractions.

mod embedded;
mod loader;
mod merge;
mod simm_validation;
#[cfg(test)]
mod tests;
mod types;
mod validation;
mod wire;

pub(crate) use loader::{
    embedded_registry, embedded_registry_or_panic, margin_registry_from_config,
};
#[cfg(test)]
pub(crate) use simm_validation::validate_simm_params;
pub(crate) use types::{
    AssetClassDefault, CcpParams, ClearedSettlementDefaults, ImDefaults, MarginRegistry,
    ScheduleBucketBoundaries, ScheduleImSchedule, SimmParams, TimingDefaults,
};
