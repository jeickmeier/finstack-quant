//! Tenor parsing and calendar-aware year fraction computation.
//!
//! This module provides market-standard tenor parsing that respects business day
//! conventions and holiday calendars. Unlike simple approximations (1M = 30 days),
//! these functions compute actual year fractions using proper date arithmetic.
//!
//! # Examples
//!
//! ```rust
//! use finstack_core::dates::{Tenor, TenorUnit, Date, DayCount, DayCountContext};
//! use finstack_core::dates::{BusinessDayConvention, HolidayCalendar};
//! use time::Month;
//! # fn main() -> finstack_core::Result<()> {
//!
//! // Parse a tenor string
//! let tenor = Tenor::parse("3M")?;
//! assert_eq!(tenor.count, 3);
//! assert_eq!(tenor.unit, TenorUnit::Months);
//!
//! // Convert to years with default settings (simple approximation)
//! let years = tenor.to_years_simple();
//! assert!((years - 0.25).abs() < 1e-6);
//! # Ok(())
//! # }
//! ```
//!
//! # Calendar-Aware Computation
//!
//! For accurate day counting that respects holidays and business day conventions:
//!
//! ```ignore
//! use finstack_core::dates::{Tenor, DayCount, DayCountContext, BusinessDayConvention};
//! use finstack_core::dates::calendar::TARGET2;
//! use time::macros::date;
//! # fn main() -> finstack_core::Result<()> {
//!
//! let as_of = date!(2025 - 01 - 31);
//! let tenor = Tenor::parse("1M")?;
//!
//! // Calendar-aware: 1M from Jan 31 -> Feb 28 (end of month)
//! let end_date = tenor.add_to_date(
//!     as_of,
//!     Some(&TARGET2),
//!     BusinessDayConvention::ModifiedFollowing,
//! )?;
//!
//! let years = tenor.to_years_with_context(
//!     as_of,
//!     Some(&TARGET2),
//!     BusinessDayConvention::ModifiedFollowing,
//!     DayCount::ActAct,
//! )?;
//! # let _ = (end_date, years);
//! # Ok(())
//! # }
//! ```

use crate::dates::{
    adjust, BusinessDayConvention, Date, DayCount, DayCountContext, HolidayCalendar,
};
use crate::error::InputError;
use time::Duration;

const MAX_TENOR_YEARS: u32 = 200;
const MAX_TENOR_DAYS: u32 = MAX_TENOR_YEARS * 366;
const MAX_TENOR_WEEKS: u32 = MAX_TENOR_DAYS / 7;
const MAX_TENOR_MONTHS: u32 = MAX_TENOR_YEARS * 12;

/// Unit of a tenor period.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
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
    /// Parse a single character unit code.
    ///
    /// # Arguments
    /// * `c` - Unit character: 'D', 'W', 'M', or 'Y' (case-insensitive)
    ///
    /// # Returns
    /// The parsed `TenorUnit` or an error if the character is invalid.
    pub fn from_char(c: char) -> crate::Result<Self> {
        match c.to_ascii_uppercase() {
            'D' => Ok(Self::Days),
            'W' => Ok(Self::Weeks),
            'M' => Ok(Self::Months),
            'Y' => Ok(Self::Years),
            _ => Err(InputError::InvalidTenor {
                tenor: c.to_string(),
                reason: "unknown unit; expected D, W, M, or Y".to_string(),
            }
            .into()),
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
/// use finstack_core::dates::{Tenor, TenorUnit};
/// # fn main() -> finstack_core::Result<()> {
///
/// let tenor = Tenor::new(3, TenorUnit::Months);
/// assert_eq!(tenor.count, 3);
/// assert_eq!(tenor.unit, TenorUnit::Months);
///
/// // Parse from string
/// let parsed = Tenor::parse("6M")?;
/// assert_eq!(parsed.count, 6);
/// assert_eq!(parsed.unit, TenorUnit::Months);
/// # Ok(())
/// # }
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(try_from = "TenorDe")]
pub struct Tenor {
    /// Number of units.
    pub count: u32,
    /// Unit type (days, weeks, months, years).
    pub unit: TenorUnit,
}

/// Deserialization mirror of [`Tenor`] used to validate inbound values.
///
/// A zero-count tenor is a no-op period that makes schedule generation
/// loop forever, so inbound payloads (e.g. JSON) with `"count": 0` must
/// fail loudly instead of constructing a degenerate `Tenor`.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TenorDe {
    count: u32,
    unit: TenorUnit,
}

impl TryFrom<TenorDe> for Tenor {
    type Error = crate::Error;

    fn try_from(raw: TenorDe) -> Result<Self, Self::Error> {
        if raw.count == 0 {
            return Err(InputError::InvalidTenor {
                tenor: "0".to_string(),
                reason: "count must be positive".to_string(),
            }
            .into());
        }
        Ok(Self {
            count: raw.count,
            unit: raw.unit,
        })
    }
}

impl Tenor {
    /// Create a new tenor with the specified count and unit.
    ///
    /// # Arguments
    /// * `count` - Number of periods
    /// * `unit` - Period unit type
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Tenor, TenorUnit};
    ///
    /// let quarterly = Tenor::new(3, TenorUnit::Months);
    /// let annual = Tenor::new(1, TenorUnit::Years);
    /// ```
    #[inline]
    pub const fn new(count: u32, unit: TenorUnit) -> Self {
        Self { count, unit }
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
    pub fn from_years(years: f64, day_count: DayCount) -> Self {
        if years < 0.0 || !years.is_finite() {
            return Self::new(0, TenorUnit::Days);
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
                DayCount::Thirty360 | DayCount::ThirtyE360 | DayCount::ThirtyE360Isda => {
                    (years * 360.0).round()
                }
                DayCount::Act360 => (years * 360.0).round(),
                DayCount::Act365F => (years * 365.0).round(),
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
    /// use finstack_core::dates::{Tenor, TenorUnit};
    ///
    /// assert!(Tenor::parse("1D").is_ok());
    /// assert!(Tenor::parse("3m").is_ok()); // Case-insensitive
    /// assert!(Tenor::parse("10Y").is_ok());
    /// assert!(Tenor::parse("").is_err());
    /// assert!(Tenor::parse("XY").is_err());
    /// ```
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
        let s_upper = s.to_uppercase();

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

        // Unit should be exactly one character
        if unit_str.len() != 1 {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: format!(
                    "invalid unit '{}'; expected single character D, W, M, or Y",
                    unit_str
                ),
            }
            .into());
        }

        // Length was checked to be 1 above, so first char exists
        let Some(unit_char) = unit_str.chars().next() else {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: "unit string unexpectedly empty".to_string(),
            }
            .into());
        };
        let unit = TenorUnit::from_char(unit_char)?;
        let max_count = match unit {
            TenorUnit::Days => MAX_TENOR_DAYS,
            TenorUnit::Weeks => MAX_TENOR_WEEKS,
            TenorUnit::Months => MAX_TENOR_MONTHS,
            TenorUnit::Years => MAX_TENOR_YEARS,
        };
        if count > max_count {
            return Err(InputError::InvalidTenor {
                tenor: s.to_string(),
                reason: format!("count exceeds maximum supported tenor of {MAX_TENOR_YEARS} years"),
            }
            .into());
        }

        Ok(Self { count, unit })
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
    /// use finstack_core::dates::Tenor;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let tenor = Tenor::parse("6M")?;
    /// assert!((tenor.to_years_simple() - 0.5).abs() < 1e-6);
    ///
    /// let tenor = Tenor::parse("1Y")?;
    /// assert!((tenor.to_years_simple() - 1.0).abs() < 1e-6);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn to_years_simple(&self) -> f64 {
        let count = f64::from(self.count);
        match self.unit {
            TenorUnit::Days => count / 365.0,
            TenorUnit::Weeks => count * 7.0 / 365.0,
            TenorUnit::Months => count / 12.0,
            TenorUnit::Years => count,
        }
    }

    /// Convert tenor to an approximate number of days.
    ///
    /// This uses consistent approximations that align with [`to_years_simple`](Self::to_years_simple):
    /// - 1D = 1 day
    /// - 1W = 7 days
    /// - 1M = 365/12 ≈ 30.42 days (consistent with 1M = 1/12 year)
    /// - 1Y = 365 days
    ///
    /// The month approximation uses 365/12 rather than 30 to maintain consistency
    /// with year fraction calculations. This avoids discrepancies when converting
    /// between days and years for multi-year periods.
    ///
    /// # Returns
    /// Approximate number of days as i64.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::Tenor;
    /// # fn main() -> finstack_core::Result<()> {
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
        const DAYS_PER_MONTH: f64 = super::CALENDAR_DAYS_PER_YEAR / 12.0; // ≈ 30.4167

        let count = f64::from(self.count);
        let days = match self.unit {
            TenorUnit::Days => count,
            TenorUnit::Weeks => count * 7.0,
            TenorUnit::Months => count * DAYS_PER_MONTH,
            TenorUnit::Years => count * super::CALENDAR_DAYS_PER_YEAR,
        };
        days.round() as i64
    }

    /// Add the tenor to a date, optionally respecting a business day calendar.
    ///
    /// # Arguments
    /// * `date` - Starting date
    /// * `calendar` - Optional holiday calendar for business day adjustment
    /// * `bdc` - Business day convention to apply if calendar is provided
    ///
    /// # Returns
    /// The resulting date after adding the tenor period.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Tenor, Date, BusinessDayConvention};
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let start = date!(2025 - 01 - 15);
    /// let tenor = Tenor::parse("1M")?;
    ///
    /// let end = tenor.add_to_date(start, None, BusinessDayConvention::ModifiedFollowing)?;
    /// assert_eq!(end, date!(2025 - 02 - 15));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_to_date(
        &self,
        date: Date,
        calendar: Option<&dyn HolidayCalendar>,
        bdc: BusinessDayConvention,
    ) -> crate::Result<Date> {
        use crate::dates::date_extensions::DateExt;

        let raw_date: crate::Result<Date> = match self.unit {
            TenorUnit::Days => Ok(date + Duration::days(i64::from(self.count))),
            TenorUnit::Weeks => Ok(date + Duration::weeks(i64::from(self.count))),
            TenorUnit::Months => {
                let count_i32 =
                    i32::try_from(self.count).map_err(|_| InputError::InvalidTenor {
                        tenor: self.to_string(),
                        reason: format!("count {} exceeds i32::MAX", self.count),
                    })?;
                Ok(date.add_months(count_i32))
            }
            TenorUnit::Years => {
                let count_i32 =
                    i32::try_from(self.count).map_err(|_| InputError::InvalidTenor {
                        tenor: self.to_string(),
                        reason: format!("count {} exceeds i32::MAX", self.count),
                    })?;
                let months = count_i32
                    .checked_mul(12)
                    .ok_or_else(|| InputError::InvalidTenor {
                        tenor: self.to_string(),
                        reason: format!(
                            "year tenor {} exceeds supported month range after conversion",
                            self.count
                        ),
                    })?;
                Ok(date.add_months(months))
            }
        };
        let raw_date = raw_date?;

        // Apply business day convention if calendar provided
        if let Some(cal) = calendar {
            adjust(raw_date, bdc, cal)
        } else {
            Ok(raw_date)
        }
    }

    /// Convert tenor to year fraction using calendar-aware date computation.
    ///
    /// This method computes the actual year fraction by:
    /// 1. Adding the tenor to the as-of date using proper date arithmetic
    /// 2. Applying business day adjustment if a calendar is provided
    /// 3. Computing the year fraction using the specified day count convention
    ///
    /// # Arguments
    /// * `as_of` - Starting date
    /// * `calendar` - Optional holiday calendar for business day adjustment
    /// * `bdc` - Business day convention to apply
    /// * `day_count` - Day count convention for year fraction calculation
    ///
    /// # Returns
    /// Year fraction computed using the specified conventions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Tenor, Date, DayCount, DayCountContext, BusinessDayConvention};
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let as_of = date!(2025 - 01 - 15);
    /// let tenor = Tenor::parse("1Y")?;
    ///
    /// let years = tenor.to_years_with_context(
    ///     as_of,
    ///     None,
    ///     BusinessDayConvention::ModifiedFollowing,
    ///     DayCount::ActAct,
    /// )?;
    ///
    /// assert!((years - 1.0).abs() < 0.01);
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_years_with_context(
        &self,
        as_of: Date,
        calendar: Option<&dyn HolidayCalendar>,
        bdc: BusinessDayConvention,
        day_count: DayCount,
    ) -> crate::Result<f64> {
        let end_date = self.add_to_date(as_of, calendar, bdc)?;

        let ctx = DayCountContext {
            calendar,
            frequency: None,
            bus_basis: None,
            coupon_period: None,
        };

        day_count.year_fraction(as_of, end_date, ctx)
    }

    /// Convenience constructor for Annual frequency (1 Year).
    #[inline]
    pub const fn annual() -> Self {
        Self::new(1, TenorUnit::Years)
    }

    /// Convenience constructor for Semi-Annual frequency (6 Months).
    #[inline]
    pub const fn semi_annual() -> Self {
        Self::new(6, TenorUnit::Months)
    }

    /// Convenience constructor for Quarterly frequency (3 Months).
    #[inline]
    pub const fn quarterly() -> Self {
        Self::new(3, TenorUnit::Months)
    }

    /// Convenience constructor for Bi-Monthly frequency (2 Months).
    #[inline]
    pub const fn bimonthly() -> Self {
        Self::new(2, TenorUnit::Months)
    }

    /// Convenience constructor for Monthly frequency (1 Month).
    #[inline]
    pub const fn monthly() -> Self {
        Self::new(1, TenorUnit::Months)
    }

    /// Convenience constructor for Bi-Weekly frequency (2 Weeks).
    #[inline]
    pub const fn biweekly() -> Self {
        Self::new(2, TenorUnit::Weeks)
    }

    /// Convenience constructor for Weekly frequency (1 Week).
    #[inline]
    pub const fn weekly() -> Self {
        Self::new(1, TenorUnit::Weeks)
    }

    /// Convenience constructor for Daily frequency (1 Day).
    #[inline]
    pub const fn daily() -> Self {
        Self::new(1, TenorUnit::Days)
    }

    /// Create a Tenor from payments per year.
    ///
    /// Returns an error if payments_per_year is 0 or does not divide 12 evenly.
    pub fn from_payments_per_year(payments: u32) -> crate::Result<Self> {
        if payments == 0 {
            return Err(InputError::InvalidTenor {
                tenor: format!("payments={}", payments),
                reason: "payments_per_year must be positive".to_string(),
            }
            .into());
        }

        // Try to fit into months first
        if 12 % payments == 0 {
            let months = 12 / payments;
            Ok(Self::new(months, TenorUnit::Months))
        } else {
            // If it doesn't fit into months, try roughly into weeks (52)
            // But standard market convention usually implies months.
            // Frequency::from_payments_per_year used to fail if not dividing 12.
            // We'll stick to that behavior for now to match strictness.
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
        let unit_char = match self.unit {
            TenorUnit::Days => 'D',
            TenorUnit::Weeks => 'W',
            TenorUnit::Months => 'M',
            TenorUnit::Years => 'Y',
        };
        write!(f, "{}{}", self.count, unit_char)
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
    fn test_parse_rejects_unreasonably_large_tenors() {
        assert!(Tenor::parse("201Y").is_err());
        assert!(Tenor::parse("2401M").is_err());
        assert!(Tenor::parse("10458W").is_err());
        assert!(Tenor::parse("73201D").is_err());
    }

    #[test]
    fn test_to_years_simple() {
        assert!((Tenor::parse("1D").expect("valid").to_years_simple() - 1.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("7D").expect("valid").to_years_simple() - 7.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("1W").expect("valid").to_years_simple() - 7.0 / 365.0).abs() < 1e-10);
        assert!((Tenor::parse("1M").expect("valid").to_years_simple() - 1.0 / 12.0).abs() < 1e-10);
        assert!((Tenor::parse("3M").expect("valid").to_years_simple() - 0.25).abs() < 1e-10);
        assert!((Tenor::parse("6M").expect("valid").to_years_simple() - 0.5).abs() < 1e-10);
        assert!((Tenor::parse("1Y").expect("valid").to_years_simple() - 1.0).abs() < 1e-10);
        assert!((Tenor::parse("5Y").expect("valid").to_years_simple() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_years_prefers_months_and_handles_invalid_values() {
        assert_eq!(
            Tenor::from_years(1.0, DayCount::Act365F),
            Tenor::new(1, TenorUnit::Years)
        );
        assert_eq!(
            Tenor::from_years(0.25, DayCount::Act365F),
            Tenor::new(3, TenorUnit::Months)
        );
        assert_eq!(
            Tenor::from_years(10.0 / 365.0, DayCount::Act365F),
            Tenor::new(10, TenorUnit::Days)
        );
        assert_eq!(
            Tenor::from_years(-1.0, DayCount::Act365F),
            Tenor::new(0, TenorUnit::Days)
        );
        assert_eq!(
            Tenor::from_years(f64::NAN, DayCount::Act365F),
            Tenor::new(0, TenorUnit::Days)
        );
    }

    #[test]
    fn test_from_payments_per_year_validation() {
        assert_eq!(
            Tenor::from_payments_per_year(4).expect("quarterly"),
            Tenor::new(3, TenorUnit::Months)
        );
        assert_eq!(
            Tenor::from_payments_per_year(12).expect("monthly"),
            Tenor::new(1, TenorUnit::Months)
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

        // Consistency: to_days_approx() / 365 ≈ to_years_simple()
        for tenor_str in &["1M", "3M", "6M", "1Y", "5Y"] {
            let tenor = Tenor::parse(tenor_str).expect("valid");
            let days = tenor.to_days_approx() as f64;
            let years = tenor.to_years_simple();
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
    fn tenor_rejects_count_exceeding_i32_max() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid");
        let tenor = Tenor::new((i32::MAX as u32) + 1, TenorUnit::Months);

        let err = tenor
            .add_to_date(start, None, BusinessDayConvention::Unadjusted)
            .expect_err("count above i32::MAX should be rejected");

        assert!(
            err.to_string().contains("exceeds i32::MAX"),
            "unexpected error: {err}"
        );
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
            )
            .expect("year fraction");

        // 2025 is not a leap year, so 365 days / 365 = 1.0
        assert!((years - 1.0).abs() < 0.001);
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
        assert_eq!(Tenor::daily(), Tenor::new(1, TenorUnit::Days));
        assert_eq!(Tenor::weekly(), Tenor::new(1, TenorUnit::Weeks));
        assert_eq!(Tenor::biweekly(), Tenor::new(2, TenorUnit::Weeks));
        assert_eq!(Tenor::monthly(), Tenor::new(1, TenorUnit::Months));
        assert_eq!(Tenor::bimonthly(), Tenor::new(2, TenorUnit::Months));
        assert_eq!(Tenor::quarterly(), Tenor::new(3, TenorUnit::Months));
        assert_eq!(Tenor::semi_annual(), Tenor::new(6, TenorUnit::Months));
        assert_eq!(Tenor::annual(), Tenor::new(1, TenorUnit::Years));
    }
}
