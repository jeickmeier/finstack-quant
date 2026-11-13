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

use smallvec::SmallVec;
use time::Date;

use super::{BusinessDayConvention, DateExt, HolidayCalendar, adjust};
use crate::InputError;

use crate::dates::Tenor;

/// Stub period handling when start/end dates don't align with payment frequency.
///
/// Controls how schedules are generated when the start and end dates don't
/// divide evenly by the payment frequency, resulting in an irregular period
/// (stub) at the beginning or end of the schedule.
///
/// # Variants
///
/// - **`None`**: No stub allowed (default). Generates regular periods from
///   start to end and returns an error
///   ([`InputError::NonIntegerScheduleTenor`]) when the dates don't divide
///   evenly by the frequency. Use a stub variant for misaligned schedules.
///
/// [`InputError::NonIntegerScheduleTenor`]: crate::InputError::NonIntegerScheduleTenor
/// - **`ShortFront`**: Short stub period at the start. Schedule is built
///   backward from the end date, creating a short first period.
/// - **`ShortBack`**: Short stub period at the end. Schedule is built forward
///   from the start date, creating a short final period.
/// - **`LongFront`**: Long stub period at the start. Combines the first two
///   periods into a single longer period.
/// - **`LongBack`**: Long stub period at the end. Combines the last two periods
///   into a single longer period.
///
/// # Financial Context
///
/// Stub conventions are important for:
/// - Interest accrual calculations (short/long first coupons)
/// - Cash flow present value computations
/// - Matching market conventions for specific instruments
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 10)?;
/// let end = Date::from_calendar_date(2025, Month::December, 15)?;
///
/// // Short stub at front
/// let sched = Schedule::builder(start, end)?
///     .frequency(Tenor::quarterly())
///     .stub_rule(StubKind::ShortFront)
///     .grid(GridMode::Natural)
///     .unadjusted()
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`ScheduleBuilder::stub_rule`] to configure stub behavior
#[derive(
    Debug, Clone, Copy, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StubKind {
    /// No stub allowed: start/end must align exactly with the frequency,
    /// otherwise schedule generation returns an error.
    #[default]
    None,
    /// Short stub period at the beginning of the schedule.
    ShortFront,
    /// Short stub period at the end of the schedule (final step truncated to maturity).
    ShortBack,
    /// Long stub period at the beginning of the schedule.
    LongFront,
    /// Long stub period at the end of the schedule (merges final two periods).
    LongBack,
}

impl std::fmt::Display for StubKind {
    /// Writes the serde label (`none`, `short_front`, ...).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for StubKind {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        crate::wire::serde_parse_with_context(
            "Unknown stub kind",
            s,
            &s.trim().to_ascii_lowercase(),
        )
    }
}

/// Regular-grid selection for schedule generation (record
/// CORE-CONVENTIONS-008 #2, #5, #6).
///
/// Exactly one grid is chosen; there is no default. Every regular roll is
/// computed from the original anchor (`start` for no-stub and back-stub
/// policies, `end` for front-stub policies) and then snapped to the grid:
///
/// - **`Natural`**: the date the tenor arithmetic produces (month-end
///   clamping only for month/year tenors).
/// - **`EndOfMonth`**: the last calendar day of each target month. Requires a
///   month/year frequency and a calendar month-end regular anchor.
/// - **`QuarterlyImm`**: the third Wednesday of Mar/Jun/Sep/Dec. Requires a
///   whole number of quarters and an IMM-date regular anchor; the anchor's
///   month sets the phase of 6M, 9M, 1Y and longer frequencies.
///
/// Only the declared stub side of a schedule may be off the grid; with
/// [`StubKind::None`] both endpoints must be on it. These are unadjusted grid
/// dates, not exchange expiries or settlement dates.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::GridMode;
///
/// assert_eq!("end_of_month".parse::<GridMode>()?, GridMode::EndOfMonth);
/// assert_eq!(GridMode::QuarterlyImm.to_string(), "quarterly_imm");
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GridMode {
    /// Tenor-arithmetic dates with no snapping.
    Natural,
    /// Last calendar day of each month.
    EndOfMonth,
    /// Third Wednesday of March, June, September and December.
    QuarterlyImm,
}

impl std::fmt::Display for GridMode {
    /// Writes the serde label (`natural`, `end_of_month`, `quarterly_imm`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for GridMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        crate::wire::serde_parse_with_context(
            "Unknown schedule grid mode",
            s,
            &s.trim().to_ascii_lowercase(),
        )
    }
}

/// Concrete schedule containing generated payment/coupon dates.
///
/// Represents the output of schedule generation: a sequence of dates
/// for cashflows, coupon payments, or other periodic events. A build either
/// returns a complete schedule or an error; there is no empty or warning
/// fallback (CORE-CONVENTIONS-008 #12).
///
/// # Invariants
///
/// Every `Schedule` (built, deserialized or constructed with
/// [`Schedule::try_new`]) satisfies record CORE-CONVENTIONS-008 #13:
///
/// - 2 to 100,000 strictly increasing raw grid dates (both endpoints once);
///   a grid that is not is an error, never repaired by dropping dates
/// - exactly one payment date per period (`dates.len() - 1`)
/// - either no fixing dates or exactly one per period
///
/// Payment and fixing collisions are kept, never deduplicated. Restoration
/// validates this shape only; it does not rebuild against a calendar or
/// prove consistency with a source specification.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::March, 15)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::monthly())
///     .stub_rule(StubKind::None)
///     .grid(GridMode::Natural)
///     .unadjusted()
///     .build()?;
///
/// assert_eq!(schedule.get_dates().len(), 3);
/// // Iterate over dates
/// for date in schedule.into_iter() {
///     println!("Payment date: {}", date);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`ScheduleBuilder`] for constructing schedules
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ScheduleWire")]
pub struct Schedule {
    /// Unadjusted accrual grid (period start plus each period end).
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    dates: Vec<Date>,
    /// Payment date for each accrual period (one per period end).
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    payment_dates: Vec<Date>,
    /// Fixing date for each accrual period, or empty when no fixing lag is
    /// configured.
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    fixing_dates: Vec<Date>,
}

/// Wire form of [`Schedule`]: all three arrays are required (an omitted
/// fixing series is an empty array), and the shape is validated on
/// conversion.
#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ScheduleWire {
    /// Unadjusted accrual grid (period start plus each period end).
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    dates: Vec<Date>,
    /// Payment date for each accrual period (one per period end).
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    payment_dates: Vec<Date>,
    /// Fixing date for each accrual period, or empty when no fixing lag is
    /// configured.
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    fixing_dates: Vec<Date>,
}

impl TryFrom<ScheduleWire> for Schedule {
    type Error = crate::Error;

    fn try_from(wire: ScheduleWire) -> Result<Self, Self::Error> {
        Self::try_new(wire.dates, wire.payment_dates, wire.fixing_dates)
    }
}

impl Schedule {
    /// Build a schedule from its three date series, validating its shape.
    ///
    /// Use this to restore a previously generated schedule; to generate one,
    /// use [`Schedule::builder`]. No calendar or specification is consulted.
    ///
    /// # Arguments
    ///
    /// * `dates` - Unadjusted accrual grid: 2 to 100,000 strictly increasing
    ///   dates, the first and last being the schedule's start and end.
    /// * `payment_dates` - One payment date per period, `dates.len() - 1` in
    ///   all, in period order; collisions are allowed.
    /// * `fixing_dates` - Either empty (no fixing series) or one fixing date
    ///   per period, in period order; collisions are allowed.
    ///
    /// # Errors
    ///
    /// Returns a validation error when any of those shape rules is violated.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::dates::Schedule;
    /// use time::macros::date;
    ///
    /// let grid = vec![date!(2025 - 01 - 15), date!(2025 - 07 - 15)];
    /// let schedule = Schedule::try_new(grid, vec![date!(2025 - 07 - 15)], vec![])?;
    /// assert_eq!(schedule.get_payment_dates().len(), 1);
    /// assert!(Schedule::try_new(vec![date!(2025 - 01 - 15)], vec![], vec![]).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn try_new(
        dates: Vec<Date>,
        payment_dates: Vec<Date>,
        fixing_dates: Vec<Date>,
    ) -> crate::Result<Self> {
        if dates.len() < 2 {
            return Err(crate::Error::Validation(format!(
                "a schedule needs at least two grid dates, got {}",
                dates.len()
            )));
        }
        check_final_count(dates.len())?;
        require_strictly_increasing(&dates)?;
        let periods = dates.len() - 1;
        if payment_dates.len() != periods {
            return Err(crate::Error::Validation(format!(
                "a schedule with {periods} periods needs {periods} payment dates, got {}",
                payment_dates.len()
            )));
        }
        if !fixing_dates.is_empty() && fixing_dates.len() != periods {
            return Err(crate::Error::Validation(format!(
                "a schedule with {periods} periods needs no fixing dates or {periods}, got {}",
                fixing_dates.len()
            )));
        }
        Ok(Self {
            dates,
            payment_dates,
            fixing_dates,
        })
    }

    /// Unadjusted accrual grid (period start plus each period end).
    ///
    /// These dates are never business-day adjusted; payment adjustment and
    /// lags live on [`Self::get_payment_dates`] and fixing lags on
    /// [`Self::get_fixing_dates`].
    pub fn get_dates(&self) -> &[Date] {
        &self.dates
    }

    /// Payment date for each accrual period, one per period end and in
    /// period order. Collisions are retained so the series stays 1:1 with
    /// period ends.
    pub fn get_payment_dates(&self) -> &[Date] {
        &self.payment_dates
    }

    /// Fixing date for each accrual period, or an empty slice when no fixing
    /// lag was configured.
    pub fn get_fixing_dates(&self) -> &[Date] {
        &self.fixing_dates
    }
}

impl IntoIterator for Schedule {
    type Item = Date;
    type IntoIter = std::vec::IntoIter<Date>;
    fn into_iter(self) -> Self::IntoIter {
        self.dates.into_iter()
    }
}

mod builder;
mod generation;
mod spec;
pub use builder::ScheduleBuilder;
use generation::*;
pub use spec::{NominalReference, ScheduleSpec};
