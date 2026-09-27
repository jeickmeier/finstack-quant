//! Unit tests for TRS core types.
//!
//! Tests for the TRS side, TrsScheduleSpec, and related type functionality.

use super::test_utils::*;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_valuations::instruments::{PayReceive, TrsScheduleSpec};

// Trade side

#[test]
fn test_trs_side_rejects_retired_total_return_spellings() {
    use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
    use finstack_quant_valuations::instruments::fixed_income::fi_trs::FiIndexTotalReturnSwap;

    let equity = serde_json::to_value(EquityTotalReturnSwap::example().unwrap()).unwrap();
    let fi = serde_json::to_value(FiIndexTotalReturnSwap::example().unwrap()).unwrap();
    for retired in [
        "receive_total_return", // schema-rejection-test: now `receive`
        "pay_total_return",     // schema-rejection-test: now `pay`
    ] {
        let mut value = equity.clone();
        value["side"] = serde_json::json!(retired);
        assert!(serde_json::from_value::<EquityTotalReturnSwap>(value).is_err());
        let mut value = fi.clone();
        value["side"] = serde_json::json!(retired);
        assert!(serde_json::from_value::<FiIndexTotalReturnSwap>(value).is_err());
    }
}

#[test]
fn test_trs_side_receive_is_long_total_return() {
    // Receive = receive the total-return leg, the same +1 sign TrsSide carried.
    assert_eq!(PayReceive::Receive.sign(), 1.0);
    assert_eq!(PayReceive::Pay.sign(), -1.0);
}

// TrsScheduleSpec Tests

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
    assert_eq!(spec.params.day_count, DayCount::Act360);
    assert_eq!(spec.params.frequency, Tenor::quarterly());
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
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::None,
        calendar_id: "weekends_only".into(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
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
        frequency: Tenor::monthly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::Following,
        stub: StubKind::None,
        calendar_id: "weekends_only".into(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
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
        frequency: Tenor::quarterly(),
        day_count: DayCount::Thirty360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::None,
        calendar_id: "weekends_only".into(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    };

    // Act
    let spec_act360 = TrsScheduleSpec::from_params(start, end, params_act360);
    let spec_30_360 = TrsScheduleSpec::from_params(start, end, params_30_360);

    // Assert
    assert_eq!(spec_act360.params.day_count, DayCount::Act360);
    assert_eq!(spec_30_360.params.day_count, DayCount::Thirty360);

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
    assert_eq!(spec.params.frequency, cloned.params.frequency);
    assert_eq!(spec.params.day_count, cloned.params.day_count);
}
