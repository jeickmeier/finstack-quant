//! Yield to worst tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_ytw_is_minimum_of_ytm_and_ytc() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-YTW-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2029 - 01 - 01))
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
        .pricing_overrides(Default::default())
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
        &[MetricId::Ytm, MetricId::Ytw, MetricId::custom("ytc")],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let ytm = *result.measures.get("ytm").unwrap();
    let ytc = *result.measures.get("ytc").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    // YTW must be minimum of YTM and YTC
    assert!(ytw <= ytm + 1e-12);
    assert!(ytw <= ytc + 1e-12);
}

/// Regression test: callable AMORTIZING loan where call falls on a coupon date.
///
/// Verifies YTW == min(YTC, YTM) even when there is amortization at the call date.
/// This exercises the kind-aware flow filtering in `solve_irr_to_exercise` which
/// must include the coupon at the call date while excluding amortization/notional.
#[test]
fn test_ytw_callable_amortizing_loan_coupon_on_call_date() {
    // Arrange: quarterly loan with 2.5% per-period amortization, callable at 102%
    // on a date that coincides with a quarterly coupon date.
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let call_date = date!(2027 - 07 - 01); // mid-life, coincides with quarterly coupon

    let mut loan = TermLoan::builder()
        .id("TL-YTW-AMORT-CALL".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::PercentPerPeriod { bp: 250 }) // 2.5% per quarter
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: call_date,
            price_pct_of_par: 102.0,
            call_type: LoanCallType::Hard,
        }],
    });

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Ytw, MetricId::custom("ytc")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing should succeed");

    let ytm = *result.measures.get("ytm").unwrap();
    let ytc = *result.measures.get("ytc").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    // Assert: all yields must be finite and positive
    assert!(ytm.is_finite() && ytm > 0.0, "YTM = {ytm}");
    assert!(ytc.is_finite() && ytc > 0.0, "YTC = {ytc}");
    assert!(ytw.is_finite() && ytw > 0.0, "YTW = {ytw}");

    // Assert: YTW <= min(YTM, YTC) with tolerance for IRR convergence
    let expected_min = ytm.min(ytc);
    assert!(
        ytw <= expected_min + 1e-8,
        "YTW ({ytw}) should be <= min(YTM={ytm}, YTC={ytc}) = {expected_min}"
    );

    // Assert: YTW should be very close to the minimum
    assert!(
        (ytw - expected_min).abs() < 1e-6,
        "YTW ({ytw}) should approximately equal min(YTM, YTC) = {expected_min}"
    );
}

#[test]
fn test_ytw_uses_quoted_clean_price_when_present() {
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-YTW-QUOTE".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2029 - 01 - 01))
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
        .pricing_overrides(Default::default())
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
            &[MetricId::Ytw],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytw_base = *base.measures.get("ytw").unwrap();

    loan.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(95.0);
    let quoted = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytw],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytw_quoted = *quoted.measures.get("ytw").unwrap();

    assert!(
        ytw_quoted > ytw_base,
        "Lower quoted clean price should increase YTW: base={ytw_base}, quoted={ytw_quoted}"
    );
}
