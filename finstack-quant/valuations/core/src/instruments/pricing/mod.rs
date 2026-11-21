//! Shared pricing kernels used by the instrument families and calibration.
//!
//! ## Sub-modules
//!
//! - [`heston_market`]: strict resolution of the `HESTON_*` market scalars
//! - [`hw1f_params`]: Hull-White 1F κ/σ resolution from pricing overrides or calibrated market scalars
//! - [`overnight_conventions`]: overnight-index convention checks
//! - [`term_fixings`]: term-index tenor validation
//! - [`default_rate_calendar_id`]: the standard rates holiday calendar for a currency
//!
//! The total-return-swap leg kernels shared by equity and fixed-income TRS
//! (`TrsEngine`) are public at `crate::instruments`. The family-facing kernels
//! `overnight` and `variance_observations`, together with
//! `GenericInstrumentPricer` (the registry pricer behind `register_generic!`),
//! are public for the instrument crates but hidden from the rendered docs.

use finstack_quant_core::currency::Currency;

mod generic;
pub mod heston_market;
pub mod hw1f_params;
#[doc(hidden)]
pub mod overnight;
pub mod overnight_conventions;
pub mod term_fixings;
pub(crate) mod trs;
#[doc(hidden)]
pub mod variance_observations;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

/// Standard rates calendar for a currency when a contract does not supply one.
///
/// # Arguments
///
/// * `currency` - Currency whose standard rates holiday calendar is wanted.
///
/// # Returns
///
/// The registry id of the calendar (`"usny"`, `"target2"`, `"gblo"`, `"jpto"`,
/// `"auce"`, `"cato"` or `"chzh"`), or `None` for a currency with no standard
/// mapping, in which case the caller must supply a calendar explicitly.
pub fn default_rate_calendar_id(currency: Currency) -> Option<&'static str> {
    match currency {
        Currency::USD => Some("usny"),
        Currency::EUR => Some("target2"),
        Currency::GBP => Some("gblo"),
        Currency::JPY => Some("jpto"),
        Currency::AUD => Some("auce"),
        Currency::CAD => Some("cato"),
        Currency::CHF => Some("chzh"),
        _ => None,
    }
}
