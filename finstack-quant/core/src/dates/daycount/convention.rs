//! Core [`DayCount`] convention enum and year-fraction dispatch.

use super::context::DayCountContext;
use time::Date;

use super::act_act::{
    validate_act_act_isma_context, validate_act_act_isma_interval, year_fraction_act_act_afb,
    year_fraction_act_act_isda, year_fraction_act_act_isma_with_ctx,
};
use super::other::{
    validate_act_365l_context, validate_act_365l_interval, year_fraction_act_365l,
    year_fraction_bus252, year_fraction_nl_365,
};
use super::thirty360::{Thirty360Variant, thirty_360_days, thirty_e_360_isda_days};
use crate::InputError;

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
/// - **`ActActIsma`** (ICMA): single-period contained ICMA fraction
///   `actual_days(start, end) / (coupons_per_year * actual_days(coupon_period))`.
///   Requires both `frequency` (`1M`, `2M`, `3M`, `4M`, `6M`, `12M` or `1Y`)
///   and the enclosing unadjusted nominal `coupon_period` in
///   [`DayCountContext`]; spans crossing nominal coupon boundaries must be
///   sliced by the schedule builder before calling `year_fraction`.
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
/// let ctx = DayCountContext {
///     frequency: Some(Tenor::semi_annual()),
///     coupon_period: Some((start, end)),
///     ..Default::default()
/// };
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
    /// Requires an ICMA coupon frequency (`1M`, `2M`, `3M`, `4M`, `6M`, `12M`,
    /// `1Y`) and the enclosing unadjusted nominal `coupon_period`.
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
    /// Check that the convention can measure elapsed annualized time.
    pub(crate) fn validate_annualized_clock(self) -> crate::Result<()> {
        if self == Self::OneOne {
            return Err(crate::Error::Validation(
                "one_one is a contractual coupon convention and cannot measure annualized time"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Compute the year fraction between `start` and `end` per this convention.
    ///
    /// Provide any required context via [`DayCountContext`]:
    /// - `Bus/252` requires a holiday calendar
    /// - `Act/Act (ISMA)` requires both a coupon frequency and the enclosing nominal `coupon_period`
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
    ///   Using `ActActIsma` without `ctx.coupon_period`
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::InputError::ActActIsmaUnsupportedFrequency):
    ///   Using `ActActIsma` with a frequency outside `{1M, 2M, 3M, 4M, 6M, 12M, 1Y}`
    /// - `Error::Validation`: using `Act365L` without an enclosing
    ///   `coupon_period`, or supplying accrual dates outside `coupon_period`
    ///   for `Act365L` or `ActActIsma` (checked for equal dates too)
    /// - `Error::Validation`: supplying ICMA reference endpoints that do not
    ///   reproduce one unadjusted nominal month grid of `frequency`
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
            match self {
                DayCount::Act365L => {
                    validate_act_365l_interval(start, end, ctx)?;
                }
                DayCount::ActActIsma => {
                    validate_act_act_isma_interval(start, end, ctx)?;
                }
                _ => {}
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
    ///   grid of `frequency`, or ACT/365L lacks `coupon_period`
    /// - [`InputError::MissingCalendarForBus252`](crate::InputError::MissingCalendarForBus252):
    ///   `Bus252` without a calendar
    /// - [`InputError::MissingFrequencyForActActIsma`](crate::InputError::MissingFrequencyForActActIsma):
    ///   `ActActIsma` without a frequency
    /// - [`InputError::MissingCouponPeriodForActActIsma`](crate::InputError::MissingCouponPeriodForActActIsma):
    ///   `ActActIsma` without `coupon_period`
    /// - [`InputError::ActActIsmaUnsupportedFrequency`](crate::InputError::ActActIsmaUnsupportedFrequency):
    ///   `ActActIsma` with a frequency outside `{1M, 2M, 3M, 4M, 6M, 12M, 1Y}`
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
pub(super) fn synthetic(days: i64) -> f64 {
    days as f64 / 360.0
}

/// 30E/360 ISDA needs the termination status of its later endpoint.
pub(super) fn missing_termination_status() -> crate::Error {
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
