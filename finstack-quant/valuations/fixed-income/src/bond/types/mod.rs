//! Bond instrument types and implementations.

pub(crate) mod construction;
pub(crate) mod definitions;
mod pricing;
pub(crate) mod return_floor;
mod traits;

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "tests/stepup.rs"]
mod tests_stepup;

pub(crate) use super::cashflow_spec::CashflowSpec;

#[cfg(test)]
pub(super) use definitions::{Bond, BondSettlementConvention, CallPut, CallPutSchedule};
