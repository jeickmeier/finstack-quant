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
//! - **End-of-month**: Snap intermediate roll dates to month-end for
//!   month-based frequencies (user-provided start/end are never snapped)
//! - **IMM mode**: Standard IMM quarterly schedules (third Wednesday of Mar/Jun/Sep/Dec)
//! - **CDS IMM mode**: Credit default swap quarterly schedules (20th of Mar/Jun/Sep/Dec)
//! - **Payment / fixing lag**: optional business-day offsets from each period end
//!   (payments) or period start (fixings)
//! - **Deterministic**: Same inputs always produce identical outputs
//! - **Deduplication**: Automatically removes duplicate dates from EOM/stub handling
//!
//! # Quick Example
//!
//! Basic monthly schedule:
//! ```rust
//! use finstack_quant_core::dates::{Schedule, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::April, 15)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .frequency(Tenor::monthly())
//!     .build()?;
//!
//! let dates: Vec<_> = sched.into_iter().collect();
//! assert_eq!(dates.len(), 4); // Jan-15, Feb-15, Mar-15, Apr-15
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! CDS IMM schedule (quarterly on 20-Mar/Jun/Sep/Dec):
//! ```rust
//! use finstack_quant_core::dates::Schedule;
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 20)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .cds_imm()  // Anchors at the CDS roll preceding start (front accrual)
//!     .build()?;
//!
//! let dates: Vec<_> = sched.into_iter().collect();
//! // Dec-20-2024 (prior roll), Mar-20, Jun-20, Sep-20, Dec-20 (2025)
//! assert_eq!(dates.len(), 5);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Standard IMM schedule (quarterly on third Wednesday):
//! ```rust
//! use finstack_quant_core::dates::Schedule;
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 31)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .imm()  // Auto-adjusts start to next IMM date (third Wednesday)
//!     .build()?;
//!
//! let dates: Vec<_> = sched.into_iter().collect();
//! // Jan-15 start plus Mar-19, Jun-18, Sep-17, Dec-17 (2025 third Wednesdays)
//! assert_eq!(dates.len(), 5);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! With business day adjustment:
//! ```rust
//! use finstack_quant_core::dates::{BusinessDayConvention, Schedule, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::June, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 15)?;
//!
//! let sched = Schedule::builder(start, end)?
//!     .frequency(Tenor::monthly())
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
//!   don't divide evenly by the frequency (default)
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

use super::{adjust, next_imm, prev_cds_date, BusinessDayConvention, DateExt, HolidayCalendar};
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
/// use finstack_quant_core::dates::{Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 10)?;
/// let end = Date::from_calendar_date(2025, Month::December, 15)?;
///
/// // Short stub at front
/// let sched = Schedule::builder(start, end)?
///     .frequency(Tenor::quarterly())
///     .stub_rule(StubKind::ShortFront)
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

/// Warning generated during schedule construction.
///
/// Warnings indicate non-fatal issues that occurred during schedule generation.
/// Unlike errors, these allow the schedule to be created but signal that
/// something unexpected happened that callers should be aware of.
///
/// # Use Cases
///
/// - **Graceful fallback**: When [`ScheduleErrorPolicy::GracefulEmpty`] is set and an error
///   would normally occur, the builder returns an empty schedule with a warning
///   describing the original error.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Schedule, ScheduleErrorPolicy, ScheduleWarning, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 1)?;
/// let end = Date::from_calendar_date(2025, Month::January, 20)?; // not a whole month
///
/// // Without a stub rule the misaligned range fails; GracefulEmpty turns the
/// // failure into an empty schedule carrying the error as a warning.
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::monthly())
///     .error_policy(ScheduleErrorPolicy::GracefulEmpty)
///     .build()?;
/// assert!(schedule.dates.is_empty());
/// assert!(matches!(
///     schedule.warnings.as_slice(),
///     [ScheduleWarning::GracefulFallback { .. }]
/// ));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ScheduleWarning {
    /// Schedule generation failed but graceful fallback returned an empty schedule.
    ///
    /// This warning captures the original error message that would have been
    /// returned if graceful fallback mode was not enabled. Callers should
    /// inspect this to understand why the schedule is empty.
    GracefulFallback {
        /// Human-readable description of the error that was suppressed.
        error_message: String,
    },

    /// A calendar ID was provided, but resolution was skipped because
    /// [`ScheduleErrorPolicy::MissingCalendarWarning`] was enabled.
    MissingCalendarId {
        /// The calendar identifier that could not be resolved.
        calendar_id: String,
    },
}

/// Explicit policy for how schedule construction should respond to recoverable issues.
#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScheduleErrorPolicy {
    /// Strict production mode: propagate all errors.
    #[default]
    Strict,
    /// Allow missing calendar IDs and continue with a warning.
    MissingCalendarWarning,
    /// Return an empty schedule with a warning instead of propagating build errors.
    GracefulEmpty,
}

impl std::fmt::Display for ScheduleErrorPolicy {
    /// Writes the serde label (`strict`, `missing_calendar_warning`, `graceful_empty`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for ScheduleErrorPolicy {
    type Err = crate::Error;

    /// Parses the serde label, ignoring surrounding whitespace and ASCII case.
    fn from_str(s: &str) -> crate::Result<Self> {
        crate::wire::serde_parse(&s.trim().to_ascii_lowercase())
    }
}

impl std::fmt::Display for ScheduleWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GracefulFallback { error_message } => {
                write!(f, "graceful fallback triggered: {error_message}")
            }
            Self::MissingCalendarId { calendar_id } => {
                write!(
                    f,
                    "calendar id '{calendar_id}' not found; adjustment skipped"
                )
            }
        }
    }
}

/// Concrete schedule containing generated payment/coupon dates.
///
/// Represents the output of schedule generation: a sequence of dates
/// for cashflows, coupon payments, or other periodic events. Dates are
/// guaranteed to be monotonically increasing with no duplicates.
///
/// # Invariants
///
/// - Dates are strictly increasing (no duplicates)
/// - Empty schedules are allowed (zero-length Vec)
/// - All dates are valid `time::Date` values
///
/// # Warnings
///
/// When using [`ScheduleErrorPolicy::GracefulEmpty`],
/// the schedule may contain warnings that describe issues encountered during
/// generation. Always check [`has_warnings()`](Schedule::has_warnings) when
/// using graceful fallback mode to detect potential pricing issues.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Schedule, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::March, 15)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::monthly())
///     .build()?;
///
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
/// - [`ScheduleWarning`] for warning types
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// Unadjusted accrual grid (period start plus each period end).
    ///
    /// These dates are never business-day adjusted. Payment-date adjustment,
    /// payment lag, and fixing lag live on [`Self::payment_dates`] and
    /// [`Self::fixing_dates`].
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub dates: Vec<Date>,
    /// Payment date for each accrual period (one per period end).
    ///
    /// Length is `dates.len().saturating_sub(1)`. Duplicate payment dates are
    /// retained so the series stays 1:1 with period ends.
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub payment_dates: Vec<Date>,
    /// Fixing dates for each accrual period.
    ///
    /// Empty when no fixing lag is configured; otherwise the same length as
    /// [`Self::payment_dates`].
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub fixing_dates: Vec<Date>,
    /// Warnings generated during schedule construction.
    ///
    /// Non-empty when graceful fallback mode suppressed an error or when
    /// other non-fatal issues occurred during generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ScheduleWarning>,
}

impl Schedule {
    /// Returns `true` if any warnings were generated during schedule construction.
    ///
    /// When using graceful fallback mode, this should be checked to ensure
    /// the schedule was generated successfully. An empty schedule with warnings
    /// indicates a generation error was suppressed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Schedule, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::March, 15)?;
    ///
    /// let schedule = Schedule::builder(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .build()?;
    ///
    /// // Valid schedules have no warnings
    /// assert!(!schedule.has_warnings());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns `true` if schedule generation used graceful fallback.
    ///
    /// This is a convenience method equivalent to checking for the presence
    /// of [`ScheduleWarning::GracefulFallback`] in the warnings.
    #[must_use]
    pub fn used_graceful_fallback(&self) -> bool {
        self.warnings
            .iter()
            .any(|w| matches!(w, ScheduleWarning::GracefulFallback { .. }))
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
pub use spec::ScheduleSpec;
