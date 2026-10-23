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
/// Parses only the canonical snake_case wire values via [`std::str::FromStr`]:
/// `daily`, `weekly`, `monthly`, `quarterly`, `semi_annual`, and `annual`.
/// Aliases, whitespace padding and case variants are rejected.
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
        crate::wire::serde_parse(s).map_err(|_| {
            crate::Error::Validation(format!(
                "unknown period kind '{s}'; expected one of daily, weekly, monthly, quarterly, semi_annual, annual"
            ))
        })
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

    /// Highest index `self` can take in any year (Gregorian or fiscal).
    fn capacity(self) -> u16 {
        match self {
            PeriodKind::Daily => 366,
            PeriodKind::Weekly => 53,
            _ => self.periods_per_year(),
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
    /// Build an annual identifier.
    pub fn annual(year: i32) -> Self {
        Self {
            year,
            index: 1,
            kind: PeriodKind::Annual,
            fiscal: false,
        }
    }

    fn try_new(year: i32, index: u16, kind: PeriodKind) -> crate::Result<Self> {
        let max = PeriodCalendar::Gregorian.max_index(year, kind)?;
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

    /// Move by a whole number of periods of the same kind and calendar.
    ///
    /// Gregorian daily and weekly identifiers move by 1 or 7 calendar days
    /// (ISO week-years for weekly). Every other kind, fiscal weeks included,
    /// has a fixed number of periods per year (fiscal years always hold 53
    /// weekly blocks), so the year rolls over at that count. Fiscal daily
    /// identifiers can only move within days `1..=365`, the part of a fiscal
    /// year that does not depend on its [`FiscalConfig`].
    ///
    /// # Arguments
    ///
    /// * `periods` - Signed number of periods to move; negative moves back
    ///   and `0` returns the identifier unchanged after validation.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the identifier's index is outside its
    /// year's range, the result's year or date is outside the supported range,
    /// or a fiscal daily result would leave days `1..=365` (step with
    /// [`Self::next_fiscal`] / [`Self::prev_fiscal`] instead).
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::PeriodId;
    ///
    /// let q3 = PeriodId::quarter(2025, 3)?;
    /// assert_eq!(q3.shift(-6)?, PeriodId::quarter(2024, 1)?);
    /// let fiscal = "FY2025M11".parse::<PeriodId>()?;
    /// assert_eq!(fiscal.shift(2)?.to_string(), "FY2026M01");
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn shift(self, periods: i32) -> crate::Result<Self> {
        let out_of_range = || {
            invalid_period(
                &self.to_string(),
                &format!("shifting by {periods} periods leaves the supported range"),
            )
        };
        let max = if self.fiscal {
            self.kind.capacity()
        } else {
            PeriodCalendar::Gregorian.max_index(self.year, self.kind)?
        };
        if !(1..=max).contains(&self.index) {
            return Err(invalid_period(
                &self.to_string(),
                "period index is outside this year's range",
            ));
        }
        match (self.kind, self.fiscal) {
            (PeriodKind::Daily, true) => {
                if periods == 0 {
                    return Ok(self);
                }
                let index = i32::from(self.index)
                    .checked_add(periods)
                    .ok_or_else(out_of_range)?;
                if !(1..=365).contains(&index) {
                    return Err(invalid_period(
                        &self.to_string(),
                        &format!(
                            "shifting by {periods} days crosses a fiscal-year boundary; \
                             use next_fiscal/prev_fiscal with an explicit FiscalConfig"
                        ),
                    ));
                }
                Ok(Self {
                    index: index as u16,
                    ..self
                })
            }
            (PeriodKind::Daily | PeriodKind::Weekly, false) => {
                let (start, days) = if self.kind == PeriodKind::Daily {
                    (Date::from_ordinal_date(self.year, self.index), 1)
                } else {
                    (
                        Date::from_iso_week_date(
                            self.year,
                            self.index as u8,
                            time::Weekday::Monday,
                        ),
                        7,
                    )
                };
                let date = start
                    .ok()
                    .and_then(|start| {
                        start.checked_add(time::Duration::days(i64::from(periods) * days))
                    })
                    .ok_or_else(out_of_range)?;
                Self::from_date(date, self.kind, None)
            }
            _ => {
                let n = i64::from(max);
                let ordinal =
                    i64::from(self.year) * n + i64::from(self.index) - 1 + i64::from(periods);
                Ok(Self {
                    year: i32::try_from(ordinal.div_euclid(n)).map_err(|_| out_of_range())?,
                    index: (ordinal.rem_euclid(n) + 1) as u16,
                    ..self
                })
            }
        }
    }

    /// Step forward using the capacity of the supplied fiscal calendar.
    ///
    /// Unlike [`Self::shift`], this can step fiscal daily identifiers across a
    /// fiscal-year end, whose length (365 or 366 days) depends on `config`.
    /// The result is always marked fiscal.
    ///
    /// # Errors
    ///
    /// Returns an error when `config` has an invalid fiscal start date or the
    /// next fiscal-year boundary cannot be represented by `time::Date`.
    ///
    /// # Arguments
    ///
    /// * `config` - Fiscal-year start that sizes the fiscal year's week and
    ///   day capacity (a fiscal year holds 365 or 366 days and 53 weeks).
    pub fn next_fiscal(self, config: FiscalConfig) -> crate::Result<Self> {
        let mut next = step(self, PeriodCalendar::Fiscal(config), true)?;
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
    /// * `config` - Fiscal-year start that sizes the preceding fiscal year's
    ///   week and day capacity.
    pub fn prev_fiscal(self, config: FiscalConfig) -> crate::Result<Self> {
        let mut prev = step(self, PeriodCalendar::Fiscal(config), false)?;
        prev.fiscal = true;
        Ok(prev)
    }
}

/// Configuration for fiscal year periods.
///
/// Always holds a start month in `1..=12` and a start day in `1..=31`;
/// deserialization applies the same validation as [`FiscalConfig::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "FiscalConfigFields", into = "FiscalConfigFields")]
pub struct FiscalConfig {
    /// The month when the fiscal year starts.
    start_month: Month,
    /// The day of the month when the fiscal year starts (1-31).
    start_day: u8,
}

/// Wire shape of [`FiscalConfig`]: plain month and day numbers.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FiscalConfigFields {
    start_month: u8,
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

/// The calendar year (January 1), the fiscal year used when none is given.
impl Default for FiscalConfig {
    fn default() -> Self {
        Self::calendar_year()
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

    /// Resolve an optionally specified fiscal-year start.
    ///
    /// Hosts expose the start as two optional arguments; this is the one place
    /// that decides what an omitted half means.
    ///
    /// # Arguments
    ///
    /// * `start_month` - Fiscal-year start month (1-12). When only
    ///   `start_day` is supplied, the month defaults to January.
    /// * `start_day` - Fiscal-year start day of month (1-31). When only
    ///   `start_month` is supplied, the day defaults to the 1st.
    ///
    /// # Returns
    ///
    /// `None` when both parts are omitted, so the caller's own default applies
    /// (for example [`FiscalConfig::default`], the calendar year). Otherwise
    /// the validated configuration: a month of 10 alone means October 1, and a
    /// day of 6 alone means January 6.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` under the same range rules as
    /// [`FiscalConfig::new`].
    pub fn from_parts(
        start_month: Option<u8>,
        start_day: Option<u8>,
    ) -> crate::Result<Option<Self>> {
        if start_month.is_none() && start_day.is_none() {
            return Ok(None);
        }
        Self::new(start_month.unwrap_or(1), start_day.unwrap_or(1)).map(Some)
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

    /// Standard calendar year (January 1).
    pub fn calendar_year() -> Self {
        Self {
            start_month: Month::January,
            start_day: 1,
        }
    }

    /// US Federal fiscal year (October 1).
    pub fn us_federal() -> Self {
        Self {
            start_month: Month::October,
            start_day: 1,
        }
    }

    /// UK fiscal year (April 6).
    pub fn uk() -> Self {
        Self {
            start_month: Month::April,
            start_day: 6,
        }
    }

    /// Japanese fiscal year (April 1).
    pub fn japan() -> Self {
        Self {
            start_month: Month::April,
            start_day: 1,
        }
    }

    /// Australian fiscal year (July 1).
    pub fn australia() -> Self {
        Self {
            start_month: Month::July,
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
#[serde(deny_unknown_fields)]
pub struct PeriodPlan {
    /// Ordered periods produced by the parser.
    pub periods: Vec<Period>,
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
        let self_bounds = PeriodCalendar::Gregorian.bounds(self.year, self.kind, self.index);
        let other_bounds = PeriodCalendar::Gregorian.bounds(other.year, other.kind, other.index);

        // Years outside the supported date range still need a total ordering.
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
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_id(s, PeriodCalendar::Gregorian)
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
mod parse;
#[cfg(test)]
mod tests;
use calendar::*;
use parse::*;
