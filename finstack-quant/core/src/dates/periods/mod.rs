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

use crate::dates::date_extensions::DateExt;
use crate::dates::Date;
use core::fmt;
use core::str::FromStr;
use time::{Duration, Month};

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
/// Parses the snake_case wire values via [`std::str::FromStr`]
/// (for example, `"quarterly"` or `"semi_annual"`). The parser also accepts
/// the `"semiannual"` spelling and the pandas offset aliases `D`/`B`
/// (daily), `W` (weekly), `M`/`ME` (monthly), `Q`/`QE` (quarterly) and
/// `A`/`Y`/`YE` (annual); serde (de)serialization stays strict snake_case.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeriodKind::Daily => f.write_str("daily"),
            PeriodKind::Weekly => f.write_str("weekly"),
            PeriodKind::Monthly => f.write_str("monthly"),
            PeriodKind::Quarterly => f.write_str("quarterly"),
            PeriodKind::SemiAnnual => f.write_str("semi_annual"),
            PeriodKind::Annual => f.write_str("annual"),
        }
    }
}

impl FromStr for PeriodKind {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "daily" | "D" | "B" => Ok(PeriodKind::Daily),
            "weekly" | "W" => Ok(PeriodKind::Weekly),
            "monthly" | "M" | "ME" => Ok(PeriodKind::Monthly),
            "quarterly" | "Q" | "QE" => Ok(PeriodKind::Quarterly),
            "semi_annual" | "semiannual" => Ok(PeriodKind::SemiAnnual),
            "annual" | "A" | "Y" | "YE" => Ok(PeriodKind::Annual),
            _ => Err(crate::Error::Validation(format!(
                "unknown period kind '{s}'; expected one of daily, weekly, monthly, quarterly, \
                 semi_annual, annual (or a pandas offset alias D, B, W, M, Q, A, Y)"
            ))),
        }
    }
}

impl PeriodKind {
    /// Get the number of periods per year for this frequency.
    ///
    /// # Returns
    /// - Daily: 252 (trading-day convention)
    /// - Quarterly: 4
    /// - Monthly: 12
    /// - Weekly: 52
    /// - Semi-Annual: 2
    /// - Annual: 1
    pub fn periods_per_year(self) -> u16 {
        match self {
            PeriodKind::Daily => 252,
            PeriodKind::Quarterly => 4,
            PeriodKind::Monthly => 12,
            PeriodKind::Weekly => 52,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
        }
    }

    /// Annualization factor for this frequency.
    ///
    /// Used to scale per-period statistics to annual equivalents.
    /// For all variants this equals `periods_per_year()` cast to `f64`.
    pub fn annualization_factor(self) -> f64 {
        self.periods_per_year() as f64
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
    /// The prior observation date. Saturates at [`Date::MIN`] if
    /// subtraction would underflow the calendar.
    #[must_use]
    pub fn prior_observation_date(self, first: Date) -> Date {
        match self {
            Self::Daily => first.checked_sub(Duration::days(1)).unwrap_or(Date::MIN),
            Self::Weekly => first.checked_sub(Duration::days(7)).unwrap_or(Date::MIN),
            Self::Monthly => first.add_months(-1),
            Self::Quarterly => first.add_months(-3),
            Self::SemiAnnual => first.add_months(-6),
            Self::Annual => first.add_months(-12),
        }
    }

    #[inline]
    fn designator(self) -> Option<char> {
        match self {
            PeriodKind::Daily => Some('D'),
            PeriodKind::Quarterly => Some('Q'),
            PeriodKind::Monthly => Some('M'),
            PeriodKind::Weekly => Some('W'),
            PeriodKind::SemiAnnual => Some('H'),
            PeriodKind::Annual => None,
        }
    }

    #[inline]
    fn build_id(self, year: i32, index: u16) -> PeriodId {
        PeriodId {
            year,
            index,
            kind: self,
            fiscal: false,
        }
    }

    #[inline]
    fn build_fiscal_id(self, year: i32, index: u16) -> PeriodId {
        PeriodId {
            year,
            index,
            kind: self,
            fiscal: true,
        }
    }

    #[inline]
    fn relative_max_index(self) -> u16 {
        match self {
            PeriodKind::Daily => 366,
            PeriodKind::Quarterly => 4,
            PeriodKind::Monthly => 12,
            PeriodKind::Weekly => 53,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
        }
    }

    fn parse_index_with_limit(self, raw: &str, max_index: u16) -> crate::Result<u16> {
        let index: u16 = raw
            .parse()
            .map_err(|_| invalid_period(raw, "index is not a positive integer"))?;
        if !(1..=max_index).contains(&index) {
            return Err(invalid_period(
                raw,
                &format!("{self} index must be in 1..={max_index}"),
            ));
        }
        Ok(index)
    }

    fn gregorian_bounds(self, year: i32, index: u16) -> crate::Result<(Date, Date)> {
        match self {
            PeriodKind::Daily => daily_bounds(year, index),
            PeriodKind::Quarterly => quarter_bounds(year, index as u8),
            PeriodKind::Monthly => month_bounds(year, index as u8),
            PeriodKind::Weekly => week_bounds(year, index as u8),
            PeriodKind::SemiAnnual => half_bounds(year, index as u8),
            PeriodKind::Annual => annual_bounds(year),
        }
    }

    fn fiscal_bounds(
        self,
        fiscal_year: i32,
        index: u16,
        config: FiscalConfig,
    ) -> crate::Result<(Date, Date)> {
        match self {
            PeriodKind::Daily => fiscal_daily_bounds(fiscal_year, index, config),
            PeriodKind::Quarterly => fiscal_quarter_bounds(fiscal_year, index as u8, config),
            PeriodKind::Monthly => fiscal_month_bounds(fiscal_year, index as u8, config),
            PeriodKind::Weekly => fiscal_week_bounds(fiscal_year, index as u8, config),
            PeriodKind::SemiAnnual => fiscal_half_bounds(fiscal_year, index as u8, config),
            PeriodKind::Annual => fiscal_annual_bounds(fiscal_year, config),
        }
    }

    fn max_index_for_year(self, year: i32) -> u16 {
        match self {
            PeriodKind::Daily => days_in_year(year),
            PeriodKind::Quarterly => 4,
            PeriodKind::Monthly => 12,
            PeriodKind::Weekly => iso_weeks_in_year(year) as u16,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
        }
    }

    fn step_forward(self, mut year: i32, mut index: u16) -> (i32, u16) {
        let max = self.max_index_for_year(year);
        if index >= max {
            year += 1;
            index = 1;
        } else {
            index += 1;
        }
        (year, index)
    }

    fn step_backward(self, mut year: i32, mut index: u16) -> (i32, u16) {
        if index == 1 {
            year -= 1;
            index = self.max_index_for_year(year);
        } else {
            index -= 1;
        }
        (year, index)
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
    pub year: i32,
    /// Ordinal index within the year (depends on `kind`).
    /// - Daily:   1..=366 (ordinal day of the calendar year)
    /// - Quarter: 1..=4
    /// - Month:   1..=12
    /// - Week:    1..=53 (ISO 8601 week-year numbering)
    /// - Half:    1..=2
    /// - Annual:  1
    pub index: u16,
    /// Kind of the period.
    kind: PeriodKind,
    /// Whether this identifier uses fiscal (`FY...`) rather than Gregorian/ISO semantics.
    fiscal: bool,
}

impl PeriodId {
    /// Identify the Gregorian or ISO period containing a valid date.
    ///
    /// # Arguments
    ///
    /// * `date` - Valid calendar date to assign to a period; weekly periods use its ISO week-year.
    /// * `kind` - Calendar bucket granularity. Annual periods use the Gregorian year, not a fiscal label.
    pub fn from_date(date: Date, kind: PeriodKind) -> Self {
        let mut year = date.year();
        let month = u16::from(date.month() as u8);
        let index = match kind {
            PeriodKind::Daily => date.ordinal(),
            PeriodKind::Weekly => {
                let (y, week, _) = date.to_iso_week_date();
                year = y;
                u16::from(week)
            }
            PeriodKind::Monthly => month,
            PeriodKind::Quarterly => (month - 1) / 3 + 1,
            PeriodKind::SemiAnnual => (month - 1) / 6 + 1,
            PeriodKind::Annual => 1,
        };
        Self {
            year,
            index,
            kind,
            fiscal: false,
        }
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
        Self::try_new(year, ordinal, PeriodKind::Daily, days_in_year(year))
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
        Self::try_new(year, u16::from(q), PeriodKind::Quarterly, 4)
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
        Self::try_new(year, u16::from(m), PeriodKind::Monthly, 12)
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
        Self::try_new(
            year,
            u16::from(w),
            PeriodKind::Weekly,
            u16::from(iso_weeks_in_year(year)),
        )
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
        Self::try_new(year, u16::from(h), PeriodKind::SemiAnnual, 2)
    }
    /// Build an annual identifier.
    pub fn annual(year: i32) -> Self {
        Self {
            year,
            index: 1,
            kind: PeriodKind::Annual,
            fiscal: false,
        }
    }

    fn try_new(year: i32, index: u16, kind: PeriodKind, max: u16) -> crate::Result<Self> {
        if !(1..=max).contains(&index) {
            return Err(invalid_period(
                &format!("{kind} {year} index {index}"),
                &format!("{kind} index must be in 1..={max} for {year}"),
            ));
        }
        Ok(Self {
            year,
            index,
            kind,
            fiscal: false,
        })
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

    /// Get the number of periods per year for this frequency.
    ///
    /// # Returns
    /// - Quarterly: 4
    /// - Monthly: 12
    /// - Weekly: 52
    /// - Semi-Annual: 2
    /// - Annual: 1
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::PeriodId;
    ///
    /// let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// assert_eq!(q1.periods_per_year(), 4);
    ///
    /// let m1 = PeriodId::month(2025, 1).expect("valid period fixture");
    /// assert_eq!(m1.periods_per_year(), 12);
    /// ```
    pub fn periods_per_year(&self) -> u16 {
        self.kind.periods_per_year()
    }

    /// Step forward to the next period.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::PeriodId;
    ///
    /// let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let q2 = q1.next().expect("Next period should exist");
    /// assert_eq!(q2, PeriodId::quarter(2025, 2).expect("valid period fixture"));
    ///
    /// let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");
    /// let next_q1 = q4.next().expect("Next period should exist");
    /// assert_eq!(next_q1, PeriodId::quarter(2026, 1).expect("valid period fixture"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error for fiscal (`FY...`) identifiers because their year
    /// capacity depends on a [`FiscalConfig`]. Use [`Self::next_fiscal`].
    pub fn next(self) -> crate::Result<Self> {
        if self.fiscal {
            return Err(crate::Error::Validation(
                "PeriodId::next cannot step a fiscal identifier; use next_fiscal with an explicit FiscalConfig"
                    .to_string(),
            ));
        }
        step(self)
    }

    /// Step backward to the previous period.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::PeriodId;
    ///
    /// let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    /// let q1 = q2.prev().expect("Previous period should exist");
    /// assert_eq!(q1, PeriodId::quarter(2025, 1).expect("valid period fixture"));
    ///
    /// let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let prev_q4 = q1.prev().expect("Previous period should exist");
    /// assert_eq!(prev_q4, PeriodId::quarter(2024, 4).expect("valid period fixture"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error for fiscal (`FY...`) identifiers because their year
    /// capacity depends on a [`FiscalConfig`]. Use [`Self::prev_fiscal`].
    pub fn prev(self) -> crate::Result<Self> {
        if self.fiscal {
            return Err(crate::Error::Validation(
                "PeriodId::prev cannot step a fiscal identifier; use prev_fiscal with an explicit FiscalConfig"
                    .to_string(),
            ));
        }
        step_backward(self)
    }

    /// Step forward using the capacity of the supplied fiscal calendar.
    ///
    /// This differs from [`Self::next`] for weekly and daily identifiers:
    /// fiscal years may contain a partial week 53 or a leap-day ordinal even
    /// when the corresponding Gregorian/ISO year does not.
    ///
    /// # Errors
    ///
    /// Returns an error when `config` has an invalid fiscal start date or the
    /// next fiscal-year boundary cannot be represented by `time::Date`.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration object controlling validation, rounding, or solver behavior
    pub fn next_fiscal(self, config: FiscalConfig) -> crate::Result<Self> {
        let mut next = step_with_calendar(self, &FiscalCalendar { config }, true)?;
        next.fiscal = true;
        Ok(next)
    }

    /// Step backward using the capacity of the supplied fiscal calendar.
    ///
    /// This is the inverse of [`Self::next_fiscal`].
    ///
    /// # Errors
    ///
    /// Returns an error when `config` has an invalid fiscal start date or the
    /// preceding fiscal-year boundary cannot be represented by `time::Date`.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration object controlling validation, rounding, or solver behavior
    pub fn prev_fiscal(self, config: FiscalConfig) -> crate::Result<Self> {
        let mut prev = step_with_calendar(self, &FiscalCalendar { config }, false)?;
        prev.fiscal = true;
        Ok(prev)
    }
}

/// Configuration for fiscal year periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FiscalConfig {
    /// The month when the fiscal year starts (1-12).
    pub start_month: u8,
    /// The day of the month when the fiscal year starts (1-31).
    pub start_day: u8,
}

impl FiscalConfig {
    /// Create a new fiscal configuration.
    ///
    /// This validates the independent month and day ranges. It does not reject
    /// a day such as February 31 until that configuration is applied to a
    /// concrete fiscal year, because leap-year validity is year-dependent.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the offending value when
    /// `start_month` is outside `1..=12` or `start_day` is outside `1..=31`.
    pub fn new(start_month: u8, start_day: u8) -> crate::Result<Self> {
        if !(1..=12).contains(&start_month) {
            return Err(crate::Error::Validation(format!(
                "fiscal start_month must be in 1..=12, got {start_month}"
            )));
        }
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

    /// Standard calendar year (January 1).
    pub fn calendar_year() -> Self {
        Self {
            start_month: 1,
            start_day: 1,
        }
    }

    /// US Federal fiscal year (October 1).
    pub fn us_federal() -> Self {
        Self {
            start_month: 10,
            start_day: 1,
        }
    }

    /// UK fiscal year (April 6).
    pub fn uk() -> Self {
        Self {
            start_month: 4,
            start_day: 6,
        }
    }

    /// Japanese fiscal year (April 1).
    pub fn japan() -> Self {
        Self {
            start_month: 4,
            start_day: 1,
        }
    }

    /// Australian fiscal year (July 1).
    pub fn australia() -> Self {
        Self {
            start_month: 7,
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
pub struct PeriodPlan {
    /// Ordered periods produced by the parser.
    pub periods: Vec<Period>,
}

impl PeriodPlan {
    /// Iterate over periods in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = &Period> {
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
///
/// # Arguments
///
/// * `range` - Period range expression using the crate's calendar-period syntax
/// * `actuals_until` - Optional inclusive boundary separating actuals from forecasts
///
/// # Returns
///
/// A `PeriodPlan` containing periods in ascending order.
///
/// # Errors
///
/// Returns an error if the range cannot be parsed, the start and end identifiers
/// are incompatible, or the `actuals_until` boundary cannot be parsed.
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
    build_periods_with_calendar(range, Gregorian, actuals_until)
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
/// * `actuals_until` - Optional inclusive fiscal-period boundary for actual results
///
/// # Returns
///
/// A `PeriodPlan` expressed in fiscal-period identifiers and calendar dates.
///
/// # Errors
///
/// Returns an error if the fiscal identifiers cannot be parsed or if the fiscal
/// configuration produces invalid calendar boundaries.
pub fn build_fiscal_periods(
    range: &str,
    fiscal_config: FiscalConfig,
    actuals_until: Option<&str>,
) -> crate::Result<PeriodPlan> {
    build_periods_with_calendar(
        range,
        FiscalCalendar {
            config: fiscal_config,
        },
        actuals_until,
    )
}

// Minimal calendar abstraction to unify bounds computation across calendar and fiscal paths.
impl PartialOrd for PeriodId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PeriodId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare by year for a fast path across different calendar years.
        if self.year != other.year {
            return self.year.cmp(&other.year);
        }

        let self_kind = self.kind;
        let other_kind = other.kind;

        // Within the same frequency kind and year, order by index.
        if self_kind == other_kind {
            return self
                .index
                .cmp(&other.index)
                .then(self.fiscal.cmp(&other.fiscal));
        }

        // Mixed frequencies in the same year: order by actual calendar span
        // (start date, then end date) using Gregorian bounds.
        let greg = Gregorian;
        let self_bounds = greg.bounds(self.year, self.kind, self.index);
        let other_bounds = greg.bounds(other.year, other.kind, other.index);

        // Malformed PeriodId still needs a total ordering.
        let (Ok((self_start, self_end)), Ok((other_start, other_end))) =
            (self_bounds, other_bounds)
        else {
            return self_kind
                .cmp(&other_kind)
                .then(self.index.cmp(&other.index));
        };

        let by_start = self_start.cmp(&other_start);
        if by_start != std::cmp::Ordering::Equal {
            return by_start;
        }
        let by_end = self_end.cmp(&other_end);
        if by_end != std::cmp::Ordering::Equal {
            return by_end;
        }

        // Deterministic tie-breaker (should be extremely rare): stable kind then index.
        let by_kind = self_kind.cmp(&other_kind);
        if by_kind != std::cmp::Ordering::Equal {
            return by_kind;
        }
        self.index.cmp(&other.index)
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
    type Err = crate::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_id(s)
    }
}

// Implement From<PeriodId> for String to enable serde(into = "String")
impl From<PeriodId> for String {
    fn from(period: PeriodId) -> Self {
        period.to_string()
    }
}

// Implement TryFrom<String> for PeriodId to enable serde(try_from = "String")
impl TryFrom<String> for PeriodId {
    type Error = crate::error::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

mod calendar;
mod parse;
#[cfg(test)]
mod tests;
use calendar::*;
use parse::*;
