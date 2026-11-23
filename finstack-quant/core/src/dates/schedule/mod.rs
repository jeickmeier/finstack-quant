//! Date schedule construction for cashflows, coupons, and payment dates.
//!
//! Provides a fluent builder API for constructing deterministic date schedules
//! with support for frequency-based generation, stub periods, end-of-month
//! conventions, and business day adjustments.
//!
//! # Features
//!
//! - **Frequency-based**: Monthly, quarterly, annual, or custom day intervals
//! - **Stub handling**: Short/long stubs at front or back of schedule
//! - **Business day adjustment**: payment dates only (Following, Modified
//!   Following, Preceding, Modified Preceding, Nearest); accrual dates stay
//!   on the unadjusted roll grid
//! - **End-of-month**: a calendar month-end grid for month/year frequencies,
//!   anchored on a month-end regular endpoint (an off-grid anchor is an error)
//! - **IMM mode**: a quarterly IMM grid (third Wednesday of Mar/Jun/Sep/Dec),
//!   anchored on an IMM regular endpoint (an off-grid anchor is an error)
//! - **Payment / fixing lag**: optional business-day offsets from each period end
//!   (payments) or period start (fixings)
//! - **Deterministic**: Same inputs always produce identical outputs
//!
//! Every schedule states its frequency, stub, grid and payment convention;
//! there are no defaults (record CORE-CONVENTIONS-008 #2).
//!
//! # Quick Example
//!
//! Basic monthly schedule:
//! ```rust
//! use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::April, 15)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .frequency(Tenor::monthly())
//!     .stub_rule(StubKind::None)
//!     .grid(GridMode::Natural)
//!     .unadjusted()
//!     .build()?;
//!
//! let dates: Vec<_> = sched.into_iter().collect();
//! assert_eq!(dates.len(), 4); // Jan-15, Feb-15, Mar-15, Apr-15
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Standard IMM schedule (quarterly rolls on the third Wednesday):
//! ```rust
//! use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::March, 19)?; // an IMM date
//! let end = Date::from_calendar_date(2025, Month::December, 31)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .frequency(Tenor::quarterly())
//!     .stub_rule(StubKind::ShortBack)
//!     .grid(GridMode::QuarterlyImm) // regular periods on IMM dates from the start
//!     .unadjusted()
//!     .build()?;
//!
//! let dates: Vec<_> = sched.into_iter().collect();
//! // Mar-19, Jun-18, Sep-17, Dec-17 (2025 third Wednesdays), then the
//! // declared short final stub to Dec-31
//! assert_eq!(dates.len(), 5);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! With business day adjustment:
//! ```rust
//! use finstack_quant_core::dates::{BusinessDayConvention, GridMode, Schedule, StubKind, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::June, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 15)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .frequency(Tenor::monthly())
//!     .stub_rule(StubKind::None)
//!     .grid(GridMode::Natural)
//!     .adjust_with(BusinessDayConvention::ModifiedFollowing, "nyse")
//!     .build()?;
//!
//! // Payment dates are adjusted to NYSE business days
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Stub Conventions
//!
//! When start/end dates don't align exactly with the frequency:
//!
//! - **`StubKind::None`**: Requires exact alignment; errors when start/end
//!   don't divide evenly by the frequency
//! - **`StubKind::ShortFront`**: Short period at start, regular thereafter
//! - **`StubKind::ShortBack`**: Regular periods, short period at end
//! - **`StubKind::LongFront`**: Long period at start, regular thereafter
//! - **`StubKind::LongBack`**: Regular periods, long period at end
//!
//! # See Also
//!
//! - [`ScheduleBuilder`] for the main builder API
//! - [`Tenor`] for payment frequency options
//! - [`StubKind`] for stub period handling
//! - [`BusinessDayConvention`] for date adjustment rules
//!
//! [`BusinessDayConvention`]: super::BusinessDayConvention

pub(crate) mod builder;
mod generation;
pub(crate) mod spec;
pub(crate) mod types;

pub(crate) use spec::ScheduleSpec;
pub(crate) use types::{GridMode, Schedule, StubKind};
