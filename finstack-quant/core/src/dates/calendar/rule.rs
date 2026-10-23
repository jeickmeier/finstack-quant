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

// China 连休 (bridge) block materialization helper
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

impl Rule {
    /// Returns `true` when the rule marks `date` a holiday.
    ///
    /// A membership test over [`Rule::dates_in_year`] for `date`'s year, so
    /// every rule shape has one evaluator.
    pub fn applies(&self, date: Date) -> bool {
        let mut holidays = smallvec::SmallVec::<[Date; 32]>::new();
        self.dates_in_year(date.year(), &mut holidays);
        holidays.contains(&date)
    }
}

impl Rule {
    /// Append the dates in `year` that this rule marks as holidays into `out`.
    ///
    /// Observance and spans can place a holiday in a neighbouring year's
    /// materialization, so the base years either side of `year` are evaluated too
    /// and only the dates landing in `year` are kept. A rule whose offset reaches
    /// further than one year boundary is not covered. `Effective` gates on `year`,
    /// the date's own year. No deduplication is performed.
    pub(crate) fn dates_in_year<A: smallvec::Array<Item = Date>>(
        &self,
        year: i32,
        out: &mut smallvec::SmallVec<A>,
    ) {
        if let Rule::Effective {
            from_year,
            to_year,
            inner,
        } = self
        {
            if year_in_effective_range(year, *from_year, *to_year) {
                inner.dates_in_year(year, out);
            }
            return;
        }
        let mut candidates = smallvec::SmallVec::<[Date; 64]>::new();
        for base in [year - 1, year, year + 1] {
            self.materialize_year(base, &mut candidates);
        }
        out.extend(candidates.into_iter().filter(|date| date.year() == year));
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
                if let Some(d) =
                    crate::dates::calendar::algo::nth_weekday_of_month(year, *month, *weekday, *n)
                {
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
                if let Some(target) = algo::easter_monday(year)
                    .and_then(|em| em.checked_add(Duration::days(i64::from(*offset))))
                {
                    out.push(target);
                }
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
                if let Some(d) = algo::qing_ming_date(year) {
                    out.push(d);
                }
            }
            Rule::HongKongPublicHolidays => super::jurisdictions::hong_kong(year, out),
            Rule::JapanPublicHolidays => super::jurisdictions::japan(year, out),
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
