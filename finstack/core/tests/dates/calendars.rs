//! Calendar tests (sample-based to reduce duplication)

use super::common::make_date;
use finstack_core::dates::calendar::{
    calendar_by_id, ALL_IDS, ASX as Asx, AUCE as Auce, BRBD as Brbd, CATO as Cato, CHZH as Chzh,
    CME as Cme, CNBE as Cnbe, DEFR as Defr, GBLO as Gblo, HKEX as Hkex, HKHK as Hkhk, NYSE as Nyse,
    SGSI as Sgsi, SIFMA as Sifma, SSE as Sse, TARGET2 as Target2, USNY as Usny,
};
use finstack_core::dates::{CalendarRegistry, Date, HolidayCalendar};
use std::collections::HashSet;

fn holiday_set(cal: &dyn HolidayCalendar, year: i32) -> HashSet<Date> {
    (1..=if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        366
    } else {
        365
    })
        .filter_map(|d| Date::from_ordinal_date(year, d).ok())
        .filter(|&dt| cal.is_holiday(dt))
        .collect()
}

#[derive(Clone, Copy)]
struct YearCheck {
    year: i32,
    expected_count: Option<usize>,
    must_have: &'static [(i32, u8, u8)],
}

#[derive(Clone, Copy)]
struct CalendarCase {
    name: &'static str,
    cal: &'static dyn HolidayCalendar,
    checks: &'static [YearCheck],
}

const CASES: &[CalendarCase] = &[
    CalendarCase {
        name: "USNY",
        cal: &Usny,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(11),
                must_have: &[(2024, 1, 1), (2024, 7, 4), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(11),
                must_have: &[(2025, 1, 1), (2025, 7, 4), (2025, 12, 25)],
            },
        ],
    },
    CalendarCase {
        name: "NYSE",
        cal: &Nyse,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(10),
                must_have: &[(2024, 1, 1), (2024, 3, 29), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(10),
                must_have: &[(2025, 1, 1), (2025, 4, 18), (2025, 12, 25)],
            },
        ],
    },
    CalendarCase {
        name: "CME",
        cal: &Cme,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: None,
                must_have: &[(2024, 3, 29), (2024, 7, 4)],
            },
            YearCheck {
                year: 2025,
                expected_count: None,
                must_have: &[(2025, 4, 18), (2025, 11, 27)],
            },
        ],
    },
    CalendarCase {
        name: "SIFMA",
        cal: &Sifma,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(12),
                must_have: &[(2024, 3, 29), (2024, 10, 14), (2024, 11, 11)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(12),
                must_have: &[(2025, 4, 18), (2025, 10, 13), (2025, 11, 11)],
            },
        ],
    },
    CalendarCase {
        name: "TARGET2",
        cal: &Target2,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(6),
                must_have: &[(2024, 3, 29), (2024, 12, 26)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(6),
                must_have: &[(2025, 4, 18), (2025, 12, 26)],
            },
        ],
    },
    CalendarCase {
        name: "DEFR",
        cal: &Defr,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(6),
                must_have: &[(2024, 5, 1), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(6),
                must_have: &[(2025, 5, 1), (2025, 12, 25)],
            },
        ],
    },
    CalendarCase {
        name: "GBLO",
        cal: &Gblo,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(8),
                must_have: &[(2024, 3, 29), (2024, 5, 6), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(8),
                must_have: &[(2025, 4, 18), (2025, 5, 5), (2025, 12, 25)],
            },
        ],
    },
    CalendarCase {
        name: "CHZH",
        cal: &Chzh,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(10),
                must_have: &[(2024, 5, 9), (2024, 8, 1)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(10),
                must_have: &[(2025, 5, 29), (2025, 8, 1)],
            },
        ],
    },
    CalendarCase {
        name: "HKHK",
        cal: &Hkhk,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(11),
                must_have: &[(2024, 2, 10), (2024, 7, 1), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(11),
                must_have: &[(2025, 1, 29), (2025, 4, 4), (2025, 10, 1)],
            },
        ],
    },
    CalendarCase {
        name: "HKEX",
        cal: &Hkex,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(11),
                must_have: &[(2024, 2, 10), (2024, 5, 15)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(11),
                must_have: &[(2025, 1, 29), (2025, 10, 1)],
            },
        ],
    },
    CalendarCase {
        name: "CNBE",
        cal: &Cnbe,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(23),
                must_have: &[(2024, 2, 10), (2024, 5, 1), (2024, 10, 1)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(23),
                must_have: &[(2025, 1, 29), (2025, 5, 1), (2025, 10, 1)],
            },
        ],
    },
    CalendarCase {
        name: "SSE",
        cal: &Sse,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: None,
                must_have: &[(2024, 2, 10), (2024, 5, 1), (2024, 10, 1)],
            },
            YearCheck {
                year: 2025,
                expected_count: None,
                must_have: &[(2025, 1, 29), (2025, 5, 1), (2025, 10, 1)],
            },
        ],
    },
    CalendarCase {
        name: "SGSI",
        cal: &Sgsi,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(7),
                must_have: &[(2024, 2, 10), (2024, 3, 29)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(7),
                must_have: &[(2025, 1, 29), (2025, 8, 11)],
            },
        ],
    },
    CalendarCase {
        name: "ASX",
        cal: &Asx,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(7),
                must_have: &[(2024, 1, 29), (2024, 3, 29)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(7),
                must_have: &[(2025, 1, 27), (2025, 4, 18)],
            },
        ],
    },
    CalendarCase {
        name: "AUCE",
        cal: &Auce,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(10),
                must_have: &[(2024, 1, 29), (2024, 6, 10), (2024, 10, 7)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(10),
                must_have: &[(2025, 1, 27), (2025, 6, 9), (2025, 10, 6)],
            },
        ],
    },
    CalendarCase {
        name: "BRBD",
        cal: &Brbd,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(9),
                must_have: &[(2024, 2, 12), (2024, 11, 20)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(9),
                must_have: &[(2025, 3, 3), (2025, 11, 20)],
            },
        ],
    },
    CalendarCase {
        name: "CATO",
        cal: &Cato,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(12),
                must_have: &[(2024, 2, 19), (2024, 7, 1), (2024, 10, 14)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(12),
                must_have: &[(2025, 2, 17), (2025, 7, 1), (2025, 10, 13)],
            },
        ],
    },
];

#[test]
fn calendars_match_sample_expectations() {
    for case in CASES {
        for check in case.checks {
            let holidays = holiday_set(case.cal, check.year);
            if let Some(expected) = check.expected_count {
                assert_eq!(
                    holidays.len(),
                    expected,
                    "{} {} expected {} holidays",
                    case.name,
                    check.year,
                    expected
                );
            }
            for &(y, m, d) in check.must_have {
                assert!(
                    holidays.contains(&make_date(y, m, d)),
                    "{} {} should include {:04}-{:02}-{:02}",
                    case.name,
                    check.year,
                    y,
                    m,
                    d
                );
            }
        }
    }
}

#[test]
fn test_calendar_by_id_lookup() {
    for &id in ALL_IDS {
        let cal = calendar_by_id(id);
        assert!(cal.is_some(), "Calendar '{}' should be found", id);

        let typed = CalendarRegistry::global().resolve_str(id);
        assert!(typed.is_some(), "Registry should find '{}'", id);

        let mid_week_date = make_date(2025, 6, 18);
        let _ = cal.unwrap().is_holiday(mid_week_date);
    }
}

#[test]
fn test_unknown_calendar_id() {
    assert!(calendar_by_id("unknown_calendar").is_none());
}

#[test]
fn test_calendar_weekend_behavior() {
    let cal = Gblo;
    assert!(!cal.is_business_day(make_date(2025, 6, 21)));
    assert!(!cal.is_business_day(make_date(2025, 6, 22)));
    assert!(cal.is_business_day(make_date(2025, 6, 18)));
}

// ============================================
// Chinese New Year Edge Cases (1970-2150)
// ============================================

fn check_cny_dates(dates: &[(i32, u8, u8)]) {
    // Check multiple calendars that observe CNY
    let calendars = [Cnbe, Hkhk, Sgsi];

    for &(y, m, d) in dates {
        let date = make_date(y, m, d);
        for cal in &calendars {
            assert!(
                cal.is_holiday(date),
                "Calendar {} should have holiday on {}-{}-{}",
                cal.metadata().unwrap().id,
                y,
                m,
                d
            );
        }
    }
}

#[test]
fn test_cny_early_years_1970s() {
    check_cny_dates(&[(1970, 2, 6), (1975, 2, 11), (1980, 2, 16), (1989, 2, 6)]);
}

#[test]
fn test_cny_late_years_2100s() {
    check_cny_dates(&[(2101, 1, 29), (2125, 2, 3), (2150, 1, 28)]);
}

// ============================================
// Observance-convention regressions
// (docs/reviews/2026-06-09-core-quant-review.md — Major: schedules/calendars)
// ============================================

/// Federal Reserve convention (USNY): Sunday holidays move to Monday; Saturday
/// holidays get NO substitute (banks open the preceding Friday). NYSE keeps the
/// OPM-style Fri-if-Sat rule.
#[test]
fn usny_fed_observance_saturday_no_substitute_sunday_to_monday() {
    // July 4, 2026 is a Saturday: Fri 2026-07-03 is a USNY BUSINESS day but an
    // NYSE holiday.
    let fri_jul3_2026 = make_date(2026, 7, 3);
    assert!(
        Usny.is_business_day(fri_jul3_2026),
        "Fed convention: no Friday substitute when July 4 falls on Saturday"
    );
    assert!(
        Nyse.is_holiday(fri_jul3_2026),
        "NYSE observes Saturday July 4 on the preceding Friday"
    );

    // Christmas 2027 falls on Saturday: Fri 2027-12-24 is a USNY business day.
    let fri_dec24_2027 = make_date(2027, 12, 24);
    assert!(
        Usny.is_business_day(fri_dec24_2027),
        "Fed convention: banks open Fri 2027-12-24 (Christmas on Saturday)"
    );

    // New Year's Day 2023 fell on Sunday: Mon 2023-01-02 is a USNY holiday.
    let mon_jan2_2023 = make_date(2023, 1, 2);
    assert!(
        Usny.is_holiday(mon_jan2_2023),
        "Fed convention: Sunday holiday observed the following Monday"
    );
}

/// UK chained substitution for Christmas/Boxing Day: the two observed days
/// never collide and never drop a substitute day.
#[test]
fn gblo_christmas_boxing_day_chained_substitution() {
    // 2021: Dec 25 = Saturday, Dec 26 = Sunday.
    // Actual UK bank holidays: Mon 27 Dec + Tue 28 Dec.
    assert!(Gblo.is_holiday(make_date(2021, 12, 27)));
    assert!(Gblo.is_holiday(make_date(2021, 12, 28)));
    assert!(
        Gblo.is_business_day(make_date(2021, 12, 24)),
        "Fri 2021-12-24 was a UK working day"
    );
    assert!(
        Gblo.is_business_day(make_date(2021, 12, 29)),
        "Wed 2021-12-29 was a UK working day"
    );

    // 2022: Dec 25 = Sunday, Dec 26 = Monday.
    // Actual UK bank holidays: Mon 26 Dec (Boxing Day) + Tue 27 Dec (substitute).
    assert!(Gblo.is_holiday(make_date(2022, 12, 26)));
    assert!(
        Gblo.is_holiday(make_date(2022, 12, 27)),
        "Tue 2022-12-27 was the Christmas Day substitute (previously collapsed)"
    );
    assert!(
        Gblo.is_business_day(make_date(2022, 12, 28)),
        "Wed 2022-12-28 was a UK working day"
    );

    // 2027: Dec 25 = Saturday, Dec 26 = Sunday (same shape as 2021).
    assert!(Gblo.is_holiday(make_date(2027, 12, 27)));
    assert!(
        Gblo.is_holiday(make_date(2027, 12, 28)),
        "Tue 2027-12-28 substitute was previously missing"
    );
    assert!(Gblo.is_business_day(make_date(2027, 12, 29)));
}

/// UK one-off bank holidays and moved May bank holidays (gov.uk history).
#[test]
fn gblo_one_off_and_moved_bank_holidays() {
    // 2012 Diamond Jubilee: Spring BH moved to Mon Jun 4, extra day Tue Jun 5;
    // the regular last-Monday-of-May (May 28) was a working day.
    assert!(Gblo.is_holiday(make_date(2012, 6, 4)));
    assert!(Gblo.is_holiday(make_date(2012, 6, 5)));
    assert!(
        Gblo.is_business_day(make_date(2012, 5, 28)),
        "Spring Bank Holiday was moved out of May in 2012"
    );

    // 2020 VE Day 75th anniversary: Early May BH moved from Mon May 4 to Fri May 8.
    assert!(Gblo.is_holiday(make_date(2020, 5, 8)));
    assert!(
        Gblo.is_business_day(make_date(2020, 5, 4)),
        "Early May Bank Holiday was moved to May 8 in 2020"
    );

    // 2022 Platinum Jubilee: Spring BH moved to Thu Jun 2, extra day Fri Jun 3;
    // last Monday of May (May 30) was a working day.
    assert!(Gblo.is_holiday(make_date(2022, 6, 2)));
    assert!(Gblo.is_holiday(make_date(2022, 6, 3)));
    assert!(
        Gblo.is_business_day(make_date(2022, 5, 30)),
        "Spring Bank Holiday was moved out of May in 2022"
    );

    // 2022-09-19: State Funeral of Queen Elizabeth II.
    assert!(Gblo.is_holiday(make_date(2022, 9, 19)));

    // 2023-05-08: Coronation of King Charles III (extra; Early May BH May 1 kept).
    assert!(Gblo.is_holiday(make_date(2023, 5, 8)));
    assert!(Gblo.is_holiday(make_date(2023, 5, 1)));
}

/// Dia da Consciência Negra became a Brazilian national holiday only from 2024
/// (Law 14.759/2023); B3 traded on Nov 20 in 2023.
#[test]
fn brbd_consciencia_negra_year_gated_from_2024() {
    assert!(
        Brbd.is_business_day(make_date(2023, 11, 20)),
        "B3 traded Mon 2023-11-20 (holiday national only from 2024)"
    );
    assert!(Brbd.is_holiday(make_date(2024, 11, 20)));
}
