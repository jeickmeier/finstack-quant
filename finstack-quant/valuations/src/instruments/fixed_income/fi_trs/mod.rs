//! Fixed-income index total-return swaps under a deterministic carry model.
//!
//! The total-return leg earns a supplied continuously compounded index yield;
//! the financing leg uses the configured discount and forward curves. This is
//! a carry analytic, not a full index mark-to-market model; see
//! [`FiIndexTotalReturnSwap`] for its inputs and limitations.

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::FiIndexTotalReturnSwap;

pub use crate::instruments::common_impl::parameters::trs_common::TrsScheduleSpec;
