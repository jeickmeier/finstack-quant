use super::*;

use std::str::FromStr;

fn d(year: i32, month: Month, day: u8) -> Date {
    crate::dates::create_date(year, month, day).expect("valid date")
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
        .next()
        .expect("next week");
    assert_eq!(next, PeriodId::week(2022, 1).expect("valid period fixture"));
}

#[test]
fn prev_rolls_to_previous_iso_year_last_week() {
    let prev = PeriodId::week(2022, 1)
        .expect("valid period fixture")
        .prev()
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
        PeriodKind::Daily.prior_observation_date(first),
        d(2024, Month::January, 2)
    );
}

#[test]
fn prior_observation_date_weekly_is_seven_days() {
    let first = d(2024, Month::January, 8);
    assert_eq!(
        PeriodKind::Weekly.prior_observation_date(first),
        d(2024, Month::January, 1)
    );
}

#[test]
fn prior_observation_date_month_end_clamps() {
    let jan31 = d(2023, Month::January, 31);
    assert_eq!(
        PeriodKind::Monthly.prior_observation_date(jan31),
        d(2022, Month::December, 31)
    );
    let feb28 = d(2023, Month::February, 28);
    assert_eq!(
        PeriodKind::Monthly.prior_observation_date(feb28),
        d(2023, Month::January, 28)
    );
    let mar31 = d(2023, Month::March, 31);
    assert_eq!(
        PeriodKind::Monthly.prior_observation_date(mar31),
        d(2023, Month::February, 28)
    );
    assert_eq!(
        PeriodKind::Quarterly.prior_observation_date(jan31),
        d(2022, Month::October, 31)
    );
    assert_eq!(
        PeriodKind::SemiAnnual.prior_observation_date(jan31),
        d(2022, Month::July, 31)
    );
    assert_eq!(
        PeriodKind::Annual.prior_observation_date(jan31),
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
    assert_eq!(
        PeriodKind::from_str("semiannual"),
        Ok(PeriodKind::SemiAnnual)
    );
    assert_eq!(PeriodKind::from_str("Y"), Ok(PeriodKind::Annual));
    assert_eq!(PeriodKind::from_str("B"), Ok(PeriodKind::Daily));
    assert_eq!(PeriodKind::from_str("Q"), Ok(PeriodKind::Quarterly));
    for noncanonical in ["q", "SemiAnnual", " quarterly"] {
        assert!(PeriodKind::from_str(noncanonical).is_err());
    }
    assert!(PeriodKind::from_str("unknown").is_err());

    assert_eq!(PeriodKind::Daily.periods_per_year(), 252);
    assert_eq!(PeriodKind::Quarterly.annualization_factor(), 4.0);
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
fn period_id_ordering_mixed_frequencies_same_year() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let m1 = PeriodId::month(2025, 1).expect("valid period fixture");
    assert!(q1 > m1);
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
    let uk = FiscalConfig::uk();
    assert_eq!(uk.start_month, 4);
    let feb_edge = FiscalConfig::new(2, 30).expect("feb clamp path");
    let plan = build_fiscal_periods("2025Q1..Q1", feb_edge, None).expect("fiscal quarter");
    assert_eq!(plan.periods.len(), 1);
}

#[test]
fn build_fiscal_periods_us_federal_and_monthly() {
    let cfg = FiscalConfig::us_federal();
    let qs = build_fiscal_periods("2025Q1..Q2", cfg, None).expect("FY quarters");
    assert_eq!(qs.periods.len(), 2);

    let jp = FiscalConfig::japan();
    let ms = build_fiscal_periods("2025M01..M02", jp, None).expect("FY months");
    assert_eq!(ms.periods.len(), 2);
}

#[test]
fn fiscal_daily_and_weekly_bounds_stay_inside_fiscal_year() {
    let cfg = FiscalConfig::us_federal();
    let day = build_fiscal_periods("2025D001..D001", cfg, None).expect("fiscal day");
    assert_eq!(day.periods[0].start, d(2024, Month::October, 1));
    assert_eq!(day.periods[0].end, d(2024, Month::October, 2));

    let week = build_fiscal_periods("2020W53..W53", cfg, None).expect("fiscal week 53");
    assert_eq!(week.periods[0].start, d(2020, Month::September, 29));
    assert_eq!(week.periods[0].end, d(2020, Month::October, 1));
}

#[test]
fn fiscal_ranges_include_partial_week_53_and_leap_day_366() {
    let federal = FiscalConfig::us_federal();
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
    let federal = FiscalConfig::us_federal();
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

    assert_eq!(week_52.next_fiscal(federal).expect("FY week 53"), week_53);
    assert_eq!(week_53.next_fiscal(federal).expect("next FY"), next_year);
    assert_eq!(
        next_year.prev_fiscal(federal).expect("previous FY week"),
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
            .next()
            .expect("ISO next remains Gregorian"),
        PeriodId::week(2026, 1).expect("valid period fixture")
    );
}

#[test]
fn fiscal_week_53_display_parse_and_serde_roundtrip() {
    let federal = FiscalConfig::us_federal();
    let plan = build_fiscal_periods("FY2025W52..W52", federal, None).expect("fiscal weekly period");
    let week_53 = plan.periods[0]
        .id
        .next_fiscal(federal)
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
fn fiscal_ids_reject_ambiguous_gregorian_stepping() {
    let week = PeriodId::from_str("FY2025W52").expect("fiscal week");

    let next_error = week.next().expect_err("fiscal next requires a calendar");
    assert!(next_error.to_string().contains("next_fiscal"));

    let prev_error = week.prev().expect_err("fiscal prev requires a calendar");
    assert!(prev_error.to_string().contains("prev_fiscal"));

    assert_eq!(
        PeriodId::from_str("2025W52")
            .expect("ISO week")
            .next()
            .expect("ISO next")
            .to_string(),
        "2026W01"
    );
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
fn period_plan_iter_and_serde_roundtrip() {
    let plan = build_periods("2025Q1..Q2", None).expect("plan");
    let count = plan.iter().count();
    assert_eq!(count, 2);
    let json = serde_json::to_string(&plan).expect("serialize plan");
    let back: PeriodPlan = serde_json::from_str(&json).expect("deserialize plan");
    assert_eq!(back.periods.len(), plan.periods.len());
}

#[test]
fn daily_next_rolls_year_on_last_ordinal() {
    let last = PeriodId::day(2023, 365).expect("valid period fixture");
    let next = last.next().expect("next day");
    assert_eq!(next, PeriodId::day(2024, 1).expect("valid period fixture"));
}

#[test]
fn quarterly_semi_and_annual_stepping() {
    assert_eq!(
        PeriodId::quarter(2025, 4)
            .expect("valid period fixture")
            .next()
            .expect("nq"),
        PeriodId::quarter(2026, 1).expect("valid period fixture")
    );
    assert_eq!(
        PeriodId::half(2025, 2)
            .expect("valid period fixture")
            .prev()
            .expect("ph"),
        PeriodId::half(2025, 1).expect("valid period fixture")
    );
    assert_eq!(
        PeriodId::annual(2025).next().expect("na"),
        PeriodId::annual(2026)
    );
}

#[test]
fn parse_id_rejects_bad_ranges() {
    assert!(PeriodId::from_str("2025Q5").is_err());
    assert!(PeriodId::from_str("2025M13").is_err());
    assert!(PeriodId::from_str("2025W99").is_err());
    assert!(PeriodId::from_str("2025D500").is_err());
}
