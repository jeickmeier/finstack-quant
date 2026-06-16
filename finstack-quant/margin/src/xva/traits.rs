//! Traits for XVA-compatible instruments.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Minimal trait for values consumed by XVA exposure calculations.
///
/// XVA exposure only needs to identify instruments and value them at future
/// dates, so this trait deliberately stays narrower than the full
/// `Instrument` interface from `finstack-quant-valuations`.
pub trait Valuable: Send + Sync {
    /// Returns the instrument identifier used in diagnostics.
    fn id(&self) -> &str;

    /// Computes the instrument value at the requested future date.
    fn value(&self, market: &MarketContext, as_of: Date) -> Result<Money>;
}
