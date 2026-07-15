//! Term loan pricing tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_par_loan_pricing() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-PAR".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 }) // 5%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    // At par, PV should be close to notional
    assert!((pv.amount() - 10_000_000.0).abs() < 50_000.0);
}

#[test]
fn test_discount_pricing() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-DISCOUNT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 300 }) // 3%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Market rate higher than coupon rate
    let disc_curve = flat_discount_curve(0.06, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    // Should trade at discount (below par)
    assert!(pv.amount() < 10_000_000.0);
}

#[test]
fn test_premium_pricing() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-PREMIUM".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 700 }) // 7%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Market rate lower than coupon rate
    let disc_curve = flat_discount_curve(0.04, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    // Should trade at premium (above par)
    assert!(pv.amount() > 10_000_000.0);
}
