//! Discounted cash-flow valuation for projected corporate free cash flows.
//!
//! [`DiscountedCashFlow`] supports Gordon-growth, exit-multiple, and H-model
//! terminal values, structured EV-to-equity bridges, dilution, and optional
//! valuation discounts. Cash-flow tenors are actual calendar days from the
//! configured valuation date divided by 365.25.

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::{
    DilutionSecurity, DiscountedCashFlow, EquityBridge, TerminalValueSpec, ValuationDiscounts,
};
