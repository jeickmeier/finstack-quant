//! Asset pool structures for structured credit instruments.

pub(crate) mod analytics;
pub(crate) mod asset;
pub(crate) mod asset_pool;
pub(crate) mod config;
#[cfg(test)]
mod tests;

pub(crate) use analytics::calculate_pool_stats;
pub(crate) use analytics::weighted_avg_rating_factor;
pub(crate) use asset::PoolAsset;
pub(crate) use asset_pool::AssetPool;
pub(crate) use asset_pool::{require_reset_lag, validate_reset_lag};
#[cfg(test)]
pub(crate) use config::ReinvestmentCriteria;
pub(crate) use config::{
    PoolAccounts, PoolStats, ReinvestmentAssumptions, ReinvestmentPeriod, RepLine,
};
