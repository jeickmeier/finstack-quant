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
mod recovery;
mod schedule;

// Re-export all public types to maintain the same API
pub use amortization::{AmortizationSpec, Notional};
pub use coupon::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    OvernightCompoundingMethod, OvernightIndexConstraintApplication, StepUpCouponSpec,
};
pub use default::{DefaultCurve, DefaultModelSpec};
pub use fees::{evaluate_fee_tiers, FeeAccrualBasis, FeeBase, FeeSpec, FeeTier};
pub use prepayment::{PrepaymentCurve, PrepaymentModelSpec};
pub use recovery::RecoveryModelSpec;
pub use schedule::{FixedWindow, ScheduleParams};
