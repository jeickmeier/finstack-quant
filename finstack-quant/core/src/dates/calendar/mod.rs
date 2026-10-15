//! Holiday calendar system for financial markets.
//!
//! Provides deterministic, high-performance holiday calendars for scheduling
//! cashflows, adjusting payment dates, and validating business days across
//! global financial markets.
//!
//! # Features
//!
//! - **27 built-in market calendars**: major exchanges, central banks, and
//!   settlement systems, generated at build time from `data/calendars/*.json`
//!   (see [`super::available_calendars`] for the exact identifier list)
//! - **Rule-based definitions**: JSON-defined rules for transparency and auditability
//! - **Cached rule evaluation**: cached years are materialized lazily into a
//!   process-wide holiday bitset and business-day prefix sums; out-of-range
//!   dates continue to scan the calendar's `&'static` rules directly
//! - **Composite calendars**: Combine multiple calendars for multi-currency schedules
//! - **Business day adjustments**: Following, Modified Following, Preceding,
//!   Modified Preceding, Nearest conventions
//!
//! # Lookup Cost
//!
//! The first lookup for a calendar year inside the cache range materializes
//! a 366-bit raw rule-holiday mask and business-day prefix sums. Subsequent
//! holiday and business-day predicates are constant-time bit lookups, while
//! interval counts combine at most one prefix-sum lookup per year. Dates outside
//! the cache range retain direct rule scanning.
//!
//! The cache is an implementation detail: public predicates are unchanged.
//! [`super::HolidayCalendar::is_holiday`] still applies each calendar's
//! `ignore_weekends` behavior, and business-day checks still combine raw rule
//! holidays with the calendar's configured weekend rule.
//!
//! # Supported Date Range
//!
//! The year cache and astronomical/lunar tables cover **1970-2150**. This is
//! a computational range, not certification of market holidays throughout it.
//! Published coverage is calendar-specific: SGSI moving holidays cover 2024-2027,
//! NSE/BSE moving holidays cover 2024-2026, and Matariki dates end in 2052.
//! Consult each generated calendar constant's source notes and the independent
//! annual fixtures. Recurring rules outside published coverage may omit closures;
//! the boolean predicate API cannot report an unsupported-year error.
//! Years outside the cache range still evaluate directly and emit a one-time
//! warning; table-based holidays may be unavailable. Trading, settlement,
//! banking/accrual, and fixing-publication calendars are distinct contracts.
//!
//! # Key Concepts
//!
//! ## Holiday vs. Business Day
//!
//! - **Holiday**: Non-working date as defined by a specific market calendar
//!   (e.g., Christmas, Lunar New Year, bank holidays)
//! - **Business day**: Any day that is not a weekend (Saturday/Sunday) AND not
//!   a market-specific holiday
//!
//! Many calendars include weekends in their holiday definitions for convenience,
//! while others intentionally omit them. Regardless, [`super::HolidayCalendar::is_business_day`]
//! uses the configured weekend rule (Saturday/Sunday for built-in calendars).
//!
//! **Guideline**: Use `is_business_day` for scheduling and date adjustments.
//! Use `is_holiday` only when you need market-specific holiday information.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_core::dates::{
//!     adjust, calendar_by_id, BusinessDayConvention, HolidayCalendar,
//! };
//! use time::{Date, Month};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! // Get New York Stock Exchange calendar
//! let nyse = calendar_by_id("nyse")
//!     .ok_or("NYSE calendar not found")?;
//!
//! // Check if a date is a business day
//! let date = Date::from_calendar_date(2025, Month::December, 25)?;
//! assert!(!nyse.is_business_day(date)); // Christmas is not a business day
//!
//! // Adjust date to next business day
//! let adjusted = adjust(date, BusinessDayConvention::Following, nyse)?;
//! assert_eq!(adjusted, Date::from_calendar_date(2025, Month::December, 26)?);
//! # Ok(())
//! # }
//! ```
//!
//! # Calendar Types
//!
//! - **Exchange calendars**: NYSE, LSE, TSE, HKEX, etc.
//! - **Settlement calendars**: TARGET (Eurozone), USGS (US Government Securities)
//! - **Central bank calendars**: Federal Reserve, ECB, BOE, BOJ
//! - **Country calendars**: Nationwide holidays (US, UK, JP, etc.)
//!
//! # See Also
//!
//! - [`super::HolidayCalendar`] for the core trait
//! - [`super::calendar_by_id`] for calendar lookup by code
//! - [`super::available_calendars`] for discovery of supported calendar identifiers
//! - [`super::BusinessDayConvention`] for adjustment conventions
//! - [`super::CompositeCalendar`] for combining calendars

pub(crate) mod algo;
pub(crate) mod business_days;
pub(crate) mod composite;
mod jurisdictions;
pub(crate) mod rule;
pub(crate) mod types;
mod year_cache;

// Isolate generated `use` imports from this module's public re-exports.
pub(super) mod calendars_generated {
    include!(concat!(env!("OUT_DIR"), "/calendars.rs"));
}

pub use calendars_generated::*;
