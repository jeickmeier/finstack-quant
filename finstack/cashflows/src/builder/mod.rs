//! Composable cashflow builder for instruments.
//!
//! Provides a builder interface for common cashflow patterns with programmatic control.
//! Use `CashFlowSchedule::builder()` as the standard entry point.
//!
//! # Public API
//!
//! ## Primary Types (stable)
//! - `CashFlowSchedule` — Output schedule containing ordered cashflows
//! - `CashFlowBuilder` — Fluent builder for constructing schedules
//! - `Notional` — Principal amount with optional amortization
//!
//! ## Coupon Specifications (stable)
//! - `FixedCouponSpec` — Fixed-rate coupon configuration
//! - `FloatingCouponSpec` — Floating-rate coupon with index, spread, caps/floors
//! - `CouponType` — Payment type (Cash, PIK, or Split)
//!
//! ## Amortization & Fees (stable)
//! - `AmortizationSpec` — Principal behavior (None, Linear, Step, Percent, Custom)
//! - `FeeSpec` — Fixed or periodic fee configuration
//!
//! ## Schedule Parameters (stable)
//! - `ScheduleParams` — Frequency, day count, business day convention
//!
//! ## Credit Models (for structured products)
//! - `PrepaymentModelSpec`, `DefaultModelSpec`, `RecoveryModelSpec`
//!
//! # Usage
//!
//! ```rust
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
//! use finstack_core::money::Money;
//! use finstack_cashflows::builder::{CashFlowSchedule, ScheduleParams, FixedCouponSpec, CouponType};
//! use rust_decimal_macros::dec;
//! use time::Month;
//!
//! let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
//! let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
//!
//! let fixed_spec = FixedCouponSpec {
//!     coupon_type: CouponType::Cash,
//!     rate: dec!(0.05),
//!     freq: Tenor::semi_annual(),
//!     dc: DayCount::Act365F,
//!     bdc: BusinessDayConvention::Following,
//!     calendar_id: "weekends_only".to_string(),
//!     stub: StubKind::None,
//!     end_of_month: false,
//!     payment_lag_days: 0,
//! };
//!
//! let schedule = CashFlowSchedule::builder()
//!     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
//!     .fixed_cf(fixed_spec)
//!     .build_with_curves(None)
//!     .expect("should succeed");
//!
//! assert!(schedule.flows.len() > 0);
//! ```

// Internal modules
pub(crate) mod compiler;
mod coupon_api;
pub mod emission;
mod orchestrator;
pub(crate) mod pipeline;
mod principal;

// Public modules
pub mod calendar;
pub(crate) mod credit_rates;
pub(crate) mod dataframe;
pub mod date_generation;
pub mod periods;
pub mod rate_helpers;
pub mod schedule;
pub mod specs;

// Export the builder as CashFlowBuilder
pub use orchestrator::{CashFlowBuilder, PrincipalEvent};

// Re-export common types
pub use dataframe::{PeriodDataFrame, PeriodDataFrameOptions};
pub use date_generation::{build_dates, PeriodSchedule, SchedulePeriod};
pub use rate_helpers::{
    project_floating_rate, project_floating_rate_from_market, FloatingRateParams,
};
pub use schedule::{
    CashFlowMeta, CashFlowSchedule, CashflowRepresentation, PvCreditAdjustment, PvDiscountSource,
};
pub use specs::{
    evaluate_fee_tiers, AmortizationSpec, CouponType, DefaultCurve, DefaultEvent, DefaultModelSpec,
    FeeAccrualBasis, FeeBase, FeeSpec, FeeTier, FixedCouponSpec, FixedWindow, FloatingCouponSpec,
    FloatingRateFallback, FloatingRateSpec, Notional, OvernightCompoundingMethod, PrepaymentCurve,
    PrepaymentModelSpec, RecoveryModelSpec, ScheduleParams, StepUpCouponSpec,
};

// Re-export credit rate conversions (hazard-style CPR↔SMM helpers)
pub use credit_rates::{cpr_to_smm, smm_to_cpr};

#[doc(hidden)]
pub use emission::{
    emit_default_on, emit_prepayment_on, emit_revolving_credit_fees, RevolvingFeeEmissionConfig,
};
