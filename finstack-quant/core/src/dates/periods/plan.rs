//! Fiscal configuration, concrete [`Period`] windows, and [`PeriodPlan`] builders.

use super::calendar::{PeriodCalendar, build_periods_with_calendar};
use super::parse::{checked_year_offset, invalid_period};
use super::period_id::PeriodId;
use crate::dates::Date;
use time::Month;

/// Configuration for fiscal year periods.
///
/// Always holds a start month in `1..=12` and a start day in `1..=31`;
/// deserialization applies the same validation as [`FiscalConfig::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "FiscalConfigFields", into = "FiscalConfigFields")]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FiscalConfig {
    /// The month when the fiscal year starts.
    start_month: Month,
    /// The day of the month when the fiscal year starts (1-31).
    start_day: u8,
}

/// Wire shape of [`FiscalConfig`]: plain month and day numbers.
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FiscalConfigFields {
    #[cfg_attr(feature = "json-schema", schemars(range(min = 1, max = 12)))]
    start_month: u8,
    #[cfg_attr(feature = "json-schema", schemars(range(min = 1, max = 31)))]
    start_day: u8,
}

impl TryFrom<FiscalConfigFields> for FiscalConfig {
    type Error = crate::Error;

    fn try_from(fields: FiscalConfigFields) -> crate::Result<Self> {
        Self::new(fields.start_month, fields.start_day)
    }
}

impl From<FiscalConfig> for FiscalConfigFields {
    fn from(config: FiscalConfig) -> Self {
        Self {
            start_month: config.start_month(),
            start_day: config.start_day,
        }
    }
}

impl FiscalConfig {
    /// Create a new fiscal configuration.
    ///
    /// This validates the independent month and day ranges. It does not reject
    /// a day such as February 31 until that configuration is applied to a
    /// concrete fiscal year, because leap-year validity is year-dependent.
    ///
    /// # Arguments
    ///
    /// * `start_month` - Month the fiscal year starts in (1 = January … 12).
    /// * `start_day` - Day of that month the fiscal year starts on (1-31).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when
    /// `start_month` is outside `1..=12` or `start_day` is outside `1..=31`.
    pub fn new(start_month: u8, start_day: u8) -> crate::Result<Self> {
        let start_month = Month::try_from(start_month).map_err(|_| {
            crate::Error::Validation(format!(
                "fiscal start_month must be in 1..=12, got {start_month}"
            ))
        })?;
        if !(1..=31).contains(&start_day) {
            return Err(crate::Error::Validation(format!(
                "fiscal start_day must be in 1..=31, got {start_day}"
            )));
        }
        Ok(Self {
            start_month,
            start_day,
        })
    }

    /// Month the fiscal year starts in (1 = January … 12).
    pub fn start_month(&self) -> u8 {
        self.start_month as u8
    }

    /// Day of [`Self::start_month`] the fiscal year starts on (1-31).
    pub fn start_day(&self) -> u8 {
        self.start_day
    }

    /// First calendar date of a fiscal year.
    ///
    /// Fiscal years are labelled by the calendar year they end in, so every
    /// start other than January 1 falls in the previous calendar year
    /// (FY2025 under an October 1 start begins 2024-10-01). A start day past
    /// the month's end (for example February 30) clamps to its last day.
    ///
    /// # Arguments
    ///
    /// * `fiscal_year` - Fiscal-year label, such as the `2025` in `FY2025`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the start date falls outside the
    /// supported calendar range.
    pub fn year_start(self, fiscal_year: i32) -> crate::Result<Date> {
        let calendar_year = if self.start_month == Month::January && self.start_day == 1 {
            fiscal_year
        } else {
            checked_year_offset(fiscal_year, -1)?
        };
        crate::dates::create_date(
            calendar_year,
            self.start_month,
            self.start_day.min(self.start_month.length(calendar_year)),
        )
    }

    /// Fiscal year opening on January 1, labelled by its own calendar year.
    ///
    /// There is no implicit default or jurisdiction preset (record
    /// CORE-CONVENTIONS-014 #2): callers state their actual opening month
    /// and day with [`Self::new`], or select this January 1 rule explicitly.
    pub fn calendar_year() -> Self {
        Self {
            start_month: Month::January,
            start_day: 1,
        }
    }
}

/// A concrete period with start/end dates and actual/forecast flag.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Period {
    /// Identifier of this period.
    pub id: PeriodId,
    /// Inclusive start date.
    #[cfg_attr(feature = "json-schema", schemars(with = "String", extend("format" = "date")))]
    pub start: Date,
    /// Exclusive end date.
    #[cfg_attr(feature = "json-schema", schemars(with = "String", extend("format" = "date")))]
    pub end: Date,
    /// True when this period is part of the "actuals" subset.
    pub is_actual: bool,
}

/// Builder/plan for a contiguous sequence of periods and their actual/forecast split.
///
/// Periods are returned in ascending order and are intended to form a contiguous
/// run of model periods. Each [`Period`] uses the crate-wide `[start, end)`
/// interval convention, so the `end` of one period naturally aligns with the
/// `start` of the next.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "PeriodPlanWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct PeriodPlan {
    /// Calendar context of every period in the plan, retained once.
    pub(super) calendar: PeriodCalendar,
    /// Chronologically ordered reporting periods, with optional gaps.
    pub(super) periods: Vec<Period>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PeriodPlanWire {
    calendar: PeriodCalendar,
    periods: Vec<Period>,
}

impl TryFrom<PeriodPlanWire> for PeriodPlan {
    type Error = crate::Error;

    fn try_from(wire: PeriodPlanWire) -> crate::Result<Self> {
        Self::try_new(wire.calendar, wire.periods)
    }
}

impl PeriodPlan {
    /// Build a plan from periods of one calendar context.
    ///
    /// # Arguments
    ///
    /// * `calendar` - Calendar context the periods belong to, retained once.
    /// * `periods` - Periods in order; every identifier must belong to
    ///   `calendar` (fiscal identifiers to a fiscal calendar, unprefixed ones
    ///   to the Gregorian calendar).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when a period's identifier does not belong
    /// to `calendar`.
    pub fn try_new(calendar: PeriodCalendar, periods: Vec<Period>) -> crate::Result<Self> {
        if let Some(period) = periods
            .iter()
            .find(|period| period.id.is_fiscal() != calendar.is_fiscal())
        {
            return Err(invalid_period(
                &period.id.to_string(),
                &format!(
                    "a {} period cannot belong to a {} plan",
                    if period.id.is_fiscal() {
                        "fiscal"
                    } else {
                        "Gregorian"
                    },
                    if calendar.is_fiscal() {
                        "fiscal"
                    } else {
                        "Gregorian"
                    }
                ),
            ));
        }
        let mut previous: Option<&Period> = None;
        for period in &periods {
            let id = period.id;
            if (period.start, period.end) != calendar.bounds(id.year(), id.kind(), id.index())? {
                return Err(invalid_period(
                    &id.to_string(),
                    "reporting dates must match the calendar bounds of the period identifier",
                ));
            }
            if previous.is_some_and(|prev| prev.id.kind() != id.kind() || prev.end > period.start) {
                return Err(invalid_period(
                    &id.to_string(),
                    "periods must have one frequency and be unique, non-overlapping and in ascending order",
                ));
            }
            previous = Some(period);
        }
        Ok(Self { calendar, periods })
    }

    /// Check that the plan is a canonical reporting timeline.
    ///
    /// A canonical plan is nonempty, holds one period kind, gives every
    /// period the exact `[start, end)` bounds its identifier has in the
    /// plan's calendar, and is contiguous: each period starts where the
    /// previous one ends (record CORE-CONVENTIONS-014 #1). Construction
    /// already requires one kind, calendar bounds and ascending order; this
    /// adds that the plan is nonempty and has no gap. The actual or forecast
    /// flags are not examined.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the first period that breaks one of
    /// these rules, or when the plan is empty.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{build_periods, PeriodPlan};
    ///
    /// let plan = build_periods("2025Q1..Q3", None)?;
    /// plan.validate_canonical()?;
    ///
    /// let mut periods = plan.to_vec();
    /// periods.remove(1);
    /// let gap = PeriodPlan::try_new(plan.get_calendar(), periods)?;
    /// assert!(gap.validate_canonical().is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn validate_canonical(&self) -> crate::Result<()> {
        if self.periods.is_empty() {
            return Err(crate::Error::Validation(
                "a period plan must hold at least one period".to_string(),
            ));
        }
        for pair in self.periods.windows(2) {
            if let [previous, next] = pair
                && next.start != previous.end
            {
                return Err(invalid_period(
                    &next.id.to_string(),
                    &format!(
                        "a plan is contiguous, but this period starts at {} and {} ends at {}",
                        next.start, previous.id, previous.end
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Calendar that defines the canonical bounds of all reporting periods.
    pub fn get_calendar(&self) -> PeriodCalendar {
        self.calendar
    }

    /// Borrow the validated reporting rows in chronological order.
    pub fn get_periods(&self) -> &[Period] {
        &self.periods
    }

    /// Consume the plan and return its reporting rows in chronological order.
    pub fn into_periods(self) -> Vec<Period> {
        self.periods
    }
}

impl std::ops::Deref for PeriodPlan {
    type Target = [Period];

    /// The plan's periods in order; the calendar stays with the plan.
    fn deref(&self) -> &[Period] {
        &self.periods
    }
}

impl<'a> IntoIterator for &'a PeriodPlan {
    type Item = &'a Period;
    type IntoIter = std::slice::Iter<'a, Period>;

    fn into_iter(self) -> Self::IntoIter {
        self.periods.iter()
    }
}

/// Build periods from a range expression (e.g., "2025Q1..Q4" or "2024Q4..2025Q2").
///
/// The `range` string may stay within a single year (`"2025Q1..Q4"`) or cross
/// years (`"2024M10..2025M03"`). The start and end identifiers must use the
/// same frequency family.
///
/// If `actuals_until` is provided, every period with an identifier less than or
/// equal to that boundary is marked actual and later periods are marked forecast.
/// The cutoff must use the same period kind and fiscal/Gregorian identifiers
/// as the range; a quarterly cutoff is invalid for a monthly range.
///
/// # Arguments
///
/// * `range` - Period range expression using the crate's calendar-period syntax
/// * `actuals_until` - Optional inclusive boundary separating actuals from forecasts,
///   using the same period kind and calendar as `range`.
///
/// # Returns
///
/// A `PeriodPlan` containing periods in ascending order.
///
/// # Errors
///
/// Returns an error if the range cannot be parsed, the start and end identifiers
/// are incompatible, the `actuals_until` boundary cannot be parsed or uses a
/// different period kind/calendar, a period's
/// start or exclusive end is outside the supported date range, or the range
/// exceeds 100,000 periods. These checks precede allocation of the period list.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::build_periods;
///
/// let plan = build_periods("2025Q1..Q4", Some("2025Q2"))?;
/// assert_eq!(plan.len(), 4);
/// assert!(plan[1].is_actual);
/// assert!(!plan[2].is_actual);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn build_periods(range: &str, actuals_until: Option<&str>) -> crate::Result<PeriodPlan> {
    build_periods_with_calendar(range, PeriodCalendar::Gregorian, actuals_until)
}

/// Build fiscal periods from a range expression with custom fiscal year configuration.
///
/// The period IDs (for example `"FY2025Q1"`) refer to fiscal periods, not
/// Gregorian calendar quarters. The returned `start`/`end` dates are mapped onto
/// calendar dates using `fiscal_config`.
///
/// # Arguments
///
/// * `range` - Fiscal period range expression
/// * `fiscal_config` - Fiscal-year start-month configuration
/// * `actuals_until` - Optional inclusive fiscal-period boundary for actual results,
///   with the same period kind as `range`; unprefixed identifiers are fiscal.
///
/// # Returns
///
/// A `PeriodPlan` expressed in fiscal-period identifiers and calendar dates.
///
/// # Errors
///
/// Returns an error if the fiscal identifiers cannot be parsed, the actuals
/// cutoff has a different period kind from the range, or if the fiscal
/// configuration produces invalid calendar boundaries, a period's start or
/// exclusive end is outside the supported date range, or the range exceeds
/// 100,000 periods. These checks precede allocation of the period list.
pub fn build_fiscal_periods(
    range: &str,
    fiscal_config: FiscalConfig,
    actuals_until: Option<&str>,
) -> crate::Result<PeriodPlan> {
    build_periods_with_calendar(range, PeriodCalendar::Fiscal(fiscal_config), actuals_until)
}
