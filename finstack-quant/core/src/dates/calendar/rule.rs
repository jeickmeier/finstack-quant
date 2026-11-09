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
//! - **Lunar calendars**: Chinese New Year, Qing Ming, Dragon Boat, Mid-Autumn
//! - **Jurisdiction rules**: Hong Kong and Japan public holidays (lunar festivals,
//!   equinox days, substitute holidays)
//! - **Multi-day spans**: Golden Week, extended holiday periods
//!
//! # Rule Evaluation
//!
//! Each rule implements two core methods:
//! - `applies(&self, date)`: membership test over `dates_in_year` for the date's year
//! - `materialize_year(&self, year, out)`: Generate all matching dates for a year
//!
//! # See Also
//!
//! - [`HolidayCalendar`] for the trait that uses these rules
//! - [`Observed`] for weekend observation conventions
//! - [`Direction`] for weekday shift logic
//!
//! [`HolidayCalendar`]: super::business_days::HolidayCalendar

use crate::dates::calendar::algo;
use time::{Date, Duration, Month, Weekday};

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
/// # Standards Reference
///
/// - **US exchanges / OPM**: FriIfSatMonIfSun (NYSE, NASDAQ, federal workforce)
/// - **Federal Reserve / Fedwire**: MonIfSun (Sunday → Monday only; banks
///   are open the Friday before a Saturday holiday — see Federal Reserve Board,
///   "K.8 Holidays Observed by the Federal Reserve System")
/// - **UK markets**: NextMonday (LSE, UK Bank Holidays)
/// - **UK Christmas/Boxing Day**: MonIfSatTueIfSun (chained substitution so the
///   two observed days never collide)
/// - **European markets**: Mixed; often NextMonday or None
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Observed {
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
    /// Federal Reserve convention (basis for Fedwire business days): when a
    /// fixed holiday falls on a Saturday, banks remain open the preceding
    /// Friday; only Sunday holidays move to Monday.
    ///
    /// # Reference
    /// Federal Reserve Board, "K.8 Holidays Observed by the Federal Reserve
    /// System": "For holidays falling on Saturday, Federal Reserve Banks and
    /// Branches will be open the preceding Friday." `docs/REFERENCES.md#fed-k8-holidays`
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Direction {
    /// Find the nearest occurrence of the weekday **on or after** the reference date.
    After,

    /// Find the nearest occurrence of the weekday **on or before** the reference date.
    Before,
}

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
/// # See Also
///
/// - [`Observed`] for weekend observation conventions
/// - [`Direction`] for weekday shift direction
/// - [`HolidayCalendar`] for using rules in calendars
///
/// [`HolidayCalendar`]: super::business_days::HolidayCalendar
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub(crate) enum Rule {
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
    /// typically falling around April 4-5. Uses an astronomical date table
    /// in UTC+8 for years 1970-2150.
    ///
    /// # Markets
    /// Public holiday in Mainland China, Hong Kong, Taiwan.
    QingMing,

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

    /// Hong Kong general holidays, including lunar festivals, Sunday
    /// substitutions and additional holidays when two holidays coincide.
    ///
    /// Includes HKSAR-era adoption dates and the 1983/2012 changes to lunar
    /// holiday substitution. Lunar and solar dates are tabulated for 1970–2150.
    /// Includes recurring colonial holidays; exceptional weather closures and
    /// discretionary historical substitutions require explicit dated rules.
    HongKongPublicHolidays,

    /// Japanese national holidays, Sunday substitutes and citizens' holidays.
    ///
    /// Applies historical adoption dates, Happy Monday transitions, the
    /// 1973/2007 substitution rules and announced imperial/Olympic exceptions.
    /// Recurring rules and equinox tables cover the computational 1970–2150 range.
    /// Future equinox dates are astronomical projections pending announcement.
    /// Bank/exchange New Year closures must be supplied separately.
    JapanPublicHolidays,

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
    ChinaBridge {
        /// Single-day festival rule to expand into a bridged closure block.
        festival: &'static Rule,
    },
}

/// Returns `true` when `year` lies within the inclusive `[from, to]` bounds
/// (`None` bounds are treated as unbounded).
#[inline]
const fn year_in_effective_range(year: i32, from: Option<i32>, to: Option<i32>) -> bool {
    if let Some(f) = from
        && year < f
    {
        return false;
    }
    if let Some(t) = to
        && year > t
    {
        return false;
    }
    true
}

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
}

/// Occurrences produced by one rule for one nominal year.
pub(crate) type Occurrences = smallvec::SmallVec<[Date; 32]>;

/// Error for an invalid built-in rule definition.
fn invalid_rule(message: impl Into<String>) -> crate::Error {
    crate::Error::Validation(format!("invalid holiday rule: {}", message.into()))
}

/// Shift `date` by `days`, or `None` when the result is outside the `time`
/// date range. Such a date lies outside every representable request year, so
/// it can never affect a query.
#[inline]
fn shift(date: Date, days: i64) -> Option<Date> {
    date.checked_add(Duration::days(days))
}

#[inline]
fn apply_observed(base: Date, observed: Observed) -> Option<Date> {
    let weekday = base.weekday();
    let (saturday, sunday) = match observed {
        Observed::None => (0, 0),
        Observed::NextMonday => (2, 1),
        Observed::FriIfSatMonIfSun => (-1, 1),
        // Federal Reserve convention: Sunday → Monday; Saturday holidays are
        // not substituted (banks open the preceding Friday).
        Observed::MonIfSun => (0, 1),
        // UK chained substitution: weekend holiday observed two calendar days
        // later, so paired Christmas/Boxing Day map to distinct days.
        Observed::MonIfSatTueIfSun => (2, 2),
    };
    let delta = match weekday {
        Weekday::Saturday => saturday,
        Weekday::Sunday => sunday,
        _ => 0,
    };
    shift(base, delta)
}

/// Inclusive on-or-after / on-or-before weekday shift (at most six days).
#[inline]
fn shift_to_weekday(base: Date, weekday: Weekday, dir: Direction) -> Option<Date> {
    let from = i64::from(base.weekday().number_days_from_monday());
    let to = i64::from(weekday.number_days_from_monday());
    let delta = match dir {
        Direction::After => (to - from).rem_euclid(7),
        Direction::Before => -((from - to).rem_euclid(7)),
    };
    shift(base, delta)
}

/// Validate a month/day pair for a recurring rule: February 29 is a valid
/// pattern (no occurrence in a common year); April 31 is not.
fn validate_month_day(month: Month, day: u8) -> crate::Result<()> {
    if day >= 1 && day <= month.length(2024) {
        Ok(())
    } else {
        Err(invalid_rule(format!(
            "{month} {day} is not a calendar date"
        )))
    }
}

/// Calendar date for a recurring month/day in `year`; `None` only for
/// February 29 in a common year.
fn recurring_date(year: i32, month: Month, day: u8) -> crate::Result<Option<Date>> {
    validate_month_day(month, day)?;
    Ok(Date::from_calendar_date(year, month, day).ok())
}

/// Required table lookup: a missing year is an error, not an absent holiday.
fn required(value: Option<Date>, table: &str, year: i32) -> crate::Result<Date> {
    value.ok_or_else(|| {
        crate::Error::from(crate::InputError::NotFound {
            id: format!("{table} table entry for year {year}"),
        })
    })
}

/// Nth weekday of a month without stepping outside the month: `n` in
/// `1..=5` counts from the 1st, `-5..=-1` from the last day. A nonexistent
/// fifth occurrence is a valid empty result; any other ordinal is invalid.
fn nth_weekday(year: i32, month: Month, weekday: Weekday, n: i8) -> crate::Result<Option<Date>> {
    if !(1..=5).contains(&n) && !(-5..=-1).contains(&n) {
        return Err(invalid_rule(format!(
            "weekday ordinal {n} is outside ±1..=5"
        )));
    }
    let last_day = month.length(year);
    let target = i64::from(weekday.number_days_from_monday());
    let day = if n > 0 {
        let Ok(first) = Date::from_calendar_date(year, month, 1) else {
            return Ok(None);
        };
        let offset = (target - i64::from(first.weekday().number_days_from_monday())).rem_euclid(7);
        1 + offset + 7 * (i64::from(n) - 1)
    } else {
        let Ok(last) = Date::from_calendar_date(year, month, last_day) else {
            return Ok(None);
        };
        let offset = (i64::from(last.weekday().number_days_from_monday()) - target).rem_euclid(7);
        i64::from(last_day) - offset - 7 * (i64::from(-n) - 1)
    };
    if day < 1 || day > i64::from(last_day) {
        return Ok(None);
    }
    Ok(u8::try_from(day)
        .ok()
        .and_then(|day| Date::from_calendar_date(year, month, day).ok()))
}

/// Gregorian Easter Monday for a positive representable year; an active
/// Easter rule outside that domain is an error.
fn easter_monday(year: i32) -> crate::Result<Date> {
    if year <= 0 {
        return Err(invalid_rule(format!(
            "Gregorian Easter is not supported for year {year}"
        )));
    }
    algo::easter_monday(year).ok_or_else(|| {
        invalid_rule(format!(
            "Easter Monday for year {year} is not representable"
        ))
    })
}

/// Day offsets, relative to January 1 of the nominal year, that bound every
/// date a rule can emit. Used to decide which nominal years can contribute
/// to a requested calendar year.
type Reach = (i64, i64);

/// Inclusive day-of-year window (0-based, leap-tolerant) of a month.
fn month_reach(month: Month) -> Reach {
    let start = i64::from(Date::from_calendar_date(2023, month, 1).map_or(0, |d| d.ordinal())) - 1;
    let end = start + i64::from(month.length(2024));
    (start, end)
}

/// Day-of-year window (0-based) of a fixed month/day, with one day of leap
/// slack after February.
fn day_reach(month: Month, day: u8) -> Reach {
    let common = Date::from_calendar_date(2023, month, day)
        .or_else(|_| Date::from_calendar_date(2024, month, day))
        .map_or(0, |d| i64::from(d.ordinal()) - 1);
    (common, common + 1)
}

/// Contribution windows of the table-backed lunar festivals, checked against
/// the generated tables by the `lunar_tables_stay_inside_declared_windows`
/// test.
pub(crate) const CNY_REACH: Reach = (19, 52); // ~Jan 20 – Feb 21
pub(crate) const QING_MING_REACH: Reach = (91, 97); // ~Apr 2 – Apr 7
pub(crate) const DRAGON_BOAT_REACH: Reach = (145, 178); // ~May 26 – Jun 27
pub(crate) const MID_AUTUMN_REACH: Reach = (248, 282); // ~Sep 6 – Oct 9

impl Rule {
    /// Bounds on the day offsets (from January 1 of the nominal year) of the
    /// dates this rule can emit.
    fn reach(&self) -> crate::Result<Reach> {
        Ok(match self {
            Rule::ExactDate { month, day, .. } => day_reach(*month, *day),
            Rule::NthWeekday { month, .. } => month_reach(*month),
            Rule::Fixed { month, day, .. } => {
                let (lo, hi) = day_reach(*month, *day);
                (lo - 1, hi + 2)
            }
            Rule::WeekdayShift { month, day, .. } => {
                let (lo, hi) = day_reach(*month, *day);
                (lo - 6, hi + 6)
            }
            // Easter Monday falls between March 23 and April 26.
            Rule::EasterOffset(offset) => (80 + i64::from(*offset), 117 + i64::from(*offset)),
            Rule::Span { start, len, offset } => {
                if *len == 0 {
                    return Err(invalid_rule("span length must be 1..=255"));
                }
                let (lo, hi) = start.reach()?;
                (
                    lo + i64::from(*offset),
                    hi + i64::from(*offset) + i64::from(*len) - 1,
                )
            }
            Rule::ChineseNewYear => CNY_REACH,
            Rule::QingMing => QING_MING_REACH,
            Rule::DragonBoat => DRAGON_BOAT_REACH,
            Rule::MidAutumn => MID_AUTUMN_REACH,
            // Jurisdiction rules emit dates only within their own nominal
            // year (substitutes and collisions resolve before December 31),
            // so a nominal year never contributes to a neighbouring one.
            Rule::HongKongPublicHolidays | Rule::JapanPublicHolidays => (0, 364),
            Rule::Effective { inner, .. } => inner.reach()?,
            Rule::ChinaBridge { festival } => {
                let (lo, hi) = festival.reach()?;
                (lo - 1, hi + 2)
            }
        })
    }

    /// Materialize this rule's occurrences for one nominal event year.
    ///
    /// Effective-range bounds gate the nominal year once, before any
    /// observation, offset, span or bridge; transforms operate on this
    /// year's occurrences only.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid definition, a required lunar/equinox
    /// table year that is missing, or an Easter rule outside positive years.
    pub(crate) fn materialize_nominal(&self, year: i32) -> crate::Result<Occurrences> {
        let mut out = Occurrences::new();
        match self {
            Rule::ExactDate {
                year: exact_year,
                month,
                day,
            } => {
                if year == *exact_year {
                    out.push(Date::from_calendar_date(year, *month, *day).map_err(|_| {
                        invalid_rule(format!("{year}-{month}-{day} is not a calendar date"))
                    })?);
                }
            }
            Rule::Fixed {
                month,
                day,
                observed,
            } => {
                if let Some(base) = recurring_date(year, *month, *day)? {
                    out.extend(apply_observed(base, *observed));
                }
            }
            Rule::NthWeekday { n, weekday, month } => {
                out.extend(nth_weekday(year, *month, *weekday, *n)?);
            }
            Rule::WeekdayShift {
                weekday,
                month,
                day,
                dir,
            } => {
                if let Some(base) = recurring_date(year, *month, *day)? {
                    out.extend(shift_to_weekday(base, *weekday, *dir));
                }
            }
            Rule::EasterOffset(offset) => {
                out.extend(shift(easter_monday(year)?, i64::from(*offset)));
            }
            Rule::Span { start, len, offset } => {
                if *len == 0 {
                    return Err(invalid_rule("span length must be 1..=255"));
                }
                for anchor in start.materialize_nominal(year)? {
                    for k in 0..i64::from(*len) {
                        out.extend(shift(anchor, i64::from(*offset) + k));
                    }
                }
            }
            Rule::ChineseNewYear => {
                out.push(required(algo::cny_date(year), "Chinese New Year", year)?)
            }
            Rule::DragonBoat => {
                out.push(required(algo::dragon_boat_date(year), "Dragon Boat", year)?)
            }
            Rule::MidAutumn => out.push(required(algo::mid_autumn_date(year), "Mid-Autumn", year)?),
            Rule::QingMing => out.push(required(algo::qing_ming_date(year), "Qing Ming", year)?),
            Rule::HongKongPublicHolidays => super::jurisdictions::hong_kong(year, &mut out)?,
            Rule::JapanPublicHolidays => super::jurisdictions::japan(year, &mut out)?,
            Rule::Effective {
                from_year,
                to_year,
                inner,
            } => {
                if year_in_effective_range(year, *from_year, *to_year) {
                    out = inner.materialize_nominal(year)?;
                }
            }
            Rule::ChinaBridge { festival } => {
                let occurrences = festival.materialize_nominal(year)?;
                if occurrences.len() > 1 {
                    return Err(invalid_rule(
                        "a China bridge needs at most one festival occurrence per year",
                    ));
                }
                for date in occurrences {
                    push_china_bridge_block(&mut out, date);
                }
            }
        }
        Ok(out)
    }

    /// Append every closure this rule contributes to calendar year `year`.
    ///
    /// All nominal years whose occurrence window can reach `year` are
    /// materialized once each, then the closures are filtered to `year`.
    /// A nominal year whose window cannot reach `year` is never evaluated, so
    /// a missing table for an irrelevant neighbour is not an error.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::materialize_nominal`] errors for contributing
    /// years, and errors when a contributing nominal year is outside the
    /// representable date range.
    pub(crate) fn dates_in_year<A: smallvec::Array<Item = Date>>(
        &self,
        year: i32,
        out: &mut smallvec::SmallVec<A>,
    ) -> crate::Result<()> {
        let (lo, hi) = self.reach()?;
        let jan1 = |y: i64| -> i64 {
            // Days from 0000-01-01 in the proleptic Gregorian calendar.
            let y1 = y - 1;
            365 * y + y1.div_euclid(4) - y1.div_euclid(100) + y1.div_euclid(400)
        };
        let request_start = jan1(i64::from(year));
        let request_end = jan1(i64::from(year) + 1) - 1;
        let span_years = (hi.max(-lo) / 365) + 2;
        for nominal in (i64::from(year) - span_years)..=(i64::from(year) + span_years) {
            let first = jan1(nominal) + lo;
            let last = jan1(nominal) + hi;
            if last < request_start || first > request_end {
                continue;
            }
            let nominal = i32::try_from(nominal)
                .ok()
                .filter(|y| (Date::MIN.year()..=Date::MAX.year()).contains(y))
                .ok_or_else(|| {
                    invalid_rule(format!(
                        "nominal year {nominal} needed for {year} is not representable"
                    ))
                })?;
            out.extend(
                self.materialize_nominal(nominal)?
                    .into_iter()
                    .filter(|date| date.year() == year),
            );
        }
        Ok(())
    }

    /// Whether `date` is a closure of this rule.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::dates_in_year`] errors.
    pub(crate) fn applies(&self, date: Date) -> crate::Result<bool> {
        let mut holidays = smallvec::SmallVec::<[Date; 32]>::new();
        self.dates_in_year(date.year(), &mut holidays)?;
        Ok(holidays.contains(&date))
    }
}

// China 连休 (bridge) block materialization helper
#[inline]
fn push_china_bridge_block<A: smallvec::Array<Item = Date>>(
    out: &mut smallvec::SmallVec<A>,
    festival: Date,
) {
    let days: &[i64] = match festival.weekday() {
        // Wednesday: single day. Mon/Fri: the festival plus a same-side weekend
        // that is already a non-business day, so only the festival is emitted.
        Weekday::Monday | Weekday::Wednesday | Weekday::Friday => &[0],
        // Tuesday: bridge the preceding Monday.
        Weekday::Tuesday => &[-1, 0],
        // Thursday: bridge the following Friday.
        Weekday::Thursday => &[0, 1],
        // Weekend: substitute the following Monday.
        Weekday::Saturday => &[2],
        Weekday::Sunday => &[1],
    };
    for &delta in days {
        out.extend(shift(festival, delta));
    }
}
