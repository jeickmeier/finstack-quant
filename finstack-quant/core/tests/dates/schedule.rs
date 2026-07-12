//! Tests for schedule iterator functionality.

use super::common::{make_date, TestCal};
use finstack_quant_core::dates::{
    BusinessDayConvention, ScheduleBuilder, ScheduleErrorPolicy, StubKind, Tenor,
};

#[test]
fn test_basic_schedule() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 4, 15);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let result = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let result = ScheduleBuilder::new(date, date)
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
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 1)); // New Year's Day (Wednesday)

    let start = make_date(2025, 1, 1); // Holiday Wednesday
    let end = make_date(2025, 1, 8);

    // Test that the builder can handle adjustment (even if AdjustIter is not public)
    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::weekly())
        .adjust_with(BusinessDayConvention::Following, &cal)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // First date should be adjusted from Jan 1 (holiday) to Jan 2
    assert_eq!(dates.len(), 2);
    assert_eq!(dates[0], make_date(2025, 1, 2)); // Thursday (adjusted from holiday)
    assert_eq!(dates[1], make_date(2025, 1, 8)); // Wednesday
}

#[test]
fn test_schedule_builder_with_adjustment() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 1)); // New Year's Day

    let start = make_date(2025, 1, 1); // Wednesday (holiday)
    let end = make_date(2025, 3, 1);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .adjust_with(BusinessDayConvention::Following, &cal)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // First date should be adjusted from Jan 1 (holiday) to Jan 2
    assert_eq!(dates[0], make_date(2025, 1, 2)); // Thursday
    assert_eq!(dates[1], make_date(2025, 2, 3)); // Saturday -> Monday Feb 3
    assert_eq!(dates[2], make_date(2025, 3, 3)); // Saturday -> Monday Mar 3
}

#[test]
fn test_uneven_period_clamping() {
    // StubKind::None now rejects non-integer tenors; use ShortBack for clamping
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 1, 20); // Not a multiple of monthly frequency

    // Verify StubKind::None rejects it
    let none_result = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .build();
    assert!(
        none_result.is_err(),
        "StubKind::None must reject non-integer tenor"
    );

    // ShortBack clamps to end date
    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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
fn test_end_of_month_convention() {
    // EOM snaps INTERMEDIATE roll dates to month-end; the user-provided
    // start and end dates are contractual and emitted verbatim.
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 4, 15);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
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
    let result = ScheduleBuilder::new(make_date(2025, 1, 15), make_date(2025, 2, 20))
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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
    // Daily schedule ending Fri Mar 29 2024 with Mar 27 + Mar 28 holidays:
    // Following adjusts all three of Mar 27/28/29 to Mar 29. The maturity
    // date must survive the post-adjustment dedup.
    let start = make_date(2024, 3, 26);
    let end = make_date(2024, 3, 29);
    let cal = TestCal::new()
        .with_holiday(make_date(2024, 3, 27))
        .with_holiday(make_date(2024, 3, 28));

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::daily())
        .adjust_with(BusinessDayConvention::Following, &cal)
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.last().copied(), Some(end), "maturity must survive");
    assert!(
        dates.windows(2).all(|w| w[0] < w[1]),
        "dates must be strictly increasing: {dates:?}"
    );
}

// ============================================================================
// IMM Schedule Tests
// ============================================================================

#[test]
fn test_imm_schedule_basic() {
    // Standard IMM schedule: third Wednesday of Mar/Jun/Sep/Dec
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 12, 31);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Effective date anchors the front accrual, then IMM roll dates follow.
    assert_eq!(dates.len(), 5);
    assert_eq!(dates[0], start);
    assert_eq!(dates[1], make_date(2025, 3, 19)); // Third Wednesday of March
    assert_eq!(dates[2], make_date(2025, 6, 18)); // Third Wednesday of June
    assert_eq!(dates[3], make_date(2025, 9, 17)); // Third Wednesday of September
    assert_eq!(dates[4], make_date(2025, 12, 17)); // Third Wednesday of December
}

#[test]
fn test_imm_schedule_start_on_imm_date() {
    // When start is already an IMM date, it should be included
    let start = make_date(2025, 3, 19); // Third Wednesday of March 2025
    let end = make_date(2025, 9, 30);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Should start from March 19
    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], make_date(2025, 3, 19)); // March IMM
    assert_eq!(dates[1], make_date(2025, 6, 18)); // June IMM
    assert_eq!(dates[2], make_date(2025, 9, 17)); // September IMM
}

#[test]
fn test_imm_schedule_start_after_first_imm() {
    // Start after March IMM should anchor the front accrual, then roll to June
    let start = make_date(2025, 3, 20); // Day after March IMM
    let end = make_date(2025, 9, 30);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], start);
    assert_eq!(dates[1], make_date(2025, 6, 18)); // June IMM
    assert_eq!(dates[2], make_date(2025, 9, 17)); // September IMM
}

#[test]
fn test_imm_schedule_empty_range_errors() {
    // No IMM date between start and end: error instead of a silent empty
    // schedule (which would mean zero cashflows / PV = 0 downstream).
    let start = make_date(2025, 3, 20); // day after the Mar 19 IMM date
    let end = make_date(2025, 4, 30); // before the Jun 18 IMM date

    let result = ScheduleBuilder::new(start, end).unwrap().imm().build();
    assert!(result.is_err(), "empty IMM range must error in strict mode");

    // Graceful policy converts the error to an empty schedule WITH warning.
    let sched = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .error_policy(ScheduleErrorPolicy::GracefulEmpty)
        .build()
        .unwrap();
    assert!(sched.dates.is_empty());
    assert!(sched.used_graceful_fallback());
}

#[test]
fn test_imm_schedule_preserves_effective_date_front_anchor() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 6, 20);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(
        dates,
        vec![start, make_date(2025, 3, 19), make_date(2025, 6, 18)]
    );
}

#[test]
fn test_imm_schedule_year_rollover() {
    // IMM schedule spanning year boundary
    let start = make_date(2025, 10, 1);
    let end = make_date(2026, 6, 30);

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // Effective date plus Dec 2025, Mar 2026, Jun 2026
    assert_eq!(dates.len(), 4);
    assert_eq!(dates[0], start);
    assert_eq!(dates[1], make_date(2025, 12, 17)); // December 2025 third Wednesday
    assert_eq!(dates[2], make_date(2026, 3, 18)); // March 2026 third Wednesday
    assert_eq!(dates[3], make_date(2026, 6, 17)); // June 2026 third Wednesday
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let imm_dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    let cds_dates: Vec<_> = ScheduleBuilder::new(start, end)
        .unwrap()
        .cds_imm()
        .build()
        .unwrap()
        .into_iter()
        .collect();

    // IMM anchors at the effective date; CDS anchors at the prior roll for the
    // standard front accrual.
    assert_eq!(imm_dates.len(), 3);
    assert_eq!(cds_dates.len(), 3);

    // IMM: effective date, then third Wednesday (Mar 19, Jun 18)
    // CDS: 20th, anchored at prior roll (Dec 20 2024, Mar 20, Jun 20)
    assert_eq!(imm_dates[0], start);
    assert_eq!(cds_dates[0], make_date(2024, 12, 20));
    assert_eq!(imm_dates[1], make_date(2025, 3, 19));
    assert_eq!(cds_dates[1], make_date(2025, 3, 20));
    assert_eq!(imm_dates[2], make_date(2025, 6, 18));
    assert_eq!(cds_dates[2], make_date(2025, 6, 20));
}

#[test]
fn imm_and_cds_imm_setters_use_last_call_wins() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 9, 30);

    let cds_dates = ScheduleBuilder::new(start, end)
        .unwrap()
        .imm()
        .cds_imm()
        .build()
        .expect("CDS IMM should win")
        .dates;
    let imm_dates = ScheduleBuilder::new(start, end)
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

    let schedule = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::monthly())
        .error_policy(ScheduleErrorPolicy::MissingCalendarWarning)
        .adjust_with_id(BusinessDayConvention::Following, "unknown_calendar")
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let dates: Vec<_> = ScheduleBuilder::new(start, end)
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

    let start = make_date(2025, 1, 1);
    let end = make_date(2026, 1, 1);

    let err = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::new(0, TenorUnit::Months))
        .build()
        .expect_err("zero-count tenor must be rejected");

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
