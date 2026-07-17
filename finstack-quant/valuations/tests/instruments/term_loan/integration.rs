//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_cashflows::builder::specs::{CouponType, FloatingRateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    self, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

fn build_flat_discount_curve(
    rate: f64,
    base_date: Date,
    curve_id: &str,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    flat_discount_curve(rate, base_date, curve_id)
}

fn build_simple_term_loan(as_of: Date, maturity: Date) -> TermLoan {
    TermLoan::builder()
        .id("TL-METRICS".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 600 }) // 6%
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap()
}

#[test]
fn test_term_loan_yields_with_callability() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2029 - 01 - 01); // 4Y
    let mut loan = build_simple_term_loan(as_of, maturity);

    // Add first call at 2027-01-01 at 101% of outstanding
    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2027 - 01 - 01),
            price_pct_of_par: 101.0,
            call_type: LoanCallType::Hard,
        }],
    });

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Compute YTM, YTC, YTW
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Ytw, MetricId::custom("ytc")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let ytm = *result.measures.get("ytm").unwrap();
    let ytc = *result.measures.get("ytc").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    // Sanity: yields are finite and positive in this setup
    assert!(ytm.is_finite() && ytm > 0.0);
    assert!(ytc.is_finite() && ytc > 0.0);
    assert!(ytw.is_finite() && ytw > 0.0);

    // YTW must be the min of YTM and YTC (maturity vs first call)
    assert!(ytw <= ytm + 1e-12);
    assert!(ytw <= ytc + 1e-12);
}

#[test]
fn test_term_loan_yields_to_horizons() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2029 - 01 - 01); // 4Y
    let loan = build_simple_term_loan(as_of, maturity);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Compute YT2Y, YT3Y, YT4Y and YTM for ordering checks
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Ytm,
                MetricId::custom("yt2y"),
                MetricId::custom("yt3y"),
                MetricId::custom("yt4y"),
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let ytm = *result.measures.get("ytm").unwrap();
    let yt2y = *result.measures.get("yt2y").unwrap();
    let yt3y = *result.measures.get("yt3y").unwrap();
    let yt4y = *result.measures.get("yt4y").unwrap();

    // All finite and positive
    for y in [ytm, yt2y, yt3y, yt4y] {
        assert!(y.is_finite() && y > 0.0);
    }

    // Horizon yields should be reasonably close in this setup
    let min_y = yt2y.min(yt3y).min(yt4y);
    let max_y = yt2y.max(yt3y).max(yt4y);
    assert!(
        max_y - min_y < 0.02,
        "Horizon yields should be within 200bp band: yt2y={yt2y}, yt3y={yt3y}, yt4y={yt4y}"
    );

    // YT4Y equals YTM when maturity is 4 years
    assert!((yt4y - ytm).abs() < 1e-8);
}

/// End-to-end floating-rate term loan yield and discount margin test.
///
/// Builds a floating-rate (SOFR + 250bp) term loan with a flat forward curve,
/// prices it, then verifies YTM and Discount Margin are both computed and
/// internally consistent.
#[test]
fn test_floating_rate_term_loan_yield_and_dm() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2028 - 01 - 01); // 3Y

    // Build floating-rate loan: SOFR + 250bp
    let loan = TermLoan::builder()
        .id("TL-FLOAT-DM".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::from("USD-SOFR"),
            spread_bp: Decimal::from(250),
            gearing: Decimal::from(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }))
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .expect("floating loan construction should succeed");

    // Market: flat SOFR = 4.5%, flat discount = 5%
    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(as_of)
        .knots([(0.0, 0.045), (3.0, 0.045), (10.0, 0.045)])
        .build()
        .expect("forward curve");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Act: compute PV, YTM, and Discount Margin
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing should succeed");

    let pv = result.value.amount();
    let ytm = *result.measures.get("ytm").unwrap();
    let dm = *result.measures.get("discount_margin").unwrap();

    // Sanity: PV should be positive and in the right ballpark
    assert!(
        pv > 0.0 && pv < 15_000_000.0,
        "PV = {pv} should be reasonable"
    );

    // YTM should be positive and reflect the SOFR + spread
    assert!(
        ytm.is_finite() && ytm > 0.0,
        "YTM = {ytm} should be finite and positive"
    );

    // Discount margin should be finite
    assert!(dm.is_finite(), "Discount margin = {dm} should be finite");

    // For a par-ish loan, DM should be in the neighborhood of the spread (250bp = 0.025).
    // We allow a band of ~300bp because DM depends on the discount/forward curves,
    // but anything beyond that signals a regression.
    assert!(
        dm.abs() < 0.03,
        "Discount margin = {dm} should be within reasonable range (< 300bp)"
    );
}
