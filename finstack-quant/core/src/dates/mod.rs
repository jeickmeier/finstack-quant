//! Date utilities: business-day adjustments, day-counts, schedules, IMM helpers.
//!
//! This module wraps the [`time`](https://docs.rs/time) crate and exposes
//! domain-specific helpers commonly needed by pricing engines through
//! `finstack_quant_core::dates`. `Date` is re-exported here; `Month`, `Duration`,
//! `Weekday` and the other `time` types are imported from `time` directly.
//!
//! # Highlights
//! - re-export of `time::Date` (import every other `time` type from `time` itself)
//! - holiday calendars and business-day conventions (`adjust`, `HolidayCalendar`)
//! - schedule generation utilities (`ScheduleBuilder`)
//! - IMM/third-Wednesday helpers for derivatives roll dates
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::dates::{
//!     adjust, build_periods, calendar_by_id_strict, BusinessDayConvention, Date, GridMode,
//!     Schedule, StubKind, Tenor, create_date,
//! };
//! use time::{Duration, Month};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! let target2 = calendar_by_id_strict("target2")?;
//! let trade_date = create_date(2024, Month::March, 29)?;
//! let adjusted = adjust(trade_date, BusinessDayConvention::Following, target2)?;
//! assert!(adjusted >= trade_date);
//!
//! let end = trade_date + Duration::days(365);
//! let schedule = Schedule::builder(trade_date, end)?
//!     .frequency(Tenor::quarterly())
//!     .stub_rule(StubKind::ShortBack)
//!     .grid(GridMode::Natural)
//!     .unadjusted()
//!     .build()?;
//! assert!(schedule.get_dates().len() >= 4);
//!
//! let periods = build_periods("2024Q1..Q4", None)?;
//! assert_eq!(periods.len(), 4);
//! # Ok(())
//! # }
//! ```
//!
//! # Holiday Calendars
//!
//! ## Features
//!
//! - **26 built-in market calendars**: major exchanges, central banks, and
//!   settlement systems, generated at build time from `data/calendars/*.json`
//!   (see [`available_calendars`](crate::dates::available_calendars) for the exact identifier list)
//! - **Rule-based definitions**: JSON-defined rules for transparency and auditability
//! - **Cached rule evaluation**: cached years are materialized lazily into a
//!   process-wide holiday bitset and business-day prefix sums; out-of-range
//!   dates continue to scan the calendar's `&'static` rules directly
//! - **Composite calendars**: `+`-joined ids (for example `"nyse+gblo"`) resolve
//!   to a union calendar for multi-currency schedules
//! - **Business day adjustments**: Following, Modified Following, Preceding,
//!   Modified Preceding, Nearest conventions
//!
//! ## Lookup Cost
//!
//! The first lookup for a calendar year inside the cache range materializes
//! a 366-bit raw rule-holiday mask and business-day prefix sums. Subsequent
//! holiday and business-day predicates are constant-time bit lookups, while
//! interval counts combine at most one prefix-sum lookup per year. Dates outside
//! the cache range retain direct rule scanning.
//!
//! The cache is an implementation detail: public predicates are unchanged.
//! [`HolidayCalendar::is_holiday`](crate::dates::HolidayCalendar::is_holiday) still applies each calendar's
//! `ignore_weekends` behavior, and business-day checks still combine raw rule
//! holidays with the calendar's configured weekend rule.
//!
//! ## Supported Date Range
//!
//! The year cache and astronomical/lunar tables cover **1970-2150**. This is
//! a computational range, not certification of market holidays throughout it.
//! Published coverage is calendar-specific: SGSI moving holidays cover 2024-2027,
//! NSE/BSE moving holidays cover 2024-2026, and Matariki dates end in 2052.
//! Consult each calendar's source notes in `data/calendars/*.json` and the
//! independent annual fixtures. Recurring rules outside published coverage may omit closures;
//! the boolean predicate API cannot report an unsupported-year error.
//! Years outside the cache range still evaluate directly and emit a one-time
//! warning; table-based holidays may be unavailable. Trading, settlement,
//! banking/accrual, and fixing-publication calendars are distinct contracts.
//!
//! ## Key Concepts
//!
//! ### Holiday vs. Business Day
//!
//! - **Holiday**: Non-working date as defined by a specific market calendar
//!   (e.g., Christmas, Lunar New Year, bank holidays)
//! - **Business day**: Any day that is not a weekend (Saturday/Sunday) AND not
//!   a market-specific holiday
//!
//! Many calendars include weekends in their holiday definitions for convenience,
//! while others intentionally omit them. Regardless, [`HolidayCalendar::is_business_day`](crate::dates::HolidayCalendar::is_business_day)
//! uses the configured weekend rule (Saturday/Sunday for built-in calendars).
//!
//! **Guideline**: Use `is_business_day` for scheduling and date adjustments.
//! Use `is_holiday` only when you need market-specific holiday information.
//!
//! ## Calendar Types
//!
//! - **Exchange calendars**: `nyse`, `jpx`, `hkex`, `tsx`, `cme`, `eurex`, etc.
//! - **Settlement calendars**: `target2` (Eurozone), `usny` (Federal Reserve
//!   Bank of New York), `sifma` (US bond market), `sofr`
//! - **Country calendars**: banking holidays such as `gblo` (London), `jpto`
//!   (Tokyo) and `defr` (Frankfurt); see [`available_calendars`](crate::dates::available_calendars) for the full list

pub use time::Date;

mod date_extensions;
mod epoch;

pub use date_extensions::DateExt;

mod daycount;
mod volatility_clock;

pub use daycount::context::{DayCountContext, DayCountContextState};
pub use daycount::convention::DayCount;
pub use volatility_clock::VolatilityClock;

pub use calendar::business_days::WEEKENDS_ONLY_ID;
pub use calendar::business_days::canonical_calendar_id;
pub use calendar::business_days::{
    BusinessDayConvention, CalendarMetadata, HolidayCalendar, adjust, available_calendars,
    calendar_by_id_strict,
};
pub use calendar::calendars_generated::WEEKENDS_ONLY;

mod schedule;

pub use schedule::builder::ScheduleBuilder;
pub use schedule::spec::{NominalReference, ScheduleSpec};
pub use schedule::types::{GridMode, Schedule, StubKind};

mod imm;
mod tenor;

pub mod fx;

pub use tenor::{Tenor, TenorUnit};

pub use imm::{
    SifmaSettlementClass, is_cds_date, is_imm_date, next_cds_date, next_imm,
    next_semiannual_cds_maturity, next_sifma_settlement, next_third_friday, prev_cds_date,
    prev_cds_semiannual_roll, sifma_settlement_date, third_friday, third_wednesday,
};

mod calendar;

mod periods;
pub use periods::calendar::PeriodCalendar;
pub use periods::ltm::{LtmWindow, ltm_window};
pub use periods::period_id::{PeriodId, PeriodKind};
pub use periods::plan::{FiscalConfig, Period, PeriodPlan, build_fiscal_periods, build_periods};

pub use epoch::{
    CALENDAR_DAYS_PER_YEAR, create_date, date_from_epoch_days, days_since_epoch, parse_iso_date,
};

#[cfg(test)]
mod tests;
