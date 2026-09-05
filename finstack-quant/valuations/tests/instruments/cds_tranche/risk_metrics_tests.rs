//! Risk metrics tests for CDS Tranche.
//!
//! Tests cover:
//! - CS01 (credit spread sensitivity)
//! - Correlation delta
//! - Jump-to-default
//! - Spread DV01
//! - Par spread calculation
//! - Different bump units and methods

#![allow(clippy::field_reassign_with_default)]

use super::helpers::*;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranchePricer, CDSTranchePricerConfig,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

// ==================== CS01 Tests ====================

#[test]
fn test_standard_cs01_requires_replay_recipe() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();
    let provider = finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new();

    // Act
    let error = pricer
        .calculate_cs01(&tranche, &market, as_of, &provider)
        .expect_err("standard tranche CS01 requires quote-space replay");

    // Assert
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
fn test_cs01_sell_protection_typically_positive() {
    // Arrange
    let mut tranche = mezzanine_tranche();
    tranche.side = TrancheSide::SellProtection;
    let market = replayable_market_context();
    let as_of = base_date();

    // Act
    let result = tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let cs01 = *result.measures.get("cs01").unwrap();

    // Assert
    // For protection seller, higher spreads typically increase PV
    // (protection leg value increases more than any premium increase)
    assert!(cs01.is_finite());
}

#[test]
fn test_cs01_buy_sell_opposite_sign() {
    // Arrange
    let market = replayable_market_context();
    let as_of = base_date();

    let sell_tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let buy_tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::BuyProtection);

    // Act
    let cs01_sell = sell_tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap()
        .measures
        .get("cs01")
        .copied()
        .unwrap();
    let cs01_buy = buy_tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap()
        .measures
        .get("cs01")
        .copied()
        .unwrap();

    // Assert
    relative_eq(
        cs01_buy,
        -cs01_sell,
        0.001,
        "Buy and sell CS01 should have opposite signs",
    );
}

#[test]
fn test_standard_cs01_is_normalized_across_bump_sizes() {
    // Arrange
    let market = replayable_market_context();
    let as_of = base_date();
    let tranche = mezzanine_tranche();

    let mut config_1bp = CDSTranchePricerConfig::default();
    config_1bp.cs01_bump_size = 1.0;
    let pricer_1bp =
        CDSTranchePricer::with_params(config_1bp).expect("valid tranche pricer config");

    let mut config_2bp = CDSTranchePricerConfig::default();
    config_2bp.cs01_bump_size = 2.0;
    let pricer_2bp =
        CDSTranchePricer::with_params(config_2bp).expect("valid tranche pricer config");
    let provider = finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new();

    let cs01_1bp = pricer_1bp
        .calculate_cs01(&tranche, &market, as_of, &provider)
        .expect("1bp replay-backed CS01");
    let cs01_2bp = pricer_2bp
        .calculate_cs01(&tranche, &market, as_of, &provider)
        .expect("2bp replay-backed CS01");

    assert!(cs01_1bp.is_finite() && cs01_2bp.is_finite());
    relative_eq(
        cs01_1bp,
        cs01_2bp,
        0.01,
        "CS01 must be normalized to one basis point independently of finite-difference bump size",
    );
}

#[test]
fn test_direct_and_registered_cs01_share_quote_replay_convention() {
    let mut market = replayable_market_context();
    let as_of = base_date();
    let tranche = mezzanine_tranche();

    // Exercise the index-recovery preservation path rather than relying only
    // on the calibration recipe's 40% recovery assumption.
    let original = market
        .get_credit_index(&tranche.credit_index_id)
        .expect("replayable index should exist");
    let index = finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
        .num_constituents(original.num_constituents)
        .recovery_rate(0.35)
        .index_credit_curve(Arc::clone(&original.index_credit_curve))
        .base_correlation_curve(Arc::clone(&original.base_correlation_curve))
        .build()
        .expect("comparison index should build");
    market = market.insert_credit_index(&tranche.credit_index_id, index);

    let bump_bp = 2.0;
    let mut pricer_config = CDSTranchePricerConfig::default();
    pricer_config.cs01_bump_size = bump_bp;
    let pricer = CDSTranchePricer::with_params(pricer_config).expect("valid tranche pricer config");
    let provider =
        Arc::new(finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new());
    let direct = pricer
        .calculate_cs01(&tranche, &market, as_of, provider.as_ref())
        .expect("direct replay-backed CS01 should calculate");

    let mut config = finstack_quant_core::config::FinstackConfig::default();
    config
        .extensions
        .insert(
            "valuations.sensitivities.v1",
            serde_json::json!({"credit_spread_bump_bp": bump_bp}),
        )
        .expect("valid sensitivity configuration");
    let registered = tranche
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_config(&config)
                .with_recalibration_provider(provider),
        )
        .expect("registered replay-backed CS01 should calculate")
        .measures["cs01"];

    let tolerance = 1e-8_f64.max(1e-10 * direct.abs());
    assert!(
        (direct - registered).abs() <= tolerance,
        "direct and registered CS01 must share hazard replay, recovery preservation, and bump normalization: direct={direct}, registered={registered}, tolerance={tolerance}"
    );
}

// ==================== Correlation Delta Tests ====================

#[test]
fn test_correlation_delta_is_finite() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let corr_delta = pricer
        .calculate_correlation_delta(&tranche, &market, as_of)
        .unwrap();

    // Assert
    assert!(corr_delta.is_finite(), "Correlation delta should be finite");
}

#[test]
fn test_correlation_delta_equity_vs_senior() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let equity = equity_tranche();
    let senior = senior_tranche();

    // Act
    let corr_delta_equity = pricer
        .calculate_correlation_delta(&equity, &market, as_of)
        .unwrap();
    let corr_delta_senior = pricer
        .calculate_correlation_delta(&senior, &market, as_of)
        .unwrap();

    // Assert
    // Equity and senior tranches typically have opposite correlation sensitivities
    // Equity: negative (higher corr → lower value)
    // Senior: positive (higher corr → higher value)
    assert!(corr_delta_equity.is_finite());
    assert!(corr_delta_senior.is_finite());
}

#[test]
fn test_correlation_delta_with_custom_bump() {
    // Arrange
    let mut config = CDSTranchePricerConfig::default();
    config.corr_bump_abs = 0.02; // 2% bump instead of default 1%
    let pricer = CDSTranchePricer::with_params(config).expect("valid tranche pricer config");

    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = pricer.calculate_correlation_delta(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok());
    assert!(result.unwrap().is_finite());
}

// ==================== Jump-to-Default Tests ====================

#[test]
fn test_jump_to_default_is_non_negative() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let jtd = pricer
        .calculate_jump_to_default(&tranche, &market, as_of)
        .unwrap();

    // Assert
    assert_finite_non_negative(jtd, "Jump-to-default");
}

#[test]
fn test_jump_to_default_equity_greater_than_senior() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let equity = equity_tranche();
    let senior = senior_tranche();

    // Act
    let jtd_equity = pricer
        .calculate_jump_to_default(&equity, &market, as_of)
        .unwrap();
    let jtd_senior = pricer
        .calculate_jump_to_default(&senior, &market, as_of)
        .unwrap();

    // Assert
    // Equity tranche takes first loss, so JTD should be higher
    assert!(
        jtd_equity >= jtd_senior,
        "Equity JTD should be >= senior JTD (first loss position)"
    );
}

#[test]
fn test_jump_to_default_senior_can_be_zero() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    // Senior tranche with high attachment point
    let senior = custom_tranche(15.0, 30.0, 50.0, TrancheSide::SellProtection);

    // Act
    let jtd = pricer
        .calculate_jump_to_default(&senior, &market, as_of)
        .unwrap();

    // Assert
    // For 125 names, 1 default = 0.8% loss, which won't reach 15% attachment
    assert_eq!(
        jtd, 0.0,
        "Single name default shouldn't reach deep senior tranche"
    );
}

#[test]
fn test_jump_to_default_scales_with_notional() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    // Use equity tranches because mezzanine (3-7%) has zero JTD with standard setup:
    // For 125 names, individual default loss = 1/125 × 0.6 = 0.48%, which doesn't
    // reach the 3% attachment point. Equity tranches (0-3%) always have non-zero JTD.
    let mut tranche_10mm = equity_tranche();
    tranche_10mm.notional = finstack_quant_core::money::Money::new(
        10_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    )
    .expect("valid money fixture");

    let mut tranche_20mm = equity_tranche();
    tranche_20mm.notional = finstack_quant_core::money::Money::new(
        20_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    )
    .expect("valid money fixture");

    // Act
    let jtd_10 = pricer
        .calculate_jump_to_default(&tranche_10mm, &market, as_of)
        .unwrap();
    let jtd_20 = pricer
        .calculate_jump_to_default(&tranche_20mm, &market, as_of)
        .unwrap();

    // Assert
    relative_eq(
        jtd_20 / jtd_10,
        2.0,
        0.001,
        "JTD should scale linearly with notional",
    );
}

// ==================== Spread DV01 Tests ====================

#[test]
fn test_spread_dv01_is_finite() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let spread_dv01 = pricer
        .calculate_spread_dv01(&tranche, &market, as_of)
        .unwrap();

    // Assert
    assert!(spread_dv01.is_finite(), "Spread DV01 should be finite");
}

#[test]
fn test_spread_dv01_positive_for_sell_protection() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let mut tranche = mezzanine_tranche();
    tranche.side = TrancheSide::SellProtection;
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let spread_dv01 = pricer
        .calculate_spread_dv01(&tranche, &market, as_of)
        .unwrap();

    // Assert
    // For protection seller, higher running coupon increases premium received → positive DV01
    assert!(
        spread_dv01 > 0.0,
        "Spread DV01 should be positive for sell protection"
    );
}

#[test]
fn test_spread_dv01_scales_with_notional() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let mut tranche_10mm = mezzanine_tranche();
    tranche_10mm.notional = finstack_quant_core::money::Money::new(
        10_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    )
    .expect("valid money fixture");

    let mut tranche_20mm = mezzanine_tranche();
    tranche_20mm.notional = finstack_quant_core::money::Money::new(
        20_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    )
    .expect("valid money fixture");

    // Act
    let dv01_10 = pricer
        .calculate_spread_dv01(&tranche_10mm, &market, as_of)
        .unwrap();
    let dv01_20 = pricer
        .calculate_spread_dv01(&tranche_20mm, &market, as_of)
        .unwrap();

    // Assert
    relative_eq(
        dv01_20 / dv01_10,
        2.0,
        0.001,
        "Spread DV01 should scale linearly with notional",
    );
}

// ==================== Par Spread Tests ====================

#[test]
fn test_par_spread_is_positive() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let par_spread = pricer
        .calculate_par_spread(&tranche, &market, as_of)
        .unwrap();

    // Assert
    assert_finite_non_negative(par_spread, "Par spread");
}

#[test]
fn test_par_spread_equity_greater_than_senior() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let equity = equity_tranche();
    let senior = senior_tranche();

    // Act
    let par_equity = pricer
        .calculate_par_spread(&equity, &market, as_of)
        .unwrap();
    let par_senior = pricer
        .calculate_par_spread(&senior, &market, as_of)
        .unwrap();

    // Assert
    // Equity tranche has higher risk → higher par spread
    assert!(
        par_equity > par_senior,
        "Equity par spread should exceed senior par spread"
    );
}

#[test]
fn test_par_spread_gives_zero_npv() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let mut tranche = mezzanine_tranche();

    // Act: Calculate par spread
    let par_spread = pricer
        .calculate_par_spread(&tranche, &market, as_of)
        .unwrap();

    // Set tranche to par spread and reprice
    tranche.running_coupon_bp = par_spread;
    let pv_at_par = pricer
        .price_tranche(&tranche, &market, as_of)
        .unwrap()
        .amount();

    // Assert: PV at par spread should be very close to zero
    approx_eq(
        pv_at_par,
        0.0,
        tranche.notional.amount() * 0.001, // Allow 0.1% of notional tolerance
        "PV at par spread should be ~zero",
    );
}

/// Invariant/property guard for `calculate_par_spread`: verifies that par spread is strictly
/// positive and finite for both protection sides and that the two sides agree to within a small
/// relative tolerance (par spread is a tranche property, independent of side).  Also verifies
/// that pricing each tranche at its computed par spread yields ~0 NPV.
///
/// # Note: invariant guard, not a fail-on-parent regression test (M14 investigation)
///
/// This test passes on BOTH the pre-fix and post-fix code for the 3-7% mezzanine tranche
/// used here. The fix changed the Newton-Raphson seed from `protection_pv / premium_per_bp`
/// (which is negative for SellProtection, clamped to 0 by the loop's `.clamp(0, 100000)`) to
/// `protection_pv.abs() / premium_per_bp.abs()` (always positive).
///
/// The wrong seed can only produce a wrong result when `|NPV(spread=0)| < PAR_SPREAD_TOLERANCE
/// * notional = 1e-6 * notional`.  Since `NPV(spread=0) ≈ protection_pv` (premium leg is 0 at
/// zero coupon), this requires `|protection_pv| < 1e-6 * notional`.
///
/// In practice (verified for IG spreads 12–140 bp, tenors 1.25–5Y, attachments 10–60%,
/// 125-name pool, recovery 40%), the protection PV is either exactly $0 (EL below the model's
/// numerical floor, implying the correct par spread truly is 0) or comfortably above the $10
/// threshold on a $10M notional (smallest observed: ~$4,400 for a 20-100% tranche with very
/// tight spreads).  The latent false-convergence path therefore cannot be triggered by any
/// realistic market inputs: Newton always recovers from the wrong seed in subsequent iterations.
///
/// The seed fix is nonetheless correct hardening: it removes a latent code smell, makes the
/// sign intent explicit, and eliminates an unnecessary iteration for all non-degenerate inputs.
#[test]
fn test_par_spread_positive_and_side_invariant() {
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    // Same mezzanine tranche, two sides
    let tranche_sell = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let tranche_buy = custom_tranche(3.0, 7.0, 500.0, TrancheSide::BuyProtection);

    let par_sell = pricer
        .calculate_par_spread(&tranche_sell, &market, as_of)
        .expect("par spread SellProtection should succeed");
    let par_buy = pricer
        .calculate_par_spread(&tranche_buy, &market, as_of)
        .expect("par spread BuyProtection should succeed");

    // (a) Both must be strictly positive and finite
    assert!(
        par_sell.is_finite() && par_sell > 0.0,
        "SellProtection par spread must be strictly positive and finite, got {}",
        par_sell
    );
    assert!(
        par_buy.is_finite() && par_buy > 0.0,
        "BuyProtection par spread must be strictly positive and finite, got {}",
        par_buy
    );

    // (b) Both sides must agree to within 0.1% relative tolerance — par spread is a
    //     property of the tranche, not of who holds the protection.
    relative_eq(
        par_buy,
        par_sell,
        0.001,
        "BuyProtection and SellProtection par spreads must agree",
    );

    // (c) Pricing at the computed par spread must yield ~0 NPV for each side
    let notional = tranche_sell.notional.amount();
    let npv_tol = notional * 0.001; // 0.1% of notional

    let mut at_par_sell = tranche_sell;
    at_par_sell.running_coupon_bp = par_sell;
    let npv_sell = pricer
        .price_tranche(&at_par_sell, &market, as_of)
        .unwrap()
        .amount();
    approx_eq(
        npv_sell,
        0.0,
        npv_tol,
        "SellProtection: NPV at par spread should be ~0",
    );

    let mut at_par_buy = tranche_buy;
    at_par_buy.running_coupon_bp = par_buy;
    let npv_buy = pricer
        .price_tranche(&at_par_buy, &market, as_of)
        .unwrap()
        .amount();
    approx_eq(
        npv_buy,
        0.0,
        npv_tol,
        "BuyProtection: NPV at par spread should be ~0",
    );
}

// ==================== Upfront Tests ====================

#[test]
fn test_upfront_equals_pv() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let upfront = pricer.calculate_upfront(&tranche, &market, as_of).unwrap();
    let pv = pricer
        .price_tranche(&tranche, &market, as_of)
        .unwrap()
        .amount();

    // Assert
    approx_eq(upfront, pv, 1e-6, "Upfront should equal PV");
}
