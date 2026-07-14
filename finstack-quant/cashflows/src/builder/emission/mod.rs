//! Date-level cashflow emission for the build pipeline.
//!
//! Submodules: `coupons`, `amortization`, `fees`, and `helpers`.
//! Each `emit_*_on` function returns flows for one date plus any PIK amount to
//! capitalize into outstanding balance.

mod amortization;
pub(crate) mod coupons;
mod fees;
mod helpers;

// Shared f64 ↔ Decimal conversion helpers live in `finstack_quant_core::decimal`
// and are re-exported here so submodules (coupons, fees, etc.) can use them
// via `super::`.
use finstack_quant_core::decimal::{decimal_to_f64, f64_to_decimal};

// Re-export coupon emission (internal to builder module)
pub(crate) use coupons::{emit_fixed_coupons_on, emit_float_coupons_on, ResolvedFloatMarket};

// Re-export amortization emission and types (internal to builder module)
pub(super) use amortization::{emit_amortization_on, AmortizationParams};

// Re-export fee emission (internal to builder module)
pub(super) use fees::emit_fees_on;

// Re-export helper utilities (internal to builder module)
pub(super) use helpers::compute_reset_date;

// Re-export the pre-computed inflation coupon adapter for inflation-linked instruments.
pub use coupons::emit_inflation_coupons;

// Re-export revolving-credit fee emission helpers (used by valuations crate).
pub use fees::{emit_revolving_credit_fees, RevolvingFeeEmissionConfig};
