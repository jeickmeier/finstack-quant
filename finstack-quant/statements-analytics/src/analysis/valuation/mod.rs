//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo`] — leveraged-buyout transaction arithmetic over a statement model
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo;
pub(crate) mod lbo_funding;
pub(crate) mod orchestrator;
