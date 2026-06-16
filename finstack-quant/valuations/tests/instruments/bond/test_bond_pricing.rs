//! Determinism tests for bond pricing.
//!
//! Verifies that bond valuation produces bitwise-identical results across
//! multiple runs with the same inputs, and validates correctness against
//! market standards.

use crate::common::test_helpers::tolerances;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn create_test_bond() -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    Bond::fixed(
        "BOND-DETERMINISM-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05, // 5% coupon
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("Test bond creation should succeed")
}

fn create_test_market(base_date: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    MarketContext::new().insert(curve)
}

#[test]
fn test_bond_pv_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Price the bond 100 times
    let prices: Vec<f64> = (0..100)
        .map(|_| bond.value(&market, as_of).unwrap().amount())
        .collect();

    // All prices must be bitwise identical
    for i in 1..prices.len() {
        assert_eq!(
            prices[i], prices[0],
            "Bond PV at iteration {} = {:.15} differs from iteration 0 = {:.15}",
            i, prices[i], prices[0]
        );
    }

    // Correctness check: Bond PV should be in reasonable range
    // Note: The discount curve uses discount factors (not rates), which implies ~2-4% rates,
    // so a 5% coupon bond will price above par. Verify PV is positive and reasonable.
    let notional = 1_000_000.0;
    assert!(
        prices[0] > notional * 0.8 && prices[0] < notional * 1.5,
        "Bond PV {} outside reasonable range (80%-150% of notional)",
        prices[0]
    );
}

#[test]
fn test_bond_ytm_determinism() {
    let mut bond = create_test_bond();
    // Set quoted clean price at par (100% of face) to enable YTM calculation
    bond.pricing_overrides.market_quotes.quoted_clean_price = Some(100.0);

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate YTM 50 times (involves iterative solver)
    let ytms: Vec<f64> = (0..50)
        .map(|_| {
            let result = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Ytm],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Ytm.as_str()]
        })
        .collect();

    // All YTMs must be bitwise identical despite solver iterations
    for i in 1..ytms.len() {
        assert_eq!(
            ytms[i], ytms[0],
            "YTM at iteration {} = {:.15} differs from iteration 0 = {:.15}",
            i, ytms[i], ytms[0]
        );
    }

    // Correctness: par bond YTM should equal coupon rate (5%)
    let coupon_rate = 0.05;
    assert!(
        (ytms[0] - coupon_rate).abs() < tolerances::NUMERICAL,
        "YTM {} should equal coupon {} for par bond (tolerance {})",
        ytms[0],
        coupon_rate,
        tolerances::NUMERICAL
    );
}

#[test]
fn test_bond_duration_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate modified duration 50 times
    let durations: Vec<f64> = (0..50)
        .map(|_| {
            let result = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::DurationMod],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::DurationMod.as_str()]
        })
        .collect();

    // All durations must be identical
    for i in 1..durations.len() {
        assert_eq!(
            durations[i], durations[0],
            "Duration differs at iteration {}: {} vs {}",
            i, durations[i], durations[0]
        );
    }

    // Correctness: 5Y bond modified duration should be in reasonable range [0, 10]
    assert!(
        durations[0] > 0.0 && durations[0] < 10.0,
        "5Y bond mod duration {} outside reasonable range [0, 10]",
        durations[0]
    );
}

#[test]
fn test_bond_convexity_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate convexity 50 times
    let convexities: Vec<f64> = (0..50)
        .map(|_| {
            let result = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Convexity],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Convexity.as_str()]
        })
        .collect();

    // All convexities must be identical
    for i in 1..convexities.len() {
        assert_eq!(
            convexities[i], convexities[0],
            "Convexity differs at iteration {}: {} vs {}",
            i, convexities[i], convexities[0]
        );
    }

    // Correctness: 5Y bond convexity should be in reasonable range [0, 100]
    assert!(
        convexities[0] > 0.0 && convexities[0] < 100.0,
        "5Y bond convexity {} outside reasonable range [0, 100]",
        convexities[0]
    );
}

#[test]
fn test_bond_dv01_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate DV01 50 times
    let dv01s: Vec<f64> = (0..50)
        .map(|_| {
            let result = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Dv01],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Dv01.as_str()]
        })
        .collect();

    // All DV01s must be identical
    for i in 1..dv01s.len() {
        assert_eq!(
            dv01s[i], dv01s[0],
            "DV01 differs at iteration {}: {} vs {}",
            i, dv01s[i], dv01s[0]
        );
    }
}

#[test]
fn test_bond_accrued_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::March, 1).unwrap();
    let market = create_test_market(as_of);

    // Calculate accrued interest 100 times
    let accrueds: Vec<f64> = (0..100)
        .map(|_| {
            let result = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Accrued],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Accrued.as_str()]
        })
        .collect();

    // All accrued values must be identical
    for i in 1..accrueds.len() {
        assert_eq!(
            accrueds[i], accrueds[0],
            "Accrued interest differs at iteration {}: {} vs {}",
            i, accrueds[i], accrueds[0]
        );
    }
}

#[test]
fn test_bond_multiple_metrics_determinism() {
    let bond = create_test_bond();
    let as_of = Date::from_calendar_date(2025, Month::February, 1).unwrap();
    let market = create_test_market(as_of);

    let metrics = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::DurationMod,
        MetricId::Convexity,
        MetricId::Dv01,
    ];

    // Calculate all metrics 20 times
    let results: Vec<_> = (0..20)
        .map(|_| {
            bond.price_with_metrics(
                &market,
                as_of,
                &metrics,
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap()
        })
        .collect();

    // Verify each metric is deterministic
    for metric in &metrics {
        let values: Vec<f64> = results
            .iter()
            .map(|r| r.measures[metric.as_str()])
            .collect();

        for i in 1..values.len() {
            assert_eq!(
                values[i],
                values[0],
                "{} differs at iteration {}: {} vs {}",
                metric.as_str(),
                i,
                values[i],
                values[0]
            );
        }
    }
}
