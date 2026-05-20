#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
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
//! Provides comprehensive cashflow modeling infrastructure for bonds, swaps,
//! loans, and structured products. All cashflows use currency-tagged `Money`
//! types to prevent accidental cross-currency arithmetic.
//!
//! # Core Components
//!
//! - **Primitives**: Fundamental `CashFlow` type with date, amount, and classification
//! - **Builder**: Composable schedule builder for coupons, principal, and amortization
//! - **Aggregation**: Currency-preserving cashflow merging and rollup
//! - **Traits**: `CashflowProvider` trait for instruments to expose schedules
//!
//! # Currency Safety
//!
//! All aggregation operations enforce currency matching:
//! - Cannot sum cashflows in different currencies without explicit FX
//! - Currency preserved through aggregation pipeline
//! - FX conversion requires explicit policy
//!
//! # Conventions
//!
//! - Coupon rates are generally decimals (for example `0.05` for 5%).
//! - Spreads and fee quotes are often expressed in basis points on the
//!   specification types.
//! - Schedule timing depends on explicit day-count, business-day, calendar, and
//!   lag settings carried by the builder specs.
//! - `CFKind` behavior should be read from the authoritative
//!   `finstack_core::cashflow::CFKind` definition rather than copied here. The
//!   enum is `#[non_exhaustive]` and downstream logic should not assume a fixed
//!   exhaustive list of variants.
//!
//! # Examples
//!
//! ## Building a Cashflow Schedule
//!
//! ```rust
//! use finstack_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
//! use finstack_core::money::Money;
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
//! use finstack_cashflows::aggregation::aggregate_cashflows_checked;
//! use finstack_core::money::Money;
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::create_date;
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
//! ## Periodized Present Value Aggregation
//!
//! Compute present values aggregated by reporting period for analysis and reconciliation.
//!
//! ### From a Schedule (Recommended In This Crate)
//!
//! The stable public interface in this crate is the schedule-level API on
//! [`builder::CashFlowSchedule`]:
//!
//! ```rust,no_run
//! use finstack_cashflows::builder::CashFlowSchedule;
//! use finstack_cashflows::aggregation::DateContext;
//! use finstack_cashflows::builder::PvDiscountSource;
//! use finstack_core::dates::{Date, DayCount, DayCountContext, Period};
//! use finstack_core::market_data::traits::Discounting;
//!
//! fn periodized_pv(
//!     schedule: &CashFlowSchedule,
//!     periods: &[Period],
//!     disc: &dyn Discounting,
//!     base: Date,
//! ) -> finstack_core::Result<()> {
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
//! ### From Higher-Level Instruments
//!
//! Higher-level instrument adapters may exist in other crates, such as
//! `finstack-valuations`, but they are outside this crate's direct public API.
//!
//! # See Also
//!
//! - `primitives` for core `CashFlow` type from finstack-core
//! - `builder` for schedule construction
//! - `aggregation` for currency-safe dated-flow aggregation
//! - `CashflowProvider` and `schedule_from_dated_flows` for schedule interfaces

/// Cash-flow primitives (`CashFlow`, `CFKind`).
pub mod primitives {
    pub use finstack_core::cashflow::{CFKind, CashFlow};
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
    accrued_interest_json, build_cashflow_schedule_json, dated_flows_json,
    validate_cashflow_schedule_json, CashflowScheduleBuildSpec, DatedFlowJson, PrincipalEventSpec,
};
pub use traits::{
    schedule_from_classified_flows, schedule_from_dated_flows, CashflowProvider, ScheduleBuildOpts,
};

pub use finstack_core::dates::Date;
pub use finstack_core::money::Money;

/// Single dated amount in a specific currency.
pub type DatedFlow = (Date, Money);

/// Currency-preserving schedule as a list of dated amounts.
pub type DatedFlows = Vec<DatedFlow>;
