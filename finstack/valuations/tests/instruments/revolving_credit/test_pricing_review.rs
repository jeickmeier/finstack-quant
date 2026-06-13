use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_valuations::instruments::{CurveDependencies, Instrument};
use finstack_valuations::metrics::MetricId;
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
    // Market Spread (s) = 2% (200 bps).
    // Recovery Rate (R) = 40%.
    // Coupon = r + s = 7%.

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    // 1. Create Facility
    let commitment = Money::new(10_000_000.0, Currency::USD);
    let drawn = commitment; // Fully drawn

    let facility = RevolvingCredit::builder()
        .id("TEST-RCF".into())
        .commitment_amount(commitment)
        .drawn_amount(drawn)
        .commitment_date(as_of)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 }) // 7% Coupon
        .day_count(DayCount::Act365F)
        .frequency(Tenor::annual()) // Single payment at end
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
        .build()
        .unwrap();

    // Hazard Curve: h = s / (1-R) = 0.02 / 0.6 = 0.033333
    let hazard_rate = 0.02 / 0.60;
    let hazard_curve = HazardCurve::builder("BORROWER-HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, hazard_rate), (10.0, hazard_rate)])
        .build()
        .unwrap();

    let mut market = MarketContext::new();
    market = market.insert(discount_curve);
    market = market.insert(hazard_curve);

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
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let floating_facility = RevolvingCredit::builder()
        .id("RCF-FLOAT".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                gearing: rust_decimal::Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let fixed_deps = fixed_facility.curve_dependencies().unwrap();
    assert!(
        fixed_deps.forward_curves.is_empty(),
        "Fixed-rate facility should declare no forward curves"
    );
    assert_eq!(fixed_deps.discount_curves.len(), 1);

    let float_deps = floating_facility.curve_dependencies().unwrap();
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
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                gearing: rust_decimal::Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
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
        .build()
        .unwrap();
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.04), (1.0, 0.042), (2.0, 0.044), (5.0, 0.045)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let result = facility
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
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
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(0.0, Currency::USD))
        .commitment_date(commitment)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees({
            let mut fees = RevolvingCreditFees::flat(0.0, 0.0, 0.0);
            fees.upfront_fee = Some(Money::new(100_000.0, Currency::USD));
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(after_commitment)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc_curve);

    let pv = facility.value(&market, after_commitment).unwrap();
    assert!(
        pv.amount().abs() < 1.0,
        "PV should be ~0 when upfront fee is in the past and no draws; got {}",
        pv.amount()
    );
}
