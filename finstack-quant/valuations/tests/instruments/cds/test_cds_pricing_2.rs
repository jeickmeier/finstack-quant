//! Determinism tests for CDS pricing.
//!
//! Verifies that CDS valuation produces bitwise-identical results including
//! par spreads, CS01, and protection/premium leg calculations, and validates
//! correctness against market standards.

#[allow(unused_imports)]
use crate::common::test_helpers::tolerances;
use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, Seniority};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn create_test_cds() -> CreditDefaultSwap {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    test_utils::cds_buy_protection(
        "CDS-DETERMINISM-TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0, // 100bp spread
        as_of,
        maturity,
        "USD-OIS",     // discount_curve_id
        "ACME-HAZARD", // credit_id (hazard curve contains recovery rate)
    )
    .expect("CDS construction should succeed")
}

fn create_test_market(base_date: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("ACME-HAZARD")
        .issuer("ACME-Corp")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .recovery_rate(0.40)
        .base_date(base_date)
        .knots([
            (0.0, 0.015), // 150bp hazard
            (1.0, 0.016),
            (3.0, 0.018),
            (5.0, 0.020),
            (10.0, 0.025),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

#[test]
fn test_cds_pv_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Price the CDS 100 times
    let prices: Vec<f64> = (0..100)
        .map(|_| cds.value(&market, as_of).unwrap().amount())
        .collect();

    // All prices must be bitwise identical
    for i in 1..prices.len() {
        assert_eq!(
            prices[i], prices[0],
            "CDS PV at iteration {} = {:.15} differs from iteration 0 = {:.15}",
            i, prices[i], prices[0]
        );
    }
}

#[test]
fn test_cds_cs01_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate CS01 50 times
    let cs01s: Vec<f64> = (0..50)
        .map(|_| {
            let result = cds
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Cs01],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Cs01.as_str()]
        })
        .collect();

    // All CS01s must be identical
    for i in 1..cs01s.len() {
        assert_eq!(
            cs01s[i], cs01s[0],
            "CS01 differs at iteration {}: {:.15} vs {:.15}",
            i, cs01s[i], cs01s[0]
        );
    }
}

#[test]
fn test_cds_par_spread_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate par spread 50 times
    let spreads: Vec<f64> = (0..50)
        .map(|_| {
            let result = cds
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::ParSpread],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::ParSpread.as_str()]
        })
        .collect();

    // All spreads must be identical
    for i in 1..spreads.len() {
        assert_eq!(
            spreads[i], spreads[0],
            "Par spread differs at iteration {}: {:.15} vs {:.15}",
            i, spreads[i], spreads[0]
        );
    }

    // Correctness: Credit triangle approximation: par_spread ≈ hazard × (1-R) × 10000
    // With hazard ~1.5% at short end and recovery 40%:
    // Expected ~= 0.015 × 0.60 × 10000 = 90bp
    // Allow wider tolerance for curve shape effects
    let min_spread = 60.0; // bp
    let max_spread = 150.0; // bp
    assert!(
        spreads[0] > min_spread && spreads[0] < max_spread,
        "Par spread {} bp outside expected range [{}, {}] bp",
        spreads[0],
        min_spread,
        max_spread
    );
}

#[test]
fn test_cds_risky_annuity_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate risky PV01 50 times
    let pv01s: Vec<f64> = (0..50)
        .map(|_| {
            let result = cds
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::RiskyPv01],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::RiskyPv01.as_str()]
        })
        .collect();

    // All risky PV01s must be identical
    for i in 1..pv01s.len() {
        assert_eq!(
            pv01s[i], pv01s[0],
            "Risky PV01 differs at iteration {}: {:.15} vs {:.15}",
            i, pv01s[i], pv01s[0]
        );
    }
}

#[test]
fn test_cds_all_metrics_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let metrics = vec![
        MetricId::Cs01,
        MetricId::ParSpread,
        MetricId::RiskyPv01,
        MetricId::ProtectionLegPv,
        MetricId::PremiumLegPv,
    ];

    // Calculate all metrics 30 times
    let results: Vec<_> = (0..30)
        .map(|_| {
            cds.price_with_metrics(
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
                "{} differs at iteration {}: {:.15} vs {:.15}",
                metric.as_str(),
                i,
                values[i],
                values[0]
            );
        }
    }

    // Correctness: Protection and premium legs should have reasonable ratio
    // Note: Legs won't exactly balance since CDS is not at par spread
    // But their ratio should be reasonable
    let protection_pv = results[0].measures[MetricId::ProtectionLegPv.as_str()];
    let premium_pv = results[0].measures[MetricId::PremiumLegPv.as_str()];
    let pv_ratio = protection_pv / premium_pv.abs();
    assert!(
        pv_ratio > 0.1 && pv_ratio < 10.0,
        "Protection/Premium ratio {} outside reasonable range [0.1, 10]",
        pv_ratio
    );
}

#[test]
fn test_cds_different_tenors_determinism() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Test 1Y, 3Y, and 5Y CDS
    let maturities = vec![
        Date::from_calendar_date(2026, Month::January, 15).unwrap(),
        Date::from_calendar_date(2028, Month::January, 15).unwrap(),
        Date::from_calendar_date(2030, Month::January, 15).unwrap(),
    ];

    for maturity in maturities {
        let cds = test_utils::cds_buy_protection(
            format!("CDS-{}", maturity),
            Money::new(10_000_000.0, Currency::USD),
            100.0, // 100bp spread
            as_of,
            maturity,
            "USD-OIS",     // discount_curve_id
            "ACME-HAZARD", // credit_id
        )
        .expect("CDS construction should succeed");

        let prices: Vec<f64> = (0..30)
            .map(|_| cds.value(&market, as_of).unwrap().amount())
            .collect();

        for i in 1..prices.len() {
            assert_eq!(
                prices[i], prices[0],
                "Maturity {} - PV differs at iteration {}: {:.15} vs {:.15}",
                maturity, i, prices[i], prices[0]
            );
        }
    }
}
