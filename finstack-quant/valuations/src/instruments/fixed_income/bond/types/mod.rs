//! Bond instrument types and implementations.

mod construction;
mod definitions;
mod pricing;
mod return_floor;
mod traits;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_stepup;

// Re-export for compatibility in tests and external users referencing bond::AmortizationSpec
pub(crate) use super::cashflow_spec::CashflowSpec;
pub use crate::cashflow::builder::AmortizationSpec;

pub use definitions::{Bond, BondSettlementConvention, CallPut, CallPutSchedule, MakeWholeSpec};
pub use return_floor::{IssuePrice, ProtectionWindow, ReturnFloorKind, ReturnFloorSpec};
