//! Cash Flow Waterfall & Sweep Mechanics
//!
//! This module implements dynamic cash flow allocation according to priority of payments,
//! excess cash flow sweeps, and PIK toggles based on model results.
//!
//! # Excess cash flow
//!
//! The sweep is authored term by term (`EcfSweepSpec`): a cash measure, the
//! deductions subtracted from it and the credits added back. Nothing is
//! deducted unless it is listed.
//!
//! ```text
//! ECF     = cash_measure - sum(deductions) + sum(credits)
//! Request = sweep_percentage x ECF     when ECF is positive, otherwise zero
//! ```
//!
//! Every term is a money amount in the pool currency. An `Input` term is a
//! signed formula over the model's nodes: a deduction of `2_000_000` lowers
//! ECF by that amount, and a negative deduction (a working-capital release
//! of 500,000 authored as `-500_000`) raises it. The `PaidCashInterest`, `PaidFees` and
//! `PaidScheduledPrincipal` terms are the cash this period's waterfall
//! actually paid at priorities ranking ahead of the sweep, including arrears;
//! PIK coupons and unpaid claims are not cash and are never deducted.
//!
//! The request is paid from remaining available cash across the elected
//! facilities (`FacilitySelection`), each up to its prepayment capacity. The
//! requested, paid and unpaid amounts of every prepayment rung are recorded as
//! a `PrepaymentOutcome`.
//!
//! ## Example
//!
//! ```text
//! EBITDA (cash measure):  10,000,000
//! Taxes (deduction):       2,000,000
//! CapEx (deduction):       1,500,000
//! Working capital build:     500,000
//! ECF:                     6,000,000
//! Request at 50%:          3,000,000
//! ```

mod cash_distribution;
mod engine;
mod excess_cash_flow;
mod payment_in_kind;
mod payment_stack;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
use engine::WaterfallPeriodResult;
pub(crate) use engine::execute_waterfall;
use engine::{eval_money, eval_number};
#[cfg(test)]
pub(crate) use test_support::{execute_waterfall_same_period, execute_waterfall_staged};
