//! Deliverable-basket bond futures and CTD-based valuation.
//!
//! Valuation starts from an embedded cheapest-to-deliver (CTD) bond, subtracts
//! the present value of its cashflows through delivery, carries the remaining
//! dirty value to the later of the valuation date and delivery start, removes
//! accrued interest at delivery, and divides the clean price per 100 face by
//! the exchange-published conversion factor. The resulting futures price is
//! marked against `quoted_price` without further discounting because futures
//! settle through daily variation margin.
//!
//! Carry uses `repo_curve_id` when present and otherwise `discount_curve_id`.
//! Valuation does not search the basket for a new CTD or derive conversion
//! factors automatically; callers provide the resolved CTD, its bond terms,
//! and the factor stored in the deliverable basket.

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::BondFuturePricer;
pub use types::{
    BondFuture, BondFutureBuilder, BondFutureSpecs, DeliverableBond, DeliverableQuote,
};
