//! Day count convention tests
//!
//! Tests for all ISDA-standard day count conventions:
//!
//! ## 30/360 Family
//! - 30/360 US (Bond Basis) with end-of-month rules
//! - 30E/360 (Eurobond Basis)
//!
//! ## Actual-Based Conventions
//! - Act/Act (ISDA)
//! - Act/Act (ISMA) - frequency-dependent
//! - Act/365L (AFB)
//! - Act/360
//! - Act/365
//!
//! ## Business Day Count
//! - Bus/252 - calendar-dependent

use super::common::DAYCOUNT_TOLERANCE;
use finstack_quant_core::dates::calendar::TARGET2;
use finstack_quant_core::dates::{
    act_act_isma_year_fraction_with_reference_period, Date, DayCount, DayCountContext, Duration,
    Tenor, TenorUnit,
};
use proptest::prelude::*;
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

// Short alias for tests
fn d(year: i32, month: u8, day: u8) -> Date {
    make_date(year, month, day)
}

const TOL: f64 = DAYCOUNT_TOLERANCE;

#[test]
fn year_fraction_rejects_inverted_dates_but_signed_accepts_them() {
    let start = d(2025, 7, 1);
    let end = d(2025, 1, 1);
    let ctx = DayCountContext::default();

    assert!(DayCount::Act365F.year_fraction(start, end, ctx).is_err());
    let signed = DayCount::Act365F
        .signed_year_fraction(start, end, ctx)
        .expect("signed year fraction should allow inverted dates");

    assert!(signed < 0.0);
    assert_eq!(
        DayCount::Act365F
            .signed_year_fraction(start, start, ctx)
            .expect("same date"),
        0.0
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn actual_daycounts_are_non_negative_and_additive(
        start_offset in 0_i64..10_000,
        first_span in 0_i64..2_000,
        second_span in 0_i64..2_000,
    ) {
        let start = d(1990, 1, 1) + Duration::days(start_offset);
        let middle = start + Duration::days(first_span);
        let end = middle + Duration::days(second_span);
        let ctx = DayCountContext::default();

        for day_count in [DayCount::Act360, DayCount::Act365F] {
            let first = day_count.year_fraction(start, middle, ctx).unwrap();
            let second = day_count.year_fraction(middle, end, ctx).unwrap();
            let total = day_count.year_fraction(start, end, ctx).unwrap();

            prop_assert!(first >= 0.0);
            prop_assert!(second >= 0.0);
            prop_assert!((first + second - total).abs() < TOL);
        }
    }
}

// =============================================================================
// 30/360 US (Bond Basis) - ISDA 2006 Section 4.16(f)
// =============================================================================

#[test]
fn thirty360_us_feb28_to_mar31_non_leap() {
    // ISDA: Feb 28 (last day of Feb in non-leap year) -> treat as day 30
    // Mar 31 -> treat as day 30 (since D1 was adjusted to 30)
    // Result: (30 - 30) + 30 = 30 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2025, 2, 28), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 30.0 / 360.0).abs() < TOL,
        "Expected 30/360, got {}",
        yf
    );
}

#[test]
fn thirty360_us_feb29_to_mar31_leap() {
    // Leap year: Feb 29 is last day of Feb -> treat as day 30
    // Mar 31 -> treat as day 30 (since D1 was adjusted to 30)
    // Result: (30 - 30) + 30 = 30 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2024, 2, 29), d(2024, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 30.0 / 360.0).abs() < TOL,
        "Expected 30/360, got {}",
        yf
    );
}

#[test]
fn thirty360_us_feb27_to_mar31() {
    // Feb 27 is NOT last day of Feb -> D1 stays 27
    // Mar 31 -> stays 31 (D1 was not 30)
    // Result: (31 - 27) + 30 = 34 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2025, 2, 27), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 34.0 / 360.0).abs() < TOL,
        "Expected 34/360, got {}",
        yf
    );
}

#[test]
fn thirty360_us_feb28_to_feb28_next_year() {
    // Both are last day of Feb (non-leap 2024 to non-leap 2025)
    // D1 = 30, D2 = 30 (both adjusted)
    // Result: 360 days = 1.0
    let yf = DayCount::Thirty360
        .year_fraction(d(2024, 2, 28), d(2025, 2, 28), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected 1.0, got {}", yf);
}

#[test]
fn thirty360_us_feb29_to_feb28_next_year() {
    // Feb 29 (leap) to Feb 28 (non-leap next year)
    // Both are last day of Feb -> both adjusted to 30
    // Result: 360 days = 1.0
    let yf = DayCount::Thirty360
        .year_fraction(d(2024, 2, 29), d(2025, 2, 28), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected 1.0, got {}", yf);
}

#[test]
fn thirty360_us_jan31_to_feb28() {
    // Jan 31 -> D1 = 30 (31 rule)
    // Feb 28 (last day) -> but D1 was not Feb EOM, so we check D2 EOM rule
    // Result depends on whether D1 was Feb EOM (it wasn't)
    // So D2 stays 28
    // Days: (28 - 30) + 30 = 28 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2025, 1, 31), d(2025, 2, 28), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 28.0 / 360.0).abs() < TOL,
        "Expected 28/360, got {}",
        yf
    );
}

#[test]
fn thirty360_us_feb28_to_mar30() {
    // Feb 28 (EOM) -> D1 = 30
    // Mar 30 -> D2 stays 30 (not 31)
    // Days: (30 - 30) + 30 = 30 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2025, 2, 28), d(2025, 3, 30), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 30.0 / 360.0).abs() < TOL,
        "Expected 30/360, got {}",
        yf
    );
}

#[test]
fn thirty360_us_feb28_to_apr30() {
    // Feb 28 (EOM) -> D1 = 30
    // Apr 30 -> D2 stays 30
    // Days: (30 - 30) + 60 = 60 days
    let yf = DayCount::Thirty360
        .year_fraction(d(2025, 2, 28), d(2025, 4, 30), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 60.0 / 360.0).abs() < TOL,
        "Expected 60/360, got {}",
        yf
    );
}

// =============================================================================
// 30E/360 (Eurobond Basis) - ISDA 2006 Section 4.16(g)
// =============================================================================

#[test]
fn thirty_e360_d2_always_30_when_31() {
    // 30E/360: D2=31 -> 30 unconditionally (unlike US)
    // Jan 30 to Mar 31: D1=30, D2=30 (adjusted from 31)
    // Days: (30-30) + 60 = 60 days
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 30), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 60.0 / 360.0).abs() < TOL,
        "Expected 60/360, got {}",
        yf
    );
}

#[test]
fn thirty_e360_vs_us_difference() {
    // Key test showing the difference between US and European conventions
    // Jan 15 to Mar 31: D1=15, D2=31
    // US: D1_adj = 15 (not 31), D2=31 stays (D1_adj != 30) -> Days = (31-15) + 60 = 76
    // Euro: D1_adj = 15, D2=30 (always adjust 31) -> Days = (30-15) + 60 = 75
    let us = DayCount::Thirty360
        .year_fraction(d(2025, 1, 15), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    let euro = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 15), d(2025, 3, 31), DayCountContext::default())
        .unwrap();

    assert!(
        (us - 76.0 / 360.0).abs() < TOL,
        "US expected 76/360, got {}",
        us
    );
    assert!(
        (euro - 75.0 / 360.0).abs() < TOL,
        "Euro expected 75/360, got {}",
        euro
    );
    assert!(
        (us - euro - 1.0 / 360.0).abs() < TOL,
        "Difference should be 1 day"
    );
}

#[test]
fn thirty_e360_full_year() {
    // Full year should be 360/360 = 1.0
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 1), d(2026, 1, 1), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected 1.0, got {}", yf);
}

#[test]
fn thirty_e360_jan31_to_mar31() {
    // D1=31 -> 30, D2=31 -> 30 (Euro always adjusts)
    // Days: (30-30) + 60 = 60 days
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 31), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 60.0 / 360.0).abs() < TOL,
        "Expected 60/360, got {}",
        yf
    );
}

#[test]
fn thirty_e360_jan31_to_mar31_same_as_us() {
    // When D1=31 (adjusted to 30), both conventions give same result for D2=31
    // US: D1_adj=30, D2=31 -> D2=30 (because D1_adj==30)
    // Euro: D1_adj=30, D2=31 -> D2=30 (always)
    let us = DayCount::Thirty360
        .year_fraction(d(2025, 1, 31), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    let euro = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 31), d(2025, 3, 31), DayCountContext::default())
        .unwrap();

    assert!(
        (us - euro).abs() < TOL,
        "US and Euro should be equal when D1=31: US={}, Euro={}",
        us,
        euro
    );
}

#[test]
fn thirty_e360_feb28_not_adjusted() {
    // 30E/360 does NOT have Feb EOM rule like US does
    // Feb 28 stays as day 28
    // Feb 28 to Mar 31: D1=28, D2=30 (31 adjusted)
    // Days: (30-28) + 30 = 32 days
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2025, 2, 28), d(2025, 3, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 32.0 / 360.0).abs() < TOL,
        "Expected 32/360, got {}",
        yf
    );
}

#[test]
fn thirty_e360_short_period() {
    // Jan 15 to Jan 25 = 10 days
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2025, 1, 15), d(2025, 1, 25), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 10.0 / 360.0).abs() < TOL,
        "Expected 10/360, got {}",
        yf
    );
}

#[test]
fn thirty_e360_dec31_to_jan31() {
    // Dec 31 to Jan 31 (next year)
    // D1=31 -> 30, D2=31 -> 30
    // Days: (30-30) + 30 = 30 days
    let yf = DayCount::ThirtyE360
        .year_fraction(d(2024, 12, 31), d(2025, 1, 31), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 30.0 / 360.0).abs() < TOL,
        "Expected 30/360, got {}",
        yf
    );
}

// =============================================================================
// Act/365L (ICMA Rule 251)
// =============================================================================

#[test]
fn act365l_period_contains_feb29_uses_366() {
    // Jan 1 to Mar 1 in 2024 (leap year) = 60 actual days, contains Feb 29
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 1, 1), d(2024, 3, 1), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 60.0 / 366.0).abs() < TOL,
        "Expected 60/366, got {}",
        yf
    );
}

#[test]
fn act365l_period_no_feb29_uses_365() {
    // Jan 1 to Mar 1 in 2025 (non-leap) = 59 actual days, no Feb 29
    let yf = DayCount::Act365L
        .year_fraction(d(2025, 1, 1), d(2025, 3, 1), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 59.0 / 365.0).abs() < TOL,
        "Expected 59/365, got {}",
        yf
    );
}

#[test]
fn act365l_period_ending_on_feb29() {
    // ICMA Rule 251 window is (start, end]: a period ENDING on Feb 29
    // includes it -> 366 denom. (Updated from the incorrect [start, end)
    // window per the )
    // 59 actual days, Feb 29 in (start, end] -> 366 denom
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 1, 1), d(2024, 2, 29), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 59.0 / 366.0).abs() < TOL,
        "Expected 59/366, got {}",
        yf
    );
}

#[test]
fn act365l_period_before_feb29_in_leap_year() {
    // Jan 1 to Feb 28 in 2024 - does NOT contain Feb 29 (end is exclusive of Feb 29)
    // 58 actual days, Feb 29 NOT in [Jan 1, Feb 28] -> 365 denom
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 1, 1), d(2024, 2, 28), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 58.0 / 365.0).abs() < TOL,
        "Expected 58/365, got {}",
        yf
    );
}

#[test]
fn act365l_full_year_leap() {
    // Full leap year: 366 days / 366 = 1.0
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 1, 1), d(2025, 1, 1), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected 1.0, got {}", yf);
}

#[test]
fn act365l_full_year_non_leap() {
    // Full non-leap year: 365 days / 365 = 1.0
    let yf = DayCount::Act365L
        .year_fraction(d(2025, 1, 1), d(2026, 1, 1), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected 1.0, got {}", yf);
}

#[test]
fn act365l_spanning_leap_year_boundary() {
    // Dec 1, 2023 to Mar 1, 2024 - spans into leap year, contains Feb 29
    // 91 actual days, contains Feb 29 -> 366 denom
    let yf = DayCount::Act365L
        .year_fraction(d(2023, 12, 1), d(2024, 3, 1), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 91.0 / 366.0).abs() < TOL,
        "Expected 91/366, got {}",
        yf
    );
}

#[test]
fn act365l_spanning_non_leap_year_boundary() {
    // Dec 1, 2024 to Mar 1, 2025 - spans year boundary, 2025 not leap
    // 90 actual days, no Feb 29 in range -> 365 denom
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 12, 1), d(2025, 3, 1), DayCountContext::default())
        .unwrap();
    assert!(
        (yf - 90.0 / 365.0).abs() < TOL,
        "Expected 90/365, got {}",
        yf
    );
}

#[test]
fn act365l_single_day_feb29() {
    // (Feb 28, Feb 29] = 1 day; Feb 29 is the period END and so is included
    // by the ICMA Rule 251 window -> 366 denom.
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 2, 28), d(2024, 2, 29), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0 / 366.0).abs() < TOL, "Expected 1/366, got {}", yf);
}

#[test]
fn act365l_single_day_not_feb29() {
    // Mar 1 to Mar 2 = 1 day, no Feb 29
    let yf = DayCount::Act365L
        .year_fraction(d(2024, 3, 1), d(2024, 3, 2), DayCountContext::default())
        .unwrap();
    assert!((yf - 1.0 / 365.0).abs() < TOL, "Expected 1/365, got {}", yf);
}

// =============================================================================
// Act/Act ISMA - Frequency-Dependent
// =============================================================================

#[test]
fn actact_isma_requires_frequency() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 7, 1);

    // Without frequency, should error
    let result = DayCount::ActActIsma.year_fraction(start, end, DayCountContext::default());
    assert!(result.is_err());

    // With frequency, should work
    let ctx = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(6, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf = DayCount::ActActIsma.year_fraction(start, end, ctx).unwrap();
    assert!(yf > 0.0);
}

#[test]
fn actact_isma_full_coupon_period() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 7, 1);

    let ctx = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(6, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::ActActIsma.year_fraction(start, end, ctx).unwrap();

    // Full semi-annual period = 0.5 year fraction (6 months / 12 months)
    assert!((yf - 0.5).abs() < TOL, "Expected 0.5, got {}", yf);
}

#[test]
fn actact_isma_multiple_frequencies() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 4, 1);

    // Quarterly
    let ctx_q = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(3, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf_q = DayCount::ActActIsma
        .year_fraction(start, end, ctx_q)
        .unwrap();

    // Monthly
    let ctx_m = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(1, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf_m = DayCount::ActActIsma
        .year_fraction(start, end, ctx_m)
        .unwrap();

    // Different frequencies should give the SAME year fraction for the same period
    // Quarterly: 1 full period × 0.25 (quarterly) = 0.25 year fraction
    assert!(
        (yf_q - 0.25).abs() < TOL,
        "Quarterly expected 0.25, got {}",
        yf_q
    );
    // Monthly: 3 full periods × (1/12) = 0.25 year fraction
    assert!(
        (yf_m - 0.25).abs() < TOL,
        "Monthly expected 0.25, got {}",
        yf_m
    );
}

#[test]
fn actact_isma_eom_regular_period_is_exactly_half() {
    // Regression for the the frequency-only
    // quasi-coupon grid was anchored at `start.add_months(-freq)` and stepped
    // cumulatively, so month-end starts drifted (grid Aug 28 instead of
    // Aug 31) and a regular EOM semi-annual period returned
    // 181/184 × 0.5 ≈ 0.49185. Anchoring k-multiples on `start` itself must
    // give exactly 0.5.
    let ctx = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(6, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    // [2025-08-31, 2026-02-28): Aug 31 + 6M (clamped) = Feb 28 → one full period.
    let yf = DayCount::ActActIsma
        .year_fraction(make_date(2025, 8, 31), make_date(2026, 2, 28), ctx)
        .unwrap();
    assert!((yf - 0.5).abs() < TOL, "Expected exactly 0.5, got {}", yf);

    // [2025-03-31, 2025-09-30): Mar 31 + 6M (clamped) = Sep 30 → one full period.
    let yf = DayCount::ActActIsma
        .year_fraction(make_date(2025, 3, 31), make_date(2025, 9, 30), ctx)
        .unwrap();
    assert!((yf - 0.5).abs() < TOL, "Expected exactly 0.5, got {}", yf);
}

#[test]
fn actact_isma_eom_grid_preserves_roll_day_across_short_month() {
    // Multi-period span across a short month: grid anchored on Aug 31 must be
    // Aug 31 / Feb 28 / Aug 31 (roll-day preserved), so two regular
    // semi-annual EOM periods sum to exactly 1.0 .
    let ctx = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(6, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf = DayCount::ActActIsma
        .year_fraction(make_date(2025, 8, 31), make_date(2026, 8, 31), ctx)
        .unwrap();
    assert!((yf - 1.0).abs() < TOL, "Expected exactly 1.0, got {}", yf);
}

#[test]
fn actact_isma_partial_period() {
    let start = make_date(2025, 1, 15);
    let end = make_date(2025, 4, 15);

    let ctx = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(6, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::ActActIsma.year_fraction(start, end, ctx).unwrap();

    // ISMA uses actual days in the quasi-coupon period
    // Jan 15 to Apr 15 = 90 actual days (31 + 28 + 31 for remaining Jan, Feb, Mar)
    // The quasi-coupon period (Jan 15 to Jul 15) = 181 days in 2025
    // Year fraction = (90 / 181) × 0.5 (semi-annual) = 0.24861878...
    let actual_days = 90.0;
    let quasi_coupon_days = 181.0;
    let expected = (actual_days / quasi_coupon_days) * 0.5;

    assert!(
        (yf - expected).abs() < TOL,
        "Expected {:.10}, got {:.10}",
        expected,
        yf
    );
}

#[test]
fn actact_vs_actact_isma_comparison() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2026, 1, 1);

    let yf_isda = DayCount::ActAct
        .year_fraction(start, end, DayCountContext::default())
        .unwrap();

    let ctx_isma = DayCountContext {
        calendar: None,
        frequency: Some(Tenor::new(12, TenorUnit::Months)),
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf_isma = DayCount::ActActIsma
        .year_fraction(start, end, ctx_isma)
        .unwrap();

    // For a full year period with annual frequency, both should give 1.0
    assert!(
        (yf_isda - 1.0).abs() < TOL,
        "ISDA expected 1.0, got {}",
        yf_isda
    );
    assert!(
        (yf_isma - 1.0).abs() < TOL,
        "ISMA expected 1.0, got {}",
        yf_isma
    );
}

#[test]
fn actact_isma_reference_period_handles_short_front_stub_with_month_end_anchor() {
    let start = make_date(2025, 3, 15);
    let end = make_date(2025, 7, 31);
    let reference_start = make_date(2025, 1, 31);
    let reference_end = make_date(2025, 7, 31);

    let yf = act_act_isma_year_fraction_with_reference_period(
        start,
        end,
        reference_start,
        reference_end,
    )
    .unwrap();

    let expected = ((end - start).whole_days() as f64
        / (reference_end - reference_start).whole_days() as f64)
        * 0.5;
    assert!(
        (yf - expected).abs() < TOL,
        "Expected {:.12}, got {:.12}",
        expected,
        yf
    );
}

#[test]
fn actact_isma_reference_period_handles_long_first_stub() {
    let start = make_date(2025, 1, 31);
    let end = make_date(2025, 10, 15);
    let reference_start = make_date(2025, 7, 31);
    let reference_end = make_date(2026, 1, 31);

    let yf = act_act_isma_year_fraction_with_reference_period(
        start,
        end,
        reference_start,
        reference_end,
    )
    .unwrap();

    let first_regular = 0.5;
    let stub_fraction = ((end - reference_start).whole_days() as f64
        / (reference_end - reference_start).whole_days() as f64)
        * 0.5;
    let expected = first_regular + stub_fraction;
    assert!(
        (yf - expected).abs() < TOL,
        "Expected {:.12}, got {:.12}",
        expected,
        yf
    );
}

#[test]
fn actact_isma_reference_period_rejects_invalid_ranges() {
    assert!(act_act_isma_year_fraction_with_reference_period(
        make_date(2025, 3, 31),
        make_date(2025, 1, 15),
        make_date(2024, 9, 30),
        make_date(2025, 3, 31),
    )
    .is_err());
    assert!(act_act_isma_year_fraction_with_reference_period(
        make_date(2025, 1, 15),
        make_date(2025, 3, 31),
        make_date(2025, 3, 31),
        make_date(2025, 3, 31),
    )
    .is_err());
}

// =============================================================================
// Bus/252 - Calendar-Dependent
// =============================================================================

#[test]
fn bus252_requires_calendar() {
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 1, 10);

    // Without calendar, should error
    let result = DayCount::Bus252.year_fraction(start, end, DayCountContext::default());
    assert!(result.is_err());

    // With calendar, should work
    let calendar = TARGET2;
    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };
    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();
    assert!(yf > 0.0);
}

#[test]
fn bus252_rejects_zero_basis() {
    let calendar = TARGET2;
    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: Some(0),
        coupon_period: None,
        end_is_termination_date: false,
    };

    assert!(DayCount::Bus252
        .year_fraction(make_date(2025, 1, 1), make_date(2025, 1, 10), ctx)
        .is_err());
}

#[test]
fn bus252_counts_only_business_days() {
    let calendar = TARGET2;
    let start = make_date(2025, 1, 2); // Thursday
    let end = make_date(2025, 1, 6); // Monday (includes weekend)

    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();

    // Should count exactly: Thu, Fri = 2 business days (skip Sat, Sun)
    // Verify the implied business-day count without rounding.
    let biz_days = yf * 252.0;
    assert!(
        (biz_days - 2.0).abs() < 1e-12,
        "Bus/252 should count exactly 2 days: yf={}, yf*252={}",
        yf,
        biz_days
    );
}

#[test]
fn bus252_full_year_is_deterministic() {
    use finstack_quant_core::dates::HolidayCalendar;

    let calendar = TARGET2;
    let start = make_date(2025, 1, 2);
    let end = make_date(2026, 1, 2);

    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();

    // Compute expected business day count by iterating through dates
    // The calendar is deterministic, so this should yield an exact count
    let mut expected_biz_days: i32 = 0;
    let mut current = start;
    while current < end {
        if calendar.is_business_day(current) {
            expected_biz_days += 1;
        }
        current += time::Duration::days(1);
    }

    // Verify the year fraction matches the expected business day count
    let computed_biz_days = yf * 252.0;
    assert!(
        (computed_biz_days - expected_biz_days as f64).abs() < 1e-10,
        "Bus/252 mismatch for TARGET2 2025-01-02 to 2026-01-02: expected_days={}, got_yf={}, got_yf*252={}",
        expected_biz_days,
        yf,
        computed_biz_days
    );

    // Sanity check: should be close to 252 (typical trading year)
    assert!(
        (expected_biz_days - 252).abs() <= 5,
        "Business day count {} is unexpectedly far from 252",
        expected_biz_days
    );
}

#[test]
fn bus252_excludes_holidays() {
    let calendar = TARGET2;

    // Period including Christmas
    let start = make_date(2024, 12, 23); // Monday
    let end = make_date(2024, 12, 30); // Monday next week

    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();
    let biz_days = yf * 252.0;

    // Dec 25, 26 are holidays, plus weekend = only a few business days
    assert!(biz_days < 5.0);
}
