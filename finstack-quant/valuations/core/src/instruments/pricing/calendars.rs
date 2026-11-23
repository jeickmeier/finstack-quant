//! Default rates holiday calendar resolution.

use finstack_quant_core::currency::Currency;

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
