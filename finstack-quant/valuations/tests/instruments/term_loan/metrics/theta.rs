//! Theta (time decay) tests.

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
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_theta_reflects_time_decay() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-THETA-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
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
    let result = loan.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Theta],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let theta = *result.measures.get("theta").unwrap();

    // Theta should be finite
    assert!(theta.is_finite());
}
