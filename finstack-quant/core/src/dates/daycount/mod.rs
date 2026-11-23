//! Day-count convention algorithms for fixed income and derivative accrual calculations.
//!
//! This module implements industry-standard day count conventions as defined by
//! ISDA (International Swaps and Derivatives Association) and ICMA (International
//! Capital Market Association). All implementations are panic-free and avoid heap
//! allocation.
//!
//! # Date Interval Convention
//!
//! Endpoint treatment follows each named convention. Actual elapsed-day counts
//! equal `end - start`, while ACT/ACT ISDA and business-day counts allocate days
//! over `[start, end)`. NL/365 removes February 29 in `(start, end]`, and
//! ACT/365L takes its 365/366 denominator from the enclosing period's end year.
//! A date equal to itself always has zero year fraction.
//!
//! # Industry Standards
//!
//! Day count conventions define how interest accrues between two dates. Different
//! markets and instruments use different conventions:
//!
//! # Precision
//!
//! Day and complete-year counts are exact integers; the year fraction is one
//! `f64` conversion of them (ACT/ACT ISDA adds its complete years to at most
//! two partial-year fractions). The whole supported `Date` range is accepted;
//! no bitwise agreement with a vendor or a particular summation order is
//! promised.
//!
//! ## ISDA Standard Conventions
//!
//! - **Actual/360** (Act/360): Money market standard for USD, EUR short-term rates
//! - **Actual/365 Fixed** (Act/365F): GBP money markets and some bond markets
//! - **30/360** (30U/360): US corporate and municipal bonds
//! - **30E/360** (30E/360): Eurobonds and international bonds
//! - **Actual/Actual (ISDA)**: US Treasury bonds, many swap contracts
//!
//! ## ICMA/ISMA Standard Conventions
//!
//! - **Actual/Actual (ICMA)**: International bonds with regular coupon schedules
//!
//! # Supported Conventions
//!
//! - [`DayCount::Act360`] - Actual/360
//! - [`DayCount::Act365F`] - Actual/365 Fixed
//! - [`DayCount::Act365L`] - Actual/365 Leap (ISDA coupon-end-year rule)
//! - [`DayCount::Nl365`] - NL/365 (Actual/365 No Leap)
//! - [`DayCount::Thirty360`] - 30/360 US (Bond Basis)
//! - [`DayCount::ThirtyE360`] - 30E/360 (Eurobond Basis)
//! - [`DayCount::ThirtyE360Isda`] - 30E/360 (ISDA), ISDA 2006 §4.16(h)
//! - [`DayCount::ActAct`] - Actual/Actual (ISDA)
//! - [`DayCount::ActActIsma`] - Actual/Actual (ICMA) regular-period helper
//! - [`DayCount::ActActAfb`] - Actual/Actual AFB (Actual/Actual Euro)
//! - [`DayCount::Thirty360It`] - 30/360 Italian
//! - [`DayCount::Bus252`] - Business/252 (Brazilian and some equity markets)
//!
//! # Examples
//! ```
//! use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
//!
//! let yf = DayCount::ActAct
//!     .year_fraction(start, end, DayCountContext::default())
//!     .expect("Year fraction calculation should succeed");
//! assert!((yf - 1.0).abs() < 1e-9);
//! ```
//!
//! # References
//!
//! - ISDA 2006 Definitions: `docs/REFERENCES.md#isda-2006-definitions`
//! - ICMA Rule Book: `docs/REFERENCES.md#icma-rule-book`
//!
//! # Bus/252 Convention
//!
//! The Bus/252 convention counts business days between dates and divides by 252 (typical trading days per year).
//! This requires a holiday calendar to determine business days. Provide the calendar via `DayCountContext`.
//!
//! ```
//! use finstack_quant_core::dates::{calendar_by_id_strict, Date, DayCount, DayCountContext};
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
//! let calendar = calendar_by_id_strict("target2").expect("built-in calendar");
//!
//! // Calculate year fraction with a calendar in context
//! let yf = DayCount::Bus252
//!     .year_fraction(start, end, DayCountContext { calendar: Some(calendar), ..DayCountContext::default() })
//!     .expect("Year fraction calculation should succeed");
//! ```
//!
//! # ACT/ACT ISMA vs ISDA
//!
//! Both conventions use actual days in numerator and actual days in denominator, but differ in how
//! the denominator is calculated:
//!
//! - **ACT/ACT (ISDA)**: Uses the actual number of days in the year containing the period
//! - **ACT/ACT (ISMA)**: Uses the actual number of days in the coupon period containing the date
//!
//! ```
//! use finstack_quant_core::dates::{Date, DayCount, Tenor, DayCountContext};
//! use time::Month;
//!
//! // Example: 6-month period in a leap year
//! let start = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"); // Leap year
//! let end   = Date::from_calendar_date(2024, Month::July, 1).expect("Valid date");
//!
//! // ACT/ACT (ISDA): 181 days / 366 days (leap year) = 0.4945355191256831
//! let yf_isda = DayCount::ActAct.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
//!
//! // ACT/ACT (ISMA): contained single-period ICMA formula
//! // Returns year fractions: a full 6-month nominal period = 0.5 years
//! let frequency = Tenor::semi_annual(); // Semi-annual
//! let yf_isma = DayCount::ActActIsma
//!     .year_fraction(
//!         start,
//!         end,
//!         DayCountContext {
//!             frequency: Some(frequency),
//!             coupon_period: Some((start, end)),
//!             ..DayCountContext::default()
//!         },
//!     )
//!     .expect("Year fraction calculation should succeed");
//! // yf_isma ≈ 0.5 (one full semi-annual period in years)
//! ```

mod act_act;
pub(crate) mod context;
pub(crate) mod convention;
mod other;
#[cfg(test)]
mod tests;
mod thirty360;

pub(crate) use context::DayCountContext;
#[cfg(test)]
pub(super) use context::DayCountContextState;
