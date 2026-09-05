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
//! - [`DayCount::ActActAfb`] - Actual/Actual AFB (Actual/Actual Euro)
//! - [`DayCount::Thirty360It`] - 30/360 Italian
//! - [`DayCount::Bus252`] - Business/252 (Brazilian and some equity markets)
//!
//! # Examples
//! ```
//! use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
//! # References
//!
//! - ISDA 2006 Definitions: `docs/REFERENCES.md#isda-2006-definitions`
//! - ICMA Rule Book: `docs/REFERENCES.md#icma-rule-book`
//!
//! # Bus/252 Convention
//!
//! The Bus/252 convention counts business days between dates and divides by 252 (typical trading days per year).
//! This requires a holiday calendar to determine business days. Provide the calendar via `DayCountContext`.
//!
//! ```
//! use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
//! use finstack_quant_core::dates::calendar::TARGET2;
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
//! let calendar = TARGET2;
//!
//! // Calculate year fraction with a calendar in context
//! let yf = DayCount::Bus252
//!     .year_fraction(start, end, DayCountContext { calendar: Some(&calendar), ..DayCountContext::default() })
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
//! use finstack_quant_core::dates::{Date, DayCount, Tenor, DayCountContext};
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
//! let frequency = Tenor::semi_annual(); // Semi-annual
//! let yf_isma = DayCount::ActActIsma
//!     .year_fraction(start, end, DayCountContext { frequency: Some(frequency), ..DayCountContext::default() })
//!     .expect("Year fraction calculation should succeed");
//! // yf_isma ≈ 0.5 (one full semi-annual period in years)
//! ```

#![allow(clippy::many_single_char_names)]

mod act_act;
mod context;
mod other;
mod thirty360;

pub use act_act::act_act_isma_year_fraction_with_reference_period;
pub use context::{DayCountContext, DayCountContextState};
pub use thirty360::{days_30_360, days_30e_360_isda, Thirty360Convention};

use time::Date;

use crate::error::InputError;
use act_act::{
    year_fraction_act_act_afb, year_fraction_act_act_isda, year_fraction_act_act_isma_with_ctx,
};
use other::{year_fraction_act_365l, year_fraction_bus252, year_fraction_nl_365};

#[cfg(test)]
use crate::dates::Tenor;

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
/// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::April, 1).expect("Valid date"); // 90 days
    ///
    /// let yf = DayCount::Act360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 90.0 / 360.0);
    /// ```
    #[serde(rename = "act_360")]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Act365F.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert!((yf - 1.0).abs() < 1e-9); // 365 days / 365 = 1.0
    /// ```
    #[serde(rename = "act_365f")]
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
    /// conventions. Use [`DayCount::ActActAfb`] for AFB / Actual/Actual Euro.
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
    #[serde(rename = "act_365l")]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::February, 28).expect("Valid date");
    ///
    /// let yf = DayCount::Thirty360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // Treats Jan 31 as day 30, Feb 28 as day 28: 28 days / 360
    /// assert_eq!(yf, 28.0 / 360.0);
    /// ```
    #[serde(rename = "30_360")]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::March, 31).expect("Valid date");
    ///
    /// let yf = DayCount::ThirtyE360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // Treats both 31st as day 30: 60 days / 360
    /// assert_eq!(yf, 60.0 / 360.0);
    /// ```
    #[serde(rename = "30e_360")]
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
    /// Set [`DayCountContext::end_is_termination_date`] for the final period
    /// to maturity; ordinary coupon periods leave it false.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // ISDA §4.16(h): both end-of-Feb and Aug 31 count as day 30.
    /// let start = Date::from_calendar_date(2011, Month::August, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2012, Month::February, 29).expect("Valid date");
    ///
    /// let yf = DayCount::ThirtyE360Isda.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 180.0 / 360.0);
    /// ```
    #[serde(rename = "30e_360_isda")]
    ThirtyE360Isda,

    /// 30/360 Italian day count convention.
    ///
    /// Assumes 30 days per month and 360 days per year. Day 31 becomes 30,
    /// and any February day after the 27th becomes 30 (QuantLib
    /// `Thirty360::Italian`).
    ///
    /// # Formula
    ///
    /// ```text
    /// D1' = 30 if D1 == 31 or (month == Feb and D1 > 27)
    /// D2' = 30 if D2 == 31 or (month == Feb and D2 > 27)
    /// days = 360*(Y2-Y1) + 30*(M2-M1) + (D2'-D1')
    /// year_fraction = days / 360
    /// ```
    ///
    /// Distinct from US SIA (February EOM only when both ends are February
    /// EOM) and 30E/360 (no February-after-27 rule).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::February, 28).expect("Valid date");
    ///
    /// let yf = DayCount::Thirty360It.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // D1=31→30, Feb 28>27 → D2=30: 30 days / 360
    /// assert_eq!(yf, 30.0 / 360.0);
    /// ```
    #[serde(rename = "30_360_it")]
    Thirty360It,

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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// // Full leap year 2024: 366 actual days, Feb 29 excluded → 365/365 = 1.0
    /// let start = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Nl365.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// assert_eq!(yf, 1.0);
    /// ```
    #[serde(rename = "nl_365")]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
    /// - ISDA (2006). "2006 ISDA Definitions." Section 4.16(b). `docs/REFERENCES.md#isda-2006-definitions`
    #[serde(rename = "act_act")]
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 15).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::July, 15).expect("Valid date");
    /// let frequency = Tenor::semi_annual(); // Semi-annual
    ///
    /// let yf = DayCount::ActActIsma.year_fraction(
    ///     start,
    ///     end,
    ///     DayCountContext { frequency: Some(frequency), ..Default::default() }
    /// ).expect("Year fraction calculation should succeed");
    ///
    /// // Full semi-annual period = 0.5 year fraction (6 months / 12 months)
    /// assert!((yf - 0.5).abs() < 1e-6);
    /// ```
    ///
    /// # References
    ///
    /// - ICMA (2010). "ICMA Rule Book." Rule 251. `docs/REFERENCES.md#icma-rule-book`
    /// - ISMA (1999). "Recommendations for Accrued Interest Calculations."
    #[serde(rename = "act_act_isma")]
    ActActIsma,

    /// Actual/Actual AFB (Association Française des Banques) day count.
    ///
    /// Also known as Actual/Actual Euro. QuantLib `ActualActual::AFB`.
    /// Walks whole years **backwards from `end`** until the candidate is
    /// before `start`. Each accepted year-step adds `1.0`. A year-step that
    /// lands on 28 February of a leap year is bumped to 29 February. The
    /// residual fraction is `days(start, residual_end) / den`, where `den`
    /// is 366 if 29 February lies in `[start, residual_end)`, else 365.
    ///
    /// No [`DayCountContext`] is required.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2024, Month::February, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2024, Month::March, 1).expect("Valid date");
    ///
    /// let yf = DayCount::ActActAfb.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
    /// // 29 days / 366 (29 February lies in the residual period)
    /// assert_eq!(yf, 29.0 / 366.0);
    /// ```
    #[serde(rename = "act_act_afb")]
    ActActAfb,

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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use finstack_quant_core::dates::calendar::NYSE;
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
    #[serde(rename = "bus_252")]
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
    /// - [`InputError::MissingCouponPeriodForActActIsma`](crate::error::InputError::MissingCouponPeriodForActActIsma):
    ///   Using `ActActIsma` on an irregular coupon without `ctx.coupon_period`
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::error::InputError::ActActIsmaUnsupportedFrequency):
    ///   Using `ActActIsma` with a Day or Week frequency
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
    ///
    /// let yf = DayCount::Act360.year_fraction(start, end, DayCountContext::default())?;
    /// assert!(yf > 0.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
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
            DayCount::ThirtyE360Isda => {
                Ok(f64::from(days_30e_360_isda(start, end, ctx.end_is_termination_date)) / 360.0)
            }
            DayCount::Thirty360It => {
                Ok(days_30_360(start, end, Thirty360Convention::Italian) as f64 / 360.0)
            }
            DayCount::Nl365 => Ok(year_fraction_nl_365(start, end)),
            DayCount::ActAct => year_fraction_act_act_isda(start, end),
            DayCount::ActActIsma => year_fraction_act_act_isma_with_ctx(start, end, ctx),
            DayCount::ActActAfb => Ok(year_fraction_act_act_afb(start, end)),
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
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
    /// # Ok::<(), finstack_quant_core::Error>(())
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
    /// use finstack_quant_core::dates::{Date, DayCount};
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

    /// Canonical snake_case names accepted by [`FromStr`](std::str::FromStr),
    /// in declaration order.
    pub const NAMES: &'static [&'static str] = &[
        "act_360",
        "act_365f",
        "act_365l",
        "nl_365",
        "30_360",
        "30e_360",
        "30e_360_isda",
        "30_360_it",
        "act_act",
        "act_act_isma",
        "act_act_afb",
        "bus_252",
    ];

    /// Leniently parse a day-count name as written on term sheets.
    ///
    /// Unlike the strict [`FromStr`](std::str::FromStr) implementation, this
    /// is ASCII case-insensitive, treats `/`, `-` and spaces as `_`, collapses
    /// repeated separators, and accepts the common market spellings
    /// `ACT/ACT ICMA` (→ `act_act_isma`), `ACT/ACT ISDA` (→ `act_act`),
    /// `ACT/365` (→ `act_365f`), `30/360` (→ `30_360`) and `30E/360`.
    ///
    /// # Arguments
    ///
    /// * `s` - Day-count label such as `"ACT/360"`, `"Act/Act ICMA"`,
    ///   `"30E/360 ISDA"` or any canonical snake_case name from [`Self::NAMES`].
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the input and listing the accepted
    /// canonical names when no spelling matches.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::DayCount;
    ///
    /// assert_eq!(DayCount::parse("ACT/360")?, DayCount::Act360);
    /// assert_eq!(DayCount::parse("Act/Act ICMA")?, DayCount::ActActIsma);
    /// assert_eq!(DayCount::parse("30E/360 ISDA")?, DayCount::ThirtyE360Isda);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn parse(s: &str) -> crate::Result<Self> {
        let mut normalized = String::with_capacity(s.len());
        let mut last_sep = true;
        for ch in s.trim().chars() {
            if ch == '/' || ch == '-' || ch == '_' || ch.is_whitespace() {
                if !last_sep {
                    normalized.push('_');
                }
                last_sep = true;
            } else {
                normalized.push(ch.to_ascii_lowercase());
                last_sep = false;
            }
        }
        let normalized = normalized.trim_end_matches('_');
        let canonical = match normalized {
            "act_act_icma" | "act_act_isma" => "act_act_isma",
            "act_act_isda" | "act_act" | "actual_actual" => "act_act",
            "act_365" | "act_365_fixed" | "act_365f" | "actual_365" => "act_365f",
            "act_360" => "act_360",
            "30_360_us" | "30_360_bond" | "30u_360" | "30_360" => "30_360",
            "30e_360_isda" | "30_360_isda" => "30e_360_isda",
            "30e_360" | "30_360_eurobond" => "30e_360",
            "30_360_it" | "30_360_italian" => "30_360_it",
            other => other,
        };
        canonical.parse::<Self>().map_err(crate::Error::Validation)
    }
}

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
            DayCount::Thirty360It => "30_360_it",
            DayCount::ActAct => "act_act",
            DayCount::ActActIsma => "act_act_isma",
            DayCount::ActActAfb => "act_act_afb",
            DayCount::Bus252 => "bus_252",
        };
        f.write_str(label)
    }
}

impl std::str::FromStr for DayCount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "act_360" => Ok(Self::Act360),
            "act_365f" => Ok(Self::Act365F),
            "act_365l" => Ok(Self::Act365L),
            "nl_365" => Ok(Self::Nl365),
            "30_360" => Ok(Self::Thirty360),
            "30e_360" => Ok(Self::ThirtyE360),
            "30e_360_isda" => Ok(Self::ThirtyE360Isda),
            "30_360_it" => Ok(Self::Thirty360It),
            "act_act" => Ok(Self::ActAct),
            "act_act_isma" => Ok(Self::ActActIsma),
            "act_act_afb" => Ok(Self::ActActAfb),
            "bus_252" => Ok(Self::Bus252),
            other => Err(format!(
                "unknown day-count convention: '{other}'; expected one of {}",
                Self::NAMES.join(", ")
            )),
        }
    }
}

#[cfg(test)]
mod parse_tests {
    use super::DayCount;

    #[test]
    fn lenient_parse_accepts_term_sheet_spellings() {
        let cases = [
            ("ACT/360", DayCount::Act360),
            ("act_360", DayCount::Act360),
            ("Act/365", DayCount::Act365F),
            ("ACT/365F", DayCount::Act365F),
            ("ACT/ACT ICMA", DayCount::ActActIsma),
            ("act/act isma", DayCount::ActActIsma),
            ("ACT/ACT ISDA", DayCount::ActAct),
            ("Act/Act", DayCount::ActAct),
            ("ACT/ACT AFB", DayCount::ActActAfb),
            ("30/360", DayCount::Thirty360),
            ("30E/360", DayCount::ThirtyE360),
            ("30E/360 ISDA", DayCount::ThirtyE360Isda),
            ("30/360 IT", DayCount::Thirty360It),
            ("NL/365", DayCount::Nl365),
            ("BUS/252", DayCount::Bus252),
            (" bus-252 ", DayCount::Bus252),
        ];
        for (input, expected) in cases {
            assert_eq!(DayCount::parse(input).expect(input), expected, "{input}");
        }
    }

    #[test]
    fn strict_from_str_rejects_lenient_spellings_and_lists_names() {
        let err = "ACT/360".parse::<DayCount>().expect_err("strict");
        assert!(err.contains("'ACT/360'") && err.contains("act_360") && err.contains("bus_252"));
        let err = DayCount::parse("nope").expect_err("unknown");
        assert!(err.to_string().contains("act_act_isma"));
        for name in DayCount::NAMES {
            assert_eq!(&name.parse::<DayCount>().expect(name).to_string(), name);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::act_act_isma_year_fraction_with_reference_period;
    use time::macros::date;

    /// Per-year and bulk additions differ for a representative ISDA seed.
    #[test]
    fn act_act_isda_per_year_loop_is_not_bulk_addable() {
        // Seed as produced by the first term: 3 days into a 365-day year.
        let seed = 3.0_f64 / 365.0;

        let mut looped = seed;
        looped += 1.0;
        looped += 1.0;

        let bulk = seed + 2.0;

        assert_ne!(
            looped.to_bits(),
            bulk.to_bits(),
            "per-year accumulation must preserve its distinct rounding"
        );
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

    #[test]
    fn act_act_isma_reference_period_preserves_eom_anchor_for_full_year() {
        let fraction = act_act_isma_year_fraction_with_reference_period(
            date!(2024 - 02 - 29),
            date!(2025 - 02 - 28),
            date!(2024 - 08 - 31),
            date!(2025 - 02 - 28),
        )
        .expect("EOM reference traversal should succeed");

        assert_eq!(fraction, 1.0);
    }

    #[test]
    fn act_act_isma_reference_period_preserves_eom_anchor_across_stubs() {
        let front_stub = act_act_isma_year_fraction_with_reference_period(
            date!(2024 - 04 - 15),
            date!(2024 - 08 - 31),
            date!(2024 - 08 - 31),
            date!(2025 - 02 - 28),
        )
        .expect("front stub should succeed");
        let back_stub = act_act_isma_year_fraction_with_reference_period(
            date!(2025 - 02 - 28),
            date!(2025 - 05 - 15),
            date!(2024 - 08 - 31),
            date!(2025 - 02 - 28),
        )
        .expect("back stub should succeed");

        let expected_front = (date!(2024 - 08 - 31) - date!(2024 - 04 - 15)).whole_days() as f64
            / (date!(2024 - 08 - 31) - date!(2024 - 02 - 29)).whole_days() as f64
            * 0.5;
        let expected_back = (date!(2025 - 05 - 15) - date!(2025 - 02 - 28)).whole_days() as f64
            / (date!(2025 - 08 - 31) - date!(2025 - 02 - 28)).whole_days() as f64
            * 0.5;

        assert!((front_stub - expected_front).abs() < 1e-14);
        assert!((back_stub - expected_back).abs() < 1e-14);
    }

    // FromStr / Display roundtrip tests

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
            super::DayCount::Thirty360It,
            super::DayCount::ActAct,
            super::DayCount::ActActIsma,
            super::DayCount::ActActAfb,
            super::DayCount::Bus252,
        ];

        for day_count in &all {
            let label = day_count.to_string();
            assert!(
                matches!(label.parse::<super::DayCount>(), Ok(value) if value == *day_count),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn daycount_from_str_rejects_noncanonical_spellings() {
        // schema-rejection-test
        use super::DayCount;

        for rejected in [
            "act360",
            "actual_360",
            "ACT/360",
            "act365f",
            "actual_365l",
            "NL/365",
            "act_365_nl",
            "30/360",
            "thirty360",
            "bond_basis",
            "30E/360",
            "eurobond_basis",
            "30E/360 ISDA",
            "act_365afb",
            "30/360 IT",
            "30_360_italian",
            "act/act ISDA",
            "isda",
            "act_act_icma",
            "icma",
            "bus252",
            "business_252",
        ] {
            assert!(rejected.parse::<DayCount>().is_err());
        }
    }

    #[test]
    fn daycount_from_str_unknown() {
        assert!("garbage".parse::<super::DayCount>().is_err());
    }

    // Act/365L ICMA Rule 251 boundary tests
    //
    // Updated the
    // Feb-29 window for the annual rule is (start, end] per ICMA Rule 251,
    // not [start, end), and a non-annual coupon frequency switches the
    // denominator rule to "366 iff the period END falls in a leap year".

    #[test]
    fn act365l_period_ending_on_feb29_uses_366() {
        use super::{DayCount, DayCountContext};

        // (2024-02-01, 2024-02-29]: end date Feb 29 is included → denom 366.
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

        // (2024-02-29, 2024-03-15]: Feb 29 is the start, excluded → denom 365.
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

    // 30E/360 (ISDA) — ISDA 2006 §4.16(h) examples

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

        // The enum variant routes through context for the termination form.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        let yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 180.0 / 360.0);

        let terminal_yf = DayCount::ThirtyE360Isda
            .year_fraction(
                start,
                end,
                DayCountContext {
                    end_is_termination_date: true,
                    ..DayCountContext::default()
                },
            )
            .expect("termination-period day count should succeed");
        assert_eq!(terminal_yf, 179.0 / 360.0);
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

    #[test]
    fn thirty_360_italian_feb_after_27_and_day_31() {
        use super::{days_30_360, Thirty360Convention};

        // Jan 31 → Feb 28 2025: Italian D1=30, D2=30 → 30.
        // European D2 stays 28; US SIA D2 stays 28 (not both Feb EOM).
        let start = date!(2025 - 01 - 31);
        let end = date!(2025 - 02 - 28);
        assert_eq!(days_30_360(start, end, Thirty360Convention::Italian), 30);
        assert_eq!(days_30_360(start, end, Thirty360Convention::European), 28);
        assert_eq!(days_30_360(start, end, Thirty360Convention::UsSia), 28);

        // Leap: Feb 29 → Mar 31 2024: Italian D1=30 (Feb>27), D2=30 → 30.
        let start = date!(2024 - 02 - 29);
        let end = date!(2024 - 03 - 31);
        assert_eq!(days_30_360(start, end, Thirty360Convention::Italian), 30);
    }

    // NL/365

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

    // DayCountContextState coupon_period round-trip

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

        // Context → state → JSON → state → context preserves coupon_period.
        let state: DayCountContextState = ctx.into();
        assert_eq!(state.coupon_period, Some(coupon));

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: DayCountContextState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.coupon_period, Some(coupon));

        let restored_ctx = restored.to_ctx().expect("calendar state hydrates");
        assert_eq!(restored_ctx.coupon_period, Some(coupon));

        let missing_required =
            r#"{"calendar_id":null,"frequency":null,"bus_basis":null,"coupon_period":null}"#;
        assert!(
            serde_json::from_str::<DayCountContextState>(missing_required).is_err(),
            "the canonical state requires end_is_termination_date"
        );
    }

    #[test]
    fn daycount_context_state_rejects_unknown_calendar() {
        let state = super::DayCountContextState {
            calendar_id: Some("not_a_calendar".to_string()),
            frequency: None,
            bus_basis: None,
            coupon_period: None,
            end_is_termination_date: false,
        };

        let error = state
            .to_ctx()
            .expect_err("unknown persisted calendar must fail hydration");
        assert!(error.to_string().contains("not_a_calendar"));
    }

    // Act/Act ISMA coupon_period routing tests

    #[test]
    fn act_act_isma_coupon_period_mid_coupon_accrual() {
        use super::{DayCount, DayCountContext, Tenor};

        let coupon_start = date!(2025 - 01 - 15);
        let coupon_end = date!(2025 - 07 - 15);

        // Mid-coupon accrual: settlement to next coupon
        let settlement = date!(2025 - 03 - 15);
        let frequency = Tenor::semi_annual();

        // With coupon_period: uses the explicit reference period
        let ctx_with = DayCountContext {
            frequency: Some(frequency),
            coupon_period: Some((coupon_start, coupon_end)),
            ..Default::default()
        };
        let yf_with = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_with)
            .expect("should succeed with coupon_period");

        // Without coupon_period an irregular span must error rather than
        // re-anchoring a quasi-coupon grid on settlement.
        let ctx_without = DayCountContext {
            frequency: Some(frequency),
            ..Default::default()
        };
        let err = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_without)
            .expect_err("irregular Act/Act ICMA without coupon_period must fail");
        assert!(
            err.to_string().contains("coupon_period"),
            "unexpected error: {err}"
        );

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
    }
}
