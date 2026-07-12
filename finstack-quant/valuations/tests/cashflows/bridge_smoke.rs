//! Smoke tests for the `finstack_quant_cashflows` bridge re-export.

use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec, ScheduleParams};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};
use time::Month;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CashflowEnvelope<T> {
    schema: String,
    #[serde(flatten)]
    payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixedCouponSpecPayload {
    fixed_coupon_spec: FixedCouponSpec,
}

#[test]
fn bridge_builder_schedule_builds() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");

    let fixed_spec = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal_macros::dec!(0.05),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(fixed_spec)
        .build_with_curves(None)
        .expect("bridge builder should build schedule");

    assert!(!schedule.flows.is_empty(), "expected non-empty schedule");
}

#[test]
fn bridge_period_generation_works() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    let params = ScheduleParams {
        freq: Tenor::quarterly(),
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "usny".to_string(),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };

    let periods = finstack_quant_cashflows::builder::periods::build_periods(
        finstack_quant_cashflows::builder::periods::BuildPeriodsParams {
            start: issue,
            end: maturity,
            frequency: params.freq,
            stub: params.stub,
            bdc: params.bdc,
            calendar_id: &params.calendar_id,
            end_of_month: params.end_of_month,
            day_count: params.dc,
            payment_lag_days: params.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: params.adjust_accrual_dates,
        },
    )
    .expect("bridge date generation should work");

    assert!(periods.len() >= 2, "expected multiple schedule periods");
}

#[test]
fn bridge_schema_serde_smoke() {
    let params = ScheduleParams::usd_sofr_swap();
    let json = serde_json::to_string(&params).expect("serialize schedule params");
    let roundtrip: ScheduleParams =
        serde_json::from_str(&json).expect("deserialize schedule params");

    assert_eq!(roundtrip.calendar_id, params.calendar_id);
    assert_eq!(roundtrip.dc, params.dc);
}

#[test]
fn bridge_schema_example_roundtrip_smoke() {
    let json =
        include_str!("../../../cashflows/tests/cashflows/examples/fixed_coupon_spec.example.json");
    let envelope: CashflowEnvelope<FixedCouponSpecPayload> =
        serde_json::from_str(json).expect("deserialize moved cashflow example via bridge types");

    let reserialized =
        serde_json::to_string(&envelope).expect("re-serialize moved cashflow example");
    let original_value: serde_json::Value =
        serde_json::from_str(json).expect("parse original cashflow example JSON");
    let reserialized_value: serde_json::Value =
        serde_json::from_str(&reserialized).expect("parse re-serialized cashflow example JSON");

    assert_eq!(original_value, reserialized_value);
}
