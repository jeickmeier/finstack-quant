//! Equity waterfall engine for private markets fund distribution calculations.
//!
//! This module implements the core waterfall allocation logic used in private
//! markets funds, including return of capital, preferred IRR hurdles, catch-up
//! provisions, promote splits, and clawback mechanisms.

mod engine;
mod ledger;
mod reconciliation;
mod spec;
#[cfg(test)]
mod tests;

pub use engine::EquityWaterfallEngine;
pub use ledger::{AllocationLedger, AllocationRow, FundEvent, FundEventKind};
pub use spec::{
    CatchUpMode, ClawbackSpec, PeFundWaterfallSpec, PeFundWaterfallTranche, WaterfallSpecBuilder,
    WaterfallStyle,
};
