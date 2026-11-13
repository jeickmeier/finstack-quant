//! Day-count convention algorithms for fixed income and derivative accrual calculations.
//!
//! This module implements industry-standard day count conventions as defined by
//! ISDA (International Swaps and Derivatives Association) and ICMA (International
//! Capital Market Association). All implementations are panic-free and avoid heap
//! allocation.
//!
//! # Date Interval Convention
//!
//! Endpoint treatment follows each named convention. Actual elapsed-day counts
//! equal `end - start`, while ACT/ACT ISDA and business-day counts allocate days
//! over `[start, end)`. NL/365 removes February 29 in `(start, end]`, and
//! ACT/365L takes its 365/366 denominator from the enclosing period's end year.
//! A date equal to itself always has zero year fraction.
//!
//! # Industry Standards
//!
//! Day count conventions define how interest accrues between two dates. Different
//! markets and instruments use different conventions:
//!
//! # Precision
//!
//! Day and complete-year counts are exact integers; the year fraction is one
//! `f64` conversion of them (ACT/ACT ISDA adds its complete years to at most
//! two partial-year fractions). The whole supported `Date` range is accepted;
//! no bitwise agreement with a vendor or a particular summation order is
//! promised.
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
//! - [`DayCount::Act365L`] - Actual/365 Leap (ISDA coupon-end-year rule)
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
//! use finstack_quant_core::dates::{calendar_by_id_strict, Date, DayCount, DayCountContext};
//! use time::Month;
//!
//! let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let end   = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
//! let calendar = calendar_by_id_strict("target2").expect("built-in calendar");
//!
//! // Calculate year fraction with a calendar in context
//! let yf = DayCount::Bus252
//!     .year_fraction(start, end, DayCountContext { calendar: Some(calendar), ..DayCountContext::default() })
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

mod act_act;
mod context;
mod other;
mod thirty360;

pub use context::{DayCountContext, DayCountContextState};

use time::Date;

use crate::InputError;
use act_act::{
    validate_act_act_isma_context, year_fraction_act_act_afb, year_fraction_act_act_isda,
    year_fraction_act_act_isma_with_ctx,
};
use other::{
    validate_act_365l_context, validate_act_365l_interval, year_fraction_act_365l,
    year_fraction_bus252, year_fraction_nl_365,
};
use thirty360::{Thirty360Variant, thirty_360_days, thirty_e_360_isda_days};

#[cfg(test)]
use crate::dates::Tenor;

/// Supported day-count conventions with industry-standard definitions.
///
/// Each variant implements a specific day count convention as defined by
/// ISDA, ICMA, or local market conventions. The conventions determine how
/// interest accrues between payment dates.
///
/// The variant docs are deliberately short: they are published verbatim as the
/// `description` of each wire value in the `common/1/day_count` JSON Schema,
/// which every instrument contract inlines. Each one carries the convention
/// name, its defining rule and its standards citation. Formulas, market usage
/// and worked examples live in the sections below.
///
/// # Standards References
///
/// Implementations follow:
/// - **ISDA**: 2006 ISDA Definitions, Section 4.16.
///   `docs/REFERENCES.md#isda-2006-definitions`
/// - **ICMA**: ICMA Rule Book, Rule 251. `docs/REFERENCES.md#icma-rule-book`
/// - **ISMA** (1999). "Recommendations for Accrued Interest Calculations."
/// - **SIA/PSA**: Standard Securities Calculation Methods (SIA Standard Formulas)
/// - **ISO**: ISO 20022 Day Count Fraction Codes
///
/// # Formulas
///
/// The 30/360 family shares one day-count formula and differs only in how the
/// day-of-month values are adjusted:
///
/// ```text
/// Days = 360(Y₂ - Y₁) + 30(M₂ - M₁) + (D₂' - D₁')
/// year_fraction = Days / 360
///
/// Thirty360 (fixed US 30U/360 EOM):
///   D₁' = 30   if D₁ is 31 or last day of February
///   D₂' = 30   if D₂ is 31 and D₁' = 30
///   D₂' = 30   if D₂ is last day of Feb and D₁ is last day of Feb
///
/// Thirty360Isda (ISDA bond basis, §4.16(f)):
///   D₁' = min(D₁, 30)
///   D₂' = 30   if D₂ is 31 and D₁' = 30
///
/// ThirtyE360 (ISDA §4.16(g)):
///   D₁' = min(D₁, 30)
///   D₂' = min(D₂, 30)
///
/// ThirtyE360Isda (ISDA §4.16(h)):
///   D₁' = 30   if D₁ is the last day of its month (incl. end of February)
///   D₂' = 30   if D₂ is 31, or if D₂ is the last day of February and the
///              period does not end on the termination (maturity) date
///
/// Thirty360It (QuantLib `Thirty360::Italian`):
///   D₁' = 30   if D₁ is 31 or (month is February and D₁ > 27)
///   D₂' = 30   if D₂ is 31 or (month is February and D₂ > 27)
/// ```
///
/// - **`Thirty360` versus `Thirty360Isda`**: `Thirty360` is the fixed US
///   30U/360 EOM algorithm with its paired February month-end rule (QuantLib
///   `Thirty360::USA`); it is not `30/360 PSA`, which lacks the paired end
///   adjustment. `Thirty360Isda` is ISDA 2006 §4.16(f) bond basis without any
///   February rule (QuantLib `Thirty360::BondBasis`). They differ for periods
///   that start or end on the last day of February.
/// - **`ThirtyE360Isda` termination-date exception**: ISDA §4.16(h) keeps D₂
///   unadjusted when the period ends on the termination date and that date is
///   the last day of February. [`DayCountContext::end_is_termination_date`]
///   must be explicit for this convention: `Some(true)` when the
///   chronologically later endpoint is the contractual termination date,
///   `Some(false)` otherwise. QuantLib's `Thirty360::ISDA` is this convention.
/// - **`Thirty360It`** is distinct from US 30U/360 EOM (February EOM only when both
///   ends are February EOM) and from 30E/360 (no February-after-27 rule).
///
/// The actual-day conventions:
///
/// - **`Act365L`** (ISDA, A009): requires the enclosing calculation period
///   `coupon_period` in [`DayCountContext`]; no frequency is needed. The
///   denominator is 366 if that period's end date is in a leap year, else
///   365, for every coupon frequency. Partial accrual keeps the enclosing
///   coupon's denominator.
///   Accrual dates outside that coupon are rejected; sum separate coupon
///   slices for calculations spanning multiple coupon periods. This is **not**
///   ACT/ACT AFB, which uses a sub-period splitting algorithm; use
///   [`DayCount::ActActAfb`] for AFB / Actual/Actual Euro.
/// - **`Nl365`**: counts the actual calendar days in `(start, end]` and removes
///   every February 29 that falls in the period, so a full leap year still
///   yields exactly 1.0.
/// - **`ActAct`** (ISDA): split the period at calendar-year boundaries, take
///   (days in segment) / (days in that year) for each segment, and sum.
/// - **`ActActIsma`** (ICMA): determine quasi-coupon periods from the payment
///   frequency, take (actual days) / (actual days in coupon period) for each,
///   and sum. Requires `frequency` in [`DayCountContext`]; for irregular
///   first/last coupons also supply the enclosing regular `coupon_period`.
/// - **`ActActAfb`** (QuantLib `ActualActual::AFB`): walk whole years
///   **backwards from `end`** until the candidate is before `start`; each
///   accepted year-step adds `1.0`. A year-step that lands on 28 February of a
///   leap year is bumped to 29 February. The residual fraction is
///   `days(start, residual_end) / den`, where `den` is 366 if 29 February lies
///   in `[start, residual_end)`, else 365. No [`DayCountContext`] is required.
/// - **`Bus252`**: requires `calendar` in [`DayCountContext`] and counts its
///   business days in `[start, end)` through
///   [`HolidayCalendar::count_business_days`](crate::dates::HolidayCalendar::count_business_days).
///   Built-in calendars answer from a cached per-year prefix count; custom
///   calendars scan each calendar day unless they override the count. A count
///   outside `0..=elapsed days` is an error.
/// - **`OneOne`**: inflation fixed legs compound across annual periods using
///   this convention. Empty intervals return zero and reversed intervals are
///   rejected, as for all day counts in this API.
///
/// # Usage
///
/// | Convention | Standard for |
/// | --- | --- |
/// | `Act360` | USD and EUR money markets, short-term rate derivatives (SOFR, €STR), FX swaps and forwards |
/// | `Act365F` | GBP money markets (SONIA), cable (GBP/USD) FX, some Commonwealth bond markets |
/// | `Act365L` | GBP floating-rate notes, some European bond markets |
/// | `Thirty360` | US corporate bonds, US municipal bonds, US agency debt |
/// | `ThirtyE360` | Eurobonds, international bonds, some interest rate swaps |
/// | `Nl365` | Some Canadian money-market and mortgage instruments, legacy systems that ignore leap days |
/// | `ActAct` | US Treasury bonds, USD and EUR swap fixed legs, government bonds in many markets |
/// | `ActActIsma` | International bonds with regular coupons, Eurobonds, ICMA-governed securities |
/// | `Bus252` | BRL-denominated instruments (ANBIMA), some equity derivatives and variance swaps |
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{calendar_by_id_strict, Date, DayCount, DayCountContext, Tenor};
/// use time::Month;
///
/// let date = |y, m, d| Date::from_calendar_date(y, m, d).expect("Valid date");
/// let yf = |convention: DayCount, start, end, ctx| {
///     convention.year_fraction(start, end, ctx).expect("Year fraction calculation should succeed")
/// };
/// let none = DayCountContext::default;
///
/// // Actual/360: 90 days / 360
/// let (start, end) = (date(2025, Month::January, 1), date(2025, Month::April, 1));
/// assert_eq!(yf(DayCount::Act360, start, end, none()), 90.0 / 360.0);
///
/// // Actual/365 Fixed: 365 days / 365
/// let (start, end) = (date(2025, Month::January, 1), date(2026, Month::January, 1));
/// assert!((yf(DayCount::Act365F, start, end, none()) - 1.0).abs() < 1e-9);
///
/// // Actual/365L: 29 days over a period containing Feb 29 → 366 denominator
/// let (start, end) = (date(2024, Month::February, 1), date(2024, Month::March, 1));
/// let ctx = DayCountContext {
///     coupon_period: Some((start, end)),
///     ..Default::default()
/// };
/// assert_eq!(yf(DayCount::Act365L, start, end, ctx), 29.0 / 366.0);
/// // Actual/Actual AFB: 29 February lies in the residual period
/// assert_eq!(yf(DayCount::ActActAfb, start, end, none()), 29.0 / 366.0);
///
/// // The 30/360 family differs on month ends: Jan 31 → Feb 28
/// let (start, end) = (date(2025, Month::January, 31), date(2025, Month::February, 28));
/// // US: Jan 31 is day 30, Feb 28 stays 28
/// assert_eq!(yf(DayCount::Thirty360, start, end, none()), 28.0 / 360.0);
/// // Italian: Feb 28 > 27 also becomes day 30
/// assert_eq!(yf(DayCount::Thirty360It, start, end, none()), 30.0 / 360.0);
///
/// // 30E/360: both 31sts count as day 30
/// let (start, end) = (date(2025, Month::January, 31), date(2025, Month::March, 31));
/// assert_eq!(yf(DayCount::ThirtyE360, start, end, none()), 60.0 / 360.0);
///
/// // 30E/360 (ISDA): both end-of-Feb and Aug 31 count as day 30 for an
/// // ordinary (non-termination) end; the termination status is required
/// let (start, end) = (date(2011, Month::August, 31), date(2012, Month::February, 29));
/// let ordinary = DayCountContext { end_is_termination_date: Some(false), ..Default::default() };
/// assert_eq!(yf(DayCount::ThirtyE360Isda, start, end, ordinary), 180.0 / 360.0);
///
/// // NL/365: full leap year, Feb 29 excluded → 365/365
/// let (start, end) = (date(2024, Month::January, 1), date(2025, Month::January, 1));
/// assert_eq!(yf(DayCount::Nl365, start, end, none()), 1.0);
///
/// // Actual/Actual (ISDA): 184/366 (Jul-Dec 2024) + 181/365 (Jan-Jun 2025)
/// let (start, end) = (date(2024, Month::July, 1), date(2025, Month::July, 1));
/// assert!((yf(DayCount::ActAct, start, end, none()) - 1.0).abs() < 0.01);
///
/// // Actual/Actual (ICMA): one full semi-annual period is 0.5
/// let (start, end) = (date(2025, Month::January, 15), date(2025, Month::July, 15));
/// let ctx = DayCountContext { frequency: Some(Tenor::semi_annual()), ..Default::default() };
/// assert!((yf(DayCount::ActActIsma, start, end, ctx) - 0.5).abs() < 1e-6);
///
/// // Business/252: Monday to next Monday is 4 business days, because the
/// // NYSE was closed on January 9, 2025.
/// let (start, end) = (date(2025, Month::January, 6), date(2025, Month::January, 13));
/// let nyse = calendar_by_id_strict("nyse").expect("built-in calendar");
/// let ctx = DayCountContext { calendar: Some(nyse), ..Default::default() };
/// assert!((yf(DayCount::Bus252, start, end, ctx) * 252.0 - 4.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum DayCount {
    /// 1/1 day count convention.
    ///
    /// One unit per contractual accrual period, irrespective of its length.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(a) - "1/1"
    #[serde(rename = "one_one")]
    OneOne,

    /// Actual/360 day count convention.
    ///
    /// Year fraction = (actual days between dates) / 360
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(e)
    /// - **ISO 20022**: Day Count Fraction Code "Actual/360" (A004)
    /// - **Also known as**: Act/360, A/360, French
    #[serde(rename = "act_360")]
    Act360,

    /// Actual/365 Fixed day count convention.
    ///
    /// Year fraction = (actual days between dates) / 365
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(d)
    /// - **ISO 20022**: Day Count Fraction Code "Actual/365 Fixed" (A005)
    /// - **Also known as**: Act/365F, A/365F, English
    #[serde(rename = "act_365f")]
    Act365F,

    /// Actual/365 Leap day count convention (Actual/365L).
    ///
    /// Year fraction = (actual days) / (365 or 366). Requires the enclosing
    /// calculation period (`coupon_period`); no frequency is used. The
    /// denominator is 366 if that period's end date is in a leap year, else
    /// 365, for every coupon frequency. Not Actual/Actual AFB.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions as amended, 2021 ISDA Definitions
    ///   section 4.6.1(ix); ISDA's October 2022 GLEIS response (A009)
    /// - **ISO 15022**: Day Count Fraction Code "Actual/365L" (A009)
    /// - **Also known as**: Act/365L, Actual/365 Leap
    #[serde(rename = "act_365l")]
    Act365L,

    /// Fixed US 30U/360 EOM day count convention.
    ///
    /// 30-day months over a 360-day year; a day-31 or February month-end
    /// start becomes 30, and the end's February month-end becomes 30 only
    /// when the start is also February month-end. Not `30/360 PSA`.
    ///
    /// # Standards Reference
    ///
    /// - **QuantLib**: `Thirty360::USA`
    /// - **Also known as**: 30U/360 EOM
    #[serde(rename = "30_360")]
    Thirty360,

    /// 30/360 ISDA bond basis day count convention.
    ///
    /// 30-day months over a 360-day year; a day-31 start becomes 30, and a
    /// day-31 end becomes 30 when the adjusted start is 30. No February rule.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(f) - "30/360"
    /// - **QuantLib**: `Thirty360::BondBasis` (also `Thirty360::ISMA`)
    /// - **Also known as**: 30/360 bond basis
    #[serde(rename = "30_360_isda")]
    Thirty360Isda,

    /// 30E/360 (Eurobond Basis) day count convention.
    ///
    /// 30-day months over a 360-day year; day 31 becomes 30 at both ends, with
    /// no February adjustment.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(g) - "30E/360"
    /// - **QuantLib**: `Thirty360::European`
    /// - **Also known as**: 30/360 European, Eurobond Basis
    #[serde(rename = "30e_360")]
    ThirtyE360,

    /// 30E/360 (ISDA) day count convention.
    ///
    /// 30-day months over a 360-day year; the last day of any month, including
    /// February, becomes 30, except a February month-end end date that is the
    /// termination date. Requires an explicit `end_is_termination_date`.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(h) - "30E/360 (ISDA)"
    /// - **QuantLib**: `Thirty360::ISDA` (with its termination date)
    /// - **Also known as**: 30E/360 ISDA, German
    #[serde(rename = "30e_360_isda")]
    ThirtyE360Isda,

    /// 30/360 Italian day count convention.
    ///
    /// 30-day months over a 360-day year; day 31 becomes 30, and any February
    /// day after the 27th becomes 30.
    ///
    /// # Standards Reference
    ///
    /// - **QuantLib**: `Thirty360::Italian`
    /// - **Also known as**: 30/360 Italian
    #[serde(rename = "30_360_it")]
    Thirty360It,

    /// NL/365 (Actual/365 No Leap) day count convention.
    ///
    /// Year fraction = (actual days excluding any February 29) / 365
    ///
    /// # Standards Reference
    ///
    /// - **Also known as**: Act/365 No Leap, NL365, Actual/365NL
    #[serde(rename = "nl_365")]
    Nl365,

    /// Actual/Actual (ISDA) day count convention.
    ///
    /// Actual days over the actual days in the containing calendar year,
    /// split and summed across year boundaries.
    ///
    /// # Standards Reference
    ///
    /// - **ISDA**: 2006 ISDA Definitions, Section 4.16(b) - "Actual/Actual (ISDA)"
    /// - **ISO 20022**: Day Count Fraction Code "Actual/Actual ISDA" (A006)
    /// - **Also known as**: Act/Act (ISDA), Actual/Actual, Act/Act
    #[serde(rename = "act_act")]
    ActAct,

    /// Actual/Actual (ICMA) day count convention.
    ///
    /// Actual days over (actual days in the coupon period × coupons per year).
    /// Requires a coupon frequency. Explicit reference coupon boundaries must
    /// be unadjusted regular dates from the contractual nominal month grid,
    /// not payment dates.
    ///
    /// # Standards Reference
    ///
    /// - **ICMA**: ICMA Rule Book, Rule 251 - "Actual/Actual (ICMA)"
    /// - **ISO 20022**: Day Count Fraction Code "Actual/Actual ICMA" (A007)
    /// - **Also known as**: Act/Act (ICMA), Act/Act (ISMA), ISMA-99
    #[serde(rename = "act_act_isma")]
    ActActIsma,

    /// Actual/Actual AFB day count convention.
    ///
    /// Whole years counted backwards from the end date, plus a residual of
    /// actual days over 366 if it contains February 29, else 365.
    ///
    /// # Standards Reference
    ///
    /// - **QuantLib**: `ActualActual::AFB`
    /// - **AFB**: Association Française des Banques master agreement definitions
    /// - **Also known as**: Actual/Actual Euro, Act/Act AFB
    #[serde(rename = "act_act_afb")]
    ActActAfb,

    /// Business/252 day count convention.
    ///
    /// Year fraction = (business days between dates) / 252. Requires a holiday
    /// calendar.
    ///
    /// # Standards Reference
    ///
    /// - **ANBIMA**: standard for BRL-denominated instruments
    /// - **Also known as**: BUS/252, Business/252
    #[serde(rename = "bus_252")]
    Bus252,
}

impl DayCount {
    /// Compute the year fraction between `start` and `end` per this convention.
    ///
    /// Provide any required context via [`DayCountContext`]:
    /// - `Bus/252` requires a holiday calendar
    /// - `Act/Act (ISMA)` requires a coupon frequency
    /// - `Act/365L` requires the enclosing coupon boundaries
    ///
    /// # Arguments
    ///
    /// * `start` - Start date (inclusive)
    /// * `end` - End date (exclusive)
    /// * `ctx` - Optional context providing calendar or frequency as needed
    ///
    /// # Returns
    ///
    /// - `Ok(0.0)` if `start == end`, after `ctx` is validated as in
    ///   [`validate_context`](Self::validate_context)
    /// - `Ok(year_fraction)` for the calculated year fraction (always ≥ 0)
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// - [`InputError::InvalidDateRange`](crate::InputError::InvalidDateRange):
    ///   `start > end` (inverted date range)
    /// - [`InputError::MissingCalendarForBus252`](crate::InputError::MissingCalendarForBus252):
    ///   Using `Bus252` without a calendar in `ctx`
    /// - [`InputError::InvalidBusBasis`](crate::InputError::InvalidBusBasis):
    ///   Using `Bus252` with a zero basis
    /// - [`InputError::MissingFrequencyForActActIsma`](crate::InputError::MissingFrequencyForActActIsma):
    ///   Using `ActActIsma` without a frequency in `ctx`
    /// - [`InputError::MissingCouponPeriodForActActIsma`](crate::InputError::MissingCouponPeriodForActActIsma):
    ///   Using `ActActIsma` on an irregular coupon without `ctx.coupon_period`
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::InputError::ActActIsmaUnsupportedFrequency):
    ///   Using `ActActIsma` with a Day or Week frequency
    /// - `Error::Validation`: using `Act365L` without an enclosing
    ///   `coupon_period`, or supplying accrual dates outside that coupon
    ///   (checked for equal dates too)
    /// - `Error::Validation`: supplying ICMA reference endpoints that do not
    ///   reproduce one unadjusted nominal month grid
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
        if start > end {
            return Err(InputError::InvalidDateRange.into());
        }
        // Structure, then this convention's requirements, then the
        // equal-date zero (CORE-CONVENTIONS-001).
        self.validate_context(ctx)?;
        if start == end {
            if self == DayCount::Act365L {
                validate_act_365l_interval(start, end, ctx)?;
            }
            return Ok(0.0);
        }
        self.year_fraction_impl(start, end, ctx)
    }

    /// Validate `ctx` for this convention without computing a fraction.
    ///
    /// Every supplied field must be structurally valid whatever the
    /// convention: a zero `bus_basis` or a `coupon_period` whose start is not
    /// before its end is rejected. Then the fields this convention needs must
    /// be present; context-free conventions need none, and valid but
    /// irrelevant fields are allowed. [`year_fraction`](Self::year_fraction)
    /// and [`signed_year_fraction`](Self::signed_year_fraction) run this
    /// before any equal-date zero, so empty intervals cannot bypass it.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Calendar, coupon frequency, business-day basis, coupon
    ///   boundaries and termination flag that the convention may use.
    ///
    /// # Errors
    ///
    /// - [`InputError::InvalidBusBasis`](crate::InputError::InvalidBusBasis):
    ///   `bus_basis` is zero
    /// - `Error::Validation`: `coupon_period` is empty or inverted, or an
    ///   ACT/ACT ICMA reference period does not reproduce one nominal month
    ///   grid, or ACT/365L lacks `coupon_period`
    /// - [`InputError::MissingCalendarForBus252`](crate::InputError::MissingCalendarForBus252):
    ///   `Bus252` without a calendar
    /// - [`InputError::MissingFrequencyForActActIsma`](crate::InputError::MissingFrequencyForActActIsma):
    ///   `ActActIsma` without a frequency
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::InputError::ActActIsmaUnsupportedFrequency):
    ///   `ActActIsma` with a Day or Week frequency and no `coupon_period`
    /// - `Error::Validation`: `ThirtyE360Isda` without an explicit
    ///   `end_is_termination_date`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{DayCount, DayCountContext};
    ///
    /// assert!(DayCount::Act360.validate_context(DayCountContext::default()).is_ok());
    /// assert!(DayCount::Bus252.validate_context(DayCountContext::default()).is_err());
    /// ```
    pub fn validate_context(self, ctx: DayCountContext<'_>) -> crate::Result<()> {
        ctx.validate_structure()?;
        match self {
            DayCount::Bus252 => {
                ctx.calendar.ok_or(InputError::MissingCalendarForBus252)?;
                Ok(())
            }
            DayCount::ActActIsma => validate_act_act_isma_context(ctx),
            DayCount::Act365L => validate_act_365l_context(ctx),
            DayCount::ThirtyE360Isda => ctx
                .end_is_termination_date
                .map(|_| ())
                .ok_or_else(missing_termination_status),
            _ => Ok(()),
        }
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
            DayCount::OneOne => Ok(1.0),
            DayCount::Act360 => Ok(days / 360.0),
            DayCount::Act365F => Ok(days / 365.0),
            DayCount::Act365L => year_fraction_act_365l(start, end, ctx),
            DayCount::Thirty360 => Ok(synthetic(thirty_360_days(
                start,
                end,
                Thirty360Variant::UsEom,
            ))),
            DayCount::Thirty360Isda => Ok(synthetic(thirty_360_days(
                start,
                end,
                Thirty360Variant::IsdaBondBasis,
            ))),
            DayCount::ThirtyE360 => Ok(synthetic(thirty_360_days(
                start,
                end,
                Thirty360Variant::European,
            ))),
            DayCount::ThirtyE360Isda => {
                let terminal = ctx
                    .end_is_termination_date
                    .ok_or_else(missing_termination_status)?;
                Ok(synthetic(thirty_e_360_isda_days(start, end, terminal)))
            }
            DayCount::Thirty360It => Ok(synthetic(thirty_360_days(
                start,
                end,
                Thirty360Variant::Italian,
            ))),
            DayCount::Nl365 => Ok(year_fraction_nl_365(start, end)),
            DayCount::ActAct => Ok(year_fraction_act_act_isda(start, end)),
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
    /// * `start` - Reference date the fraction is measured from; it may lie
    ///   after `end`.
    /// * `end` - Target date; a date before `start` gives a negative fraction
    ///   of the same magnitude as the forward measure.
    /// * `ctx` - Optional context providing calendar or frequency as needed
    ///
    /// # Returns
    ///
    /// - `Ok(0.0)` if `start == end`, after `ctx` is validated as in
    ///   [`validate_context`](Self::validate_context)
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
        // The context (including the termination status of the
        // chronologically later endpoint) applies unchanged to the forward
        // span, which is then negated.
        if end >= start {
            self.year_fraction(start, end, ctx)
        } else {
            Ok(-self.year_fraction(end, start, ctx)?)
        }
    }
}

/// `N / 360` for an exact synthetic day count.
fn synthetic(days: i64) -> f64 {
    days as f64 / 360.0
}

/// 30E/360 ISDA needs the termination status of its later endpoint.
fn missing_termination_status() -> crate::Error {
    crate::Error::Validation(
        "30E/360 ISDA requires an explicit end_is_termination_date in DayCountContext".into(),
    )
}

impl std::fmt::Display for DayCount {
    /// Writes the serde label (`act_360`, `30e_360_isda`, ...).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for DayCount {
    type Err = crate::Error;

    /// Parse the canonical wire label, ignoring surrounding whitespace and ASCII case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse_with_context(
            "unknown day-count convention",
            s,
            &s.trim().to_ascii_lowercase(),
        )
    }
}

#[cfg(test)]
mod parse_tests {
    use super::DayCount;

    /// Every canonical label, in declaration order.
    const NAMES: &[&str] = &[
        "one_one",
        "act_360",
        "act_365f",
        "act_365l",
        "30_360",
        "30_360_isda",
        "30e_360",
        "30e_360_isda",
        "30_360_it",
        "nl_365",
        "act_act",
        "act_act_isma",
        "act_act_afb",
        "bus_252",
    ];

    #[test]
    fn canonical_parse_normalizes_only_case_and_surrounding_whitespace() {
        for name in NAMES {
            let normalized = format!("  {}  ", name.to_ascii_uppercase());
            assert_eq!(
                normalized.parse::<DayCount>().expect(name).to_string(),
                *name
            );
        }
    }

    #[test]
    fn from_str_lists_names_for_unknown_spellings() {
        let err = "nope".parse::<DayCount>().expect_err("unknown").to_string();
        assert!(err.contains("'nope'") && err.contains("act_360") && err.contains("bus_252"));
        for name in NAMES {
            assert_eq!(&name.parse::<DayCount>().expect(name).to_string(), name);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::DayCount;
    use super::act_act::act_act_isma_year_fraction_with_reference_period;
    use crate::InputError;
    use crate::dates::DayCountContext;
    use time::Date;
    use time::macros::date;

    #[test]
    fn equal_dates_validate_context_before_returning_zero() {
        let day = Date::from_calendar_date(2025, time::Month::March, 3).expect("valid date");
        let zero_basis = DayCountContext {
            bus_basis: Some(0),
            ..DayCountContext::default()
        };
        let inverted = DayCountContext {
            coupon_period: Some((day, day)),
            ..DayCountContext::default()
        };
        for convention in [DayCount::Act360, DayCount::OneOne, DayCount::ActAct] {
            assert!(
                convention.year_fraction(day, day, zero_basis).is_err(),
                "{convention:?}"
            );
            assert!(
                convention.signed_year_fraction(day, day, inverted).is_err(),
                "{convention:?}"
            );
            assert_eq!(
                convention
                    .year_fraction(day, day, DayCountContext::default())
                    .expect("context-free"),
                0.0
            );
        }
        assert!(matches!(
            DayCount::Bus252.year_fraction(day, day, DayCountContext::default()),
            Err(crate::Error::Input(InputError::MissingCalendarForBus252))
        ));
        assert!(
            DayCount::Act365L
                .signed_year_fraction(day, day, DayCountContext::default())
                .is_err()
        );
    }

    #[test]
    fn bus252_rejects_a_count_beyond_elapsed_days() {
        struct OverCounting;
        impl crate::dates::HolidayCalendar for OverCounting {
            fn is_holiday(&self, _date: Date) -> crate::Result<bool> {
                Ok(false)
            }

            fn is_business_day(&self, _date: Date) -> crate::Result<bool> {
                Ok(true)
            }

            fn count_business_days(&self, _start: Date, _end: Date) -> crate::Result<i32> {
                Ok(10)
            }
        }
        let start = Date::from_calendar_date(2025, time::Month::March, 3).expect("valid date");
        let end = Date::from_calendar_date(2025, time::Month::March, 5).expect("valid date");
        let ctx = DayCountContext {
            calendar: Some(&OverCounting),
            ..DayCountContext::default()
        };
        assert!(DayCount::Bus252.year_fraction(start, end, ctx).is_err());
    }

    #[test]
    fn act_act_isda_spans_the_supported_date_range() {
        let start = Date::from_calendar_date(-9999, time::Month::January, 1).expect("min year");
        let end = Date::from_calendar_date(9999, time::Month::December, 31).expect("max year");
        let yf = DayCount::ActAct
            .year_fraction(start, end, DayCountContext::default())
            .expect("whole range");
        // 19998 complete years plus 364 of 365 days in 9999.
        assert_eq!(yf, 19_998.0 + 364.0 / 365.0);
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
            super::DayCount::Thirty360Isda,
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
    fn daycount_from_str_rejects_retired_spellings() {
        // schema-rejection-test: retired spellings must never regain parser support.
        for rejected in [
            "1/1",
            "ACT/360",
            "act360",
            "actual_360",
            "act_365",
            "act_365_fixed",
            "actual_365",
            "act365f",
            "actual_365l",
            "act_365_nl",
            "30/360",
            "30_360_us",
            "30_360_bond",
            "30u_360",
            "30_360_eurobond",
            "30_360_italian",
            "30E/360",
            "thirty360",
            "bond_basis",
            "eurobond_basis",
            "act_365afb",
            "isda",
            "icma",
            "act_act_icma",
            "act_act_isda",
            "actual_actual",
            "bus252",
            "business_252",
            "act 360",
            "act-360",
            "act__360",
        ] {
            assert!(rejected.parse::<DayCount>().is_err(), "{rejected}");
        }
    }

    #[test]
    fn thirty_360_isda_selects_bond_basis_not_30e_360_isda() {
        let start = time::macros::date!(2025 - 02 - 28);
        let end = time::macros::date!(2025 - 03 - 31);
        let bond_basis: DayCount = "30_360_isda".parse().expect("canonical");
        assert_eq!(bond_basis, DayCount::Thirty360Isda);
        let ordinary = DayCountContext {
            end_is_termination_date: Some(false),
            ..DayCountContext::default()
        };
        assert_eq!(
            bond_basis
                .year_fraction(start, end, DayCountContext::default())
                .expect("bond basis needs no termination status"),
            33.0 / 360.0
        );
        let european_isda: DayCount = "30e_360_isda".parse().expect("canonical");
        assert_eq!(
            european_isda
                .year_fraction(start, end, ordinary)
                .expect("fraction"),
            30.0 / 360.0
        );
    }

    #[test]
    fn daycount_from_str_unknown() {
        assert!("garbage".parse::<super::DayCount>().is_err());
    }

    // ACT/365L (ISDA coupon-end-year rule, record 002 #5) boundary tests

    fn act365l(start: Date, end: Date, period: (Date, Date)) -> crate::Result<f64> {
        DayCount::Act365L.year_fraction(
            start,
            end,
            DayCountContext {
                coupon_period: Some(period),
                ..DayCountContext::default()
            },
        )
    }

    #[test]
    fn act365l_whole_annual_periods_use_the_end_year() {
        // 2023 has no Feb 29 but the period ends in leap year 2024.
        let (s, e) = (date!(2023 - 01 - 01), date!(2024 - 01 - 01));
        assert_eq!(act365l(s, e, (s, e)).expect("annual"), 365.0 / 366.0);
        // 2024 contains Feb 29 but the period ends in 2025.
        let (s, e) = (date!(2024 - 01 - 01), date!(2025 - 01 - 01));
        assert_eq!(act365l(s, e, (s, e)).expect("annual"), 366.0 / 365.0);
    }

    #[test]
    fn act365l_partial_slice_keeps_the_period_denominator() {
        let period = (date!(2023 - 07 - 01), date!(2024 - 07 - 01));
        let yf = act365l(date!(2023 - 07 - 01), date!(2023 - 12 - 01), period).expect("partial");
        assert_eq!(yf, 153.0 / 366.0);
    }

    #[test]
    fn act365l_needs_no_frequency_and_rejects_slices_outside_the_period() {
        let (s, e) = (date!(2024 - 02 - 29), date!(2024 - 03 - 15));
        assert_eq!(act365l(s, e, (s, e)).expect("no frequency"), 15.0 / 366.0);
        let outside = date!(2024 - 04 - 01);
        assert!(act365l(e, outside, (s, e)).is_err());
        // Containment is checked before the equal-date zero.
        assert!(act365l(outside, outside, (s, e)).is_err());
        assert_eq!(act365l(e, e, (s, e)).expect("inside"), 0.0);
        assert!(
            DayCount::Act365L
                .year_fraction(e, e, DayCountContext::default())
                .is_err()
        );
    }

    // 30E/360 (ISDA) — ISDA 2006 §4.16(h) examples

    #[test]
    fn thirty_e_360_isda_last_day_of_month_rules() {
        use super::thirty360::thirty_e_360_isda_days as days_30e_360_isda;
        use super::{DayCount, DayCountContext};

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

        // The enum variant routes through context for the termination form,
        // which must be explicit, including for equal dates.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        let status = |terminal| DayCountContext {
            end_is_termination_date: Some(terminal),
            ..DayCountContext::default()
        };
        let yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, status(false))
            .expect("should succeed");
        assert_eq!(yf, 180.0 / 360.0);
        let terminal_yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, status(true))
            .expect("termination-period day count should succeed");
        assert_eq!(terminal_yf, 179.0 / 360.0);
        for (a, b) in [(start, end), (end, end)] {
            assert!(
                DayCount::ThirtyE360Isda
                    .year_fraction(a, b, DayCountContext::default())
                    .is_err()
            );
        }
        // Signed reversal keeps the later endpoint's termination status.
        assert_eq!(
            DayCount::ThirtyE360Isda
                .signed_year_fraction(end, start, status(true))
                .expect("signed"),
            -179.0 / 360.0
        );
    }

    #[test]
    fn thirty_e_360_isda_differs_from_european_on_end_of_february() {
        use super::thirty360::{
            Thirty360Variant, thirty_360_days as days_30_360,
            thirty_e_360_isda_days as days_30e_360_isda,
        };

        // 30E/360 (European, §4.16(g)) has NO end-of-February rule;
        // 30E/360 (ISDA, §4.16(h)) adjusts end-of-Feb to 30.
        let start = date!(2012 - 02 - 29);
        let end = date!(2012 - 03 - 31);
        // European: D1=29 kept, D2=31→30: 30 + (30-29) = 31.
        assert_eq!(days_30_360(start, end, Thirty360Variant::European), 31);
        // ISDA: D1=29→30 (last day of Feb), D2=31→30: 30 + (30-30) = 30.
        assert_eq!(days_30e_360_isda(start, end, false), 30);
    }

    #[test]
    fn thirty_360_italian_feb_after_27_and_day_31() {
        use super::thirty360::{Thirty360Variant, thirty_360_days as days_30_360};

        // Jan 31 → Feb 28 2025: Italian D1=30, D2=30 → 30.
        // European D2 stays 28; US SIA D2 stays 28 (not both Feb EOM).
        let start = date!(2025 - 01 - 31);
        let end = date!(2025 - 02 - 28);
        assert_eq!(days_30_360(start, end, Thirty360Variant::Italian), 30);
        assert_eq!(days_30_360(start, end, Thirty360Variant::European), 28);
        assert_eq!(days_30_360(start, end, Thirty360Variant::UsEom), 28);

        // Leap: Feb 29 → Mar 31 2024: Italian D1=30 (Feb>27), D2=30 → 30.
        let start = date!(2024 - 02 - 29);
        let end = date!(2024 - 03 - 31);
        assert_eq!(days_30_360(start, end, Thirty360Variant::Italian), 30);
    }

    // NL/365

    #[test]
    fn nl365_excludes_end_leap_day_but_includes_start_leap_day() {
        use super::{DayCount, DayCountContext};

        let year_fraction = |start, end| {
            DayCount::Nl365
                .year_fraction(start, end, DayCountContext::default())
                .expect("valid interval")
        };
        assert_eq!(
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 02 - 29)),
            0.0
        );
        assert_eq!(
            year_fraction(date!(2024 - 02 - 29), date!(2024 - 03 - 01)),
            1.0 / 365.0
        );
        // Adjacent subperiods must sum to the unsplit accrual.
        assert_eq!(
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 02 - 29))
                + year_fraction(date!(2024 - 02 - 29), date!(2024 - 03 - 01)),
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 03 - 01))
        );
    }

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
        let state = DayCountContextState::try_from(ctx).expect("snapshot context");
        assert_eq!(state.coupon_period, Some(coupon));

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: DayCountContextState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.coupon_period, Some(coupon));

        let restored_ctx = restored.to_ctx().expect("calendar state hydrates");
        assert_eq!(restored_ctx.coupon_period, Some(coupon));

        // An omitted termination status is unspecified, not false: 30E/360
        // ISDA rejects it, including for equal dates.
        let omitted =
            r#"{"calendar_id":null,"frequency":null,"bus_basis":null,"coupon_period":null}"#;
        let state: DayCountContextState = serde_json::from_str(omitted).expect("omission is valid");
        assert_eq!(state.end_is_termination_date, None);
        let day = date!(2025 - 02 - 28);
        assert!(
            super::DayCount::ThirtyE360Isda
                .year_fraction(day, day, state.to_ctx().expect("context"))
                .is_err()
        );
    }

    #[test]
    fn daycount_context_state_rejects_unknown_calendar() {
        let state = super::DayCountContextState {
            calendar_id: Some("not_a_calendar".to_string()),
            frequency: None,
            bus_basis: None,
            coupon_period: None,
            end_is_termination_date: None,
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
