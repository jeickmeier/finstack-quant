//! Tests for Discountable trait and NPV calculations.

use finstack_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::traits::{Discounting, TermStructure};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::Discountable;
use rust_decimal_macros::dec;
use time::Month;

use crate::common::test_helpers::*;

struct FlatCurve {
    id: CurveId,
    rate: f64,
}

impl TermStructure for FlatCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for FlatCurve {
    fn base_date(&self) -> Date {
        dates::TODAY
    }

    fn df(&self, t: f64) -> f64 {
        (-self.rate * t).exp()
    }
}

#[test]
fn test_schedule_discountable_simple() {
    // Arrange
    let curve = FlatCurve {
        id: CurveId::new("USD-OIS"),
        rate: 0.05,
    };

    let issue = dates::TODAY;
    let maturity = Date::from_calendar_date(2025, Month::July, 1).unwrap();
    let params = ScheduleParams {
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: dec!(0.05),
        freq: params.freq,
        dc: params.dc,
        bdc: params.bdc,
        calendar_id: params.calendar_id.clone(),
        stub: params.stub,
        end_of_month: params.end_of_month,
        payment_lag_days: params.payment_lag_days,
    };

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000.0, Currency::USD), issue, maturity)
        .fixed_cf(fixed)
        .build_with_curves(None)
        .unwrap();

    // Act - use explicit day count
    let pv = schedule
        .npv(&curve, curve.base_date(), Some(DayCount::Act365F))
        .unwrap();

    // Assert
    assert!(pv.amount().is_finite(), "PV is finite");
    // Note: PV includes initial notional outflow at issue date
    // With 5% coupon and 5% discount rate, bond trades near par
    // PV should be close to 0 (slightly negative due to time value)
    assert!(
        pv.amount().abs() < 1.0,
        "PV near zero for at-par bond: got {}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn test_npv_zero_rate() {
    // Arrange: Zero rate means no discounting
    let curve = FlatCurve {
        id: CurveId::new("TEST"),
        rate: 0.0,
    };

    let issue = dates::TODAY;
    let maturity = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let params = ScheduleParams {
        freq: Tenor::annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: dec!(0.05),
        freq: params.freq,
        dc: params.dc,
        bdc: params.bdc,
        calendar_id: params.calendar_id.clone(),
        stub: params.stub,
        end_of_month: params.end_of_month,
        payment_lag_days: params.payment_lag_days,
    };

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000.0, Currency::USD), issue, maturity)
        .fixed_cf(fixed)
        .build_with_curves(None)
        .unwrap();

    // Act - use explicit day count
    let pv = schedule
        .npv(&curve, curve.base_date(), Some(DayCount::Act365F))
        .unwrap();

    // Assert: With zero rate, PV = sum of cashflows
    // Cashflows: -1000 (initial outflow) + coupon + 1000 (principal repayment)
    // With Act365F over a leap year (366 days): coupon ≈ 1000 * 0.05 * (366/365) ≈ 50.14
    // Tolerance of 1.0 covers day count variations across leap/non-leap years
    let expected = 1_000.0 * 0.05;
    assert_approx_eq(pv.amount(), expected, 1.0, "PV equals cashflow sum at 0%");
}
