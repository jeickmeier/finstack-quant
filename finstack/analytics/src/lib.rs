#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Performance analytics on numeric slices and `finstack_core::dates::Date`.
//!
//! [`Performance`] is the only entry point. Construct it from a price or
//! return panel and every analytic — return / risk scalars, drawdown
//! statistics, rolling windows, periodic returns (MTD / QTD / YTD / FYTD),
//! benchmark alpha / beta, basic factor models — is a method on the
//! resulting instance.
//!
//! Result and config types ([`PeriodStats`], [`DrawdownEpisode`],
//! [`BetaResult`], [`GreeksResult`], [`RollingGreeks`], [`MultiFactorResult`],
//! [`CagrBasis`], [`AnnualizationConvention`], [`DatedSeries`],
//! [`RollingSharpe`], [`RollingSortino`], [`RollingVolatility`],
//! [`LookbackReturns`]) are re-exported here because `Performance` returns
//! them.
//!
//! The single freestanding function exception is [`beta`], kept public for
//! cross-crate use by `finstack-valuations`.
//!
//! Key conventions:
//! - returns are simple decimal returns
//! - annualization is derived from `finstack_core::dates::PeriodKind`
//! - drawdown depths are non-positive fractions such as `-0.25` for a 25% loss
//! - benchmark inputs are assumed pre-aligned to the panel's date grid
//! - rolling series are right-labeled: each output value is dated by the last
//!   observation in its window

// Internal re-exports of frequently used `finstack-core` modules.
// Kept `pub(crate)` so they don't leak into the public API; downstream callers
// should import from `finstack_core` directly.
pub(crate) use finstack_core::{dates, error, math};

pub(crate) type Result<T> = finstack_core::Result<T>;

pub(crate) mod aggregation;
pub(crate) mod benchmark;
pub(crate) mod drawdown;
pub(crate) mod lookback;
pub(crate) mod performance;
pub(crate) mod returns;
pub(crate) mod risk_metrics;

#[cfg(test)]
mod fixture_test;

pub use aggregation::PeriodStats;
pub use benchmark::{beta, BetaResult, GreeksResult, MultiFactorResult, RollingGreeks};
pub use drawdown::DrawdownEpisode;
pub use performance::{LookbackReturns, Performance};
pub use risk_metrics::{
    AnnualizationConvention, CagrBasis, DatedSeries, RollingSharpe, RollingSortino,
    RollingVolatility,
};
