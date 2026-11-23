//! Shared pricing kernels used by the instrument families and calibration.
//!
//! ## Sub-modules
//!
//! - [`discounting`]: curve-clock schedule PV and discount-factor-to-zero-rate helpers
//! - [`heston_market`]: strict resolution of the `HESTON_*` market scalars
//! - [`hw1f_params`]: Hull-White 1F κ/σ resolution from pricing overrides or calibrated market scalars
//! - [`inflation`]: inflation observation lag and reference CPI resolution
//! - [`market_inputs`]: scalar price, unitless scalar, and optional dividend-yield lookups
//! - [`monte_carlo`]: Monte Carlo path-count capping, override merging, and diagnostics attachment
//! - [`overnight_conventions`]: overnight-index convention checks
//! - [`term_fixings`]: term-index tenor validation
//! - [`volatility`]: override-then-surface volatility resolution for equity and FX consumers
//! - [`default_rate_calendar_id`]: the standard rates holiday calendar for a currency
//!
//! The total-return-swap leg kernels shared by equity and fixed-income TRS
//! (`TrsEngine`) are public at `crate::instruments`. The family-facing kernels
//! `overnight` and `variance_observations`, together with
//! `GenericInstrumentPricer` (the registry pricer behind `register_generic!`),
//! are public for the instrument crates but hidden from the rendered docs.

mod calendars;
pub mod discounting;
mod generic;
pub mod heston_market;
pub mod hw1f_params;
pub mod inflation;
pub mod market_inputs;
pub mod monte_carlo;
#[doc(hidden)]
pub mod overnight;
pub mod overnight_conventions;
pub mod term_fixings;
pub(crate) mod trs;
#[doc(hidden)]
pub mod variance_observations;
pub mod volatility;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

pub use calendars::default_rate_calendar_id;
