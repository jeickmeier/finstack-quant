//! Tests for Discountable trait and NPV calculations.

use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
};
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::traits::{Discounting, TermStructure};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
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
    let pv = schedule.npv(&curve, curve.base_date()).unwrap();

    // Assert
    assert!(pv.amount().is_finite(), "PV is finite");
    // The issue-date funding exchange is already settled, so schedule NPV is
    // the value of the strictly future coupon and redemption cashflows.
    assert!(
        (pv.amount() - 1_000.0).abs() < 1.0,
        "future cashflows should value near par: got {}",
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
    let pv = schedule.npv(&curve, curve.base_date()).unwrap();

    // The issue-date funding exchange is excluded; at zero rates the NPV is
    // exactly the sum of strictly future coupon and redemption cashflows.
    let expected = schedule
        .flows
        .iter()
        .filter(|cf| cf.date > curve.base_date())
        .map(|cf| cf.amount.amount())
        .sum();
    assert_approx_eq(pv.amount(), expected, 1.0, "PV equals cashflow sum at 0%");
}
