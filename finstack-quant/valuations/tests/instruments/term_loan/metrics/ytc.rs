//! Yield to call tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_ytc_callable_loan() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-YTC-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 600 })
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

    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2027 - 01 - 01),
            price_pct_of_par: 101.0,
            call_type: LoanCallType::Hard,
        }],
    });

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let result = loan.price_with_metrics(
        &market,
        as_of,
        &[MetricId::custom("ytc")],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let ytc = *result.measures.get("ytc").unwrap();

    assert!(ytc.is_finite() && ytc > 0.0);
}

#[test]
fn test_ytc_uses_quoted_clean_price_when_present() {
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-YTC-QUOTE".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 600 })
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

    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2027 - 01 - 01),
            price_pct_of_par: 101.0,
            call_type: LoanCallType::Hard,
        }],
    });

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let base = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::custom("ytc")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytc_base = *base.measures.get("ytc").unwrap();

    loan.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(95.0);
    let quoted = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::custom("ytc")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytc_quoted = *quoted.measures.get("ytc").unwrap();

    assert!(
        ytc_quoted > ytc_base,
        "Lower quoted clean price should increase YTC: base={ytc_base}, quoted={ytc_quoted}"
    );
}
