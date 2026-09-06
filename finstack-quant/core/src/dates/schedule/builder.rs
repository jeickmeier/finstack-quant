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
    pub(super) start: Date,
    pub(super) end: Date,
    pub(super) frequency: Tenor,
    pub(super) stub: StubKind,
    pub(super) conv: Option<BusinessDayConvention>,
    /// Borrowed calendar (set by [`adjust_with`](Self::adjust_with)).
    /// Mutually exclusive with [`Self::deferred_calendar_id`].
    pub(super) cal: Option<&'a dyn HolidayCalendar>,
    /// Calendar ID to be resolved at [`build`](Self::build) time.
    ///
    /// Set by [`adjust_with_id`](Self::adjust_with_id) when the caller has a
    /// string ID rather than a borrowed `&dyn HolidayCalendar`. Deferred
    /// resolution is intentional: it lets the build path apply
    /// [`ScheduleErrorPolicy`] uniformly (strict / warning / graceful) when
    /// the registry lookup fails, instead of forcing every binding caller
    /// (Python, WASM) to thread the registry + error-policy themselves.
    /// Mutually exclusive with [`Self::cal`].
    pub(super) deferred_calendar_id: Option<String>,
    pub(super) eom: bool,
    /// Standard IMM mode (third Wednesday of Mar/Jun/Sep/Dec) for futures.
    pub(super) imm_mode: bool,
    /// CDS IMM mode (20th of Mar/Jun/Sep/Dec) for credit default swaps.
    pub(super) cds_imm_mode: bool,
    pub(super) error_policy: ScheduleErrorPolicy,
    /// Business days after each (adjusted) period end for the payment date.
    pub(super) payment_lag_business_days: i32,
    /// Optional T-minus business days from each period's accrual start.
    pub(super) fixing_lag_business_days: Option<i32>,
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
                    crate::dates::TenorUnit::Months | crate::dates::TenorUnit::Years
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
        use crate::dates::calendar::calendar_by_id_strict;

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
pub(super) fn strict_fail_closed_on_warnings(
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
