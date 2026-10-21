//! Shared pricing helpers used by this crate's instruments and by
//! `finstack-quant-calibration`.
//!
//! - [`swap_legs`](crate::instruments::pricing::swap_legs): shared floating/fixed leg pricing for swaps
//! - [`hw1f_params`](crate::instruments::pricing::hw1f_params): Hull-White 1F κ/σ resolution from pricing overrides or calibrated market scalars
//! - [`heston_market`](crate::instruments::pricing::heston_market): strict resolution of the `HESTON_*` market scalars
//! - [`term_fixings`](crate::instruments::pricing::term_fixings): term-index tenor validation and reset-date fixings
//! - [`overnight_conventions`](crate::instruments::pricing::overnight_conventions): overnight-index conventions for RFR coupons
//! - [`default_rate_calendar_id`](crate::instruments::pricing::default_rate_calendar_id): the standard rates holiday calendar for a currency

use finstack_quant_core::currency::Currency;

pub mod heston_market;
pub mod hw1f_params;
pub mod overnight_conventions;
pub mod swap_legs;
pub mod term_fixings;

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
