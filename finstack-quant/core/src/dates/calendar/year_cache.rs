//! Process-wide lazy year holiday bitset and business-day prefix sums.
//!
//! Built-in calendars evaluate a linear rule list per date. Materializing each
//! year once into a 366-bit holiday mask plus an inclusive business-day prefix
//! makes `is_holiday` O(1) and Bus/252 counting O(years) inside the cache
//! range [`BASE_YEAR`, `END_YEAR`].

use std::sync::{OnceLock, RwLock};

fn recover<T>(res: Result<T, std::sync::PoisonError<T>>) -> T {
    res.unwrap_or_else(std::sync::PoisonError::into_inner)
}
use time::{Date, Duration};

use super::algo::{BASE_YEAR, END_YEAR};
use super::business_days::HolidayCalendar;
use super::types::Calendar;
use crate::HashMap;

/// 366-day holiday mask plus inclusive business-day prefix sums for one year.
#[derive(Clone, Copy)]
struct YearBits {
    /// Bit `ordinal - 1` is set when that day is a holiday under
    /// [`Calendar::holiday_from_rules`].
    holiday: [u64; 6],
    /// `bd_prefix[i]` is the number of business days in ordinals `1..=i`.
    /// `bd_prefix[0]` is 0.
    bd_prefix: [u16; 367],
    /// Last valid ordinal in this year (365 or 366).
    last_ordinal: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct YearKey {
    id: &'static str,
    year: i32,
    ignore_weekends: bool,
    weekend_rule: super::types::WeekendRule,
    /// Static rule slice identity (`as_ptr` / `len`), not a raw pointer (Send+Sync).
    rules_addr: usize,
    rules_len: usize,
}

fn cache() -> &'static RwLock<HashMap<YearKey, YearBits>> {
    static CACHE: OnceLock<RwLock<HashMap<YearKey, YearBits>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::default()))
}

impl YearBits {
    #[inline]
    fn is_holiday(self, ordinal: u16) -> bool {
        let i = usize::from(ordinal.saturating_sub(1));
        let word = i / 64;
        let bit = i % 64;
        self.holiday.get(word).is_some_and(|w| (*w >> bit) & 1 == 1)
    }

    #[inline]
    fn count_range(self, from_inclusive: u16, to_exclusive: u16) -> u16 {
        if to_exclusive <= from_inclusive {
            return 0;
        }
        let hi = usize::from(to_exclusive.saturating_sub(1));
        let lo = usize::from(from_inclusive.saturating_sub(1));
        self.bd_prefix[hi].saturating_sub(self.bd_prefix[lo])
    }
}

fn set_holiday_bit(bits: &mut [u64; 6], ordinal: u16) {
    let i = usize::from(ordinal.saturating_sub(1));
    let word = i / 64;
    let bit = i % 64;
    if let Some(slot) = bits.get_mut(word) {
        *slot |= 1_u64 << bit;
    }
}

fn materialize(cal: &Calendar, year: i32) -> YearBits {
    let mut holiday = [0_u64; 6];
    let mut bd_prefix = [0_u16; 367];
    let mut bd = 0_u16;
    let mut last_ordinal = 0_u16;

    for ordinal in 1_u16..=366 {
        let Some(date) = Date::from_ordinal_date(year, ordinal).ok() else {
            break;
        };
        last_ordinal = ordinal;
        let is_hol = cal.holiday_from_rules(date);
        if is_hol {
            set_holiday_bit(&mut holiday, ordinal);
        }
        if !cal.weekend_rule.is_weekend(date.weekday()) && !is_hol {
            bd = bd.saturating_add(1);
        }
        bd_prefix[usize::from(ordinal)] = bd;
    }

    YearBits {
        holiday,
        bd_prefix,
        last_ordinal,
    }
}

fn year_bits(cal: &Calendar, year: i32) -> YearBits {
    let key = YearKey {
        id: cal.id,
        year,
        ignore_weekends: cal.ignore_weekends,
        weekend_rule: cal.weekend_rule,
        rules_addr: cal.rules.as_ptr() as usize,
        rules_len: cal.rules.len(),
    };
    if let Some(bits) = recover(cache().read()).get(&key).copied() {
        return bits;
    }
    let mut cache = recover(cache().write());
    if let Some(bits) = cache.get(&key).copied() {
        return bits;
    }
    let bits = materialize(cal, year);
    cache.insert(key, bits);
    bits
}

/// Holiday lookup via the year bitset. Caller must keep `date` inside
/// [`BASE_YEAR`, `END_YEAR`].
#[inline]
pub(super) fn is_holiday_cached(cal: &Calendar, date: Date) -> bool {
    year_bits(cal, date.year()).is_holiday(date.ordinal())
}

fn count_by_scan(cal: &Calendar, start: Date, end: Date) -> i32 {
    let mut count = 0_i32;
    let mut current = start;
    while current < end {
        if cal.is_business_day(current) {
            count += 1;
        }
        current += Duration::days(1);
    }
    count
}

/// Business days in `[start, end)` using per-year prefix sums when the
/// half-open interval lies inside the cache year range.
pub(super) fn count_business_days_cached(cal: &Calendar, start: Date, end: Date) -> i32 {
    if start >= end {
        return 0;
    }
    let last = end - Duration::days(1);
    if start.year() < BASE_YEAR || last.year() > END_YEAR {
        return count_by_scan(cal, start, end);
    }

    let mut total = 0_i32;
    let mut year = start.year();
    let last_year = last.year();
    while year <= last_year {
        let bits = year_bits(cal, year);
        let from = if year == start.year() {
            start.ordinal()
        } else {
            1
        };
        let to_exclusive = if year == end.year() {
            end.ordinal()
        } else {
            bits.last_ordinal.saturating_add(1)
        };
        total += i32::from(bits.count_range(from, to_exclusive));
        year += 1;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::calendar::rule::Rule;
    use crate::dates::calendar::{Calendar, WeekendRule, BSE, CNBE, NYSE, TARGET2};
    use crate::dates::HolidayCalendar;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    struct DefaultScan<'a>(&'a Calendar);

    impl HolidayCalendar for DefaultScan<'_> {
        fn is_holiday(&self, date: Date) -> bool {
            self.0.is_holiday(date)
        }

        fn is_business_day(&self, date: Date) -> bool {
            self.0.is_business_day(date)
        }
    }

    fn assert_cached_count_matches_default_scan(
        name: &str,
        cal: &Calendar,
        start: Date,
        end: Date,
    ) {
        let cached = cal.count_business_days(start, end);
        let scanned = DefaultScan(cal).count_business_days(start, end);
        assert_eq!(
            cached, scanned,
            "{name}: cached count {cached} != default scan {scanned} for {start}..{end}"
        );
    }

    #[test]
    fn year_bits_match_rule_scan_for_high_rule_calendars() {
        for cal in [&CNBE, &BSE, &NYSE, &TARGET2] {
            for year in [2024, 2025, 2028] {
                for ordinal in 1_u16..=366 {
                    let Some(d) = Date::from_ordinal_date(year, ordinal).ok() else {
                        break;
                    };
                    let rule_holiday = cal.holiday_from_rules(d);
                    assert_eq!(
                        is_holiday_cached(cal, d),
                        rule_holiday,
                        "{} {} holiday bit must match the rule scan",
                        cal.id,
                        d
                    );
                    let expected_public_holiday = rule_holiday
                        && !(cal.ignore_weekends && cal.weekend_rule.is_weekend(d.weekday()));
                    assert_eq!(
                        cal.is_holiday(d),
                        expected_public_holiday,
                        "{} {} public holiday must preserve the weekend override",
                        cal.id,
                        d
                    );
                    assert_eq!(
                        cal.is_business_day(d),
                        !cal.weekend_rule.is_weekend(d.weekday()) && !rule_holiday,
                        "{} {} business-day bit must match weekend + rule scan",
                        cal.id,
                        d
                    );
                }
            }
        }
    }

    #[test]
    fn cached_counts_match_default_scan_for_interval_shapes() {
        let cases = [
            (
                "single year",
                date(2025, Month::January, 2),
                date(2025, Month::March, 10),
            ),
            (
                "cross year",
                date(2024, Month::December, 15),
                date(2026, Month::March, 10),
            ),
            (
                "empty",
                date(2025, Month::January, 6),
                date(2025, Month::January, 6),
            ),
            (
                "reversed",
                date(2025, Month::January, 7),
                date(2025, Month::January, 6),
            ),
            (
                "out of range",
                date(1969, Month::December, 20),
                date(1970, Month::January, 10),
            ),
        ];

        for (name, start, end) in cases {
            assert_cached_count_matches_default_scan(name, &NYSE, start, end);
        }
    }

    #[test]
    fn year_cache_distinguishes_same_id_different_rule_slices() {
        static RULES_JAN1: [Rule; 1] = [Rule::fixed(Month::January, 1)];
        static RULES_JULY4: [Rule; 1] = [Rule::fixed(Month::July, 4)];

        let cal_jan1 = Calendar::new("dup_id", "Jan 1 only", false, &RULES_JAN1);
        let cal_july4 = Calendar::new("dup_id", "July 4 only", false, &RULES_JULY4);

        let jan1 = date(2025, Month::January, 1);
        let july4 = date(2025, Month::July, 4);

        // Populate both years in the process-wide cache.
        assert!(is_holiday_cached(&cal_jan1, jan1));
        assert!(is_holiday_cached(&cal_july4, july4));

        assert!(is_holiday_cached(&cal_jan1, jan1));
        assert!(!is_holiday_cached(&cal_july4, jan1));
        assert!(!is_holiday_cached(&cal_jan1, july4));
        assert!(is_holiday_cached(&cal_july4, july4));
    }

    #[test]
    fn friday_saturday_cached_count_matches_default_scan() {
        let cal = Calendar::new("me_prefix", "Middle East prefix", true, &[])
            .with_weekend_rule(WeekendRule::FridaySaturday);
        let friday = date(2025, Month::January, 3);
        let next_friday = date(2025, Month::January, 10);
        // Fri/Sat weekend: Sun–Thu are business days → 5 days in [Fri, next Fri).
        assert_eq!(cal.count_business_days(friday, next_friday), 5);
        assert_cached_count_matches_default_scan(
            "Friday/Saturday weekend",
            &cal,
            friday,
            next_friday,
        );
    }
}
