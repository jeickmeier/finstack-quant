//! CDS Index edge cases and error handling tests.
//!
//! Tests cover:
//! - Matured positions
//! - Zero notional
//! - Invalid weight configurations
//! - Missing market data
//! - Extreme parameter values
//! - Recovery rate bounds
//! - Date ordering validation

use super::test_utils::*;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams,
};
use finstack_valuations::instruments::CreditParams;
use finstack_valuations::instruments::Instrument;
use time::macros::date;

const TEST_NOTIONAL: f64 = 10_000_000.0;
const TEST_RECOVERY: f64 = 0.40;

#[test]
fn test_zero_notional() {
    // Edge Case: Zero notional produces zero NPV
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-ZERO", start, end, 0.0);
    let ctx = standard_market_context(as_of);

    let npv = idx.value(&ctx, as_of).unwrap();

    assert_eq!(npv.amount(), 0.0);
}

#[test]
fn test_very_small_notional() {
    // Edge Case: Very small notional (still computes)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-TINY", start, end, 1.0);
    let ctx = standard_market_context(as_of);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());
}

#[test]
fn test_very_large_notional() {
    // Edge Case: Very large notional (still computes)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-LARGE", start, end, 1_000_000_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());
}

#[test]
fn test_matured_position() {
    // Edge Case: Pricing after maturity
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = date!(2031 - 01 - 01); // After maturity

    let idx = standard_single_curve_index("CDX-MATURED", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    // Should either return zero or error gracefully
    let result = idx.value(&ctx, as_of);
    if let Ok(npv) = result {
        // If it returns, NPV should be zero or very small
        assert!(
            npv.amount().abs() < 1000.0,
            "Matured position should have ~zero NPV"
        );
    }
}

#[test]
fn test_at_maturity() {
    // Edge Case: Pricing exactly at maturity date
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = end; // Exactly at maturity

    let idx = standard_single_curve_index("CDX-AT-MAT", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let npv = idx
        .value(&ctx, as_of)
        .expect("pricing at maturity should collapse to zero residual value");
    assert!(
        npv.amount().abs() < 1_000.0,
        "Pricing at maturity should have ~zero residual value"
    );
}

#[test]
fn test_very_short_maturity() {
    // Edge Case: Very short maturity (1 day)
    let start = date!(2025 - 01 - 01);
    let end = date!(2025 - 01 - 02); // 1 day
    let as_of = start;

    let idx = standard_single_curve_index("CDX-1D", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());
}

#[test]
fn test_very_long_maturity() {
    // Edge Case: Very long maturity (30Y)
    let start = date!(2025 - 01 - 01);
    let end = date!(2055 - 01 - 01); // 30Y
    let as_of = start;

    let idx = standard_single_curve_index("CDX-30Y", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());
}

#[test]
fn test_missing_discount_curve() {
    // Edge Case: Missing discount curve in market context
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-MISSING", start, end, 10_000_000.0);
    let ctx = MarketContext::new(); // Empty context

    let result = idx.value(&ctx, as_of);
    assert!(result.is_err(), "Should error with missing discount curve");
}

#[test]
fn test_missing_hazard_curve() {
    // Edge Case: Missing hazard curve in market context
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-NO-HZ", start, end, 10_000_000.0);

    // Context with only discount curve
    let disc = flat_discount_curve("USD-OIS", as_of, 0.03);
    let ctx = MarketContext::new().insert(disc);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_err(), "Should error with missing hazard curve");
}

#[test]
fn test_constituents_weights_dont_sum_to_one() {
    // Edge Case: Constituent weights that don't sum to 1.0 should error during pricing
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let constituents = vec![
        CDSIndexConstituent::active(CreditParams::corporate_standard("N1", "HZ1"), 0.3),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N2", "HZ2"), 0.3),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N3", "HZ3"), 0.3),
        // Sum = 0.9, not 1.0
    ];

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-BAD-WEIGHTS",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(constituents);

    let ctx = multi_constituent_market_context(as_of, 3);
    let result = idx.value(&ctx, as_of);

    // Should error or handle gracefully
    assert!(result.is_err(), "Bad weights should error during pricing");
}

#[test]
fn test_negative_weights() {
    // Edge Case: Negative weights should error
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let constituents = vec![
        CDSIndexConstituent::active(CreditParams::corporate_standard("N1", "HZ1"), -0.5),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N2", "HZ2"), 1.5),
    ];

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-NEG-WEIGHTS",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(constituents);

    let ctx = multi_constituent_market_context(as_of, 2);
    let result = idx.value(&ctx, as_of);

    assert!(result.is_err(), "Negative weights should error");
}

#[test]
fn test_recovery_rate_zero() {
    // Edge Case: Zero recovery rate (100% LGD)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-ZERO-REC",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        0.0, // Zero recovery (100% LGD)
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters");

    // The hazard curve's bootstrap recovery must agree with the index trade
    // recovery (ISDA "same R in both legs"); build HZ-INDEX at 0.0 to match.
    let ctx = MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", as_of, 0.03))
        .insert(flat_hazard_curve(
            "HZ-INDEX",
            as_of,
            0.0,
            STANDARD_HAZARD_RATE,
        ));
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Zero recovery should be valid");
}

#[test]
fn test_recovery_rate_one() {
    // Edge Case: 100% recovery rate (zero LGD)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-FULL-REC",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        1.0, // Full recovery (zero LGD)
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters");

    // The hazard curve's bootstrap recovery must agree with the index trade
    // recovery (ISDA "same R in both legs"); build HZ-INDEX at 1.0 to match.
    let ctx = MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", as_of, 0.03))
        .insert(flat_hazard_curve(
            "HZ-INDEX",
            as_of,
            1.0,
            STANDARD_HAZARD_RATE,
        ));
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Full recovery should be valid");
}

#[test]
fn test_index_factor_zero() {
    // Edge Case: Index factor of 0 (all constituents defaulted)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-ZERO-FACTOR",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_index_factor(0.0);

    let ctx = standard_market_context(as_of);
    let result = idx.value(&ctx, as_of);

    // Should handle gracefully
    // Note: Zero index factor doesn't necessarily produce zero NPV due to
    // fixed coupon payments and protection leg interactions
    assert!(
        result.is_ok(),
        "Zero index factor should price without error"
    );
}

#[test]
fn test_index_factor_greater_than_one() {
    // Edge Case: Index factor > 1.0 (invalid but should handle)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-BIG-FACTOR",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_index_factor(1.5);

    let ctx = standard_market_context(as_of);
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Index factor > 1.0 should still price");
    let npv = result.unwrap();
    assert!(npv.amount().is_finite(), "NPV should be finite");
}

#[test]
fn test_empty_constituents_list() {
    // Edge Case: Empty constituents list reverts to single-curve
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-EMPTY",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(vec![]);

    // Should be single-curve mode
    assert!(idx.constituents.is_empty());
}

#[test]
fn test_missing_constituent_hazard_curve() {
    // Edge Case: Constituent references non-existent hazard curve
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let constituents = vec![CDSIndexConstituent::active(
        CreditParams::corporate_standard("N1", "MISSING-CURVE"),
        1.0,
    )];

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-MISSING-CONST",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::PayFixed,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(constituents);

    let ctx = standard_market_context(as_of);
    let result = idx.value(&ctx, as_of);

    assert!(
        result.is_err(),
        "Should error with missing constituent curve"
    );
}

#[test]
fn test_upfront_payment_larger_than_npv() {
    // Edge Case: Large upfront payment
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let mut idx = standard_single_curve_index("CDX-BIG-UPFRONT", start, end, 10_000_000.0);
    idx.pricing_overrides.market_quotes.upfront_payment =
        Some(Money::new(10_000_000.0, Currency::USD));

    let ctx = standard_market_context(as_of);
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Large upfront should be handled");
}

#[test]
fn test_negative_upfront_payment() {
    // Edge Case: Negative upfront (payment from protection buyer)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let mut idx = standard_single_curve_index("CDX-NEG-UPFRONT", start, end, 10_000_000.0);
    idx.pricing_overrides.market_quotes.upfront_payment =
        Some(Money::new(-100_000.0, Currency::USD));

    let ctx = standard_market_context(as_of);
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Negative upfront should be handled");
}

#[test]
fn test_extremely_high_hazard_rate() {
    // Edge Case: Very high hazard rate (near certain default)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-HIGH-HZ", start, end, 10_000_000.0);

    // Create context with very high hazard rate
    let disc = flat_discount_curve("USD-OIS", as_of, 0.03);
    let hz = flat_hazard_curve("HZ-INDEX", as_of, 0.40, 0.50); // 50% hazard rate
    let ctx = MarketContext::new().insert(disc).insert(hz);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok(), "High hazard rate should be handled");
}

#[test]
fn test_zero_hazard_rate() {
    // Edge Case: Zero hazard rate (no default risk)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-ZERO-HZ", start, end, 10_000_000.0);

    // Create context with zero hazard rate
    let disc = flat_discount_curve("USD-OIS", as_of, 0.03);
    let hz = flat_hazard_curve("HZ-INDEX", as_of, 0.40, 0.0); // Zero hazard
    let ctx = MarketContext::new().insert(disc).insert(hz);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok(), "Zero hazard rate should be handled");
}
