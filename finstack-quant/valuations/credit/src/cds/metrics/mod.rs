//! CDS metrics module.
//!
//! Provides metric calculators specific to `CreditDefaultSwap`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_cds_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bp)
//! - Risky PV01
//! - Risky annuity
//! - CS01
//! - Protection leg PV
//! - Premium leg PV
//! - Expected loss
//! - Jump to default (includes accrued premium)
//! - Jump to default LGD only (excludes accrued premium)

mod cs_gamma;
pub(crate) mod dv01;
mod jump_to_default;
pub(crate) mod recovery01;
mod register;
#[cfg(test)]
mod tests;

#[cfg(test)]
use register::ExpectedLossCalculator;
pub(crate) use register::{deal_quote_override, market_doc_clause, register_cds_metrics};
