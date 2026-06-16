#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Cashflow schedule generation, aggregation, and currency-safe operations.
//!
//! Build dated schedules for bonds, swaps, loans, and structured products using
//! currency-tagged [`Money`]. Aggregation and period PV helpers keep currency
//! boundaries explicit.
//!
//! # Modules
//!
//! - [`primitives`]: `CashFlow` and `CFKind` (from `finstack-quant-core`)
//! - [`builder`]: [`builder::CashFlowSchedule`] and schedule construction
//! - [`aggregation`]: period rollups and PV aggregation inputs
//! - [`accrual`]: schedule-driven accrued interest
//! - [`traits`]: [`CashflowProvider`] and `schedule_from_*` helpers
//! - [`json`]: serde-first JSON bridge for building and validating schedules
//!   (binding surface)
//!
//! # Conventions
//!
//! - Coupon rates are decimals (for example `0.05` for 5%).
//! - Spreads and periodic fee quotes are often basis points on spec types.
//! - Timing follows the day-count, calendar, and lag fields on builder specs.
//! - [`primitives::CFKind`] is `#[non_exhaustive]` in `finstack_quant_core::cashflow`; do not
//!   assume a fixed variant set in downstream matches.
//!
//! # Examples
//!
//! ## Building a Cashflow Schedule
//!
//! ```rust
//! use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
//! use finstack_quant_core::money::Money;
//! use rust_decimal_macros::dec;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = Date::from_calendar_date(2025, Month::January, 15)?;
//! let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
//!
//! let schedule = CashFlowSchedule::builder()
//!     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
//!     .fixed_cf(FixedCouponSpec {
//!         coupon_type: CouponType::Cash,
//!         rate: dec!(0.05),
//!         freq: Tenor::semi_annual(),
//!         dc: DayCount::Thirty360,
//!         bdc: BusinessDayConvention::Following,
//!         calendar_id: "weekends_only".to_string(),
//!         stub: StubKind::None,
//!         end_of_month: false,
//!         payment_lag_days: 0,
//!     })
//!     .build_with_curves(None)?;
//!
//! assert!(!schedule.flows.is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! ## Aggregating Cashflows
//!
//! ```rust
//! use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::create_date;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let date1 = create_date(2025, Month::January, 15)?;
//! let date2 = create_date(2025, Month::July, 15)?;
//!
//! let flows = vec![
//!     (date1, Money::new(50_000.0, Currency::USD)),
//!     (date2, Money::new(50_000.0, Currency::USD)),
//! ];
//!
//! // Aggregate using explicit target currency
//! let aggregated = aggregate_cashflows_checked(&flows, Currency::USD)?;
//! assert_eq!(aggregated.amount(), Money::new(100_000.0, Currency::USD).amount());
//! # Ok(())
//! # }
//! ```
//!
//! ## Periodized present value
//!
//! Use [`builder::CashFlowSchedule::pv_by_period`] with [`aggregation::DateContext`]:
//!
//! ```ignore
//! use finstack_quant_cashflows::builder::CashFlowSchedule;
//! use finstack_quant_cashflows::aggregation::DateContext;
//! use finstack_quant_cashflows::builder::PvDiscountSource;
//! use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period};
//! use finstack_quant_core::market_data::traits::Discounting;
//!
//! fn periodized_pv(
//!     schedule: &CashFlowSchedule,
//!     periods: &[Period],
//!     disc: &dyn Discounting,
//!     base: Date,
//! ) -> finstack_quant_core::Result<()> {
//!     let pv_map = schedule.pv_by_period(
//!         periods,
//!         PvDiscountSource::Discount { disc, credit: None },
//!         DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
//!     )?;
//!
//!     let _ = pv_map;
//!     Ok(())
//! }
//! ```
//!
//! Instrument-level PV adapters live in other workspace crates; this crate stops
//! at schedule construction and schedule-level period PV.

/// Cash-flow primitives (`CashFlow`, `CFKind`).
pub mod primitives {
    pub use finstack_quant_core::cashflow::{CFKind, CashFlow};
}

/// Currency-preserving aggregation utilities for cashflows.
pub mod aggregation;

/// Composable cashflow builder (phase 1: principal, amortization, fixed coupons).
pub mod builder;

/// Cashflow-related traits and aliases.
pub mod traits;

/// Generic schedule-driven interest accrual engine.
pub mod accrual;
pub mod json;

mod serde_defaults;

// -----------------------------------------------------------------------------
// Canonical flow aliases (deduplicated across the cashflow module)
// -----------------------------------------------------------------------------

pub use accrual::{accrued_interest_amount, AccrualConfig, AccrualMethod, ExCouponRule};
pub use aggregation::RecoveryTiming;
pub use builder::CashFlowBuilder;
pub use json::{
    accrued_interest_json, build_cashflow_schedule_envelope_json, build_cashflow_schedule_json,
    dated_flows_json, validate_cashflow_schedule_envelope_json, validate_cashflow_schedule_json,
    CashflowScheduleBuildSpec, CashflowScheduleEnvelope, DatedFlowJson, PrincipalEventSpec,
    CASHFLOW_SCHEDULE_SCHEMA_VERSION,
};
pub use traits::{
    schedule_from_classified_flows, schedule_from_dated_flows, CashflowProvider, ScheduleBuildOpts,
};

pub use finstack_quant_core::dates::Date;
pub use finstack_quant_core::money::Money;

/// Single dated amount in a specific currency.
pub type DatedFlow = (Date, Money);

/// Currency-preserving schedule as a list of dated amounts.
pub type DatedFlows = Vec<DatedFlow>;
