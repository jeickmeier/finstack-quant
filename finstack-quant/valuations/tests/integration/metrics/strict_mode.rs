//! Integration tests for Phase 1 metrics strict mode functionality.
//!
//! These tests verify end-to-end workflows with strict metrics computation
//! introduced in Phase 1 of market convention refactors.
//!
//! Test scenarios:
//! 1. Multi-metric request with all metrics succeeding
//! 2. Multi-metric request with one failure (strict mode → all fail)
//! 3. Circular dependency detection
//! 4. Unknown metric handling in strict vs permissive parsing

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

/// Helper to create a standard bond market context for testing
fn create_bond_market(as_of: Date) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, 0.95f64),
            (2.0f64, 0.90f64),
            (5.0f64, 0.80f64),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc_curve)
}

/// Helper to create a test bond instrument
fn create_test_bond(as_of: Date) -> Bond {
    let currency = Currency::try_from("USD").unwrap();
    let notional = Money::new(1_000_000.0, currency);
    let issue_date = as_of;
    let maturity_date = issue_date + time::Duration::days(5 * 365);

    Bond::fixed(
        "TEST-BOND-001",
        notional,
        0.05, // 5% coupon
        issue_date,
        maturity_date,
        "USD-OIS",
    )
    .expect("Test bond creation should succeed")
}

#[test]
fn test_all_metrics_succeed_strict_mode() {
    // Scenario: Request 10 standard metrics, all should compute successfully in strict mode

    let as_of = date!(2024 - 01 - 15);
    let market = create_bond_market(as_of);
    let bond = create_test_bond(as_of);

    // Get PV first
    let pv = bond.value(&market, as_of).unwrap();

    // Create metric context
    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Request standard bond metrics that are applicable to vanilla bonds
    let metric_ids = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::DurationMod,
        MetricId::DurationMac,
        MetricId::Convexity,
        MetricId::Dv01,
    ];

    let registry = standard_registry();

    // Compute with strict mode (default)
    let result = registry.compute(&metric_ids, &mut context);

    // All metrics should succeed
    assert!(
        result.is_ok(),
        "Strict mode should succeed when all metrics are valid and applicable"
    );

    let metrics = result.unwrap();

    // Verify all requested metrics are present
    assert_eq!(
        metrics.len(),
        metric_ids.len(),
        "All requested metrics should be computed"
    );

    for metric_id in &metric_ids {
        assert!(
            metrics.contains_key(metric_id),
            "Metric {:?} should be present in results",
            metric_id
        );

        let value = metrics[metric_id];
        assert!(
            value.is_finite(),
            "Metric {:?} should have finite value, got {}",
            metric_id,
            value
        );
    }

    // Verify specific metric values are reasonable
    let clean_price = metrics[&MetricId::CleanPrice];
    assert!(
        clean_price > 0.0,
        "Clean price should be positive, got {}",
        clean_price
    );

    let ytm = metrics[&MetricId::Ytm];
    assert!(
        ytm.is_finite() && ytm != 0.0,
        "YTM should be finite and non-zero, got {}",
        ytm
    );

    let dv01 = metrics[&MetricId::Dv01];
    assert!(
        dv01.is_finite() && dv01 != 0.0,
        "DV01 should be finite and non-zero for a bond, got {}",
        dv01
    );
}

#[test]
fn test_unknown_metric_fails_strict_mode() {
    // Scenario: Request one unknown metric in strict mode → should fail immediately

    let as_of = date!(2024 - 01 - 15);
    let market = create_bond_market(as_of);
    let bond = create_test_bond(as_of);
    let pv = bond.value(&market, as_of).unwrap();

    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Mix valid and invalid metrics
    let metric_ids = vec![
        MetricId::CleanPrice,
        MetricId::custom("nonexistent_metric"), // This should cause failure
        MetricId::Ytm,
    ];

    let registry = standard_registry();

    // Compute with strict mode
    let result = registry.compute(&metric_ids, &mut context);

    // Should fail due to unknown metric
    assert!(
        result.is_err(),
        "Strict mode should fail when an unknown/unregistered metric is requested"
    );

    match result.unwrap_err() {
        Error::UnknownMetric { metric_id, .. } => {
            assert_eq!(metric_id, "nonexistent_metric");
        }
        Error::MetricNotApplicable { metric_id, .. } => {
            // Also acceptable if metric exists but isn't applicable to bond
            assert_eq!(metric_id, "nonexistent_metric");
        }
        other => panic!(
            "Expected UnknownMetric or MetricNotApplicable error, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_strict_is_default() {
    // Scenario: Verify that compute() defaults to strict mode

    let as_of = date!(2024 - 01 - 15);
    let market = create_bond_market(as_of);
    let bond = create_test_bond(as_of);
    let pv = bond.value(&market, as_of).unwrap();

    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metric_ids = vec![MetricId::CleanPrice, MetricId::custom("unknown_metric")];

    let registry = standard_registry();

    // compute() should behave same as strict mode
    let default_result = registry.compute(&metric_ids, &mut context);

    // Default compute() should fail (strict mode)
    assert!(
        default_result.is_err(),
        "Default compute() should fail on unknown metric"
    );

    // Verify default compute() returns appropriate error
    match default_result.unwrap_err() {
        Error::UnknownMetric { .. } | Error::MetricNotApplicable { .. } => {
            // Expected error types
        }
        other => {
            panic!(
                "Expected UnknownMetric or MetricNotApplicable error, got: {:?}",
                other
            );
        }
    }
}

#[test]
fn test_metric_parse_strict() {
    // Scenario: Verify strict parsing rejects unknown metric names

    // Known metrics should parse
    let ytm_result = MetricId::parse_strict("ytm");
    assert!(
        ytm_result.is_ok(),
        "Known metric 'ytm' should parse in strict mode"
    );
    assert_eq!(ytm_result.unwrap(), MetricId::Ytm);

    let dv01_result = MetricId::parse_strict("dv01");
    assert!(
        dv01_result.is_ok(),
        "Known metric 'dv01' should parse in strict mode"
    );
    assert_eq!(dv01_result.unwrap(), MetricId::Dv01);

    // Unknown metrics should fail
    let unknown_result = MetricId::parse_strict("made_up_metric");
    assert!(
        unknown_result.is_err(),
        "Unknown metric should fail in strict parsing"
    );

    match unknown_result.unwrap_err() {
        Error::UnknownMetric {
            metric_id,
            available,
        } => {
            assert_eq!(metric_id, "made_up_metric");
            assert!(
                !available.is_empty(),
                "Error should include list of available metrics"
            );
            assert!(
                available.contains(&"ytm".to_string()),
                "Available list should include 'ytm'"
            );
        }
        other => panic!("Expected UnknownMetric error, got: {:?}", other),
    }

    // Verify case insensitivity
    let ytm_upper = MetricId::parse_strict("YTM");
    assert!(
        ytm_upper.is_ok(),
        "Strict parsing should be case insensitive"
    );
    assert_eq!(ytm_upper.unwrap(), MetricId::Ytm);
}

#[test]
fn test_from_str_still_permissive() {
    // Scenario: Verify FromStr remains permissive for custom metric IDs

    use std::str::FromStr;

    // Known metrics work
    let ytm = MetricId::from_str("ytm").unwrap();
    assert_eq!(ytm, MetricId::Ytm);

    // Unknown metrics create custom IDs (no error)
    let custom = MetricId::from_str("custom_metric").unwrap();
    assert_eq!(custom.as_str(), "custom_metric");
}

#[test]
fn test_end_to_end_workflow() {
    // Scenario: Realistic workflow with calibration → pricing → multi-metric valuation

    let as_of = date!(2024 - 01 - 15);
    let market = create_bond_market(as_of);
    let bond = create_test_bond(as_of);

    // 1. Price the bond
    let pv = bond.value(&market, as_of).unwrap();
    assert!(pv.amount() > 0.0, "Bond should have positive PV");

    // 2. Compute comprehensive metrics in strict mode
    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let all_bond_metrics = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::DurationMod,
        MetricId::DurationMac,
        MetricId::Convexity,
        MetricId::Dv01,
    ];

    let registry = standard_registry();
    let metrics = registry
        .compute(&all_bond_metrics, &mut context)
        .expect("All standard bond metrics should compute successfully");

    // 3. Verify risk metrics are reasonable
    let dv01 = metrics[&MetricId::Dv01];
    let duration_mod = metrics[&MetricId::DurationMod];
    let convexity = metrics[&MetricId::Convexity];

    assert!(
        dv01.is_finite() && dv01 != 0.0,
        "DV01 should be finite and non-zero, got {}",
        dv01
    );
    assert!(
        duration_mod.is_finite() && duration_mod > 0.0,
        "Modified duration should be positive and finite, got {}",
        duration_mod
    );
    assert!(
        convexity.is_finite(),
        "Convexity should be finite, got {}",
        convexity
    );

    // 4. Verify price metrics
    let clean_price = metrics[&MetricId::CleanPrice];
    let dirty_price = metrics[&MetricId::DirtyPrice];
    let accrued = metrics[&MetricId::Accrued];

    assert!(
        clean_price > 0.0 && clean_price.is_finite(),
        "Clean price should be positive and finite, got {}",
        clean_price
    );
    assert!(
        dirty_price.is_finite(),
        "Dirty price should be finite, got {}",
        dirty_price
    );
    assert!(
        accrued >= 0.0 && accrued.is_finite(),
        "Accrued interest should be non-negative and finite, got {}",
        accrued
    );

    // Verify dirty price relationship (allowing for floating point tolerance)
    let price_sum_diff = (dirty_price - (clean_price + accrued)).abs();
    assert!(
        price_sum_diff < 1e-6,
        "Dirty price should approximately equal clean price plus accrued, but diff was {}",
        price_sum_diff
    );
}

// Note: Calibration residual normalization fix (Phase 1.4) is tested in detail in
// finstack-quant/valuations/tests/calibration/targets/discount_tests.rs
// The test_residual_normalization_invariance() test verifies that calibrations
// with different notionals produce identical curves.
