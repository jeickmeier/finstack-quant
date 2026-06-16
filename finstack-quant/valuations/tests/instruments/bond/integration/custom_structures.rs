//! Custom bond structure integration tests (PIK, step-up, etc.).

use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use time::macros::date;

fn create_curve() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(date!(2025 - 01 - 01))
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    MarketContext::new().insert(curve)
}

#[test]
fn test_pik_bond() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::PIK,
            rate: rust_decimal::Decimal::try_from(0.08).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .unwrap();

    let bond = Bond::from_cashflows("PIK", schedule, "USD-OIS", None).unwrap();
    let market = create_curve();
    let pv = bond.value(&market, issue).unwrap();

    assert!(pv.amount() > 0.0);
}

#[test]
fn test_step_up_bond() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2028 - 01 - 01);
    let step1 = date!(2026 - 01 - 01);
    let step2 = date!(2027 - 01 - 01);

    let params = ScheduleParams {
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1000.0, Currency::USD), issue, maturity)
        .fixed_stepup_decimal(
            &[
                (step1, Decimal::new(4, 2)),
                (step2, Decimal::new(5, 2)),
                (maturity, Decimal::new(6, 2)),
            ],
            params,
            CouponType::Cash,
        )
        .build_with_curves(None)
        .unwrap();

    let bond = Bond::from_cashflows("STEPUP", schedule, "USD-OIS", None).unwrap();
    let market = create_curve();
    let pv = bond.value(&market, issue).unwrap();

    assert!(pv.amount() > 0.0);
}
