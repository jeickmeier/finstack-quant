//! Specification types for the cashflow builder.
//!
//! This module contains type definitions for coupon, fee, and scheduling specifications
//! that configure the `CashFlowBuilder`. These are the primary input types that users
//! interact with when building cashflow schedules.
//!
//! ## Organization
//!
//! Specifications are organized into logical modules:
//! - `coupon`: Fixed and floating coupon specifications
//! - `fees`: Fee specifications and tier evaluation
//! - `schedule`: Schedule parameters and timing windows
//! - `prepayment`: Prepayment models (CPR/PSA)
//! - `default`: Default models (CDR/SDA) and events
//! - `recovery`: Recovery specifications
//!
//! ## Responsibilities
//!
//! - Type definitions for fixed and floating coupon specifications
//! - Fee specification types (fixed and periodic)
//! - Schedule parameter types (frequency, day count, business day conventions)
//! - Coupon type enums (Cash, PIK, Split)
//! - Behavioral models for credit instruments (prepayment, default, recovery)
//! - Helper constructors for common market conventions (USD, EUR, GBP, etc.)

mod amortization;
mod coupon;
mod default;
mod fees;
mod prepayment;
mod principal;
mod recovery;
mod schedule;

/// Read a per-month vector at a seasoning month: month 0 and month 1 read
/// the first entry, months past the end hold the last one.
///
/// # Errors
///
/// Returns `Error::Validation` when the vector is empty or the entry is not
/// a finite decimal in `[0, 1]` (rates) — the caller names the field.
pub(super) fn vector_at(
    values: &[f64],
    seasoning_months: u32,
    name: &str,
) -> finstack_quant_core::Result<f64> {
    let Some(last) = values.len().checked_sub(1) else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{name} must hold at least one value"
        )));
    };
    let index = (seasoning_months.saturating_sub(1) as usize).min(last);
    let value = values[index];
    if !value.is_finite() || value < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{name}[{index}] ({value}) must be finite and non-negative"
        )));
    }
    Ok(value)
}

pub use amortization::{AmortizationSpec, Notional};
pub use coupon::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    OvernightCompoundingMethod, OvernightIndexConstraintApplication, StepUpCouponSpec,
};
pub use default::{DefaultCurve, DefaultModelSpec};
pub use fees::{evaluate_fee_tiers, FeeAccrualBasis, FeeBase, FeeSpec, FeeTier};
pub use prepayment::{PrepaymentCurve, PrepaymentModelSpec};
pub use principal::PrincipalExchange;
pub use recovery::RecoveryModelSpec;
pub use schedule::{RollRule, ScheduleParams};
