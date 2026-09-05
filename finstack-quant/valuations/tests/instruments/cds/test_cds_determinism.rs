//! Determinism tests for CDS pricing.
//!
//! Verifies that CDS valuation produces bitwise-identical results including
//! par spreads, CS01, and protection/premium leg calculations, and validates
//! correctness against market standards.

use crate::test_support::credit as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
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
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
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

    let source = MarketContext::new().insert(disc);
    let hazard = test_utils::calibrated_hazard_curve(
        &source,
        base_date,
        "ACME-HAZARD",
        "ACME-Corp",
        "USD-OIS",
    )
    .unwrap();
    source.insert(hazard)
}

fn metric_value(
    cds: &CreditDefaultSwap,
    market: &MarketContext,
    as_of: Date,
    metric: MetricId,
) -> f64 {
    let result = cds
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            test_utils::pricing_options(),
        )
        .unwrap();
    result.measures[metric.as_str()]
}

#[test]
fn test_cds_pv_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let first = cds.value(&market, as_of).unwrap().amount();
    let second = cds.value(&market, as_of).unwrap().amount();

    assert_eq!(second, first, "CDS PV must be bitwise deterministic");
}

#[test]
fn test_cds_cs01_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let first = metric_value(&cds, &market, as_of, MetricId::Cs01);
    let second = metric_value(&cds, &market, as_of, MetricId::Cs01);

    assert_eq!(second, first, "CS01 must be bitwise deterministic");
}

#[test]
fn test_cds_par_spread_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let first = metric_value(&cds, &market, as_of, MetricId::ParSpread);
    let second = metric_value(&cds, &market, as_of, MetricId::ParSpread);

    assert_eq!(second, first, "par spread must be bitwise deterministic");

    // Correctness: Credit triangle approximation: par_spread ≈ hazard × (1-R) × 10000
    // With hazard ~1.5% at short end and recovery 40%:
    // Expected ~= 0.015 × 0.60 × 10000 = 90bp
    // Allow wider tolerance for curve shape effects
    let min_spread = 60.0; // bp
    let max_spread = 150.0; // bp
    assert!(
        first > min_spread && first < max_spread,
        "Par spread {} bp outside expected range [{}, {}] bp",
        first,
        min_spread,
        max_spread
    );
}

#[test]
fn test_cds_risky_annuity_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let first = metric_value(&cds, &market, as_of, MetricId::RiskyPv01);
    let second = metric_value(&cds, &market, as_of, MetricId::RiskyPv01);

    assert_eq!(second, first, "risky PV01 must be bitwise deterministic");
}

#[test]
fn test_cds_all_metrics_determinism() {
    let cds = create_test_cds();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let metrics = [
        MetricId::Cs01,
        MetricId::ParSpread,
        MetricId::RiskyPv01,
        MetricId::ProtectionLegPv,
        MetricId::PremiumLegPv,
    ];

    let first = cds
        .price_with_metrics(&market, as_of, &metrics, test_utils::pricing_options())
        .unwrap();
    let second = cds
        .price_with_metrics(&market, as_of, &metrics, test_utils::pricing_options())
        .unwrap();

    for metric in &metrics {
        assert_eq!(
            second.measures[metric.as_str()],
            first.measures[metric.as_str()],
            "{} must be bitwise deterministic",
            metric.as_str()
        );
    }

    // Correctness: Protection and premium legs should have reasonable ratio
    // Note: Legs won't exactly balance since CDS is not at par spread
    // But their ratio should be reasonable
    let protection_pv = first.measures[MetricId::ProtectionLegPv.as_str()];
    let premium_pv = first.measures[MetricId::PremiumLegPv.as_str()];
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
            Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            100.0, // 100bp spread
            as_of,
            maturity,
            "USD-OIS",     // discount_curve_id
            "ACME-HAZARD", // credit_id
        )
        .expect("CDS construction should succeed");

        let first = cds.value(&market, as_of).unwrap().amount();
        let second = cds.value(&market, as_of).unwrap().amount();

        assert_eq!(
            second, first,
            "maturity {maturity}: CDS PV must be bitwise deterministic"
        );
    }
}
