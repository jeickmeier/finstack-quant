//! Metrics module for revolving credit facilities.
//!
//! Provides both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
//! facility-specific metrics (utilization rate, available capacity, weighted average
//! cost, and the draw option cost of stochastic facilities).

pub(crate) mod cs01;
pub(crate) mod exposure;
pub(crate) mod oid_eir;
pub(crate) mod quotes;
mod register;
pub(crate) mod weighted_average_cost;

pub(crate) use register::register_revolving_credit_metrics;
use register::{drawn_at, undrawn_at};
pub(crate) use weighted_average_cost::ApproxWeightedAverageCostCalculator;
