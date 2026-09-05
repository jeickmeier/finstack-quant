#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Cashflow schedule construction, accrual, and currency-preserving aggregation.
//!
//! [`builder`] creates currency-tagged [`builder::CashFlowSchedule`] values;
//! [`accrual`] and [`aggregation`] provide schedule-level calculations;
//! [`traits`] integrates instrument cashflow providers; and [`json`] plus
//! [`schema`] expose serde and schema boundaries. Instrument pricing belongs in
//! the valuation crates.
//!
//! # Conventions
//!
//! Amounts retain their [`Money`] currency, and checked aggregation does not
//! perform FX conversion. Coupon rates are decimals (`0.05` means 5%); fields
//! ending in `_bp` are basis points. Schedule dates follow the day-count,
//! calendar, business-day, stub, roll, and payment-lag rules in
//! [`builder::ScheduleParams`]. [`primitives::CFKind`] is non-exhaustive, so
//! downstream matches require a catch-all arm.
//!
//! # Errors
//!
//! Fallible builders, checked aggregation, and JSON helpers return
//! `finstack_quant_core::Error` for malformed or incomplete inputs, invalid
//! dates, currency mismatches, and missing floating-rate market data when a
//! leg's fallback policy requires it.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_cashflows::builder::{
//!     CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
//! };
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::money::Money;
//! use rust_decimal_macros::dec;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = Date::from_calendar_date(2025, Month::January, 15)?;
//! let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
//!
//! let schedule = CashFlowSchedule::builder()
//!     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
//!     .fixed_cf(FixedCouponSpec {
//!         coupon_type: CouponType::Cash,
//!         rate: dec!(0.05),
//!         schedule: ScheduleParams::semiannual_30360(),
//!     })
//!     .build(None)?;
//!
//! assert!(!schedule.get_flows().is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - Day-count and schedule conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market accrued-interest conventions: `docs/REFERENCES.md#icma-rule-book`

/// Cash-flow primitives (`CashFlow`, `CFKind`).
pub mod primitives {
    pub use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual};

    /// Returns whether a classified flow represents a cash settlement.
    ///
    /// PIK is a capitalization event and `DefaultedNotional` is a write-down;
    /// neither should be emitted by APIs whose contract is dated cash.
    ///
    /// # Arguments
    ///
    /// * `kind` - Classified cashflow kind to test; `PIK` and
    ///   `DefaultedNotional` return `false`, while all settlement kinds return
    ///   `true`.
    pub fn is_cash_settlement_kind(kind: CFKind) -> bool {
        !matches!(kind, CFKind::Pik | CFKind::DefaultedNotional)
    }
}

/// Currency-preserving aggregation utilities for cashflows.
pub mod aggregation;

/// Composable cashflow builder.
pub mod builder;

/// Cashflow-related traits and aliases.
pub mod traits;

/// Generic schedule-driven interest accrual engine.
pub mod accrual;
pub mod json;
#[cfg(feature = "json-schema")]
pub mod schema;

/// Serde default helpers shared by the spec types and host bindings.
pub mod serde_defaults;

// Canonical flow aliases (deduplicated across the cashflow module)

pub use accrual::{
    accrued_interest_amount, AccrualConfig, AccrualIndex, AccrualMethod, ExCouponRule,
};
pub use aggregation::PeriodAggregation;
pub use builder::CashFlowBuilder;
// Prepayment/default rate-convention conversions, re-exported at the crate
// root so host bindings can expose them flat beside the JSON bridge while the
// canonical definitions stay in `builder::credit_rates`.
pub use builder::{cdr_to_mdr, cpr_to_smm, mdr_to_cdr, smm_to_cpr};
pub use json::{
    accrued_interest, build_cashflow_schedule_json, dated_flows_json,
    validate_cashflow_schedule_json, CashflowScheduleBuildSpec, CouponLegSpec, DatedFlowJson,
    PaymentProgramSpec,
};
pub use traits::{
    schedule_from_classified_flows, schedule_from_dated_flows, CashflowProvider,
    CashflowScheduleSource, ScheduleBuildOpts,
};

pub use finstack_quant_core::dates::Date;
pub use finstack_quant_core::money::Money;

/// Single dated amount in a specific currency.
pub type DatedFlow = (Date, Money);

/// Currency-preserving schedule as a list of dated amounts.
pub type DatedFlows = Vec<DatedFlow>;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
