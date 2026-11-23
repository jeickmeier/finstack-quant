//! Deposit-specific metrics module.
//!
//! Provides metric calculators for deposit instruments. Metrics include:
//! - Year fraction (instrument day-count)
//! - Discount factors at start and end dates
//! - Par (simple) rate
//! - Implied end-date discount factor from a quoted rate
//! - Quoted rate passthrough
//!
//! See unit tests and `examples/` for usage.

mod df;
mod df_end_from_quote;
mod par_rate;
mod register;

pub(crate) use df_end_from_quote::DfEndFromQuoteCalculator;
pub(crate) use par_rate::DepositParRateCalculator;
pub(crate) use register::register_deposit_metrics;
