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
//! - `rate`: Loan coupon specification (fixed all-in rate or floating index plus spread)
//! - `amortization`: Notional and amortization specifications
//! - `principal`: Principal exchange policy and explicit principal events
//!
//! ## Responsibilities
//!
//! - Type definitions for fixed and floating coupon specifications
//! - Fee specification types (fixed and periodic)
//! - Schedule parameter types (frequency, day count, business day conventions)
//! - Coupon type enums (Cash, PIK, Split)
//! - Behavioral models for credit instruments (prepayment, default, recovery)
//! - Helper constructors for common market conventions (USD, EUR, GBP, etc.)

pub(crate) mod amortization;
pub(crate) mod coupon;
pub(crate) mod default;
pub(crate) mod fees;
pub(crate) mod prepayment;
pub(crate) mod principal;
pub(crate) mod rate;
pub(crate) mod recovery;
pub(crate) mod schedule;
mod vectors;

pub(crate) use amortization::AmortizationSpec;
#[cfg(test)]
pub(crate) use coupon::FloatingRateFallback;
pub(crate) use coupon::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingLegCompounding, FloatingRateSpec,
    OvernightIndexConstraintApplication, StepUpCouponSpec,
};
pub(crate) use fees::{FeeAccrualBasis, FeeBase, FeeSpec};
pub(crate) use principal::{PrincipalEventSpec, PrincipalExchange};
pub(crate) use schedule::{RollRule, ScheduleParams};

use vectors::{check_unit_vector, vector_at};
