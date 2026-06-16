//! Bond instrument types and implementations.

mod construction;
mod definitions;
mod pricing;
mod traits;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests;
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests_stepup;

// Re-export for compatibility in tests and external users referencing bond::AmortizationSpec
pub(crate) use super::cashflow_spec::CashflowSpec;
pub use crate::cashflow::builder::AmortizationSpec;

pub use definitions::{Bond, BondSettlementConvention, CallPut, CallPutSchedule, MakeWholeSpec};
