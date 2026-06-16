//! Additional coverage tests for calendar rules
//!
//! This module tests edge cases and less commonly used code paths

use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{Direction, Observed, Rule};
use smallvec::SmallVec;
use time::{Month, Weekday};

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

fn assert_applies(rule: &Rule, yes: &[(i32, u8, u8)], no: &[(i32, u8, u8)]) {
    for &(y, m, d) in yes {
        assert!(
            rule.applies(make_date(y, m, d)),
            "expected {}-{}-{} to match",
            y,
            m,
            d
        );
    }
    for &(y, m, d) in no {
        assert!(
            !rule.applies(make_date(y, m, d)),
            "expected {}-{}-{} to miss",
            y,
            m,
            d
        );
    }
}

fn materialize(rule: &Rule, year: i32) -> SmallVec<[Date; 32]> {
    let mut out = SmallVec::<[Date; 32]>::new();
    rule.materialize_year(year, &mut out);
    out
}

#[test]
fn span_rule_cases() {
    struct SpanCase {
        start: (Month, u8),
        len: u8,
        hits: &'static [(i32, u8, u8)],
        misses: &'static [(i32, u8, u8)],
        materialize_year: Option<i32>,
    }

    static CASES: &[SpanCase] = &[
        SpanCase {
            start: (Month::December, 24),
            len: 3,
            hits: &[(2025, 12, 24), (2025, 12, 25), (2025, 12, 26)],
            misses: &[(2025, 12, 23), (2025, 12, 27)],
            materialize_year: None,
        },
        SpanCase {
            start: (Month::December, 30),
            len: 5,
            hits: &[
                (2024, 12, 30),
                (2024, 12, 31),
                (2025, 1, 1),
                (2025, 1, 2),
                (2025, 1, 3),
            ],
            misses: &[(2025, 1, 4)],
            materialize_year: Some(2025),
        },
        SpanCase {
            start: (Month::January, 1),
            len: 0,
            hits: &[],
            misses: &[(2025, 1, 1), (2025, 1, 2)],
            materialize_year: Some(2025),
        },
        SpanCase {
            start: (Month::January, 1),
            len: 1,
            hits: &[(2025, 1, 1)],
            misses: &[(2025, 1, 2)],
            materialize_year: Some(2025),
        },
        SpanCase {
            start: (Month::April, 29),
            len: 5,
            hits: &[
                (2025, 4, 29),
                (2025, 4, 30),
                (2025, 5, 1),
                (2025, 5, 2),
                (2025, 5, 3),
            ],
            misses: &[],
            materialize_year: Some(2025),
        },
    ];

    for case in CASES {
        let start_rule: &'static Rule = Box::leak(Box::new(Rule::Fixed {
            month: case.start.0,
            day: case.start.1,
            observed: Observed::None,
        }));
        let rule = Rule::Span {
            start: start_rule,
            len: case.len,
        };
        assert_applies(&rule, case.hits, case.misses);

        if let Some(year) = case.materialize_year {
            let mats = materialize(&rule, year);
            for &(y, m, d) in case.hits.iter().filter(|(y, _, _)| *y == year) {
                assert!(
                    mats.contains(&make_date(y, m, d)),
                    "span should materialize {}-{}-{}",
                    y,
                    m,
                    d
                );
            }
        }
    }
}

#[test]
fn equinox_rules() {
    let vernal = Rule::VernalEquinoxJP;
    assert_applies(&vernal, &[(2024, 3, 20)], &[(2024, 3, 19), (2024, 3, 22)]);

    let autumnal = Rule::AutumnalEquinoxJP;
    for year in 2020..2030 {
        let eq_date = {
            let out = materialize(&autumnal, year);
            assert_eq!(out.len(), 1, "autumnal equinox should yield one date");
            out[0]
        };
        assert!(autumnal.applies(eq_date), "autumnal {} should apply", year);
        let prev = eq_date - time::Duration::days(1);
        let next = eq_date + time::Duration::days(1);
        assert!(!autumnal.applies(prev));
        assert!(!autumnal.applies(next));
    }
}

#[test]
fn buddhas_birthday_rules() {
    let rule = Rule::BuddhasBirthday;
    for year in 2020..2030 {
        let out = materialize(&rule, year);
        assert_eq!(out.len(), 1, "Buddha's Birthday should yield one date");
        let date = out[0];
        assert!(
            matches!(date.month(), Month::April | Month::May | Month::June),
            "Buddha's Birthday should be in Apr-Jun"
        );
        assert!(rule.applies(date));
        let prev = date - time::Duration::days(1);
        let next = date + time::Duration::days(1);
        assert!(!rule.applies(prev));
        assert!(!rule.applies(next));
    }
}

#[test]
fn qing_ming_rules() {
    let rule = Rule::QingMing;
    for year in 2020..2030 {
        let out = materialize(&rule, year);
        assert_eq!(out.len(), 1);
        let date = out[0];
        assert_eq!(date.month(), Month::April);
        assert!(
            (4..=6).contains(&date.day()),
            "Qing Ming should be April 4-6"
        );
    }
}

#[test]
fn chinese_new_year_rules() {
    let rule = Rule::ChineseNewYear;
    for year in 2020..2030 {
        let out = materialize(&rule, year);
        assert_eq!(out.len(), 1);
        let date = out[0];
        assert!(
            matches!(date.month(), Month::January | Month::February),
            "CNY should be Jan/Feb"
        );
    }

    let known = [
        (2020, 1, 25),
        (2021, 2, 12),
        (2022, 2, 1),
        (2023, 1, 22),
        (2024, 2, 10),
        (2025, 1, 29),
    ];
    assert_applies(&rule, &known, &[]);
}

#[test]
fn fixed_feb_29_rules() {
    let rule = Rule::Fixed {
        month: Month::February,
        day: 29,
        observed: Observed::None,
    };

    assert_applies(&rule, &[(2024, 2, 29)], &[(2023, 2, 28), (2023, 3, 1)]);

    let non_leap = materialize(&rule, 2023);
    assert!(
        non_leap.is_empty() || non_leap.iter().all(|d| d.year() != 2023),
        "non-leap year should not produce 2023 dates"
    );

    let leap = materialize(&rule, 2024);
    assert_eq!(leap.len(), 1);
    assert_eq!(leap[0], make_date(2024, 2, 29));
}

#[test]
fn equinox_rules_out_of_supported_range_do_not_fabricate_dates() {
    let vernal = Rule::VernalEquinoxJP;
    let autumnal = Rule::AutumnalEquinoxJP;

    let vernal_out = materialize(&vernal, 1800);
    let autumnal_out = materialize(&autumnal, 2201);

    assert!(vernal_out.is_empty());
    assert!(autumnal_out.is_empty());
    assert!(!vernal.applies(make_date(1800, 3, 20)));
    assert!(!autumnal.applies(make_date(2201, 9, 23)));
}

#[test]
fn weekday_shift_rules() {
    let after = Rule::WeekdayShift {
        weekday: Weekday::Tuesday,
        month: Month::November,
        day: 2,
        dir: Direction::After,
    };
    let after_out = materialize(&after, 2024);
    assert_eq!(after_out.len(), 1);
    assert_eq!(after_out[0], make_date(2024, 11, 5));
    assert_eq!(after_out[0].weekday(), Weekday::Tuesday);

    let before = Rule::WeekdayShift {
        weekday: Weekday::Friday,
        month: Month::June,
        day: 15,
        dir: Direction::Before,
    };
    let before_out = materialize(&before, 2025);
    assert_eq!(before_out.len(), 1);
    assert_eq!(before_out[0].weekday(), Weekday::Friday);
    assert!(before_out[0] <= make_date(2025, 6, 15));
}

#[test]
fn nth_weekday_rules() {
    let fifth_monday = Rule::NthWeekday {
        n: 5,
        weekday: Weekday::Monday,
        month: Month::December,
    };
    let fifth_out = materialize(&fifth_monday, 2025);
    assert_eq!(fifth_out.as_slice(), &[make_date(2025, 12, 29)]);

    let second_last_friday = Rule::NthWeekday {
        n: -2,
        weekday: Weekday::Friday,
        month: Month::November,
    };
    let sl_out = materialize(&second_last_friday, 2025);
    assert_eq!(sl_out.as_slice(), &[make_date(2025, 11, 21)]);
}

#[test]
fn easter_offset_rules() {
    let ascension = Rule::EasterOffset(38);
    let whit = Rule::EasterOffset(49);

    let ascension_out = materialize(&ascension, 2025);
    assert_eq!(ascension_out.len(), 1);
    assert_eq!(ascension_out[0], make_date(2025, 5, 29));
    assert_eq!(ascension_out[0].weekday(), Weekday::Thursday);

    let whit_out = materialize(&whit, 2025);
    assert_eq!(whit_out.len(), 1);
    assert_eq!(whit_out[0], make_date(2025, 6, 9));
    assert_eq!(whit_out[0].weekday(), Weekday::Monday);
}

#[test]
fn observed_variants() {
    let july4 = Rule::Fixed {
        month: Month::July,
        day: 4,
        observed: Observed::FriIfSatMonIfSun,
    };
    assert_applies(
        &july4,
        &[(2020, 7, 3), (2021, 7, 5)],
        &[(2020, 7, 4), (2021, 7, 4), (2021, 7, 2)],
    );

    let christmas = Rule::Fixed {
        month: Month::December,
        day: 25,
        observed: Observed::NextMonday,
    };
    assert_applies(
        &christmas,
        &[(2021, 12, 27), (2022, 12, 26)],
        &[(2021, 12, 25), (2022, 12, 25)],
    );
}

#[test]
fn direction_same_day() {
    let after = Rule::WeekdayShift {
        weekday: Weekday::Wednesday,
        month: Month::January,
        day: 1,
        dir: Direction::After,
    };
    let before = Rule::WeekdayShift {
        weekday: Weekday::Wednesday,
        month: Month::January,
        day: 1,
        dir: Direction::Before,
    };

    assert!(after.applies(make_date(2025, 1, 1)));
    assert!(before.applies(make_date(2025, 1, 1)));
}
