//! Serializable schedule specification.

use super::*;

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
    pub payment_lag_days: i32,
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
    payment_lag_days: i32,
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
            payment_lag_days: wire.payment_lag_days,
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
        if start > end {
            return Err(crate::error::InputError::InvalidDateRange.into());
        }
        Ok(Self {
            start,
            end,
            frequency: Tenor::monthly(),
            stub: StubKind::None,
            business_day_convention: None,
            calendar_id: None,
            end_of_month: false,
            imm_mode: false,
            cds_imm_mode: false,
            error_policy: ScheduleErrorPolicy::Strict,
            payment_lag_days: 0,
            fixing_lag_business_days: None,
        })
    }

    /// Switch the specification to standard IMM dates, with
    /// [`ScheduleBuilder::imm`] semantics.
    ///
    /// Sets `frequency` to quarterly, `stub` to [`StubKind::ShortBack`],
    /// turns `imm_mode` on and `cds_imm_mode` off. As on the builder, a later
    /// change to `frequency` or `stub` wins: [`build`](Self::build) uses the
    /// stored values as they are.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::dates::{ScheduleSpec, StubKind, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2025, Month::December, 31)?;
    /// let spec = ScheduleSpec::new(start, end)?.with_imm();
    /// assert!(spec.imm_mode && !spec.cds_imm_mode);
    /// assert_eq!(spec.frequency, Tenor::quarterly());
    /// assert_eq!(spec.stub, StubKind::ShortBack);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn with_imm(mut self) -> Self {
        self.frequency = Tenor::quarterly();
        self.stub = StubKind::ShortBack;
        self.imm_mode = true;
        self.cds_imm_mode = false;
        self
    }

    /// Switch the specification to CDS IMM dates (the 20th of March, June,
    /// September and December), with [`ScheduleBuilder::cds_imm`] semantics.
    ///
    /// Sets `frequency` to quarterly, `stub` to [`StubKind::ShortBack`],
    /// turns `cds_imm_mode` on and `imm_mode` off. As on the builder, a later
    /// change to `frequency` or `stub` wins: CDS IMM generation uses the stored
    /// values.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::dates::{ScheduleSpec, StubKind, Tenor};
    /// use time::{Date, Month};
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let end = Date::from_calendar_date(2026, Month::January, 15)?;
    /// let spec = ScheduleSpec::new(start, end)?.with_cds_imm();
    /// assert!(spec.cds_imm_mode && !spec.imm_mode);
    /// assert_eq!(spec.frequency, Tenor::quarterly());
    /// assert_eq!(spec.stub, StubKind::ShortBack);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn with_cds_imm(mut self) -> Self {
        self.frequency = Tenor::quarterly();
        self.stub = StubKind::ShortBack;
        self.cds_imm_mode = true;
        self.imm_mode = false;
        self
    }

    /// Reconstruct a [`Schedule`] using the persisted configuration.
    ///
    /// This applies the same scheduling rules as [`ScheduleBuilder`], including
    /// stub handling, end-of-month logic, standard or CDS IMM mode, and the
    /// configured error policy. The stored `frequency` and `stub` are used as
    /// they are in every mode; [`with_imm`](Self::with_imm) and
    /// [`with_cds_imm`](Self::with_cds_imm) set them to the IMM defaults. Business-day adjustment is enabled only when
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
        if self.start > self.end {
            return Err(crate::error::InputError::InvalidDateRange.into());
        }
        let mut spec = self.clone();
        if spec.business_day_convention.is_none() {
            spec.calendar_id = None;
        }
        ScheduleBuilder { spec, cal: None }.build()
    }
}
