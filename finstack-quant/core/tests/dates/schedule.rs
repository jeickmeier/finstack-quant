//! Tests for schedule iterator functionality.

use super::common::make_date;
use finstack_quant_core::dates::{
    BusinessDayConvention, Schedule, ScheduleErrorPolicy, StubKind, Tenor,
};

#[test]
fn eom_rejects_incompatible_schedule_rules() {
    let builder = || Schedule::builder(make_date(2025, 1, 1), make_date(2025, 12, 20)).unwrap();
    for frequency in [Tenor::daily(), Tenor::weekly()] {
        assert!(builder()
            .frequency(frequency)
            .end_of_month(true)
            .build()
            .is_err());
    }
    assert!(builder().cds_imm().end_of_month(true).build().is_err());
    assert!(builder().end_of_month(true).cds_imm().build().is_err());
    assert!(builder().imm().end_of_month(true).build().is_err());
    let weekly = Schedule::builder(make_date(2025, 1, 1), make_date(2025, 1, 29))
        .unwrap()
        .frequency(Tenor::weekly())
        .build()
        .unwrap();
    assert_eq!(weekly.dates.len(), 5);
    let cds = builder().cds_imm().build().unwrap();
    assert!(cds.dates.iter().all(|date| date.day() == 20));
}

#[test]
fn schedule_calendar_setters_replace_the_previous_representation() {
    let builder = || Schedule::builder(make_date(2025, 6, 4), make_date(2025, 7, 4)).unwrap();
    let replaced_id = builder()
        .adjust_with(BusinessDayConvention::Following, "usny")
        .adjust_with(BusinessDayConvention::Following, "weekends_only")
        .build()
        .unwrap();
    assert_eq!(
        replaced_id.payment_dates.last(),
        Some(&make_date(2025, 7, 4))
    );
    let replaced_calendar = builder()
        .adjust_with(BusinessDayConvention::Following, "weekends_only")
        .adjust_with(BusinessDayConvention::Following, "usny")
        .build()
        .unwrap();
    assert_eq!(
        replaced_calendar.payment_dates.last(),
        Some(&make_date(2025, 7, 7))
    );
    // Replacing an invalid deferred ID must discard its pending lookup too.
    assert!(builder()
        .adjust_with(BusinessDayConvention::Following, "invalid_calendar")
        .adjust_with(BusinessDayConvention::Following, "weekends_only")
        .build()
        .is_ok());
}

#[test]
fn eom_roll_grid_accepts_regular_quarters_from_february() {
    for (year, feb_end) in [(2024, 29), (2025, 28)] {
        for stub in [
            StubKind::None,
            StubKind::ShortBack,
            StubKind::LongBack,
            StubKind::ShortFront,
            StubKind::LongFront,
        ] {
            let schedule = Schedule::builder(make_date(year, 2, feb_end), make_date(year, 8, 31))
                .unwrap()
                .frequency(Tenor::quarterly())
                .end_of_month(true)
                .stub_rule(stub)
                .build()
                .unwrap();
            assert_eq!(
                schedule.dates.as_slice(),
                [
                    make_date(year, 2, feb_end),
                    make_date(year, 5, 31),
                    make_date(year, 8, 31)
                ]
            );
        }
    }
    assert!(
        Schedule::builder(make_date(2025, 2, 28), make_date(2025, 8, 28))
            .unwrap()
            .frequency(Tenor::quarterly())
            .end_of_month(true)
            .build()
            .is_err()
    );
}

#[test]
fn test_basic_schedule() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 4, 15);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], make_date(2025, 1, 15));
    assert_eq!(dates[1], make_date(2025, 2, 15));
    assert_eq!(dates[2], make_date(2025, 3, 15));
    assert_eq!(dates[3], make_date(2025, 4, 15));
}

#[test]
fn test_quarterly_schedule_with_short_back_stub() {
    // Period not evenly divisible by quarterly frequency
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 11, 1); // 10 months

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::ShortBack)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Should get: Jan, Apr, Jul, Oct, Nov (short stub at end)
    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 4, 1));
    assert_eq!(dates[2], make_date(2025, 7, 1));
    assert_eq!(dates[3], make_date(2025, 10, 1));
    assert_eq!(dates[4], make_date(2025, 11, 1));
}

#[test]
fn test_stub_none_rejects_non_integer_tenor() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 11, 1); // 10 months, not a multiple of 3

    let result = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::None)
        .build();

    assert!(
        result.is_err(),
        "StubKind::None must reject non-integer tenor"
    );
}

#[test]
fn test_short_front_stub() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 11, 1);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::ShortFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Should get: Jan, Feb, May, Aug, Nov (short stub at front)
    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 2, 1));
    assert_eq!(dates[2], make_date(2025, 5, 1));
    assert_eq!(dates[3], make_date(2025, 8, 1));
    assert_eq!(dates[4], make_date(2025, 11, 1));
}

#[test]
fn test_day_based_frequency() {
    let start = make_date(2025, 1, 1); // Wednesday
    let end = make_date(2025, 1, 15);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::weekly()) // 7 days
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2025, 1, 1)); // Wed
    assert_eq!(dates[1], make_date(2025, 1, 8)); // Wed + 7 days
    assert_eq!(dates[2], make_date(2025, 1, 15)); // Wed + 14 days
}

#[test]
fn test_single_date_schedule() {
    let date = make_date(2025, 1, 15);

    let result = Schedule::builder(date, date)
        .unwrap()
        .frequency(Tenor::monthly())
        .build();

    assert!(
        result.is_err(),
        "start == end is no longer valid; must have start < end"
    );
}

#[test]
fn test_schedule_with_business_day_adjustment() {
    // TARGET2 closes on New Year's Day (Wednesday 1 Jan 2025).
    let start = make_date(2025, 1, 1); // Holiday Wednesday
    let end = make_date(2025, 1, 8);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::weekly())
        .adjust_with(BusinessDayConvention::Following, "target2")
        .build()
        .unwrap();
    let dates: Vec<_> = schedule.dates.clone();

    // Accrual grid stays unadjusted; payment dates are adjusted.
    assert_eq!(dates.len(), 2);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 1, 8));
    assert_eq!(schedule.payment_dates, vec![make_date(2025, 1, 8)]);
}

#[test]
fn test_schedule_builder_with_adjustment() {
    // TARGET2: New Year's Day is the only Jan-Mar 2025 holiday.
    let start = make_date(2025, 1, 1); // Wednesday (holiday)
    let end = make_date(2025, 3, 1);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .adjust_with(BusinessDayConvention::Following, "target2")
        .build()
        .unwrap();

    // Accrual dates stay on the unadjusted roll grid.
    assert_eq!(schedule.dates[0], make_date(2025, 1, 1));
    assert_eq!(schedule.dates[1], make_date(2025, 2, 1));
    assert_eq!(schedule.dates[2], make_date(2025, 3, 1));
    // Period-end payments: Feb 1 (Sat) → Feb 3, Mar 1 (Sat) → Mar 3.
    assert_eq!(
        schedule.payment_dates,
        vec![make_date(2025, 2, 3), make_date(2025, 3, 3)]
    );
}

#[test]
fn test_uneven_period_clamping() {
    // StubKind::None now rejects non-integer tenors; use ShortBack for clamping
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 1, 20); // Not a multiple of monthly frequency

    // Verify StubKind::None rejects it
    let none_result = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build();
    assert!(
        none_result.is_err(),
        "StubKind::None must reject non-integer tenor"
    );

    // ShortBack clamps to end date
    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 2);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 1, 20));
}

#[test]
fn test_long_front_stub() {
    // Test LongFront creates a longer first period
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 11, 1);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::LongFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Regular quarters backward from the end date are Nov, Aug, May, Feb.
    // The residual Jan 1 -> Feb 1 stub is MERGED with the first regular
    // period (Feb -> May), producing a genuinely long 4-month first period
    // Jan 1 -> May 1. (Keeping the Feb 1 anchor — the previous behavior —
    // made LongFront identical to ShortFront.)
    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], make_date(2025, 1, 1)); // Start date
    assert_eq!(dates[1], make_date(2025, 5, 1)); // End of the long front stub
    assert_eq!(dates[2], make_date(2025, 8, 1)); // Regular quarter
    assert_eq!(dates[3], make_date(2025, 11, 1)); // End date
}

#[test]
fn test_long_front_stub_aligned_schedule_has_no_stub() {
    // When the range divides evenly, LongFront must emit all regular dates
    // (no merge: there is no stub to merge).
    let start = make_date(2025, 1, 1);
    let end = make_date(2026, 1, 1); // exactly 4 quarters

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::LongFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 4, 1));
    assert_eq!(dates[2], make_date(2025, 7, 1));
    assert_eq!(dates[3], make_date(2025, 10, 1));
    assert_eq!(dates[4], make_date(2026, 1, 1));
}

#[test]
fn test_long_front_stub_sub_period_range_is_single_period() {
    // Range shorter than two tenors: the stub merges with the single regular
    // period into one long period [start, end].
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 6, 1); // 4.5 months on a quarterly schedule

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::LongFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates, vec![make_date(2025, 1, 15), make_date(2025, 6, 1)]);
}

#[test]
fn test_no_roll_day_drift_backward_through_short_month() {
    // Backward semi-annual generation from Aug 31 must pass through
    // Feb 28 and come back to Aug 31 (anchor-based generation), not drift
    // to Aug 28 (chained generation off the clamped Feb date).
    let start = make_date(2024, 8, 31);
    let end = make_date(2026, 8, 31);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::semi_annual())
        .stub_rule(StubKind::ShortFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![
            make_date(2024, 8, 31),
            make_date(2025, 2, 28),
            make_date(2025, 8, 31), // not Aug 28
            make_date(2026, 2, 28),
            make_date(2026, 8, 31),
        ]
    );
}

#[test]
fn test_no_roll_day_drift_forward_through_short_month() {
    // Forward monthly generation from Jan 31 (eom = false) must clamp each
    // month independently: Feb 28, Mar 31, Apr 30, May 31 — not
    // Feb 28 -> Mar 28 -> ...
    let start = make_date(2025, 1, 31);
    let end = make_date(2025, 5, 31);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![
            make_date(2025, 1, 31),
            make_date(2025, 2, 28),
            make_date(2025, 3, 31),
            make_date(2025, 4, 30),
            make_date(2025, 5, 31),
        ]
    );
}

#[test]
fn test_stub_none_accepts_aligned_month_end_schedule() {
    // StubKind::None must not spuriously error on a schedule whose anchors
    // are aligned but pass through short months (Jan 31 -> Jul 31 monthly).
    let start = make_date(2025, 1, 31);
    let end = make_date(2025, 7, 31);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![
            make_date(2025, 1, 31),
            make_date(2025, 2, 28),
            make_date(2025, 3, 31),
            make_date(2025, 4, 30),
            make_date(2025, 5, 31),
            make_date(2025, 6, 30),
            make_date(2025, 7, 31),
        ]
    );
}

#[test]
fn test_long_back_stub() {
    // Test LongBack creates a longer last period
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 11, 1);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::LongBack)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Should create regular quarters from start: Jan, Apr, Jul
    // Then create a long back period from Jul to Nov (4 months)
    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], make_date(2025, 1, 1)); // Start date
    assert_eq!(dates[1], make_date(2025, 4, 1)); // Regular quarter
    assert_eq!(dates[2], make_date(2025, 7, 1)); // Regular quarter
    assert_eq!(dates[3], make_date(2025, 11, 1)); // End date (long back period)
}

#[test]
fn test_long_back_even_schedule_emits_all_dates() {
    // When the schedule divides evenly, LongBack must still emit every period date.
    // Regression: the penultimate date was previously dropped.
    let start = make_date(2025, 1, 1);
    let end = make_date(2026, 1, 1); // exactly 4 quarters

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::LongBack)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], make_date(2025, 1, 1));
    assert_eq!(dates[1], make_date(2025, 4, 1));
    assert_eq!(dates[2], make_date(2025, 7, 1));
    assert_eq!(dates[3], make_date(2025, 10, 1));
    assert_eq!(dates[4], make_date(2026, 1, 1));
}

#[test]
fn stub_schedules_terminate_at_finite_date_boundaries() {
    let maximum = time::Date::MAX;
    let minimum = time::Date::MIN;
    for frequency in [
        Tenor::monthly(),
        Tenor::annual(),
        Tenor::weekly(),
        Tenor::parse("2D").unwrap(),
    ] {
        for stub in [StubKind::ShortBack, StubKind::LongBack] {
            let start = make_date(maximum.year(), 12, 30);
            let schedule = Schedule::builder(start, maximum)
                .unwrap()
                .frequency(frequency)
                .stub_rule(stub)
                .build()
                .expect("a back stub needs no unrepresentable successor");
            assert_eq!(schedule.dates, vec![start, maximum]);
        }
        for stub in [StubKind::ShortFront, StubKind::LongFront] {
            let end = make_date(minimum.year(), 1, 2);
            let schedule = Schedule::builder(minimum, end)
                .unwrap()
                .frequency(frequency)
                .stub_rule(stub)
                .build()
                .expect("a front stub needs no unrepresentable predecessor");
            assert_eq!(schedule.dates, vec![minimum, end]);
        }
    }
}

#[test]
fn monthly_stubs_preserve_contractual_boundary_endpoints_with_eom() {
    let maximum = time::Date::MAX;
    let minimum = time::Date::MIN;
    for eom in [false, true] {
        for stub in [StubKind::ShortBack, StubKind::LongBack] {
            let start = make_date(maximum.year(), 12, 1);
            let schedule = Schedule::builder(start, maximum)
                .unwrap()
                .frequency(Tenor::monthly())
                .stub_rule(stub)
                .end_of_month(eom)
                .build()
                .unwrap();
            assert_eq!(schedule.dates, vec![start, maximum]);
        }
        for stub in [StubKind::ShortFront, StubKind::LongFront] {
            let end = make_date(minimum.year(), 1, 31);
            let schedule = Schedule::builder(minimum, end)
                .unwrap()
                .frequency(Tenor::monthly())
                .stub_rule(stub)
                .end_of_month(eom)
                .build()
                .unwrap();
            assert_eq!(schedule.dates, vec![minimum, end]);
        }
    }
}

#[test]
fn aligned_long_stubs_keep_regular_anchors_at_both_date_boundaries() {
    let maximum = time::Date::MAX;
    let minimum = time::Date::MIN;
    let cases = [
        vec![
            make_date(maximum.year(), 10, 31),
            make_date(maximum.year(), 11, 30),
            maximum,
        ],
        vec![
            minimum,
            make_date(minimum.year(), 2, 1),
            make_date(minimum.year(), 3, 1),
        ],
    ];
    for expected in cases {
        for stub in [StubKind::LongBack, StubKind::LongFront] {
            let schedule = Schedule::builder(expected[0], expected[2])
                .unwrap()
                .frequency(Tenor::monthly())
                .stub_rule(stub)
                .build()
                .expect("aligned schedules do not need another comparison roll");
            assert_eq!(schedule.dates, expected);
        }
    }
}

#[test]
fn boundary_rolls_keep_noninteger_and_invalid_tenor_errors() {
    let start = make_date(time::Date::MAX.year(), 12, 1);
    let end = time::Date::MAX;
    let error = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build()
        .expect_err("no-stub schedules still require exact tenor alignment");
    assert!(error
        .to_string()
        .contains("requires the tenor to divide evenly"));

    // Unsupported tenor sizes are rejected before a schedule can be built.
    assert!(Tenor::parse("4294967295M").is_err());
}

#[test]
fn test_end_of_month_convention() {
    // EOM snaps INTERMEDIATE roll dates to month-end; the user-provided
    // start and end dates are contractual and emitted verbatim.
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 4, 15);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], make_date(2025, 1, 15)); // start unchanged
    assert_eq!(dates[1], make_date(2025, 2, 28)); // Feb 15 -> Feb 28
    assert_eq!(dates[2], make_date(2025, 3, 31)); // Mar 15 -> Mar 31
    assert_eq!(dates[3], make_date(2025, 4, 15)); // end unchanged
}

#[test]
fn stub_none_rejects_maturity_hidden_by_eom_snap() {
    let result = Schedule::builder(make_date(2025, 1, 15), make_date(2025, 2, 20))
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::None)
        .end_of_month(true)
        .build();
    assert!(
        result.is_err(),
        "EOM must not hide an undeclared final stub"
    );
}

#[test]
fn test_end_of_month_with_leap_year() {
    // Test EOM convention with leap year (intermediate snapping only)
    let start = make_date(2024, 1, 15); // 2024 is a leap year
    let end = make_date(2024, 3, 15);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2024, 1, 15)); // start unchanged
    assert_eq!(dates[1], make_date(2024, 2, 29)); // Feb 29 (leap year)
    assert_eq!(dates[2], make_date(2024, 3, 15)); // end unchanged
}

#[test]
fn test_eom_with_stub_conventions() {
    // EOM with stubs: intermediates snap, user start/end do not
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 5, 15);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::ShortBack)
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2025, 1, 15)); // start unchanged
    assert_eq!(dates[1], make_date(2025, 4, 30)); // Regular quarter -> Apr 30
    assert_eq!(dates[2], make_date(2025, 5, 15)); // end unchanged
}

#[test]
fn test_eom_jan30_roll_to_feb_leap_year() {
    // EOM: intermediate Feb roll snaps to month-end (Feb 29 in leap year)
    let start = make_date(2024, 1, 30);
    let end = make_date(2024, 3, 30);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2024, 1, 30)); // start unchanged
    assert_eq!(dates[1], make_date(2024, 2, 29)); // Feb -> 29 (leap)
    assert_eq!(dates[2], make_date(2024, 3, 30)); // end unchanged
}

#[test]
fn test_eom_jan30_roll_to_feb_non_leap() {
    // EOM: intermediate Feb roll snaps to month-end, Feb 28 in non-leap year
    let start = make_date(2025, 1, 30);
    let end = make_date(2025, 3, 30);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .stub_rule(StubKind::ShortBack)
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2025, 1, 30)); // start unchanged
    assert_eq!(dates[1], make_date(2025, 2, 28)); // Feb -> 28 (non-leap)
    assert_eq!(dates[2], make_date(2025, 3, 30)); // end unchanged
}

#[test]
fn test_eom_quarterly_through_feb() {
    // Quarterly schedule starting Jan 31 through May should handle Feb correctly
    let start = make_date(2024, 1, 31);
    let end = make_date(2024, 7, 31);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .end_of_month(true)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2024, 1, 31)); // Jan 31
    assert_eq!(dates[1], make_date(2024, 4, 30)); // Apr 30 (not 31)
    assert_eq!(dates[2], make_date(2024, 7, 31)); // Jul 31
}

#[test]
fn test_adjustment_collision_keeps_maturity_date() {
    // Daily schedule ending Fri Dec 27 2024 with the London Dec 25 + Dec 26
    // holidays. Accrual dates stay unadjusted (including the holidays).
    // Payment dates may collide after Following adjustment; they stay 1:1
    // with period ends rather than merging the accrual grid.
    let start = make_date(2024, 12, 24);
    let end = make_date(2024, 12, 27);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::daily())
        .adjust_with(BusinessDayConvention::Following, "gblo")
        .build()
        .unwrap();

    assert_eq!(
        schedule.dates,
        vec![
            make_date(2024, 12, 24),
            make_date(2024, 12, 25),
            make_date(2024, 12, 26),
            make_date(2024, 12, 27),
        ]
    );
    assert_eq!(
        schedule.dates.last().copied(),
        Some(end),
        "maturity must survive"
    );
    assert_eq!(
        schedule.payment_dates,
        vec![
            make_date(2024, 12, 27),
            make_date(2024, 12, 27),
            make_date(2024, 12, 27),
        ]
    );
}

// IMM Schedule Tests
//
// Standard IMM is a roll convention: regular periods roll on the third
// Wednesday of Mar/Jun/Sep/Dec at the schedule frequency, and the contractual
// start and end are always kept.

fn imm_dates(start: time::Date, end: time::Date) -> Vec<time::Date> {
    Schedule::builder(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .dates
}

#[test]
fn imm_schedule_stops_at_last_representable_roll() {
    let start = make_date(time::Date::MAX.year(), 1, 1);
    let end = time::Date::MAX;
    let dates = imm_dates(start, end);

    assert_eq!(dates.len(), 6);
    assert_eq!(dates[0], start);
    assert_eq!(
        dates[4],
        finstack_quant_core::dates::third_wednesday(time::Month::December, end.year())
            .expect("last IMM date")
    );
    assert_eq!(dates[5], end);
}

#[test]
fn test_imm_schedule_basic() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 12, 31);

    // Short initial stub to the March roll, quarterly IMM rolls, then a short
    // final stub to the contractual end.
    assert_eq!(
        imm_dates(start, end),
        vec![
            start,
            make_date(2025, 3, 19),
            make_date(2025, 6, 18),
            make_date(2025, 9, 17),
            make_date(2025, 12, 17),
            end,
        ]
    );
}

#[test]
fn test_imm_schedule_start_on_imm_date() {
    let start = make_date(2025, 3, 19); // Third Wednesday of March 2025
    let end = make_date(2025, 9, 30);

    assert_eq!(
        imm_dates(start, end),
        vec![start, make_date(2025, 6, 18), make_date(2025, 9, 17), end]
    );
}

#[test]
fn test_imm_schedule_start_after_first_imm() {
    let start = make_date(2025, 3, 20); // Day after March IMM
    let end = make_date(2025, 9, 30);

    assert_eq!(
        imm_dates(start, end),
        vec![start, make_date(2025, 6, 18), make_date(2025, 9, 17), end]
    );
}

#[test]
fn imm_schedule_on_cycle_endpoints_have_no_stubs() {
    let start = make_date(2025, 3, 19);
    let end = make_date(2026, 3, 18);

    let expected = vec![
        start,
        make_date(2025, 6, 18),
        make_date(2025, 9, 17),
        make_date(2025, 12, 17),
        end,
    ];
    assert_eq!(imm_dates(start, end), expected);
    let aligned = Schedule::builder(start, end)
        .unwrap()
        .imm()
        .stub_rule(StubKind::None)
        .build()
        .unwrap();
    assert_eq!(aligned.dates, expected);
}

#[test]
fn imm_schedule_without_roll_inside_is_a_single_period() {
    let start = make_date(2025, 3, 20); // day after the Mar 19 IMM date
    let end = make_date(2025, 4, 30); // before the Jun 18 IMM date

    assert_eq!(imm_dates(start, end), vec![start, end]);
    // An IMM date only at the far end is still a single period.
    let to_roll = make_date(2025, 6, 18);
    assert_eq!(imm_dates(start, to_roll), vec![start, to_roll]);
}

#[test]
fn test_imm_schedule_preserves_effective_date_front_anchor() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 6, 20);

    assert_eq!(
        imm_dates(start, end),
        vec![start, make_date(2025, 3, 19), make_date(2025, 6, 18), end]
    );
}

#[test]
fn test_imm_schedule_year_rollover() {
    let start = make_date(2025, 10, 1);
    let end = make_date(2026, 6, 30);

    assert_eq!(
        imm_dates(start, end),
        vec![
            start,
            make_date(2025, 12, 17),
            make_date(2026, 3, 18),
            make_date(2026, 6, 17),
            end,
        ]
    );
}

#[test]
fn imm_schedule_quarterly_direction_does_not_change_the_dates() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2026, 2, 10);
    let build = |stub| {
        Schedule::builder(start, end)
            .unwrap()
            .imm()
            .stub_rule(stub)
            .build()
            .unwrap()
            .dates
    };

    assert_eq!(build(StubKind::ShortFront), build(StubKind::ShortBack));
}

#[test]
fn imm_schedule_honours_frequency_and_generation_direction() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2026, 2, 10);
    let semi_annual = |stub| {
        Schedule::builder(start, end)
            .unwrap()
            .imm()
            .frequency(Tenor::semi_annual())
            .stub_rule(stub)
            .build()
            .unwrap()
            .dates
    };

    // Forward from the first IMM date on or after start (Mar 2025).
    assert_eq!(
        semi_annual(StubKind::ShortBack),
        vec![start, make_date(2025, 3, 19), make_date(2025, 9, 17), end,]
    );
    // Backward from the last IMM date on or before end (Dec 2025).
    assert_eq!(
        semi_annual(StubKind::ShortFront),
        vec![start, make_date(2025, 6, 18), make_date(2025, 12, 17), end,]
    );
}

#[test]
fn imm_schedule_long_stubs_merge_into_the_adjacent_regular_period() {
    let start = make_date(2025, 3, 19);
    let end = make_date(2026, 2, 10);
    let semi_annual = |stub| {
        Schedule::builder(start, end)
            .unwrap()
            .imm()
            .frequency(Tenor::semi_annual())
            .stub_rule(stub)
            .build()
            .unwrap()
            .dates
    };

    // Short back: Mar-25, Sep-25, then a short final stub to Feb-26.
    assert_eq!(
        semi_annual(StubKind::ShortBack),
        vec![start, make_date(2025, 9, 17), end]
    );
    // Long back folds that final stub into the last regular period.
    assert_eq!(semi_annual(StubKind::LongBack), vec![start, end]);

    // Backward from Dec-25: a short initial stub Mar-25 to Jun-25, or a long
    // one folded into the first regular period.
    assert_eq!(
        semi_annual(StubKind::ShortFront),
        vec![start, make_date(2025, 6, 18), make_date(2025, 12, 17), end]
    );
    assert_eq!(
        semi_annual(StubKind::LongFront),
        vec![start, make_date(2025, 12, 17), end]
    );
}

#[test]
fn imm_schedule_rejects_frequencies_off_the_quarterly_cycle() {
    let builder = || {
        Schedule::builder(make_date(2025, 1, 15), make_date(2025, 12, 31))
            .unwrap()
            .imm()
    };
    for frequency in [Tenor::monthly(), Tenor::weekly(), Tenor::daily()] {
        assert!(builder().frequency(frequency).build().is_err());
    }
    for frequency in [Tenor::semi_annual(), Tenor::annual()] {
        assert!(builder().frequency(frequency).build().is_ok());
    }
}

#[test]
fn imm_schedule_without_stubs_requires_on_cycle_endpoints() {
    let off_cycle = Schedule::builder(make_date(2025, 1, 15), make_date(2025, 12, 17))
        .unwrap()
        .imm()
        .stub_rule(StubKind::None)
        .build();
    assert!(off_cycle.is_err());

    let uneven = Schedule::builder(make_date(2025, 3, 19), make_date(2025, 12, 17))
        .unwrap()
        .imm()
        .frequency(Tenor::semi_annual())
        .stub_rule(StubKind::None)
        .build();
    assert!(uneven.is_err());
}

#[test]
fn test_cds_imm_schedule_basic() {
    // CDS IMM schedule: 20th of Mar/Jun/Sep/Dec.
    //
    // Post-Big-Bang convention :
    // the schedule anchors at the CDS roll PRECEDING the start date, so the
    // first period carries the standard front accrual from 2024-12-20.
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 12, 20);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .cds_imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Should get 5 dates: Dec 20 (prior roll), Mar 20, Jun 20, Sep 20, Dec 20
    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], make_date(2024, 12, 20));
    assert_eq!(dates[1], make_date(2025, 3, 20));
    assert_eq!(dates[2], make_date(2025, 6, 20));
    assert_eq!(dates[3], make_date(2025, 9, 20));
    assert_eq!(dates[4], make_date(2025, 12, 20));
}

#[test]
fn test_cds_imm_schedule_start_on_roll_date_has_no_front_accrual() {
    // When the start date IS a CDS roll date, the schedule anchors on it
    // directly (no prior-roll front accrual period).
    let start = make_date(2025, 3, 20);
    let end = make_date(2025, 12, 20);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .cds_imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], make_date(2025, 3, 20));
    assert_eq!(dates[3], make_date(2025, 12, 20));
}

#[test]
fn test_imm_vs_cds_imm_difference() {
    // Verify that IMM and CDS IMM produce different dates
    // Use end date on a CDS roll date to avoid short back stub
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 6, 20); // Exactly on CDS roll date

    let imm_dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    let cds_dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .cds_imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // IMM keeps the effective date and maturity around its third-Wednesday
    // rolls; CDS anchors at the prior roll for the standard front accrual.
    assert_eq!(
        imm_dates,
        vec![start, make_date(2025, 3, 19), make_date(2025, 6, 18), end]
    );
    assert_eq!(
        cds_dates,
        vec![make_date(2024, 12, 20), make_date(2025, 3, 20), end]
    );
}

#[test]
fn imm_and_cds_imm_setters_use_last_call_wins() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 9, 30);

    let cds_dates = Schedule::builder(start, end)
        .unwrap()
        .imm()
        .cds_imm()
        .build()
        .expect("CDS IMM should win")
        .dates;
    let imm_dates = Schedule::builder(start, end)
        .unwrap()
        .cds_imm()
        .imm()
        .build()
        .expect("IMM should win")
        .dates;

    assert!(cds_dates.contains(&make_date(2025, 3, 20)));
    assert!(!cds_dates.contains(&make_date(2025, 3, 19)));
    assert!(imm_dates.contains(&make_date(2025, 3, 19)));
    assert!(!imm_dates.contains(&make_date(2025, 3, 20)));
}

#[test]
fn test_schedule_error_policy_missing_calendar_warning() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 3, 15);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .error_policy(ScheduleErrorPolicy::MissingCalendarWarning)
        .adjust_with(BusinessDayConvention::Following, "unknown_calendar")
        .build()
        .expect("warning policy should preserve schedule generation");

    assert!(!schedule.dates.is_empty());
    assert!(schedule.has_warnings());
    assert!(!schedule.used_graceful_fallback());
}

#[test]
fn test_no_roll_day_drift_backward_day30_anchor() {
    // Backward semi-annual generation from an Aug-30 maturity must
    // alternate Feb 28 / Aug 30 with the roll day (30th) preserved,
    // instead of collapsing to the 28th after the first short February.
    let start = make_date(2025, 8, 30);
    let end = make_date(2027, 8, 30);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::semi_annual())
        .stub_rule(StubKind::ShortFront)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![
            make_date(2025, 8, 30),
            make_date(2026, 2, 28),
            make_date(2026, 8, 30), // not Aug 28
            make_date(2027, 2, 28),
            make_date(2027, 8, 30), // not Aug 28
        ]
    );
}

#[test]
fn test_stub_none_accepts_month_end_quarterly_schedule() {
    // Aug 31 -> Aug 31 quarterly is an integer number of tenors when
    // anchors are computed as single jumps from the seed (Nov 30, Feb 28,
    // May 31, then exactly Aug 31 at k = 4). The drifted iterative scheme
    // wrongly raised NonIntegerScheduleTenor here (B1).
    let start = make_date(2025, 8, 31);
    let end = make_date(2026, 8, 31);

    let dates: Vec<_> = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::None)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![
            make_date(2025, 8, 31),
            make_date(2025, 11, 30),
            make_date(2026, 2, 28),
            make_date(2026, 5, 31),
            make_date(2026, 8, 31),
        ]
    );
}

#[test]
fn test_zero_count_tenor_rejected_at_generation() {
    // A zero-count tenor makes every roll a no-op; generation must fail
    // loudly instead of looping forever.
    use finstack_quant_core::dates::TenorUnit;

    let err = Tenor::new(0, TenorUnit::Months).expect_err("zero-count tenor rejected");

    assert!(
        err.to_string().contains("positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_zero_count_tenor_rejected_via_serde() {
    // Inbound JSON with "count": 0 must fail at deserialization (M10);
    // valid counts continue to round-trip.
    let err = serde_json::from_str::<Tenor>(r#"{"count":0,"unit":"months"}"#)
        .expect_err("zero-count tenor JSON must be rejected");
    assert!(
        err.to_string().contains("positive"),
        "unexpected error: {err}"
    );

    let tenor: Tenor =
        serde_json::from_str(r#"{"count":3,"unit":"months"}"#).expect("valid tenor deserializes");
    assert_eq!(tenor, Tenor::quarterly());
}

#[test]
fn cds_imm_accrual_20ths_stay_unadjusted() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 12, 20);
    // Thursday 20 Mar 2025 is Japan's Vernal Equinox Day (a JPTO holiday).

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .cds_imm()
        .adjust_with(BusinessDayConvention::Following, "jpto")
        .build()
        .unwrap();

    assert!(
        schedule.dates.contains(&make_date(2025, 3, 20)),
        "CDS 20ths stay on the unadjusted accrual grid: {:?}",
        schedule.dates
    );
    assert!(
        schedule.payment_dates.contains(&make_date(2025, 3, 21)),
        "holiday 20th pays Following: {:?}",
        schedule.payment_dates
    );
}

#[test]
fn payment_lag_shifts_adjusted_period_ends() {
    let start = make_date(2025, 1, 2);
    let end = make_date(2025, 1, 9);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::weekly())
        .adjust_with(BusinessDayConvention::Following, "weekends_only")
        .payment_lag_days(2)
        .build()
        .unwrap();

    assert_eq!(schedule.dates, vec![start, end]);
    assert_eq!(schedule.payment_dates, vec![make_date(2025, 1, 13)]);
}

#[test]
fn fixing_lag_is_t_minus_from_accrual_start() {
    let start = make_date(2025, 1, 2);
    let end = make_date(2025, 1, 9);

    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::weekly())
        .adjust_with(BusinessDayConvention::Following, "weekends_only")
        .fixing_lag_business_days(2)
        .build()
        .unwrap();

    assert_eq!(schedule.fixing_dates, vec![make_date(2024, 12, 31)]);
}

#[test]
fn nearest_payment_tie_rolls_following() {
    // Wednesday 1 Jan 2025 is a holiday; Tue 31 Dec and Thu 2 Jan are equally
    // close, so Nearest must roll Following.
    let start = make_date(2024, 12, 18);
    let end = make_date(2025, 1, 1);
    // TARGET2 closes on New Year's Day; 31 Dec 2024 and 2 Jan 2025 are open.
    let schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::weekly())
        .stub_rule(StubKind::ShortBack)
        .adjust_with(BusinessDayConvention::Nearest, "target2")
        .build()
        .unwrap();

    assert_eq!(schedule.dates.last().copied(), Some(end));
    assert_eq!(
        schedule.payment_dates.last().copied(),
        Some(make_date(2025, 1, 2))
    );
}
