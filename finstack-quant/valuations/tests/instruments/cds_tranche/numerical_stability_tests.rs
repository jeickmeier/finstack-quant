//! Numerical stability tests for CDS Tranche pricer.
//!
//! Tests cover:
//! - Extreme correlation values (near 0 and 1)
//! - Extreme market factor values
//! - Recovery rate edge cases
//! - Portfolio size edge cases
//!
//! Note: Internal numerical methods (conditional default probabilities,
//! correlation smoothing, etc.) are tested indirectly through end-to-end
//! pricing tests with extreme scenarios.

use super::helpers::*;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer;

// ==================== Extreme Correlation Tests ====================

#[test]
fn test_extreme_low_correlation_pricing() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Test with very low correlation
    let low_corr_curve =
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
            "TEST_LOW_CORR",
        )
        .knots(vec![(3.0, 0.001), (7.0, 0.001), (10.0, 0.001)])
        .build()
        .unwrap();

    let test_index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::new(low_corr_curve))
        .build()
        .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", test_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle very low correlation");
    let pv = result.unwrap();
    assert!(pv.amount().is_finite(), "PV should be finite");
}

#[test]
fn test_extreme_high_correlation_pricing() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Test with very high correlation
    let high_corr_curve =
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
            "TEST_HIGH_CORR",
        )
        .knots(vec![(3.0, 0.999), (7.0, 0.999), (10.0, 0.999)])
        .build()
        .unwrap();

    let test_index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::new(high_corr_curve))
        .build()
        .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", test_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle very high correlation");
    let pv = result.unwrap();
    assert!(pv.amount().is_finite(), "PV should be finite");
}

// ==================== Extreme Market Scenarios Tests ====================

#[test]
fn test_pricing_with_zero_recovery_rate() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Create index with zero recovery
    let zero_recovery_index =
        finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.0)
            .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
            .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
            .build()
            .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", zero_recovery_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle zero recovery rate");
    let pv = result.unwrap();
    assert!(
        pv.amount().is_finite(),
        "PV should be finite with zero recovery"
    );
}

#[test]
fn test_pricing_with_high_recovery_rate() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Create index with very high recovery
    let high_recovery_index =
        finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.90)
            .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
            .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
            .build()
            .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", high_recovery_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle high recovery rate");
    let pv = result.unwrap();
    assert!(
        pv.amount().is_finite(),
        "PV should be finite with high recovery"
    );
}

// ==================== Very Large/Small Default Probabilities ====================

#[test]
fn test_pricing_with_near_zero_default_probability() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();

    // Create hazard curve with very low hazard rates
    let low_hazard_curve =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("LOW_HAZARD")
            .base_date(base_date())
            .recovery_rate(0.40)
            .knots(vec![(1.0, 0.0001), (5.0, 0.0002), (10.0, 0.0003)])
            .build()
            .unwrap();

    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();
    let low_hazard_index =
        finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(std::sync::Arc::new(low_hazard_curve))
            .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
            .build()
            .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", low_hazard_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(
        result.is_ok(),
        "Should handle near-zero default probability"
    );
    let pv = result.unwrap();
    assert!(pv.amount().is_finite(), "PV should be finite");
}

// Note: Adaptive integration for extreme correlations is tested indirectly
// through the extreme correlation pricing tests above

// ==================== Portfolio Size Edge Cases ====================

#[test]
fn test_very_small_portfolio() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Create index with only 5 constituents
    let small_index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(5)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
        .build()
        .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", small_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle small portfolio");
    let pv = result.unwrap();
    assert!(pv.amount().is_finite());
}

#[test]
fn test_very_large_portfolio() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Create index with 500 constituents
    let large_index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(500)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
        .build()
        .unwrap();

    let market = base_market.insert_credit_index("CDX.NA.IG.42", large_index);

    let tranche = mezzanine_tranche();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Should handle large portfolio");
    let pv = result.unwrap();
    assert!(pv.amount().is_finite());
}

// Note: CDF overflow protection is tested indirectly through extreme correlation
// and extreme recovery rate pricing tests which exercise the full calculation path
