//! Schedule and cashflow generation tests for caps/floors.
//!
//! Validates period generation for multi-period caps and floors.

use finstack_quant_cashflows::builder::date_generation::build_dates;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::Result;
use time::macros::date;

#[test]
fn test_quarterly_schedule_generation() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::quarterly(),
        StubKind::None,
        BusinessDayConvention::ModifiedFollowing,
        false,
        0,
        "weekends_only",
    )?;

    // Should have 4 quarterly periods
    assert_eq!(schedule.periods.len(), 4, "Should have 4 quarterly periods");
    assert_eq!(schedule.periods[0].accrual_start, start);
    assert_eq!(schedule.periods.last().unwrap().accrual_end, end);
    Ok(())
}

#[test]
fn test_semi_annual_schedule() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2026 - 01 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::semi_annual(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )?;

    // 2 years semi-annual = 4 periods
    assert_eq!(
        schedule.periods.len(),
        4,
        "Should have 4 semi-annual periods"
    );
    Ok(())
}

#[test]
fn test_annual_schedule() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::annual(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )?;

    // 5 years annual = 5 periods
    assert_eq!(schedule.periods.len(), 5, "Should have 5 annual periods");
    Ok(())
}

#[test]
fn test_monthly_schedule() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 07 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::monthly(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )?;

    // 6 months = 6 periods
    assert_eq!(schedule.periods.len(), 6, "Should have 6 monthly periods");
    Ok(())
}

#[test]
fn test_schedule_ordering() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::quarterly(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )?;

    // Verify dates are in ascending order
    for i in 1..schedule.dates.len() {
        assert!(
            schedule.dates[i] > schedule.dates[i - 1],
            "Dates should be in ascending order"
        );
    }
    Ok(())
}

#[test]
fn test_period_coverage() -> Result<()> {
    let start = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let schedule = build_dates(
        start,
        end,
        Tenor::quarterly(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )?;

    // First period should start at start
    assert_eq!(
        schedule.periods[0].accrual_start, start,
        "First period should start at schedule start"
    );

    // Last period should end at end
    assert_eq!(
        schedule.periods.last().unwrap().accrual_end,
        end,
        "Last period should end at schedule end"
    );
    Ok(())
}

#[test]
fn test_year_fraction_calculation() {
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01);

    let day_count = DayCount::Act360;
    let yf = day_count
        .year_fraction(
            start,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();

    // 91 days / 360 = 0.2527...
    assert!(
        yf > 0.25 && yf < 0.26,
        "Year fraction should be ~0.25: {}",
        yf
    );
}

#[test]
fn test_different_day_count_conventions() {
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 07 - 01);

    let act360 = DayCount::Act360
        .year_fraction(
            start,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let act365 = DayCount::Act365F
        .year_fraction(
            start,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let thirty_360 = DayCount::Thirty360
        .year_fraction(
            start,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();

    // Different day counts should produce different results
    assert!(act360 != act365, "ACT/360 should differ from ACT/365F");
    assert!(act360 != thirty_360, "ACT/360 should differ from 30/360");
}

#[test]
fn test_leap_year_handling() {
    let start = date!(2024 - 02 - 28);
    let end = date!(2024 - 03 - 01); // 2024 is a leap year

    let day_count = DayCount::Act365F;
    let yf = day_count
        .year_fraction(
            start,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();

    // Should account for Feb 29 - 2 days in a 366-day year
    assert!(yf > 0.0, "Should handle leap year: {}", yf);
    assert!(yf < 0.01, "Two days should be small fraction: {}", yf);
}
