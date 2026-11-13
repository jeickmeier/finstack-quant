//! Tenor parsing and calendar-aware year fraction computation.
//!
//! This module provides market-standard tenor parsing that respects business day
//! conventions and holiday calendars. Unlike simple approximations (1M = 30 days),
//! these functions compute actual year fractions using proper date arithmetic.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{Tenor, TenorUnit};
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! // Parse a tenor string
//! let tenor = Tenor::parse("3M")?;
//! assert_eq!(tenor.count(), 3);
//! assert_eq!(tenor.unit(), TenorUnit::Months);
//!
//! // Convert to years with default settings (simple approximation)
//! let years = tenor.to_years_approx();
//! assert!((years - 0.25).abs() < 1e-6);
//! # Ok(())
//! # }
//! ```
//!
//! # Calendar-Aware Computation
//!
//! For accurate day counting that respects holidays and business day conventions:
//!
//! ```
//! use finstack_quant_core::dates::{
//!     calendar_by_id_strict, BusinessDayConvention, DayCount, DayCountContext, Tenor,
//! };
//! use time::macros::date;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let target2 = calendar_by_id_strict("target2")?;
//! let as_of = date!(2025 - 01 - 31);
//! let tenor = Tenor::parse("1M")?;
//!
//! // Calendar-aware: 1M from Jan 31 -> Feb 28 (end of month)
//! let end_date = tenor.add_to_date(
//!     as_of,
//!     Some(target2),
//!     BusinessDayConvention::ModifiedFollowing,
//! )?;
//!
//! let years = tenor.to_years_with_context(
//!     as_of,
//!     Some(target2),
//!     BusinessDayConvention::ModifiedFollowing,
//!     DayCount::ActAct,
//!     DayCountContext::default(),
//! )?;
//! # let _ = (end_date, years);
//! # Ok(())
//! # }
//! ```

use crate::InputError;
use crate::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, HolidayCalendar, adjust,
};

// Representation limits (CORE-CONVENTIONS-007 #2), not financial maturity
// caps: day counts fit `u32`, month counts fit the signed month-offset API.
const MAX_TENOR_DAYS: u32 = u32::MAX;
const MAX_TENOR_WEEKS: u32 = u32::MAX / 7;
const MAX_TENOR_MONTHS: u32 = i32::MAX as u32;
const MAX_TENOR_YEARS: u32 = MAX_TENOR_MONTHS / 12;

/// Unit of a tenor period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TenorUnit {
    /// Days (D)
    Days,
    /// Weeks (W)
    Weeks,
    /// Months (M)
    Months,
    /// Years (Y)
    Years,
}

impl TenorUnit {
    /// Single-character unit code: `'D'`, `'W'`, `'M'` or `'Y'`.
    ///
    /// The inverse of `TenorUnit`'s `FromStr` and the unit suffix of the
    /// tenor's `Display` form (`"3M"`).
    #[must_use]
    pub const fn designator(self) -> char {
        match self {
            Self::Days => 'D',
            Self::Weeks => 'W',
            Self::Months => 'M',
            Self::Years => 'Y',
        }
    }
}

impl std::str::FromStr for TenorUnit {
    type Err = crate::Error;

    /// Parses one unit character, `D`, `W`, `M` or `Y`, case-insensitively.
    ///
    /// An unknown character is `InputError::InvalidTenor`; anything other
    /// than exactly one character is `Error::Validation`.
    fn from_str(s: &str) -> crate::Result<Self> {
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => match c.to_ascii_uppercase() {
                'D' => Ok(Self::Days),
                'W' => Ok(Self::Weeks),
                'M' => Ok(Self::Months),
                'Y' => Ok(Self::Years),
                _ => Err(InputError::InvalidTenor {
                    tenor: c.to_string(),
                    reason: "unknown unit; expected D, W, M, or Y".to_string(),
                }
                .into()),
            },
            _ => Err(crate::Error::Validation(format!(
                "expected a single unit character D, W, M or Y, got {s:?}"
            ))),
        }
    }
}

/// A parsed tenor representing a time period.
///
/// Tenors are commonly used in financial markets to specify maturities,
/// payment frequencies, and rate fixing periods.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Tenor, TenorUnit};
/// # fn main() -> finstack_quant_core::Result<()> {
///
/// let tenor = Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture");
/// assert_eq!(tenor.count(), 3);
/// assert_eq!(tenor.unit(), TenorUnit::Months);
///
/// // Parse from string
/// let parsed = Tenor::parse("6M")?;
/// assert_eq!(parsed.count(), 6);
/// assert_eq!(parsed.unit(), TenorUnit::Months);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "TenorDe")]
pub struct Tenor {
    /// Number of units.
    count: u32,
    /// Unit type (days, weeks, months, years).
    unit: TenorUnit,
}

/// Deserialization mirror of [`Tenor`] used to validate inbound values.
///
/// A zero-count tenor is a no-op period that makes schedule generation
/// loop forever, so inbound payloads (e.g. JSON) with `"count": 0` must
/// fail loudly instead of constructing a degenerate `Tenor`.
#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct TenorDe {
    /// Number of `unit` periods in the tenor. Must be at least 1; `0` is
    /// rejected because a zero-length period makes schedule generation loop.
    count: u32,
    /// Calendar unit the count is expressed in, such as days, weeks, months,
    /// or years.
    unit: TenorUnit,
}

impl TryFrom<TenorDe> for Tenor {
    type Error = crate::Error;

    fn try_from(raw: TenorDe) -> Result<Self, Self::Error> {
        Self::new(raw.count, raw.unit)
    }
}

impl Tenor {
    /// Number of units in this tenor.
    pub const fn count(self) -> u32 {
        self.count
    }

    /// Unit represented by this tenor.
    pub const fn unit(self) -> TenorUnit {
        self.unit
    }

    /// Create a validated tenor.
    ///
    /// This is the canonical constructor for external input. It rejects zero
    /// counts and counts beyond the representation limits: D `1..=u32::MAX`,
    /// W `1..=u32::MAX/7` (days fit `u32`), M `1..=i32::MAX` and Y
    /// `1..=i32::MAX/12` (months fit the signed month offset). These are not
    /// maturity caps; applying a tenor must still reach a valid date.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` when `count` is zero or exceeds the
    /// representation limit for `unit`.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of tenor units in the resulting interval.
    /// * `unit` - Unit enum selecting the interpretation of the numeric magnitude
    pub fn new(count: u32, unit: TenorUnit) -> crate::Result<Self> {
        let max_count = match unit {
            TenorUnit::Days => MAX_TENOR_DAYS,
            TenorUnit::Weeks => MAX_TENOR_WEEKS,
            TenorUnit::Months => MAX_TENOR_MONTHS,
            TenorUnit::Years => MAX_TENOR_YEARS,
        };
        if count == 0 || count > max_count {
            return Err(InputError::InvalidTenor {
                tenor: format!("{count}{}", unit.designator()),
                reason: if count == 0 {
                    "count must be positive".to_string()
                } else {
                    format!(
                        "count exceeds the representation limit of {max_count} for unit {}",
                        unit.designator()
                    )
                },
            }
            .into());
        }
        Ok(Self { count, unit })
    }

    /// Get the number of months if the tenor is month-based or year-based.
    ///
    /// Returns `Some(months)` for Month and Year units (converting years to months).
    /// Returns `None` for Day and Week units.
    pub const fn months(&self) -> Option<u32> {
        match self.unit {
            TenorUnit::Months => Some(self.count),
            TenorUnit::Years => Some(self.count * 12),
            _ => None,
        }
    }

    /// Get the number of days if the tenor is day-based or week-based.
    ///
    /// Returns `Some(days)` for Day and Week units (converting weeks to days).
    /// Returns `None` for Month and Year units.
    pub const fn days(&self) -> Option<u32> {
        match self.unit {
            TenorUnit::Days => Some(self.count),
            TenorUnit::Weeks => Some(self.count * 7),
            _ => None,
        }
    }

    /// Create a Tenor from a year fraction using a day count convention.
    ///
    /// If the year fraction corresponds to an integer number of months (within a small epsilon),
    /// it returns a Month-based tenor. Otherwise, it converts to days using the provided
    /// day count convention.
    ///
    /// # Arguments
    /// * `years` - The time period in years
    /// * `day_count` - The day count convention to use for day conversion
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` when `years` is non-positive,
    /// non-finite, or converts to a tenor outside the supported range.
    pub fn from_years(years: f64, day_count: DayCount) -> crate::Result<Self> {
        if years <= 0.0 || !years.is_finite() {
            return Err(InputError::InvalidTenor {
                tenor: format!("{years}Y"),
                reason: "year fraction must be positive and finite".to_string(),
            }
            .into());
        }
        let months = years * 12.0;
        let rounded_months = months.round();

        if (months - rounded_months).abs() < 1e-4 {
            // It's effectively an integer number of months
            let m = rounded_months as u32;
            if m > 0 && m.is_multiple_of(12) {
                Self::new(m / 12, TenorUnit::Years)
            } else {
                Self::new(m, TenorUnit::Months)
            }
        } else {
            // Convert to days
            let days = match day_count {
                DayCount::Thirty360
                | DayCount::ThirtyE360
                | DayCount::ThirtyE360Isda
                | DayCount::Thirty360It
                | DayCount::Act360 => (years * 360.0).round(),
                DayCount::Act365F | DayCount::Nl365 | DayCount::Act365L => (years * 365.0).round(),
                DayCount::Bus252 => (years * 252.0).round(),
                _ => (years * 365.25).round(),
            };
            Self::new(days as u32, TenorUnit::Days)
        }
    }

    /// Parse a tenor string like "1D", "1W", "3M", "5Y".
    ///
    /// # Format
    ///
    /// `<count><unit>` where:
    /// - `count` is a positive integer
    /// - `unit` is one of: D (days), W (weeks), M (months), Y (years)
    ///
    /// Parsing is case-insensitive and trims whitespace.
    ///
    /// # Arguments
    /// * `s` - Tenor string to parse
    ///
    /// # Returns
    /// Parsed `Tenor` or an error if the format is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Tenor, TenorUnit};
    ///
    /// assert!(Tenor::parse("1D").is_ok());
    /// assert!(Tenor::parse("3m").is_ok()); // Case-insensitive
    /// assert!(Tenor::parse("10Y").is_ok());
    /// assert_eq!(Tenor::parse(" 1d ")?, Tenor::daily());
    /// assert!(Tenor::parse("").is_err());
    /// assert!(Tenor::parse("XY").is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// Parsing ignores surrounding whitespace and ASCII case. Only a positive
    /// integer followed by `D`, `W`, `M` or `Y` is accepted.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` when the string is empty, lacks a
    /// positive integer count, has more than one unit character, uses an
    /// unsupported unit, or exceeds the supported tenor range.
    pub fn parse(s: &str) -> crate::Result<Self> {
        let s = s.trim();

        if s.is_empty() {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: "empty tenor string".to_string(),
            }
            .into());
        }

        // Find where the unit character starts (last character)
        let s_upper = s.to_ascii_uppercase();

        // Find position of first alphabetic character
        let unit_pos =
            s_upper
                .find(|c: char| c.is_alphabetic())
                .ok_or_else(|| InputError::InvalidTenor {
                    tenor: s.to_string(),
                    reason: "no unit found; expected D, W, M, or Y suffix".to_string(),
                })?;

        let (count_str, unit_str) = s_upper.split_at(unit_pos);

        if count_str.is_empty() {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: "no count found; expected format like '3M' or '1Y'".to_string(),
            }
            .into());
        }

        if !count_str.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: "count must contain only ASCII digits".to_string(),
            }
            .into());
        }

        let count: u32 = count_str.parse().map_err(|_| InputError::InvalidTenor {
            tenor: s.to_string(),
            reason: format!("invalid count '{}'; expected a positive integer", count_str),
        })?;

        if count == 0 {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: "count must be positive".to_string(),
            }
            .into());
        }

        let unit = unit_str
            .parse::<TenorUnit>()
            .map_err(|err| InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: err.to_string(),
            })?;
        Self::new(count, unit)
    }

    /// Convert tenor to a simple year fraction approximation.
    ///
    /// This uses fixed approximations:
    /// - 1D = 1/365 years
    /// - 1W = 7/365 years
    /// - 1M = 1/12 years
    /// - 1Y = 1 year
    ///
    /// For more accurate calculations, use [`to_years_with_context`](Self::to_years_with_context).
    ///
    /// # Returns
    /// Year fraction as f64.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::Tenor;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let tenor = Tenor::parse("6M")?;
    /// assert!((tenor.to_years_approx() - 0.5).abs() < 1e-6);
    ///
    /// let tenor = Tenor::parse("1Y")?;
    /// assert!((tenor.to_years_approx() - 1.0).abs() < 1e-6);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn to_years_approx(&self) -> f64 {
        let count = f64::from(self.count);
        match self.unit {
            TenorUnit::Days => count / 365.0,
            TenorUnit::Weeks => count * 7.0 / 365.0,
            TenorUnit::Months => count / 12.0,
            TenorUnit::Years => count,
        }
    }

    /// Nominal payments per year of a monthly or yearly payment tenor.
    ///
    /// `nM` gives `12/n` and `nY` gives `1/n`, including fractional ratios
    /// such as `5M` → 2.4. This is a nominal ratio, not the number of
    /// payments observed in a calendar year.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` for day and week tenors: no 365,
    /// 252 or 52 observation convention is inferred; an explicit product
    /// convention or actual schedule owns those choices.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::Tenor;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// assert_eq!(Tenor::parse("3M")?.payments_per_year()?, 4.0);
    /// assert_eq!(Tenor::parse("2Y")?.payments_per_year()?, 0.5);
    /// assert!(Tenor::parse("1W")?.payments_per_year().is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn payments_per_year(&self) -> crate::Result<f64> {
        let count = f64::from(self.count);
        match self.unit {
            TenorUnit::Months => Ok(12.0 / count),
            TenorUnit::Years => Ok(1.0 / count),
            TenorUnit::Days | TenorUnit::Weeks => Err(InputError::InvalidTenor {
                tenor: self.to_string(),
                reason: "payments per year is defined only for month and year tenors".into(),
            }
            .into()),
        }
    }

    /// Convert tenor to an approximate number of whole calendar days.
    ///
    /// The declared approximation matching [`to_years_approx`](Self::to_years_approx),
    /// computed exactly in integers: `nD` → `n`, `nW` → `7n`, `nM` →
    /// `floor((365n + 6) / 12)` (365/12 days a month, half-day ties rounded
    /// up, so `6M` → 183) and `nY` → `365n`. It does not recover actual dated
    /// accrual or the endpoint of calendar addition.
    ///
    /// # Returns
    /// Approximate number of days as i64.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::Tenor;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let tenor = Tenor::parse("1D")?;
    /// assert_eq!(tenor.to_days_approx(), 1);
    ///
    /// let tenor = Tenor::parse("1W")?;
    /// assert_eq!(tenor.to_days_approx(), 7);
    ///
    /// let tenor = Tenor::parse("1M")?;
    /// assert_eq!(tenor.to_days_approx(), 30); // 365/12 ≈ 30.42, rounded
    ///
    /// let tenor = Tenor::parse("6M")?;
    /// assert_eq!(tenor.to_days_approx(), 183); // 182.5 rounds up
    ///
    /// let tenor = Tenor::parse("1Y")?;
    /// assert_eq!(tenor.to_days_approx(), 365);
    ///
    /// // Multi-year consistency: 5Y = 5 * 365 days
    /// let tenor = Tenor::parse("5Y")?;
    /// assert_eq!(tenor.to_days_approx(), 1825);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn to_days_approx(&self) -> i64 {
        let count = i64::from(self.count);
        match self.unit {
            TenorUnit::Days => count,
            TenorUnit::Weeks => 7 * count,
            TenorUnit::Months => (365 * count + 6) / 12,
            TenorUnit::Years => 365 * count,
        }
    }

    /// Add the tenor to a date, then roll the raw target once with the
    /// selected business-day convention (CORE-CONVENTIONS-007 #5).
    ///
    /// # Arguments
    /// * `date` - Starting date
    /// * `calendar` - Holiday calendar used to roll the raw target; may be
    ///   omitted only with `BusinessDayConvention::Unadjusted`
    /// * `business_day_convention` - Business-day convention applied to the raw target
    ///
    /// # Returns
    /// The resulting date after adding the tenor period.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Tenor, Date, BusinessDayConvention};
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let start = date!(2025 - 01 - 15);
    /// let tenor = Tenor::parse("1M")?;
    ///
    /// let end = tenor.add_to_date(start, None, BusinessDayConvention::Unadjusted)?;
    /// assert_eq!(end, date!(2025 - 02 - 15));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` when a month or year count cannot be
    /// represented for date arithmetic, or a validation error if the shifted
    /// date exceeds the supported calendar range or `calendar` is omitted with
    /// a convention other than `Unadjusted`. With a calendar it also
    /// propagates the calendar's business-day adjustment error.
    pub fn add_to_date(
        &self,
        date: Date,
        calendar: Option<&dyn HolidayCalendar>,
        business_day_convention: BusinessDayConvention,
    ) -> crate::Result<Date> {
        use crate::dates::date_extensions::DateExt;

        let raw_date = match self.unit {
            TenorUnit::Days => date.add_days(i64::from(self.count))?,
            TenorUnit::Weeks => date.add_days(7 * i64::from(self.count))?,
            TenorUnit::Months | TenorUnit::Years => {
                let months = self
                    .months()
                    .and_then(|months| i32::try_from(months).ok())
                    .ok_or_else(|| InputError::InvalidTenor {
                        tenor: self.to_string(),
                        reason: "month count does not fit the signed month offset".into(),
                    })?;
                date.add_months(months)?
            }
        };

        match (calendar, business_day_convention) {
            (_, BusinessDayConvention::Unadjusted) => Ok(raw_date),
            (Some(cal), convention) => adjust(raw_date, convention, cal),
            (None, convention) => Err(crate::Error::Validation(format!(
                "business-day convention {convention} requires a calendar; only \
                 `unadjusted` may omit it"
            ))),
        }
    }

    /// Convert tenor to year fraction using calendar-aware date computation.
    ///
    /// Adds the tenor to `as_of`, rolls the end with `calendar` and
    /// `business_day_convention` when a calendar is given, then measures
    /// `as_of` to that end with `day_count` under the caller's `context`. The
    /// endpoint-adjustment calendar and the accrual context keep separate
    /// roles: no coupon frequency or period is invented from the tenor.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Anchor calendar date from which the tenor is projected forward
    /// * `calendar` - Optional holiday calendar used when applying `business_day_convention` to the end date
    /// * `business_day_convention` - Business-day convention applied when a calendar is provided
    /// * `day_count` - Day-count convention used to convert the date span into a year fraction
    /// * `context` - Caller-supplied day-count context (calendar, frequency,
    ///   coupon period, termination status) required by `day_count`; the empty
    ///   context suits context-free conventions
    ///
    /// # Returns
    /// Year fraction computed using the specified conventions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Tenor, Date, DayCount, DayCountContext, BusinessDayConvention};
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let as_of = date!(2025 - 01 - 15);
    /// let tenor = Tenor::parse("1Y")?;
    ///
    /// let years = tenor.to_years_with_context(
    ///     as_of,
    ///     None,
    ///     BusinessDayConvention::Unadjusted,
    ///     DayCount::ActAct,
    ///     DayCountContext::default(),
    /// )?;
    ///
    /// assert!((years - 1.0).abs() < 0.01);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Propagates failures from date addition, optional business-day
    /// adjustment, or the selected day-count calculation (including missing
    /// required context). Returns `Error::Validation` when the adjusted end
    /// precedes `as_of`; an end equal to `as_of` gives zero only after
    /// `context` is valid for `day_count`.
    pub fn to_years_with_context(
        &self,
        as_of: Date,
        calendar: Option<&dyn HolidayCalendar>,
        business_day_convention: BusinessDayConvention,
        day_count: DayCount,
        context: DayCountContext<'_>,
    ) -> crate::Result<f64> {
        let end_date = self.add_to_date(as_of, calendar, business_day_convention)?;
        if end_date < as_of {
            return Err(crate::Error::Validation(format!(
                "tenor {self} from {as_of} rolls to {end_date}, before its origin"
            )));
        }
        day_count.year_fraction(as_of, end_date, context)
    }

    /// Convenience constructor for Annual frequency (1 Year).
    #[inline]
    pub const fn annual() -> Self {
        Self {
            count: 1,
            unit: TenorUnit::Years,
        }
    }

    /// Convenience constructor for Semi-Annual frequency (6 Months).
    #[inline]
    pub const fn semi_annual() -> Self {
        Self {
            count: 6,
            unit: TenorUnit::Months,
        }
    }

    /// Convenience constructor for Quarterly frequency (3 Months).
    #[inline]
    pub const fn quarterly() -> Self {
        Self {
            count: 3,
            unit: TenorUnit::Months,
        }
    }

    /// Convenience constructor for Bi-Monthly frequency (2 Months).
    #[inline]
    pub const fn bimonthly() -> Self {
        Self {
            count: 2,
            unit: TenorUnit::Months,
        }
    }

    /// Convenience constructor for Monthly frequency (1 Month).
    #[inline]
    pub const fn monthly() -> Self {
        Self {
            count: 1,
            unit: TenorUnit::Months,
        }
    }

    /// Convenience constructor for Bi-Weekly frequency (2 Weeks).
    #[inline]
    pub const fn biweekly() -> Self {
        Self {
            count: 2,
            unit: TenorUnit::Weeks,
        }
    }

    /// Convenience constructor for Weekly frequency (1 Week).
    #[inline]
    pub const fn weekly() -> Self {
        Self {
            count: 1,
            unit: TenorUnit::Weeks,
        }
    }

    /// Convenience constructor for Daily frequency (1 Day).
    #[inline]
    pub const fn daily() -> Self {
        Self {
            count: 1,
            unit: TenorUnit::Days,
        }
    }

    /// Create a Tenor from payments per year.
    ///
    /// This maps only regular month-based schedules: `12` is monthly, `4` is
    /// quarterly, and `2` is semi-annual.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidTenor` when `payments` is zero or does not
    /// divide twelve months evenly.
    pub fn from_payments_per_year(payments: u32) -> crate::Result<Self> {
        if payments == 0 {
            return Err(InputError::InvalidTenor {
                tenor: format!("payments={}", payments),
                reason: "payments_per_year must be positive".to_string(),
            }
            .into());
        }

        if 12 % payments == 0 {
            let months = 12 / payments;
            Self::new(months, TenorUnit::Months)
        } else {
            Err(InputError::InvalidTenor {
                tenor: format!("payments={}", payments),
                reason: "payments_per_year for Tenor currently requires even division of 12 months"
                    .to_string(),
            }
            .into())
        }
    }
}

impl std::fmt::Display for Tenor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.count, self.unit.designator())
    }
}

impl std::str::FromStr for Tenor {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn unit_designator_round_trips_from_str() {
        for unit in [
            TenorUnit::Days,
            TenorUnit::Weeks,
            TenorUnit::Months,
            TenorUnit::Years,
        ] {
            assert_eq!(
                unit.designator()
                    .to_string()
                    .parse::<TenorUnit>()
                    .expect("unit"),
                unit
            );
        }
        assert_eq!(Tenor::parse("3M").expect("3M").unit().designator(), 'M');
    }

    #[test]
    fn test_parse_valid_tenors() {
        let cases = vec![
            ("1D", 1, TenorUnit::Days),
            ("7D", 7, TenorUnit::Days),
            ("1W", 1, TenorUnit::Weeks),
            ("2W", 2, TenorUnit::Weeks),
            ("1M", 1, TenorUnit::Months),
            ("3M", 3, TenorUnit::Months),
            ("6M", 6, TenorUnit::Months),
            ("12M", 12, TenorUnit::Months),
            ("1Y", 1, TenorUnit::Years),
            ("5Y", 5, TenorUnit::Years),
            ("10Y", 10, TenorUnit::Years),
            ("30Y", 30, TenorUnit::Years),
        ];

        for (input, expected_count, expected_unit) in cases {
            let tenor = Tenor::parse(input).expect(input);
            assert_eq!(tenor.count, expected_count, "count mismatch for {}", input);
            assert_eq!(tenor.unit, expected_unit, "unit mismatch for {}", input);
        }
    }

    #[test]
    fn test_parse_rejects_money_market_aliases() {
        for alias in ["ON", "on", "TN", "SN", "O/N", "t/n", "S/N", " ON "] {
            assert!(Tenor::parse(alias).is_err(), "{alias}");
        }
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!(
            Tenor::parse("3m").expect("3m"),
            Tenor::parse("3M").expect("3M")
        );
        assert_eq!(
            Tenor::parse("1y").expect("1y"),
            Tenor::parse("1Y").expect("1Y")
        );
    }

    #[test]
    fn test_parse_with_whitespace() {
        let tenor = Tenor::parse("  3M  ").expect("trimmed");
        assert_eq!(tenor.count, 3);
        assert_eq!(tenor.unit, TenorUnit::Months);
    }

    #[test]
    fn test_parse_invalid_tenors() {
        assert!(Tenor::parse("").is_err());
        assert!(Tenor::parse("M").is_err()); // No count
        assert!(Tenor::parse("3").is_err()); // No unit
        assert!(Tenor::parse("3X").is_err()); // Invalid unit
        assert!(Tenor::parse("0M").is_err()); // Zero count
        assert!(Tenor::parse("-1M").is_err()); // Negative (parsed as invalid)
        assert!(Tenor::parse("3MM").is_err()); // Multiple unit chars
    }

    #[test]
    fn test_parse_rejects_counts_beyond_representation_limits() {
        // The old 200-year cap is gone: long tenors parse up to the
        // representation limits (CORE-CONVENTIONS-007 #2).
        assert!(Tenor::parse("201Y").is_ok());
        assert!(Tenor::parse("178956970Y").is_ok());
        assert!(Tenor::parse("178956971Y").is_err());
        assert!(Tenor::parse("2147483647M").is_ok());
        assert!(Tenor::parse("2147483648M").is_err());
        assert!(Tenor::parse("613566756W").is_ok());
        assert!(Tenor::parse("613566757W").is_err());
        assert!(Tenor::parse("4294967295D").is_ok());
        assert!(Tenor::parse("4294967296D").is_err());
    }

    #[test]
    fn test_to_years() {
        assert!((Tenor::parse("1D").expect("valid").to_years_approx() - 1.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("7D").expect("valid").to_years_approx() - 7.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("1W").expect("valid").to_years_approx() - 7.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("1M").expect("valid").to_years_approx() - 1.0 / 12.0).abs() < 1e-10);
        assert!((Tenor::parse("3M").expect("valid").to_years_approx() - 0.25).abs() < 1e-10);
        assert!((Tenor::parse("6M").expect("valid").to_years_approx() - 0.5).abs() < 1e-10);
        assert!((Tenor::parse("1Y").expect("valid").to_years_approx() - 1.0).abs() < 1e-10);
        assert!((Tenor::parse("5Y").expect("valid").to_years_approx() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_years_prefers_months_and_handles_invalid_values() {
        assert_eq!(
            Tenor::from_years(1.0, DayCount::Act365F).expect("valid"),
            Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::from_years(0.25, DayCount::Act365F).expect("valid"),
            Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::from_years(10.0 / 365.0, DayCount::Act365F).expect("valid"),
            Tenor::new(10, TenorUnit::Days).expect("valid tenor fixture")
        );
        assert!(Tenor::from_years(-1.0, DayCount::Act365F).is_err());
        assert!(Tenor::from_years(f64::NAN, DayCount::Act365F).is_err());
    }

    #[test]
    fn test_from_payments_per_year_validation() {
        assert_eq!(
            Tenor::from_payments_per_year(4).expect("quarterly"),
            Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::from_payments_per_year(12).expect("monthly"),
            Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert!(Tenor::from_payments_per_year(0).is_err());
        assert!(Tenor::from_payments_per_year(5).is_err());
    }

    #[test]
    fn test_to_days_approx() {
        // Days and weeks are exact
        assert_eq!(Tenor::parse("1D").expect("valid").to_days_approx(), 1);
        assert_eq!(Tenor::parse("7D").expect("valid").to_days_approx(), 7);
        assert_eq!(Tenor::parse("1W").expect("valid").to_days_approx(), 7);
        assert_eq!(Tenor::parse("2W").expect("valid").to_days_approx(), 14);

        // Months use 365/12 ≈ 30.4167, rounded
        // 1M: 30.4167 → 30
        // 3M: 91.25 → 91
        // 6M: 182.5 → 183 (rounds up)
        // 12M: 365.0 → 365
        assert_eq!(Tenor::parse("1M").expect("valid").to_days_approx(), 30);
        assert_eq!(Tenor::parse("3M").expect("valid").to_days_approx(), 91);
        assert_eq!(Tenor::parse("6M").expect("valid").to_days_approx(), 183);
        assert_eq!(Tenor::parse("12M").expect("valid").to_days_approx(), 365);

        // Years are exact
        assert_eq!(Tenor::parse("1Y").expect("valid").to_days_approx(), 365);
        assert_eq!(Tenor::parse("5Y").expect("valid").to_days_approx(), 1825);

        // Consistency: to_days_approx() / 365 ≈ to_years()
        for tenor_str in &["1M", "3M", "6M", "1Y", "5Y"] {
            let tenor = Tenor::parse(tenor_str).expect("valid");
            let days = tenor.to_days_approx() as f64;
            let years = tenor.to_years_approx();
            // Allow 1 day tolerance due to rounding
            assert!(
                (days / 365.0 - years).abs() < 0.003,
                "Consistency failed for {}: days={}, years={}",
                tenor_str,
                days,
                years
            );
        }
    }

    #[test]
    fn tenor_rejects_unknown_json_fields() {
        let err = serde_json::from_str::<Tenor>(r#"{"count":3,"unit":"months","extra":true}"#)
            .expect_err("unknown tenor fields must be rejected");
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn checked_constructor_rejects_zero_and_conversion_overflow() {
        assert!(Tenor::new(0, TenorUnit::Months).is_err());
        assert!(Tenor::new(u32::MAX, TenorUnit::Years).is_err());
        assert!(Tenor::new(u32::MAX, TenorUnit::Weeks).is_err());
        assert_eq!(
            Tenor::new(3, TenorUnit::Months).unwrap(),
            Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture")
        );
    }

    #[test]
    fn test_add_to_date_months() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid");
        let tenor = Tenor::parse("1M").expect("valid");

        let end = tenor
            .add_to_date(start, None, BusinessDayConvention::Unadjusted)
            .expect("add");
        assert_eq!(
            end,
            Date::from_calendar_date(2025, Month::February, 15).expect("valid")
        );
    }

    #[test]
    fn test_add_to_date_end_of_month() {
        // Jan 31 + 1M should go to Feb 28 (or 29 in leap year)
        let start = Date::from_calendar_date(2025, Month::January, 31).expect("valid");
        let tenor = Tenor::parse("1M").expect("valid");

        let end = tenor
            .add_to_date(start, None, BusinessDayConvention::Unadjusted)
            .expect("add");
        // 2025 is not a leap year, so Feb has 28 days
        assert_eq!(
            end,
            Date::from_calendar_date(2025, Month::February, 28).expect("valid")
        );
    }

    #[test]
    fn test_add_to_date_years() {
        let start = Date::from_calendar_date(2024, Month::February, 29).expect("valid"); // Leap day
        let tenor = Tenor::parse("1Y").expect("valid");

        let end = tenor
            .add_to_date(start, None, BusinessDayConvention::Unadjusted)
            .expect("add");
        // 2025 is not a leap year, so Feb 29 -> Feb 28
        assert_eq!(
            end,
            Date::from_calendar_date(2025, Month::February, 28).expect("valid")
        );
    }

    #[test]
    fn tenor_rejects_count_exceeding_supported_range() {
        let err = Tenor::new((i32::MAX as u32) + 1, TenorUnit::Months)
            .expect_err("oversized tenor rejected at construction");
        assert!(err.to_string().contains("representation limit"), "{err}");
        assert!(Tenor::new(i32::MAX as u32, TenorUnit::Months).is_ok());
        assert!(Tenor::new(u32::MAX, TenorUnit::Days).is_ok());
        assert!(Tenor::new(u32::MAX / 7 + 1, TenorUnit::Weeks).is_err());
        assert!(Tenor::new(i32::MAX as u32 / 12 + 1, TenorUnit::Years).is_err());
    }

    #[test]
    fn test_to_years_with_context_act_act() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
        let tenor = Tenor::parse("1Y").expect("valid");

        let years = tenor
            .to_years_with_context(
                as_of,
                None,
                BusinessDayConvention::Unadjusted,
                DayCount::ActAct,
                DayCountContext::default(),
            )
            .expect("year fraction");

        // 2025 is not a leap year, so 365 days / 365 = 1.0
        assert!((years - 1.0).abs() < 0.001);

        // ActActIsma with calendar adjustment when unadjusted end lands on a weekend
        // (2025-02-15 is a Saturday -> rolls to Monday 2025-02-17 under Following).
        let cal = crate::dates::calendar_by_id_strict("target2").expect("target2 calendar");
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid");
        // The caller supplies the ICMA reference period; none is invented
        // from the tenor.
        let reference = DayCountContext {
            frequency: Some(Tenor::monthly()),
            coupon_period: Some((
                start,
                Date::from_calendar_date(2025, Month::February, 15).expect("valid"),
            )),
            ..DayCountContext::default()
        };
        let isma_years = Tenor::monthly()
            .to_years_with_context(
                start,
                Some(cal),
                BusinessDayConvention::Following,
                DayCount::ActActIsma,
                reference,
            )
            .expect("ActActIsma with adjusted end date");
        assert!(isma_years > 1.0 / 12.0);
        assert!(
            Tenor::monthly()
                .to_years_with_context(
                    start,
                    Some(cal),
                    BusinessDayConvention::Following,
                    DayCount::ActActIsma,
                    DayCountContext::default(),
                )
                .is_err(),
            "missing ICMA context is an error, not an invented frequency"
        );
    }

    #[test]
    fn approximations_are_exact_integer_rules() {
        assert_eq!(Tenor::parse("6M").expect("valid").to_days_approx(), 183);
        assert_eq!(Tenor::parse("1M").expect("valid").to_days_approx(), 30);
        assert_eq!(Tenor::parse("2W").expect("valid").to_days_approx(), 14);
        assert_eq!(
            Tenor::new(i32::MAX as u32, TenorUnit::Months)
                .expect("max months")
                .to_days_approx(),
            (365 * i64::from(i32::MAX) + 6) / 12
        );
        assert_eq!(
            Tenor::parse("5M")
                .expect("valid")
                .payments_per_year()
                .expect("ratio"),
            2.4
        );
        assert!(
            Tenor::parse("1D")
                .expect("valid")
                .payments_per_year()
                .is_err()
        );
        assert!(
            Tenor::parse("1W")
                .expect("valid")
                .payments_per_year()
                .is_err()
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", Tenor::parse("3M").expect("valid tenor")),
            "3M"
        );
        assert_eq!(
            format!("{}", Tenor::parse("1Y").expect("valid tenor")),
            "1Y"
        );
        assert_eq!(
            format!("{}", Tenor::parse("7D").expect("valid tenor")),
            "7D"
        );
    }

    #[test]
    fn test_from_str() {
        let tenor: Tenor = "6M".parse().expect("valid");
        assert_eq!(tenor.count, 6);
        assert_eq!(tenor.unit, TenorUnit::Months);
    }

    #[test]
    fn test_convenience_constructors() {
        assert_eq!(
            Tenor::daily(),
            Tenor::new(1, TenorUnit::Days).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::weekly(),
            Tenor::new(1, TenorUnit::Weeks).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::biweekly(),
            Tenor::new(2, TenorUnit::Weeks).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::monthly(),
            Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::bimonthly(),
            Tenor::new(2, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::quarterly(),
            Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::semi_annual(),
            Tenor::new(6, TenorUnit::Months).expect("valid tenor fixture")
        );
        assert_eq!(
            Tenor::annual(),
            Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture")
        );
    }
}
