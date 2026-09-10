//! Date-level cashflow emission for the build pipeline.
//!
//! Submodules: `coupons`, `amortization`, `fees`, and `helpers`.
//! Each `emit_*_on` function returns flows for one date plus any PIK amount to
//! capitalize into outstanding balance.

mod amortization;
mod balances;
pub(crate) mod coupons;
mod fees;
mod helpers;

use finstack_quant_core::decimal::{decimal_to_f64, f64_to_decimal};

pub(crate) use coupons::{
    emit_fixed_coupons_on, emit_float_coupons_on, FloatEmissionOutput, ResolvedFloatMarket,
};

pub(super) use amortization::{emit_amortization_on, AmortizationParams};

pub(super) use fees::emit_fees_on;

pub(super) use helpers::compute_reset_date;

pub use coupons::emit_inflation_coupons;

pub use fees::{emit_revolving_credit_fees, RevolvingFeeEmissionConfig};
