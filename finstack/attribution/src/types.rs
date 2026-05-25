//! Core data structures for P&L attribution.
//!
//! This module provides types for decomposing multi-period P&L changes into
//! constituent factors: carry, curve shifts, credit spreads, FX, volatility,
//! cross-factor interactions, model parameters, and market scalars.

use finstack_core::currency::Currency;
use finstack_core::money::Money;

mod detail;
mod result;

pub use detail::*;
pub use result::*;

/// Zero USD `Money` — serde default for [`CreditFactorAttribution::curve_shape_pnl`]
/// so attributions serialized before that field was added still deserialize.
pub(crate) fn zero_money_usd() -> Money {
    Money::new(0.0, Currency::USD)
}
