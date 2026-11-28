//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo`] — dated leveraged-buyout funding, exit bridge and sponsor returns
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo;
pub(crate) mod orchestrator;
