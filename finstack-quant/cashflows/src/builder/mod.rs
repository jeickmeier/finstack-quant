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
//!   (credit calculators re-exported here; `CashFlowBuilder.build()` does not
//!   consume them)
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
//!     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
//!     .fixed_cf(fixed_spec)
//!     .build(None)?;
//!
//! assert!(!schedule.get_flows().is_empty());
//! # Ok(())
//! # }
//! ```

pub(crate) mod calendar;
pub(crate) mod compiler;
mod coupon_api;
pub(crate) mod credit_rates;
pub(crate) mod date_generation;
pub(crate) mod emission;
mod floating_replay;
mod orchestrator;
pub(crate) mod overnight;
pub(crate) mod periods;
pub(crate) mod pipeline;
mod principal;
pub(crate) mod rate_helpers;
pub(crate) mod schedule;
pub(crate) mod specs;

pub use calendar::WEEKENDS_ONLY_ID;
pub use credit_rates::{
    abs_to_smm, cdr_to_mdr, cpr_to_smm, mdr_to_cdr, psa_cpr, smm_to_cpr, PSA_RAMP_MONTHS,
    PSA_TERMINAL_CPR,
};
pub use emission::emit_inflation_coupons;
pub use floating_replay::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod,
    FloatingCouponReplayState, FloatingCouponSettlement, FloatingRateObservation,
};
pub use orchestrator::CashFlowBuilder;
pub use overnight::{
    OvernightObservationSchedule, OvernightObservationSlice, OvernightRateAccumulator,
    OvernightRateConstraints, OvernightRateReplay,
};
pub use periods::{
    build_periods, build_single_period, period_accrual, BuildPeriodsParams, SchedulePeriod,
};
pub use rate_helpers::{
    calculate_floating_rate, project_floating_rate, project_index_rate, FloatingRateParams,
};
pub use schedule::{
    merge_cashflow_schedules, weighted_average_life_from_principal, CashFlowMeta, CashFlowSchedule,
    CashflowRepresentation,
};
pub use specs::{
    evaluate_fee_tiers, AmortizationSpec, CouponType, DefaultModelSpec, FeeAccrualBasis, FeeBase,
    FeeSpec, FeeTier, FixedCouponSpec, FloatingCouponSpec, FloatingLegCompounding,
    FloatingRateFallback, FloatingRateSpec, Notional, OvernightIndexConstraintApplication,
    PrepaymentModelSpec, PrincipalExchange, RecoveryModelSpec, RollRule, ScheduleParams,
    StepUpCouponSpec,
};
