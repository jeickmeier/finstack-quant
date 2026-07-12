//! Metric framework integration tests for CDS Tranche.
//!
//! Tests the integration between CDS Tranche instrument and the metric calculation framework.
//! Validates that all registered metrics can be calculated successfully via price_with_metrics().

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

fn sum_bucketed_cs01(result: &finstack_quant_valuations::results::ValuationResult) -> f64 {
    result
        .measures
        .iter()
        .filter(|(id, _)| id.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum()
}

// ==================== Individual Metric Calculation Tests ====================

#[test]
fn test_upfront_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::custom("upfront")],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Upfront calculation should succeed");
    let valuation = result.unwrap();
    let upfront = *valuation
        .measures
        .get("upfront")
        .expect("upfront should be in measures");
    assert!(upfront.is_finite(), "Upfront should be finite");
}

#[test]
fn test_spread_dv01_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::SpreadDv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Spread DV01 calculation should succeed");
    let valuation = result.unwrap();
    let spread_dv01 = *valuation
        .measures
        .get("spread_dv01")
        .expect("spread_dv01 should be in measures");
    assert!(spread_dv01.is_finite(), "Spread DV01 should be finite");
    assert!(
        spread_dv01 > 0.0,
        "Spread DV01 should be positive for sell protection"
    );
}

#[test]
fn test_correlation_delta_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Correlation01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "Correlation delta calculation should succeed"
    );
    let valuation = result.unwrap();
    let corr_delta = *valuation
        .measures
        .get("correlation01")
        .expect("correlation01 should be in measures");
    assert!(corr_delta.is_finite(), "Correlation delta should be finite");
}

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn test_cs01_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Cs01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "CS01 calculation should succeed: {:?}",
        result
    );
    let valuation = result.unwrap();
    let cs01 = *valuation
        .measures
        .get("cs01")
        .expect("cs01 should be in measures");
    assert!(cs01.is_finite(), "CS01 should be finite");
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_parallel_cs01_reconciles_with_bucketed_cs01() {
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let result = tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let parallel = *result.measures.get("cs01").unwrap();
    let bucket_total = *result.measures.get("bucketed_cs01").unwrap();
    let bucket_sum = sum_bucketed_cs01(&result);

    let total_tol = 1e-6_f64.max(1e-10 * bucket_total.abs());
    assert!(
        (bucket_sum - bucket_total).abs() <= total_tol,
        "Tranche bucketed CS01 total should equal bucket sum: total={bucket_total}, sum={bucket_sum}"
    );

    let parallel_tol = 1e-4_f64.max(5e-2 * parallel.abs());
    assert!(
        (bucket_total - parallel).abs() <= parallel_tol,
        "Tranche parallel CS01 should reconcile with bucketed CS01 under par-spread rebootstrap: bucketed={bucket_total}, parallel={parallel}, diff={}, tol={parallel_tol}",
        (bucket_total - parallel).abs()
    );
}

#[test]
fn test_par_spread_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ParSpread],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Par spread calculation should succeed");
    let valuation = result.unwrap();
    let par_spread = *valuation
        .measures
        .get("par_spread")
        .expect("par_spread should be in measures");
    assert_finite_non_negative(par_spread, "Par spread");
}

#[test]
fn test_expected_loss_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ExpectedLoss],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Expected loss calculation should succeed");
    let valuation = result.unwrap();
    let el = *valuation
        .measures
        .get("expected_loss")
        .expect("expected_loss should be in measures");
    assert_finite_non_negative(el, "Expected loss");
}

#[test]
fn test_jump_to_default_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::JumpToDefault],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Jump-to-default calculation should succeed");
    let valuation = result.unwrap();
    let jtd = *valuation
        .measures
        .get("jump_to_default")
        .expect("jump_to_default should be in measures");
    assert_finite_non_negative(jtd, "Jump-to-default");
}

#[test]
fn test_dv01_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "DV01 calculation should succeed");
    let valuation = result.unwrap();
    let dv01 = *valuation
        .measures
        .get("dv01")
        .expect("dv01 should be in measures");
    // DV01 = PV(rate+1bp) - PV(base); sign depends on instrument structure
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_theta_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Theta],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "Theta calculation should succeed: {result:?}"
    );
    let valuation = result.unwrap();
    let theta = *valuation
        .measures
        .get("theta")
        .expect("theta should be in measures");
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_bucketed_dv01_metric_via_price_with_metrics() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &[MetricId::BucketedDv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok(), "Bucketed DV01 calculation should succeed");
    let valuation = result.unwrap();
    let bucketed_dv01 = *valuation
        .measures
        .get("bucketed_dv01")
        .expect("bucketed_dv01 should be in measures");
    assert!(bucketed_dv01.is_finite(), "Bucketed DV01 should be finite");
}

// ==================== Multiple Metrics Calculation Tests ====================

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn test_calculate_multiple_metrics_simultaneously() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let metrics = vec![
        MetricId::Cs01,
        MetricId::ParSpread,
        MetricId::ExpectedLoss,
        MetricId::JumpToDefault,
    ];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "Multiple metrics calculation should succeed: {:?}",
        result
    );
    let valuation = result.unwrap();

    assert!(valuation.value.amount().is_finite(), "PV should be finite");
    assert!(
        valuation.measures.len() >= metrics.len(),
        "Should have at least all requested metrics (got {}, expected >= {})",
        valuation.measures.len(),
        metrics.len()
    );

    assert!(valuation.measures.get("cs01").is_some());
    assert!(valuation.measures.get("par_spread").is_some());
    assert!(valuation.measures.get("expected_loss").is_some());
    assert!(valuation.measures.get("jump_to_default").is_some());
}

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn test_all_standard_metrics_calculable() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let all_metrics = vec![
        MetricId::custom("upfront"),
        MetricId::SpreadDv01,
        MetricId::Correlation01,
        MetricId::Cs01,
        MetricId::ParSpread,
        MetricId::ExpectedLoss,
        MetricId::JumpToDefault,
        MetricId::Dv01,
        MetricId::Theta,
    ];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &all_metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "All metrics calculation should succeed: {:?}",
        result
    );
    let valuation = result.unwrap();

    // Verify all metrics are present and finite
    for metric_value in valuation.measures.values() {
        assert!(
            metric_value.is_finite(),
            "All metrics should produce finite values"
        );
    }
}

// ==================== Edge Case: Missing Market Data ====================

#[test]
fn test_metrics_with_missing_credit_index() {
    // Arrange
    let tranche = mezzanine_tranche();

    // Create market without credit index
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(standard_discount_curve());

    let as_of = base_date();

    // Metrics that should fallback to zero when credit index is missing
    let fallback_metrics = vec![
        MetricId::custom("upfront"),
        MetricId::custom("spread_dv01"),
        MetricId::ExpectedLoss,
        MetricId::JumpToDefault,
    ];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &fallback_metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert: Should either succeed with 0.0 values or fail gracefully
    if let Ok(valuation) = result {
        for &metric_value in valuation.measures.values() {
            assert_absolute_eq(
                metric_value,
                0.0,
                1e-10,
                "Metrics should fallback to zero when credit index missing",
            );
        }
    }
}

// ==================== Metric Calculation Order Independence ====================

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn test_metrics_order_independence() {
    // Arrange
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let metrics_order_1 = vec![MetricId::Cs01, MetricId::ParSpread, MetricId::ExpectedLoss];
    let metrics_order_2 = vec![MetricId::ExpectedLoss, MetricId::Cs01, MetricId::ParSpread];

    // Act
    let result_1 = tranche
        .price_with_metrics(
            &market,
            as_of,
            &metrics_order_1,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_2 = tranche
        .price_with_metrics(
            &market,
            as_of,
            &metrics_order_2,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert: Results should be consistent regardless of order
    let cs01_1 = *result_1.measures.get("cs01").unwrap();
    let cs01_2 = *result_2.measures.get("cs01").unwrap();
    assert_relative_eq(cs01_1, cs01_2, 1e-6, "CS01 should be order-independent");

    let par_1 = *result_1.measures.get("par_spread").unwrap();
    let par_2 = *result_2.measures.get("par_spread").unwrap();
    assert_relative_eq(par_1, par_2, 1e-6, "Par spread should be order-independent");

    let el_1 = *result_1.measures.get("expected_loss").unwrap();
    let el_2 = *result_2.measures.get("expected_loss").unwrap();
    assert_relative_eq(
        el_1,
        el_2,
        1e-6,
        "Expected loss should be order-independent",
    );
}

// ==================== Instrument Trait Integration ====================

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn test_price_with_metrics_returns_pv_and_metrics() {
    // Arrange
    let tranche: Box<dyn Instrument> = Box::new(mezzanine_tranche());
    let market = standard_market_context();
    let as_of = base_date();

    let metrics = vec![MetricId::Cs01, MetricId::ParSpread];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "price_with_metrics should succeed: {:?}",
        result
    );
    let valuation = result.unwrap();

    assert!(valuation.value.amount().is_finite(), "PV should be finite");
    assert!(
        valuation.measures.len() >= metrics.len(),
        "Should have at least all requested metrics (got {}, expected >= {})",
        valuation.measures.len(),
        metrics.len()
    );
}

#[test]
fn test_equity_tranche_metrics() {
    // Arrange
    let tranche = equity_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let metrics = vec![MetricId::ParSpread, MetricId::ExpectedLoss];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let valuation = result.unwrap();

    let par_spread = *valuation.measures.get("par_spread").unwrap();
    let expected_loss = *valuation.measures.get("expected_loss").unwrap();

    assert_finite_non_negative(par_spread, "Equity par spread");
    assert_finite_non_negative(expected_loss, "Equity expected loss");
}

#[test]
fn test_senior_tranche_metrics() {
    // Arrange
    let tranche = senior_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let metrics = vec![MetricId::ParSpread, MetricId::ExpectedLoss];

    // Act
    let result = tranche.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let valuation = result.unwrap();

    let par_spread = *valuation.measures.get("par_spread").unwrap();
    let expected_loss = *valuation.measures.get("expected_loss").unwrap();

    assert_finite_non_negative(par_spread, "Senior par spread");
    assert_finite_non_negative(expected_loss, "Senior expected loss");
}

// ==================== Metrics Scaling Tests ====================

#[test]
fn test_metrics_scale_with_notional() {
    // Arrange
    let market = standard_market_context();
    let as_of = base_date();

    // Use equity tranches because mezzanine (3-7%) has zero JTD with standard setup:
    // For 125 names, individual default loss = 1/125 × 0.6 = 0.48%, which doesn't
    // reach the 3% attachment point. Equity tranches (0-3%) always have non-zero JTD.
    let mut tranche_10mm = equity_tranche();
    tranche_10mm.notional = finstack_quant_core::money::Money::new(
        10_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let mut tranche_20mm = equity_tranche();
    tranche_20mm.notional = finstack_quant_core::money::Money::new(
        20_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let metrics = vec![MetricId::ExpectedLoss, MetricId::JumpToDefault];

    // Act
    let result_10 = tranche_10mm
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_20 = tranche_20mm
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert: Dollar metrics should scale with notional
    let el_10 = *result_10.measures.get("expected_loss").unwrap();
    let el_20 = *result_20.measures.get("expected_loss").unwrap();
    assert_relative_eq(
        el_20 / el_10,
        2.0,
        0.001,
        "Expected loss should scale with notional",
    );

    let jtd_10 = *result_10.measures.get("jump_to_default").unwrap();
    let jtd_20 = *result_20.measures.get("jump_to_default").unwrap();
    assert_relative_eq(
        jtd_20 / jtd_10,
        2.0,
        0.001,
        "JTD should scale with notional",
    );
}

/// `Recovery01` (PV sensitivity to a +1% recovery-rate bump) is registered for
/// CDS tranches but was previously unexercised. The bump is *spread-preserving*:
/// the hazard curve is recalibrated to hold par spreads (higher recovery ⇒
/// higher implied hazard, since spread ≈ hazard·(1−R)), so the net tranche
/// sensitivity is non-trivial — we assert it is finite and materially non-zero
/// for a default-exposed mezzanine tranche.
#[test]
fn test_recovery01_metric_via_price_with_metrics() {
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    let result = tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Recovery01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Recovery01 calculation should succeed");

    let recovery01 = *result
        .measures
        .get("recovery_01")
        .expect("recovery_01 should be in measures");

    assert!(
        recovery01.is_finite(),
        "Recovery01 should be finite, got {recovery01}"
    );
    assert!(
        recovery01.abs() > 1.0,
        "Recovery01 should be materially non-zero for a default-exposed tranche, got {recovery01}"
    );
}
