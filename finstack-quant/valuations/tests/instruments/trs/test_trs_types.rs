//! Unit tests for TRS core types.
//!
//! Tests for TrsSide, TrsScheduleSpec, and related type functionality.

use super::test_utils::*;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_valuations::instruments::{TrsScheduleSpec, TrsSide};

// ================================================================================================
// TrsSide Tests
// ================================================================================================

#[test]
fn test_trs_side_display() {
    // Arrange
    let receive = TrsSide::ReceiveTotalReturn;
    let pay = TrsSide::PayTotalReturn;

    // Act & Assert
    assert_eq!(receive.to_string(), "receive_total_return");
    assert_eq!(pay.to_string(), "pay_total_return");
}

#[test]
fn test_trs_side_from_str_canonical() {
    // Arrange & Act
    let receive = "receive_total_return".parse::<TrsSide>().unwrap();
    let pay = "pay_total_return".parse::<TrsSide>().unwrap();

    // Assert
    assert_eq!(receive, TrsSide::ReceiveTotalReturn);
    assert_eq!(pay, TrsSide::PayTotalReturn);
}

#[test]
fn test_trs_side_from_str_short_forms() {
    // Arrange & Act
    let receive = "receive".parse::<TrsSide>().unwrap();
    let pay = "pay".parse::<TrsSide>().unwrap();

    // Assert
    assert_eq!(receive, TrsSide::ReceiveTotalReturn);
    assert_eq!(pay, TrsSide::PayTotalReturn);
}

#[test]
fn test_trs_side_from_str_case_insensitive() {
    // Arrange & Act
    let receive_upper = "RECEIVE_TOTAL_RETURN".parse::<TrsSide>().unwrap();
    let pay_mixed = "Pay_Total_Return".parse::<TrsSide>().unwrap();

    // Assert
    assert_eq!(receive_upper, TrsSide::ReceiveTotalReturn);
    assert_eq!(pay_mixed, TrsSide::PayTotalReturn);
}

#[test]
fn test_trs_side_from_str_with_hyphens() {
    // Arrange & Act
    let receive = "receive-total-return".parse::<TrsSide>().unwrap();
    let pay = "pay-total-return".parse::<TrsSide>().unwrap();

    // Assert
    assert_eq!(receive, TrsSide::ReceiveTotalReturn);
    assert_eq!(pay, TrsSide::PayTotalReturn);
}

#[test]
fn test_trs_side_from_str_invalid() {
    // Arrange & Act
    let result = "invalid_side".parse::<TrsSide>();

    // Assert
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown TRS side"));
}

#[test]
fn test_trs_side_sign() {
    // Arrange
    let receive = TrsSide::ReceiveTotalReturn;
    let pay = TrsSide::PayTotalReturn;

    // Act & Assert
    assert_eq!(receive.sign(), 1.0);
    assert_eq!(pay.sign(), -1.0);
}

#[test]
fn test_trs_side_equality() {
    // Arrange
    let receive1 = TrsSide::ReceiveTotalReturn;
    let receive2 = TrsSide::ReceiveTotalReturn;
    let pay = TrsSide::PayTotalReturn;

    // Act & Assert
    assert_eq!(receive1, receive2);
    assert_ne!(receive1, pay);
}

#[test]
fn test_trs_side_clone_and_copy() {
    // Arrange
    let side = TrsSide::ReceiveTotalReturn;

    // Act
    let copied = side;

    // Assert
    assert_eq!(side, copied);
}

// ================================================================================================
// TrsScheduleSpec Tests
// ================================================================================================

#[test]
fn test_trs_schedule_spec_creation() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2026, 1, 2);
    let params = ScheduleParams::quarterly_act360();

    // Act
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Assert
    assert_eq!(spec.start, start);
    assert_eq!(spec.end, end);
    assert_eq!(spec.params.dc, DayCount::Act360);
    assert_eq!(spec.params.freq, Tenor::quarterly());
}

#[test]
fn test_trs_schedule_spec_period_schedule_quarterly() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2026, 1, 2);
    let params = ScheduleParams::quarterly_act360();
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let schedule = spec.period_schedule().expect("Schedule should build");

    // Assert
    // 1 year quarterly = 4 periods, so 5 dates (start + 4 ends)
    assert_eq!(
        schedule.dates.len(),
        5,
        "Should have 5 dates for 4 quarterly periods"
    );
    assert_eq!(schedule.dates[0], start);
    assert_eq!(schedule.dates[4], end);
}

#[test]
fn test_trs_schedule_spec_period_schedule_semiannual() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2026, 1, 2);
    let params = ScheduleParams {
        freq: Tenor::semi_annual(),
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::None,
        calendar_id: "weekends_only".to_string(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let schedule = spec.period_schedule().expect("Schedule should build");

    // Assert
    // 1 year semiannual = 2 periods, so 3 dates
    assert_eq!(
        schedule.dates.len(),
        3,
        "Should have 3 dates for 2 semiannual periods"
    );
    assert_eq!(schedule.dates[0], start);
    assert_eq!(schedule.dates[2], end);
}

#[test]
fn test_trs_schedule_spec_period_schedule_monthly() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2025, 7, 2); // 6 months
    let params = ScheduleParams {
        freq: Tenor::monthly(),
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::Following,
        stub: StubKind::None,
        calendar_id: "weekends_only".to_string(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let schedule = spec.period_schedule().expect("Schedule should build");

    // Assert
    // 6 months monthly = 6 periods, so 7 dates
    assert_eq!(
        schedule.dates.len(),
        7,
        "Should have 7 dates for 6 monthly periods"
    );
    assert_eq!(schedule.dates[0], start);
    assert_eq!(schedule.dates[6], end);
}

#[test]
fn test_trs_schedule_spec_different_day_counts() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2026, 1, 2);

    let params_act360 = ScheduleParams::quarterly_act360();
    let params_30_360 = ScheduleParams {
        freq: Tenor::quarterly(),
        dc: DayCount::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::None,
        calendar_id: "weekends_only".to_string(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };

    // Act
    let spec_act360 = TrsScheduleSpec::from_params(start, end, params_act360);
    let spec_30_360 = TrsScheduleSpec::from_params(start, end, params_30_360);

    // Assert
    assert_eq!(spec_act360.params.dc, DayCount::Act360);
    assert_eq!(spec_30_360.params.dc, DayCount::Thirty360);

    // Both should produce same number of dates (different year fractions though)
    let sched1 = spec_act360
        .period_schedule()
        .expect("Schedule should build");
    let sched2 = spec_30_360
        .period_schedule()
        .expect("Schedule should build");
    assert_eq!(sched1.dates.len(), sched2.dates.len());
}

#[test]
fn test_trs_schedule_spec_short_tenor() {
    // Arrange - 3 month tenor
    let start = d(2025, 1, 2);
    let end = d(2025, 4, 2);
    let params = ScheduleParams::quarterly_act360();
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let schedule = spec.period_schedule().expect("Schedule should build");

    // Assert
    // 3 months with quarterly frequency = 1 period, so 2 dates
    assert_eq!(
        schedule.dates.len(),
        2,
        "Should have 2 dates for 1 quarterly period"
    );
    assert_eq!(schedule.dates[0], start);
    assert_eq!(schedule.dates[1], end);
}

#[test]
fn test_trs_schedule_spec_long_tenor() {
    // Arrange - 5 year tenor
    let start = d(2025, 1, 2);
    let end = d(2030, 1, 2);
    let params = ScheduleParams::quarterly_act360();
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let schedule = spec.period_schedule().expect("Schedule should build");

    // Assert
    // 5 years quarterly = 20 periods, so 21 dates
    assert_eq!(
        schedule.dates.len(),
        21,
        "Should have 21 dates for 20 quarterly periods"
    );
    assert_eq!(schedule.dates[0], start);
    assert_eq!(schedule.dates[20], end);
}

#[test]
fn test_trs_schedule_spec_clone() {
    // Arrange
    let start = d(2025, 1, 2);
    let end = d(2026, 1, 2);
    let params = ScheduleParams::quarterly_act360();
    let spec = TrsScheduleSpec::from_params(start, end, params);

    // Act
    let cloned = spec.clone();

    // Assert
    assert_eq!(spec.start, cloned.start);
    assert_eq!(spec.end, cloned.end);
    assert_eq!(spec.params.freq, cloned.params.freq);
    assert_eq!(spec.params.dc, cloned.params.dc);
}
