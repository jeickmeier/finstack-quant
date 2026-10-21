//! Date-level cashflow emission for the build pipeline.
//!
//! Submodules: `coupons`, `amortization`, `fees`, and `balances`. Each
//! `emit_*_on` function pushes the flows for one date into the build state;
//! the coupon emitters also return any PIK amount to capitalize into the
//! outstanding balance.

mod amortization;
mod balances;
mod coupons;
mod fees;

pub(crate) use coupons::{emit_fixed_coupons_on, emit_float_coupons_on, ResolvedFloatMarket};

pub(super) use amortization::emit_amortization_on;

pub(super) use fees::emit_fees_on;
