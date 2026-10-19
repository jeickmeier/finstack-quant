//! Shared pricing kernels used by the instrument families and calibration.
//!
//! ## Sub-modules
//!
//! - [`heston_market`]: strict resolution of the `HESTON_*` market scalars
//! - [`hw1f_params`]: Hull-White 1F κ/σ resolution from pricing overrides or calibrated market scalars
//! - [`overnight_conventions`]: overnight-index convention checks
//! - [`swap_legs`]: shared floating/fixed leg pricing for swaps
//! - [`term_fixings`]: term-index tenor validation and reset-date fixings
//!
//! [`TrsEngine`] holds the total-return-swap leg kernels shared by equity and
//! fixed-income TRS. The family-facing kernels `coupon_period`,
//! `floating_reset_descriptors`, `overnight`, `rates_credit` and
//! `variance_observations`, and `GenericInstrumentPricer` (the registry pricer
//! behind `register_generic!`), are public for the instrument crates but hidden
//! from the rendered docs.

#[doc(hidden)]
pub mod coupon_period;
#[doc(hidden)]
pub mod floating_reset_descriptors;
mod generic;
pub mod heston_market;
pub mod hw1f_params;
#[doc(hidden)]
pub mod overnight;
pub mod overnight_conventions;
#[doc(hidden)]
pub mod rates_credit;
pub mod swap_legs;
pub mod term_fixings;
mod trs;
#[doc(hidden)]
pub mod variance_observations;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

pub use trs::{
    FinancingLegFlows, PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel,
};
