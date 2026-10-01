//! Calendar tests (sample-based to reduce duplication)

use super::common::make_date;
use finstack_quant_core::dates::calendar::{
    calendar_by_id, ALL_IDS, ASX as Asx, AUCE as Auce, BRBD as Brbd, BSE as Bse, CATO as Cato,
    CHZH as Chzh, CME as Cme, CNBE as Cnbe, DEFR as Defr, EUREX as Eurex, GBLO as Gblo,
    HKEX as Hkex, HKHK as Hkhk, NSE as Nse, NYSE as Nyse, SGSI as Sgsi, SIFMA as Sifma, SIX as Six,
    SSE as Sse, TARGET2 as Target2, TSX as Tsx, USNY as Usny,
};
use finstack_quant_core::dates::{
    adjust, available_calendars, BusinessDayConvention, Date, DateExt, HolidayCalendar, Month, Rule,
};
use std::collections::HashSet;
use time::Weekday;

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

#[test]
fn calendars_match_complete_published_years_including_open_days() {
    let rows = include_str!("fixtures/published_weekday_closures.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1);
    for row in rows {
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!(fields.len(), 3, "invalid fixture row {row}");
        let id = fields[0];
        let year = fields[1].parse::<i32>().unwrap();
        let calendar = calendar_by_id(id).unwrap();
        let closed: HashSet<_> = fields[2]
            .split(';')
            .map(|value| {
                let (month, day) = value.split_once('-').unwrap();
                make_date(year, month.parse().unwrap(), day.parse().unwrap())
            })
            .collect();
        assert!(closed.iter().all(|date| !date.is_weekend()));
        let mut expected_count = 0;
        for ordinal in 1..=366 {
            let Ok(date) = Date::from_ordinal_date(year, ordinal) else {
                continue;
            };
            let expected = !date.is_weekend() && !closed.contains(&date);
            assert_eq!(calendar.is_business_day(date), expected, "{id}: {date}");
            expected_count += i32::from(expected);
        }
        assert_eq!(
            calendar.count_business_days(make_date(year, 1, 1), make_date(year + 1, 1, 1)),
            expected_count,
            "{id}: full-year accrual count {year}"
        );
    }
}

#[test]
fn calendar_conventions_preserve_neighboring_open_dates() {
    for id in ["cato", "tsx"] {
        let calendar = calendar_by_id(id).unwrap();
        assert_eq!(
            adjust(
                make_date(2026, 5, 18),
                BusinessDayConvention::Following,
                calendar
            )
            .unwrap(),
            make_date(2026, 5, 19)
        );
        assert!(calendar.is_business_day(make_date(2026, 5, 25)));
    }
    for id in ["asx", "auce"] {
        let calendar = calendar_by_id(id).unwrap();
        assert!(!calendar.is_business_day(make_date(2024, 1, 26)));
        assert!(calendar.is_business_day(make_date(2024, 1, 29)));
    }
    for id in ["asx", "auce", "cato", "tsx", "nzau"] {
        let calendar = calendar_by_id(id).unwrap();
        for closed in [make_date(2027, 12, 27), make_date(2027, 12, 28)] {
            assert!(!calendar.is_business_day(closed), "{id}: {closed}");
        }
        assert!(calendar.is_business_day(make_date(2027, 12, 29)));
        assert!(!calendar.is_business_day(make_date(2022, 12, 27)));
        assert!(calendar.is_business_day(make_date(2022, 12, 28)));
    }
    assert!(Nyse.is_business_day(make_date(2027, 12, 31)));
    assert!(!Nyse.is_business_day(make_date(2025, 1, 9)));
    assert!(Usny.is_business_day(make_date(2025, 1, 9)));
    assert!(Sifma.is_business_day(make_date(2026, 4, 3)));
    assert!(!Nyse.is_business_day(make_date(2026, 4, 3)));
    assert!(Sgsi.is_business_day(make_date(2025, 8, 11)));
    assert!(!Sgsi.is_business_day(make_date(2027, 2, 8)));
    // ANBIMA national accrual holidays deliberately differ from B3 trading.
    assert!(Brbd.is_business_day(make_date(2025, 12, 24)));
    assert!(Brbd.is_business_day(make_date(2025, 12, 31)));
    assert!(Brbd.name.contains("ANBIMA"));
}

#[test]
fn holiday_adoption_bounds_are_respected() {
    for calendar in [&Cato, &Tsx] {
        assert!(calendar.is_business_day(make_date(2007, 2, 19)));
        assert!(!calendar.is_business_day(make_date(2008, 2, 18)));
    }
    assert!(Cato.is_business_day(make_date(2020, 9, 30)));
    assert!(!Cato.is_business_day(make_date(2021, 9, 30)));
    let auckland = calendar_by_id("nzau").unwrap();
    assert!(!auckland.is_business_day(make_date(2022, 6, 24)));
    assert!(!auckland.is_business_day(make_date(2052, 6, 21)));
    assert!(auckland.is_business_day(make_date(2021, 6, 25)));
    assert!(!auckland.is_business_day(make_date(2022, 9, 26)));
    assert!(auckland.is_business_day(make_date(2010, 4, 26)));
    assert!(!auckland.is_business_day(make_date(2015, 4, 27)));
    assert!(!auckland.is_business_day(make_date(2022, 1, 4)));
    assert!(auckland.is_business_day(make_date(2022, 1, 5)));
}

#[test]
fn hong_kong_matches_published_general_holidays() {
    // Complete gazetted lists (excluding ordinary Sundays), not model outputs:
    // https://www.legco.gov.hk/yr98-99/english/bc/bc51/papers/p66e01.pdf
    // https://www.info.gov.hk/gia/general/202405/03/P2024043000417.htm
    // https://www.info.gov.hk/gia/general/202505/16/P2025051300353.htm
    // https://www.info.gov.hk/gia/general/202605/15/P2026051400300.htm
    let years: &[(i32, &[(u8, u8)])] = &[
        (
            1999,
            &[
                (1, 1),
                (2, 16),
                (2, 17),
                (2, 18),
                (4, 2),
                (4, 3),
                (4, 5),
                (4, 6),
                (5, 1),
                (5, 22),
                (6, 18),
                (7, 1),
                (9, 25),
                (10, 1),
                (10, 18),
                (12, 25),
                (12, 27),
            ],
        ),
        (
            2025,
            &[
                (1, 1),
                (1, 29),
                (1, 30),
                (1, 31),
                (4, 4),
                (4, 18),
                (4, 19),
                (4, 21),
                (5, 1),
                (5, 5),
                (5, 31),
                (7, 1),
                (10, 1),
                (10, 7),
                (10, 29),
                (12, 25),
                (12, 26),
            ],
        ),
        (
            2026,
            &[
                (1, 1),
                (2, 17),
                (2, 18),
                (2, 19),
                (4, 3),
                (4, 4),
                (4, 6),
                (4, 7),
                (5, 1),
                (5, 25),
                (6, 19),
                (7, 1),
                (9, 26),
                (10, 1),
                (10, 19),
                (12, 25),
                (12, 26),
            ],
        ),
        (
            2027,
            &[
                (1, 1),
                (2, 6),
                (2, 8),
                (2, 9),
                (3, 26),
                (3, 27),
                (3, 29),
                (4, 5),
                (5, 1),
                (5, 13),
                (6, 9),
                (7, 1),
                (9, 16),
                (10, 1),
                (10, 8),
                (12, 25),
                (12, 27),
            ],
        ),
    ];
    for id in ["hkhk", "hkex"] {
        let calendar = calendar_by_id(id).unwrap();
        for &(year, month_days) in years {
            let expected: HashSet<_> = month_days
                .iter()
                .map(|&(month, day)| make_date(year, month, day))
                .collect();
            assert_eq!(holiday_set(calendar, year), expected, "{id} {year}");
        }
        assert_eq!(
            make_date(2026, 4, 2)
                .add_business_days(1, calendar)
                .unwrap(),
            make_date(2026, 4, 8),
            "{id}: Easter/Ching Ming collision"
        );
        // Saturday holidays do not close the preceding Friday.
        assert!(calendar.is_business_day(make_date(2027, 4, 30)));
    }
}

#[test]
fn hong_kong_historical_adoption_and_substitution() {
    for id in ["hkhk", "hkex"] {
        let calendar = calendar_by_id(id).unwrap();
        for date in [
            make_date(1981, 7, 29),  // Royal wedding one-off holiday.
            make_date(1982, 4, 21),  // Colonial Queen's Birthday.
            make_date(1983, 6, 13),  // Monday after second Saturday in June.
            make_date(1984, 6, 18),  // Announced exception one week later.
            make_date(1986, 10, 22), // Royal visit one-off holiday.
            make_date(1997, 6, 16),  // Last Queen's Birthday holiday.
            make_date(1997, 7, 2),   // One-off handover holiday.
            make_date(1998, 8, 17),  // Transitional Sino-Japanese War Victory Day.
            make_date(1998, 10, 2),
            make_date(1999, 5, 22), // First general Buddha birthday holiday.
            make_date(2010, 2, 13), // Pre-2012 substitution on New Year's Eve.
            make_date(2013, 2, 13), // Post-2012 fourth lunar day substitution.
            make_date(2012, 10, 2), // Mid-Autumn/National Day collision.
            make_date(2015, 9, 3),  // Published one-off special holiday.
            make_date(2020, 10, 2), // Mid-Autumn/National Day collision.
        ] {
            assert!(calendar.is_holiday(date), "{id} missing {date}");
        }
        for date in [
            make_date(1998, 5, 1), // Labour Day only added to general holidays in 1999.
            make_date(1984, 6, 11),
            make_date(1998, 6, 15),
            make_date(1999, 8, 16),
            make_date(1999, 10, 2),
            make_date(2010, 2, 17), // No fourth-day substitute under the older regime.
        ] {
            assert!(!calendar.is_holiday(date), "{id} unexpected {date}");
        }
    }
}

#[test]
fn japan_matches_cabinet_office_holidays_1970_through_2027() {
    // Snapshot of the independent Cabinet Office CSV (accessed 2026-09-30):
    // https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv
    let expected: HashSet<_> = include_str!("fixtures/japan_public_holidays.csv")
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<_> = line.split(',').collect();
            make_date(
                fields[0].parse().unwrap(),
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
            )
        })
        .collect();
    for year in 1970..=2027 {
        for ordinal in 1..=366 {
            let Ok(date) = Date::from_ordinal_date(year, ordinal) else {
                continue;
            };
            assert_eq!(
                Rule::JapanPublicHolidays.applies(date),
                expected.contains(&date),
                "national/citizens/substitute holiday {date}"
            );
            let bank_closure = (date.month() == Month::January && matches!(date.day(), 2 | 3))
                || (date.month() == Month::December && date.day() == 31);
            let business = !expected.contains(&date)
                && !bank_closure
                && !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday);
            for id in ["jpto", "jpx"] {
                assert_eq!(
                    calendar_by_id(id).unwrap().is_business_day(date),
                    business,
                    "{id} {date}"
                );
            }
        }
    }
}

#[test]
fn japan_golden_week_and_citizens_holiday_adjustments() {
    for id in ["jpto", "jpx"] {
        let calendar = calendar_by_id(id).unwrap();
        assert_eq!(
            adjust(
                make_date(2026, 5, 5),
                BusinessDayConvention::Following,
                calendar
            )
            .unwrap(),
            make_date(2026, 5, 7),
            "{id}: substitute skips May 4 and May 5"
        );
        assert!(!calendar.is_business_day(make_date(2026, 9, 22)));
        assert!(calendar.is_business_day(make_date(2026, 9, 24)));
        assert!(
            calendar.is_business_day(make_date(2023, 2, 10)),
            "{id}: Saturday Feb 11 has no Friday substitute"
        );
        assert!(!calendar.is_business_day(make_date(2027, 3, 22)));
        assert!(!calendar.is_business_day(make_date(2026, 12, 31)));
    }
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
                expected_count: Some(11),
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
                expected_count: Some(17),
                must_have: &[(2024, 2, 10), (2024, 7, 1), (2024, 12, 25)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(17),
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
                expected_count: Some(17),
                must_have: &[(2024, 2, 10), (2024, 5, 15)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(17),
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
                expected_count: Some(27),
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
                expected_count: Some(27),
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
                expected_count: Some(26),
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
                expected_count: Some(12),
                must_have: &[(2024, 2, 10), (2024, 3, 29)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(12),
                must_have: &[(2025, 1, 29), (2025, 8, 9)],
            },
        ],
    },
    CalendarCase {
        name: "ASX",
        cal: &Asx,
        checks: &[
            YearCheck {
                year: 2024,
                expected_count: Some(8),
                must_have: &[(2024, 1, 26), (2024, 3, 29)],
            },
            YearCheck {
                year: 2025,
                expected_count: Some(8),
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
                must_have: &[(2024, 1, 26), (2024, 6, 10), (2024, 10, 7)],
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
    CalendarCase {
        name: "EUREX",
        cal: &Eurex,
        checks: &[YearCheck {
            year: 2025,
            expected_count: Some(8),
            must_have: &[(2025, 4, 18), (2025, 12, 24), (2025, 12, 31)],
        }],
    },
    CalendarCase {
        name: "SIX",
        cal: &Six,
        checks: &[YearCheck {
            year: 2025,
            expected_count: Some(12),
            must_have: &[(2025, 5, 29), (2025, 8, 1), (2025, 12, 24), (2025, 12, 31)],
        }],
    },
    CalendarCase {
        name: "TSX",
        cal: &Tsx,
        checks: &[
            YearCheck {
                year: 2025,
                expected_count: Some(10),
                must_have: &[(2025, 2, 17), (2025, 7, 1), (2025, 10, 13)],
            },
            YearCheck {
                year: 2026,
                expected_count: Some(10),
                must_have: &[(2026, 12, 28)],
            },
        ],
    },
    CalendarCase {
        name: "NSE",
        cal: &Nse,
        checks: &[YearCheck {
            year: 2026,
            expected_count: None,
            must_have: &[(2026, 1, 26), (2026, 3, 3)],
        }],
    },
    CalendarCase {
        name: "BSE",
        cal: &Bse,
        checks: &[YearCheck {
            year: 2026,
            expected_count: None,
            must_have: &[(2026, 1, 26), (2026, 3, 3)],
        }],
    },
];

/// Exchange calendars are not aliases of the nearest country/settlement set.
#[test]
fn exchange_calendars_are_not_aliases() {
    let eve = make_date(2025, 12, 24);
    let nye = make_date(2025, 12, 31);
    assert!(Eurex.is_holiday(eve));
    assert!(Eurex.is_holiday(nye));
    assert!(
        Defr.is_business_day(eve),
        "defr must remain open on Christmas Eve (eurex is not an alias)"
    );
    assert!(
        Defr.is_business_day(nye),
        "defr must remain open on New Year's Eve (eurex is not an alias)"
    );

    let truth = make_date(2025, 9, 30);
    let remembrance = make_date(2025, 11, 11);
    assert!(
        Tsx.is_business_day(truth),
        "TSX trades on National Day for Truth and Reconciliation"
    );
    assert!(
        Tsx.is_business_day(remembrance),
        "TSX trades on Remembrance Day"
    );
    assert!(Cato.is_holiday(truth));
    assert!(Cato.is_holiday(remembrance));

    assert!(Nse.is_holiday(make_date(2026, 1, 26)));
    assert!(Nse.is_holiday(make_date(2026, 3, 3)));
    let independence_saturday = make_date(2026, 8, 15);
    assert_eq!(
        independence_saturday.weekday(),
        time::Weekday::Saturday,
        "2026-08-15 must be Saturday for this regression"
    );
    assert!(
        Nse.is_business_day(make_date(2026, 8, 14)),
        "Independence Day 2026 is Saturday; Friday is not an observed weekday holiday"
    );
    assert!(
        Nse.is_business_day(make_date(2026, 8, 17)),
        "Independence Day 2026 is Saturday; Monday is not an observed weekday holiday"
    );
    assert!(Bse.is_holiday(make_date(2026, 1, 26)));
    assert!(Bse.is_holiday(make_date(2026, 3, 3)));

    let ids = available_calendars();
    for id in ["eurex", "six", "tsx", "nse", "bse"] {
        assert!(ids.contains(&id), "available_calendars must include {id}");
    }
}

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

        let typed = calendar_by_id(id);
        assert!(typed.is_some(), "Free resolver should find '{}'", id);

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

// Chinese New Year Edge Cases (1970-2150)

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

// Mainland China (SSE / CNBE) closure notices

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
fn mainland_calendars_use_published_2022_closure_boundaries() {
    for (name, calendar) in [
        ("sse", &Sse as &dyn HolidayCalendar),
        ("cnbe", &Cnbe as &dyn HolidayCalendar),
    ] {
        assert!(
            calendar.is_holiday(make_date(2022, 1, 31)),
            "{name} must close on the first published Spring Festival day"
        );
        assert!(
            calendar.is_business_day(make_date(2022, 2, 7)),
            "{name} must reopen after the published Spring Festival closure"
        );
        assert!(calendar.is_holiday(make_date(2022, 4, 30)));
        assert!(calendar.is_holiday(make_date(2022, 5, 4)));
        assert!(calendar.is_business_day(make_date(2022, 5, 5)));
    }
}

#[test]
fn mainland_calendars_include_historical_official_one_off_closures() {
    let closures = [
        make_date(2009, 10, 8),
        make_date(2010, 9, 23),
        make_date(2010, 9, 24),
        make_date(2015, 9, 3),
        make_date(2015, 9, 4),
    ];
    let adjacent_business_days = [
        make_date(2009, 9, 30),
        make_date(2009, 10, 9),
        make_date(2010, 9, 21),
        make_date(2010, 9, 27),
        make_date(2015, 9, 2),
        make_date(2015, 9, 7),
    ];

    for (name, calendar) in [
        ("sse", &Sse as &dyn HolidayCalendar),
        ("cnbe", &Cnbe as &dyn HolidayCalendar),
    ] {
        for date in closures {
            assert!(calendar.is_holiday(date), "{name} must close on {date}");
        }
        for date in adjacent_business_days {
            assert!(
                calendar.is_business_day(date),
                "{name} must remain open on published boundary {date}"
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

// Observance-convention regressions
// ( — Major: schedules/calendars)

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

#[test]
fn sofr_calendar_follows_repo_closures_independently_of_banking_and_trading() {
    let sofr = calendar_by_id("sofr").expect("registered SOFR calendar");
    // NY Fed operating policies 250328a, 260312a and 260618a: there is no
    // SOFR value date on these Fridays; Thursday's fixing covers to Monday.
    for (holiday, following) in [
        (make_date(2025, 4, 18), make_date(2025, 4, 21)),
        (make_date(2026, 4, 3), make_date(2026, 4, 6)),
        (make_date(2026, 7, 3), make_date(2026, 7, 6)),
    ] {
        assert!(!sofr.is_business_day(holiday), "SOFR closure: {holiday}");
        assert!(Usny.is_business_day(holiday), "banking day: {holiday}");
        assert_eq!(
            adjust(holiday, BusinessDayConvention::Following, sofr).unwrap(),
            following
        );
    }
    // Good Friday 2026 is a SIFMA early-close trading session, but the
    // cleared repo market is closed and SOFR does not fix.
    assert!(Sifma.is_business_day(make_date(2026, 4, 3)));

    // Repo observance is holiday-specific; do not import the federal-workforce
    // Friday-substitution rule for New Year's Day or Veterans Day.
    assert!(sofr.is_business_day(make_date(2021, 12, 31)));
    assert!(sofr.is_business_day(make_date(2023, 11, 10)));
    assert!(!sofr.is_business_day(make_date(2021, 12, 24)));
    assert!(!sofr.is_business_day(make_date(2023, 1, 2)));
    assert!(!sofr.is_business_day(make_date(2022, 6, 20)));
    assert!(!sofr.is_business_day(make_date(2018, 12, 5)));
    // The 2025 Carter mourning day was an early close, with unchanged SOFR.
    assert!(sofr.is_business_day(make_date(2025, 1, 9)));
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
/// (Law 14.759/2023); ANBIMA national accrual holidays include it from 2024.
#[test]
fn brbd_consciencia_negra_year_gated_from_2024() {
    assert!(
        Brbd.is_business_day(make_date(2023, 11, 20)),
        "National accrual holiday applies from 2024"
    );
    assert!(Brbd.is_holiday(make_date(2024, 11, 20)));
}
