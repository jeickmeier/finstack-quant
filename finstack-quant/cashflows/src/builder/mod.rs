//! Composable cashflow builder for instruments.
//!
//! Entry point: `CashFlowSchedule::builder()`.
//!
//! # Types
//!
//! - `CashFlowSchedule`, `CashFlowBuilder`, `Notional`
//! - `FixedCouponSpec`, `FloatingCouponSpec`, `CouponType`
//! - `AmortizationSpec`, `FeeSpec`, `ScheduleParams`
//! - `PrepaymentModelSpec`, `DefaultModelSpec`, `RecoveryModelSpec`
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
//! use finstack_quant_core::money::Money;
//! use finstack_quant_cashflows::builder::{CashFlowSchedule, ScheduleParams, FixedCouponSpec, CouponType};
//! use rust_decimal_macros::dec;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = Date::from_calendar_date(2025, Month::January, 15)?;
//! let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
//!
//! let fixed_spec = FixedCouponSpec {
//!     coupon_type: CouponType::Cash,
//!     rate: dec!(0.05),
//!     schedule: ScheduleParams::semiannual_30360(),
//! };
//!
//! let schedule = CashFlowSchedule::builder()
//!     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
//!     .fixed_cf(fixed_spec)
//!     .build_with_curves(None)?;
//!
//! assert!(!schedule.flows.is_empty());
//! # Ok(())
//! # }
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
pub(crate) mod date_generation;
pub mod periods;
pub mod rate_helpers;
pub mod schedule;
pub mod specs;

// Export the builder as CashFlowBuilder
pub use orchestrator::{CashFlowBuilder, PrincipalEvent};

// Re-export common types
pub use dataframe::{PeriodDataFrame, PeriodDataFrameOptions};
pub use periods::SchedulePeriod;
pub use rate_helpers::{
    project_floating_rate, project_floating_rate_from_market, FloatingRateParams,
};
pub use schedule::{
    sort_flows, CashFlowMeta, CashFlowSchedule, CashflowRepresentation, PvCreditAdjustment,
    PvDiscountSource,
};
pub use specs::{
    evaluate_fee_tiers, AmortizationSpec, CouponType, DefaultCurve, DefaultEvent, DefaultModelSpec,
    FeeAccrualBasis, FeeBase, FeeSpec, FeeTier, FixedCouponSpec, FixedWindow, FloatingCouponSpec,
    FloatingRateFallback, FloatingRateSpec, Notional, OvernightCompoundingMethod,
    OvernightIndexConstraintApplication, PrepaymentCurve, PrepaymentModelSpec, RecoveryModelSpec,
    ScheduleParams, StepUpCouponSpec,
};

// Re-export credit rate conversions (hazard-style CPR↔SMM helpers)
pub use credit_rates::{cpr_to_smm, smm_to_cpr};

#[doc(hidden)]
pub use emission::{
    emit_default_on, emit_prepayment_on, emit_revolving_credit_fees, RevolvingFeeEmissionConfig,
};
