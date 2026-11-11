//! Validated construction.

use super::*;

/// Fluent builder that collects a complete [`ScheduleSpec`].
///
/// [`Schedule::builder`] fixes `start` and `end`; every financial choice is
/// then explicit (record CORE-CONVENTIONS-008 #2): a frequency
/// ([`frequency`](Self::frequency)), a stub policy
/// ([`stub_rule`](Self::stub_rule)), one grid ([`grid`](Self::grid)) and a
/// payment convention ([`adjust_with`](Self::adjust_with) or
/// [`unadjusted`](Self::unadjusted)). An incomplete builder can be passed
/// around, but [`build`](Self::build) and [`to_spec`](Self::to_spec) fail
/// until every choice is made. Payment lag defaults to zero and the fixing
/// series to none.
///
/// # Examples
///
/// Quarterly schedule with payment adjustment:
/// ```rust
/// use finstack_quant_core::dates::{BusinessDayConvention, GridMode, Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::March, 20)?;
/// let end = Date::from_calendar_date(2025, Month::December, 20)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::quarterly())
///     .stub_rule(StubKind::None)
///     .grid(GridMode::Natural)
///     .adjust_with(BusinessDayConvention::ModifiedFollowing, "nyse")
///     .build()?;
/// assert_eq!(schedule.get_dates().len(), 4);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Standard IMM roll schedule:
/// ```rust
/// use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::March, 19)?; // an IMM date
/// let end = Date::from_calendar_date(2025, Month::December, 31)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::quarterly())
///     .stub_rule(StubKind::ShortBack)
///     .grid(GridMode::QuarterlyImm)
///     .unadjusted()
///     .build()?;
/// // Mar-19, Jun-18, Sep-17, Dec-17, then the declared short final stub.
/// assert_eq!(schedule.get_dates().len(), 5);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// End-of-month grid:
/// ```rust
/// use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 31)?;
/// let end = Date::from_calendar_date(2025, Month::June, 30)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::monthly())
///     .stub_rule(StubKind::None)
///     .grid(GridMode::EndOfMonth)
///     .unadjusted()
///     .build()?;
/// // Jan-31, Feb-28, Mar-31, Apr-30, May-31, Jun-30
/// assert_eq!(schedule.get_dates().len(), 6);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`Tenor`] for payment frequency options
/// - [`StubKind`] for stub period handling
/// - [`GridMode`] for the regular grid
/// - [`BusinessDayConvention`] for adjustment rules
///
/// [`BusinessDayConvention`]: super::BusinessDayConvention
#[derive(Clone, Debug)]
pub struct ScheduleBuilder {
    start: Date,
    end: Date,
    frequency: Option<Tenor>,
    stub: Option<StubKind>,
    grid: Option<GridMode>,
    cds_imm_mode: bool,
    business_day_convention: Option<BusinessDayConvention>,
    calendar_id: Option<String>,
    payment_lag_days: i32,
    fixing_lag_business_days: Option<i32>,
}

impl Schedule {
    /// Start a [`ScheduleBuilder`] for the range `start..end`.
    ///
    /// The builder has no financial defaults: frequency, stub, grid and
    /// payment convention must all be chosen before [`ScheduleBuilder::build`].
    ///
    /// # Arguments
    ///
    /// * `start` - First unadjusted grid date, included in the schedule.
    /// * `end` - Last unadjusted grid date; must be after `start`.
    ///
    /// # Errors
    ///
    /// Returns `Err(InputError::InvalidDateRange)` unless `start < end`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{GridMode, Schedule, StubKind, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::April, 15)?;
    ///
    /// let schedule = Schedule::builder(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .stub_rule(StubKind::None)
    ///     .grid(GridMode::Natural)
    ///     .unadjusted()
    ///     .build()?;
    /// assert_eq!(schedule.get_dates().len(), 4);
    /// assert!(Schedule::builder(start, end)?.build().is_err(), "choices are required");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder(start: Date, end: Date) -> crate::Result<ScheduleBuilder> {
        if start >= end {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        Ok(ScheduleBuilder {
            start,
            end,
            frequency: None,
            stub: None,
            grid: None,
            cds_imm_mode: false,
            business_day_convention: None,
            calendar_id: None,
            payment_lag_days: 0,
            fixing_lag_business_days: None,
        })
    }
}

/// A builder holding every choice of a complete [`ScheduleSpec`]; this is how
/// a deserialized specification is built.
impl From<ScheduleSpec> for ScheduleBuilder {
    fn from(spec: ScheduleSpec) -> Self {
        Self {
            start: spec.start,
            end: spec.end,
            frequency: Some(spec.frequency),
            stub: Some(spec.stub),
            grid: Some(spec.grid),
            cds_imm_mode: spec.cds_imm_mode,
            business_day_convention: Some(spec.business_day_convention),
            calendar_id: spec.calendar_id,
            payment_lag_days: spec.payment_lag_days,
            fixing_lag_business_days: spec.fixing_lag_business_days,
        }
    }
}

impl ScheduleBuilder {
    /// Set the regular period length.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Regular period length between unadjusted roll dates
    ///   (for example `Tenor::quarterly()` for a 3M coupon leg).
    #[must_use]
    pub fn frequency(mut self, frequency: Tenor) -> Self {
        self.frequency = Some(frequency);
        self
    }

    /// Set the stub policy, which also selects the regular anchor side.
    ///
    /// # Arguments
    ///
    /// * `stub` - Stub policy controlling irregular first or final periods;
    ///   no-stub and back-stub policies anchor on `start`, front-stub
    ///   policies on `end`.
    #[must_use]
    pub fn stub_rule(mut self, stub: StubKind) -> Self {
        self.stub = Some(stub);
        self
    }

    /// Select the regular grid.
    ///
    /// Selecting a grid never changes the frequency or stub. See
    /// [`GridMode`] for the anchor rules each grid imposes.
    ///
    /// # Arguments
    ///
    /// * `grid` - `Natural`, `EndOfMonth` (month/year frequency, month-end
    ///   anchor) or `QuarterlyImm` (whole quarters, IMM-date anchor).
    #[must_use]
    pub fn grid(mut self, grid: GridMode) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Create a CDS IMM schedule (quarterly on the 20th: 20-Mar, 20-Jun, 20-Sep, 20-Dec).
    ///
    /// Sets the frequency to quarterly and the stub rule to
    /// [`StubKind::ShortBack`] and turns CDS IMM mode on; it requires the
    /// natural grid. When the start date is not itself a CDS roll date the
    /// schedule is anchored at the roll date immediately **preceding** the
    /// start, so the first period carries the standard front accrual.
    ///
    /// The generated 20ths are **unadjusted** roll dates; payment-date
    /// business-day adjustment is applied separately via
    /// [`adjust_with`](Self::adjust_with).
    #[must_use]
    pub fn cds_imm(mut self) -> Self {
        self.frequency = Some(Tenor::quarterly());
        self.stub = Some(StubKind::ShortBack);
        self.cds_imm_mode = true;
        self
    }

    /// Shift each payment date by `lag` business days after the (adjusted)
    /// period end.
    ///
    /// A lag of zero (the default) leaves the payment date on the adjusted
    /// period end. A positive lag requires a calendar from
    /// [`adjust_with`](Self::adjust_with).
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative business-day delay from each period's adjusted
    ///   end to the actual payment date. Negative values are rejected at
    ///   [`build`](Self::build).
    #[must_use]
    pub fn payment_lag_days(mut self, lag: i32) -> Self {
        self.payment_lag_days = lag;
        self
    }

    /// Set a T-minus fixing lag from each period's unadjusted accrual start.
    ///
    /// The fixing date is `accrual_start` minus `lag` business days. A lag of
    /// zero stores the accrual start itself. A positive lag requires a
    /// calendar from [`adjust_with`](Self::adjust_with).
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

    /// Adjust payment dates with `conv` on the calendar `calendar_id`.
    ///
    /// The calendar is resolved strictly at build time and also serves both
    /// lags. A later call replaces both the convention and the calendar.
    ///
    /// # Arguments
    ///
    /// * `conv` - Business day convention (Following, Modified Following,
    ///   etc.); `Unadjusted` with a calendar keeps payment dates on the raw
    ///   period ends while still allowing lags.
    /// * `calendar_id` - Registered calendar identifier (e.g., "nyse",
    ///   "target2", "gblo"); `+`-joined ids such as "nyse+gblo" resolve to
    ///   the union calendar.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{BusinessDayConvention, GridMode, Schedule, StubKind, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::December, 15)?;
    ///
    /// let schedule = Schedule::builder(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .stub_rule(StubKind::None)
    ///     .grid(GridMode::Natural)
    ///     .adjust_with(BusinessDayConvention::Following, "nyse")
    ///     .build()?;
    /// assert_eq!(schedule.get_dates().len(), 12);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn adjust_with(mut self, conv: BusinessDayConvention, calendar_id: &str) -> Self {
        self.business_day_convention = Some(conv);
        self.calendar_id = Some(calendar_id.to_string());
        self
    }

    /// Keep payment dates on the raw period ends, with no calendar.
    ///
    /// Selects the `Unadjusted` convention and clears any calendar; a
    /// positive payment or fixing lag then fails at [`build`](Self::build),
    /// since lags need a calendar (use [`adjust_with`](Self::adjust_with)
    /// with `Unadjusted` for that).
    #[must_use]
    pub fn unadjusted(mut self) -> Self {
        self.business_day_convention = Some(BusinessDayConvention::Unadjusted);
        self.calendar_id = None;
        self
    }

    /// The complete specification these choices describe.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming every missing choice (frequency,
    /// stub, grid, payment convention), or the [`ScheduleSpec::validate`]
    /// error for an inconsistent combination.
    pub fn to_spec(&self) -> crate::Result<ScheduleSpec> {
        let missing: Vec<&str> = [
            ("frequency", self.frequency.is_none()),
            ("stub", self.stub.is_none()),
            ("grid", self.grid.is_none()),
            ("payment convention", self.business_day_convention.is_none()),
        ]
        .into_iter()
        .filter_map(|(name, absent)| absent.then_some(name))
        .collect();
        let (Some(frequency), Some(stub), Some(grid), Some(business_day_convention)) = (
            self.frequency,
            self.stub,
            self.grid,
            self.business_day_convention,
        ) else {
            return Err(crate::Error::Validation(format!(
                "schedule specification is incomplete: choose {}",
                missing.join(", ")
            )));
        };
        let spec = ScheduleSpec {
            start: self.start,
            end: self.end,
            frequency,
            stub,
            grid,
            cds_imm_mode: self.cds_imm_mode,
            business_day_convention,
            calendar_id: self.calendar_id.clone(),
            payment_lag_days: self.payment_lag_days,
            fixing_lag_business_days: self.fixing_lag_business_days,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Build the schedule.
    ///
    /// The raw grid is never business-day adjusted; payment dates are the
    /// adjusted period ends shifted by the payment lag, and fixing dates the
    /// raw period starts shifted back by the fixing lag. The result is a
    /// complete schedule or an error; failures are never turned into an
    /// empty schedule.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - a required choice is missing or the specification is inconsistent
    ///   (see [`to_spec`](Self::to_spec))
    /// - a given calendar id is unknown (it is always resolved strictly)
    /// - the regular anchor is off the selected grid, or [`StubKind::None`]
    ///   is used with an endpoint off the grid or between grid points
    /// - the generated grid is not strictly increasing or exceeds 100,000
    ///   dates
    pub fn build(self) -> crate::Result<Schedule> {
        build_spec(&self.to_spec()?)
    }
}

/// Generate the schedule of a validated specification.
fn build_spec(spec: &ScheduleSpec) -> crate::Result<Schedule> {
    use crate::dates::calendar_by_id_strict;

    // A supplied calendar is always resolved strictly (record 008 #8).
    let calendar: Option<&dyn HolidayCalendar> = spec
        .calendar_id
        .as_deref()
        .map(calendar_by_id_strict)
        .transpose()?;

    let dates = if spec.cds_imm_mode {
        // CDS IMM: 20th of quarterly months (unadjusted; any business-day
        // adjustment is applied separately below). Post-Big-Bang standard
        // CDS contracts accrue from the CDS roll date immediately PRECEDING
        // the start date.
        let start = if crate::dates::imm::is_cds_date(spec.start) {
            spec.start
        } else {
            prev_cds_date(spec.start)?
        };
        BuilderInternal {
            start,
            end: spec.end,
            frequency: spec.frequency,
            stub: spec.stub,
            roll: GridMode::Natural,
        }
        .generate()?
    } else {
        BuilderInternal {
            start: spec.start,
            end: spec.end,
            frequency: spec.frequency,
            stub: spec.stub,
            roll: spec.grid,
        }
        .generate()?
    };

    let (payment_dates, fixing_dates) = build_payment_and_fixing_dates(
        &dates,
        spec.business_day_convention,
        calendar,
        spec.payment_lag_days,
        spec.fixing_lag_business_days,
    )?;
    Schedule::try_new(dates, payment_dates, fixing_dates)
}

/// Build the payment and fixing series from an unadjusted accrual grid.
///
/// Each period end is adjusted with `conv` and then shifted by
/// `payment_lag` business days (record 008 #9). Fixing dates are T-minus
/// from each raw period start when a fixing lag is configured (#10). The
/// specification has already checked that a calendar is present whenever an
/// adjustment or a positive lag needs one.
fn build_payment_and_fixing_dates(
    dates: &[Date],
    conv: BusinessDayConvention,
    cal: Option<&dyn HolidayCalendar>,
    payment_lag: i32,
    fixing_lag: Option<i32>,
) -> crate::Result<(Vec<Date>, Vec<Date>)> {
    let n_periods = dates.len() - 1;
    let mut payment_dates = Vec::with_capacity(n_periods);
    for end in dates.windows(2).map(|window| window[1]) {
        let pay = match cal {
            Some(cal) => {
                let adjusted = adjust(end, conv, cal)?;
                if payment_lag == 0 {
                    adjusted
                } else {
                    adjusted.add_business_days(payment_lag, cal)?
                }
            }
            None => end,
        };
        payment_dates.push(pay);
    }

    let fixing_dates = match fixing_lag {
        Some(lag) => dates
            .windows(2)
            .map(|window| match cal {
                Some(cal) if lag != 0 => window[0].add_business_days(-lag, cal),
                _ => Ok(window[0]),
            })
            .collect::<crate::Result<Vec<_>>>()?,
        None => Vec::new(),
    };

    Ok((payment_dates, fixing_dates))
}
