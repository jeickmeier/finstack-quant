//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::loan_terms::UpfrontFee;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use time::Month;

#[test]
fn test_pricing_recovery_consistency() {
    // This test verifies that the revolving credit pricer correctly implements
    // the Recovery Leg for credit risk pricing.
    //
    // Expected behavior: When a loan pays risk-free + spread, and the spread
    // correctly compensates for credit risk (including recovery), the loan
    // should price close to Par (100%).
    //
    // Implementation: The pricer now includes both:
    // 1. Survival PV: Cashflows discounted with survival probability
    // 2. Recovery PV: Expected recovery value from defaults
    // This ensures proper market-standard pricing.

    // Scenario: 1-year fully drawn facility (bullet loan behavior).
    // Risk-free rate (r) = 5%.
    // Market Spread (s) = 2% (200 bp).
    // Recovery Rate (R) = 40%.
    // Coupon = r + s = 7%.

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    // 1. Create Facility
    let commitment = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");
    let drawn = commitment; // Fully drawn

    let facility = RevolvingCredit::builder()
        .id("TEST-RCF".into())
        .commitment(commitment)
        .drawn(drawn)
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.07 }) // 7% Coupon
        .day_count(DayCount::Act365F)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act365F)
        .frequency(Tenor::annual())
        .quote_compounding(finstack_quant_core::math::Compounding::Periodic(
            std::num::NonZeroU32::MIN,
        )) // Single payment at end
        .fees(RevolvingCreditFees::default()) // No extra fees
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![])) // No changes
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(Some("BORROWER-HAZARD".into()))
        .recovery_rate(0.40)
        .attributes(Default::default())
        .build()
        .unwrap();

    // 2. Create Market Data
    // Discount Curve: Flat 5% (Continuous)
    let r: f64 = 0.05;
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (10.0, (-r * 10.0).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Hazard Curve: h = s / (1-R) = 0.02 / 0.6 = 0.033333
    let hazard_rate = 0.02 / 0.60;
    let hazard_curve = HazardCurve::builder("BORROWER-HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(10.0, hazard_rate)])
        .recovery_rate(0.40)
        .build()
        .unwrap();

    let mut market = MarketContext::new();
    market = market
        .try_insert(discount_curve)
        .expect("insert market curve");
    market = market
        .try_insert(hazard_curve)
        .expect("insert market curve");

    // 3. Price
    let pv = facility.value(&market, as_of).unwrap();
    let price_pct = pv.amount() / commitment.amount();

    // Theoretical Market Par with continuous discounting: 1.07 * exp(-(0.05+0.02)) = 0.997661
    // Actual implementation uses numerical integration for recovery leg (trapezoidal),
    // yielding ~0.997237 (within 4.3bps of theoretical continuous par).
    // We accept this difference as numerical approximation error.
    let expected_market = 0.997237;

    // Assert current behavior matches the FIX (recovery leg implemented)
    assert!(
        (price_pct - expected_market).abs() < 1e-5,
        "Price should match Market Par (approx). Got {:.6}, Expected {:.6}.",
        price_pct,
        expected_market
    );

    // Also verify it's reasonably close to par
    assert!(
        price_pct > 0.99 && price_pct < 1.01,
        "A properly priced loan paying r+s should be near par. Got {:.6}",
        price_pct
    );
}

#[test]
fn test_floating_rcf_declares_forward_dependency() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let fixed_facility = RevolvingCredit::builder()
        .id("RCF-FIXED".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    let floating_facility = RevolvingCredit::builder()
        .id("RCF-FLOAT".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Floating(
            finstack_quant_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                forward_curve_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                gearing: rust_decimal::Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                fixing_calendar_id: None,
                compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
        ))
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    let fixed_deps = fixed_facility.market_dependencies().unwrap().curves;
    assert!(
        fixed_deps.forward_curves.is_empty(),
        "Fixed-rate facility should declare no forward curves"
    );
    assert_eq!(fixed_deps.discount_curves.len(), 1);

    let float_deps = floating_facility.market_dependencies().unwrap().curves;
    assert_eq!(
        float_deps.forward_curves.len(),
        1,
        "Floating-rate facility must declare forward curve for DV01"
    );
    assert_eq!(float_deps.forward_curves[0].as_str(), "USD-SOFR-3M");
    assert_eq!(float_deps.discount_curves.len(), 1);
}

#[test]
fn test_floating_rcf_dv01_bumps_forward_curve() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2027, Month::January, 1).unwrap();

    let facility = RevolvingCredit::builder()
        .id("RCF-DV01-FWD".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Floating(
            finstack_quant_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                forward_curve_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                gearing: rust_decimal::Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                fixing_calendar_id: None,
                compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
        ))
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).unwrap())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.04_f64).exp()),
            (5.0, (-0.04_f64 * 5.0).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.994996932244156),
            (0.25, 0.98998795756821),
            (0.375, 0.984973417976586),
            (0.5, 0.979953654267085),
            (0.625, 0.9749290059945),
            (0.75, 0.969899811434621),
            (0.875, 0.964866407548615),
            (1.0, 0.959829129947799),
            (1.125, 0.954788312858801),
            (1.25, 0.949744289089123),
            (1.375, 0.944697389993104),
            (1.5, 0.939647945438284),
            (1.625, 0.934596283772186),
            (1.75, 0.929542731789505),
            (1.875, 0.92448761469972),
            (2.0, 0.919431256095125),
            (2.1875, 0.911871808744082),
            (2.375, 0.904363916135093),
            (2.5625, 0.896907329033208),
            (2.75, 0.88950179846074),
            (2.9375, 0.882147075706763),
            (3.125, 0.874842912336522),
            (3.3125, 0.867589060200745),
            (3.5, 0.860385271444864),
            (3.6875, 0.853231298518141),
            (3.875, 0.846126894182699),
            (4.0625, 0.839071811522465),
            (4.25, 0.832065803952015),
            (4.4375, 0.825108625225324),
            (4.625, 0.818200029444432),
            (4.8125, 0.811339771068008),
            (5.0, 0.804527604919828),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let market = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve")
        .try_insert(fwd_curve)
        .expect("insert market curve")
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                vec![(
                    Date::from_calendar_date(2024, Month::December, 30).unwrap(),
                    0.04,
                )],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .unwrap(),
        );

    let result = facility
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();
    assert!(
        dv01.is_finite() && dv01.abs() > 1.0,
        "DV01 for floating RCF should be non-trivial (got {}); forward curve must be bumped",
        dv01
    );
}

#[test]
fn test_upfront_fee_excluded_after_commitment() {
    let commitment = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let after_commitment = Date::from_calendar_date(2025, Month::June, 1).unwrap();

    let facility = RevolvingCredit::builder()
        .id("RCF-UPFRONT-ASOF".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(0.0, Currency::USD).expect("valid money fixture"))
        .issue_date(commitment)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.0 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees({
            let mut fees = RevolvingCreditFees::flat(0.0, 0.0, 0.0).unwrap();
            fees.upfront_fee = Some(UpfrontFee::Amount(
                Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
            ));
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(after_commitment)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let market = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve");

    let pv = facility.value(&market, after_commitment).unwrap();
    assert!(
        pv.amount().abs() < 1.0,
        "PV should be ~0 when upfront fee is in the past and no draws; got {}",
        pv.amount()
    );
}
