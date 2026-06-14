//! Expected loss calculation tests for CDS Tranche.
//!
//! Tests cover:
//! - Basic expected loss calculation
//! - EL curve generation and monotonicity
//! - Homogeneous pool calculations
//! - Heterogeneous pool calculations (SPA and exact convolution)
//! - Tranche subordination effects on EL

#![allow(clippy::field_reassign_with_default)]

use super::helpers::*;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranchePricer, CDSTranchePricerConfig, HeteroMethod,
};

// ==================== Basic Expected Loss Tests ====================

#[test]
fn test_expected_loss_is_non_negative() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();

    // Act
    let el = pricer.calculate_expected_loss(&tranche, &market).unwrap();

    // Assert
    assert_finite_non_negative(el, "Expected loss");
}

#[test]
fn test_expected_loss_bounded_by_notional() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();

    // Act
    let el = pricer.calculate_expected_loss(&tranche, &market).unwrap();

    // Assert
    assert!(
        el <= tranche.notional.amount(),
        "Expected loss should not exceed tranche notional"
    );
}

#[test]
fn test_expected_loss_equity_vs_senior() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();

    let equity = equity_tranche();
    let senior = senior_tranche();

    // Act
    let el_equity = pricer.calculate_expected_loss(&equity, &market).unwrap();
    let el_senior = pricer.calculate_expected_loss(&senior, &market).unwrap();

    // Assert
    // Equity tranche takes first loss, so EL should be higher
    assert!(
        el_equity >= el_senior,
        "Equity expected loss should be >= senior expected loss"
    );
}

#[test]
fn test_expected_loss_scales_with_notional() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();

    let mut tranche_10mm = mezzanine_tranche();
    tranche_10mm.notional =
        finstack_core::money::Money::new(10_000_000.0, finstack_core::currency::Currency::USD);

    let mut tranche_20mm = mezzanine_tranche();
    tranche_20mm.notional =
        finstack_core::money::Money::new(20_000_000.0, finstack_core::currency::Currency::USD);

    // Act
    let el_10 = pricer
        .calculate_expected_loss(&tranche_10mm, &market)
        .unwrap();
    let el_20 = pricer
        .calculate_expected_loss(&tranche_20mm, &market)
        .unwrap();

    // Assert
    assert_relative_eq(
        el_20 / el_10,
        2.0,
        0.001,
        "Expected loss should scale linearly with notional",
    );
}

// Note: EL curve generation and monotonicity are internal implementation details
// tested indirectly through expected loss calculations and pricing

// ==================== Homogeneous vs Heterogeneous Tests ====================

#[test]
fn test_homogeneous_expected_loss() {
    // Arrange
    let mut config = CDSTranchePricerConfig::default();
    config.use_issuer_curves = false; // Force homogeneous
    let pricer = CDSTranchePricer::with_params(config);

    let tranche = mezzanine_tranche();
    let market = standard_market_context(); // No issuer curves

    // Act
    let result = pricer.calculate_expected_loss(&tranche, &market);

    // Assert
    assert!(result.is_ok());
    assert_finite_non_negative(result.unwrap(), "Homogeneous EL");
}

#[test]
fn test_heterogeneous_spa_expected_loss() {
    // Arrange
    let mut config = CDSTranchePricerConfig::default();
    config.use_issuer_curves = true;
    config.hetero_method = HeteroMethod::Spa;
    let pricer = CDSTranchePricer::with_params(config);

    let tranche = mezzanine_tranche();
    let market = market_context_with_issuers(50);

    // Act
    let result = pricer.calculate_expected_loss(&tranche, &market);

    // Assert
    assert!(result.is_ok());
    assert_finite_non_negative(result.unwrap(), "Heterogeneous SPA EL");
}

#[test]
fn test_heterogeneous_exact_convolution_expected_loss() {
    // Arrange
    let mut config = CDSTranchePricerConfig::default();
    config.use_issuer_curves = true;
    config.hetero_method = HeteroMethod::ExactConvolution;
    config.grid_step = 0.002;
    let pricer = CDSTranchePricer::with_params(config);

    let tranche = mezzanine_tranche();
    let market = market_context_with_issuers(10); // Small pool for exact method

    // Act
    let result = pricer.calculate_expected_loss(&tranche, &market);

    // Assert
    assert!(result.is_ok());
    assert_finite_non_negative(result.unwrap(), "Heterogeneous exact convolution EL");
}

#[test]
fn test_hetero_spa_matches_homogeneous_when_issuers_identical() {
    // Arrange
    let base_market = standard_market_context();
    let index_data = base_market.get_credit_index("CDX.NA.IG.42").unwrap();

    // Build heterogeneous market with identical issuer curves
    let mut issuer_curves = finstack_core::HashMap::default();
    for i in 0..10 {
        let id = format!("ISSUER-{:03}", i + 1);
        issuer_curves.insert(id, std::sync::Arc::clone(&index_data.index_credit_curve));
    }

    let hetero_index = finstack_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(10)
        .recovery_rate(index_data.recovery_rate)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
        .issuer_curves(issuer_curves)
        .build()
        .unwrap();

    let hetero_market = base_market.insert_credit_index("CDX.NA.IG.42", hetero_index);

    let mut homo_config = CDSTranchePricerConfig::default();
    homo_config.use_issuer_curves = false;
    let homo_pricer = CDSTranchePricer::with_params(homo_config);

    let mut hetero_config = CDSTranchePricerConfig::default();
    hetero_config.use_issuer_curves = true;
    hetero_config.hetero_method = HeteroMethod::Spa;
    let hetero_pricer = CDSTranchePricer::with_params(hetero_config);

    let tranche = custom_tranche(
        3.0,
        7.0,
        0.0,
        finstack_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection,
    );

    // Act
    let el_homo = homo_pricer
        .calculate_expected_loss(&tranche, &hetero_market)
        .unwrap();
    let el_hetero = hetero_pricer
        .calculate_expected_loss(&tranche, &hetero_market)
        .unwrap();

    // Assert
    assert_relative_eq(
        el_hetero,
        el_homo,
        0.0001,
        "Hetero SPA should match homogeneous when issuers identical",
    );
}

#[test]
fn test_hetero_spa_vs_exact_convolution_small_pool() {
    // Arrange
    let market = market_context_with_issuers(8);

    let mut spa_config = CDSTranchePricerConfig::default();
    spa_config.use_issuer_curves = true;
    spa_config.hetero_method = HeteroMethod::Spa;
    let spa_pricer = CDSTranchePricer::with_params(spa_config);

    let mut exact_config = CDSTranchePricerConfig::default();
    exact_config.use_issuer_curves = true;
    exact_config.hetero_method = HeteroMethod::ExactConvolution;
    exact_config.grid_step = 0.002;
    let exact_pricer = CDSTranchePricer::with_params(exact_config);

    let tranche = mezzanine_tranche();

    // Act
    let el_spa = spa_pricer
        .calculate_expected_loss(&tranche, &market)
        .unwrap();
    let el_exact = exact_pricer
        .calculate_expected_loss(&tranche, &market)
        .unwrap();

    // Assert: SPA and exact should be close for small pools
    assert_relative_eq(
        el_spa,
        el_exact,
        0.05,
        "SPA and exact convolution should be close for small pools",
    );
}

#[test]
fn test_exact_convolution_grid_refinement() {
    // Arrange
    let market = market_context_with_issuers(10);

    let mut coarse_config = CDSTranchePricerConfig::default();
    coarse_config.use_issuer_curves = true;
    coarse_config.hetero_method = HeteroMethod::ExactConvolution;
    coarse_config.grid_step = 0.005;
    let coarse_pricer = CDSTranchePricer::with_params(coarse_config);

    let mut fine_config = CDSTranchePricerConfig::default();
    fine_config.use_issuer_curves = true;
    fine_config.hetero_method = HeteroMethod::ExactConvolution;
    fine_config.grid_step = 0.001;
    let fine_pricer = CDSTranchePricer::with_params(fine_config);

    let tranche = equity_tranche();

    // Act
    let el_coarse = coarse_pricer
        .calculate_expected_loss(&tranche, &market)
        .unwrap();
    let el_fine = fine_pricer
        .calculate_expected_loss(&tranche, &market)
        .unwrap();

    // Assert: Finer grid should converge to similar result
    assert_relative_eq(
        el_coarse,
        el_fine,
        0.02,
        "Grid refinement should converge to similar EL",
    );
}

// ==================== Tranche Subordination Tests ====================

#[test]
fn test_el_ordering_by_subordination() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();

    let equity = equity_tranche();
    let mezzanine = mezzanine_tranche();
    let senior = senior_tranche();

    // Act
    let el_equity = pricer.calculate_expected_loss(&equity, &market).unwrap();
    let el_mezz = pricer.calculate_expected_loss(&mezzanine, &market).unwrap();
    let el_senior = pricer.calculate_expected_loss(&senior, &market).unwrap();

    // Assert: EL should decrease with seniority
    // (as fraction of tranche notional, equity loses most)
    let el_frac_equity = el_equity / equity.notional.amount();
    let el_frac_mezz = el_mezz / mezzanine.notional.amount();
    let el_frac_senior = el_senior / senior.notional.amount();

    // Note: With statrs providing more accurate normal distribution functions,
    // very small numerical differences can occur. Use epsilon comparison for near-equal values.
    const EPSILON: f64 = 1e-10;

    assert!(
        el_frac_equity >= el_frac_mezz - EPSILON,
        "Equity EL fraction should be >= mezzanine: equity={}, mezz={}, diff={}",
        el_frac_equity,
        el_frac_mezz,
        el_frac_equity - el_frac_mezz
    );
    assert!(
        el_frac_mezz >= el_frac_senior - EPSILON,
        "Mezzanine EL fraction should be >= senior: mezz={}, senior={}, diff={}",
        el_frac_mezz,
        el_frac_senior,
        el_frac_mezz - el_frac_senior
    );
}

#[test]
fn test_super_senior_low_expected_loss() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();

    let super_senior = custom_tranche(
        15.0,
        30.0,
        50.0,
        finstack_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection,
    );

    // Act
    let el = pricer
        .calculate_expected_loss(&super_senior, &market)
        .unwrap();
    let el_fraction = el / super_senior.notional.amount();

    // Assert: Super senior should have very low expected loss
    assert!(
        el_fraction < 0.05,
        "Super senior EL fraction should be < 5%, got {}",
        el_fraction
    );
}

// ==================== Edge Cases ====================

#[test]
#[should_panic(expected = "attach_pct (5) must be less than detach_pct (5)")]
fn test_zero_width_tranche_expected_loss() {
    // Degenerate tranche: 5-5% should be rejected by validation
    // attach_pct must be strictly less than detach_pct
    let _zero_width = custom_tranche(
        5.0,
        5.0,
        100.0,
        finstack_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection,
    );
}

#[test]
fn test_full_portfolio_tranche_expected_loss() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();

    // Full portfolio: 0-100%
    let full_portfolio = custom_tranche(
        0.0,
        100.0,
        100.0,
        finstack_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection,
    );

    // Act
    let el = pricer
        .calculate_expected_loss(&full_portfolio, &market)
        .unwrap();

    // Assert: Should capture all portfolio expected loss
    assert_finite_non_negative(el, "Full portfolio EL");
    assert!(
        el <= full_portfolio.notional.amount(),
        "EL should not exceed notional"
    );
}
