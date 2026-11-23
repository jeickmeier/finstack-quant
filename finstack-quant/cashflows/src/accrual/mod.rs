//! Schedule-driven accrued interest.
//!
//! [`accrued_interest_amount`] reads a [`crate::builder::CashFlowSchedule`]
//! only: coupon shape, PIK splits, amortization, draws/repays, and ex-coupon
//! rules are inferred from `CFKind`-tagged flows and the schedule outstanding
//! path, not from instrument specs.

mod engine;
#[cfg(test)]
#[path = "../../tests/performance/accrual_work.rs"]
mod performance_work;
#[cfg(test)]
mod tests;

pub use engine::{
    AccrualConfig, AccrualIndex, AccrualMethod, ExCouponRule, accrued_interest_amount,
};
