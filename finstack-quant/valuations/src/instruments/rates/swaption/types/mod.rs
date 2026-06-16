//! Swaption (option on interest rate swap) implementation with SABR volatility.
//!
//! This module defines the `Swaption` data structure and integrates with the
//! common instrument trait via `impl_instrument!`. All pricing math is
//! implemented in the `pricing/` submodule; metrics are provided in the
//! `metrics/` submodule. The type exposes helper methods for forward swap
//! rate, annuity, and day-count based year fractions that reuse core library
//! functionality.

mod bermudan;
mod definitions;
mod swaption;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests;

pub use bermudan::BermudanSwaption;
pub use definitions::{
    BermudanSchedule, BermudanType, CashSettlementMethod, SABRParameters, SwaptionExercise,
    SwaptionSettlement, VolatilityModel,
};
pub use swaption::{GreekInputs, Swaption};

pub(crate) use bermudan::lognormal_to_normal_vol;
