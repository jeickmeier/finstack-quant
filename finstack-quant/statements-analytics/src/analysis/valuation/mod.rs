//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo_funding`] — dated leveraged-buyout funding, exit bridge and sponsor returns
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo_funding;
pub(crate) mod orchestrator;
