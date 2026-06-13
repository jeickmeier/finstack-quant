//! Bond Future instrument implementation.
//!
//! This module provides support for bond futures (e.g., UST Treasury futures,
//! German Bund futures, UK Gilt futures) with deliverable basket mechanics.
//!
//! # Features
//!
//! - Deliverable basket with conversion factors
//! - Cheapest-to-deliver (CTD) bond selection
//! - Invoice price calculation
//! - Contract DV01 and bucketed risk metrics
//!
//! # Example
//!
//! ```ignore
//! use finstack_valuations::instruments::fixed_income::bond_future::{
//!     BondFuture, BondFutureSpecs, DeliverableBond, Position,
//! };
//! use finstack_valuations::instruments::Attributes;
//! use finstack_core::money::Money;
//! use finstack_core::currency::Currency;
//! use finstack_core::types::{InstrumentId, CurveId};
//! use time::macros::date;
//!
//! let future = BondFuture::builder()
//!     .id(InstrumentId::new("TYH5"))
//!     .notional(Money::new(1_000_000.0, Currency::USD))
//!     .expiry(date!(2025-03-20))
//!     .delivery_start(date!(2025-03-21))
//!     .delivery_end(date!(2025-03-31))
//!     .quoted_price(125.50)
//!     .position(Position::Long)
//!     .contract_specs(BondFutureSpecs::ust_10y())
//!     .deliverable_basket(vec![DeliverableBond {
//!         bond_id: InstrumentId::new("US912828XG33"),
//!         conversion_factor: 0.8234,
//!     }])
//!     .ctd_bond_id(InstrumentId::new("US912828XG33"))
//!     .discount_curve_id(CurveId::new("USD-TREASURY"))
//!     .attributes(Attributes::new())
//!     .build_validated()
//!     .expect("Valid bond future");
//! ```

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

// Re-export commonly used types
pub use pricer::BondFuturePricer;
pub use types::{BondFuture, BondFutureBuilder, BondFutureSpecs, DeliverableBond, Position};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        // This test exists only to ensure the module compiles
    }
}
