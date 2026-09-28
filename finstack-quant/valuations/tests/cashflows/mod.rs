//! Cashflows test suite modules.
//!
//! This module consolidates:
//! - `provider_contract`: `CashflowProvider` trait contract compliance
//! - `bridge_smoke`: compatibility smoke tests for `finstack_quant_cashflows::*`

pub(crate) mod helpers;

// Loaded once by `instruments::test_support`.
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::rates as rates_support;

mod bridge_smoke;
mod instrument_bridge;
mod provider_contract;
