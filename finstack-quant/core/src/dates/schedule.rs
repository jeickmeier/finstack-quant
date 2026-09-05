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
//! use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::April, 15)?;
//!
//! let sched = ScheduleBuilder::new(start, end)?
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
//! use finstack_quant_core::dates::ScheduleBuilder;
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 20)?;
//!
//! let sched = ScheduleBuilder::new(start, end)?
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
//! use finstack_quant_core::dates::ScheduleBuilder;
//! use time::{Date, Month};
//!
//! let start = Date::from_calendar_date(2025, Month::January, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 31)?;
//!
//! let sched = ScheduleBuilder::new(start, end)?
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
//! use finstack_quant_core::dates::{calendar_by_id, ScheduleBuilder, Tenor, BusinessDayConvention};
//! use time::{Date, Month};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! let start = Date::from_calendar_date(2025, Month::June, 15)?;
//! let end = Date::from_calendar_date(2025, Month::December, 15)?;
//! let nyse = calendar_by_id("nyse")
//!     .ok_or("NYSE calendar not found")?;
//!
//! let sched = ScheduleBuilder::new(start, end)?
//!     .frequency(Tenor::monthly())
//!     .adjust_with(BusinessDayConvention::ModifiedFollowing, nyse)
//!     .build()?;
//!
//! // Dates are adjusted to business days according to NYSE calendar
//! # Ok(())
//! # }
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
use time::{Date, Duration};

use super::{adjust, next_imm, prev_cds_date, BusinessDayConvention, DateExt, HolidayCalendar};
use crate::error::InputError;

/// Payment or coupon frequency for schedule generation.
///
/// This is a re-export of [`crate::dates::Tenor`] documented here because it is
/// the canonical schedule frequency type used by [`ScheduleBuilder`].
///
/// Month-based tenors (for example monthly or quarterly) advance by calendar
/// months and therefore interact with end-of-month rules. Day-based tenors
/// (for example weekly) advance by a fixed number of days.
///
/// # Common usages
///
/// - Month-based coupon schedules such as monthly, quarterly, or semi-annual
/// - Day-based operational schedules such as weekly or biweekly
/// - ACT/ACT (ICMA) frequency metadata via [`crate::dates::DayCountContext`]
///
/// # Examples
///
/// Using predefined tenor constructors:
/// ```rust
/// use finstack_quant_core::dates::Tenor;
///
/// let quarterly = Tenor::quarterly();
/// assert_eq!(quarterly.months(), Some(3));
///
/// let weekly = Tenor::weekly();
/// assert_eq!(weekly.days(), Some(7));
/// ```
///
/// Creating from payments per year:
/// ```rust
/// use finstack_quant_core::dates::Tenor;
///
/// // 4 payments per year = quarterly
/// let frequency = Tenor::from_payments_per_year(4)?;
/// assert_eq!(frequency, Tenor::quarterly());
///
/// // 2 payments per year = semi-annual
/// let frequency = Tenor::from_payments_per_year(2)?;
/// assert_eq!(frequency, Tenor::semi_annual());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # See Also
///
/// - [`ScheduleBuilder::frequency`] to use with schedule builder
/// - [`crate::dates::DayCountContext`] for conventions that also require frequency metadata
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
/// [`InputError::NonIntegerScheduleTenor`]: crate::error::InputError::NonIntegerScheduleTenor
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
/// use finstack_quant_core::dates::{ScheduleBuilder, Tenor, StubKind};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 10)?;
/// let end = Date::from_calendar_date(2025, Month::December, 15)?;
///
/// // Short stub at front
/// let sched = ScheduleBuilder::new(start, end)?
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StubKind::None => write!(f, "none"),
            StubKind::ShortFront => write!(f, "short_front"),
            StubKind::ShortBack => write!(f, "short_back"),
            StubKind::LongFront => write!(f, "long_front"),
            StubKind::LongBack => write!(f, "long_back"),
        }
    }
}

impl std::str::FromStr for StubKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "none" => Ok(StubKind::None),
            "short_front" => Ok(StubKind::ShortFront),
            "short_back" => Ok(StubKind::ShortBack),
            "long_front" => Ok(StubKind::LongFront),
            "long_back" => Ok(StubKind::LongBack),
            other => Err(format!("Unknown stub kind: {}", other)),
        }
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
/// use finstack_quant_core::dates::{ScheduleBuilder, Tenor, ScheduleWarning};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::December, 31)?;
/// let end = Date::from_calendar_date(2025, Month::January, 1)?; // Invalid: end before start
///
/// // Invalid date ranges are rejected by new() before an error policy applies.
/// // rather than an error. Note: new() itself returns Result, so we handle the error
/// let result = ScheduleBuilder::new(start, end);
/// assert!(result.is_err()); // new() validates start <= end
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
/// use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::March, 15)?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
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
pub struct Schedule {
    /// Unadjusted accrual grid (period start plus each period end).
    ///
    /// These dates are never business-day adjusted. Payment-date adjustment,
    /// payment lag, and fixing lag live on [`Self::payment_dates`] and
    /// [`Self::fixing_dates`].
    #[serde(with = "crate::wire::dates")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub dates: Vec<Date>,
    /// Payment date for each accrual period (one per period end).
    ///
    /// Length is `dates.len().saturating_sub(1)`. Duplicate payment dates are
    /// retained so the series stays 1:1 with period ends.
    #[serde(default, with = "crate::wire::dates")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub payment_dates: Vec<Date>,
    /// Fixing dates for each accrual period.
    ///
    /// Empty when no fixing lag is configured; otherwise the same length as
    /// [`Self::payment_dates`].
    #[serde(default, with = "crate::wire::dates")]
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
    /// use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::March, 15)?;
    ///
    /// let schedule = ScheduleBuilder::new(start, end)?
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

/// Fluent builder for constructing date schedules with full configurability.
///
/// Provides a type-safe, fluent API for generating payment/coupon schedules
/// with support for frequency, stub periods, business day adjustments, and
/// end-of-month conventions.
///
/// # Configuration Options
///
/// - **Frequency**: Monthly, quarterly, annual, or day-based intervals
/// - **Stub handling**: Short/long stubs at front or back
/// - **Business day adjustment**: Following, Modified Following, Preceding
/// - **End-of-month**: Snap to last day of month for month-based frequencies
/// - **IMM mode**: Standard IMM quarterly schedule (third Wednesday of Mar/Jun/Sep/Dec)
/// - **CDS IMM mode**: CDS quarterly schedule (20th of Mar/Jun/Sep/Dec)
///
/// # Construction Flow
///
/// 1. Create builder with `new(start, end)`
/// 2. Configure options via fluent methods
/// 3. Call `build()` to generate the [`Schedule`]
///
/// # Examples
///
/// Basic quarterly schedule:
/// ```rust
/// use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::March, 20)?;
/// let end = Date::from_calendar_date(2025, Month::December, 20)?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
///     .frequency(Tenor::quarterly())
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// With business day adjustment:
/// ```rust
/// use finstack_quant_core::dates::{calendar_by_id, ScheduleBuilder, Tenor, BusinessDayConvention};
/// use time::{Date, Month};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::December, 15)?;
/// let nyse = calendar_by_id("nyse")
///     .ok_or("NYSE calendar not found")?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
///     .frequency(Tenor::monthly())
///     .adjust_with(BusinessDayConvention::ModifiedFollowing, nyse)
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// CDS IMM schedule (credit default swaps):
/// ```rust
/// use finstack_quant_core::dates::ScheduleBuilder;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2026, Month::December, 20)?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
///     .cds_imm()  // Quarterly on 20-Mar/Jun/Sep/Dec
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Standard IMM schedule (futures):
/// ```rust
/// use finstack_quant_core::dates::ScheduleBuilder;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::December, 31)?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
///     .imm()  // Quarterly on third Wednesday of Mar/Jun/Sep/Dec
///     .build()?;
/// // Generates: Mar-19, Jun-18, Sep-17, Dec-17 (2025 third Wednesdays)
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// End-of-month convention:
/// ```rust
/// use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 31)?;
/// let end = Date::from_calendar_date(2025, Month::June, 30)?;
///
/// let schedule = ScheduleBuilder::new(start, end)?
///     .frequency(Tenor::monthly())
///     .end_of_month(true)  // Snap to month-end
///     .build()?;
///
/// // Generates: Jan-31, Feb-28, Mar-31, Apr-30, May-31, Jun-30
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`Tenor`] for payment frequency options
/// - [`StubKind`] for stub period handling
/// - [`BusinessDayConvention`] for adjustment rules
///
/// [`BusinessDayConvention`]: super::BusinessDayConvention
#[derive(Clone)]
pub struct ScheduleBuilder<'a> {
    start: Date,
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    conv: Option<BusinessDayConvention>,
    /// Borrowed calendar (set by [`adjust_with`](Self::adjust_with)).
    /// Mutually exclusive with [`Self::deferred_calendar_id`].
    cal: Option<&'a dyn HolidayCalendar>,
    /// Calendar ID to be resolved at [`build`](Self::build) time.
    ///
    /// Set by [`adjust_with_id`](Self::adjust_with_id) when the caller has a
    /// string ID rather than a borrowed `&dyn HolidayCalendar`. Deferred
    /// resolution is intentional: it lets the build path apply
    /// [`ScheduleErrorPolicy`] uniformly (strict / warning / graceful) when
    /// the registry lookup fails, instead of forcing every binding caller
    /// (Python, WASM) to thread the registry + error-policy themselves.
    /// Mutually exclusive with [`Self::cal`].
    deferred_calendar_id: Option<String>,
    eom: bool,
    /// Standard IMM mode (third Wednesday of Mar/Jun/Sep/Dec) for futures.
    imm_mode: bool,
    /// CDS IMM mode (20th of Mar/Jun/Sep/Dec) for credit default swaps.
    cds_imm_mode: bool,
    error_policy: ScheduleErrorPolicy,
    /// Business days after each (adjusted) period end for the payment date.
    payment_lag_business_days: i32,
    /// Optional T-minus business days from each period's accrual start.
    fixing_lag_business_days: Option<i32>,
}

impl<'a> ScheduleBuilder<'a> {
    /// Create a new builder with mandatory `start` and `end` dates.
    ///
    /// Defaults: frequency = Monthly, stub = None, no adjustment, no EOM.
    ///
    /// # Errors
    ///
    /// Returns `Err(InputError::InvalidDateRange)` if `start > end`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{ScheduleBuilder, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::April, 15)?;
    ///
    /// let schedule = ScheduleBuilder::new(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(start: Date, end: Date) -> crate::Result<Self> {
        if start > end {
            return Err(crate::error::InputError::InvalidDateRange.into());
        }
        Ok(Self {
            start,
            end,
            frequency: Tenor::monthly(),
            stub: StubKind::None,
            conv: None,
            cal: None,
            deferred_calendar_id: None,
            eom: false,
            imm_mode: false,
            cds_imm_mode: false,
            error_policy: ScheduleErrorPolicy::Strict,
            payment_lag_business_days: 0,
            fixing_lag_business_days: None,
        })
    }

    /// Set coupon/payment frequency.
    #[must_use]
    pub fn frequency(mut self, frequency: Tenor) -> Self {
        self.frequency = frequency;
        self
    }

    /// Set stub handling rule.
    ///
    /// # Arguments
    ///
    /// * `stub` - Stub policy controlling irregular first or final schedule periods.
    #[must_use]
    pub fn stub_rule(mut self, stub: StubKind) -> Self {
        self.stub = stub;
        self
    }

    /// Configure business-day adjustment using `conv` and `cal`.
    ///
    /// # Arguments
    ///
    /// * `conv` - Business-day convention applied when an unadjusted date is not a business day.
    /// * `cal` - Holiday calendar used for business-day adjustment.
    #[must_use]
    pub fn adjust_with(
        mut self,
        conv: BusinessDayConvention,
        cal: &'a dyn HolidayCalendar,
    ) -> Self {
        self.conv = Some(conv);
        self.cal = Some(cal);
        self
    }

    /// Enable End-of-Month (EOM) convention.
    ///
    /// When enabled, computed intermediate roll dates are snapped to the
    /// last day of their month. The user-provided start and end dates are
    /// contractual and are never snapped.
    /// EOM requires a month/year tenor and cannot be combined with IMM or CDS IMM.
    ///
    /// # Arguments
    ///
    /// * `eom` - Whether to snap intermediate dates to month-end; incompatible
    ///   tenor or generation-rule combinations are rejected by `build`.
    #[must_use]
    pub fn end_of_month(mut self, eom: bool) -> Self {
        self.eom = eom;
        self
    }

    /// Create a CDS IMM schedule (quarterly on the 20th: 20-Mar, 20-Jun, 20-Sep, 20-Dec).
    /// This is a convenience method for credit default swap schedules that follow
    /// standard CDS roll dates.
    ///
    /// # Front accrual
    ///
    /// Per post-Big-Bang (2009) market convention, when the start date is not
    /// itself a CDS roll date the schedule is anchored at the roll date
    /// immediately **preceding** the start, so the first period carries the
    /// standard front accrual (the first generated date lies before `start`).
    ///
    /// # Adjustment
    ///
    /// The generated 20ths are **unadjusted** roll dates; payment-date
    /// business-day adjustment is applied separately via
    /// [`adjust_with`](Self::adjust_with) / [`adjust_with_id`](Self::adjust_with_id).
    #[must_use]
    pub fn cds_imm(mut self) -> Self {
        self.frequency = Tenor::quarterly();
        self.stub = StubKind::ShortBack;
        self.cds_imm_mode = true;
        self.imm_mode = false;
        self
    }

    /// Create a standard IMM schedule (quarterly on third Wednesday: Mar, Jun, Sep, Dec).
    ///
    /// This is used for interest rate futures (Eurodollar, SOFR), currency futures,
    /// and equity index futures that follow CME IMM roll conventions.
    ///
    /// Unlike [`cds_imm()`](Self::cds_imm) which uses the 20th of quarterly months,
    /// standard IMM dates fall on the third Wednesday.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::dates::ScheduleBuilder;
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::December, 31)?;
    ///
    /// let schedule = ScheduleBuilder::new(start, end)?
    ///     .imm()  // Quarterly on third Wednesday
    ///     .build()?;
    ///
    /// // Generates: Mar-19, Jun-18, Sep-17, Dec-17 (2025 third Wednesdays)
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn imm(mut self) -> Self {
        self.frequency = Tenor::quarterly();
        self.stub = StubKind::ShortBack;
        self.imm_mode = true;
        self.cds_imm_mode = false;
        self
    }

    /// Configure how recoverable schedule-construction errors are handled.
    ///
    /// # Arguments
    ///
    /// * `policy` - Policy enum controlling error handling, unmatched keys, or fallbacks
    #[must_use]
    pub fn error_policy(mut self, policy: ScheduleErrorPolicy) -> Self {
        self.error_policy = policy;
        self
    }

    /// Shift each payment date by `lag` business days after the (adjusted)
    /// period end.
    ///
    /// A lag of zero leaves the payment date on the adjusted period end.
    /// A positive lag requires a holiday calendar from
    /// [`adjust_with`](Self::adjust_with) or [`adjust_with_id`](Self::adjust_with_id).
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative business-day delay from each period's payment
    ///   anchor to the actual payment date. Zero is T+0 (pay on the adjusted
    ///   end). Negative values are rejected at [`build`](Self::build).
    #[must_use]
    pub fn payment_lag_business_days(mut self, lag: i32) -> Self {
        self.payment_lag_business_days = lag;
        self
    }

    /// Set a T-minus fixing lag from each period's unadjusted accrual start.
    ///
    /// The fixing date is `accrual_start` minus `lag` business days. A lag of
    /// zero stores the accrual start itself. A positive lag requires a holiday
    /// calendar from [`adjust_with`](Self::adjust_with) or
    /// [`adjust_with_id`](Self::adjust_with_id).
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative business-day lookback from each period's accrual
    ///   start. Negative values are rejected at [`build`](Self::build).
    #[must_use]
    pub fn fixing_lag_business_days(mut self, lag: i32) -> Self {
        self.fixing_lag_business_days = Some(lag);
        self
    }

    /// Configure business-day adjustment using calendar ID string lookup.
    ///
    /// This is a convenience method that combines calendar lookup with adjustment
    /// configuration. The calendar lookup is performed at build time.
    ///
    /// # Errors
    ///
    /// By default, returns an error at [`build()`](Self::build) time if the calendar ID
    /// is not found. Use [`error_policy`](Self::error_policy) with
    /// [`ScheduleErrorPolicy::MissingCalendarWarning`] to opt into lenient behavior.
    ///
    /// # Arguments
    ///
    /// * `conv` - Business day convention (Following, Modified Following, etc.)
    /// * `calendar_id` - Calendar identifier string (e.g., "nyse", "target2", "gblo")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{ScheduleBuilder, Tenor, BusinessDayConvention};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::December, 15).expect("Valid date");
    ///
    /// let schedule = ScheduleBuilder::new(start, end)
    ///     .expect("Valid dates")
    ///     .frequency(Tenor::monthly())
    ///     .adjust_with_id(BusinessDayConvention::Following, "nyse")
    ///     .build()
    ///     .expect("Schedule builder should succeed");
    /// # assert!(schedule.dates.len() > 0);
    /// ```
    #[must_use]
    pub fn adjust_with_id(mut self, conv: BusinessDayConvention, calendar_id: &str) -> Self {
        self.conv = Some(conv);
        self.deferred_calendar_id = Some(calendar_id.to_string());
        self
    }

    /// Build a concrete schedule (adjusted if configured).
    ///
    /// When [`ScheduleErrorPolicy::GracefulEmpty`] is selected,
    /// this method returns an empty schedule with a [`ScheduleWarning::GracefulFallback`]
    /// warning instead of propagating errors. Always check [`Schedule::has_warnings()`]
    /// when using graceful mode to detect potential pricing issues.
    ///
    /// Under [`ScheduleErrorPolicy::Strict`] the build **fails closed on
    /// warnings**: a schedule that would carry any [`ScheduleWarning`] is
    /// rejected with a validation error instead of being returned.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Start date is after end date (and graceful mode is disabled)
    /// - Calendar lookup fails under [`ScheduleErrorPolicy::Strict`]
    /// - Any warning is produced under [`ScheduleErrorPolicy::Strict`]
    /// - EOM is combined with a day/week tenor or an IMM/CDS IMM rule
    pub fn build(self) -> crate::Result<Schedule> {
        if self.eom
            && (self.imm_mode
                || self.cds_imm_mode
                || !matches!(
                    self.frequency.unit(),
                    super::TenorUnit::Months | super::TenorUnit::Years
                ))
        {
            return Err(crate::Error::Validation(
                "end-of-month requires a month/year tenor and cannot be combined with IMM or CDS IMM".to_string(),
            ));
        }
        if self.imm_mode && self.cds_imm_mode {
            return Err(crate::Error::Validation(
                "standard IMM and CDS IMM modes are mutually exclusive".to_string(),
            ));
        }
        let error_policy = self.error_policy;
        let result = self.build_impl();

        match result {
            Ok(schedule) => strict_fail_closed_on_warnings(error_policy, schedule),
            Err(e) if error_policy == ScheduleErrorPolicy::GracefulEmpty => {
                tracing::warn!(error = %e, "schedule build fell back to empty schedule");
                // Capture the error as a warning instead of propagating
                Ok(Schedule {
                    dates: Vec::new(),
                    payment_dates: Vec::new(),
                    fixing_dates: Vec::new(),
                    warnings: vec![ScheduleWarning::GracefulFallback {
                        error_message: e.to_string(),
                    }],
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Internal implementation of schedule building.
    fn build_impl(self) -> crate::Result<Schedule> {
        use super::calendar::calendar_by_id_strict;

        if self.start > self.end {
            return Err(crate::error::InputError::InvalidDateRange.into());
        }

        let mut warnings: Vec<ScheduleWarning> = Vec::new();

        // Resolve pending calendar ID if present, otherwise use directly provided calendar
        let resolved_cal: Option<&dyn HolidayCalendar> =
            if let Some(ref calendar_id) = self.deferred_calendar_id {
                match calendar_by_id_strict(calendar_id) {
                    Ok(cal) => Some(cal),
                    Err(_) if self.error_policy == ScheduleErrorPolicy::MissingCalendarWarning => {
                        tracing::warn!(
                            calendar_id,
                            "schedule build skipped missing calendar due to warning policy"
                        );
                        warnings.push(ScheduleWarning::MissingCalendarId {
                            calendar_id: calendar_id.clone(),
                        });
                        None
                    }
                    // Strict mode: error on missing calendar
                    Err(err) => return Err(err),
                }
            } else {
                self.cal
            };

        // Generate dates based on mode
        let mut dates = if self.imm_mode {
            // Standard IMM: generate dates using next_imm to get proper third Wednesdays
            let mut imm_dates = generate_imm_dates(self.start, self.end);
            if imm_dates.is_empty() {
                // No IMM date falls inside [start, end]: a silently empty
                // schedule means zero cashflows / PV = 0 downstream. Error
                // in strict mode (graceful policies convert this to an
                // empty schedule WITH a warning).
                return Err(crate::error::Error::Validation(format!(
                    "IMM schedule from {} to {} contains no IMM dates \
                     (first IMM date after start exceeds end)",
                    self.start, self.end
                )));
            }
            if imm_dates.first().is_some_and(|&first| self.start < first) {
                imm_dates.insert(0, self.start);
            }
            imm_dates
        } else if self.cds_imm_mode {
            // CDS IMM: 20th of quarterly months (unadjusted; any business-day
            // adjustment configured on the builder is applied separately below).
            //
            // Post-Big-Bang (2009) standard CDS contracts include a FRONT
            // ACCRUAL period: the first premium period accrues from the CDS
            // roll date immediately PRECEDING the start date, not the next
            // one. Snapping the start forward (the previous behavior) dropped
            // that initial accrual period (2026-06-09 core quant review,
            // Moderate/Dates).
            let adj_start = if crate::dates::imm::is_cds_date(self.start) {
                self.start
            } else {
                prev_cds_date(self.start)
            };

            let builder = BuilderInternal {
                start: adj_start,
                end: self.end,
                frequency: self.frequency,
                stub: self.stub,
                eom: self.eom,
            };
            builder.generate()?
        } else {
            let builder = BuilderInternal {
                start: self.start,
                end: self.end,
                frequency: self.frequency,
                stub: self.stub,
                eom: self.eom,
            };
            builder.generate()?
        };

        // Enforce monotonicity and remove duplicates produced by EOM/stub handling
        let pre_dedup_len = dates.len();
        enforce_monotonic_and_dedup(&mut dates);
        if dates.len() != pre_dedup_len {
            tracing::warn!(
                dropped = pre_dedup_len - dates.len(),
                "schedule generation dropped duplicate or non-monotonic dates"
            );
        }

        // Apply business day adjustment to period-end payment dates only.
        // Accrual dates stay on the unadjusted roll grid (CDS 20ths included).
        let (payment_dates, fixing_dates) = build_payment_and_fixing_dates(
            &dates,
            self.conv,
            resolved_cal,
            self.payment_lag_business_days,
            self.fixing_lag_business_days,
        )?;

        Ok(Schedule {
            dates,
            payment_dates,
            fixing_dates,
            warnings,
        })
    }
}

/// Build the payment and fixing series from an unadjusted accrual grid.
///
/// Period ends are optionally business-day adjusted, then shifted by
/// `payment_lag`. Fixing dates are T-minus from each period start when a
/// fixing lag is configured. Accrual dates themselves are never adjusted.
fn build_payment_and_fixing_dates(
    dates: &[Date],
    conv: Option<BusinessDayConvention>,
    cal: Option<&dyn HolidayCalendar>,
    payment_lag: i32,
    fixing_lag: Option<i32>,
) -> crate::Result<(Vec<Date>, Vec<Date>)> {
    if payment_lag < 0 {
        return Err(InputError::NegativeScheduleLag { lag: payment_lag }.into());
    }
    if let Some(lag) = fixing_lag {
        if lag < 0 {
            return Err(InputError::NegativeScheduleLag { lag }.into());
        }
    }
    let needs_calendar = payment_lag > 0 || fixing_lag.is_some_and(|lag| lag > 0);
    if needs_calendar && cal.is_none() {
        return Err(InputError::ScheduleLagRequiresCalendar.into());
    }

    let n_periods = dates.len().saturating_sub(1);
    let mut payment_dates = Vec::with_capacity(n_periods);
    for end in dates.windows(2).map(|window| window[1]) {
        let mut pay = end;
        if let (Some(conv), Some(cal)) = (conv, cal) {
            pay = adjust(pay, conv, cal)?;
        }
        if payment_lag != 0 {
            let cal = cal.ok_or(InputError::ScheduleLagRequiresCalendar)?;
            pay = pay.add_business_days(payment_lag, cal)?;
        }
        payment_dates.push(pay);
    }

    let fixing_dates = if let Some(lag) = fixing_lag {
        let mut out = Vec::with_capacity(n_periods);
        for start in dates.windows(2).map(|window| window[0]) {
            let fix = if lag == 0 {
                start
            } else {
                let cal = cal.ok_or(InputError::ScheduleLagRequiresCalendar)?;
                start.add_business_days(-lag, cal)?
            };
            out.push(fix);
        }
        out
    } else {
        Vec::new()
    };

    Ok((payment_dates, fixing_dates))
}

/// Enforce the strict policy's fail-closed contract on a built schedule.
///
/// [`ScheduleErrorPolicy::Strict`] means "no silent degradation": if any
/// [`ScheduleWarning`] was attached during construction, the schedule is
/// rejected rather than returned. Non-strict policies pass the schedule
/// through carrying its warnings for the caller to inspect.
fn strict_fail_closed_on_warnings(
    policy: ScheduleErrorPolicy,
    schedule: Schedule,
) -> crate::Result<Schedule> {
    if policy == ScheduleErrorPolicy::Strict && schedule.has_warnings() {
        let joined = schedule
            .warnings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(crate::Error::Validation(format!(
            "schedule build produced warnings; strict policy fails closed: {joined}"
        )));
    }
    Ok(schedule)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ScheduleSpecWire")]
/// Serializable specification for building a schedule.
///
/// This struct captures all parameters needed to generate a schedule of dates
/// for cashflows, coupons, or other periodic events. It can be deserialized
/// from configuration files and converted to a runtime [`ScheduleBuilder`].
pub struct ScheduleSpec {
    /// Start date of the schedule.
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub start: Date,
    /// End date (maturity) of the schedule.
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub end: Date,
    /// Payment frequency (e.g., quarterly, monthly).
    pub frequency: Tenor,
    /// Stub convention (short/long front/back).
    pub stub: StubKind,
    /// Business day convention for adjusting dates.
    pub business_day_convention: Option<BusinessDayConvention>,
    /// Optional calendar identifier for holiday adjustments.
    pub calendar_id: Option<String>,
    /// If true, always roll to end of month when applicable.
    pub end_of_month: bool,
    /// If true, use standard IMM date logic (third Wednesday of quarterly months).
    #[serde(default)]
    pub imm_mode: bool,
    /// If true, use CDS IMM date logic (20th of quarterly months).
    pub cds_imm_mode: bool,
    /// Policy for recoverable schedule-construction errors.
    pub error_policy: ScheduleErrorPolicy,
    /// Business days after each (adjusted) period end for the payment date.
    #[serde(default)]
    pub payment_lag_business_days: i32,
    /// Optional T-minus business days from each period's accrual start.
    #[serde(default)]
    pub fixing_lag_business_days: Option<i32>,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ScheduleSpecWire {
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    start: Date,
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    business_day_convention: Option<BusinessDayConvention>,
    calendar_id: Option<String>,
    end_of_month: bool,
    #[serde(default)]
    imm_mode: bool,
    cds_imm_mode: bool,
    error_policy: ScheduleErrorPolicy,
    #[serde(default)]
    payment_lag_business_days: i32,
    #[serde(default)]
    fixing_lag_business_days: Option<i32>,
}

impl TryFrom<ScheduleSpecWire> for ScheduleSpec {
    type Error = String;

    fn try_from(wire: ScheduleSpecWire) -> Result<Self, Self::Error> {
        if wire.imm_mode && wire.cds_imm_mode {
            return Err("standard IMM and CDS IMM modes are mutually exclusive".to_string());
        }
        Ok(Self {
            start: wire.start,
            end: wire.end,
            frequency: wire.frequency,
            stub: wire.stub,
            business_day_convention: wire.business_day_convention,
            calendar_id: wire.calendar_id,
            end_of_month: wire.end_of_month,
            imm_mode: wire.imm_mode,
            cds_imm_mode: wire.cds_imm_mode,
            error_policy: wire.error_policy,
            payment_lag_business_days: wire.payment_lag_business_days,
            fixing_lag_business_days: wire.fixing_lag_business_days,
        })
    }
}

impl ScheduleSpec {
    /// Create a persisted specification using the canonical builder defaults.
    ///
    /// # Arguments
    ///
    /// * `start` - First unadjusted accrual date, included in the schedule.
    /// * `end` - Last unadjusted accrual date; must be on or after start.
    ///
    /// # Errors
    ///
    /// Returns InvalidDateRange if start is after end.
    pub fn new(start: Date, end: Date) -> crate::Result<Self> {
        let builder = ScheduleBuilder::new(start, end)?;
        Ok(Self {
            start: builder.start,
            end: builder.end,
            frequency: builder.frequency,
            stub: builder.stub,
            business_day_convention: builder.conv,
            calendar_id: builder.deferred_calendar_id,
            end_of_month: builder.eom,
            imm_mode: builder.imm_mode,
            cds_imm_mode: builder.cds_imm_mode,
            error_policy: builder.error_policy,
            payment_lag_business_days: builder.payment_lag_business_days,
            fixing_lag_business_days: builder.fixing_lag_business_days,
        })
    }

    /// Reconstruct a [`Schedule`] using the persisted configuration.
    ///
    /// This applies the same scheduling rules as [`ScheduleBuilder`], including
    /// stub handling, end-of-month logic, standard or CDS IMM mode, and the
    /// configured error policy. Business-day adjustment is enabled only when
    /// both `business_day_convention` and `calendar_id` are present; either
    /// value alone leaves dates unadjusted.
    ///
    /// # Errors
    ///
    /// Returns an error if both IMM modes are selected, the date range or
    /// frequency is invalid, a strict calendar lookup or business-day
    /// adjustment fails, or schedule generation fails. With
    /// [`ScheduleErrorPolicy::MissingCalendarWarning`], a missing calendar
    /// produces an unadjusted schedule carrying a warning. With
    /// [`ScheduleErrorPolicy::GracefulEmpty`], recoverable builder errors
    /// instead produce an empty schedule with a graceful-fallback warning;
    /// mutually exclusive IMM modes always remain errors.
    pub fn build(&self) -> crate::Result<Schedule> {
        if self.imm_mode && self.cds_imm_mode {
            return Err(crate::Error::Validation(
                "standard IMM and CDS IMM modes are mutually exclusive".to_string(),
            ));
        }
        let mut builder = ScheduleBuilder::new(self.start, self.end)?
            .frequency(self.frequency)
            .stub_rule(self.stub)
            .end_of_month(self.end_of_month);

        builder = builder.error_policy(self.error_policy);

        if let (Some(conv), Some(id)) = (self.business_day_convention, self.calendar_id.as_deref())
        {
            builder = builder.adjust_with_id(conv, id);
        }

        if self.payment_lag_business_days != 0 {
            builder = builder.payment_lag_business_days(self.payment_lag_business_days);
        }
        if let Some(lag) = self.fixing_lag_business_days {
            builder = builder.fixing_lag_business_days(lag);
        }

        if self.imm_mode {
            builder = builder.imm();
        } else if self.cds_imm_mode {
            builder = builder.cds_imm();
        }

        builder.build()
    }
}

// ---------------------------------------------------------------------------
// Internal date generation: raw date sequences from a frequency / stub / EOM
// specification, consumed only by `ScheduleBuilder::build_impl`.
// ---------------------------------------------------------------------------

/// Small helper alias when we need to pre-buffer (used only for `ShortFront`).
type Buffer = SmallVec<[Date; 32]>;

const MAX_SCHEDULE_ANCHORS: usize = 100_000;

fn schedule_too_large_error() -> crate::Error {
    crate::Error::Validation(format!(
        "schedule generation exceeded {MAX_SCHEDULE_ANCHORS} anchors; \
         check date range and tenor frequency"
    ))
}

fn check_anchor_count(len: usize) -> crate::Result<()> {
    if len > MAX_SCHEDULE_ANCHORS {
        return Err(schedule_too_large_error());
    }
    Ok(())
}

fn next_roll_index(i: i32) -> crate::Result<i32> {
    i.checked_add(1).ok_or_else(schedule_too_large_error)
}

#[inline]
fn maybe_eom(eom: bool, d: Date) -> Date {
    if eom {
        d.end_of_month()
    } else {
        d
    }
}

#[inline]
fn push_if_new(buf: &mut Buffer, d: Date) {
    if buf.last().copied() != Some(d) {
        buf.push(d)
    }
}
/// Generate IMM dates (third Wednesday of Mar/Jun/Sep/Dec) within the given range.
///
/// Unlike regular schedule generation which adds fixed intervals, this function
/// computes the actual third Wednesday of each quarterly month to handle the
/// variable day-of-month correctly.
fn generate_imm_dates(start: Date, end: Date) -> Vec<Date> {
    let mut dates = Vec::new();

    let first_imm = if crate::dates::imm::is_imm_date(start) {
        start
    } else {
        next_imm(start)
    };

    if first_imm > end {
        return dates;
    }

    dates.push(first_imm);

    let mut current = first_imm;
    loop {
        let next = next_imm(current);
        if next > end {
            break;
        }
        dates.push(next);
        current = next;
    }

    dates
}

/// Enforce strictly increasing, duplicate-free dates while preserving original order.
/// Drops any consecutive duplicates and any dates that would not increase.
fn enforce_monotonic_and_dedup(dates: &mut Vec<Date>) {
    if dates.is_empty() {
        return;
    }
    let mut write = 0;
    for read in 1..dates.len() {
        if dates[read] > dates[write] {
            write += 1;
            if read != write {
                dates[write] = dates[read];
            }
        }
    }
    dates.truncate(write + 1);
}

// BuilderInternal – raw date sequence generator

#[derive(Clone, Copy)]
struct BuilderInternal {
    pub start: Date,
    pub end: Date,
    pub frequency: Tenor,
    pub stub: StubKind,
    pub eom: bool,
}

impl BuilderInternal {
    fn generate(self) -> crate::Result<Vec<Date>> {
        if self.start >= self.end {
            return Err(crate::error::InputError::InvalidDateRange.into());
        }
        match self.stub {
            StubKind::ShortFront => self.gen_short_front(),
            StubKind::LongFront => self.gen_long_front(),
            StubKind::LongBack => self.gen_long_back(),
            StubKind::None => self.gen_regular(),
            StubKind::ShortBack => self.gen_short_back(),
        }
    }

    /// The `n`-th roll date from a fixed `anchor` (`n` may be negative for
    /// backward generation).
    ///
    /// Every date is computed as `anchor + n·tenor` directly from the anchor
    /// (QuantLib-style), so month-end clamping in short months never
    /// propagates: backward semi-annual from Aug 31 yields Feb 28/29 and then
    /// Aug **31** again, not Aug 28. Chaining `prev + tenor` (the previous
    /// implementation) drifted the roll day by 1–3 days per short month.
    fn nth_tenor(self, anchor: Date, n: i32) -> crate::Result<Date> {
        let tenor = self.frequency;
        if n == 0 {
            return Ok(anchor);
        }
        let count_i32 =
            i32::try_from(tenor.count()).map_err(|_| crate::error::InputError::InvalidTenor {
                tenor: tenor.to_string(),
                reason: format!("count {} exceeds i32::MAX", tenor.count()),
            })?;
        let checked_mul_i32 = |lhs: i32, rhs: i32| -> crate::Result<i32> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::error::InputError::InvalidTenor {
                    tenor: tenor.to_string(),
                    reason: "tenor multiplication overflowed while generating schedule".to_string(),
                })
            })
        };
        let checked_mul_i64 = |lhs: i64, rhs: i64| -> crate::Result<i64> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::error::InputError::InvalidTenor {
                    tenor: tenor.to_string(),
                    reason: "tenor multiplication overflowed while generating schedule".to_string(),
                })
            })
        };
        Ok(match tenor.unit() {
            crate::dates::TenorUnit::Months => anchor.add_months(checked_mul_i32(n, count_i32)?),
            crate::dates::TenorUnit::Years => {
                let years = checked_mul_i32(n, count_i32)?;
                anchor.add_months(checked_mul_i32(years, 12)?)
            }
            crate::dates::TenorUnit::Weeks => {
                anchor + Duration::weeks(checked_mul_i64(i64::from(n), i64::from(tenor.count()))?)
            }
            crate::dates::TenorUnit::Days => {
                anchor + Duration::days(checked_mul_i64(i64::from(n), i64::from(tenor.count()))?)
            }
        })
    }

    // EOM convention: `maybe_eom` snaps only the COMPUTED intermediate roll
    // dates to month-end. The user-provided `start` and `end` dates are
    // contractual and are emitted verbatim (QuantLib-style); snapping them
    // (the previous behavior) silently moved the effective date and maturity.

    fn gen_regular(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            // StubKind::None requires exact contractual alignment with the raw
            // tenor roll. EOM only adjusts intermediate generated dates; it
            // must not turn an arbitrary maturity before month-end into an
            // undeclared final stub.
            let raw = self.nth_tenor(self.start, i)?;
            let snapped = maybe_eom(self.eom, raw);
            if raw == self.end {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            if raw > self.end {
                return Err(crate::error::InputError::NonIntegerScheduleTenor.into());
            }
            if snapped < self.end {
                push_if_new(&mut buf, snapped);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }

    fn gen_short_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            let dt = maybe_eom(self.eom, self.nth_tenor(self.start, i)?);
            if dt >= self.end {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            push_if_new(&mut buf, dt);
            check_anchor_count(buf.len())?;
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }

    fn gen_short_front(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.end;
        push_if_new(&mut buf, anchor);
        let mut i = 1;
        loop {
            let dt = self.nth_tenor(anchor, -i)?;
            if dt <= self.start {
                push_if_new(&mut buf, self.start);
                check_anchor_count(buf.len())?;
                break;
            }
            let snapped = maybe_eom(self.eom, dt);
            if snapped > self.start && snapped < self.end {
                push_if_new(&mut buf, snapped);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        buf.as_mut_slice().reverse();
        Ok(buf.into_vec())
    }

    fn gen_long_front(self) -> crate::Result<Vec<Date>> {
        // Regular anchors backward from `end`; `aligned` records whether the
        // lowest anchor lands exactly on `start` (no stub at all).
        let mut anchors: Vec<Date> = vec![self.end];
        let mut i = 1;
        let aligned = loop {
            let dt = self.nth_tenor(self.end, -i)?;
            if dt <= self.start {
                break dt == self.start;
            }
            anchors.push(dt);
            check_anchor_count(anchors.len())?;
            i = next_roll_index(i)?;
        };
        // Long front stub: merge the residual short stub with the first
        // regular period by dropping the lowest anchor. Skipping this merge
        // (the previous behavior) made LongFront identical to ShortFront.
        if !aligned && anchors.len() > 1 {
            anchors.pop();
        }
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        for (idx, &a) in anchors.iter().enumerate().rev() {
            // anchors[0] is the user-provided end date: never snap it.
            let dt = if idx == 0 { a } else { maybe_eom(self.eom, a) };
            if dt > self.start && (idx == 0 || dt < self.end) {
                push_if_new(&mut buf, dt);
            }
        }
        Ok(buf.into_vec())
    }

    fn gen_long_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.start;
        buf.push(anchor);
        let mut i = 1;
        loop {
            let next = self.nth_tenor(anchor, i)?;
            let next_after = self.nth_tenor(anchor, next_roll_index(i)?)?;
            if next_after > self.end {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            let dt = maybe_eom(self.eom, next);
            if dt < self.end {
                push_if_new(&mut buf, dt);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warned_schedule() -> Schedule {
        Schedule {
            dates: Vec::new(),
            payment_dates: Vec::new(),
            fixing_dates: Vec::new(),
            warnings: vec![ScheduleWarning::MissingCalendarId {
                calendar_id: "nope".to_string(),
            }],
        }
    }

    #[test]
    fn strict_policy_fails_closed_on_warnings() {
        let err = strict_fail_closed_on_warnings(ScheduleErrorPolicy::Strict, warned_schedule())
            .expect_err("strict must reject schedules carrying warnings");
        let msg = err.to_string();
        assert!(msg.contains("strict policy fails closed"), "got: {msg}");
        assert!(msg.contains("nope"), "got: {msg}");
    }

    #[test]
    fn strict_policy_passes_clean_schedules_through() {
        let clean = Schedule {
            dates: Vec::new(),
            payment_dates: Vec::new(),
            fixing_dates: Vec::new(),
            warnings: Vec::new(),
        };
        assert!(strict_fail_closed_on_warnings(ScheduleErrorPolicy::Strict, clean).is_ok());
    }

    #[test]
    fn non_strict_policies_pass_warnings_through() {
        for policy in [
            ScheduleErrorPolicy::MissingCalendarWarning,
            ScheduleErrorPolicy::GracefulEmpty,
        ] {
            let schedule = strict_fail_closed_on_warnings(policy, warned_schedule())
                .expect("non-strict policies keep warned schedules");
            assert!(schedule.has_warnings());
        }
    }
}
