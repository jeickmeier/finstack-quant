//! Holiday calendar system for financial markets.
//!
//! Provides deterministic, high-performance holiday calendars for scheduling
//! cashflows, adjusting payment dates, and validating business days across
//! global financial markets.
//!
//! # Features
//!
//! - **100+ market calendars**: Major exchanges, central banks, and settlement systems
//! - **Rule-based definitions**: JSON-defined rules for transparency and auditability
//! - **Rule evaluation**: [`is_holiday`](HolidayCalendar::is_holiday) checks a
//!   date against the calendar's `&'static` rule set (a short linear scan;
//!   typically a handful of rules per calendar) — no per-date heap allocation
//! - **Composite calendars**: Combine multiple calendars for multi-currency schedules
//! - **Business day adjustments**: Following, Modified Following, Preceding conventions
//!
//! # Supported Date Range
//!
//! Holiday rules are validated for years **1970-2150**. Years outside this range
//! still evaluate via the same rules (a one-time warning is emitted), but their
//! accuracy is not guaranteed.
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
//! while others intentionally omit them. Regardless, [`HolidayCalendar::is_business_day`]
//! always treats Saturday/Sunday as non-business days.
//!
//! **Guideline**: Use `is_business_day` for scheduling and date adjustments.
//! Use `is_holiday` only when you need market-specific holiday information.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_core::dates::{adjust, BusinessDayConvention, HolidayCalendar};
//! use finstack_quant_core::dates::calendar_by_id;
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
//! # Architecture
//!
//! - `rule`: Rule-based holiday definitions (Easter, IMM, lunar calendars)
//! - generated free functions for calendar lookup and discovery
//! - `business_days`: Business day adjustment and counting
//! - `composite`: Multi-calendar union support
//! - `generated`: Build-time generated year-range constants and shared date helpers
//!
//! # See Also
//!
//! - [`HolidayCalendar`] for the core trait
//! - [`calendar_by_id`] for calendar lookup by code
//! - [`available_calendars`] for discovery of supported calendar identifiers
//! - `BusinessDayConvention` for adjustment conventions
//! - `CompositeCalendar` for combining calendars

pub(crate) mod algo;
pub(crate) mod business_days;
pub(crate) mod composite;
pub(crate) mod generated;
pub(crate) mod rule;
pub(crate) mod types;

// -----------------------------------------------------------------------------
// Public re-exports
// -----------------------------------------------------------------------------

// `finstack_quant_core::dates::*` is the canonical facade for adjustment, registry, and
// calendar traits. This namespace keeps the generated calendars and related
// implementation types available for callers that need them directly.
pub use business_days::{adjust, available_calendars, BusinessDayConvention, HolidayCalendar};
pub use composite::{CompositeCalendar, CompositeMode};
pub use rule::{Direction, Observed, Rule};
pub use types::{Calendar, WeekendRule};

// Include generated calendar implementations.
//
// Important: wrap the include so its internal `use ...` imports don't collide
// with our public re-export facade above.
mod calendars_generated {
    include!(concat!(env!("OUT_DIR"), "/calendars.rs"));
}

pub use calendars_generated::*;

/// Shared calendar that treats only Saturdays and Sundays as non-business days.
///
/// This is the explicit fallback for APIs whose calendar identifier is optional.
pub static WEEKENDS_ONLY: Calendar = Calendar::new("weekends_only", "Weekends Only", true, &[]);

/// Resolve typed calendar identifiers strictly and preserve their input order.
///
/// # Errors
///
/// Returns an error naming the first unknown identifier. Unlike the removed
/// registry helper, unknown identifiers are never silently dropped.
pub fn calendars_by_ids(
    ids: &[crate::types::CalendarId],
) -> crate::Result<Vec<&'static dyn HolidayCalendar>> {
    ids.iter()
        .map(|id| {
            calendar_by_id(id.as_str()).ok_or_else(|| {
                crate::Error::calendar_not_found_with_suggestions(
                    id.as_str(),
                    available_calendars(),
                )
            })
        })
        .collect()
}
