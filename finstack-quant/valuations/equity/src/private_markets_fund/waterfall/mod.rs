//! Equity waterfall engine for private markets fund distribution calculations.
//!
//! This module implements the core waterfall allocation logic used in private
//! markets funds, including return of capital, preferred IRR hurdles, catch-up
//! provisions, promote splits, and clawback mechanisms.

pub(crate) mod engine;
pub(crate) mod ledger;
mod reconciliation;
pub(crate) mod spec;
#[cfg(test)]
mod tests;

pub(crate) use engine::EquityWaterfallEngine;
pub(crate) use ledger::{AllocationLedger, FundEvent, FundEventKind};
#[cfg(test)]
pub(super) use spec::{CatchUpMode, ClawbackSpec};
pub(crate) use spec::{PeFundWaterfallSpec, PeFundWaterfallTranche, WaterfallStyle};
