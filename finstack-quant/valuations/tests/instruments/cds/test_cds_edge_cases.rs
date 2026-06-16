//! CDS edge cases and error handling tests.
//!
//! Tests boundary conditions, error cases, and numerical stability.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_curves(as_of: Date) -> (DiscountCurve, HazardCurve) {
    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78), (10.0, 0.61)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots([(0.0, 0.01), (1.0, 0.01), (5.0, 0.015), (10.0, 0.02)])
        .build()
        .unwrap();

    (disc, hazard)
}

fn metric_value<I: Instrument>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    metric: MetricId,
) -> f64 {
    let result = instrument
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metric should compute");
    result.measures[&metric]
}

#[test]
fn test_zero_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_NOTIONAL",
        Money::new(0.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert_eq!(npv.amount(), 0.0, "Zero notional should give zero NPV");
}

#[test]
fn test_zero_spread() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_SPREAD",
        Money::new(10_000_000.0, Currency::USD),
        0.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();

    // With zero spread, buyer pays nothing but receives protection
    // NPV should be positive (value of protection)
    assert!(
        npv.amount() > 0.0,
        "Zero spread CDS should have positive NPV for buyer"
    );
}

#[test]
fn test_negative_spread() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "NEG_SPREAD",
        Money::new(10_000_000.0, Currency::USD),
        -50.0, // Negative spread
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // Should not panic with negative spread
    let npv = cds.value(&market, as_of);
    assert!(npv.is_ok(), "Negative spread should be handled");

    // Premium leg PV should reflect the negative spread (i.e., be negative).
    let prem_pv = metric_value(&cds, &market, as_of, MetricId::PremiumLegPv);
    assert!(
        prem_pv < 0.0,
        "Negative spread should produce negative premium-leg PV"
    );
}

#[test]
fn test_very_high_spread() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "HIGH_SPREAD",
        Money::new(10_000_000.0, Currency::USD),
        10000.0, // 10000 bps = 100%
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert!(
        npv.amount().is_finite(),
        "Very high spread should not cause numerical issues"
    );
}

#[test]
fn test_zero_recovery_rate() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.0) // Zero recovery
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_RECOVERY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.0;

    let npv = cds.value(&market, as_of).unwrap();
    assert!(npv.amount().is_finite(), "Zero recovery should be handled");
}

#[test]
fn test_full_recovery_rate_is_rejected() {
    // R = 1.0 means LGD = 0, which makes the protection-leg integrand vanish
    // identically and breaks hazard-curve bootstrapping. We reject it at
    // pricing time rather than silently producing meaningless zero PVs.
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "FULL_RECOVERY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 1.0;

    // Pricing must surface a structured error rather than silently degenerate.
    let registry = finstack_quant_valuations::pricer::standard_registry();
    let err = registry
        .price_with_metrics(
            &cds,
            finstack_quant_valuations::pricer::ModelKey::HazardRate,
            &market,
            as_of,
            &[MetricId::ProtectionLegPv],
            Default::default(),
        )
        .expect_err("R=1.0 must be rejected at pricing time");
    let msg = format!("{err}");
    assert!(
        msg.contains("Recovery rate") && msg.contains("[0.0, 1.0)"),
        "expected recovery-rate validation error, got: {msg}"
    );
}

#[test]
fn test_very_short_tenor() {
    // 1-day CDS
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2024 - 01 - 02);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ONE_DAY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of);
    assert!(npv.is_ok(), "Very short tenor should be handled");
}

#[test]
fn test_maturity_equals_valuation_date() {
    let as_of = date!(2024 - 01 - 01);
    let end = as_of; // Maturity = valuation date
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "EXPIRED",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of);

    // CDS with maturity = valuation date may error (no future cashflows) or have zero value
    if let Ok(value) = npv {
        // Expired CDS at maturity should have near-zero value (no future cashflows)
        assert!(
            value.amount().abs() < 100.0,
            "Expired CDS should have near-zero value, got {}",
            value.amount()
        );
    } else {
        // It's acceptable for expired CDS to return an error
        assert!(npv.is_err(), "Expired CDS valuation may error");
    }
}

#[test]
fn test_valuation_after_maturity() {
    let start = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);
    let as_of = date!(2026 - 01 - 01); // After maturity
    let (disc, hazard) = build_curves(start);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "PAST_MATURITY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of);

    // Valuation past maturity may error or return zero value
    // Both are acceptable behaviors for expired instruments
    match npv {
        Ok(value) => {
            // Past-maturity CDS should have zero value (all cashflows have expired)
            assert!(
                value.amount().abs() < 100.0,
                "Past maturity CDS should have near-zero value, got {}",
                value.amount()
            );
        }
        Err(_) => {
            // Acceptable - past maturity instruments may error
        }
    }
}

#[test]
fn test_very_high_hazard_rate() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots([(0.0, 0.5), (10.0, 0.5)]) // 50% hazard rate
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "HIGH_HAZARD",
        Money::new(10_000_000.0, Currency::USD),
        1000.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert!(
        npv.amount().is_finite(),
        "High hazard rate should not cause numerical issues"
    );
}

#[test]
fn test_zero_hazard_rate() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots([(0.0, 0.0), (10.0, 0.0)]) // Zero hazard
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_HAZARD",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // With zero hazard rate, protection leg should be zero
    let protection_pv = metric_value(&cds, &market, as_of, MetricId::ProtectionLegPv);

    assert!(
        protection_pv.abs() < 1.0,
        "Zero hazard should give near-zero protection value"
    );
}

#[test]
fn test_metrics_with_zero_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_METRICS",
        Money::new(0.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Cs01,
                MetricId::RiskyPv01,
                MetricId::ExpectedLoss,
                MetricId::JumpToDefault,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // All notional-dependent metrics should be zero
    for k in ["cs01", "risky_pv01", "expected_loss", "jump_to_default"] {
        let v = *result.measures.get(k).unwrap();
        assert!(
            v.abs() < 1e-12,
            "Expected metric {} to be 0.0 for zero notional, got {}",
            k,
            v
        );
    }
}

#[test]
fn test_par_spread_with_mismatched_curves_errors() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (_disc, _hazard) = build_curves(as_of);

    let market = MarketContext::new()
        // Intentionally insert under IDs that *do not* match the instrument's curve IDs.
        .insert(
            DiscountCurve::builder("USD_OIS_ACTUAL")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78), (10.0, 0.61)])
                .build()
                .unwrap(),
        )
        .insert(
            HazardCurve::builder("CORP_ACTUAL")
                .base_date(as_of)
                .recovery_rate(0.40)
                .knots([(0.0, 0.01), (1.0, 0.01), (5.0, 0.015), (10.0, 0.02)])
                .build()
                .unwrap(),
        );

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "MISMATCH_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS_REQUESTED",
        "CORP_REQUESTED",
    )
    .expect("CDS construction should succeed");

    // With mismatched curve IDs, valuation must error (missing curve dependency).
    let result = cds.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ParSpread],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    assert!(
        result.is_err(),
        "Expected error when instrument curve IDs are missing from the market"
    );
}

#[test]
fn test_numerical_stability_with_extreme_dates() {
    // Test with dates far in the future (30-year CDS)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2054 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "LONG_DATED",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert!(
        npv.amount().is_finite(),
        "Long-dated CDS should be numerically stable"
    );
}

#[test]
fn test_very_small_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "TINY_NOTIONAL",
        Money::new(0.01, Currency::USD), // 1 cent
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert!(npv.amount().is_finite(), "Tiny notional should be handled");
    assert!(
        npv.amount().abs() < 1.0,
        "Tiny notional should give tiny NPV"
    );
}

#[test]
fn test_very_large_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "HUGE_NOTIONAL",
        Money::new(1_000_000_000_000.0, Currency::USD), // 1 trillion
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();
    assert!(npv.amount().is_finite(), "Large notional should be handled");
}

#[test]
fn test_missing_discount_curve_error() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // Market with only hazard curve
    let hazard = HazardCurve::builder("CORP")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(hazard);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "MISSING_DISC",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS", // This curve doesn't exist
        "CORP",
    )
    .expect("CDS construction should succeed");

    let result = cds.value(&market, as_of);
    assert!(result.is_err(), "Should error with missing discount curve");
}

#[test]
fn test_missing_hazard_curve_error() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // Market with only discount curve
    let disc = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc);

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "MISSING_HAZARD",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP", // This curve doesn't exist
    )
    .expect("CDS construction should succeed");

    let result = cds.value(&market, as_of);
    assert!(result.is_err(), "Should error with missing hazard curve");
}

#[test]
fn test_settlement_delay_zero_is_valid() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "ZERO_DELAY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds.protection.settlement_delay = 0;

    let npv = cds.value(&market, as_of);
    assert!(npv.is_ok(), "Zero settlement delay should be valid");
}

#[test]
fn test_recovery_rate_bounds_not_enforced() {
    // Recovery rate is part of instrument validation and must be in [0,1].
    // This test ensures invalid recovery rates fail loudly (rather than producing NaNs).
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let (disc, hazard) = build_curves(as_of);

    let market = MarketContext::new().insert(disc).insert(hazard);

    // Test with negative recovery (invalid)
    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "NEG_RECOVERY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = -0.2;

    let result = cds.value(&market, as_of);
    assert!(
        result.is_err(),
        "Expected validation error for recovery_rate outside [0,1]"
    );
}
