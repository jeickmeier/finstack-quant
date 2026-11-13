//! Period system for financial statement and time-series modeling.
//!
//! Provides types and parsers for working with financial reporting periods
//! (quarters, months, years, etc.) commonly used in financial statement models
//! and forecast scenarios.
//!
//! # Features
//!
//! - Period identifiers: Q1-Q4, M01-M12, H1-H2, W01-W52, annual
//! - Range expressions: "2025Q1..Q4", "2024M06..2025M06"
//! - Fiscal year support with custom month offsets
//! - Actual vs forecast period tracking
//!
//! # Period Formats
//!
//! - **Quarterly**: 2025Q1, 2025Q2, 2025Q3, 2025Q4
//! - **Monthly**: 2025M01 through 2025M12
//! - **Semi-annual**: 2025H1, 2025H2
//! - **Weekly**: 2025W01 through 2025W52/53 (ISO 8601 week-year)
//! - **Annual**: 2025

use crate::dates::Date;
use crate::dates::date_extensions::DateExt;
use core::fmt;
use core::str::FromStr;
use time::Month;

/// Period frequency type.
///
/// Defines the frequency of periodic schedules (cashflow rolls, return-series
/// resampling, statement reporting). Each variant carries an implied
/// "periods-per-year" used by [`PeriodKind::periods_per_year`] and by
/// downstream annualization helpers in `finstack-quant-analytics`.
///
/// `Daily` follows the trading-day convention (252 per year), not the
/// calendar-day convention (365 per year). Use `Weekly` if you need
/// calendar-week granularity.
///
/// Parses only the canonical snake_case wire values via [`std::str::FromStr`]:
/// `daily`, `weekly`, `monthly`, `quarterly`, `semi_annual`, and `annual`.
/// Aliases, whitespace padding and case variants are rejected.
///
/// Ordering follows [`PeriodKind::rank`] (daily through annual). It is a
/// deterministic key order, not a chronology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodKind {
    /// Daily periods (252 trading days per year by convention)
    Daily,
    /// Quarterly periods (4 per year)
    Quarterly,
    /// Monthly periods (12 per year)
    Monthly,
    /// Weekly periods (ISO 8601 week-year, typically 52 or 53 per year)
    Weekly,
    /// Semi-annual periods (2 per year)
    SemiAnnual,
    /// Annual periods (1 per year)
    Annual,
}

impl fmt::Display for PeriodKind {
    /// Writes the serde label (`daily`, `semi_annual`, ...).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| fmt::Error)?)
    }
}

impl FromStr for PeriodKind {
    type Err = crate::Error;

    /// Parse one exact canonical observation frequency.
    ///
    /// # Arguments
    ///
    /// * `s` - One of daily, weekly, monthly, quarterly, semi_annual, or annual; aliases, case variants and padding are rejected.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse_with_context("unknown period kind", s, s)
    }
}

impl PartialOrd for PeriodKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PeriodKind {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PeriodKind {
    /// Structural rank used to order kinds and period identifiers:
    /// daily `0`, weekly `1`, monthly `2`, quarterly `3`, semi-annual `4`,
    /// annual `5` (record CORE-CONVENTIONS-014 #8).
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            PeriodKind::Daily => 0,
            PeriodKind::Weekly => 1,
            PeriodKind::Monthly => 2,
            PeriodKind::Quarterly => 3,
            PeriodKind::SemiAnnual => 4,
            PeriodKind::Annual => 5,
        }
    }

    /// Get the number of periods per year for this frequency.
    ///
    /// # Returns
    /// - Daily: 252 (trading-day convention)
    /// - Quarterly: 4
    /// - Monthly: 12
    /// - Weekly: 52
    /// - Semi-Annual: 2
    /// - Annual: 1
    pub const fn periods_per_year(self) -> u16 {
        match self {
            PeriodKind::Daily => 252,
            PeriodKind::Quarterly => 4,
            PeriodKind::Monthly => 12,
            PeriodKind::Weekly => 52,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
        }
    }

    /// Observation date immediately before `first` at this frequency.
    ///
    /// Reconstructs the synthetic prior price date so a return-aligned
    /// panel has a holding period of one frequency step, independent of
    /// irregular gaps in the observed date grid.
    ///
    /// - Daily: one calendar day before `first`
    /// - Weekly: seven calendar days before `first`
    /// - Monthly / quarterly / semi-annual / annual: step back 1 / 3 / 6 /
    ///   12 months, clamping to the last valid day of the target month
    ///
    /// # Arguments
    ///
    /// * `first` - First return-aligned observation date in the series.
    ///
    /// # Returns
    ///
    /// The prior observation date.
    ///
    /// # Errors
    ///
    /// Returns a validation error if subtracting the observation period underflows the calendar.
    pub fn prior_observation_date(self, first: Date) -> crate::Result<Date> {
        match self {
            Self::Daily => first.add_days(-1),
            Self::Weekly => first.add_days(-7),
            Self::Monthly => first.add_months(-1),
            Self::Quarterly => first.add_months(-3),
            Self::SemiAnnual => first.add_months(-6),
            Self::Annual => first.add_months(-12),
        }
    }

    /// Highest index `self` can take in any year (Gregorian or fiscal).
    fn capacity(self) -> u16 {
        match self {
            PeriodKind::Daily => 366,
            PeriodKind::Weekly => 53,
            _ => self.periods_per_year(),
        }
    }
}

/// Identifier for a Gregorian period like `2025Q1` or a fiscal period like
/// `FY2025W53`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(into = "String", try_from = "String")]
#[cfg_attr(feature = "json-schema", schemars(with = "String"))]
pub struct PeriodId {
    /// Gregorian or fiscal year label.
    year: i32,
    /// One-based index within the year; see [`PeriodId::index`].
    index: u16,
    /// Kind of the period.
    kind: PeriodKind,
    /// Whether this identifier uses fiscal (`FY...`) rather than Gregorian/ISO semantics.
    fiscal: bool,
}

impl PeriodId {
    /// Identify the calendar or fiscal period containing a date.
    ///
    /// Gregorian weeks use ISO week-years. Fiscal weeks are seven-day blocks
    /// from the configured year start, with a partial final week. Month-based
    /// fiscal periods use the same clamped boundaries as [`build_fiscal_periods`].
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date to classify within an inclusive-start, exclusive-end period.
    /// * `kind` - Daily, weekly, monthly, quarterly, semi-annual, or annual granularity.
    /// * `fiscal_config` - `None` uses Gregorian periods and ISO weeks. `Some(config)`
    ///   uses fiscal boundaries and an `FY` year label at every granularity.
    ///
    /// # Errors
    ///
    /// Returns a validation error if either boundary of the containing period
    /// cannot be represented in the supported date range.
    pub fn from_date(
        date: Date,
        kind: PeriodKind,
        fiscal_config: Option<FiscalConfig>,
    ) -> crate::Result<Self> {
        let (calendar, year, index) = if let Some(config) = fiscal_config {
            let calendar = PeriodCalendar::Fiscal(config);
            let year = date.fiscal_year(config);
            let index = match kind {
                PeriodKind::Daily | PeriodKind::Weekly => {
                    let offset = (date - config.year_start(year)?).whole_days() as u16;
                    if kind == PeriodKind::Daily {
                        offset + 1
                    } else {
                        offset / 7 + 1
                    }
                }
                _ => {
                    let mut index = 1;
                    while index < kind.periods_per_year()
                        && date >= calendar.bounds(year, kind, index)?.1
                    {
                        index += 1;
                    }
                    index
                }
            };
            (calendar, year, index)
        } else {
            let mut year = date.year();
            let month = u16::from(date.month() as u8);
            let index = match kind {
                PeriodKind::Daily => date.ordinal(),
                PeriodKind::Weekly => {
                    let (iso_year, week, _) = date.to_iso_week_date();
                    year = iso_year;
                    u16::from(week)
                }
                PeriodKind::Monthly => month,
                PeriodKind::Quarterly => (month - 1) / 3 + 1,
                PeriodKind::SemiAnnual => (month - 1) / 6 + 1,
                PeriodKind::Annual => 1,
            };
            (PeriodCalendar::Gregorian, year, index)
        };
        let (start, end) = calendar.bounds(year, kind, index)?;
        if !(start..end).contains(&date) {
            return Err(crate::Error::Validation(format!(
                "date {date} is outside the resolved {kind} period [{start}, {end})"
            )));
        }
        Ok(Self {
            year,
            index,
            kind,
            fiscal: calendar.is_fiscal(),
        })
    }

    /// Try to build a daily identifier from an ordinal day (1..=366).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when `ordinal` is not valid for `year`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar or fiscal year used to construct the period identifier.
    /// * `ordinal` - One-based day ordinal valid for the supplied Gregorian year.
    pub fn day(year: i32, ordinal: u16) -> crate::Result<Self> {
        Self::try_new(year, ordinal, PeriodKind::Daily)
    }
    /// Try to build a quarterly identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when `q` is outside `1..=4`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar or fiscal year containing the quarter.
    /// * `q` - One-based quarter number in the inclusive range `1..=4`.
    pub fn quarter(year: i32, q: u8) -> crate::Result<Self> {
        Self::try_new(year, u16::from(q), PeriodKind::Quarterly)
    }
    /// Try to build a monthly identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when `m` is outside `1..=12`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar or fiscal year used to construct the period identifier.
    /// * `m` - One-based month number in the inclusive range `1..=12`.
    pub fn month(year: i32, m: u8) -> crate::Result<Self> {
        Self::try_new(year, u16::from(m), PeriodKind::Monthly)
    }
    /// Try to build a weekly identifier for a Gregorian ISO week-year.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when `w` is not a valid ISO week number
    /// for `year`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar or fiscal year used to construct the period identifier.
    /// * `w` - One-based ISO week number valid for the supplied ISO week-year.
    pub fn week(year: i32, w: u8) -> crate::Result<Self> {
        Self::try_new(year, u16::from(w), PeriodKind::Weekly)
    }
    /// Try to build a semi-annual identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when `h` is outside `1..=2`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar or fiscal year used to construct the period identifier.
    /// * `h` - One-based half-year number in the inclusive range `1..=2`.
    pub fn half(year: i32, h: u8) -> crate::Result<Self> {
        Self::try_new(year, u16::from(h), PeriodKind::SemiAnnual)
    }
    /// Try to build a Gregorian annual identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the year's start or exclusive end
    /// (January 1 of the next year) is outside the supported date range.
    ///
    /// # Arguments
    ///
    /// * `year` - Gregorian calendar year.
    pub fn annual(year: i32) -> crate::Result<Self> {
        Self::try_new(year, 1, PeriodKind::Annual)
    }

    fn try_new(year: i32, index: u16, kind: PeriodKind) -> crate::Result<Self> {
        let max = PeriodCalendar::Gregorian.max_index(year, kind)?;
        if !(1..=max).contains(&index) {
            return Err(invalid_period(
                &format!("{kind} {year} index {index}"),
                &format!("{kind} index must be in 1..={max} for {year}"),
            ));
        }
        PeriodCalendar::Gregorian.bounds(year, kind, index)?;
        Ok(Self {
            year,
            index,
            kind,
            fiscal: false,
        })
    }

    /// Gregorian (or ISO week-year) or fiscal year label.
    pub fn year(&self) -> i32 {
        self.year
    }

    /// One-based index within the year, by kind.
    ///
    /// - Daily: 1..=366, ordinal day of the Gregorian or fiscal year
    /// - Weekly: 1..=53, ISO 8601 week, or 7-day block from the fiscal-year start
    /// - Monthly: 1..=12
    /// - Quarterly: 1..=4
    /// - Semi-annual: 1..=2
    /// - Annual: 1
    pub fn index(&self) -> u16 {
        self.index
    }

    /// Get the period kind (frequency).
    ///
    /// # Returns
    /// The frequency type of this period (Quarterly, Monthly, etc.)
    pub fn kind(&self) -> PeriodKind {
        self.kind
    }

    /// Whether this identifier uses fiscal-year (`FY...`) semantics.
    #[must_use]
    pub fn is_fiscal(&self) -> bool {
        self.fiscal
    }

    /// Inclusive start and exclusive end date of a Gregorian period.
    ///
    /// Daily ids cover one calendar day, weekly ids one ISO 8601 week (Monday
    /// to the next Monday), and monthly to annual ids their calendar months.
    /// Fiscal ids need their [`FiscalConfig`]; bound them with
    /// [`build_fiscal_periods`].
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a fiscal identifier, or when the period
    /// or its exclusive end is outside the supported date range.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{create_date, PeriodId};
    /// use time::Month;
    ///
    /// let (start, end) = PeriodId::quarter(2025, 2)?.bounds()?;
    /// assert_eq!(start, create_date(2025, Month::April, 1)?);
    /// assert_eq!(end, create_date(2025, Month::July, 1)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn bounds(self) -> crate::Result<(Date, Date)> {
        if self.fiscal {
            return Err(crate::Error::Validation(format!(
                "fiscal period {self} needs its FiscalConfig to be bounded; use build_fiscal_periods"
            )));
        }
        PeriodCalendar::Gregorian.bounds(self.year, self.kind, self.index)
    }

    /// Move by a whole number of periods of the same kind in `calendar`.
    ///
    /// The source is validated under `calendar` (even for a zero shift) and
    /// the result keeps its kind and calendar context (record
    /// CORE-CONVENTIONS-014 #7). Daily periods step by actual calendar days,
    /// so fiscal days traverse each fiscal year's real 365 or 366 days and
    /// Gregorian weeks step by seven days across ISO week-years. Fiscal weeks
    /// step through the 53 reporting blocks of each fiscal year (the last is
    /// the short final block). Monthly, quarterly, semi-annual and annual
    /// periods step ordinal reporting slots.
    ///
    /// # Arguments
    ///
    /// * `periods` - Signed number of periods to move; negative moves back
    ///   and `0` returns the identifier unchanged after validation.
    /// * `calendar` - The identifier's calendar context: `Gregorian` for an
    ///   unprefixed identifier, `Fiscal(config)` for an `FY` identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `calendar` does not match the
    /// identifier's fiscal flag, the identifier is not a valid period of
    /// `calendar`, or the result's year or bounds are outside the supported
    /// range.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{FiscalConfig, PeriodCalendar, PeriodId};
    ///
    /// let q3 = PeriodId::quarter(2025, 3)?;
    /// assert_eq!(q3.shift(-6, PeriodCalendar::Gregorian)?, PeriodId::quarter(2024, 1)?);
    /// let fiscal = "FY2025M11".parse::<PeriodId>()?;
    /// let october = PeriodCalendar::Fiscal(FiscalConfig::new(10, 1)?);
    /// assert_eq!(fiscal.shift(2, october)?.to_string(), "FY2026M01");
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn shift(self, periods: i32, calendar: PeriodCalendar) -> crate::Result<Self> {
        if self.fiscal != calendar.is_fiscal() {
            return Err(invalid_period(
                &self.to_string(),
                &format!(
                    "a {} identifier cannot be shifted in the {} calendar",
                    if self.fiscal { "fiscal" } else { "Gregorian" },
                    if calendar.is_fiscal() {
                        "fiscal"
                    } else {
                        "Gregorian"
                    }
                ),
            ));
        }
        let (start, _) = calendar.bounds(self.year, self.kind, self.index)?;
        let out_of_range = || {
            invalid_period(
                &self.to_string(),
                &format!("shifting by {periods} periods leaves the supported range"),
            )
        };
        let day_step = match (self.kind, calendar) {
            (PeriodKind::Daily, _) => Some(1),
            (PeriodKind::Weekly, PeriodCalendar::Gregorian) => Some(7),
            _ => None,
        };
        let target = if let Some(days) = day_step {
            let date = start
                .checked_add(time::Duration::days(i64::from(periods) * days))
                .ok_or_else(out_of_range)?;
            Self::from_date(date, self.kind, calendar.fiscal_config())?
        } else {
            let slots = i64::from(self.kind.capacity());
            let ordinal =
                i64::from(self.year) * slots + i64::from(self.index) - 1 + i64::from(periods);
            Self {
                year: i32::try_from(ordinal.div_euclid(slots)).map_err(|_| out_of_range())?,
                index: u16::try_from(ordinal.rem_euclid(slots) + 1).map_err(|_| out_of_range())?,
                ..self
            }
        };
        calendar.bounds(target.year, target.kind, target.index)?;
        Ok(target)
    }
}

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
    pub calendar: PeriodCalendar,
    /// Ordered periods produced by the parser.
    pub periods: Vec<Period>,
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
        Ok(Self { calendar, periods })
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
/// assert_eq!(plan.periods.len(), 4);
/// assert!(plan.periods[1].is_actual);
/// assert!(!plan.periods[2].is_actual);
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

impl PartialOrd for PeriodId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PeriodId {
    /// Structural total order on `(year, kind rank, index, fiscal)`.
    ///
    /// This is a deterministic key order consistent with equality, not a
    /// chronology across kinds or calendar contexts (record
    /// CORE-CONVENTIONS-014 #8): chronological work sorts resolved periods by
    /// their bounds under a known calendar.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.year, self.kind.rank(), self.index, self.fiscal).cmp(&(
            other.year,
            other.kind.rank(),
            other.index,
            other.fiscal,
        ))
    }
}

impl fmt::Display for PeriodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.fiscal {
            f.write_str("FY")?;
        }
        match self.kind {
            PeriodKind::Daily => write!(f, "{}D{:03}", self.year, self.index),
            PeriodKind::Quarterly => write!(f, "{}Q{}", self.year, self.index),
            PeriodKind::Monthly => write!(f, "{}M{:02}", self.year, self.index),
            PeriodKind::Weekly => write!(f, "{}W{:02}", self.year, self.index),
            PeriodKind::SemiAnnual => write!(f, "{}H{}", self.year, self.index),
            PeriodKind::Annual => write!(f, "{}", self.year),
        }
    }
}

impl FromStr for PeriodId {
    type Err = crate::Error;

    /// Parse a standalone identifier: unprefixed text is Gregorian and must
    /// have representable bounds; `FY` text is fiscal and is checked against
    /// symbolic capacities only (D366, W53, M12, Q4, H2, annual 1).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_id(s, IdContext::Standalone)
    }
}

impl From<PeriodId> for String {
    fn from(period: PeriodId) -> Self {
        period.to_string()
    }
}

impl TryFrom<String> for PeriodId {
    type Error = crate::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

mod calendar;
mod ltm;
mod parse;
#[cfg(test)]
mod tests;
pub use calendar::PeriodCalendar;
use calendar::*;
pub use ltm::{LtmWindow, ltm_window};
use parse::*;
