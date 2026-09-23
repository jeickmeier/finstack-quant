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
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 }) // 5%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 300 }) // 3%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 700 }) // 7%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
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

#[test]
fn expired_term_loan_prices_at_zero_without_market_history() {
    let loan = TermLoan::example().expect("example loan");
    let value = loan
        .value(&MarketContext::new(), date!(2030 - 01 - 01))
        .expect("expired loan should price without curves or historical fixings");
    assert_eq!(
        value,
        Money::new(0.0, Currency::USD).expect("valid money fixture")
    );
}

/// The instrument PV is anchored at `as_of`, like the bond and the revolver:
/// every flow after `as_of` is discounted to `as_of`, including a coupon paid
/// between the trade date and settlement. 10M at 5% Act/360 quarterly from
/// 2025-01-01, valued Monday 2025-03-31 (T+2 settlement 2025-04-02) on a flat
/// 3% continuously compounded ACT/365F curve.
#[test]
fn pv_is_discounted_to_as_of_and_includes_flows_before_settlement() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let issue = date!(2025 - 01 - 01);
    let as_of = date!(2025 - 03 - 31);
    let loan = TermLoan::builder()
        .id("TL-ANCHOR".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("money"))
        .issue_date(issue)
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::Unadjusted)
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
    assert_eq!(loan.settlement_date(as_of).unwrap(), date!(2025 - 04 - 02));

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots((0..=5).map(|i| (f64::from(i), (-0.03 * f64::from(i)).exp())))
        .build()
        .unwrap();
    let market = MarketContext::new().insert(curve);

    let df = |d: time::Date| (-0.03 * (d - as_of).whole_days() as f64 / 365.0).exp();
    let coupon = |start: time::Date, end: time::Date| {
        10_000_000.0 * 0.05 * (end - start).whole_days() as f64 / 360.0
    };
    let dates = [
        issue,
        date!(2025 - 04 - 01),
        date!(2025 - 07 - 01),
        date!(2025 - 10 - 01),
        date!(2026 - 01 - 01),
    ];
    let mut expected = 10_000_000.0 * df(dates[4]);
    for pair in dates.windows(2) {
        expected += coupon(pair[0], pair[1]) * df(pair[1]);
    }

    let pv = loan.value(&market, as_of).unwrap().amount();
    assert!((pv - expected).abs() < 1e-4, "pv {pv} vs hand {expected}");
}
