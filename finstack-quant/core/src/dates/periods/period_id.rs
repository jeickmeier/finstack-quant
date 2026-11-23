//! Period granularity ([`PeriodKind`]) and canonical period identifiers ([`PeriodId`]).

use super::calendar::PeriodCalendar;
use super::parse::{IdContext, invalid_period, parse_id};
use super::plan::FiscalConfig;
use crate::dates::Date;
use crate::dates::date_extensions::DateExt;
use core::fmt;
use core::str::FromStr;

/// Period frequency type.
///
/// Defines the frequency of periodic schedules (cashflow rolls, return-series
/// resampling, statement reporting). Two different yearly counts exist:
/// [`PeriodKind::annualization_factor`] is the conventional statistical
/// factor for explicitly selected observations (daily trading 252), and
/// [`PeriodKind::slots_per_year`] is the calendar partition count, defined
/// only for the month-block kinds (record CORE-CONVENTIONS-014 #9).
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

    /// Conventional statistical annualization factor for observations
    /// explicitly selected at this frequency.
    ///
    /// Daily trading 252, weekly 52, monthly 12, quarterly 4, semi-annual 2,
    /// annual 1. It scales return statistics only: it is not an observed
    /// count, a calendar check, an LTM length, a payment frequency or a
    /// conversion of dated time (use [`PeriodKind::slots_per_year`] for
    /// calendar partitions).
    pub const fn annualization_factor(self) -> u16 {
        match self {
            PeriodKind::Daily => 252,
            PeriodKind::Weekly => 52,
            PeriodKind::Monthly => 12,
            PeriodKind::Quarterly => 4,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
        }
    }

    /// Number of periods of this kind in every year, Gregorian or fiscal.
    ///
    /// Monthly 12, quarterly 4, semi-annual 2, annual 1. `None` for daily
    /// and weekly periods, whose count depends on the year and calendar
    /// (365 or 366 days; 52 or 53 weeks).
    pub const fn slots_per_year(self) -> Option<u16> {
        match self {
            PeriodKind::Daily | PeriodKind::Weekly => None,
            PeriodKind::Monthly => Some(12),
            PeriodKind::Quarterly => Some(4),
            PeriodKind::SemiAnnual => Some(2),
            PeriodKind::Annual => Some(1),
        }
    }

    /// Highest index `self` can take in any year (Gregorian or fiscal).
    pub(super) fn capacity(self) -> u16 {
        match self {
            PeriodKind::Daily => 366,
            PeriodKind::Weekly => 53,
            PeriodKind::Monthly => 12,
            PeriodKind::Quarterly => 4,
            PeriodKind::SemiAnnual => 2,
            PeriodKind::Annual => 1,
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
    pub(super) year: i32,
    /// One-based index within the year; see [`PeriodId::index`].
    pub(super) index: u16,
    /// Kind of the period.
    pub(super) kind: PeriodKind,
    /// Whether this identifier uses fiscal (`FY...`) rather than Gregorian/ISO semantics.
    pub(super) fiscal: bool,
}

impl PeriodId {
    /// Identify the calendar or fiscal period containing a date.
    ///
    /// Gregorian weeks use ISO week-years. Fiscal weeks are seven-day blocks
    /// from the configured year start, with a partial final week. Month-based
    /// fiscal periods use the same clamped boundaries as [`build_fiscal_periods`](crate::dates::build_fiscal_periods).
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
                    while index < kind.capacity() && date >= calendar.bounds(year, kind, index)?.1 {
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
    /// [`build_fiscal_periods`](crate::dates::build_fiscal_periods).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a fiscal identifier, or when the period
    /// or its exclusive end is outside the supported date range.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{create_date, PeriodId};
    ///     ///
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
    /// let fiscal = "FY2025M11".super::parse::<PeriodId>()?;
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
