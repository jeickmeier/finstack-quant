//! Calendar tests (sample-based to reduce duplication)

use super::common::make_date;
use finstack_quant_core::dates::calendar::{
    calendar_by_id, ALL_IDS, ASX as Asx, AUCE as Auce, BRBD as Brbd, CATO as Cato, CHZH as Chzh,
    CME as Cme, CNBE as Cnbe, DEFR as Defr, GBLO as Gblo, HKEX as Hkex, HKHK as Hkhk, NYSE as Nyse,
    SGSI as Sgsi, SIFMA as Sifma, SSE as Sse, TARGET2 as Target2, USNY as Usny,
};
use finstack_quant_core::dates::{CalendarRegistry, Date, HolidayCalendar};
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
                expected_count: Some(26),
                must_have: &[(2024, 2, 10), (2024, 5, 1), (2024, 10, 1), (2024, 6, 10)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(24),
                must_have: &[(2025, 1, 29), (2025, 5, 1), (2025, 10, 1), (2025, 6, 2)],
            },
        ],
    },
    CalendarCase {
        name: "SSE",
        cal: &Sse,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(26),
                // Dragon Boat (6/10), Mid-Autumn (9/17), CNY eve (2/9) were
                // previously missing / hardcoded.
                must_have: &[
                    (2024, 2, 9),
                    (2024, 2, 10),
                    (2024, 5, 1),
                    (2024, 6, 10),
                    (2024, 9, 17),
                    (2024, 10, 1),
                ],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(24),
                must_have: &[
                    (2025, 1, 28),
                    (2025, 1, 29),
                    (2025, 5, 1),
                    (2025, 6, 2),
                    (2025, 10, 1),
                    (2025, 10, 8),
                ],
            },
            YearCheck {
                year: 2026,
                // 2026 is fully rule-generated (no exact_date needed).
                expected_count: Some(25),
                must_have: &[
                    (2026, 2, 16),
                    (2026, 4, 6),
                    (2026, 6, 19),
                    (2026, 9, 25),
                    (2026, 10, 1),
                ],
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

// ============================================
// Mainland China (SSE / CNBE) closure notices
// ============================================

/// Every date SSE officially announced as closed for 2024-2026 must be a
/// non-business day, and each post-holiday reopen weekday must be a business
/// day. Ranges transcribed from the SSE 休市安排 notices (2023-12, 2024-12,
/// 2025-12). CNBE mirrors the same national arrangement.
#[test]
fn sse_cnbe_match_official_closure_notices_2024_2026() {
    // Inclusive (start, end) closed ranges per SSE notices.
    type Ymd = (i32, u8, u8);
    let closed: &[(Ymd, Ymd)] = &[
        // 2024
        ((2024, 1, 1), (2024, 1, 1)),
        ((2024, 2, 9), (2024, 2, 17)),
        ((2024, 4, 4), (2024, 4, 6)),
        ((2024, 5, 1), (2024, 5, 5)),
        ((2024, 6, 8), (2024, 6, 10)),
        ((2024, 9, 15), (2024, 9, 17)),
        ((2024, 10, 1), (2024, 10, 7)),
        // 2025
        ((2025, 1, 1), (2025, 1, 1)),
        ((2025, 1, 28), (2025, 2, 4)),
        ((2025, 4, 4), (2025, 4, 6)),
        ((2025, 5, 1), (2025, 5, 5)),
        ((2025, 5, 31), (2025, 6, 2)),
        ((2025, 10, 1), (2025, 10, 8)),
        // 2026
        ((2026, 1, 1), (2026, 1, 3)),
        ((2026, 2, 15), (2026, 2, 23)),
        ((2026, 4, 4), (2026, 4, 6)),
        ((2026, 5, 1), (2026, 5, 5)),
        ((2026, 6, 19), (2026, 6, 21)),
        ((2026, 9, 25), (2026, 9, 27)),
        ((2026, 10, 1), (2026, 10, 7)),
    ];

    // Post-holiday reopen weekdays (business days again).
    let reopen: &[Ymd] = &[
        (2024, 1, 2),
        (2024, 2, 19),
        (2024, 5, 6),
        (2025, 1, 2),
        (2025, 2, 5),
        (2025, 5, 6),
        (2025, 6, 3),
        (2026, 1, 5),
        (2026, 2, 24),
        (2026, 5, 6),
        (2026, 6, 22),
        (2026, 9, 28),
        (2026, 10, 8),
    ];

    for cal in [&Sse as &dyn HolidayCalendar, &Cnbe] {
        let id = cal.metadata().unwrap().id;
        for &((sy, sm, sd), (ey, em, ed)) in closed {
            let mut d = make_date(sy, sm, sd);
            let end = make_date(ey, em, ed);
            while d <= end {
                assert!(
                    !cal.is_business_day(d),
                    "{id}: {d} should be closed (official holiday range)"
                );
                d += time::Duration::days(1);
            }
        }
        for &(y, m, d) in reopen {
            assert!(
                cal.is_business_day(make_date(y, m, d)),
                "{id}: {y}-{m:02}-{d:02} should be a business day (reopen)"
            );
        }
    }
}

/// Regression guard for holidays that were previously missing entirely (Dragon
/// Boat, Mid-Autumn) or only present as hardcoded single-year `exact_date`
/// patches. These are now derived from lunar tables + the 连休 bridge rules.
#[test]
fn sse_lunar_festivals_and_bridges_present() {
    // (date, note) — every one is a weekday, so is_holiday must be true.
    let must_be_holiday: &[(i32, u8, u8)] = &[
        (2024, 6, 10), // Dragon Boat (Mon)
        (2024, 9, 16), // Mid-Autumn bridge (Mon)
        (2024, 9, 17), // Mid-Autumn (Tue)
        (2024, 2, 9),  // Spring Festival eve (Fri)
        (2024, 4, 5),  // Qingming bridge (Fri)
        (2025, 1, 28), // Spring Festival eve (Tue)
        (2025, 6, 2),  // Dragon Boat substitute Monday
        (2025, 10, 8), // National / Mid-Autumn merged 8th day
        (2026, 6, 19), // Dragon Boat (Fri)
        (2026, 9, 25), // Mid-Autumn (Fri)
        (2026, 4, 6),  // Qingming substitute Monday
        (2026, 1, 2),  // New Year bridge Friday
        (2026, 2, 16), // Spring Festival eve (Mon)
    ];
    for &(y, m, d) in must_be_holiday {
        assert!(
            Sse.is_holiday(make_date(y, m, d)),
            "SSE should mark {y}-{m:02}-{d:02} as a holiday"
        );
    }

    // Dragon Boat and Mid-Autumn only became statutory in 2008; SSE traded on
    // them before then. 2007 Dragon Boat = Jun 19, 2007 Mid-Autumn = Sep 25.
    assert!(
        Sse.is_business_day(make_date(2007, 6, 19)),
        "Dragon Boat pre-dates its 2008 adoption"
    );
    assert!(
        Sse.is_business_day(make_date(2007, 9, 25)),
        "Mid-Autumn pre-dates its 2008 adoption"
    );
    // 2008 was the first year Dragon Boat was observed; it fell on Sunday
    // Jun 8, so the market closure is the substitute Monday Jun 9.
    assert!(
        Sse.is_holiday(make_date(2008, 6, 9)),
        "Dragon Boat 2008 substitute Monday"
    );
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
// ( — Major: schedules/calendars)
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
