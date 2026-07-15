//! Yield to maturity tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_ytm_par_loan() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-YTM-PAR".into())
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
        &[MetricId::Ytm],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let ytm = *result.measures.get("ytm").unwrap();

    // YTM for par loan should approximately match coupon rate (5%)
    //
    // Sources of difference between YTM and coupon rate:
    // 1. Compounding convention mismatch:
    //    - Discount curve uses continuous compounding: DF = exp(-r*t)
    //    - XIRR uses annual compounding: DF = (1+r)^(-t)
    //    - For 5% rate: e^0.05 - 1 = 5.127% vs 5.0% → ~13bp difference
    //
    // 2. Act/360 day count effect:
    //    - Year fractions slightly exceed 1.0 for full years (365/360 ≈ 1.014)
    //    - This adds ~7bp to effective rate
    //
    // Total expected difference: ~20-30bp from par coupon rate
    assert!(ytm.is_finite() && ytm > 0.0);
    assert!(
        (ytm - 0.05).abs() < 0.003, // 30bp tolerance for documented compounding + day count effects
        "YTM {} should be close to coupon 0.05 (within 30bp)",
        ytm
    );
}

#[test]
fn test_ytm_discount_loan() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-YTM-DISC".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 300 })
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

    let disc_curve = flat_discount_curve(0.06, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let result = loan.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Ytm],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let ytm = *result.measures.get("ytm").unwrap();

    // YTM should be higher than coupon for discount loan
    assert!(ytm > 0.03);
}

#[test]
fn test_ytm_uses_quoted_clean_price_when_present() {
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-YTM-QUOTE".into())
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

    let base = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm_base = *base.measures.get("ytm").unwrap();

    loan.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(95.0);
    let quoted = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm_quoted = *quoted.measures.get("ytm").unwrap();

    assert!(
        ytm_quoted > ytm_base,
        "Lower quoted clean price should increase YTM: base={ytm_base}, quoted={ytm_quoted}"
    );
}

#[test]
fn test_ytm_quoted_price_applies_to_outstanding_not_commitment() {
    // Loan-market convention: a quoted price applies to the funded outstanding
    // at settlement, not the original commitment. For a heavily amortized loan
    // quoted at 99, pricing against the full commitment would imply paying far
    // more than the remaining claim and drive the IRR deeply negative.
    let as_of = date!(2027 - 01 - 02);
    let issue = date!(2024 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let commitment = 10_000_000.0;

    // Custom amortization: 30% repaid in 2026 → 70% outstanding at as_of.
    let amort = AmortizationSpec::Custom(vec![(
        date!(2026 - 01 - 01),
        Money::new(0.3 * commitment, Currency::USD),
    )]);

    let mut loan = TermLoan::builder()
        .id("TL-YTM-AMORT-QUOTE".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(commitment, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(amort)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();
    loan.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *result.measures.get("ytm").unwrap();

    // Paying 99% of the 70% outstanding for a 5% coupon claim → yield near the
    // coupon (within ~150bp). The old commitment-based target (0.99 × 10mm for
    // a 7mm claim) produced a deeply negative IRR.
    assert!(
        (ytm - 0.05).abs() < 0.015,
        "YTM should be near the coupon when quoted at ~par on outstanding, got {ytm}"
    );
}
