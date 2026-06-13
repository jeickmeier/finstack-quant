//! Integration tests for portfolio margin aggregation.
//!
//! Tests currency validation and FX conversion for margin aggregation across
//! netting sets with different currencies.

mod common;

use common::base_date;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_margin::{ImMethodology, NettingSetId};
use finstack_portfolio::{NettingSetMargin, PortfolioMarginResult};

fn test_date() -> finstack_core::dates::Date {
    base_date()
}

// =============================================================================
// Same-Currency Aggregation Tests
// =============================================================================

#[test]
fn test_same_currency_aggregation_succeeds() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // Add first USD netting set
    let ns1 = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_001"),
        test_date(),
        Money::new(5_000_000.0, Currency::USD),
        Money::new(1_000_000.0, Currency::USD),
        10,
        ImMethodology::Simm,
    );
    assert!(result.add_netting_set(ns1).is_ok());

    // Add second USD netting set
    let ns2 = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        test_date(),
        Money::new(3_000_000.0, Currency::USD),
        Money::new(500_000.0, Currency::USD),
        5,
        ImMethodology::ClearingHouse,
    );
    assert!(result.add_netting_set(ns2).is_ok());

    // Verify aggregation
    assert_eq!(result.total_initial_margin.amount(), 8_000_000.0);
    assert_eq!(result.total_variation_margin.amount(), 1_500_000.0);
    assert_eq!(result.total_positions, 15);
    assert_eq!(result.netting_set_count(), 2);
}

// =============================================================================
// Currency Mismatch Tests
// =============================================================================

#[test]
fn test_currency_mismatch_returns_error() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // Try to add EUR netting set to USD portfolio
    let eur_ns = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_B", "CSA_002"),
        test_date(),
        Money::new(1_000_000.0, Currency::EUR),
        Money::new(200_000.0, Currency::EUR),
        5,
        ImMethodology::Simm,
    );

    let add_result = result.add_netting_set(eur_ns);
    assert!(add_result.is_err());

    let err = add_result.unwrap_err();
    assert_eq!(err.netting_set_currency, Currency::EUR);
    assert_eq!(err.base_currency, Currency::USD);

    // Verify no netting sets were added
    assert_eq!(result.netting_set_count(), 0);
    assert_eq!(result.total_initial_margin.amount(), 0.0);
}

#[test]
fn test_error_message_is_informative() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    let eur_ns = NettingSetMargin::new(
        NettingSetId::bilateral("DEUTSCHE", "CSA_EUR"),
        test_date(),
        Money::new(1_000_000.0, Currency::EUR),
        Money::new(200_000.0, Currency::EUR),
        5,
        ImMethodology::Simm,
    );

    let err = result.add_netting_set(eur_ns).unwrap_err();
    let error_msg = err.to_string();

    // Error message should mention:
    // - The currencies involved
    // - The suggested alternative method
    assert!(error_msg.contains("EUR"));
    assert!(error_msg.contains("USD"));
    assert!(error_msg.contains("add_netting_set_with_fx"));
}

// =============================================================================
// FX Conversion Tests
// =============================================================================

#[test]
fn test_add_netting_set_with_fx_converts_correctly() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // EUR netting set with 1M EUR IM
    let eur_ns = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_B", "CSA_002"),
        test_date(),
        Money::new(1_000_000.0, Currency::EUR),
        Money::new(200_000.0, Currency::EUR),
        5,
        ImMethodology::Simm,
    );

    // EUR/USD = 1.10
    let eur_usd_rate = 1.10;
    result
        .add_netting_set_with_fx(eur_ns, eur_usd_rate)
        .expect("valid FX rate should convert");

    // Verify conversion: 1M EUR * 1.10 = 1.1M USD
    assert_eq!(result.total_initial_margin.amount(), 1_100_000.0);
    // VM: 200k EUR * 1.10 = 220k USD
    assert!((result.total_variation_margin.amount() - 220_000.0).abs() < 1e-9);
    assert_eq!(result.total_positions, 5);
    assert_eq!(result.netting_set_count(), 1);
}

#[test]
fn test_mixed_currency_aggregation_with_fx() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // Add USD netting set directly
    let usd_ns = NettingSetMargin::new(
        NettingSetId::cleared("CME"),
        test_date(),
        Money::new(5_000_000.0, Currency::USD),
        Money::new(500_000.0, Currency::USD),
        10,
        ImMethodology::ClearingHouse,
    );
    result.add_netting_set(usd_ns).expect("USD should work");

    // Add EUR netting set with FX conversion
    let eur_ns = NettingSetMargin::new(
        NettingSetId::bilateral("DB", "CSA_EUR"),
        test_date(),
        Money::new(2_000_000.0, Currency::EUR),
        Money::new(300_000.0, Currency::EUR),
        8,
        ImMethodology::Simm,
    );
    result
        .add_netting_set_with_fx(eur_ns, 1.08)
        .expect("valid EUR/USD rate should convert");

    // Add GBP netting set with FX conversion
    let gbp_ns = NettingSetMargin::new(
        NettingSetId::bilateral("BARC", "CSA_GBP"),
        test_date(),
        Money::new(1_000_000.0, Currency::GBP),
        Money::new(100_000.0, Currency::GBP),
        5,
        ImMethodology::Simm,
    );
    result
        .add_netting_set_with_fx(gbp_ns, 1.27)
        .expect("valid GBP/USD rate should convert");

    // Verify aggregation:
    // USD IM: 5M
    // EUR IM: 2M * 1.08 = 2.16M
    // GBP IM: 1M * 1.27 = 1.27M
    // Total IM: 8.43M
    let expected_im = 5_000_000.0 + (2_000_000.0 * 1.08) + (1_000_000.0 * 1.27);
    assert!((result.total_initial_margin.amount() - expected_im).abs() < 1e-6);

    // USD VM: 500k
    // EUR VM: 300k * 1.08 = 324k
    // GBP VM: 100k * 1.27 = 127k
    // Total VM: 951k
    let expected_vm = 500_000.0 + (300_000.0 * 1.08) + (100_000.0 * 1.27);
    assert!((result.total_variation_margin.amount() - expected_vm).abs() < 1e-6);

    assert_eq!(result.total_positions, 23); // 10 + 8 + 5
    assert_eq!(result.netting_set_count(), 3);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_zero_fx_rate() {
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    let eur_ns = NettingSetMargin::new(
        NettingSetId::bilateral("BANK", "CSA"),
        test_date(),
        Money::new(1_000_000.0, Currency::EUR),
        Money::new(200_000.0, Currency::EUR),
        5,
        ImMethodology::Simm,
    );

    // M-11: invalid FX rates must fail loudly instead of dropping the set.
    let err = result
        .add_netting_set_with_fx(eur_ns, 0.0)
        .expect_err("zero FX rate should fail");

    assert!(err.to_string().contains("invalid FX rate"));
    assert_eq!(result.total_initial_margin.amount(), 0.0);
    assert_eq!(result.total_variation_margin.amount(), 0.0);
    assert_eq!(result.netting_set_count(), 0);
}

#[test]
fn test_negative_vm_aggregation() {
    // Test that negative VM (counterparty owes us) aggregates correctly
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    let ns1 = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_001"),
        test_date(),
        Money::new(1_000_000.0, Currency::USD),
        Money::new(-500_000.0, Currency::USD), // Negative VM
        5,
        ImMethodology::Simm,
    );
    result.add_netting_set(ns1).expect("should succeed");

    let ns2 = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_B", "CSA_002"),
        test_date(),
        Money::new(2_000_000.0, Currency::USD),
        Money::new(300_000.0, Currency::USD), // Positive VM
        3,
        ImMethodology::Simm,
    );
    result.add_netting_set(ns2).expect("should succeed");

    // IM: 1M + 2M = 3M
    assert_eq!(result.total_initial_margin.amount(), 3_000_000.0);

    // VM: -500k + 300k = -200k (net receivable)
    assert_eq!(result.total_variation_margin.amount(), -200_000.0);
}

#[test]
fn test_cleared_bilateral_split_same_currency() {
    // Note: cleared_bilateral_split reads from stored NettingSetMargin objects
    // which retain their original currency amounts. For mixed-currency portfolios,
    // use the portfolio-level totals which are properly converted.
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // Cleared netting set (USD)
    let cleared = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        test_date(),
        Money::new(3_000_000.0, Currency::USD),
        Money::new(500_000.0, Currency::USD),
        5,
        ImMethodology::ClearingHouse,
    );
    result.add_netting_set(cleared).expect("should succeed");

    // Bilateral netting set (USD) - same currency for proper split
    let bilateral = NettingSetMargin::new(
        NettingSetId::bilateral("GS", "CSA_USD"),
        test_date(),
        Money::new(2_000_000.0, Currency::USD),
        Money::new(300_000.0, Currency::USD),
        3,
        ImMethodology::Simm,
    );
    result.add_netting_set(bilateral).expect("should succeed");

    let (cleared_total, bilateral_total) = result.cleared_bilateral_split();

    // Cleared: 3M + max(0, 500k) = 3.5M
    assert_eq!(cleared_total.amount(), 3_500_000.0);

    // Bilateral: 2M + max(0, 300k) = 2.3M
    assert_eq!(bilateral_total.amount(), 2_300_000.0);
}

#[test]
fn test_portfolio_totals_with_mixed_currencies() {
    // This test verifies that portfolio-level totals are properly converted
    // to base currency, even when individual netting sets are in different currencies.
    let mut result = PortfolioMarginResult::new(test_date(), Currency::USD);

    // Cleared netting set (USD)
    let cleared = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        test_date(),
        Money::new(3_000_000.0, Currency::USD),
        Money::new(500_000.0, Currency::USD),
        5,
        ImMethodology::ClearingHouse,
    );
    result.add_netting_set(cleared).expect("should succeed");

    // Bilateral netting set (EUR, converted at 1.10)
    let bilateral = NettingSetMargin::new(
        NettingSetId::bilateral("DB", "CSA"),
        test_date(),
        Money::new(2_000_000.0, Currency::EUR),
        Money::new(300_000.0, Currency::EUR),
        3,
        ImMethodology::Simm,
    );
    result
        .add_netting_set_with_fx(bilateral, 1.10)
        .expect("valid EUR/USD rate should convert");

    // Portfolio totals should be in USD (properly converted)
    // USD IM: 3M, EUR IM converted: 2M * 1.10 = 2.2M, Total: 5.2M
    let expected_im = 3_000_000.0 + (2_000_000.0 * 1.10);
    assert!((result.total_initial_margin.amount() - expected_im).abs() < 1e-6);

    // USD VM: 500k, EUR VM converted: 300k * 1.10 = 330k, Total: 830k
    let expected_vm = 500_000.0 + (300_000.0 * 1.10);
    assert!((result.total_variation_margin.amount() - expected_vm).abs() < 1e-6);

    // Total margin: USD (3M + 500k) + EUR converted (2M + 300k) * 1.10
    // = 3.5M + 2.53M = 6.03M
    let expected_total = 3_500_000.0 + ((2_000_000.0 + 300_000.0) * 1.10);
    assert!((result.total_margin.amount() - expected_total).abs() < 1e-6);

    // M-12: stored netting-set rows must be base-currency amounts so splits do
    // not add native EUR into USD totals.
    let (cleared_total, bilateral_total) = result.cleared_bilateral_split();
    assert_eq!(cleared_total.currency(), Currency::USD);
    assert_eq!(bilateral_total.currency(), Currency::USD);
    assert!((cleared_total.amount() - 3_500_000.0).abs() < 1e-6);
    assert!((bilateral_total.amount() - 2_530_000.0).abs() < 1e-6);
}
