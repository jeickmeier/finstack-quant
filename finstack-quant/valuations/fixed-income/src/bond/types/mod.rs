//! Bond instrument types and implementations.

mod construction;
mod definitions;
mod pricing;
mod return_floor;
mod traits;

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "tests/stepup.rs"]
mod tests_stepup;

pub(crate) use super::cashflow_spec::CashflowSpec;

pub use construction::FloatingBondParams;
pub use definitions::{
    Bond, BondBuilder, BondSettlementConvention, CallPut, CallPutSchedule, MakeWholeSpec,
};
pub use return_floor::{IssuePrice, ProtectionWindow, ReturnFloorKind, ReturnFloorSpec};
