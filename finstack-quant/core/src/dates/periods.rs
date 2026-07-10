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
/// Parses from short and long string forms via [`std::str::FromStr`]
/// (e.g. `"q"` or `"quarterly"`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
            PeriodKind::SemiAnnual => f.write_str("semiannual"),
            PeriodKind::Annual => f.write_str("annual"),
        }
    }
}

impl FromStr for PeriodKind {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "daily" | "d" => Ok(PeriodKind::Daily),
            "weekly" | "w" => Ok(PeriodKind::Weekly),
            "monthly" | "m" => Ok(PeriodKind::Monthly),
            "quarterly" | "q" => Ok(PeriodKind::Quarterly),
            "semiannual" | "semi_annual" | "h" => Ok(PeriodKind::SemiAnnual),
            "annual" | "yearly" | "a" | "y" => Ok(PeriodKind::Annual),
            _ => Err(crate::error::InputError::Invalid.into()),
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
        let index = raw.parse().map_err(|_| crate::error::InputError::Invalid)?;
        if !(1..=max_index).contains(&index) {
            return Err(crate::error::InputError::Invalid.into());
        }
        Ok(index)
    }

    fn parse_absolute_index(self, year: i32, raw: &str) -> crate::Result<u16> {
        match self {
            PeriodKind::Weekly => self.parse_index_with_limit(raw, iso_weeks_in_year(year) as u16),
            PeriodKind::Annual => Ok(1),
            _ => self.parse_index_with_limit(raw, self.relative_max_index()),
        }
    }

    fn parse_relative_id(self, year: i32, rhs: &str) -> crate::Result<PeriodId> {
        let index = match self.designator() {
            Some(designator) => self.parse_index_with_limit(
                rhs.trim_start_matches(designator),
                self.relative_max_index(),
            )?,
            None => 1,
        };
        Ok(self.build_id(year, index))
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

/// Identifier for a period like 2025Q1 or 2025M03.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct PeriodId {
    /// Gregorian calendar year.
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
}

impl PeriodId {
    /// Build a daily identifier from an ordinal day (1..=366).
    pub fn day(year: i32, ordinal: u16) -> Self {
        Self {
            year,
            index: ordinal,
            kind: PeriodKind::Daily,
        }
    }
    /// Build a quarterly identifier.
    pub fn quarter(year: i32, q: u8) -> Self {
        Self {
            year,
            index: q as u16,
            kind: PeriodKind::Quarterly,
        }
    }
    /// Build a monthly identifier.
    pub fn month(year: i32, m: u8) -> Self {
        Self {
            year,
            index: m as u16,
            kind: PeriodKind::Monthly,
        }
    }
    /// Build a weekly identifier.
    pub fn week(year: i32, w: u8) -> Self {
        Self {
            year,
            index: w as u16,
            kind: PeriodKind::Weekly,
        }
    }
    /// Build a semi-annual identifier.
    pub fn half(year: i32, h: u8) -> Self {
        Self {
            year,
            index: h as u16,
            kind: PeriodKind::SemiAnnual,
        }
    }
    /// Build an annual identifier.
    pub fn annual(year: i32) -> Self {
        Self {
            year,
            index: 1,
            kind: PeriodKind::Annual,
        }
    }

    /// Get the period kind (frequency).
    ///
    /// # Returns
    /// The frequency type of this period (Quarterly, Monthly, etc.)
    pub fn kind(&self) -> PeriodKind {
        self.kind
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
    /// let q1 = PeriodId::quarter(2025, 1);
    /// assert_eq!(q1.periods_per_year(), 4);
    ///
    /// let m1 = PeriodId::month(2025, 1);
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
    /// let q1 = PeriodId::quarter(2025, 1);
    /// let q2 = q1.next().expect("Next period should exist");
    /// assert_eq!(q2, PeriodId::quarter(2025, 2));
    ///
    /// let q4 = PeriodId::quarter(2025, 4);
    /// let next_q1 = q4.next().expect("Next period should exist");
    /// assert_eq!(next_q1, PeriodId::quarter(2026, 1));
    /// ```
    pub fn next(self) -> crate::Result<Self> {
        step(self)
    }

    /// Step backward to the previous period.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::PeriodId;
    ///
    /// let q2 = PeriodId::quarter(2025, 2);
    /// let q1 = q2.prev().expect("Previous period should exist");
    /// assert_eq!(q1, PeriodId::quarter(2025, 1));
    ///
    /// let q1 = PeriodId::quarter(2025, 1);
    /// let prev_q4 = q1.prev().expect("Previous period should exist");
    /// assert_eq!(prev_q4, PeriodId::quarter(2024, 4));
    /// ```
    pub fn prev(self) -> crate::Result<Self> {
        step_backward(self)
    }
}

/// Configuration for fiscal year periods.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FiscalConfig {
    /// The month when the fiscal year starts (1-12).
    pub start_month: u8,
    /// The day of the month when the fiscal year starts (1-31).
    pub start_day: u8,
}

impl FiscalConfig {
    /// Create a new fiscal configuration.
    pub fn new(start_month: u8, start_day: u8) -> crate::Result<Self> {
        if !(1..=12).contains(&start_month) {
            return Err(crate::error::InputError::Invalid.into());
        }
        if !(1..=31).contains(&start_day) {
            return Err(crate::error::InputError::Invalid.into());
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

    /// Canadian fiscal year (April 1).
    pub fn canada() -> Self {
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

    /// German fiscal year (January 1).
    pub fn germany() -> Self {
        Self {
            start_month: 1,
            start_day: 1,
        }
    }

    /// French fiscal year (January 1).
    pub fn france() -> Self {
        Self {
            start_month: 1,
            start_day: 1,
        }
    }
}

/// A concrete period with start/end dates and actual/forecast flag.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Period {
    /// Identifier of this period.
    pub id: PeriodId,
    /// Inclusive start date.
    pub start: Date,
    /// Exclusive end date.
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
trait PeriodCalendar {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)>;
}

#[derive(Clone, Copy, Debug)]
struct Gregorian;

impl PeriodCalendar for Gregorian {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)> {
        kind.gregorian_bounds(year, index)
    }
}

#[derive(Clone, Copy, Debug)]
struct FiscalCalendar {
    config: FiscalConfig,
}

impl PeriodCalendar for FiscalCalendar {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)> {
        kind.fiscal_bounds(year, index, self.config)
    }
}

/// Generic builder using a provided calendar policy.
fn build_periods_with_calendar<C: PeriodCalendar>(
    range: &str,
    calendar: C,
    actuals_until: Option<&str>,
) -> crate::Result<PeriodPlan> {
    let (start, end) = parse_range(range)?;
    let mut ids = enumerate_ids(start, end)?;

    let actual_cut = actuals_until.map(parse_id).transpose()?;
    let periods = ids
        .drain(..)
        .map(|pid| make_period_with_calendar(pid, &calendar, actual_cut.as_ref()))
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(PeriodPlan { periods })
}

// (old local variants of make_period were replaced by calendar-based helper)

fn make_period_with_calendar<C: PeriodCalendar>(
    pid: PeriodId,
    calendar: &C,
    cut: Option<&PeriodId>,
) -> crate::Result<Period> {
    let (start, end) = calendar.bounds(pid.year, pid.kind, pid.index)?;
    let is_actual = cut.map(|c| pid <= *c).unwrap_or(false);
    Ok(Period {
        id: pid,
        start,
        end,
        is_actual,
    })
}

// Period bounds helpers are fallible to avoid sentinel dates and silent corruption.

fn daily_bounds(year: i32, ordinal: u16) -> crate::Result<(Date, Date)> {
    use time::Duration;
    let start =
        Date::from_ordinal_date(year, ordinal).map_err(|_| crate::error::InputError::Invalid)?;
    let end = start + Duration::days(1);
    Ok((start, end))
}

fn quarter_bounds(year: i32, q: u8) -> crate::Result<(Date, Date)> {
    let (sm, em) = match q {
        1 => (Month::January, Month::April),
        2 => (Month::April, Month::July),
        3 => (Month::July, Month::October),
        _ => (Month::October, Month::January),
    };
    let start = crate::dates::create_date(year, sm, 1)?;
    let end_year = if q == 4 { year + 1 } else { year };
    let end = crate::dates::create_date(end_year, em, 1)?;
    Ok((start, end))
}

fn month_bounds(year: i32, m: u8) -> crate::Result<(Date, Date)> {
    let sm = Month::try_from(m).map_err(|_| crate::error::InputError::Invalid)?;
    let start = crate::dates::create_date(year, sm, 1)?;
    let (ey, em) = if m == 12 {
        (year + 1, Month::January)
    } else {
        (
            year,
            Month::try_from(m + 1).map_err(|_| crate::error::InputError::Invalid)?,
        )
    };
    let end = crate::dates::create_date(ey, em, 1)?;
    Ok((start, end))
}

fn iso_weeks_in_year(year: i32) -> u8 {
    use time::Weekday;

    if Date::from_iso_week_date(year, 53, Weekday::Monday).is_ok() {
        53
    } else {
        52
    }
}

/// Calculate ISO 8601 week bounds for a given ISO week-year and week number.
fn week_bounds(year: i32, w: u8) -> crate::Result<(Date, Date)> {
    use time::Duration;
    use time::Weekday;

    if w == 0 || w > iso_weeks_in_year(year) {
        return Err(crate::error::InputError::Invalid.into());
    }
    let start = Date::from_iso_week_date(year, w, Weekday::Monday)
        .map_err(|_| crate::error::InputError::Invalid)?;
    let end = start + Duration::days(7);
    Ok((start, end))
}

fn half_bounds(year: i32, h: u8) -> crate::Result<(Date, Date)> {
    let jan1 = crate::dates::create_date(year, Month::January, 1)?;
    let jul1 = crate::dates::create_date(year, Month::July, 1)?;
    let jan1_next = crate::dates::create_date(year + 1, Month::January, 1)?;
    match h {
        1 => Ok((jan1, jul1)),
        _ => Ok((jul1, jan1_next)),
    }
}

fn annual_bounds(year: i32) -> crate::Result<(Date, Date)> {
    let start = crate::dates::create_date(year, Month::January, 1)?;
    let end = crate::dates::create_date(year + 1, Month::January, 1)?;
    Ok((start, end))
}

// Fiscal year bounds functions

fn fiscal_daily_bounds(
    fiscal_year: i32,
    ordinal: u16,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    use time::Duration;

    if ordinal == 0 {
        return Err(crate::error::InputError::Invalid.into());
    }
    let fy_start = fiscal_year_start(fiscal_year, config)?;
    let fy_end = fiscal_year_start(fiscal_year + 1, config)?;
    let start = fy_start + Duration::days(i64::from(ordinal - 1));
    if start >= fy_end {
        return Err(crate::error::InputError::Invalid.into());
    }
    Ok((start, (start + Duration::days(1)).min(fy_end)))
}

fn fiscal_quarter_bounds(
    fiscal_year: i32,
    q: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    // Calculate the start of the fiscal year
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    // Each quarter is 3 months
    let quarter_start_month_offset = (q - 1) * 3;
    let quarter_end_month_offset = q * 3;

    // Calculate start and end dates for the quarter
    let start = fy_start.add_months(quarter_start_month_offset as i32);
    let end = fy_start.add_months(quarter_end_month_offset as i32);

    Ok((start, end))
}

fn fiscal_month_bounds(
    fiscal_year: i32,
    m: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    // Calculate the start of the fiscal year
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    // Calculate start and end dates for the month
    let start = fy_start.add_months((m - 1) as i32);
    let end = fy_start.add_months(m as i32);

    Ok((start, end))
}

/// Calculate fiscal week bounds using simple fiscal year start anchoring.
///
/// Like regular week_bounds, this uses simple 7-day blocks starting from the
/// fiscal year start date, not ISO 8601 week numbering.
fn fiscal_week_bounds(
    fiscal_year: i32,
    w: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    use time::Duration;

    // Calculate the start of the fiscal year
    let fy_start = fiscal_year_start(fiscal_year, config)?;
    let fy_end = fiscal_year_start(fiscal_year + 1, config)?;

    // Calculate start and end dates for the week
    let start = fy_start + Duration::days(((w - 1) as i64) * 7);
    if start >= fy_end {
        return Err(crate::error::InputError::Invalid.into());
    }
    let end = (start + Duration::days(7)).min(fy_end);

    Ok((start, end))
}

fn fiscal_half_bounds(
    fiscal_year: i32,
    h: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    // Calculate the start of the fiscal year
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    // Each half is 6 months
    let half_start_month_offset = (h - 1) * 6;
    let half_end_month_offset = h * 6;

    let start = fy_start.add_months(half_start_month_offset as i32);
    let end = fy_start.add_months(half_end_month_offset as i32);

    Ok((start, end))
}

fn fiscal_annual_bounds(fiscal_year: i32, config: FiscalConfig) -> crate::Result<(Date, Date)> {
    let start = fiscal_year_start(fiscal_year, config)?;
    let end = fiscal_year_start(fiscal_year + 1, config)?;
    Ok((start, end))
}

/// Calculate the start date of a fiscal year
fn fiscal_year_start(fiscal_year: i32, config: FiscalConfig) -> crate::Result<Date> {
    // For fiscal years that start in months other than January,
    // we need to determine the correct calendar year
    let calendar_year = if config.start_month == 1 {
        fiscal_year
    } else {
        // Fiscal year starts in the previous calendar year
        // E.g., FY2025 starting Oct 1 begins on Oct 1, 2024
        // E.g., FY2025 starting Apr 1 begins on Apr 1, 2024
        fiscal_year - 1
    };

    let month =
        Month::try_from(config.start_month).map_err(|_| crate::error::InputError::Invalid)?;
    match crate::dates::create_date(calendar_year, month, config.start_day) {
        Ok(d) => Ok(d),
        Err(_) => {
            // If the day doesn't exist (e.g., Feb 30), use the last day of the month.
            let last_day = month.length(calendar_year);
            crate::dates::create_date(calendar_year, month, last_day)
        }
    }
}

fn parse_range(s: &str) -> crate::Result<(PeriodId, PeriodId)> {
    let s = s.trim();
    let (lhs, rhs_raw) = s
        .split_once("..")
        .ok_or(crate::error::InputError::Invalid)?;
    let start = parse_id(lhs)?;
    let rhs_raw = rhs_raw.trim();
    let rhs_upper = rhs_raw.to_ascii_uppercase();
    let rhs = rhs_upper.as_str();
    // Relative if RHS starts with a letter (Q/M/W/H/A). Absolute if it starts with a digit (YYYY...).
    let end = if rhs
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        parse_id(rhs)?
    } else {
        // relative form like "..D100" / "..Q4" / "..M12" / "..W52" / "..H2" / "..A"
        start.kind.parse_relative_id(start.year, rhs)?
    };
    // Validate period kind consistency and non-inverted ranges
    if start.kind != end.kind {
        return Err(crate::error::InputError::Invalid.into());
    }
    if start > end {
        return Err(crate::error::InputError::InvalidDateRange.into());
    }
    Ok((start, end))
}

fn parse_designated_id(s: &str, split_index: usize, kind: PeriodKind) -> crate::Result<PeriodId> {
    let year: i32 = s[..split_index]
        .parse()
        .map_err(|_| crate::error::InputError::Invalid)?;
    let index = kind.parse_absolute_index(year, &s[split_index + 1..])?;
    Ok(kind.build_id(year, index))
}

fn parse_id(s: &str) -> crate::Result<PeriodId> {
    let s = s.trim();
    // Normalize to uppercase to accept lowercase inputs.
    let s = s.to_ascii_uppercase();
    let s = s.as_str();
    if let Some(i) = s.find('D') {
        return parse_designated_id(s, i, PeriodKind::Daily);
    }
    if let Some(i) = s.find('Q') {
        return parse_designated_id(s, i, PeriodKind::Quarterly);
    }
    if let Some(i) = s.find('M') {
        return parse_designated_id(s, i, PeriodKind::Monthly);
    }
    if let Some(i) = s.find('W') {
        return parse_designated_id(s, i, PeriodKind::Weekly);
    }
    if let Some(i) = s.find('H') {
        return parse_designated_id(s, i, PeriodKind::SemiAnnual);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        // annual
        let year: i32 = s.parse().map_err(|_| crate::error::InputError::Invalid)?;
        return Ok(PeriodId::annual(year));
    }
    Err(crate::error::InputError::Invalid.into())
}

fn enumerate_ids(mut cur: PeriodId, end: PeriodId) -> crate::Result<Vec<PeriodId>> {
    let mut out = Vec::new();
    while cur <= end {
        out.push(cur);
        cur = step(cur)?;
    }
    Ok(out)
}

fn days_in_year(year: i32) -> u16 {
    if time::util::is_leap_year(year) {
        366
    } else {
        365
    }
}

fn step(mut id: PeriodId) -> crate::Result<PeriodId> {
    (id.year, id.index) = id.kind.step_forward(id.year, id.index);
    Ok(id)
}

/// Step backward by one period (inverse of step).
fn step_backward(mut id: PeriodId) -> crate::Result<PeriodId> {
    (id.year, id.index) = id.kind.step_backward(id.year, id.index);
    Ok(id)
}

// local helper removed; ordering uses Gregorian bounds directly

// Ordering helpers for PeriodId
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
            return self.index.cmp(&other.index);
        }

        // Mixed frequencies in the same year: order by actual calendar span
        // (start date, then end date) using Gregorian bounds.
        let greg = Gregorian;
        let self_bounds = greg.bounds(self.year, self.kind, self.index);
        let other_bounds = greg.bounds(other.year, other.kind, other.index);

        // Defensive fallback: bounds should be infallible for valid PeriodId values,
        // but if a malformed PeriodId slips through, we still need a total ordering.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(year: i32, month: Month, day: u8) -> Date {
        crate::dates::create_date(year, month, day).expect("valid date")
    }

    #[test]
    fn build_periods_weekly_uses_iso_week_bounds() {
        let plan = build_periods("2025W01..W01", None).expect("weekly plan");
        assert_eq!(plan.periods.len(), 1);
        let period = &plan.periods[0];
        assert_eq!(period.start, d(2024, Month::December, 30));
        assert_eq!(period.end, d(2025, Month::January, 6));
    }

    #[test]
    fn parse_id_rejects_invalid_iso_week_for_year() {
        assert!(PeriodId::from_str("2021W53").is_err());
        assert!(PeriodId::from_str("2020W53").is_ok());
    }

    #[test]
    fn next_rolls_to_next_iso_year_after_last_week() {
        let next = PeriodId::week(2021, 52).next().expect("next week");
        assert_eq!(next, PeriodId::week(2022, 1));
    }

    #[test]
    fn prev_rolls_to_previous_iso_year_last_week() {
        let prev = PeriodId::week(2022, 1).prev().expect("previous week");
        assert_eq!(prev, PeriodId::week(2021, 52));
    }

    #[test]
    fn period_kind_display_parse_and_counts() {
        assert_eq!(PeriodKind::SemiAnnual.to_string(), "semiannual");
        assert_eq!(
            PeriodKind::from_str("semi_annual"),
            Ok(PeriodKind::SemiAnnual)
        );
        assert_eq!(PeriodKind::from_str("Y"), Ok(PeriodKind::Annual));
        assert!(PeriodKind::from_str("unknown").is_err());

        assert_eq!(PeriodKind::Daily.periods_per_year(), 252);
        assert_eq!(PeriodKind::Quarterly.annualization_factor(), 4.0);
    }

    #[test]
    fn period_id_display_parse_and_serde_roundtrip() {
        let q = PeriodId::from_str("2025Q3").expect("quarter");
        assert_eq!(q.to_string(), "2025Q3");
        let m = PeriodId::from_str("2025m06").expect("month lowercase");
        assert_eq!(m, PeriodId::month(2025, 6));
        let d = PeriodId::from_str("2025D059").expect("ordinal day");
        assert_eq!(d, PeriodId::day(2025, 59));

        let json = serde_json::to_string(&q).expect("serialize");
        let back: PeriodId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, q);
    }

    #[test]
    fn period_id_ordering_mixed_frequencies_same_year() {
        let q1 = PeriodId::quarter(2025, 1);
        let m1 = PeriodId::month(2025, 1);
        assert!(q1 > m1);
    }

    #[test]
    fn build_periods_quarterly_monthly_daily_and_annual() {
        let q = build_periods("2025Q1..Q4", None).expect("quarters");
        assert_eq!(q.periods.len(), 4);
        assert_eq!(q.periods[0].start, d(2025, Month::January, 1));

        let cross = build_periods("2024M11..2025M02", None).expect("cross-year months");
        assert_eq!(cross.periods.len(), 4);

        let rel_m = build_periods("2025M01..M03", None).expect("relative months");
        assert_eq!(rel_m.periods.len(), 3);

        let rel_q = build_periods("2025Q1..Q3", Some("2025Q2")).expect("actuals boundary");
        assert!(rel_q.periods[0].is_actual);
        assert!(rel_q.periods[1].is_actual);
        assert!(!rel_q.periods[2].is_actual);

        let days = build_periods("2025D001..D003", None).expect("daily");
        assert_eq!(days.periods.len(), 3);

        let halves = build_periods("2025H1..H2", None).expect("halves");
        assert_eq!(halves.periods.len(), 2);

        let years = build_periods("2024..2026", None).expect("annual range");
        assert_eq!(years.periods.len(), 3);
    }

    #[test]
    fn build_periods_rejects_mixed_kinds_or_inverted_ranges() {
        assert!(build_periods("2025Q1..2025M01", None).is_err());
        assert!(build_periods("2025Q2..2025Q1", None).is_err());
    }

    #[test]
    fn fiscal_config_constructors_and_validation() {
        assert!(FiscalConfig::new(13, 1).is_err());
        assert!(FiscalConfig::new(1, 32).is_err());
        let uk = FiscalConfig::uk();
        assert_eq!(uk.start_month, 4);
        let feb_edge = FiscalConfig::new(2, 30).expect("feb clamp path");
        let plan = build_fiscal_periods("2025Q1..Q1", feb_edge, None).expect("fiscal quarter");
        assert_eq!(plan.periods.len(), 1);
    }

    #[test]
    fn build_fiscal_periods_us_federal_and_monthly() {
        let cfg = FiscalConfig::us_federal();
        let qs = build_fiscal_periods("2025Q1..Q2", cfg, None).expect("FY quarters");
        assert_eq!(qs.periods.len(), 2);

        let jp = FiscalConfig::japan();
        let ms = build_fiscal_periods("2025M01..M02", jp, None).expect("FY months");
        assert_eq!(ms.periods.len(), 2);
    }

    #[test]
    fn fiscal_daily_and_weekly_bounds_stay_inside_fiscal_year() {
        let cfg = FiscalConfig::us_federal();
        let day = build_fiscal_periods("2025D001..D001", cfg, None).expect("fiscal day");
        assert_eq!(day.periods[0].start, d(2024, Month::October, 1));
        assert_eq!(day.periods[0].end, d(2024, Month::October, 2));

        let week = build_fiscal_periods("2020W53..W53", cfg, None).expect("fiscal week 53");
        assert_eq!(week.periods[0].start, d(2020, Month::September, 29));
        assert_eq!(week.periods[0].end, d(2020, Month::October, 1));
    }

    #[test]
    fn period_plan_iter_and_serde_roundtrip() {
        let plan = build_periods("2025Q1..Q2", None).expect("plan");
        let count = plan.iter().count();
        assert_eq!(count, 2);
        let json = serde_json::to_string(&plan).expect("serialize plan");
        let back: PeriodPlan = serde_json::from_str(&json).expect("deserialize plan");
        assert_eq!(back.periods.len(), plan.periods.len());
    }

    #[test]
    fn daily_next_rolls_year_on_last_ordinal() {
        let last = PeriodId::day(2023, 365);
        let next = last.next().expect("next day");
        assert_eq!(next, PeriodId::day(2024, 1));
    }

    #[test]
    fn quarterly_semi_and_annual_stepping() {
        assert_eq!(
            PeriodId::quarter(2025, 4).next().expect("nq"),
            PeriodId::quarter(2026, 1)
        );
        assert_eq!(
            PeriodId::half(2025, 2).prev().expect("ph"),
            PeriodId::half(2025, 1)
        );
        assert_eq!(
            PeriodId::annual(2025).next().expect("na"),
            PeriodId::annual(2026)
        );
    }

    #[test]
    fn parse_id_rejects_bad_ranges() {
        assert!(PeriodId::from_str("2025Q5").is_err());
        assert!(PeriodId::from_str("2025M13").is_err());
        assert!(PeriodId::from_str("2025W99").is_err());
        assert!(PeriodId::from_str("2025D500").is_err());
    }
}
