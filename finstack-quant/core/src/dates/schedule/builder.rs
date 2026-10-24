//! Validated construction.

use super::*;

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
/// 1. Create builder with [`Schedule::builder`]
/// 2. Configure options via fluent methods
/// 3. Call `build()` to generate the [`Schedule`]
///
/// # Examples
///
/// Basic quarterly schedule:
/// ```rust
/// use finstack_quant_core::dates::{Schedule, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::March, 20)?;
/// let end = Date::from_calendar_date(2025, Month::December, 20)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::quarterly())
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// With business day adjustment:
/// ```rust
/// use finstack_quant_core::dates::{BusinessDayConvention, Schedule, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::December, 15)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .frequency(Tenor::monthly())
///     .adjust_with(BusinessDayConvention::ModifiedFollowing, "nyse")
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// CDS IMM schedule (credit default swaps):
/// ```rust
/// use finstack_quant_core::dates::Schedule;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2026, Month::December, 20)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .cds_imm()  // Quarterly on 20-Mar/Jun/Sep/Dec
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Standard IMM roll schedule:
/// ```rust
/// use finstack_quant_core::dates::Schedule;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 15)?;
/// let end = Date::from_calendar_date(2025, Month::December, 31)?;
///
/// let schedule = Schedule::builder(start, end)?
///     .imm()  // Quarterly rolls on the third Wednesday of Mar/Jun/Sep/Dec
///     .build()?;
/// // Jan-15, then Mar-19, Jun-18, Sep-17, Dec-17, then Dec-31: the
/// // contractual start and end bound a short initial and final stub.
/// assert_eq!(schedule.dates.len(), 6);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// End-of-month convention:
/// ```rust
/// use finstack_quant_core::dates::{Schedule, Tenor};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 31)?;
/// let end = Date::from_calendar_date(2025, Month::June, 30)?;
///
/// let schedule = Schedule::builder(start, end)?
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
#[derive(Clone, Debug)]
pub struct ScheduleBuilder {
    pub(super) spec: ScheduleSpec,
}

impl Schedule {
    /// Start a [`ScheduleBuilder`] with mandatory `start` and `end` dates.
    ///
    /// Defaults: frequency = Monthly, stub = None, no adjustment, no EOM,
    /// strict error policy (see [`ScheduleSpec::new`]).
    ///
    /// # Arguments
    ///
    /// * `start` - First unadjusted accrual date, included in the schedule.
    /// * `end` - Last unadjusted accrual date; must be on or after `start`.
    ///
    /// # Errors
    ///
    /// Returns `Err(InputError::InvalidDateRange)` if `start > end`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Schedule, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::April, 15)?;
    ///
    /// let schedule = Schedule::builder(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder(start: Date, end: Date) -> crate::Result<ScheduleBuilder> {
        Ok(ScheduleBuilder {
            spec: ScheduleSpec::new(start, end)?,
        })
    }
}

/// A builder that applies a persisted [`ScheduleSpec`] as it is; this is how
/// a deserialized specification is built.
impl From<ScheduleSpec> for ScheduleBuilder {
    fn from(spec: ScheduleSpec) -> Self {
        Self { spec }
    }
}

impl ScheduleBuilder {
    /// Set coupon/payment frequency.
    #[must_use]
    pub fn frequency(mut self, frequency: Tenor) -> Self {
        self.spec.frequency = frequency;
        self
    }

    /// Set stub handling rule.
    ///
    /// # Arguments
    ///
    /// * `stub` - Stub policy controlling irregular first or final schedule periods.
    #[must_use]
    pub fn stub_rule(mut self, stub: StubKind) -> Self {
        self.spec.stub = stub;
        self
    }

    /// Enable End-of-Month (EOM) convention.
    ///
    /// When enabled, computed intermediate roll dates are snapped to the
    /// last day of their month. The user-provided start and end dates are
    /// contractual and are never snapped.
    /// With [`StubKind::None`], maturity must lie on the resulting EOM roll
    /// grid. Use an explicit stub rule for a maturity between roll dates.
    /// EOM requires a month/year tenor and cannot be combined with IMM or CDS IMM.
    ///
    /// # Arguments
    ///
    /// * `eom` - Whether to snap intermediate dates to month-end; incompatible
    ///   tenor or generation-rule combinations are rejected by `build`.
    #[must_use]
    pub fn end_of_month(mut self, eom: bool) -> Self {
        self.spec.end_of_month = eom;
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
    /// [`adjust_with`](Self::adjust_with).
    #[must_use]
    pub fn cds_imm(mut self) -> Self {
        self.spec = self.spec.with_cds_imm();
        self
    }

    /// Roll the schedule on standard IMM dates (third Wednesday of Mar, Jun,
    /// Sep, Dec), the FpML `IMM` roll convention used by IMM swaps, FRA strips
    /// and futures-aligned legs.
    ///
    /// Sets the IMM defaults (quarterly frequency, [`StubKind::ShortBack`]);
    /// call [`frequency`](Self::frequency) or [`stub_rule`](Self::stub_rule)
    /// afterwards to change them. Then [`build`](Self::build):
    ///
    /// - keeps the contractual `start` and `end` as the first and last dates;
    /// - lays the regular periods on the IMM cycle at `frequency`, which must
    ///   be a whole number of quarters (3M, 6M, 1Y, ...);
    /// - generates forward from the first IMM date on or after `start` for
    ///   back-stub rules, or backward from the last IMM date on or before `end`
    ///   for front-stub rules. An off-cycle `start` or `end` becomes a stub
    ///   period, so a schedule can have both an initial and a final stub;
    /// - returns the single period `[start, end]` when no IMM date lies
    ///   strictly between them;
    /// - with [`StubKind::None`], requires `start` and `end` on IMM dates.
    ///
    /// Unlike [`cds_imm()`](Self::cds_imm), which uses the 20th of quarterly
    /// months and accrues from the roll date before `start`, standard IMM
    /// dates fall on the third Wednesday and the schedule starts at `start`.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::dates::{Schedule, StubKind, Tenor};
    /// use time::{Date, Month};
    ///
    /// let date = |y, m, d| Date::from_calendar_date(y, m, d);
    /// let start = date(2025, Month::January, 15)?;
    /// let end = date(2025, Month::December, 31)?;
    ///
    /// let quarterly = Schedule::builder(start, end)?.imm().build()?;
    /// assert_eq!(
    ///     quarterly.dates,
    ///     vec![
    ///         start,
    ///         date(2025, Month::March, 19)?,
    ///         date(2025, Month::June, 18)?,
    ///         date(2025, Month::September, 17)?,
    ///         date(2025, Month::December, 17)?,
    ///         end,
    ///     ]
    /// );
    ///
    /// // A semi-annual leg generated backward from the last IMM date.
    /// let semi_annual = Schedule::builder(start, end)?
    ///     .imm()
    ///     .frequency(Tenor::semi_annual())
    ///     .stub_rule(StubKind::ShortFront)
    ///     .build()?;
    /// assert_eq!(
    ///     semi_annual.dates,
    ///     vec![start, date(2025, Month::June, 18)?, date(2025, Month::December, 17)?, end]
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn imm(mut self) -> Self {
        self.spec = self.spec.with_imm();
        self
    }

    /// Configure how recoverable schedule-construction errors are handled.
    ///
    /// # Arguments
    ///
    /// * `policy` - Policy enum controlling error handling, unmatched keys, or fallbacks
    #[must_use]
    pub fn error_policy(mut self, policy: ScheduleErrorPolicy) -> Self {
        self.spec.error_policy = policy;
        self
    }

    /// Shift each payment date by `lag` business days after the (adjusted)
    /// period end.
    ///
    /// A lag of zero leaves the payment date on the adjusted period end.
    /// A positive lag requires a holiday calendar from
    /// [`adjust_with`](Self::adjust_with).
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative business-day delay from each period's payment
    ///   anchor to the actual payment date. Zero is T+0 (pay on the adjusted
    ///   end). Negative values are rejected at [`build`](Self::build).
    #[must_use]
    pub fn payment_lag_days(mut self, lag: i32) -> Self {
        self.spec.payment_lag_days = lag;
        self
    }

    /// Set a T-minus fixing lag from each period's unadjusted accrual start.
    ///
    /// The fixing date is `accrual_start` minus `lag` business days. A lag of
    /// zero stores the accrual start itself. A positive lag requires a holiday
    /// calendar from [`adjust_with`](Self::adjust_with).
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative business-day lookback from each period's accrual
    ///   start. Negative values are rejected at [`build`](Self::build).
    #[must_use]
    pub fn fixing_lag_business_days(mut self, lag: i32) -> Self {
        self.spec.fixing_lag_business_days = Some(lag);
        self
    }

    /// Configure business-day adjustment of payment dates with a calendar id.
    ///
    /// The calendar is resolved at build time, so a later call replaces both
    /// the convention and the calendar.
    ///
    /// # Errors
    ///
    /// By default, [`build()`](Self::build) returns an error if the calendar ID
    /// is not found. Use [`error_policy`](Self::error_policy) with
    /// [`ScheduleErrorPolicy::MissingCalendarWarning`] to opt into lenient behavior.
    ///
    /// # Arguments
    ///
    /// * `conv` - Business day convention (Following, Modified Following, etc.)
    /// * `calendar_id` - Registered calendar identifier (e.g., "nyse", "target2",
    ///   "gblo"); `+`-joined ids such as "nyse+gblo" resolve to the union calendar.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{BusinessDayConvention, Schedule, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::December, 15)?;
    ///
    /// let schedule = Schedule::builder(start, end)?
    ///     .frequency(Tenor::monthly())
    ///     .adjust_with(BusinessDayConvention::Following, "nyse")
    ///     .build()?;
    /// assert!(!schedule.dates.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn adjust_with(mut self, conv: BusinessDayConvention, calendar_id: &str) -> Self {
        self.spec.business_day_convention = Some(conv);
        self.spec.calendar_id = Some(calendar_id.to_string());
        self
    }

    /// Build a concrete schedule (adjusted if configured).
    ///
    /// The stored `frequency` and `stub` are used as they are in every mode
    /// ([`imm`](Self::imm) and [`cds_imm`](Self::cds_imm) set the IMM
    /// defaults, which a later setter overrides; see [`imm`](Self::imm) for
    /// the IMM roll rules). Business-day adjustment is applied only when both a
    /// convention and a calendar id are set; either alone (possible in a
    /// deserialized [`ScheduleSpec`]) leaves payment dates unadjusted.
    ///
    /// When [`ScheduleErrorPolicy::GracefulEmpty`] is selected,
    /// this method returns an empty schedule with a [`ScheduleWarning::GracefulFallback`]
    /// warning instead of propagating errors. Always check [`Schedule::has_warnings()`]
    /// when using graceful mode to detect potential pricing issues.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Start date is after end date (and graceful mode is disabled)
    /// - Calendar lookup fails under [`ScheduleErrorPolicy::Strict`]
    /// - EOM is combined with a day/week tenor or an IMM/CDS IMM rule
    /// - In IMM mode, `frequency` is not a whole number of quarters, or
    ///   [`StubKind::None`] is used with an off-cycle `start` or `end`
    pub fn build(self) -> crate::Result<Schedule> {
        if self.spec.end_of_month
            && (self.spec.imm_mode
                || self.spec.cds_imm_mode
                || !matches!(
                    self.spec.frequency.unit(),
                    crate::dates::TenorUnit::Months | crate::dates::TenorUnit::Years
                ))
        {
            return Err(crate::Error::Validation(
                "end-of-month requires a month/year tenor and cannot be combined with IMM or CDS IMM".to_string(),
            ));
        }
        if self.spec.imm_mode && self.spec.cds_imm_mode {
            return Err(crate::Error::Validation(
                "standard IMM and CDS IMM modes are mutually exclusive".to_string(),
            ));
        }
        if self.spec.start > self.spec.end {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        let error_policy = self.spec.error_policy;
        let result = self.build_impl();

        match result {
            Ok(schedule) => Ok(schedule),
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
        use crate::dates::calendar_by_id_strict;

        let mut warnings: Vec<ScheduleWarning> = Vec::new();

        // A calendar id adjusts payment dates only together with a convention.
        let calendar_id = self
            .spec
            .business_day_convention
            .and(self.spec.calendar_id.as_ref());
        let resolved_cal: Option<&dyn HolidayCalendar> = match calendar_id {
            None => None,
            Some(calendar_id) => match calendar_by_id_strict(calendar_id) {
                Ok(cal) => Some(cal),
                Err(_) if self.spec.error_policy == ScheduleErrorPolicy::MissingCalendarWarning => {
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
            },
        };

        // Generate dates based on mode
        let mut dates = if self.spec.imm_mode {
            BuilderInternal {
                start: self.spec.start,
                end: self.spec.end,
                frequency: self.spec.frequency,
                stub: self.spec.stub,
                roll: RollDay::ThirdWednesday,
            }
            .generate_imm()?
        } else if self.spec.cds_imm_mode {
            // CDS IMM: 20th of quarterly months (unadjusted; any business-day
            // adjustment configured on the builder is applied separately below).
            //
            // Post-Big-Bang (2009) standard CDS contracts include a FRONT
            // ACCRUAL period: the first premium period accrues from the CDS
            // roll date immediately PRECEDING the start date, not the next
            // one. Snapping the start forward (the previous behavior) dropped
            // that initial accrual period (2026-06-09 core quant review,
            // Moderate/Dates).
            let adj_start = if crate::dates::imm::is_cds_date(self.spec.start) {
                self.spec.start
            } else {
                prev_cds_date(self.spec.start)?
            };

            let builder = BuilderInternal {
                start: adj_start,
                end: self.spec.end,
                frequency: self.spec.frequency,
                stub: self.spec.stub,
                roll: RollDay::Natural,
            };
            builder.generate()?
        } else {
            let builder = BuilderInternal {
                start: self.spec.start,
                end: self.spec.end,
                frequency: self.spec.frequency,
                stub: self.spec.stub,
                roll: if self.spec.end_of_month {
                    RollDay::EndOfMonth
                } else {
                    RollDay::Natural
                },
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
            self.spec.business_day_convention,
            resolved_cal,
            self.spec.payment_lag_days,
            self.spec.fixing_lag_business_days,
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
        // `needs_calendar` above guarantees a calendar whenever a lag is set.
        if let Some(cal) = cal.filter(|_| payment_lag != 0) {
            pay = pay.add_business_days(payment_lag, cal)?;
        }
        payment_dates.push(pay);
    }

    let fixing_dates = if let Some(lag) = fixing_lag {
        let mut out = Vec::with_capacity(n_periods);
        for start in dates.windows(2).map(|window| window[0]) {
            let fix = match cal {
                Some(cal) if lag != 0 => start.add_business_days(-lag, cal)?,
                _ => start,
            };
            out.push(fix);
        }
        out
    } else {
        Vec::new()
    };

    Ok((payment_dates, fixing_dates))
}
