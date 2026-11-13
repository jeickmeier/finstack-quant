use super::*;

use std::str::FromStr;

fn d(year: i32, month: Month, day: u8) -> Date {
    crate::dates::create_date(year, month, day).expect("valid date")
}

#[test]
fn january_fiscal_start_labels_round_trip_to_containing_year() {
    for day in [1, 2, 15, 31] {
        let config = FiscalConfig::new(1, day).unwrap();
        for date in [
            d(2025, Month::January, 1),
            d(2025, Month::January, day),
            d(2025, Month::December, 31),
        ] {
            let year = date.fiscal_year(config);
            let plan = build_fiscal_periods(&format!("FY{year}..FY{year}"), config, None).unwrap();
            assert!(
                plan.periods[0].start <= date && date < plan.periods[0].end,
                "{date} must be inside FY{year} for {config:?}"
            );
        }
    }
}

#[test]
fn leap_fiscal_subperiods_partition_the_annual_interval() {
    let config = FiscalConfig::new(2, 29).unwrap();
    for year in [2024, 2025, 2026] {
        let annual = build_fiscal_periods(&format!("FY{year}..FY{year}"), config, None).unwrap();
        for (kind, count) in [("M", 12), ("Q", 4), ("H", 2)] {
            let plan = build_fiscal_periods(
                &format!("FY{year}{kind}1..FY{}{kind}1", year + 1),
                config,
                None,
            )
            .unwrap();
            assert_eq!(plan.periods[0].start, annual.periods[0].start);
            assert_eq!(plan.periods[count - 1].end, annual.periods[0].end);
            for adjacent in plan.periods.windows(2) {
                assert_eq!(
                    adjacent[0].end, adjacent[1].start,
                    "fiscal {kind} periods must cover every day"
                );
            }
            assert_eq!(
                plan.periods[..count]
                    .iter()
                    .map(|period| (period.end - period.start).whole_days())
                    .sum::<i64>(),
                (annual.periods[0].end - annual.periods[0].start).whole_days()
            );
        }
    }
}

#[test]
fn actuals_cutoff_requires_matching_period_kind_and_calendar() {
    for cutoff in ["2025Q1", "2025"] {
        let error = build_periods("2025M01..M04", Some(cutoff)).unwrap_err();
        assert!(error.to_string().contains("same period kind and calendar"));
    }
    let error = build_periods("2025M01..M04", Some("FY2025M02")).unwrap_err();
    assert!(
        error.to_string().contains("need a fiscal builder"),
        "{error}"
    );
    let fiscal = FiscalConfig::new(10, 1).expect("valid fiscal start");
    assert!(build_fiscal_periods("FY2025M01..M04", fiscal, Some("2025Q1")).is_err());
    for plan in [
        build_periods("2025M01..M04", Some("2025M02")).unwrap(),
        build_fiscal_periods("FY2025M01..M04", fiscal, Some("2025M02")).unwrap(),
    ] {
        assert_eq!(
            plan.periods.iter().map(|p| p.is_actual).collect::<Vec<_>>(),
            [true, true, false, false]
        );
    }
}

#[test]
fn build_periods_weekly_uses_iso_week_bounds() {
    let plan = build_periods("2025W01..W01", None).expect("weekly plan");
    assert_eq!(plan.periods.len(), 1);
    let period = &plan.periods[0];
    assert_eq!(period.start, d(2024, Month::December, 30));
    assert_eq!(period.end, d(2025, Month::January, 6));
}

#[test]
fn parse_id_rejects_invalid_iso_week_for_year() {
    assert!(PeriodId::from_str("2021W53").is_err());
    assert!(PeriodId::from_str("2020W53").is_ok());
}

#[test]
fn period_errors_name_the_value_and_grammar() {
    let err = PeriodId::from_str("2024X9").expect_err("bad designator");
    let msg = err.to_string();
    assert!(msg.contains("'2024X9'"), "{msg}");
    assert!(msg.contains("2024Q1..Q4"), "{msg}");

    let msg = PeriodId::month(2025, 13).expect_err("13").to_string();
    assert!(msg.contains("13") && msg.contains("1..=12"), "{msg}");

    let msg = build_periods("2024Q1..2024M06", None)
        .expect_err("kind mismatch")
        .to_string();
    assert!(
        msg.contains("'2024Q1..2024M06'") && msg.contains("period kind"),
        "{msg}"
    );
    let msg = build_periods("2024Q1..M6", None)
        .expect_err("relative index of the wrong kind")
        .to_string();
    assert!(msg.contains("'M6'"), "{msg}");

    let msg = build_periods("2024Q3..Q1", None)
        .expect_err("inverted")
        .to_string();
    assert!(msg.contains("after end"), "{msg}");

    let msg = build_periods("2024Q1", None)
        .expect_err("no range")
        .to_string();
    assert!(msg.contains("'..'"), "{msg}");

    let msg = build_periods("2021W01..W53", None)
        .expect_err("week 53")
        .to_string();
    assert!(msg.contains("53") && msg.contains("1..=52"), "{msg}");
}

#[test]
fn next_rolls_to_next_iso_year_after_last_week() {
    let next = PeriodId::week(2021, 52)
        .expect("valid period fixture")
        .shift(1, PeriodCalendar::Gregorian)
        .expect("next week");
    assert_eq!(next, PeriodId::week(2022, 1).expect("valid period fixture"));
}

#[test]
fn prev_rolls_to_previous_iso_year_last_week() {
    let prev = PeriodId::week(2022, 1)
        .expect("valid period fixture")
        .shift(-1, PeriodCalendar::Gregorian)
        .expect("previous week");
    assert_eq!(
        prev,
        PeriodId::week(2021, 52).expect("valid period fixture")
    );
}

#[test]
fn prior_observation_date_daily_is_one_calendar_day() {
    let first = d(2024, Month::January, 3);
    assert_eq!(
        PeriodKind::Daily
            .prior_observation_date(first)
            .expect("valid prior date"),
        d(2024, Month::January, 2)
    );
}

#[test]
fn prior_observation_date_weekly_is_seven_days() {
    let first = d(2024, Month::January, 8);
    assert_eq!(
        PeriodKind::Weekly
            .prior_observation_date(first)
            .expect("valid prior date"),
        d(2024, Month::January, 1)
    );
}

#[test]
fn prior_observation_date_month_end_clamps() {
    let jan31 = d(2023, Month::January, 31);
    assert_eq!(
        PeriodKind::Monthly
            .prior_observation_date(jan31)
            .expect("valid prior date"),
        d(2022, Month::December, 31)
    );
    let feb28 = d(2023, Month::February, 28);
    assert_eq!(
        PeriodKind::Monthly
            .prior_observation_date(feb28)
            .expect("valid prior date"),
        d(2023, Month::January, 28)
    );
    let mar31 = d(2023, Month::March, 31);
    assert_eq!(
        PeriodKind::Monthly
            .prior_observation_date(mar31)
            .expect("valid prior date"),
        d(2023, Month::February, 28)
    );
    assert_eq!(
        PeriodKind::Quarterly
            .prior_observation_date(jan31)
            .expect("valid prior date"),
        d(2022, Month::October, 31)
    );
    assert_eq!(
        PeriodKind::SemiAnnual
            .prior_observation_date(jan31)
            .expect("valid prior date"),
        d(2022, Month::July, 31)
    );
    assert_eq!(
        PeriodKind::Annual
            .prior_observation_date(jan31)
            .expect("valid prior date"),
        d(2022, Month::January, 31)
    );
}

#[test]
fn period_kind_display_parse_and_counts() {
    assert_eq!(PeriodKind::SemiAnnual.to_string(), "semi_annual");
    assert_eq!(
        PeriodKind::from_str("semi_annual"),
        Ok(PeriodKind::SemiAnnual)
    );
    assert_eq!(PeriodKind::from_str("annual"), Ok(PeriodKind::Annual));
    for noncanonical in [
        "D",
        "B",
        "W",
        "M",
        "ME",
        "Q",
        "QE",
        "A",
        "Y",
        "YE",
        "semiannual",
        "q",
        "SemiAnnual",
        "DAILY",
        " quarterly",
        "monthly ",
        "",
    ] {
        assert!(
            PeriodKind::from_str(noncanonical).is_err(),
            "{noncanonical}"
        );
    }
    for kind in [
        PeriodKind::Daily,
        PeriodKind::Weekly,
        PeriodKind::Monthly,
        PeriodKind::Quarterly,
        PeriodKind::SemiAnnual,
        PeriodKind::Annual,
    ] {
        assert_eq!(
            kind.to_string()
                .parse::<PeriodKind>()
                .expect("canonical label"),
            kind
        );
    }
    assert!(PeriodKind::from_str("unknown").is_err());

    assert_eq!(PeriodKind::Daily.periods_per_year(), 252);
    assert_eq!(PeriodKind::Quarterly.periods_per_year(), 4);
}

#[test]
fn period_id_display_parse_and_serde_roundtrip() {
    let q = PeriodId::from_str("2025Q3").expect("quarter");
    assert_eq!(q.to_string(), "2025Q3");
    let m = PeriodId::from_str("2025m06").expect("month lowercase");
    assert_eq!(m, PeriodId::month(2025, 6).expect("valid period fixture"));
    let d = PeriodId::from_str("2025D059").expect("ordinal day");
    assert_eq!(d, PeriodId::day(2025, 59).expect("valid period fixture"));

    let json = serde_json::to_string(&q).expect("serialize");
    let back: PeriodId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, q);
}

#[test]
fn period_id_ordering_is_structural_and_total() {
    // (year, kind rank, index, fiscal): daily < weekly < monthly < quarterly
    // < semi-annual < annual within a year; not a chronology across kinds.
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let m1 = PeriodId::month(2025, 1).expect("valid period fixture");
    let m12 = PeriodId::month(2025, 12).expect("valid period fixture");
    let w53 = PeriodId::week(2026, 53).expect("valid period fixture");
    let fy = "FY2025M01".parse::<PeriodId>().expect("fiscal period");
    assert!(
        m1 < q1 && m12 < q1,
        "every month of a year ranks below its quarters"
    );
    assert!(m1 < fy, "the Gregorian flag precedes the fiscal flag");
    assert!(q1 < w53, "a later year ranks above an earlier one");
    let mut ids = vec![q1, fy, w53, m12, m1];
    ids.sort();
    assert_eq!(ids, vec![m1, fy, m12, q1, w53]);
    assert!(PeriodKind::Daily < PeriodKind::Weekly && PeriodKind::Weekly < PeriodKind::Monthly);
    assert!(
        PeriodKind::Quarterly < PeriodKind::SemiAnnual
            && PeriodKind::SemiAnnual < PeriodKind::Annual
    );
}

#[test]
fn build_periods_quarterly_monthly_daily_and_annual() {
    let q = build_periods("2025Q1..Q4", None).expect("quarters");
    assert_eq!(q.periods.len(), 4);
    assert_eq!(q.periods[0].start, d(2025, Month::January, 1));

    let cross = build_periods("2024M11..2025M02", None).expect("cross-year months");
    assert_eq!(cross.periods.len(), 4);

    let rel_m = build_periods("2025M01..M03", None).expect("relative months");
    assert_eq!(rel_m.periods.len(), 3);

    let rel_q = build_periods("2025Q1..Q3", Some("2025Q2")).expect("actuals boundary");
    assert!(rel_q.periods[0].is_actual);
    assert!(rel_q.periods[1].is_actual);
    assert!(!rel_q.periods[2].is_actual);

    let days = build_periods("2025D001..D003", None).expect("daily");
    assert_eq!(days.periods.len(), 3);

    let halves = build_periods("2025H1..H2", None).expect("halves");
    assert_eq!(halves.periods.len(), 2);

    let years = build_periods("2024..2026", None).expect("annual range");
    assert_eq!(years.periods.len(), 3);
}

#[test]
fn build_periods_rejects_mixed_kinds_or_inverted_ranges() {
    assert!(build_periods("2025Q1..2025M01", None).is_err());
    assert!(build_periods("2025Q2..2025Q1", None).is_err());
}

#[test]
fn fiscal_config_constructors_and_validation() {
    assert!(FiscalConfig::new(13, 1).is_err());
    assert!(FiscalConfig::new(1, 32).is_err());
    assert!(FiscalConfig::new(0, 1).is_err());
    assert!(FiscalConfig::new(1, 0).is_err());
    let uk = FiscalConfig::new(4, 6).expect("valid fiscal start");
    assert_eq!(uk.start_month(), 4);
    let feb_edge = FiscalConfig::new(2, 30).expect("feb clamp path");
    let plan = build_fiscal_periods("2025Q1..Q1", feb_edge, None).expect("fiscal quarter");
    assert_eq!(plan.periods.len(), 1);
}

#[test]
fn fiscal_config_deserialization_validates_and_year_start_clamps() {
    let uk: FiscalConfig =
        serde_json::from_str(r#"{"start_month":4,"start_day":6}"#).expect("valid config");
    assert_eq!(uk, FiscalConfig::new(4, 6).expect("valid fiscal start"));
    assert_eq!(
        serde_json::to_string(&uk).expect("serialize"),
        r#"{"start_month":4,"start_day":6}"#
    );
    for json in [
        r#"{"start_month":4,"start_day":0}"#,
        r#"{"start_month":4,"start_day":40}"#,
        r#"{"start_month":13,"start_day":1}"#,
        r#"{"start_month":4,"start_day":6,"extra":1}"#,
    ] {
        assert!(
            serde_json::from_str::<FiscalConfig>(json).is_err(),
            "{json}"
        );
    }

    assert_eq!(
        FiscalConfig::calendar_year().year_start(2025),
        Ok(d(2025, Month::January, 1))
    );
    assert_eq!(
        FiscalConfig::new(10, 1)
            .expect("valid fiscal start")
            .year_start(2025),
        Ok(d(2024, Month::October, 1))
    );
    let feb_30 = FiscalConfig::new(2, 30).expect("clamped start");
    assert_eq!(feb_30.year_start(2025), Ok(d(2024, Month::February, 29)));
    assert_eq!(feb_30.year_start(2026), Ok(d(2025, Month::February, 28)));
}

#[test]
fn build_fiscal_periods_us_federal_and_monthly() {
    let cfg = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let qs = build_fiscal_periods("2025Q1..Q2", cfg, None).expect("FY quarters");
    assert_eq!(qs.periods.len(), 2);

    let jp = FiscalConfig::new(4, 1).expect("valid fiscal start");
    let ms = build_fiscal_periods("2025M01..M02", jp, None).expect("FY months");
    assert_eq!(ms.periods.len(), 2);
}

#[test]
fn fiscal_daily_and_weekly_bounds_stay_inside_fiscal_year() {
    let cfg = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let day = build_fiscal_periods("2025D001..D001", cfg, None).expect("fiscal day");
    assert_eq!(day.periods[0].start, d(2024, Month::October, 1));
    assert_eq!(day.periods[0].end, d(2024, Month::October, 2));

    let week = build_fiscal_periods("2020W53..W53", cfg, None).expect("fiscal week 53");
    assert_eq!(week.periods[0].start, d(2020, Month::September, 29));
    assert_eq!(week.periods[0].end, d(2020, Month::October, 1));
}

#[test]
fn fiscal_ranges_include_partial_week_53_and_leap_day_366() {
    let federal = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let week = build_fiscal_periods("FY2025W53..W53", federal, None)
        .expect("FY2025 has a partial week 53");
    assert_eq!(week.periods[0].start, d(2025, Month::September, 30));
    assert_eq!(week.periods[0].end, d(2025, Month::October, 1));

    let crossing =
        build_fiscal_periods("2025W52..2026W01", federal, None).expect("cross-fiscal-year weeks");
    assert_eq!(crossing.periods.len(), 3);
    assert_eq!(crossing.periods[1].id.index, 53);
    assert_eq!(crossing.periods[1].end, crossing.periods[2].start);

    let february = FiscalConfig::new(2, 1).expect("valid fiscal start");
    let leap_day = build_fiscal_periods("FY2025D366..D366", february, None)
        .expect("FY2025 spans leap day and has D366");
    assert_eq!(leap_day.periods[0].start, d(2025, Month::January, 31));
    assert_eq!(leap_day.periods[0].end, d(2025, Month::February, 1));
}

#[test]
fn fiscal_week_stepping_uses_fiscal_year_capacity() {
    let federal = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let week_52 = PeriodId {
        year: 2025,
        index: 52,
        kind: PeriodKind::Weekly,
        fiscal: true,
    };
    let week_53 = PeriodId {
        year: 2025,
        index: 53,
        kind: PeriodKind::Weekly,
        fiscal: true,
    };
    let next_year = PeriodId {
        year: 2026,
        index: 1,
        kind: PeriodKind::Weekly,
        fiscal: true,
    };

    assert_eq!(
        week_52
            .shift(1, PeriodCalendar::Fiscal(federal))
            .expect("FY week 53"),
        week_53
    );
    assert_eq!(
        week_53
            .shift(1, PeriodCalendar::Fiscal(federal))
            .expect("next FY"),
        next_year
    );
    assert_eq!(
        next_year
            .shift(-1, PeriodCalendar::Fiscal(federal))
            .expect("previous FY week"),
        week_53
    );

    let plan =
        build_fiscal_periods("FY2025W52..FY2026W01", federal, None).expect("fiscal weekly range");
    assert_eq!(
        plan.periods
            .iter()
            .map(|period| period.id)
            .collect::<Vec<_>>(),
        vec![week_52, week_53, next_year]
    );

    assert_eq!(
        PeriodId::week(2025, 52)
            .expect("valid period fixture")
            .shift(1, PeriodCalendar::Gregorian)
            .expect("ISO next remains Gregorian"),
        PeriodId::week(2026, 1).expect("valid period fixture")
    );
}

#[test]
fn fiscal_week_53_display_parse_and_serde_roundtrip() {
    let federal = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let plan = build_fiscal_periods("FY2025W52..W52", federal, None).expect("fiscal weekly period");
    let week_53 = plan.periods[0]
        .id
        .shift(1, PeriodCalendar::Fiscal(federal))
        .expect("FY2025 week 53");

    assert_eq!(week_53.to_string(), "FY2025W53");
    assert_eq!(
        PeriodId::from_str(&week_53.to_string()).expect("fiscal display must parse"),
        week_53
    );
    let json = serde_json::to_string(&week_53).expect("serialize fiscal week");
    assert_eq!(json, r#""FY2025W53""#);
    assert_eq!(
        serde_json::from_str::<PeriodId>(&json).expect("deserialize fiscal week"),
        week_53
    );
    assert!(PeriodId::from_str("2025W53").is_err());
}

#[test]
fn fallible_period_id_constructors_reject_invalid_indices() {
    assert!(PeriodId::month(2025, 13).is_err());
    assert!(PeriodId::quarter(2025, 0).is_err());
    assert!(PeriodId::week(2021, 53).is_err());
    assert!(PeriodId::day(2025, 366).is_err());
    assert!(PeriodId::half(2025, 3).is_err());
}

#[test]
fn period_plan_serde_roundtrip() {
    let plan = build_periods("2025Q1..Q2", None).expect("plan");
    assert_eq!(plan.periods.len(), 2);
    let json = serde_json::to_string(&plan).expect("serialize plan");
    let back: PeriodPlan = serde_json::from_str(&json).expect("deserialize plan");
    assert_eq!(back.periods.len(), plan.periods.len());
}

#[test]
fn shift_walks_plans_contiguously_in_each_calendar() {
    // Oracle independent of `shift`: consecutive plan periods meet, and each
    // period is the one `from_date` finds for its own start.
    let october = PeriodCalendar::Fiscal(FiscalConfig::new(10, 1).expect("valid fiscal start"));
    for (range, calendar) in [
        ("2023D300..2025D60", PeriodCalendar::Gregorian),
        ("2019W40..2021W20", PeriodCalendar::Gregorian),
        ("2023M11..2026M02", PeriodCalendar::Gregorian),
        ("FY2023D300..FY2025D60", october),
        ("FY2023W40..FY2025W20", october),
        ("FY2023Q3..FY2026Q2", october),
    ] {
        let plan = match calendar {
            PeriodCalendar::Gregorian => build_periods(range, None),
            PeriodCalendar::Fiscal(config) => build_fiscal_periods(range, config, None),
        }
        .expect("plan");
        for pair in plan.periods.windows(2) {
            assert_eq!(
                pair[0].end, pair[1].start,
                "{range}: {} -> {}",
                pair[0].id, pair[1].id
            );
            assert_eq!(pair[0].id.shift(1, calendar), Ok(pair[1].id), "{range}");
            assert_eq!(pair[1].id.shift(-1, calendar), Ok(pair[0].id), "{range}");
        }
        for period in &plan.periods {
            assert_eq!(
                PeriodId::from_date(period.start, period.id.kind(), calendar.fiscal_config()),
                Ok(period.id)
            );
        }
        let first = plan.periods[0].id;
        let last = plan.periods[plan.periods.len() - 1].id;
        let span = i32::try_from(plan.periods.len() - 1).expect("span");
        assert_eq!(first.shift(span, calendar), Ok(last), "{range}");
    }
}

#[test]
fn shift_pins_calendar_boundaries_and_context() {
    let october = PeriodCalendar::Fiscal(FiscalConfig::new(10, 1).expect("valid fiscal start"));
    // A zero shift still validates the source: FY2025 (Oct 2024 - Sep 2025) has 365 days.
    assert!(
        PeriodId::from_str("FY2025D366")
            .expect("symbolic")
            .shift(0, october)
            .is_err()
    );
    // FY2024 (Oct 2023 - Sep 2024) holds February 29, so it has 366 days.
    let leap_last = PeriodId::from_str("FY2024D366").expect("symbolic");
    assert_eq!(
        leap_last.shift(1, october),
        Ok(PeriodId::from_str("FY2025D001").expect("day"))
    );
    let w53 = PeriodId::from_str("FY2025W53").expect("symbolic");
    assert_eq!(
        w53.shift(1, october),
        Ok(PeriodId::from_str("FY2026W01").expect("week"))
    );
    // Context mismatch in both directions.
    assert!(
        PeriodId::quarter(2025, 1)
            .expect("q")
            .shift(1, october)
            .is_err()
    );
    assert!(
        PeriodId::from_str("FY2025Q1")
            .expect("q")
            .shift(1, PeriodCalendar::Gregorian)
            .is_err()
    );
    let day = PeriodId::from_str("FY2025D010").expect("fiscal day");
    assert!(day.shift(i32::MAX, october).is_err());
    assert!(day.shift(i32::MIN, october).is_err());
}

#[test]
fn daily_next_rolls_year_on_last_ordinal() {
    let last = PeriodId::day(2023, 365).expect("valid period fixture");
    let next = last.shift(1, PeriodCalendar::Gregorian).expect("next day");
    assert_eq!(next, PeriodId::day(2024, 1).expect("valid period fixture"));
}

#[test]
fn quarterly_semi_and_annual_stepping() {
    assert_eq!(
        PeriodId::quarter(2025, 4)
            .expect("valid period fixture")
            .shift(1, PeriodCalendar::Gregorian)
            .expect("nq"),
        PeriodId::quarter(2026, 1).expect("valid period fixture")
    );
    assert_eq!(
        PeriodId::half(2025, 2)
            .expect("valid period fixture")
            .shift(-1, PeriodCalendar::Gregorian)
            .expect("ph"),
        PeriodId::half(2025, 1).expect("valid period fixture")
    );
    assert_eq!(
        PeriodId::annual(2025)
            .expect("valid annual period")
            .shift(1, PeriodCalendar::Gregorian)
            .expect("na"),
        PeriodId::annual(2026).expect("valid annual period")
    );
}

#[test]
fn parse_id_rejects_bad_ranges() {
    assert!(PeriodId::from_str("2025Q5").is_err());
    assert!(PeriodId::from_str("2025M13").is_err());
    assert!(PeriodId::from_str("2025W99").is_err());
    assert!(PeriodId::from_str("2025D500").is_err());
}

#[test]
fn fiscal_config_default_is_calendar_year() {
    assert_eq!(FiscalConfig::calendar_year(), FiscalConfig::calendar_year());
}

#[test]
fn period_kind_annualization_factor_is_const() {
    const DAILY: f64 = PeriodKind::Daily.periods_per_year() as f64;
    assert_eq!(DAILY, 252.0);
}

#[test]
fn period_builders_reject_unsupported_dates_and_excessive_work() {
    for range in [
        "1..2147483647",
        "2147483647..2147483647",
        "9999D365..D365",
        "9999W52..W52",
        "2025D1..FY2025D2",
        "2025Q1..FY2025Q2",
    ] {
        assert!(build_periods(range, None).is_err(), "{range}");
    }
    let err = build_periods("1D1..9998D365", None).expect_err("bounded period count");
    assert!(err.to_string().contains("100000-period limit"));
    let config = FiscalConfig::new(10, 1).expect("valid fiscal start");
    assert!(build_fiscal_periods("FY2147483647..FY2147483647", config, None).is_err());
    assert!(build_fiscal_periods("FY1D1..FY9999D365", config, None).is_err());
}

#[test]
fn period_builder_does_not_step_beyond_the_requested_terminal_period() {
    let plan = build_periods("9999D364..D364", None).expect("exclusive end is representable");
    assert_eq!(plan.periods.len(), 1);
    assert_eq!(plan.periods[0].end, Date::MAX);
    let first_half = build_periods("9999H1..H1", None).expect("half-year end is representable");
    assert_eq!(first_half.periods[0].end.month(), Month::July);
}

#[test]
fn period_year_stepping_rejects_integer_overflow() {
    // Gregorian years whose bounds are unrepresentable cannot be built.
    assert!(PeriodId::annual(i32::MAX).is_err());
    assert!(PeriodId::annual(i32::MIN).is_err());
    let last: PeriodId = "FY2147483647".parse().expect("symbolic fiscal year");
    let first: PeriodId = "FY-2147483648".parse().expect("symbolic fiscal year");
    let october = PeriodCalendar::Fiscal(FiscalConfig::new(10, 1).expect("valid fiscal start"));
    assert!(last.shift(1, october).is_err());
    assert!(first.shift(-1, october).is_err());
}

#[test]
fn from_date_matches_calendar_and_fiscal_plan_boundaries() {
    let kinds = [
        (PeriodKind::Daily, "D001"),
        (PeriodKind::Weekly, "W01"),
        (PeriodKind::Monthly, "M01"),
        (PeriodKind::Quarterly, "Q1"),
        (PeriodKind::SemiAnnual, "H1"),
        (PeriodKind::Annual, ""),
    ];
    let configs = [
        None,
        Some(FiscalConfig::calendar_year()),
        Some(FiscalConfig::new(10, 1).expect("valid fiscal start")),
        Some(FiscalConfig::new(4, 6).expect("valid fiscal start")),
        Some(FiscalConfig::new(1, 15).unwrap()),
        Some(FiscalConfig::new(1, 31).unwrap()),
        Some(FiscalConfig::new(2, 29).unwrap()),
    ];
    for config in configs {
        for (kind, suffix) in kinds {
            let prefix = if config.is_some() { "FY" } else { "" };
            let range = format!("{prefix}2024{suffix}..{prefix}2025{suffix}");
            let plan = match config {
                Some(config) => build_fiscal_periods(&range, config, None),
                None => build_periods(&range, None),
            }
            .expect("representable period plan");
            for period in plan.periods {
                let midpoint = period
                    .start
                    .add_days((period.end - period.start).whole_days() / 2)
                    .unwrap();
                for date in [period.start, midpoint, period.end.previous_day().unwrap()] {
                    assert_eq!(
                        PeriodId::from_date(date, kind, config).unwrap(),
                        period.id,
                        "{date}, {kind}, {config:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn from_date_observes_midmonth_clamping_leap_days_and_partial_weeks() {
    let federal = Some(FiscalConfig::new(10, 1).expect("valid fiscal start"));
    assert_eq!(
        PeriodId::from_date(d(2025, Month::September, 30), PeriodKind::Weekly, federal)
            .unwrap()
            .to_string(),
        "FY2025W53"
    );
    assert_eq!(
        PeriodId::from_date(d(2025, Month::October, 1), PeriodKind::Weekly, federal)
            .unwrap()
            .to_string(),
        "FY2026W01"
    );
    let uk = Some(FiscalConfig::new(4, 6).expect("valid fiscal start"));
    assert_eq!(
        PeriodId::from_date(d(2024, Month::April, 5), PeriodKind::Annual, uk)
            .unwrap()
            .to_string(),
        "FY2024"
    );
    assert_eq!(
        PeriodId::from_date(d(2024, Month::April, 6), PeriodKind::Annual, uk)
            .unwrap()
            .to_string(),
        "FY2025"
    );
    let january = Some(FiscalConfig::new(1, 15).unwrap());
    assert_eq!(
        PeriodId::from_date(d(2024, Month::February, 14), PeriodKind::Monthly, january)
            .unwrap()
            .to_string(),
        "FY2025M01"
    );
    assert_eq!(
        PeriodId::from_date(d(2024, Month::February, 15), PeriodKind::Monthly, january)
            .unwrap()
            .to_string(),
        "FY2025M02"
    );
    let month_end = Some(FiscalConfig::new(1, 31).unwrap());
    assert_eq!(
        PeriodId::from_date(d(2024, Month::February, 28), PeriodKind::Monthly, month_end)
            .unwrap()
            .to_string(),
        "FY2025M01"
    );
    assert_eq!(
        PeriodId::from_date(d(2024, Month::February, 29), PeriodKind::Monthly, month_end)
            .unwrap()
            .to_string(),
        "FY2025M02"
    );
    let february = Some(FiscalConfig::new(2, 29).unwrap());
    assert_eq!(
        PeriodId::from_date(d(2023, Month::March, 27), PeriodKind::Monthly, february)
            .unwrap()
            .to_string(),
        "FY2024M01"
    );
    assert_eq!(
        PeriodId::from_date(d(2023, Month::March, 28), PeriodKind::Monthly, february)
            .unwrap()
            .to_string(),
        "FY2024M02"
    );
    assert_eq!(
        PeriodId::from_date(d(2024, Month::January, 1), PeriodKind::Weekly, None)
            .unwrap()
            .to_string(),
        "2024W01"
    );
    assert_eq!(
        PeriodId::from_date(d(2021, Month::January, 1), PeriodKind::Weekly, None)
            .unwrap()
            .to_string(),
        "2020W53"
    );
}

#[test]
fn from_date_rejects_unrepresentable_period_boundaries() {
    for kind in [
        PeriodKind::Daily,
        PeriodKind::Weekly,
        PeriodKind::Monthly,
        PeriodKind::Quarterly,
        PeriodKind::SemiAnnual,
        PeriodKind::Annual,
    ] {
        for config in [
            None,
            Some(FiscalConfig::calendar_year()),
            Some(FiscalConfig::new(10, 1).expect("valid fiscal start")),
        ] {
            assert!(
                PeriodId::from_date(Date::MAX, kind, config).is_err(),
                "{kind}, {config:?}"
            );
        }
    }
    assert!(PeriodId::from_date(Date::MIN, PeriodKind::Daily, None).is_ok());
    assert!(
        PeriodId::from_date(
            Date::MIN,
            PeriodKind::Monthly,
            Some(FiscalConfig::new(1, 15).unwrap())
        )
        .is_err()
    );
    assert!(
        PeriodId::from_date(Date::MAX.previous_day().unwrap(), PeriodKind::Daily, None).is_ok()
    );
}

#[test]
fn period_grammar_is_exact_signed_and_round_trips() {
    for (input, canonical) in [
        ("+2025q1", "2025Q1"),
        ("fy2025q1", "FY2025Q1"),
        (" 02025M3 ", "2025M03"),
        ("2025d7", "2025D007"),
        ("-5Q1", "-5Q1"),
        ("-5", "-5"),
        ("0H2", "0H2"),
        ("FY-3W53", "FY-3W53"),
    ] {
        let id: PeriodId = input.parse().unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_eq!(id.to_string(), canonical, "{input}");
        assert_eq!(canonical.parse::<PeriodId>().expect("canonical"), id);
    }
    for bad in [
        "2025Q+1",
        "2025Q0",
        "2025Q",
        "2025QQ1",
        "2025 Q1",
        "2025-Q1",
        "2025Q1.0",
        "2025Q5",
        "2025M13",
        "2025D366",
        "2026W54",
        "FYFY2025",
        "Q1",
        "",
        "2025A",
        "2025Q1M2",
        "99999999999Q1",
        "1e3",
    ] {
        assert!(bad.parse::<PeriodId>().is_err(), "{bad:?} must be rejected");
    }
    // FY text is symbolic standalone: capacities only.
    assert!("FY2025D366".parse::<PeriodId>().is_ok());
    assert!("FY2025W54".parse::<PeriodId>().is_err());
}

#[test]
fn ranges_classify_endpoints_by_grammar() {
    assert_eq!(
        build_periods("-5Q1..-4Q2", None)
            .expect("negative years")
            .periods
            .len(),
        6
    );
    assert_eq!(
        build_periods("-5..-3", None)
            .expect("negative annual")
            .periods
            .len(),
        3
    );
    assert!(build_periods("2025Q1..QQ4", None).is_err());
    assert!(build_periods("2025Q1..M4", None).is_err());
    assert!(build_periods("2025Q1..Q4..Q4", None).is_err());
    assert!(build_periods("2024..A", None).is_err());
    assert!(
        build_periods("FY2025Q1..Q2", None).is_err(),
        "Gregorian builders reject FY"
    );
    assert!(build_periods("2025Q1..Q2", Some("FY2025Q1")).is_err());
    let fiscal = FiscalConfig::new(10, 1).expect("valid fiscal start");
    let plan = build_fiscal_periods("2025Q1..FY2025Q2", fiscal, None).expect("fiscal builder");
    assert!(plan.periods.iter().all(|period| period.id.is_fiscal()));
}

#[test]
fn constructors_validate_actual_gregorian_bounds() {
    assert!(PeriodId::annual(2025).is_ok());
    assert!(PeriodId::annual(i32::MAX).is_err());
    assert!(PeriodId::day(2024, 366).is_ok());
    assert!(PeriodId::day(2025, 366).is_err());
    assert!(PeriodId::week(2026, 53).is_ok());
    assert!(PeriodId::week(2025, 53).is_err());
}
