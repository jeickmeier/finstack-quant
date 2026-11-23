//! Pricing and observation helpers for FX variance swaps.

mod engine;
mod replication;
#[cfg(test)]
mod tests;

use engine::get_historical_prices_with_dates;
pub(crate) use engine::{
    annualization_factor, compute_pv, expected_variance, observation_dates,
    partial_realized_variance, realized_fraction_by_observations,
};
pub(crate) use replication::remaining_forward_variance;
