//! Calendar rule implementation tests
//!
//! Tests for all rule types:
//! - Fixed date rules with observation modes
//! - Nth weekday rules (positive and negative)
//! - Weekday shift rules (before/after)
//! - Easter-based rules (offset from Easter Monday)
//! - Chinese calendar rules (CNY, QingMing, Buddha's Birthday)
//! - Japanese equinox rules (Vernal, Autumnal)
//! - Span rules (multi-day holidays)

use finstack_quant_core::dates::{Date, HolidayCalendar};
use finstack_quant_core::dates::{Direction, Observed, Rule};
use smallvec::SmallVec;
use time::{Month, Weekday};

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

// ============================================
// Fixed Date Rules
// ============================================

#[test]
fn rule_fixed_no_observation() {
    let rule = Rule::fixed(Month::July, 4);

    // Should match exact date in any year
    assert!(rule.applies(make_date(2025, 7, 4)));
    assert!(rule.applies(make_date(2024, 7, 4)));
    assert!(rule.applies(make_date(2026, 7, 4)));

    // Should not match other dates
    assert!(!rule.applies(make_date(2025, 7, 5)));
    assert!(!rule.applies(make_date(2025, 7, 3)));
    assert!(!rule.applies(make_date(2025, 6, 4)));
}

#[test]
fn rule_fixed_next_monday() {
    let rule = Rule::Fixed {
        month: Month::January,
        day: 1,
        observed: Observed::NextMonday,
    };

    // 2023: Jan 1 is Sunday, observed on Monday Jan 2
    assert!(!rule.applies(make_date(2023, 1, 1)));
    assert!(rule.applies(make_date(2023, 1, 2)));

    // 2024: Jan 1 is Monday, observed same day
    assert!(rule.applies(make_date(2024, 1, 1)));

    // 2022: Jan 1 is Saturday, observed on Monday Jan 3
    assert!(!rule.applies(make_date(2022, 1, 1)));
    assert!(rule.applies(make_date(2022, 1, 3)));
}

#[test]
fn rule_fixed_fri_if_sat_mon_if_sun() {
    let rule = Rule::Fixed {
        month: Month::July,
        day: 4,
        observed: Observed::FriIfSatMonIfSun,
    };

    // 2026: July 4 is Saturday, observed on Friday July 3
    assert!(rule.applies(make_date(2026, 7, 3)));
    assert!(!rule.applies(make_date(2026, 7, 4)));

    // 2027: July 4 is Sunday, observed on Monday July 5
    assert!(!rule.applies(make_date(2027, 7, 4)));
    assert!(rule.applies(make_date(2027, 7, 5)));

    // 2025: July 4 is Friday, observed same day
    assert!(rule.applies(make_date(2025, 7, 4)));
}

#[test]
fn rule_convenience_constructors() {
    // Test convenience constructors
    let fixed = Rule::fixed(Month::December, 25);
    assert!(matches!(
        fixed,
        Rule::Fixed {
            observed: Observed::None,
            ..
        }
    ));

    let next_mon = Rule::fixed_next_monday(Month::January, 1);
    assert!(matches!(
        next_mon,
        Rule::Fixed {
            observed: Observed::NextMonday,
            ..
        }
    ));

    let weekend = Rule::fixed_weekend(Month::July, 4);
    assert!(matches!(
        weekend,
        Rule::Fixed {
            observed: Observed::FriIfSatMonIfSun,
            ..
        }
    ));
}

#[test]
fn rule_fixed_feb_29_leap_year() {
    let rule = Rule::Fixed {
        month: Month::February,
        day: 29,
        observed: Observed::None,
    };

    // Feb 29 exists in 2024 (leap year)
    assert!(rule.applies(make_date(2024, 2, 29)));

    // Feb 29 doesn't exist in 2023 - shouldn't match any date
    assert!(!rule.applies(make_date(2023, 2, 28)));
    assert!(!rule.applies(make_date(2023, 3, 1)));
}

#[test]
fn rule_fixed_materialize_feb_29_non_leap() {
    let rule = Rule::Fixed {
        month: Month::February,
        day: 29,
        observed: Observed::None,
    };

    // Non-leap year 2023
    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2023, &mut out);

    // Should not produce any dates (invalid date in non-leap year)
    assert!(
        out.is_empty() || out.iter().all(|d| d.year() != 2023),
        "Feb 29 in non-leap year should not produce valid 2023 dates"
    );
}

#[test]
fn rule_fixed_materialize_feb_29_leap() {
    let rule = Rule::Fixed {
        month: Month::February,
        day: 29,
        observed: Observed::None,
    };

    // Leap year 2024
    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2024, &mut out);

    // Should produce Feb 29, 2024
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], make_date(2024, 2, 29));
}

#[test]
fn rule_materialize_year_fixed() {
    let rule = Rule::fixed(Month::July, 4);
    let mut out = SmallVec::<[Date; 16]>::new();

    rule.materialize_year(2025, &mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0], make_date(2025, 7, 4));
}

// ============================================
// Nth Weekday Rules
// ============================================

#[test]
fn rule_nth_weekday_positive() {
    // Third Monday of January
    let rule = Rule::NthWeekday {
        n: 3,
        weekday: Weekday::Monday,
        month: Month::January,
    };

    // 2025: Third Monday is Jan 20
    assert!(rule.applies(make_date(2025, 1, 20)));
    assert!(!rule.applies(make_date(2025, 1, 13))); // Second Monday
    assert!(!rule.applies(make_date(2025, 1, 27))); // Fourth Monday
}

#[test]
fn rule_nth_weekday_negative() {
    // Last Monday of May
    let rule = Rule::NthWeekday {
        n: -1,
        weekday: Weekday::Monday,
        month: Month::May,
    };

    // 2025: Last Monday of May is May 26
    assert!(rule.applies(make_date(2025, 5, 26)));
    assert!(!rule.applies(make_date(2025, 5, 19))); // Second-to-last Monday
}

#[test]
fn rule_nth_weekday_fifth_occurrence() {
    // 5th Monday if exists
    let rule = Rule::NthWeekday {
        n: 5,
        weekday: Weekday::Monday,
        month: Month::December,
    };

    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2025, &mut out);

    // December 2025 has 5 Mondays: 1, 8, 15, 22, 29
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], make_date(2025, 12, 29));
}

#[test]
fn rule_nth_weekday_second_to_last() {
    // Second-to-last Friday
    let rule = Rule::NthWeekday {
        n: -2,
        weekday: Weekday::Friday,
        month: Month::November,
    };

    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2025, &mut out);

    assert_eq!(out.len(), 1);
    // November 2025: Fridays are 7, 14, 21, 28
    // Second-to-last = 21st
    assert_eq!(out[0], make_date(2025, 11, 21));
}

#[test]
fn rule_materialize_year_nth_weekday() {
    let rule = Rule::NthWeekday {
        n: 4,
        weekday: Weekday::Thursday,
        month: Month::November,
    };
    let mut out = SmallVec::<[Date; 16]>::new();

    rule.materialize_year(2025, &mut out);

    assert_eq!(out.len(), 1);
    // 2025: Fourth Thursday of November is Nov 27
    assert_eq!(out[0], make_date(2025, 11, 27));
}

// ============================================
// Weekday Shift Rules
// ============================================

#[test]
fn rule_weekday_shift_after() {
    // First Monday on or after May 25
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Monday,
        month: Month::May,
        day: 25,
        dir: Direction::After,
    };

    // 2025: May 25 is Sunday, first Monday after is May 26
    assert!(rule.applies(make_date(2025, 5, 26)));
    assert!(!rule.applies(make_date(2025, 5, 25)));
}

#[test]
fn rule_weekday_shift_before() {
    // First Monday on or before May 25
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Monday,
        month: Month::May,
        day: 25,
        dir: Direction::Before,
    };

    // 2025: May 25 is Sunday, first Monday before is May 19
    assert!(rule.applies(make_date(2025, 5, 19)));
    assert!(!rule.applies(make_date(2025, 5, 25)));
    assert!(!rule.applies(make_date(2025, 5, 26)));
}

#[test]
fn rule_weekday_shift_on_weekday() {
    // If base date is already the target weekday
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Monday,
        month: Month::January,
        day: 6, // Jan 6, 2025 is already Monday
        dir: Direction::After,
    };

    assert!(rule.applies(make_date(2025, 1, 6)));
}

#[test]
fn rule_weekday_shift_materialize() {
    // First Tuesday on or after Nov 2 (US Election Day)
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Tuesday,
        month: Month::November,
        day: 2,
        dir: Direction::After,
    };

    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2024, &mut out);

    // 2024: Nov 2 is Saturday, first Tuesday after is Nov 5
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], make_date(2024, 11, 5));
    assert_eq!(out[0].weekday(), Weekday::Tuesday);
}

#[test]
fn rule_weekday_shift_before_materialize() {
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Friday,
        month: Month::June,
        day: 15,
        dir: Direction::Before,
    };

    let mut out = SmallVec::<[Date; 1]>::new();
    rule.materialize_year(2025, &mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].weekday(), Weekday::Friday);
    // Should be on or before June 15
    assert!(out[0] <= make_date(2025, 6, 15));
}

#[test]
fn direction_after_finds_same_day() {
    // If reference is already the target weekday, After should find it
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Wednesday,
        month: Month::January,
        day: 1, // 2025: Jan 1 is Wednesday
        dir: Direction::After,
    };

    assert!(rule.applies(make_date(2025, 1, 1)));
}

#[test]
fn direction_before_finds_same_day() {
    // If reference is already the target weekday, Before should find it
    let rule = Rule::WeekdayShift {
        weekday: Weekday::Wednesday,
        month: Month::January,
        day: 1, // 2025: Jan 1 is Wednesday
        dir: Direction::Before,
    };

    assert!(rule.applies(make_date(2025, 1, 1)));
}

// ============================================
// Easter-Based Rules
// ============================================

#[test]
fn rule_easter_offset() {
    // Good Friday is Easter Monday - 3 days
    let rule = Rule::EasterOffset(-3);

    // 2025: Easter Monday is April 21, Good Friday is April 18
    assert!(rule.applies(make_date(2025, 4, 18)));
    assert!(!rule.applies(make_date(2025, 4, 21)));

    // 2024: Easter Monday is April 1, Good Friday is March 29
    assert!(rule.applies(make_date(2024, 3, 29)));
}

#[test]
fn rule_materialize_year_easter_offset() {
    let rule = Rule::EasterOffset(-3); // Good Friday
    let mut out = SmallVec::<[Date; 16]>::new();

    rule.materialize_year(2025, &mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0], make_date(2025, 4, 18));
}

#[test]
fn rule_easter_known_dates_2020_2030() {
    // Known Easter Monday dates (authoritative reference from astronomical calculations)
    let known_easter_mondays = [
        (2020, 4, 13),
        (2021, 4, 5),
        (2022, 4, 18),
        (2023, 4, 10),
        (2024, 4, 1),
        (2025, 4, 21),
        (2026, 4, 6),
        (2027, 3, 29),
        (2028, 4, 17),
        (2029, 4, 2),
        (2030, 4, 22),
    ];

    let rule = Rule::EasterOffset(0); // Easter Monday

    for (year, month, day) in known_easter_mondays {
        let expected = make_date(year, month, day);
        assert!(
            rule.applies(expected),
            "Easter Monday {} should be {:?}",
            year,
            expected
        );
    }
}

#[test]
fn rule_good_friday_is_3_days_before_easter_monday() {
    let good_friday = Rule::EasterOffset(-3);
    let easter_monday = Rule::EasterOffset(0);

    for year in 2020..=2030 {
        let mut gf_out = SmallVec::<[Date; 1]>::new();
        let mut em_out = SmallVec::<[Date; 1]>::new();

        good_friday.materialize_year(year, &mut gf_out);
        easter_monday.materialize_year(year, &mut em_out);

        assert_eq!(gf_out.len(), 1);
        assert_eq!(em_out.len(), 1);

        let diff = (em_out[0] - gf_out[0]).whole_days();
        assert_eq!(
            diff, 3,
            "Good Friday should be 3 days before Easter Monday in {}",
            year
        );
    }
}

#[test]
fn rule_easter_sunday_is_1_day_before_easter_monday() {
    let easter_sunday = Rule::EasterOffset(-1);
    let easter_monday = Rule::EasterOffset(0);

    for year in 2020..=2030 {
        let mut sun_out = SmallVec::<[Date; 1]>::new();
        let mut mon_out = SmallVec::<[Date; 1]>::new();

        easter_sunday.materialize_year(year, &mut sun_out);
        easter_monday.materialize_year(year, &mut mon_out);

        assert_eq!(sun_out.len(), 1);
        assert_eq!(mon_out.len(), 1);

        let diff = (mon_out[0] - sun_out[0]).whole_days();
        assert_eq!(
            diff, 1,
            "Easter Sunday should be 1 day before Easter Monday in {}",
            year
        );

        // Easter Sunday should always be a Sunday
        assert_eq!(
            sun_out[0].weekday(),
            time::Weekday::Sunday,
            "Easter {} should fall on Sunday",
            year
        );
    }
}

// ============================================
// Chinese Calendar Rules
// ============================================

#[test]
fn rule_qing_ming() {
    let rule = Rule::QingMing;

    // Qing Ming is typically around April 4-5
    // 2024: April 4
    assert!(rule.applies(make_date(2024, 4, 4)));

    // 2025: April 4
    assert!(rule.applies(make_date(2025, 4, 4)));
}

#[test]
fn rule_buddhas_birthday() {
    let rule = Rule::BuddhasBirthday;

    // Buddha's Birthday is approximately CNY + 95 days
    // 2024: CNY is Feb 10, so Buddha's Birthday ~ May 15
    let bb_2024 = make_date(2024, 2, 10)
        .checked_add(time::Duration::days(95))
        .unwrap();
    assert!(rule.applies(bb_2024));
}

// ============================================
// Japanese Equinox Rules
// ============================================

#[test]
fn rule_vernal_equinox_jp() {
    let rule = Rule::VernalEquinoxJP;

    // Vernal equinox in Japan is typically around March 20-21
    // Test that it produces a valid date in that range
    for year in 2020..2030 {
        let mut out = SmallVec::<[Date; 1]>::new();
        rule.materialize_year(year, &mut out);

        assert_eq!(out.len(), 1);
        let date = out[0];
        assert_eq!(date.month(), Month::March);
        assert!(date.day() >= 19 && date.day() <= 21);
    }
}

#[test]
fn rule_vernal_equinox_jp_applies() {
    let rule = Rule::VernalEquinoxJP;

    // Test specific known dates for vernal equinox
    // 2024: March 20
    assert!(rule.applies(make_date(2024, 3, 20)));

    // Non-equinox dates should not match
    assert!(!rule.applies(make_date(2024, 3, 19)));
    assert!(!rule.applies(make_date(2024, 3, 22)));
}

// ============================================
// Span Rules (Multi-Day Holidays)
// ============================================

// A static start rule for Dec 31 each year
static DEC31: Rule = Rule::fixed(Month::December, 31);

#[test]
fn span_len2_cross_year() {
    let rules: &[Rule] = &[Rule::Span {
        start: &DEC31,
        len: 2,
        offset: 0,
    }];

    let dec31 = Date::from_calendar_date(2024, Month::December, 31).unwrap();
    let jan01 = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let jan02 = Date::from_calendar_date(2025, Month::January, 2).unwrap();

    assert!(rules.is_holiday(dec31));
    assert!(rules.is_holiday(jan01));
    assert!(!rules.is_holiday(jan02));
}

#[test]
fn span_len3_cross_year() {
    let rules: &[Rule] = &[Rule::Span {
        start: &DEC31,
        len: 3,
        offset: 0,
    }];

    let dec31 = Date::from_calendar_date(2024, Month::December, 31).unwrap();
    let jan01 = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let jan02 = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let jan03 = Date::from_calendar_date(2025, Month::January, 3).unwrap();

    assert!(rules.is_holiday(dec31));
    assert!(rules.is_holiday(jan01));
    assert!(rules.is_holiday(jan02));
    assert!(!rules.is_holiday(jan03));
}

// ============================================
// Observed Edge Cases
// ============================================

#[test]
fn rule_observed_friday_for_saturday() {
    let rule = Rule::Fixed {
        month: Month::July,
        day: 4,
        observed: Observed::FriIfSatMonIfSun,
    };

    // 2020: July 4 is Saturday
    // Observed on Friday July 3
    assert!(rule.applies(make_date(2020, 7, 3)));
    assert!(!rule.applies(make_date(2020, 7, 4)));
    assert!(!rule.applies(make_date(2020, 7, 6)));
}

#[test]
fn rule_observed_monday_for_sunday() {
    let rule = Rule::Fixed {
        month: Month::July,
        day: 4,
        observed: Observed::FriIfSatMonIfSun,
    };

    // 2021: July 4 is Sunday
    // Observed on Monday July 5
    assert!(rule.applies(make_date(2021, 7, 5)));
    assert!(!rule.applies(make_date(2021, 7, 4)));
    assert!(!rule.applies(make_date(2021, 7, 2)));
}

#[test]
fn rule_observed_next_monday_saturday() {
    let rule = Rule::Fixed {
        month: Month::December,
        day: 25,
        observed: Observed::NextMonday,
    };

    // 2021: Dec 25 is Saturday
    // UK style: observed on Monday Dec 27
    assert!(rule.applies(make_date(2021, 12, 27)));
    assert!(!rule.applies(make_date(2021, 12, 25)));
}

#[test]
fn rule_observed_next_monday_sunday() {
    let rule = Rule::Fixed {
        month: Month::December,
        day: 25,
        observed: Observed::NextMonday,
    };

    // 2022: Dec 25 is Sunday
    // UK style: observed on Monday Dec 26
    assert!(rule.applies(make_date(2022, 12, 26)));
    assert!(!rule.applies(make_date(2022, 12, 25)));
}

// ============================================
// Rule Combinations
// ============================================

#[test]
fn rule_slice_as_holiday_calendar() {
    let rules: &[Rule] = &[
        Rule::fixed(Month::January, 1),
        Rule::fixed(Month::December, 25),
    ];

    // Should implement HolidayCalendar trait
    assert!(rules.is_holiday(make_date(2025, 1, 1)));
    assert!(rules.is_holiday(make_date(2025, 12, 25)));
    assert!(!rules.is_holiday(make_date(2025, 7, 4)));
}

#[test]
fn rule_multiple_rules_combine() {
    let rules: &[Rule] = &[
        Rule::fixed(Month::January, 1),
        Rule::fixed(Month::July, 4),
        Rule::fixed(Month::December, 25),
        Rule::EasterOffset(-3),
    ];

    // New Year
    assert!(rules.is_holiday(make_date(2025, 1, 1)));

    // Independence Day
    assert!(rules.is_holiday(make_date(2025, 7, 4)));

    // Christmas
    assert!(rules.is_holiday(make_date(2025, 12, 25)));

    // Good Friday 2025
    assert!(rules.is_holiday(make_date(2025, 4, 18)));

    // Regular day
    assert!(!rules.is_holiday(make_date(2025, 6, 15)));
}

// ============================================
// Year-boundary observance
// ============================================

#[test]
fn rule_fixed_observance_crosses_year_boundary() {
    // OPM-style Fri-if-Sat rule on Jan 1: when Jan 1 falls on a Saturday
    // (e.g. 2022), the holiday is observed on Friday Dec 31 of the PRIOR
    // year. `applies()` must agree with `materialize_year()` for that date.
    let rule = Rule::Fixed {
        month: Month::January,
        day: 1,
        observed: Observed::FriIfSatMonIfSun,
    };

    // Jan 1, 2022 is a Saturday.
    assert_eq!(make_date(2022, 1, 1).weekday(), Weekday::Saturday);

    // materialize_year(2022) yields the observed date Dec 31, 2021.
    let mut out: SmallVec<[Date; 8]> = SmallVec::new();
    rule.materialize_year(2022, &mut out);
    assert_eq!(out.as_slice(), &[make_date(2021, 12, 31)]);

    // applies() on the materialized date must agree (previously false because
    // the base was reconstructed only from date.year() == 2021).
    assert!(rule.applies(make_date(2021, 12, 31)));

    // The unobserved Saturday itself is not a holiday under this rule.
    assert!(!rule.applies(make_date(2022, 1, 1)));

    // Sanity: a year where Jan 1 is a weekday is unaffected.
    assert!(rule.applies(make_date(2025, 1, 1)));
    assert!(!rule.applies(make_date(2024, 12, 31)));
}

// ============================================
// Nth weekday occurrence overflow
// ============================================

#[test]
fn rule_nth_weekday_overflow_does_not_spill_into_next_month() {
    // February 2025 has exactly four Mondays (3, 10, 17, 24). A "5th Monday
    // of February" rule must match nothing — previously the raw arithmetic
    // spilled to Monday Mar 3, 2025 and marked it a phantom holiday.
    let rule = Rule::NthWeekday {
        n: 5,
        weekday: Weekday::Monday,
        month: Month::February,
    };

    assert!(!rule.applies(make_date(2025, 3, 3)));

    let mut out: SmallVec<[Date; 8]> = SmallVec::new();
    rule.materialize_year(2025, &mut out);
    assert!(out.is_empty(), "no 5th Monday exists in Feb 2025: {out:?}");

    // A month that DOES have five Mondays still works (June 2025: 2, 9, 16,
    // 23, 30).
    let rule_june = Rule::NthWeekday {
        n: 5,
        weekday: Weekday::Monday,
        month: Month::June,
    };
    assert!(rule_june.applies(make_date(2025, 6, 30)));

    // Negative overflow: "-5th Monday" of Feb 2025 must not reach January.
    let rule_neg = Rule::NthWeekday {
        n: -5,
        weekday: Weekday::Monday,
        month: Month::February,
    };
    assert!(!rule_neg.applies(make_date(2025, 1, 27)));
    let mut out_neg: SmallVec<[Date; 8]> = SmallVec::new();
    rule_neg.materialize_year(2025, &mut out_neg);
    assert!(out_neg.is_empty());
}
