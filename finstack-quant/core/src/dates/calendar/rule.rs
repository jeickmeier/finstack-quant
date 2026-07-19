//! Holiday rule definitions for calendar computations.
//!
//! Provides a unified `Rule` enum system for expressing common holiday patterns
//! across global financial market calendars. Rules are used to define when holidays
//! occur, supporting fixed dates, movable holidays, Easter-based calculations,
//! and lunar calendar observances.
//!
//! # Features
//!
//! - **Fixed dates**: New Year's Day, Independence Day, Christmas
//! - **Nth weekday**: MLK Day (3rd Monday), Thanksgiving (4th Thursday)
//! - **Weekend observation**: US-style (Fri/Mon) or UK-style (next Monday)
//! - **Easter-based**: Good Friday, Easter Monday, Ascension Day
//! - **Lunar calendars**: Chinese New Year, Qing Ming, Buddha's Birthday
//! - **Japanese holidays**: Vernal/Autumnal Equinox Days
//! - **Multi-day spans**: Golden Week, extended holiday periods
//!
//! # Rule Evaluation
//!
//! Each rule implements two core methods:
//! - `applies(&self, date)`: O(1) check if date matches the rule
//! - `materialize_year(&self, year, out)`: Generate all matching dates for a year
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_core::dates::{Rule, Observed};
//! use time::{Date, Month};
//!
//! // Fixed date: July 4th (US Independence Day)
//! let july4 = Rule::fixed_weekend(Month::July, 4);
//!
//! // Check if specific date is a holiday
//! let date = Date::from_calendar_date(2025, Month::July, 4)?;
//! assert!(july4.applies(date));
//!
//! // If July 4 falls on Saturday, observed on Friday July 3
//! let saturday = Date::from_calendar_date(2026, Month::July, 4)?; // Saturday
//! assert!(!july4.applies(saturday));
//! let friday = Date::from_calendar_date(2026, Month::July, 3)?;
//! assert!(july4.applies(friday));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # See Also
//!
//! - [`HolidayCalendar`] for the trait that uses these rules
//! - [`Observed`] for weekend observation conventions
//! - [`Direction`] for weekday shift logic
//!
//! [`HolidayCalendar`]: super::business_days::HolidayCalendar

use crate::dates::calendar::algo;
use crate::dates::calendar::business_days::HolidayCalendar;
use time::{Date, Duration, Month, Weekday};

// ---------------------------------------------------------------------------
// Supporting enums
// ---------------------------------------------------------------------------

/// Weekend observation convention for fixed-date holidays.
///
/// Defines how holidays are observed when the calendar date falls on a weekend.
/// Different jurisdictions use different conventions, particularly between
/// US markets (Friday/Monday) and UK/European markets (next Monday only).
///
/// # Variants
///
/// - **`None`**: No adjustment—holiday observed only on exact calendar date
/// - **`NextMonday`**: Weekend holidays observed on following Monday
/// - **`FriIfSatMonIfSun`**: Saturday → Friday, Sunday → Monday (OPM/NYSE convention)
/// - **`MonIfSun`**: Sunday → Monday only; no substitute for Saturday (Federal Reserve convention)
/// - **`MonIfSatTueIfSun`**: Saturday → Monday, Sunday → Tuesday (two days later;
///   UK Christmas/Boxing Day chained substitution)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Rule, Observed};
/// use time::{Date, Month};
///
/// // US Independence Day: if weekend, observe Fri (Sat) or Mon (Sun)
/// let july4 = Rule::Fixed {
///     month: Month::July,
///     day: 4,
///     observed: Observed::FriIfSatMonIfSun,
/// };
///
/// // July 4, 2026 is Saturday → observed July 3 (Friday)
/// let sat = Date::from_calendar_date(2026, Month::July, 4)?;
/// assert!(!july4.applies(sat));
/// let fri = Date::from_calendar_date(2026, Month::July, 3)?;
/// assert!(july4.applies(fri));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Standards Reference
///
/// - **US exchanges / OPM**: FriIfSatMonIfSun (NYSE, NASDAQ, federal workforce)
/// - **Federal Reserve / SOFR / Fedwire**: MonIfSun (Sunday → Monday only; banks
///   are open the Friday before a Saturday holiday — see Federal Reserve Board,
///   "K.8 Holidays Observed by the Federal Reserve System")
/// - **UK markets**: NextMonday (LSE, UK Bank Holidays)
/// - **UK Christmas/Boxing Day**: MonIfSatTueIfSun (chained substitution so the
///   two observed days never collide)
/// - **European markets**: Mixed; often NextMonday or None
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Observed {
    /// No adjustment—holiday is observed **only** on the exact calendar date.
    ///
    /// If the date falls on a weekend, the weekend itself is the holiday.
    /// No substitute business day is designated.
    None,

    /// If holiday falls on Saturday **or** Sunday, observe on following Monday.
    ///
    /// Common in UK and many Commonwealth countries.
    NextMonday,

    /// Saturday → previous Friday; Sunday → following Monday.
    ///
    /// US exchange/OPM convention (NYSE, NASDAQ).
    FriIfSatMonIfSun,

    /// Sunday → following Monday; Saturday → **no substitute**.
    ///
    /// Federal Reserve convention (basis for SOFR/Fedwire business days): when a
    /// fixed holiday falls on a Saturday, banks remain open the preceding
    /// Friday; only Sunday holidays move to Monday.
    ///
    /// # Reference
    /// Federal Reserve Board, "K.8 Holidays Observed by the Federal Reserve
    /// System": "For holidays falling on Saturday, Federal Reserve Banks and
    /// Branches will be open the preceding Friday."
    MonIfSun,

    /// Saturday → following Monday (+2 days); Sunday → following Tuesday (+2 days).
    ///
    /// UK chained-substitution convention for Christmas Day (Dec 25 → observed
    /// Dec 27 when on a weekend) and Boxing Day (Dec 26 → observed Dec 28 when
    /// on a weekend), so the two substitute days never collide:
    ///
    /// - 2021 (Dec 25 Sat): observed Mon Dec 27 + Tue Dec 28
    /// - 2022 (Dec 25 Sun): observed Mon Dec 26 (Boxing, actual day) + Tue Dec 27
    ///
    /// Matches UK government bank-holiday history and QuantLib's
    /// `UnitedKingdom` calendar.
    MonIfSatTueIfSun,
}

/// Search direction for weekday shift rules.
///
/// Used by [`Rule::WeekdayShift`] to specify whether to search forward or
/// backward from a reference date to find the nearest occurrence of a
/// specific weekday.
///
/// # Variants
///
/// - **`After`**: Find nearest weekday on or after the reference date
/// - **`Before`**: Find nearest weekday on or before the reference date
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Rule, Direction};
/// use time::{Date, Month, Weekday};
///
/// // US Election Day: Tuesday on or after November 2
/// let election_day = Rule::WeekdayShift {
///     weekday: Weekday::Tuesday,
///     month: Month::November,
///     day: 2,
///     dir: Direction::After,
/// };
///
/// // November 2, 2026 is Monday → find Tuesday after (Nov 3)
/// let date = Date::from_calendar_date(2026, Month::November, 3)?;
/// assert!(election_day.applies(date));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Direction {
    /// Find the nearest occurrence of the weekday **on or after** the reference date.
    After,

    /// Find the nearest occurrence of the weekday **on or before** the reference date.
    Before,
}

// ---------------------------------------------------------------------------
// Rule enum
// ---------------------------------------------------------------------------

/// Holiday rule pattern for calendar date computations.
///
/// A unified enum representing common holiday patterns across global financial
/// market calendars. Each variant encapsulates a specific holiday calculation
/// pattern (fixed date, movable holiday, lunar calendar, etc.).
///
/// # Variants
///
/// ## Fixed and Weekday-Based
///
/// - **`Fixed`**: Fixed calendar date (Jan 1, Dec 25) with optional weekend observation
/// - **`NthWeekday`**: nth weekday of month (3rd Monday, last Friday)
/// - **`WeekdayShift`**: First weekday on/after or on/before a reference date
///
/// ## Religious and Cultural
///
/// - **`EasterOffset`**: Offset from Easter Monday (Good Friday = -3, Ascension = +38)
/// - **`ChineseNewYear`**: Spring Festival (lunar new year)
/// - **`QingMing`**: Tomb-Sweeping Day (Chinese solar term)
/// - **`BuddhasBirthday`**: Vesak (8th day of 4th lunar month)
///
/// ## Regional
///
/// - **`VernalEquinoxJP`**: Japanese Vernal Equinox Day (Shunbun no Hi)
/// - **`AutumnalEquinoxJP`**: Japanese Autumnal Equinox Day (Shūbun no Hi)
///
/// ## Composite
///
/// - **`Span`**: Multi-day consecutive holiday period (Golden Week, extended breaks)
///
/// # Usage
///
/// Rules are typically defined in JSON calendar files and loaded at build time.
/// Each rule can be evaluated against a specific date using `applies()` or
/// materialized for an entire year using `materialize_year()`.
///
/// # Examples
///
/// Fixed date with weekend observation:
/// ```rust
/// use finstack_quant_core::dates::{Rule, Observed};
/// use time::{Date, Month};
///
/// let new_years = Rule::fixed_next_monday(Month::January, 1);
///
/// // Jan 1, 2022 is Saturday → observed Monday Jan 3
/// let sat = Date::from_calendar_date(2022, Month::January, 1)?;
/// assert!(!new_years.applies(sat));
/// let mon = Date::from_calendar_date(2022, Month::January, 3)?;
/// assert!(new_years.applies(mon));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Nth weekday of month:
/// ```rust
/// use finstack_quant_core::dates::Rule;
/// use time::{Date, Month, Weekday};
///
/// // US Thanksgiving: 4th Thursday of November
/// let thanksgiving = Rule::NthWeekday {
///     n: 4,
///     weekday: Weekday::Thursday,
///     month: Month::November,
/// };
///
/// let date = Date::from_calendar_date(2025, Month::November, 27)?;
/// assert!(thanksgiving.applies(date));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Easter offset:
/// ```rust
/// use finstack_quant_core::dates::Rule;
/// use time::{Date, Month};
///
/// // Good Friday = Easter Monday - 3 days
/// let good_friday = Rule::EasterOffset(-3);
///
/// let date = Date::from_calendar_date(2025, Month::April, 18)?;
/// assert!(good_friday.applies(date));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`Observed`] for weekend observation conventions
/// - [`Direction`] for weekday shift direction
/// - [`HolidayCalendar`] for using rules in calendars
///
/// [`HolidayCalendar`]: super::business_days::HolidayCalendar
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Rule {
    /// Exact one-off calendar date.
    ///
    /// Used for exchange-announced closures and other dated exceptions that
    /// cannot be represented safely by a recurring Gregorian rule.
    ExactDate {
        /// Calendar year.
        year: i32,
        /// Calendar month.
        month: Month,
        /// Day of month.
        day: u8,
    },

    /// Fixed calendar date with optional weekend observation.
    ///
    /// Examples: New Year's Day (Jan 1), Christmas (Dec 25), Independence Day (Jul 4).
    ///
    /// The `observed` field controls how the holiday is handled when it falls
    /// on a weekend (see [`Observed`]).
    Fixed {
        /// Month of the holiday
        month: Month,
        /// Day of the month (1-31)
        day: u8,
        /// Weekend observation convention
        observed: Observed,
    },

    /// Nth occurrence of a weekday within a month.
    ///
    /// Examples: MLK Day (3rd Monday of January), Thanksgiving (4th Thursday of November).
    ///
    /// # Convention
    /// - `n > 0`: nth occurrence from **start** of month (1 = first, 2 = second, ...)
    /// - `n < 0`: nth occurrence from **end** of month (-1 = last, -2 = second-to-last, ...)
    NthWeekday {
        /// Occurrence count (positive from start, negative from end)
        n: i8,
        /// Target weekday
        weekday: Weekday,
        /// Month
        month: Month,
    },

    /// Shift to nearest weekday on or after/before a reference date.
    ///
    /// Examples: US Election Day (Tuesday on or after Nov 2).
    ///
    /// Starts from `month/day` and shifts to the nearest `weekday` in the
    /// specified `dir`ection.
    WeekdayShift {
        /// Target weekday
        weekday: Weekday,
        /// Reference month
        month: Month,
        /// Reference day
        day: u8,
        /// Search direction (After or Before)
        dir: Direction,
    },

    /// Offset in days from Easter Monday.
    ///
    /// Examples: Good Friday (-3), Easter Monday (0), Ascension Day (+38).
    ///
    /// # Calculation
    /// Easter Monday is computed using the Anonymous Gregorian algorithm.
    /// The offset is then applied as calendar days.
    ///
    /// # Common Offsets
    /// - Good Friday: -3
    /// - Easter Sunday: -1
    /// - Easter Monday: 0
    /// - Ascension Day: +38
    /// - Whit Monday: +49
    EasterOffset(i16),

    /// Consecutive multi-day holiday period.
    ///
    /// Examples: Golden Week (Japan/China), extended Christmas breaks.
    ///
    /// Materializes `len` consecutive days starting from each date that
    /// matches the `start` rule, shifted by `offset` calendar days. Handles year
    /// boundaries correctly.
    ///
    /// The `offset` allows anchoring a span relative to a movable start rule —
    /// e.g. China's Spring Festival break begins on Lunar New Year's **Eve**
    /// (`start: ChineseNewYear`, `offset: -1`).
    ///
    /// # Note
    /// This variant cannot be serialized (contains `&'static Rule`).
    /// Used only in compiled calendar definitions.
    #[serde(skip)]
    Span {
        /// Rule defining the start date(s)
        start: &'static Rule,
        /// Number of consecutive days (including start day)
        len: u8,
        /// Calendar-day shift applied to each start date before spanning
        /// (0 = span begins on the start date; -1 = the day before).
        offset: i16,
    },

    /// Chinese New Year (Spring Festival, 春节).
    ///
    /// Celebrated on the first day of the Chinese lunar calendar, typically
    /// between January 21 and February 20. Uses pre-computed lookup table
    /// for years 1970-2150.
    ///
    /// # Markets
    /// Public holiday in Mainland China, Hong Kong, Taiwan, Singapore, and
    /// other Asian markets with significant Chinese populations.
    ChineseNewYear,

    /// Qing Ming Festival (清明节, Tomb-Sweeping Day).
    ///
    /// One of the 24 solar terms in the traditional Chinese calendar,
    /// typically falling around April 4-5. Computed using solar longitude
    /// formula.
    ///
    /// # Markets
    /// Public holiday in Mainland China, Hong Kong, Taiwan.
    QingMing,

    /// Buddha's Birthday (Vesak, 佛誕).
    ///
    /// Celebrated on the 8th day of the 4th Chinese lunar month. Approximated
    /// as Chinese New Year + 95 days.
    ///
    /// # Markets
    /// Public holiday in Hong Kong, Macau, and some other Asian markets.
    BuddhasBirthday,

    /// Dragon Boat Festival (端午节, Duānwǔ).
    ///
    /// Celebrated on the 5th day of the 5th Chinese lunar month, typically
    /// falling between late May and mid June. Uses a pre-computed lookup table
    /// for years 1970-2150.
    ///
    /// # Markets
    /// National statutory holiday in Mainland China (since 2008); also observed
    /// in Hong Kong, Taiwan, and Macau.
    DragonBoat,

    /// Mid-Autumn Festival (中秋节, Zhōngqiū).
    ///
    /// Celebrated on the 15th day of the 8th Chinese lunar month, typically
    /// falling between mid September and early October. Uses a pre-computed
    /// lookup table for years 1970-2150.
    ///
    /// # Markets
    /// National statutory holiday in Mainland China (since 2008); also observed
    /// in Hong Kong, Taiwan, and Macau.
    MidAutumn,

    /// Vernal Equinox Day (春分の日, Shunbun no Hi).
    ///
    /// Japanese national holiday around March 20-21, computed using
    /// astronomical formula from the National Astronomical Observatory of Japan.
    ///
    /// # Reference
    /// - Formula valid for years 1900-2100
    /// - Source: Japan National Astronomical Observatory (国立天文台)
    VernalEquinoxJP,

    /// Autumnal Equinox Day (秋分の日, Shūbun no Hi).
    ///
    /// Japanese national holiday around September 22-23, computed using
    /// astronomical formula from the National Astronomical Observatory of Japan.
    ///
    /// # Reference
    /// - Formula valid for years 1900-2100
    /// - Source: Japan National Astronomical Observatory (国立天文台)
    AutumnalEquinoxJP,

    /// Effective-date wrapper that gates an inner rule to a closed range of
    /// calendar years.
    ///
    /// Holidays are adopted (and occasionally retired) on specific dates — e.g.
    /// NYSE first closed for Juneteenth in 2022 and for Martin Luther King Jr.
    /// Day in 1998. Without gating, a rule applies to every year in
    /// `[BASE_YEAR, END_YEAR]`, which silently marks pre-adoption dates as
    /// holidays and corrupts historical accruals, settlement, and day counts.
    ///
    /// `from_year` / `to_year` are **inclusive** bounds; `None` means unbounded
    /// on that side. The inner rule is delegated to only when
    /// `from_year <= date.year() <= to_year`.
    ///
    /// # Note
    /// Like [`Span`](Rule::Span) this variant contains `&'static Rule` and so is
    /// not serializable; it is produced only by the compiled calendar
    /// definitions.
    #[serde(skip)]
    Effective {
        /// Inclusive first year the inner rule applies (`None` = unbounded).
        from_year: Option<i32>,
        /// Inclusive last year the inner rule applies (`None` = unbounded).
        to_year: Option<i32>,
        /// The gated rule.
        inner: &'static Rule,
    },

    /// Mainland China single-day festival with the modern 3-day 连休 (bridge)
    /// convention.
    ///
    /// Wraps a `festival` rule that materializes exactly one date per year (e.g.
    /// [`QingMing`](Rule::QingMing), [`DragonBoat`](Rule::DragonBoat),
    /// [`MidAutumn`](Rule::MidAutumn), or a fixed New Year's Day) and expands it
    /// into the actual **weekday** market closures produced by joining the
    /// festival to the adjacent weekend.
    ///
    /// This reflects the arrangement codified by the State Council's November
    /// 2024 amendment (effective 2025-01-01), where each of New Year's Day,
    /// Qingming, Dragon Boat, and Mid-Autumn forms a 3-day break "如逢周三则只在
    /// 当日放假" (a single day when it falls on a Wednesday). It is an exact match
    /// for the 2025+ regime and a close approximation for 2008-2024.
    ///
    /// Weekend days inside the block are intentionally **not** emitted (they are
    /// already non-business days); only the festival day and any weekday
    /// bridge/substitute days are produced:
    ///
    /// | Festival weekday | Weekday closures                      |
    /// |------------------|---------------------------------------|
    /// | Mon / Wed / Fri  | festival day only                     |
    /// | Tue              | preceding Mon (bridge) + festival day |
    /// | Thu              | festival day + following Fri (bridge) |
    /// | Sat / Sun        | following Mon (substitute)            |
    ///
    /// # Note
    /// Like [`Span`](Rule::Span) this variant contains `&'static Rule` and so is
    /// not serializable; it is produced only by the compiled calendar
    /// definitions.
    #[serde(skip)]
    ChinaBridge {
        /// Single-day festival rule to expand into a bridged closure block.
        festival: &'static Rule,
    },
}

/// Returns `true` when `year` lies within the inclusive `[from, to]` bounds
/// (`None` bounds are treated as unbounded).
#[inline]
const fn year_in_effective_range(year: i32, from: Option<i32>, to: Option<i32>) -> bool {
    if let Some(f) = from {
        if year < f {
            return false;
        }
    }
    if let Some(t) = to {
        if year > t {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Public helper constructors for ergonomics
// ---------------------------------------------------------------------------
impl Rule {
    /// Convenience for `Rule::Fixed { … }` with no observation.
    #[inline]
    pub const fn fixed(month: Month, day: u8) -> Self {
        Rule::Fixed {
            month,
            day,
            observed: Observed::None,
        }
    }

    /// Convenience for fixed date with Monday substitution.
    #[inline]
    pub const fn fixed_next_monday(month: Month, day: u8) -> Self {
        Rule::Fixed {
            month,
            day,
            observed: Observed::NextMonday,
        }
    }

    /// Convenience for US-style Fri/Sat-Mon substitution.
    #[inline]
    pub const fn fixed_weekend(month: Month, day: u8) -> Self {
        Rule::Fixed {
            month,
            day,
            observed: Observed::FriIfSatMonIfSun,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers reused by applies()
// ---------------------------------------------------------------------------

// is_leap_year and add_months provided by shared utils

// Easter Monday is now provided by calendar::algo

// Chinese New Year helpers now provided by calendar::algo

/// Calculate Qing Ming (Tomb-Sweeping Day) based on solar term calculations.
///
/// Qing Ming is one of the 24 solar terms in the traditional Chinese calendar,
/// typically falling around April 4-5 when the sun reaches celestial longitude 15°.
///
/// The formula uses mean solar longitude calculations with epoch 1900.
/// Accurate for years 1900-2100.
///
/// # Constants
/// - Base offset: 5.59 days into April
/// - Slope: 0.2422 days per year (accounts for calendar drift)
/// - Epoch: 1900 (reference year for calculation)
fn qing_ming_day(year: i32) -> u8 {
    const QINGMING_BASE: f64 = 5.59;
    const QINGMING_SLOPE: f64 = 0.2422;
    const QINGMING_EPOCH: i32 = 1900;

    let y = (year - QINGMING_EPOCH) as f64;
    (QINGMING_BASE + QINGMING_SLOPE * y - (y / 4.0).floor()) as u8
}

// Helper for Buddha's Birthday approximation (CNY +95 days)
fn buddhas_birthday_date(year: i32) -> Option<Date> {
    algo::cny_date(year).map(|cny| cny + Duration::days(95))
}

/// Calculate Vernal Equinox Day for Japan.
///
/// Uses the formula from Japan's National Astronomical Observatory (NAO)
/// for approximating the date of the vernal (spring) equinox, which is
/// a national holiday in Japan.
///
/// The formula is based on astronomical calculations with epoch 1980.
/// Accurate for years 1900-2100.
///
/// # Constants
/// - Epoch: 1980 (reference year for NAO formula)
/// - Base: 20.8431 days into March
/// - Slope: 0.242194 days per year (accounts for precession)
///
/// # Reference
/// Japan National Astronomical Observatory (国立天文台)
fn vernal_equinox_jp(year: i32) -> Option<Date> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    const VERNAL_EPOCH: i32 = 1980;
    const VERNAL_BASE: f64 = 20.8431;
    const VERNAL_SLOPE: f64 = 0.242194;

    let y = (year - VERNAL_EPOCH) as f64;
    let day = (VERNAL_BASE + VERNAL_SLOPE * y - (y / 4.0).floor()).floor() as u8;
    let day = day.clamp(1, 31);
    Date::from_calendar_date(year, Month::March, day)
        .or_else(|_| Date::from_calendar_date(year, Month::March, 21))
        .ok()
}

/// Calculate Autumnal Equinox Day for Japan.
///
/// Uses the formula from Japan's National Astronomical Observatory (NAO)
/// for approximating the date of the autumnal (fall) equinox, which is
/// a national holiday in Japan.
///
/// The formula is based on astronomical calculations with epoch 1980.
/// Accurate for years 1900-2100.
///
/// # Constants
/// - Epoch: 1980 (reference year for NAO formula)
/// - Base: 23.2488 days into September
/// - Slope: 0.242194 days per year (accounts for precession)
///
/// # Reference
/// Japan National Astronomical Observatory (国立天文台)
fn autumnal_equinox_jp(year: i32) -> Option<Date> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    const AUTUMNAL_EPOCH: i32 = 1980;
    const AUTUMNAL_BASE: f64 = 23.2488;
    const AUTUMNAL_SLOPE: f64 = 0.242194;

    let y = (year - AUTUMNAL_EPOCH) as f64;
    let day = (AUTUMNAL_BASE + AUTUMNAL_SLOPE * y - (y / 4.0).floor()).floor() as u8;
    let day = day.clamp(1, 30); // September has 30 days
    Date::from_calendar_date(year, Month::September, day)
        .or_else(|_| Date::from_calendar_date(year, Month::September, 23))
        .ok()
}

#[inline]
fn apply_observed(mut base: Date, observed: Observed) -> Date {
    match observed {
        Observed::None => {}
        Observed::NextMonday => {
            if matches!(base.weekday(), Weekday::Saturday) {
                base += Duration::days(2);
            } else if matches!(base.weekday(), Weekday::Sunday) {
                base += Duration::days(1);
            }
        }
        Observed::FriIfSatMonIfSun => {
            if matches!(base.weekday(), Weekday::Saturday) {
                base -= Duration::days(1);
            } else if matches!(base.weekday(), Weekday::Sunday) {
                base += Duration::days(1);
            }
        }
        Observed::MonIfSun => {
            // Federal Reserve convention: Sunday → Monday; Saturday holidays
            // are not substituted (banks open the preceding Friday).
            if matches!(base.weekday(), Weekday::Sunday) {
                base += Duration::days(1);
            }
        }
        Observed::MonIfSatTueIfSun => {
            // UK chained substitution: weekend holiday observed two calendar
            // days later (Sat → Mon, Sun → Tue), so paired holidays such as
            // Christmas/Boxing Day map to distinct substitute days.
            if matches!(base.weekday(), Weekday::Saturday | Weekday::Sunday) {
                base += Duration::days(2);
            }
        }
    }
    base
}

#[inline]
fn shift_to_weekday(mut d: Date, weekday: Weekday, dir: Direction) -> Date {
    match dir {
        Direction::After => {
            for _ in 0..7 {
                if d.weekday() == weekday {
                    return d;
                }
                d += Duration::days(1);
            }
        }
        Direction::Before => {
            for _ in 0..7 {
                if d.weekday() == weekday {
                    return d;
                }
                d -= Duration::days(1);
            }
        }
    }
    d
}

// ---------------------------------------------------------------------------
// Reusable span materialization helper
// ---------------------------------------------------------------------------
#[inline]
fn push_span_range<A: smallvec::Array<Item = Date>>(
    out: &mut smallvec::SmallVec<A>,
    starts: &[Date],
    len: u8,
    offset: i16,
) {
    if len == 0 {
        return;
    }
    let span_days = len as i64;
    for &sd in starts {
        let base = sd + Duration::days(offset as i64);
        for k in 0..span_days {
            out.push(base + Duration::days(k));
        }
    }
}

// ---------------------------------------------------------------------------
// China 连休 (bridge) block materialization helper
// ---------------------------------------------------------------------------
/// Push the **weekday** market closures for a single-day Chinese festival that
/// falls on `festival`, joining it to the adjacent weekend per the modern 3-day
/// 连休 convention (see [`Rule::ChinaBridge`]). Weekend days are not emitted.
#[inline]
fn push_china_bridge_block<A: smallvec::Array<Item = Date>>(
    out: &mut smallvec::SmallVec<A>,
    festival: Date,
) {
    match festival.weekday() {
        // Wednesday: single day. Mon/Fri: the festival plus a same-side weekend
        // that is already a non-business day, so only the festival is emitted.
        Weekday::Monday | Weekday::Wednesday | Weekday::Friday => out.push(festival),
        // Tuesday: bridge the preceding Monday.
        Weekday::Tuesday => {
            out.push(festival - Duration::days(1));
            out.push(festival);
        }
        // Thursday: bridge the following Friday.
        Weekday::Thursday => {
            out.push(festival);
            out.push(festival + Duration::days(1));
        }
        // Weekend: substitute the following Monday.
        Weekday::Saturday => out.push(festival + Duration::days(2)),
        Weekday::Sunday => out.push(festival + Duration::days(1)),
    }
}

// ---------------------------------------------------------------------------
// Core implementation – applies()
// ---------------------------------------------------------------------------
impl Rule {
    /// Returns `true` when the rule marks `date` a holiday.
    #[inline]
    pub fn applies(&self, date: Date) -> bool {
        match self {
            Rule::ExactDate { year, month, day } => {
                Date::from_calendar_date(*year, *month, *day).ok() == Some(date)
            }
            Rule::Fixed {
                month,
                day,
                observed,
            } => {
                // Observance can cross a year boundary: e.g. Jan-1-on-Saturday
                // observed Friday Dec 31 of the PRIOR year under
                // FriIfSatMonIfSun. Reconstructing the base only from
                // `date.year()` made `applies()` diverge from
                // `materialize_year()` for those dates (2026-06-09 core quant
                // review, Moderate/Dates), so test the rule materialized from
                // the adjacent years as well (observance shifts at most 2
                // days, so ±1 year is sufficient).
                //
                // Fast path: the current year matches almost every query. An
                // adjacent-year base can only land on `date` within 2 days of a
                // year boundary, so skip those two date constructions otherwise.
                let check = |y: i32| {
                    Date::from_calendar_date(y, *month, *day)
                        .ok()
                        .map(|base| apply_observed(base, *observed) == date)
                        .unwrap_or(false)
                };
                check(date.year())
                    || (date.month() == time::Month::January
                        && date.day() <= 2
                        && check(date.year() - 1))
                    || (date.month() == time::Month::December
                        && date.day() >= 30
                        && check(date.year() + 1))
            }
            Rule::NthWeekday { n, weekday, month } => {
                // Month guard first: `nth_weekday_of_month` only ever returns a date
                // inside `month`, so a query in any other month cannot match. This
                // skips the computation for ~11/12 of queries without changing the
                // answer.
                date.month() == *month
                    && crate::dates::calendar::generated::nth_weekday_of_month(
                        date.year(),
                        *month,
                        *weekday,
                        *n,
                    ) == Some(date)
            }
            Rule::WeekdayShift {
                weekday,
                month,
                day,
                dir,
            } => Date::from_calendar_date(date.year(), *month, *day)
                .ok()
                .map(|base| shift_to_weekday(base, *weekday, *dir) == date)
                .unwrap_or(false),
            Rule::EasterOffset(offset) => {
                let easter_mon = algo::easter_monday(date.year());
                let target = easter_mon + Duration::days(*offset as i64);
                target == date
            }
            Rule::Span { start, len, offset } => {
                // Pre-compute start dates for this and previous year, then range-check.
                // Previous year is needed for spans that cross year boundaries.
                let y = date.year();
                let mut starts = smallvec::SmallVec::<[Date; 64]>::new();
                start.materialize_year(y, &mut starts);
                if *len > 1 || *offset != 0 {
                    start.materialize_year(y - 1, &mut starts);
                }
                let span_days = *len as i64;
                for sd in starts {
                    let base = sd + Duration::days(*offset as i64);
                    if date >= base && date < base + Duration::days(span_days) {
                        return true;
                    }
                }
                false
            }
            Rule::ChineseNewYear => algo::is_cny(date),
            Rule::DragonBoat => algo::is_dragon_boat(date),
            Rule::MidAutumn => algo::is_mid_autumn(date),
            Rule::QingMing => {
                date.month() == Month::April && date.day() == qing_ming_day(date.year())
            }
            Rule::BuddhasBirthday => buddhas_birthday_date(date.year()) == Some(date),
            Rule::VernalEquinoxJP => vernal_equinox_jp(date.year()).is_some_and(|d| d == date),
            Rule::AutumnalEquinoxJP => autumnal_equinox_jp(date.year()).is_some_and(|d| d == date),
            Rule::Effective {
                from_year,
                to_year,
                inner,
            } => year_in_effective_range(date.year(), *from_year, *to_year) && inner.applies(date),
            Rule::ChinaBridge { festival } => {
                // Compute the festival's bridge block for the current year and,
                // when `date` is near a year boundary, the adjacent year (a
                // Tuesday New Year's Day bridges back to the preceding Dec 31).
                let mut block = smallvec::SmallVec::<[Date; 4]>::new();
                let mut fdates = smallvec::SmallVec::<[Date; 2]>::new();
                festival.materialize_year(date.year(), &mut fdates);
                if date.month() == Month::December && date.day() >= 29 {
                    festival.materialize_year(date.year() + 1, &mut fdates);
                }
                for fd in fdates {
                    push_china_bridge_block(&mut block, fd);
                }
                block.contains(&date)
            }
        }
    }
}

impl Rule {
    /// Append all dates in `year` that this rule marks as a holiday into `out`.
    /// No deduplication is performed.
    pub fn materialize_year<A: smallvec::Array<Item = Date>>(
        &self,
        year: i32,
        out: &mut smallvec::SmallVec<A>,
    ) {
        match self {
            Rule::ExactDate {
                year: exact_year,
                month,
                day,
            } => {
                if year == *exact_year {
                    if let Ok(date) = Date::from_calendar_date(year, *month, *day) {
                        out.push(date);
                    }
                }
            }
            Rule::Fixed {
                month,
                day,
                observed,
            } => {
                if let Ok(base) = Date::from_calendar_date(year, *month, *day) {
                    let base = apply_observed(base, *observed);
                    out.push(base);
                }
            }
            Rule::NthWeekday { n, weekday, month } => {
                if let Some(d) = crate::dates::calendar::generated::nth_weekday_of_month(
                    year, *month, *weekday, *n,
                ) {
                    out.push(d);
                }
            }
            Rule::WeekdayShift {
                weekday,
                month,
                day,
                dir,
            } => {
                if let Ok(base) = Date::from_calendar_date(year, *month, *day) {
                    out.push(shift_to_weekday(base, *weekday, *dir));
                }
            }
            Rule::EasterOffset(offset) => {
                let em = algo::easter_monday(year);
                out.push(em + Duration::days(*offset as i64));
            }
            Rule::Span { start, len, offset } => {
                let mut tmp = smallvec::SmallVec::<[Date; 64]>::new();
                start.materialize_year(year, &mut tmp);
                // Also materialize previous year starts for spans that may cross year boundaries
                if *len > 1 || *offset != 0 {
                    start.materialize_year(year - 1, &mut tmp);
                }
                push_span_range(out, &tmp, *len, *offset);
            }
            Rule::ChineseNewYear => {
                if let Some(d) = algo::cny_date(year) {
                    out.push(d);
                }
            }
            Rule::DragonBoat => {
                if let Some(d) = algo::dragon_boat_date(year) {
                    out.push(d);
                }
            }
            Rule::MidAutumn => {
                if let Some(d) = algo::mid_autumn_date(year) {
                    out.push(d);
                }
            }
            Rule::QingMing => {
                if let Ok(d) = Date::from_calendar_date(year, Month::April, qing_ming_day(year)) {
                    out.push(d);
                }
            }
            Rule::BuddhasBirthday => {
                if let Some(d) = buddhas_birthday_date(year) {
                    out.push(d);
                }
            }
            Rule::VernalEquinoxJP => {
                if let Some(d) = vernal_equinox_jp(year) {
                    out.push(d);
                }
            }
            Rule::AutumnalEquinoxJP => {
                if let Some(d) = autumnal_equinox_jp(year) {
                    out.push(d);
                }
            }
            Rule::Effective {
                from_year,
                to_year,
                inner,
            } => {
                if year_in_effective_range(year, *from_year, *to_year) {
                    inner.materialize_year(year, out);
                }
            }
            Rule::ChinaBridge { festival } => {
                let mut fdates = smallvec::SmallVec::<[Date; 2]>::new();
                festival.materialize_year(year, &mut fdates);
                for fd in fdates {
                    push_china_bridge_block(out, fd);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Implement HolidayCalendar for &[Rule]
// ---------------------------------------------------------------------------
impl HolidayCalendar for &[Rule] {
    fn is_holiday(&self, date: Date) -> bool {
        self.iter().any(|r| r.applies(date))
    }
}

// ---------------------------------------------------------------------------
// Blanket impl for slices of holiday calendars (composite-union semantics)
// (removed: use CompositeCalendar for composition)
// ---------------------------------------------------------------------------
