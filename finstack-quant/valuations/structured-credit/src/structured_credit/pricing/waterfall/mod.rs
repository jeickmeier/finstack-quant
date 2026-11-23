//! Waterfall execution functions for structured credit instruments.
//!
//! This module contains pure functions for executing waterfall distributions.
//! All type definitions are in `types::waterfall`.

mod allocation;
pub(crate) mod execute;
mod payments;
#[cfg(test)]
mod tests;

pub(crate) use execute::{WaterfallContext, execute_waterfall};
pub(crate) use execute::{coverage_inputs, evaluate_coverage_tests, special_serviced_balance};
pub(crate) use payments::{SeniorFeeInputs, interest_claim_caps, senior_fee_accrual};
