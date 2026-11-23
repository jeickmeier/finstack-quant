//! Revolving credit facility types and instrument trait implementations.
//!
//! Defines the `RevolvingCredit` instrument with support for deterministic and
//! stochastic cashflow modeling. Supports standard fee structures (upfront,
//! commitment, usage, and facility fees) and both fixed and floating rate bases.

pub(crate) mod facility;
pub(crate) mod fees;
mod instrument;
pub(crate) mod specs;
#[cfg(test)]
mod tests;

pub(crate) use facility::RevolvingCredit;
pub(crate) use fees::RevolvingCreditFees;
pub(crate) use specs::{
    CreditSpreadProcessSpec, DrawRepayEvent, DrawRepaySpec, InterestRateProcessSpec, McConfig,
    RevolvingCreditMcRun, StochasticUtilizationSpec, UtilizationProcess,
};
