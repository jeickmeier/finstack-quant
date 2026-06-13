//! Day-count convention algorithms for fixed income and derivative accrual calculations.
//!
//! This module implements industry-standard day count conventions as defined by
//! ISDA (International Swaps and Derivatives Association) and ICMA (International
//! Capital Market Association). All implementations are panic-free and avoid heap
//! allocation.
//!
//! # Date Interval Convention
//!
//! **All day-count calculations use start-inclusive, end-exclusive intervals `[start, end)`.**
//!
//! This means:
//! - The start date **is** counted in the accrual period
//! - The end date **is not** counted in the accrual period
//! - A period from Jan 1 to Jan 2 contains 1 day (Jan 1 only)
//! - A period from Jan 1 to Jan 1 contains 0 days
//!
//! This convention is consistent with how payment dates work in financial instruments:
//! the accrual period ends the day before the payment date, and you don't accrue
//! interest on the payment date itself.
//!
//! # Industry Standards
//!
//! Day count conventions define how interest accrues between two dates. Different
//! markets and instruments use different conventions:
//!
//! # Precision
//!
//! Year fractions are computed as `f64` with typical precision around `1e-9`
//! for standard tenors under roughly 50 years. Precision degrades for very
//! long tenors due to floating-point accumulation, but for most bond and swap
//! applications this remains well within market convention tolerances.
//!
//! ## ISDA Standard Conventions
//!
//! - **Actual/360** (Act/360): Money market standard for USD, EUR short-term rates
//! - **Actual/365 Fixed** (Act/365F): GBP money markets and some bond markets
//! - **30/360** (30U/360): US corporate and municipal bonds
//! - **30E/360** (30E/360): Eurobonds and international bonds
//! - **Actual/Actual (ISDA)**: US Treasury bonds, many swap contracts
//!
//! ## ICMA/ISMA Standard Conventions
//!
//! - **Actual/Actual (ICMA)**: International bonds with regular coupon schedules
//!
//! # Supported Conventions
//!
//! - [`DayCount::Act360`] - Actual/360
//! - [`DayCount::Act365F`] - Actual/365 Fixed
//! - [`DayCount::Act365L`] - Actual/365 Leap (ICMA Rule 251)
//! - [`DayCount::Nl365`] - NL/365 (Actual/365 No Leap)
//! - [`DayCount::Thirty360`] - 30/360 US (Bond Basis)
//! - [`DayCount::ThirtyE360`] - 30E/360 (Eurobond Basis)
//! - [`DayCount::ThirtyE360Isda`] - 30E/360 (ISDA), ISDA 2006 §4.16(h)
//! - [`DayCount::ActAct`] - Actual/Actual (ISDA)
//! - [`DayCount::ActActIsma`] - Actual/Actual (ICMA) regular-period helper
//! - [`DayCount::Bus252`] - Business/252 (Brazilian and some equity markets)
//!
//! # Examples
//! ```
//! use finstack_core::dates::{Date, DayCount, DayCountContext};
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
//!
//! let yf = DayCount::ActAct
//!     .year_fraction(start, end, DayCountContext::default())
//!     .expect("Year fraction calculation should succeed");
//! assert!((yf - 1.0).abs() < 1e-9);
//! ```
//!
//! # Bus/252 Convention
//!
//! The Bus/252 convention counts business days between dates and divides by 252 (typical trading days per year).
//! This requires a holiday calendar to determine business days. Provide the calendar via `DayCountContext`.
//!
//! ```
//! use finstack_core::dates::{Date, DayCount, DayCountContext};
//! use finstack_core::dates::calendar::TARGET2;
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
//! let calendar = TARGET2;
//!
//! // Calculate year fraction with a calendar in context
//! let yf = DayCount::Bus252
//!     .year_fraction(start, end, DayCountContext { calendar: Some(&calendar), frequency: None, bus_basis: None, coupon_period: None })
//!     .expect("Year fraction calculation should succeed");
//! ```
//!
//! # ACT/ACT ISMA vs ISDA
//!
//! Both conventions use actual days in numerator and actual days in denominator, but differ in how
//! the denominator is calculated:
//!
//! - **ACT/ACT (ISDA)**: Uses the actual number of days in the year containing the period
//! - **ACT/ACT (ISMA)**: Uses the actual number of days in the coupon period containing the date
//!
//! ```
//! use finstack_core::dates::{Date, DayCount, Tenor, DayCountContext};
//! use time::Month;
//!
//! // Example: 6-month period in a leap year
//! let start = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"); // Leap year
//! let end   = Date::from_calendar_date(2024, Month::July, 1).expect("Valid date");
//!
//! // ACT/ACT (ISDA): 181 days / 366 days (leap year) = 0.4945355191256831
//! let yf_isda = DayCount::ActAct.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
//!
//! // ACT/ACT (ISMA): frequency-only helper for regular coupon periods
//! // Returns year fractions: a full 6-month regular period = 0.5 years
//! let freq = Tenor::semi_annual(); // Semi-annual
//! let yf_isma = DayCount::ActActIsma
//!     .year_fraction(start, end, DayCountContext { calendar: None, frequency: Some(freq), bus_basis: None, coupon_period: None })
//!     .expect("Year fraction calculation should succeed");
//! // yf_isma ≈ 0.5 (one full semi-annual period in years)
//! ```

#![allow(clippy::many_single_char_names)]

use crate::dates::date_extensions::DateExt;
use smallvec::SmallVec;
use time::{Date, Month};

use crate::dates::date_extensions::BusinessDayIter;
use crate::dates::tenor::TenorUnit;
use crate::dates::{CalendarRegistry, HolidayCalendar, Tenor};
use crate::error::InputError;

/// Optional context for day-count year-fraction calculations.
///
/// Certain conventions require additional information:
/// - `Bus/252` requires a holiday `calendar`.
/// - `Act/Act (ISMA)` requires the coupon `frequency`.
#[derive(Clone, Copy, Default)]
pub struct DayCountContext<'a> {
    /// Holiday calendar for business day conventions
    pub calendar: Option<&'a dyn HolidayCalendar>,
    /// Payment frequency (required for ACT/ACT ISMA)
    pub frequency: Option<Tenor>,
    /// Business day convention (required for Bus/252)
    pub bus_basis: Option<u16>,
    /// Reference coupon period `(start, end)` for ACT/ACT ISMA.
    ///
    /// When set, the ISMA year fraction uses this explicit reference period
    /// instead of re-anchoring from the accrual start date. Required for
    /// correct accrued interest calculations on mid-coupon dates or
    /// irregular first/last coupons.
    pub coupon_period: Option<(Date, Date)>,
}

impl<'a> std::fmt::Debug for DayCountContext<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DayCountContext")
            .field("calendar", &self.calendar.map(|_| "HolidayCalendar"))
            .field("frequency", &self.frequency)
            .field("bus_basis", &self.bus_basis)
            .field("coupon_period", &self.coupon_period)
            .finish()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
/// Serializable snapshot of [`DayCountContext`] state for persistence and interchange.
///
/// This struct captures the optional context parameters (calendar, frequency, business-day basis)
/// needed to reconstruct a [`DayCountContext`] at runtime using a [`CalendarRegistry`].
pub struct DayCountContextState {
    /// Optional calendar code (e.g. "target2").
    pub calendar_id: Option<String>,
    /// Optional coupon frequency for Act/Act ISMA.
    pub frequency: Option<Tenor>,
    /// Optional custom business-day divisor (defaults to 252 when `None`).
    pub bus_basis: Option<u16>,
    /// Optional reference coupon period `(start, end)` for ACT/ACT ISMA,
    /// serialized as two ISO dates.
    ///
    /// Previously this field was silently dropped on serialization, downgrading
    /// exact ICMA accrual to the drifting frequency-only path on round-trip
    /// . The `#[serde(default)]`
    /// keeps the addition wire-compatible: payloads written before this field
    /// existed deserialize with `None`.
    #[serde(default)]
    pub coupon_period: Option<(Date, Date)>,
}

impl DayCountContextState {
    /// Build a runtime [`DayCountContext`] using the provided calendar registry.
    pub fn to_ctx<'a>(&self, registry: &'a CalendarRegistry<'a>) -> DayCountContext<'a> {
        let calendar = self
            .calendar_id
            .as_deref()
            .and_then(|code| registry.resolve_str(code));
        DayCountContext {
            calendar,
            frequency: self.frequency,
            bus_basis: self.bus_basis,
            coupon_period: self.coupon_period,
        }
    }
}

impl<'a> From<DayCountContext<'a>> for DayCountContextState {
    fn from(value: DayCountContext<'a>) -> Self {
        let calendar_id = value
            .calendar
            .and_then(|cal| cal.metadata().map(|meta| meta.id.to_string()));
        Self {
            calendar_id,
            frequency: value.frequency,
            bus_basis: value.bus_basis,
            coupon_period: value.coupon_period,
        }
    }
}

/// Supported day-count conventions with industry-standard definitions.
///
/// Each variant implements a specific day count convention as defined by
/// ISDA, ICMA, or local market conventions. The conventions determine how
/// interest accrues between payment dates.
///
/// # Standards References
///
/// Implementations follow:
/// - **ISDA**: 2006 ISDA Definitions, Section 4.16
/// - **ICMA**: ICMA Rule Book, Rule 251
/// - **ISO**: ISO 20022 Day Count Fraction Codes
///
/// # Examples
///
/// ```rust
/// use finstack_core::dates::{Date, DayCount, DayCountContext};
/// use time::Month;
///
/// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
///
/// // Actual/360 - money market convention
/// let yf_360 = DayCount::Act360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
///
/// // 30/360 - bond convention
/// let yf_30360 = DayCount::Thirty360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
///
/// assert!(yf_360 > yf_30360); // Act/360 has larger denominator
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub enum DayCount {
    /// Actual/360 day count convention.
    ///
    /// Year fraction = (actual days between dates) / 360
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(d)
    /// - **ISO 20022**: Day Count Fraction Code "Actual/360" (A004)
    /// - **Also known as**: Act/360, A/360, French
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - USD money market deposits
    /// - EUR money market instruments
    /// - Short-term rate derivatives (SOFR, €STR)
    /// - FX swaps and forwards
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::April, 1).expect("Valid date"); // 90 days
    ///
    /// let yf = DayCount::Act360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 90.0 / 360.0);
    /// ```
    Act360,

    /// Actual/365 Fixed day count convention.
    ///
    /// Year fraction = (actual days between dates) / 365
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(e)
    /// - **ISO 20022**: Day Count Fraction Code "Actual/365 Fixed" (A005)
    /// - **Also known as**: Act/365F, A/365F, English
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - GBP money markets (SONIA)
    /// - Cable (GBP/USD) FX transactions
    /// - Some Commonwealth bond markets
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Act365F.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert!((yf - 1.0).abs() < 1e-9); // 365 days / 365 = 1.0
    /// ```
    Act365F,

    /// Actual/365 Leap day count convention (Actual/365L) per ICMA Rule 251.
    ///
    /// Year fraction = (actual days) / (365 or 366), where the denominator
    /// rule depends on the coupon frequency supplied via [`DayCountContext`]:
    ///
    /// - **Annual** (or no frequency supplied): 366 if February 29 falls in
    ///   the interval `(start, end]` (exclusive of start, inclusive of end),
    ///   else 365.
    /// - **Non-annual**: 366 if the period END date falls in a leap year,
    ///   else 365.
    ///
    /// # Standards Reference
    ///
    /// - **ICMA**: ICMA Rule Book, Rule 251.1(i)(c)
    /// - **ISO 20022**: Day Count Fraction Code "Actual/365L" (A008)
    /// - **Also known as**: Act/365L, ISMA-Year
    ///
    /// Note: this is **not** ACT/ACT AFB (Association Française des Banques),
    /// which uses a different (sub-period splitting) algorithm. The former
    /// `act_365afb` parse alias was removed because it conflated the two
    /// .
    ///
    /// # Usage
    ///
    /// Used in:
    /// - GBP floating-rate notes
    /// - Some European bond markets
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // Period containing Feb 29, 2024 (leap year)
    /// let start = Date::from_calendar_date(2024, Month::February, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2024, Month::March, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Act365L.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // 29 days / 366 (leap year denominator)
    /// assert_eq!(yf, 29.0 / 366.0);
    /// ```
    Act365L,

    /// 30/360 US (Bond Basis) day count convention.
    ///
    /// Assumes 30 days per month and 360 days per year with US market adjustments.
    ///
    /// # Standards Reference
    ///
    /// - **SIA/PSA**: Standard Securities Calculation Methods (SIA Standard Formulas)
    ///   — primary reference for this implementation, including the February
    ///   end-of-month rule
    /// - **ISO 20022**: Day Count Fraction Code "30/360" (A001)
    /// - **Also known as**: 30U/360, 30/360 US, Bond Basis, 30/360 PSA
    ///
    /// # SIA/PSA vs ISDA
    ///
    /// This implementation follows the SIA/PSA convention, which includes a
    /// February end-of-month rule: when both the start date and the end date
    /// fall on the last day of February, D₂ is changed to 30. ISDA 2006
    /// §4.16(f) specifies a slightly different set of adjustment rules that
    /// omit this February-EOM logic. Both are commonly referred to as
    /// "30/360 US", but they can produce different day counts for periods
    /// that start or end on the last day of February.
    ///
    /// # Formula
    ///
    /// ```text
    /// Days = 360(Y₂ - Y₁) + 30(M₂ - M₁) + (D₂' - D₁')
    ///
    /// where (SIA/PSA rules):
    ///   D₁' = 30                       if D₁ is 31 or last day of February
    ///   D₂' = 30                       if D₂ is 31 and D₁' = 30
    ///   D₂' = 30                       if D₂ is last day of Feb and D₁ is last day of Feb
    ///   otherwise D₁' = D₁, D₂' = D₂
    /// ```
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - US corporate bonds
    /// - US municipal bonds
    /// - US agency debt
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::February, 28).expect("Valid date");
    ///
    /// let yf = DayCount::Thirty360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // Treats Jan 31 as day 30, Feb 28 as day 28: 28 days / 360
    /// assert_eq!(yf, 28.0 / 360.0);
    /// ```
    Thirty360,

    /// 30E/360 (Eurobond Basis) day count convention.
    ///
    /// Assumes 30 days per month and 360 days per year with European adjustments.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(g) - "30E/360"
    /// - **ISO 20022**: Day Count Fraction Code "30E/360" (A002)
    /// - **Also known as**: 30/360 ISDA, 30/360 European, Eurobond Basis
    ///
    /// # Formula
    ///
    /// ```text
    /// Days = 360(Y₂ - Y₁) + 30(M₂ - M₁) + (D₂' - D₁')
    ///
    /// where:
    ///   D₁' = min(D₁, 30)
    ///   D₂' = min(D₂, 30)
    /// ```
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - Eurobonds
    /// - International bonds
    /// - Some interest rate swaps
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::March, 31).expect("Valid date");
    ///
    /// let yf = DayCount::ThirtyE360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // Treats both 31st as day 30: 60 days / 360
    /// assert_eq!(yf, 60.0 / 360.0);
    /// ```
    ThirtyE360,

    /// 30E/360 (ISDA) day count convention.
    ///
    /// Assumes 30 days per month and 360 days per year with the ISDA 2006
    /// §4.16(h) last-day-of-month adjustments (including end-of-February).
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(h) - "30E/360 (ISDA)"
    /// - **Also known as**: 30E/360 ISDA, German, Eurobond Basis (ISDA 2006)
    ///
    /// # Formula
    ///
    /// ```text
    /// Days = 360(Y₂ - Y₁) + 30(M₂ - M₁) + (D₂' - D₁')
    ///
    /// where:
    ///   D₁' = 30 if D₁ is the last day of its month (incl. end of February)
    ///   D₂' = 30 if D₂ is 31, or if D₂ is the last day of February and the
    ///         period does not end on the termination (maturity) date
    /// ```
    ///
    /// # Termination-date exception
    ///
    /// ISDA §4.16(h) keeps D₂ unadjusted when the period ends on the
    /// termination date and that date is the last day of February. Because
    /// [`DayCountContext`] carries no termination flag, this enum variant
    /// always applies the end-of-February rule (i.e. treats `end` as a
    /// non-terminal coupon date). For the final period to maturity, use
    /// [`days_30e_360_isda`] with `end_is_termination_date = true`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // ISDA §4.16(h): both end-of-Feb and Aug 31 count as day 30.
    /// let start = Date::from_calendar_date(2011, Month::August, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2012, Month::February, 29).expect("Valid date");
    ///
    /// let yf = DayCount::ThirtyE360Isda.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 180.0 / 360.0);
    /// ```
    ThirtyE360Isda,

    /// NL/365 (Actual/365 No Leap) day count convention.
    ///
    /// Year fraction = (actual days excluding any February 29) / 365
    ///
    /// # Standards Reference
    ///
    /// - **Also known as**: Act/365 No Leap, NL365, Actual/365NL
    /// - Counts the actual calendar days in `[start, end)` and removes every
    ///   February 29 that falls in the period, so a full leap year still
    ///   yields exactly 1.0.
    ///
    /// # Usage
    ///
    /// Used in:
    /// - Some Canadian money-market and mortgage instruments
    /// - Legacy systems that ignore leap days for accrual
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // Full leap year 2024: 366 actual days, Feb 29 excluded → 365/365 = 1.0
    /// let start = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Nl365.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 1.0);
    /// ```
    Nl365,

    /// Actual/Actual (ISDA) day count convention.
    ///
    /// Uses actual days in numerator and actual days in the containing year(s)
    /// as denominator, splitting across year boundaries.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(b) - "Actual/Actual (ISDA)"
    /// - **ISO 20022**: Day Count Fraction Code "Actual/Actual ISDA" (A006)
    /// - **Also known as**: Act/Act (ISDA), Actual/Actual, Act/Act
    ///
    /// # Algorithm
    ///
    /// For a period spanning multiple calendar years:
    /// 1. Split period at year boundaries
    /// 2. For each year segment: (days in segment) / (days in that year)
    /// 3. Sum the year fractions
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - US Treasury bonds
    /// - Interest rate swaps (USD, EUR fixed legs)
    /// - Government bonds in many markets
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // Period spanning year boundary (leap year 2024)
    /// let start = Date::from_calendar_date(2024, Month::July, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
    ///
    /// let yf = DayCount::ActAct.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // 184/366 (Jul-Dec 2024 in leap year) + 365/365 (all of 2025)
    /// assert!((yf - 1.0).abs() < 0.01);
    /// ```
    ///
    /// # References
    ///
    /// - ISDA (2006). "2006 ISDA Definitions." Section 4.16(b).
    ActAct,

    /// Actual/Actual (ICMA) day count convention.
    ///
    /// Uses actual days in numerator and actual days in the coupon period
    /// as denominator, requiring knowledge of payment frequency.
    ///
    /// # Standards Reference
    ///
    /// - **ICMA**: ICMA Rule Book, Rule 251 - "Actual/Actual (ICMA)"
    /// - **ISO 20022**: Day Count Fraction Code "Actual/Actual ICMA" (A007)
    /// - **Also known as**: Act/Act (ICMA), Act/Act (ISMA), ISMA-99
    ///
    /// # Algorithm
    ///
    /// 1. Determine quasi-coupon periods based on payment frequency
    /// 2. For each period: (actual days) / (actual days in coupon period)
    /// 3. Sum fractions across periods
    ///
    /// # Usage
    ///
    /// Standard for:
    /// - International bonds with regular coupons
    /// - Eurobonds with semi-annual or annual payments
    /// - ICMA-governed securities
    ///
    /// # Requirements
    ///
    /// Requires `frequency` in [`DayCountContext`] to determine regular coupon periods.
    /// For irregular first/last coupons, use
    /// [`act_act_isma_year_fraction_with_reference_period`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext, Tenor};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::July, 15).expect("Valid date");
    /// let freq = Tenor::semi_annual(); // Semi-annual
    ///
    /// let yf = DayCount::ActActIsma.year_fraction(
    ///     start,
    ///     end,
    ///     DayCountContext { frequency: Some(freq), ..Default::default() }
    /// ).expect("Year fraction calculation should succeed");
    ///
    /// // Full semi-annual period = 0.5 year fraction (6 months / 12 months)
    /// assert!((yf - 0.5).abs() < 1e-6);
    /// ```
    ///
    /// # References
    ///
    /// - ICMA (2010). "ICMA Rule Book." Rule 251.
    /// - ISMA (1999). "Recommendations for Accrued Interest Calculations."
    ActActIsma,

    /// Business/252 day count convention.
    ///
    /// Year fraction = (business days between dates) / 252
    ///
    /// # Market Convention
    ///
    /// - **Brazil**: Standard for BRL-denominated instruments (ANBIMA)
    /// - **Also used**: Some equity derivatives and variance swaps
    /// - **Basis**: 252 represents typical trading days per year
    ///
    /// # Requirements
    ///
    /// Requires `calendar` in [`DayCountContext`] to determine business days.
    ///
    /// # Performance
    ///
    /// Iterates each calendar day in the range to check business-day status,
    /// giving O(n) cost where n is the number of calendar days between the
    /// dates. For 30Y instruments this is ~11,000 iterations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use finstack_core::dates::calendar::NYSE;
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 6).expect("Valid date"); // Monday
    /// let end = Date::from_calendar_date(2025, Month::January, 13).expect("Valid date"); // Next Monday
    ///
    /// let yf = DayCount::Bus252.year_fraction(
    ///     start,
    ///     end,
    ///     DayCountContext { calendar: Some(&NYSE), ..Default::default() }
    /// ).expect("Year fraction calculation should succeed");
    ///
    /// // 5 business days / 252
    /// assert!((yf * 252.0 - 5.0).abs() < 0.1);
    /// ```
    Bus252,
}

impl DayCount {
    /// Compute the year fraction between `start` and `end` per this convention.
    ///
    /// Provide any required context via [`DayCountContext`]:
    /// - `Bus/252` requires a holiday calendar
    /// - `Act/Act (ISMA)` requires a coupon frequency
    ///
    /// # Arguments
    ///
    /// * `start` - Start date (inclusive)
    /// * `end` - End date (exclusive)
    /// * `ctx` - Optional context providing calendar or frequency as needed
    ///
    /// # Returns
    ///
    /// - `Ok(0.0)` if `start == end`
    /// - `Ok(year_fraction)` for the calculated year fraction (always ≥ 0)
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// - [`InputError::InvalidDateRange`](crate::error::InputError::InvalidDateRange):
    ///   `start > end` (inverted date range)
    /// - [`InputError::MissingCalendarForBus252`](crate::error::InputError::MissingCalendarForBus252):
    ///   Using `Bus252` without a calendar in `ctx`
    /// - [`InputError::InvalidBusBasis`](crate::error::InputError::InvalidBusBasis):
    ///   Using `Bus252` with a zero basis
    /// - [`InputError::MissingFrequencyForActActIsma`](crate::error::InputError::MissingFrequencyForActActIsma):
    ///   Using `ActActIsma` without a frequency in `ctx`
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::error::InputError::ActActIsmaUnsupportedFrequency):
    ///   Using `ActActIsma` with a Day or Week frequency
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Act360.year_fraction(start, end, DayCountContext::default())?;
    /// assert!(yf > 0.0);
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    pub fn year_fraction(
        self,
        start: Date,
        end: Date,
        ctx: DayCountContext<'_>,
    ) -> crate::Result<f64> {
        // Early returns for edge cases - flattens nesting
        if start > end {
            return Err(InputError::InvalidDateRange.into());
        }
        if start == end {
            return Ok(0.0);
        }

        // Dispatch to convention-specific calculations
        self.year_fraction_impl(start, end, ctx)
    }

    /// Internal implementation dispatching to convention-specific calculations.
    ///
    /// Precondition: `start < end` (validated by `year_fraction`).
    fn year_fraction_impl(
        self,
        start: Date,
        end: Date,
        ctx: DayCountContext<'_>,
    ) -> crate::Result<f64> {
        let days = (end - start).whole_days() as f64;

        match self {
            DayCount::Act360 => Ok(days / 360.0),
            DayCount::Act365F => Ok(days / 365.0),
            DayCount::Act365L => Ok(year_fraction_act_365l(start, end, ctx)),
            DayCount::Thirty360 => {
                Ok(days_30_360(start, end, Thirty360Convention::UsSia) as f64 / 360.0)
            }
            DayCount::ThirtyE360 => {
                Ok(days_30_360(start, end, Thirty360Convention::European) as f64 / 360.0)
            }
            DayCount::ThirtyE360Isda => Ok(f64::from(days_30e_360_isda(start, end, false)) / 360.0),
            DayCount::Nl365 => Ok(year_fraction_nl_365(start, end)),
            DayCount::ActAct => year_fraction_act_act_isda(start, end),
            DayCount::ActActIsma => year_fraction_act_act_isma_with_ctx(start, end, ctx),
            DayCount::Bus252 => year_fraction_bus252(start, end, ctx),
        }
    }

    /// Calculate signed year fraction between two dates.
    ///
    /// Returns positive if `end > start`, negative if `end < start`, and zero if equal.
    /// This is useful for cashflow discounting where time can be negative relative to a base date.
    ///
    /// # Arguments
    ///
    /// * `start` - Reference date
    /// * `end` - Target date
    /// * `ctx` - Optional context providing calendar or frequency as needed
    ///
    /// # Returns
    ///
    /// - `Ok(0.0)` if `start == end`
    /// - `Ok(positive)` if `end > start`
    /// - `Ok(negative)` if `end < start`
    ///
    /// # Errors
    ///
    /// Same errors as [`year_fraction`](Self::year_fraction), but never returns
    /// `InvalidDateRange` since inverted dates produce negative fractions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let base = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
    /// let past = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let future = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    ///
    /// let yf_past = DayCount::Act365F.signed_year_fraction(base, past, DayCountContext::default())?;
    /// let yf_future = DayCount::Act365F.signed_year_fraction(base, future, DayCountContext::default())?;
    ///
    /// assert!(yf_past < 0.0);  // Past is negative
    /// assert!(yf_future > 0.0); // Future is positive
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    pub fn signed_year_fraction(
        self,
        start: Date,
        end: Date,
        ctx: DayCountContext<'_>,
    ) -> crate::Result<f64> {
        if start == end {
            Ok(0.0)
        } else if end > start {
            self.year_fraction(start, end, ctx)
        } else {
            Ok(-self.year_fraction(end, start, ctx)?)
        }
    }

    /// Calendar days between two dates (signed: negative when `end < start`).
    ///
    /// This is a thin convenience wrapper around `(end - start).whole_days()`.
    /// It counts raw calendar days without regard for any day-count convention,
    /// business-day calendar, or holiday schedule.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::dates::{Date, DayCount};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end   = Date::from_calendar_date(2025, Month::February, 1).expect("Valid date");
    /// assert_eq!(DayCount::calendar_days(start, end), 31);
    ///
    /// // Negative when end < start
    /// assert_eq!(DayCount::calendar_days(end, start), -31);
    /// ```
    pub fn calendar_days(start: Date, end: Date) -> i64 {
        (end - start).whole_days()
    }
}

const MAX_ACT_ACT_ISMA_RECURSION_DEPTH: usize = 512;

/// Calculate ACT/ACT (ICMA/ISMA) year fraction using explicit reference coupon boundaries.
///
/// This helper is intended for irregular first/last coupons where the regular
/// coupon period cannot be inferred from `start`, `end`, and `frequency` alone.
/// The `reference_start`/`reference_end` pair must describe one regular coupon
/// period from the underlying schedule.
///
/// Use this helper when you already know the surrounding regular coupon period
/// from the bond schedule. For regular coupons, prefer
/// [`DayCount::ActActIsma`](DayCount::ActActIsma) with a [`DayCountContext`] that
/// supplies only the coupon frequency.
///
/// # Arguments
///
/// * `start` - Accrual start date of the coupon being measured
/// * `end` - Accrual end date of the coupon being measured
/// * `reference_start` - Start of the corresponding regular coupon period
/// * `reference_end` - End of the corresponding regular coupon period
///
/// # Returns
///
/// The ICMA/ISMA year fraction for the irregular coupon period.
///
/// # Errors
///
/// Returns an error if the accrual dates are reversed, the reference period is
/// invalid, or the algorithm would need an implausibly deep recursion to align
/// the supplied reference period.
///
/// # References
///
/// - ICMA convention background: `docs/REFERENCES.md#icma-rule-book`
pub fn act_act_isma_year_fraction_with_reference_period(
    start: Date,
    end: Date,
    reference_start: Date,
    reference_end: Date,
) -> crate::Result<f64> {
    if start > end {
        return Err(InputError::InvalidDateRange.into());
    }
    if start == end {
        return Ok(0.0);
    }
    if reference_start >= reference_end {
        return Err(InputError::InvalidDateRange.into());
    }

    let period_months = reference_start.months_until(reference_end);
    if period_months == 0 {
        return Err(InputError::Invalid.into());
    }
    let coupon_length_years = period_months as f64 / 12.0;

    fn recurse(
        start: Date,
        end: Date,
        reference_start: Date,
        reference_end: Date,
        period_months: u32,
        coupon_length_years: f64,
        depth: usize,
    ) -> crate::Result<f64> {
        if start == end {
            return Ok(0.0);
        }
        if depth >= MAX_ACT_ACT_ISMA_RECURSION_DEPTH {
            tracing::warn!(
                "ACT/ACT ISMA reference-period traversal exceeded maximum depth of {MAX_ACT_ACT_ISMA_RECURSION_DEPTH}"
            );
            return Err(InputError::Invalid.into());
        }
        if reference_start >= reference_end {
            return Err(InputError::InvalidDateRange.into());
        }

        if start >= reference_start && end <= reference_end {
            let accrual_days = (end - start).whole_days() as f64;
            let reference_days = (reference_end - reference_start).whole_days() as f64;
            if reference_days <= 0.0 {
                return Err(InputError::Invalid.into());
            }
            return Ok((accrual_days / reference_days) * coupon_length_years);
        }

        let period_months_i32 = i32::try_from(period_months).map_err(|_| InputError::Invalid)?;

        if end <= reference_start {
            let previous_start = reference_start.add_months(-period_months_i32);
            return recurse(
                start,
                end,
                previous_start,
                reference_start,
                period_months,
                coupon_length_years,
                depth + 1,
            );
        }

        if start >= reference_end {
            let next_end = reference_end.add_months(period_months_i32);
            return recurse(
                start,
                end,
                reference_end,
                next_end,
                period_months,
                coupon_length_years,
                depth + 1,
            );
        }

        if start < reference_start {
            let previous_start = reference_start.add_months(-period_months_i32);
            return Ok(recurse(
                start,
                reference_start,
                previous_start,
                reference_start,
                period_months,
                coupon_length_years,
                depth + 1,
            )? + recurse(
                reference_start,
                end,
                reference_start,
                reference_end,
                period_months,
                coupon_length_years,
                depth + 1,
            )?);
        }

        if end > reference_end {
            let next_end = reference_end.add_months(period_months_i32);
            return Ok(recurse(
                start,
                reference_end,
                reference_start,
                reference_end,
                period_months,
                coupon_length_years,
                depth + 1,
            )? + recurse(
                reference_end,
                end,
                reference_end,
                next_end,
                period_months,
                coupon_length_years,
                depth + 1,
            )?);
        }

        Err(InputError::Invalid.into())
    }

    recurse(
        start,
        end,
        reference_start,
        reference_end,
        period_months,
        coupon_length_years,
        0,
    )
}

// -------------------------------------------------------------------------------------------------
// 30/360 generalized helper
// -------------------------------------------------------------------------------------------------
/// 30/360 day-count variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Thirty360Convention {
    /// 30U/360 (US SIA / Bond Basis).
    #[serde(rename = "Us", alias = "us")]
    UsSia,
    /// 30/360 ISDA bond basis (ISDA 2006 §4.16(f); no February EOM rule).
    ///
    /// Reachable via the public [`days_30_360`] helper; the [`DayCount`] enum
    /// exposes the SIA/PSA ([`DayCount::Thirty360`]) and 30E/360 variants
    /// instead.
    Isda,
    /// 30E/360 (European).
    European,
}

/// Compute day count between `start` (inclusive) and `end` (exclusive) under a 30/360 convention.
///
/// Precondition: `start <= end`. If violated, the returned value will be negative.
/// This helper is panic-free and allocation-free.
///
/// # Examples
///
/// ```rust
/// use finstack_core::dates::{days_30_360, Thirty360Convention};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
/// let end = Date::from_calendar_date(2025, Month::March, 31).expect("Valid date");
///
/// // ISDA 2006 §4.16(f): D1 31 → 30, then D2 31 → 30.
/// assert_eq!(days_30_360(start, end, Thirty360Convention::Isda), 60);
/// ```
#[inline]
pub fn days_30_360(start: Date, end: Date, convention: Thirty360Convention) -> i32 {
    let (y1, m1, d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, d2) = (end.year(), end.month() as i32, end.day() as i32);

    let (d1_adj, d2_adj) = match convention {
        Thirty360Convention::UsSia => {
            // SIA/PSA 30/360 US Bond Basis:
            // - If D1 is 31 or last day of February, change D1 to 30
            // - If D2 is 31 and D1 was adjusted to 30, change D2 to 30
            // - If D2 is last day of Feb AND D1 was last day of Feb, change D2 to 30
            // (The Feb-EOM rule is SIA/PSA-specific; ISDA 2006 §4.16(f) omits it.)
            let d1_adj = if d1 == 31 || is_last_day_of_february(start) {
                30
            } else {
                d1
            };
            let d2_adj = if (d2 == 31 && d1_adj == 30)
                || (is_last_day_of_february(end) && is_last_day_of_february(start))
            {
                30
            } else {
                d2
            };
            (d1_adj, d2_adj)
        }
        Thirty360Convention::Isda => {
            let d1_adj = if d1 == 31 { 30 } else { d1 };
            let d2_adj = if d2 == 31 && d1_adj == 30 { 30 } else { d2 };
            (d1_adj, d2_adj)
        }
        Thirty360Convention::European => {
            // ISDA 2006 §4.16(g) - 30E/360:
            // - If D1 is 31, change D1 to 30
            // - If D2 is 31, change D2 to 30
            // Note: NO February EOM rule for European convention
            let d1_adj = if d1 == 31 { 30 } else { d1 };
            let d2_adj = if d2 == 31 { 30 } else { d2 };
            (d1_adj, d2_adj)
        }
    };

    (y2 - y1) * 360 + (m2 - m1) * 30 + (d2_adj - d1_adj)
}

/// Check if date is the last day of February (28 or 29 depending on leap year).
///
/// Per SIA/PSA Standard Formulas, the last day of February receives special
/// treatment in 30/360 US Bond Basis calculations.
#[inline]
fn is_last_day_of_february(date: Date) -> bool {
    date.month() == Month::February && date.day() == date.month().length(date.year())
}

/// Check if a date is the last calendar day of its month.
#[inline]
fn is_last_day_of_month(date: Date) -> bool {
    date.day() == date.month().length(date.year())
}

/// Compute the 30E/360 (ISDA) day count per ISDA 2006 §4.16(h).
///
/// Adjustment rules:
/// - D₁ becomes 30 when `start` is the last day of its month (including the
///   last day of February).
/// - D₂ becomes 30 when `end` is day 31, or when `end` is the last day of
///   February **and** `end_is_termination_date` is `false`.
///
/// The termination-date exception means the final accrual period of an
/// instrument maturing on the last day of February keeps the actual day
/// number (28/29); pass `end_is_termination_date = true` for that period.
/// [`DayCount::ThirtyE360Isda`] always passes `false` because
/// [`DayCountContext`] carries no termination flag.
///
/// Precondition: `start <= end`. If violated, the returned value will be
/// negative. This helper is panic-free and allocation-free.
///
/// # Examples
///
/// ```rust
/// use finstack_core::dates::days_30e_360_isda;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2012, Month::January, 28).expect("Valid date");
/// let end = Date::from_calendar_date(2012, Month::February, 29).expect("Valid date");
///
/// // Intermediate coupon: end-of-Feb → 30; 30 + (30 - 28) = 32
/// assert_eq!(days_30e_360_isda(start, end, false), 32);
/// // Final period to maturity: Feb 29 kept; 30 + (29 - 28) = 31
/// assert_eq!(days_30e_360_isda(start, end, true), 31);
/// ```
///
/// # References
///
/// - ISDA (2006). "2006 ISDA Definitions." Section 4.16(h).
#[inline]
pub fn days_30e_360_isda(start: Date, end: Date, end_is_termination_date: bool) -> i32 {
    let (y1, m1, d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, d2) = (end.year(), end.month() as i32, end.day() as i32);

    let d1_adj = if is_last_day_of_month(start) { 30 } else { d1 };
    let d2_adj = if d2 == 31 || (is_last_day_of_february(end) && !end_is_termination_date) {
        30
    } else {
        d2
    };

    (y2 - y1) * 360 + (m2 - m1) * 30 + (d2_adj - d1_adj)
}

// (Wrappers removed in favor of the public `days_30_360` with `Thirty360Convention`.)

// -------------------------------------------------------------------------------------------------
// ACT/ACT (ISDA) helper
// -------------------------------------------------------------------------------------------------
fn year_fraction_act_act_isda(start: Date, end: Date) -> crate::Result<f64> {
    if start == end {
        return Ok(0.0);
    }

    if start.year() == end.year() {
        let denom = days_in_year(start.year()) as f64;
        let days = (end - start).whole_days() as f64;
        return Ok(days / denom);
    }

    // Days from start to 31-Dec of start year (inclusive of start, exclusive of next year 1-Jan).
    let start_year_end = crate::dates::create_date(start.year() + 1, Month::January, 1)?;
    let days_start_year = (start_year_end - start).whole_days() as f64;
    let mut frac = days_start_year / days_in_year(start.year()) as f64;

    // Full intermediate years
    for _year in (start.year() + 1)..end.year() {
        frac += 1.0; // each full year counts as exactly 1.0
    }

    // Days from 1-Jan of end year to end date
    let start_of_end_year = crate::dates::create_date(end.year(), Month::January, 1)?;
    let days_end_year = (end - start_of_end_year).whole_days() as f64;
    frac += days_end_year / days_in_year(end.year()) as f64;

    Ok(frac)
}

// -------------------------------------------------------------------------------------------------
// Context-aware helpers for year_fraction_impl
// -------------------------------------------------------------------------------------------------

/// ACT/ACT (ISMA) with context extraction.
///
/// When `ctx.coupon_period` is set, delegates to
/// [`act_act_isma_year_fraction_with_reference_period`] for exact
/// mid-coupon accrual. Otherwise falls back to the frequency-based
/// approach that re-anchors from `start`.
fn year_fraction_act_act_isma_with_ctx(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let freq = ctx
        .frequency
        .ok_or(InputError::MissingFrequencyForActActIsma)?;
    if let Some((ref_start, ref_end)) = ctx.coupon_period {
        act_act_isma_year_fraction_with_reference_period(start, end, ref_start, ref_end)
    } else {
        year_fraction_act_act_isma(start, end, freq)
    }
}

/// Bus/252 with context extraction - validates calendar is present and basis is non-zero.
fn year_fraction_bus252(start: Date, end: Date, ctx: DayCountContext<'_>) -> crate::Result<f64> {
    let cal = ctx.calendar.ok_or(InputError::MissingCalendarForBus252)?;
    let basis = ctx.bus_basis.unwrap_or(252);
    if basis == 0 {
        return Err(InputError::InvalidBusBasis { basis }.into());
    }
    let biz_days = count_business_days(start, end, cal) as f64;
    Ok(biz_days / f64::from(basis))
}

// -------------------------------------------------------------------------------------------------
// ACT/ACT (ISMA/ICMA) helper
// -------------------------------------------------------------------------------------------------
/// Calculate year fraction for ACT/ACT (ISMA/ICMA) convention with coupon-period awareness.
fn year_fraction_act_act_isma(start: Date, end: Date, freq: Tenor) -> crate::Result<f64> {
    if start == end {
        return Ok(0.0);
    }

    // Coupon length in years based on frequency (e.g., 0.5 for semi-annual, 0.25 for quarterly).
    // ISMA/ICMA is defined for regular coupon periods; treat Week/Day frequencies as invalid.
    let coupon_length_years = match freq.unit {
        TenorUnit::Months => freq.count as f64 / 12.0,
        TenorUnit::Years => freq.count as f64,
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: freq.to_string(),
            }
            .into());
        }
    };

    // For ISMA, we need to work with quasi-coupon periods.
    //
    // The quasi-coupon grid is anchored on `start` itself: each boundary is
    // `start + k·freq` computed directly from the unadjusted anchor
    // (k-multiples, roll-day preserved with per-month clamping), NOT by
    // chaining `prev + freq`. Chained stepping from `start - freq` (the
    // previous implementation) lost the roll day for month-end starts: a
    // regular EOM semi-annual period [2025-08-31, 2026-02-28) drifted to a
    // grid ending Aug 28 and returned 181/184 × 0.5 ≈ 0.49185 instead of
    // exactly 0.5 .
    let months_per_period = match freq.unit {
        TenorUnit::Months => freq.count as i32,
        TenorUnit::Years => freq.count as i32 * 12,
        // Unreachable: rejected above when computing `coupon_length_years`.
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: freq.to_string(),
            }
            .into());
        }
    };
    if months_per_period <= 0 {
        return Err(InputError::ActActIsmaUnsupportedFrequency {
            frequency: freq.to_string(),
        }
        .into());
    }

    let mut total_fraction = 0.0;

    // Optimization: Manually generate dates to avoid heap allocation of ScheduleBuilder
    // Most ISMA calculations involve very few periods, but long-dated bonds (15+ years)
    // with semi-annual coupons can have 30+ periods. Using 32 elements covers ~16 years
    // of semi-annual coupons without heap allocation.
    let mut periods: SmallVec<[Date; 32]> = SmallVec::new();
    periods.push(start);
    let mut k: i32 = 1;
    loop {
        let boundary = start.add_months(k * months_per_period);
        periods.push(boundary);
        if boundary >= end {
            break;
        }
        k += 1;
    }

    // Find the periods that overlap with our [start, end) interval
    for window in periods.windows(2) {
        let period_start = window[0];
        let period_end = window[1];

        // Check if this period overlaps with our target interval
        let overlap_start = start.max(period_start);
        let overlap_end = end.min(period_end);

        if overlap_start < overlap_end {
            // Numerator: actual days in the overlapping slice
            let days_in_overlap = (overlap_end - overlap_start).whole_days() as f64;

            // Denominator (ISMA): actual days in the coupon period that contains this slice
            let coupon_days = (period_end - period_start).whole_days() as f64;
            if coupon_days <= 0.0 {
                return Err(InputError::Invalid.into());
            }

            // Year fraction = (days in slice / days in coupon period) × coupon period in years
            total_fraction += (days_in_overlap / coupon_days) * coupon_length_years;
        }
    }

    Ok(total_fraction)
}

// -------------------------------------------------------------------------------------------------
// ACT/365L helper
// -------------------------------------------------------------------------------------------------
/// Calculate year fraction for Act/365L convention per ICMA Rule 251.1(i)(c).
///
/// The denominator rule depends on the coupon frequency supplied via
/// [`DayCountContext`]:
///
/// - **Annual** (or no frequency supplied): 366 if February 29 falls in the
///   interval `(start, end]` (exclusive of start, inclusive of end), else 365.
/// - **Non-annual**: 366 if the period END date falls in a leap year, else 365.
///
/// Previously the Feb-29 window was `[start, end)` and the frequency rule was
/// ignored .
fn year_fraction_act_365l(start: Date, end: Date, ctx: DayCountContext<'_>) -> f64 {
    if start == end {
        return 0.0;
    }

    let actual_days = (end - start).whole_days() as f64;

    // ICMA Rule 251: the Feb-29 rule applies to annual-pay instruments; for
    // any other frequency the leap-year status of the period end date decides.
    // With no frequency in context, default to the annual rule.
    let annual = match ctx.frequency {
        Some(freq) => matches!(
            (freq.unit, freq.count),
            (TenorUnit::Years, 1) | (TenorUnit::Months, 12)
        ),
        None => true,
    };

    let leap = if annual {
        interval_contains_feb_29(start, end)
    } else {
        time::util::is_leap_year(end.year())
    };

    actual_days / if leap { 366.0 } else { 365.0 }
}

/// Check if February 29 falls in the interval `(start, end]` (exclusive of
/// start, inclusive of end) per ICMA Rule 251.
fn interval_contains_feb_29(start: Date, end: Date) -> bool {
    let start_year = start.year();
    let end_year = end.year();

    for year in start_year..=end_year {
        if time::util::is_leap_year(year) {
            if let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29) {
                if feb_29 > start && feb_29 <= end {
                    return true;
                }
            }
        }
    }
    false
}

// -------------------------------------------------------------------------------------------------
// NL/365 helper
// -------------------------------------------------------------------------------------------------
/// Calculate year fraction for NL/365 (Actual/365 No Leap).
///
/// Counts actual days in `[start, end)` excluding any February 29, divided by
/// a fixed 365-day year.
fn year_fraction_nl_365(start: Date, end: Date) -> f64 {
    if start == end {
        return 0.0;
    }

    let actual_days = (end - start).whole_days();
    let mut leap_days: i64 = 0;
    for year in start.year()..=end.year() {
        if time::util::is_leap_year(year) {
            if let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29) {
                // Day-count intervals are [start, end): exclude Feb 29 when it
                // is an accrued day of the period.
                if feb_29 >= start && feb_29 < end {
                    leap_days += 1;
                }
            }
        }
    }
    (actual_days - leap_days) as f64 / 365.0
}

// -------------------------------------------------------------------------------------------------
// Bus/252 helper
// -------------------------------------------------------------------------------------------------
/// Count business days between start (inclusive) and end (exclusive) using the given calendar.
fn count_business_days<C: HolidayCalendar + ?Sized>(start: Date, end: Date, calendar: &C) -> i32 {
    BusinessDayIter::new(start, end, calendar).count() as i32
}

#[inline]
const fn days_in_year(year: i32) -> i32 {
    if time::util::is_leap_year(year) {
        366
    } else {
        365
    }
}

// ---------------------------------------------------------------------------
// Display + FromStr
// ---------------------------------------------------------------------------

impl std::fmt::Display for DayCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            DayCount::Act360 => "act_360",
            DayCount::Act365F => "act_365f",
            DayCount::Act365L => "act_365l",
            DayCount::Nl365 => "nl_365",
            DayCount::Thirty360 => "30_360",
            DayCount::ThirtyE360 => "30e_360",
            DayCount::ThirtyE360Isda => "30e_360_isda",
            DayCount::ActAct => "act_act",
            DayCount::ActActIsma => "act_act_isma",
            DayCount::Bus252 => "bus_252",
        };
        f.write_str(label)
    }
}

impl crate::parse::NormalizedEnum for DayCount {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("act_360", Self::Act360),
        ("act360", Self::Act360),
        ("actual_360", Self::Act360),
        ("act_365f", Self::Act365F),
        ("act365f", Self::Act365F),
        ("actual_365f", Self::Act365F),
        ("act_365l", Self::Act365L),
        ("act365l", Self::Act365L),
        ("actual_365l", Self::Act365L),
        // NOTE: the former "act_365afb" alias was removed: ACT/ACT AFB is a
        // different convention from Act/365L .
        ("nl_365", Self::Nl365),
        ("nl365", Self::Nl365),
        ("act_365_nl", Self::Nl365),
        ("actual_365_nl", Self::Nl365),
        ("30_360", Self::Thirty360),
        ("thirty_360", Self::Thirty360),
        ("thirty360", Self::Thirty360),
        ("30u_360", Self::Thirty360),
        ("bond_basis", Self::Thirty360),
        ("30_360_bond_basis", Self::Thirty360),
        ("30e_360", Self::ThirtyE360),
        ("30e360", Self::ThirtyE360),
        ("30_360e", Self::ThirtyE360),
        ("eurobond_basis", Self::ThirtyE360),
        ("30e_360_isda", Self::ThirtyE360Isda),
        ("30e360_isda", Self::ThirtyE360Isda),
        ("30e_360isda", Self::ThirtyE360Isda),
        ("act_act", Self::ActAct),
        ("actact", Self::ActAct),
        ("actual_actual", Self::ActAct),
        ("act_act_isda", Self::ActAct),
        ("isda", Self::ActAct),
        ("act_act_isma", Self::ActActIsma),
        ("act_act_icma", Self::ActActIsma),
        ("actactisma", Self::ActActIsma),
        ("icma", Self::ActActIsma),
        ("bus_252", Self::Bus252),
        ("bus252", Self::Bus252),
        ("business_252", Self::Bus252),
    ];
}

impl std::str::FromStr for DayCount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
    }
}

#[cfg(test)]
mod tests {
    use super::act_act_isma_year_fraction_with_reference_period;
    use time::macros::date;

    fn assert_parses_to(label: &str, expected: super::DayCount) {
        assert!(matches!(label.parse::<super::DayCount>(), Ok(value) if value == expected));
    }

    #[test]
    fn act_act_isma_reference_period_rejects_excessive_recursion_depth() {
        let result = act_act_isma_year_fraction_with_reference_period(
            date!(1700 - 01 - 01),
            date!(1700 - 01 - 02),
            date!(2025 - 01 - 01),
            date!(2025 - 07 - 01),
        );

        assert!(
            result.is_err(),
            "far-away reference traversal should be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // FromStr / Display roundtrip tests
    // -----------------------------------------------------------------------

    #[test]
    fn daycount_display_roundtrip() {
        let all = [
            super::DayCount::Act360,
            super::DayCount::Act365F,
            super::DayCount::Act365L,
            super::DayCount::Nl365,
            super::DayCount::Thirty360,
            super::DayCount::ThirtyE360,
            super::DayCount::ThirtyE360Isda,
            super::DayCount::ActAct,
            super::DayCount::ActActIsma,
            super::DayCount::Bus252,
        ];

        for dc in &all {
            let label = dc.to_string();
            assert!(
                matches!(label.parse::<super::DayCount>(), Ok(value) if value == *dc),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn daycount_from_str_aliases() {
        use super::DayCount;

        // Act360
        assert_parses_to("act360", DayCount::Act360);
        assert_parses_to("actual_360", DayCount::Act360);
        assert_parses_to("ACT/360", DayCount::Act360);

        // Act365F
        assert_parses_to("act365f", DayCount::Act365F);
        assert_parses_to("actual_365f", DayCount::Act365F);

        // Act365L
        assert_parses_to("actual_365l", DayCount::Act365L);
        // The "act_365afb" alias was removed: ACT/ACT AFB is a different
        // convention from Act/365L .
        assert!("act_365afb".parse::<DayCount>().is_err());

        // Nl365
        assert_parses_to("nl_365", DayCount::Nl365);
        assert_parses_to("NL/365", DayCount::Nl365);
        assert_parses_to("act_365_nl", DayCount::Nl365);

        // Thirty360
        assert_parses_to("30/360", DayCount::Thirty360);
        assert_parses_to("thirty360", DayCount::Thirty360);
        assert_parses_to("bond_basis", DayCount::Thirty360);
        assert_parses_to("30U/360", DayCount::Thirty360);

        // ThirtyE360
        assert_parses_to("30E/360", DayCount::ThirtyE360);
        assert_parses_to("eurobond_basis", DayCount::ThirtyE360);

        // ThirtyE360Isda
        assert_parses_to("30E/360 ISDA", DayCount::ThirtyE360Isda);
        assert_parses_to("30e_360_isda", DayCount::ThirtyE360Isda);

        // ActAct
        assert_parses_to("act_act", DayCount::ActAct);
        assert_parses_to("act/act ISDA", DayCount::ActAct);
        assert_parses_to("isda", DayCount::ActAct);

        // ActActIsma
        assert_parses_to("act_act_icma", DayCount::ActActIsma);
        assert_parses_to("icma", DayCount::ActActIsma);

        // Bus252
        assert_parses_to("bus252", DayCount::Bus252);
        assert_parses_to("business_252", DayCount::Bus252);
    }

    #[test]
    fn daycount_from_str_unknown() {
        assert!("garbage".parse::<super::DayCount>().is_err());
    }

    // -----------------------------------------------------------------------
    // Act/365L ICMA Rule 251 boundary tests
    //
    // Updated the
    // Feb-29 window for the annual rule is (start, end] per ICMA Rule 251,
    // not [start, end), and a non-annual coupon frequency switches the
    // denominator rule to "366 iff the period END falls in a leap year".
    // -----------------------------------------------------------------------

    #[test]
    fn act365l_period_ending_on_feb29_uses_366() {
        use super::{DayCount, DayCountContext};

        // (2024-02-01, 2024-02-29]: end date Feb 29 is INCLUDED in the ICMA
        // window → denominator 366. (Previously pinned to 365 under the
        // incorrect [start, end) window.)
        let start = date!(2024 - 02 - 01);
        let end = date!(2024 - 02 - 29);
        let yf = DayCount::Act365L
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");

        let days = (end - start).whole_days() as f64;
        assert_eq!(
            yf,
            days / 366.0,
            "denominator should be 366 when end == Feb 29 (included per ICMA Rule 251)"
        );
    }

    #[test]
    fn act365l_period_starting_on_feb29_uses_365() {
        use super::{DayCount, DayCountContext};

        // (2024-02-29, 2024-03-15]: Feb 29 is the start, excluded from the
        // ICMA window → denominator 365. (Previously the [start, end) window
        // wrongly included it.)
        let start = date!(2024 - 02 - 29);
        let end = date!(2024 - 03 - 15);
        let yf = DayCount::Act365L
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");

        let days = (end - start).whole_days() as f64;
        assert_eq!(
            yf,
            days / 365.0,
            "denominator should be 365 when Feb 29 == start (excluded per ICMA Rule 251)"
        );
    }

    #[test]
    fn act365l_period_containing_feb29_uses_366() {
        use super::{DayCount, DayCountContext};

        // (2024-02-01, 2024-03-01]: Feb 29 is strictly inside → denominator 366.
        let start = date!(2024 - 02 - 01);
        let end = date!(2024 - 03 - 01);
        let yf = DayCount::Act365L
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");

        let days = (end - start).whole_days() as f64;
        assert_eq!(
            yf,
            days / 366.0,
            "denominator should be 366 when Feb 29 is in interior"
        );
    }

    #[test]
    fn act365l_non_annual_frequency_uses_end_year_leap_rule() {
        use super::{DayCount, DayCountContext, Tenor};

        // ICMA Rule 251 non-annual rule: 366 iff the period END is in a leap
        // year, regardless of whether Feb 29 is in the period.
        let semi = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            ..Default::default()
        };

        // Period entirely after Feb 29, ending in leap year 2024 → 366.
        let start = date!(2024 - 06 - 01);
        let end = date!(2024 - 12 - 01);
        let yf = DayCount::Act365L
            .year_fraction(start, end, semi)
            .expect("should succeed");
        let days = (end - start).whole_days() as f64;
        assert_eq!(yf, days / 366.0, "semi-annual, end in leap year → 366");

        // Period containing Feb 29 2024 but ending in non-leap 2025... not
        // constructible for a 6M period ending after Dec; instead: period
        // ending in non-leap 2025 → 365 even though it starts in a leap year.
        let start = date!(2024 - 09 - 01);
        let end = date!(2025 - 03 - 01);
        let yf = DayCount::Act365L
            .year_fraction(start, end, semi)
            .expect("should succeed");
        let days = (end - start).whole_days() as f64;
        assert_eq!(yf, days / 365.0, "semi-annual, end in non-leap year → 365");

        // Annual frequency keeps the Feb-29 (start, end] rule: same dates,
        // annual context, no Feb 29 in (start, end] → 365.
        let annual = DayCountContext {
            frequency: Some(Tenor::annual()),
            ..Default::default()
        };
        let start = date!(2024 - 06 - 01);
        let end = date!(2024 - 12 - 01);
        let yf = DayCount::Act365L
            .year_fraction(start, end, annual)
            .expect("should succeed");
        let days = (end - start).whole_days() as f64;
        assert_eq!(yf, days / 365.0, "annual, no Feb 29 in (start,end] → 365");
    }

    // -----------------------------------------------------------------------
    // 30E/360 (ISDA) — ISDA 2006 §4.16(h) examples
    // -----------------------------------------------------------------------

    #[test]
    fn thirty_e_360_isda_last_day_of_month_rules() {
        use super::{days_30e_360_isda, DayCount, DayCountContext};

        // Aug 31 → 30 (last day of month), Feb 29 → 30 (last day of Feb,
        // not termination): 360 + 30·(2-8) + (30-30) = 180.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        assert_eq!(days_30e_360_isda(start, end, false), 180);

        // Same period as the final period to maturity: Feb 29 kept → 179.
        assert_eq!(days_30e_360_isda(start, end, true), 179);

        // Feb 29 2012 → 30 (last day of Feb as D1), Aug 31 → 30:
        // 30·(8-2) + (30-30) = 180.
        let start = date!(2012 - 02 - 29);
        let end = date!(2012 - 08 - 31);
        assert_eq!(days_30e_360_isda(start, end, false), 180);

        // Non-leap end-of-Feb: Jan 28 (not last day) kept, Feb 28 → 30
        // (intermediate): 30 + (30-28) = 32; termination: 30 + (28-28) = 30.
        let start = date!(2011 - 01 - 28);
        let end = date!(2011 - 02 - 28);
        assert_eq!(days_30e_360_isda(start, end, false), 32);
        assert_eq!(days_30e_360_isda(start, end, true), 30);

        // The enum variant routes through the non-termination form.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        let yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 180.0 / 360.0);
    }

    #[test]
    fn thirty_e_360_isda_differs_from_european_on_end_of_february() {
        use super::{days_30_360, days_30e_360_isda, Thirty360Convention};

        // 30E/360 (European, §4.16(g)) has NO end-of-February rule;
        // 30E/360 (ISDA, §4.16(h)) adjusts end-of-Feb to 30.
        let start = date!(2012 - 02 - 29);
        let end = date!(2012 - 03 - 31);
        // European: D1=29 kept, D2=31→30: 30 + (30-29) = 31.
        assert_eq!(days_30_360(start, end, Thirty360Convention::European), 31);
        // ISDA: D1=29→30 (last day of Feb), D2=31→30: 30 + (30-30) = 30.
        assert_eq!(days_30e_360_isda(start, end, false), 30);
    }

    // -----------------------------------------------------------------------
    // NL/365
    // -----------------------------------------------------------------------

    #[test]
    fn nl365_excludes_feb_29() {
        use super::{DayCount, DayCountContext};

        // Full leap year: 366 actual days − 1 leap day = 365 → exactly 1.0.
        let start = date!(2024 - 01 - 01);
        let end = date!(2025 - 01 - 01);
        let yf = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 1.0);

        // Feb 28 → Mar 1 in a leap year: 2 actual days − Feb 29 = 1 day.
        let start = date!(2024 - 02 - 28);
        let end = date!(2024 - 03 - 01);
        let yf = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 1.0 / 365.0);

        // Non-leap year: identical to Act/365F.
        let start = date!(2025 - 01 - 01);
        let end = date!(2025 - 07 - 01);
        let nl = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        let act365f = DayCount::Act365F
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(nl, act365f);
    }

    // -----------------------------------------------------------------------
    // DayCountContextState coupon_period round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn daycount_context_state_roundtrips_coupon_period() {
        use super::{DayCountContext, DayCountContextState};
        use crate::dates::Tenor;

        let coupon = (date!(2025 - 01 - 15), date!(2025 - 07 - 15));
        let ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some(coupon),
            ..Default::default()
        };

        // Context → state → JSON → state → context preserves coupon_period
        // (previously silently dropped; ).
        let state: DayCountContextState = ctx.into();
        assert_eq!(state.coupon_period, Some(coupon));

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: DayCountContextState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.coupon_period, Some(coupon));

        let registry = crate::dates::CalendarRegistry::global();
        let restored_ctx = restored.to_ctx(registry);
        assert_eq!(restored_ctx.coupon_period, Some(coupon));

        // Serde-additive: payloads written before the field existed still
        // deserialize (coupon_period defaults to None).
        let legacy = r#"{"calendar_id":null,"frequency":null,"bus_basis":null}"#;
        let legacy_state: DayCountContextState =
            serde_json::from_str(legacy).expect("legacy payload deserializes");
        assert_eq!(legacy_state.coupon_period, None);
    }

    // -----------------------------------------------------------------------
    // Act/Act ISMA coupon_period routing tests
    // -----------------------------------------------------------------------

    #[test]
    fn act_act_isma_coupon_period_mid_coupon_accrual() {
        use super::{DayCount, DayCountContext, Tenor};

        let coupon_start = date!(2025 - 01 - 15);
        let coupon_end = date!(2025 - 07 - 15);

        // Mid-coupon accrual: settlement to next coupon
        let settlement = date!(2025 - 03 - 15);
        let freq = Tenor::semi_annual();

        // With coupon_period: uses the explicit reference period
        let ctx_with = DayCountContext {
            frequency: Some(freq),
            coupon_period: Some((coupon_start, coupon_end)),
            ..Default::default()
        };
        let yf_with = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_with)
            .expect("should succeed with coupon_period");

        // Without coupon_period: re-anchors from settlement
        let ctx_without = DayCountContext {
            frequency: Some(freq),
            ..Default::default()
        };
        let yf_without = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_without)
            .expect("should succeed without coupon_period");

        // With reference period: 122 days / 181 days × 0.5 ≈ 0.33702
        let expected_days = (coupon_end - settlement).whole_days() as f64;
        let ref_days = (coupon_end - coupon_start).whole_days() as f64;
        let expected = (expected_days / ref_days) * 0.5;
        assert!(
            (yf_with - expected).abs() < 1e-10,
            "coupon_period path: {yf_with} vs expected {expected}"
        );

        // The reference-period path should match calling the function directly
        let yf_direct = act_act_isma_year_fraction_with_reference_period(
            settlement,
            coupon_end,
            coupon_start,
            coupon_end,
        )
        .expect("direct call should succeed");
        assert!(
            (yf_with - yf_direct).abs() < 1e-14,
            "coupon_period routing should match direct call: {yf_with} vs {yf_direct}"
        );

        // The two paths may diverge for mid-coupon dates because the
        // re-anchor path infers a different reference period.
        // We just assert the reference-period path gives the expected result.
        let _ = yf_without;
    }
}
