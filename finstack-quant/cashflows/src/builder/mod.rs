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

mod compiler;
mod coupon_api;
mod credit_rates;
mod date_generation;
mod emission;
mod floating_replay;
mod orchestrator;
mod overnight;
pub(crate) mod periods;
mod pipeline;
mod rate_helpers;
pub(crate) mod schedule;
mod specs;

pub use credit_rates::{
    abs_to_smm, cdr_to_mdr, clamped_cdr_to_mdr, clamped_cpr_to_smm, cpr_to_smm, mdr_to_cdr,
    psa_cpr, smm_to_cpr, PSA_RAMP_MONTHS, PSA_TERMINAL_CPR, SDA_PEAK_CDR,
};
pub use floating_replay::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod,
    FloatingCouponReplayState, FloatingCouponSettlement, FloatingRateObservation,
};
pub use orchestrator::CashFlowBuilder;
pub use overnight::{
    overnight_day_count_basis, OvernightObservationSchedule, OvernightObservationSlice,
    OvernightRateAccumulator, OvernightRateConstraints, OvernightRateReplay,
};
pub use periods::{
    build_periods, build_single_period, compute_reset_date, period_accrual, BuildPeriodsParams,
    SchedulePeriod,
};
pub use rate_helpers::{
    apply_bp_bounds, calculate_floating_rate, observed_term_fixing, project_floating_rate,
    project_index_rate, project_overnight_rate, FloatingRateParams,
};
pub use schedule::{
    merge_cashflow_schedules, weighted_average_life_from_principal, CashFlowMeta, CashFlowSchedule,
    CashflowRepresentation,
};
pub use specs::{
    evaluate_fee_tiers, AmortizationSpec, CouponType, DefaultModelSpec, FeeAccrualBasis, FeeBase,
    FeeSpec, FeeTier, FixedCouponSpec, FloatingCouponSpec, FloatingLegCompounding,
    FloatingRateFallback, FloatingRateSpec, Notional, OvernightIndexConstraintApplication,
    PrepaymentModelSpec, PrincipalEventSpec, PrincipalExchange, RateSpec, RecoveryModelSpec,
    RollRule, ScheduleParams, StepUpCouponSpec,
};
