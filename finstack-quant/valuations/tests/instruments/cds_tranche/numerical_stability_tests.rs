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

// ==================== Economic-Invariant Helpers ====================

/// Market context whose base-correlation curve is flat at `level`, so every
/// strike prices at the same copula correlation. Flat curves are trivially
/// arbitrage-free, which isolates the correlation-monotonicity invariants
/// from base-correlation-skew effects.
fn flat_correlation_market(level: f64) -> finstack_quant_core::market_data::context::MarketContext {
    let corr_curve =
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
            "FLAT_CORR",
        )
        .knots(vec![(3.0, level), (10.0, level), (30.0, level)])
        .build()
        .unwrap();
    let index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::new(standard_hazard_curve()))
        .base_correlation_curve(std::sync::Arc::new(corr_curve))
        .build()
        .unwrap();
    standard_market_context().insert_credit_index("CDX.NA.IG.42", index)
}

/// Undiscounted maturity EL (currency) of a tranche at a flat correlation.
fn el_at_flat_corr(attach: f64, detach: f64, coupon_bp: f64, level: f64) -> f64 {
    let pricer = CDSTranchePricer::new();
    let market = flat_correlation_market(level);
    let tranche = custom_tranche(
        attach,
        detach,
        coupon_bp,
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection,
    );
    pricer.calculate_expected_loss(&tranche, &market).unwrap()
}

// ==================== Economic Invariants (2026-07 audit) ====================
//
// The extreme-correlation tests below previously asserted only
// `is_ok()`/`is_finite()`. These invariants pin the copula economics:
// equity EL decreasing in correlation, senior EL increasing, and the
// 0-100% tranche reproducing the (correlation-invariant) pool EL.

/// One-factor copula: equity-tranche expected loss `E[min(L, K)]` is
/// decreasing in the flat correlation (O'Kane 2008 Ch. 18; Vasicek).
/// Includes the extreme levels that the smoke tests exercise.
#[test]
fn test_equity_el_monotone_decreasing_in_correlation() {
    let levels = [0.001, 0.05, 0.30, 0.60, 0.95, 0.999];
    let els: Vec<f64> = levels
        .iter()
        .map(|&rho| el_at_flat_corr(0.0, 3.0, 500.0, rho))
        .collect();

    for (i, window) in els.windows(2).enumerate() {
        // Allow a small relative epsilon: near the clamped correlation
        // boundaries the integrand is step-like and 20-node quadrature can
        // break exact monotonicity by O(1e-6) relative.
        let eps = 1e-6 * window[0].abs().max(1.0);
        assert!(
            window[1] <= window[0] + eps,
            "equity EL must be non-increasing in correlation: \
             EL(rho={}) = {:.4} < EL(rho={}) = {:.4} violated",
            levels[i + 1],
            window[1],
            levels[i],
            window[0],
        );
    }
    // The move across the full correlation range must be economically
    // material, not a flat line of numerical noise.
    assert!(
        els.first().unwrap() > &(els.last().unwrap() * 1.05),
        "equity EL should drop materially from rho~0 to rho~1: {els:?}"
    );
}

/// One-factor copula: senior-tranche expected loss is increasing in the
/// flat correlation (loss mass migrates up the capital structure).
#[test]
fn test_senior_el_monotone_increasing_in_correlation() {
    let levels = [0.001, 0.05, 0.30, 0.60, 0.95, 0.999];
    let els: Vec<f64> = levels
        .iter()
        .map(|&rho| el_at_flat_corr(15.0, 30.0, 100.0, rho))
        .collect();

    for (i, window) in els.windows(2).enumerate() {
        // Same relative epsilon rationale as the equity test: quadrature on
        // the near-step integrand at clamped extreme correlations.
        let eps = 1e-6 * window[0].abs().max(1.0);
        assert!(
            window[1] >= window[0] - eps,
            "senior EL must be non-decreasing in correlation: \
             EL(rho={}) = {:.4} > EL(rho={}) = {:.4} violated",
            levels[i + 1],
            window[1],
            levels[i],
            window[0],
        );
    }
    assert!(
        els.last().unwrap() > &(els.first().unwrap() * 1.05),
        "senior EL should grow materially from rho~0 to rho~1: {els:?}"
    );
}

/// Marginal preservation: the 0-100% tranche is the whole pool, so its EL
/// must equal the analytic pool EL `Notional × PD(T) × LGD` and be
/// invariant to the copula correlation. This is the invariant that anchors
/// every tranchelet decomposition to the bootstrapped index curve.
#[test]
fn test_full_capital_structure_el_reproduces_pool_el() {
    let curve = standard_hazard_curve();
    let t_maturity = curve
        .day_count()
        .year_fraction(
            base_date(),
            maturity_5y(),
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let pool_pd = 1.0 - curve.sp(t_maturity);
    let analytic_pool_el = 10_000_000.0 * pool_pd * (1.0 - 0.40);

    for rho in [0.10, 0.50, 0.90] {
        let el = el_at_flat_corr(0.0, 100.0, 100.0, rho);
        assert_relative_eq(
            el,
            analytic_pool_el,
            5e-3,
            &format!("0-100% tranche EL vs analytic pool EL at rho={rho}"),
        );
    }
}

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
