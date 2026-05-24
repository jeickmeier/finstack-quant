//! Core pricing tests for FX swaps.
//!
//! Tests the fundamental valuation logic including:
//! - Basic PV calculation at inception and over time
//! - Contract rates vs. model-implied rates
//! - Fair value pricing
//! - Currency consistency

use super::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_valuations::instruments::FxSwap;
use finstack_valuations::instruments::Instrument;

#[test]
fn test_basic_pv_at_inception() {
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("BASIC_PV", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let pv = swap.value(&market, dates.as_of).unwrap();

    // At inception with model-implied rates, PV should be small
    // Note: The near leg has T+2 settlement which creates a small discounting difference
    // due to different domestic/foreign rates: PV ≈ N × S × (DF_for(near) - DF_dom(near))
    // With USD ~1%, EUR ~0.5%, this is approximately $30 for $1M notional
    // Market standard: < $100 on $1M notional (< 1bp) accounting for settlement effects
    assert!(
        pv.amount().abs() < 100.0,
        "PV at inception should be near zero, got: {}",
        pv.amount()
    );
    assert_eq!(
        pv.currency(),
        Currency::USD,
        "PV should be in quote currency"
    );
}

#[test]
fn test_pv_with_contract_rates_fair() {
    // Test that when contract rates match model rates, PV is near zero
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    // Calculate model-implied forward from approximate curve values
    // Note: These are approximations - the actual curve may differ slightly
    // For a 1-year swap, USD ~1% gives DF≈0.99, EUR ~0.5% gives DF≈0.995
    // CIP formula: F = S × (DF_for/DF_dom) when r_dom > r_for means F > S
    let spot = 1.1;
    let df_dom_far = 0.99; // Approximate from curve
    let df_for_far = 0.995; // Approximate from curve
    let model_fwd = spot * df_for_far / df_dom_far;

    let swap = create_fx_swap_with_rates(
        "FAIR_CONTRACT",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        spot,
        model_fwd,
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // With fair contract rates (approximated from curve shape), PV should be small
    // The approximation error causes some deviation from zero (~$15)
    // Market standard: < $100 on $1M notional (< 1bp)
    assert!(
        pv.amount().abs() < 100.0,
        "PV with fair contract rates should be near zero, got: {}",
        pv.amount()
    );
}

#[test]
fn test_pv_with_mispriced_far_rate() {
    // Test that mispriced contract rates produce non-zero PV
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_fx_swap_with_rates(
        "MISPRICED",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10, // spot
        1.25, // significantly off-market forward
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // With mispriced far rate, should have material PV
    assert!(
        pv.amount().abs() > 1000.0,
        "PV with mispriced far rate should be material, got: {}",
        pv.amount()
    );
}

#[test]
fn test_pv_different_tenors() {
    // Test PV calculation for various swap tenors
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap_1m =
        create_standard_fx_swap("SWAP_1M", dates.near_date, dates.far_date_1m, 1_000_000.0);

    let swap_3m =
        create_standard_fx_swap("SWAP_3M", dates.near_date, dates.far_date_3m, 1_000_000.0);

    let swap_1y =
        create_standard_fx_swap("SWAP_1Y", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let pv_1m = swap_1m.value(&market, dates.as_of).unwrap();
    let pv_3m = swap_3m.value(&market, dates.as_of).unwrap();
    let pv_1y = swap_1y.value(&market, dates.as_of).unwrap();

    // All should be close to zero at inception
    // Note: T+2 settlement creates small discounting differences (~$30-50)
    // Market standard: < $100 on $1M notional (< 1bp)
    assert!(
        pv_1m.amount().abs() < 100.0,
        "1M swap PV should be near zero, got: {}",
        pv_1m.amount()
    );
    assert!(
        pv_3m.amount().abs() < 100.0,
        "3M swap PV should be near zero, got: {}",
        pv_3m.amount()
    );
    assert!(
        pv_1y.amount().abs() < 100.0,
        "1Y swap PV should be near zero, got: {}",
        pv_1y.amount()
    );
}

#[test]
fn test_pv_with_different_notionals() {
    // Test that PV scales linearly with notional
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap_1m = create_fx_swap_with_rates(
        "NOTIONAL_1M",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,
        1.20,
    );

    let swap_2m = create_fx_swap_with_rates(
        "NOTIONAL_2M",
        dates.near_date,
        dates.far_date_1y,
        2_000_000.0,
        1.10,
        1.20,
    );

    let pv_1m = swap_1m.value(&market, dates.as_of).unwrap();
    let pv_2m = swap_2m.value(&market, dates.as_of).unwrap();

    // 2M notional should produce roughly 2x the PV
    assert_within_pct(
        pv_2m.amount(),
        pv_1m.amount() * 2.0,
        1.0,
        "PV should scale linearly with notional",
    );
}

#[test]
fn test_pv_steep_curves() {
    // Test PV calculation with steep interest rate curves
    let dates = TestDates::standard();
    let market = setup_steep_curve_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "STEEP_CURVES",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should still produce a valid PV
    assert!(
        pv.amount().is_finite(),
        "PV with steep curves should be finite"
    );
}

#[test]
fn test_pv_inverted_curves() {
    // Test PV with inverted yield curves (negative term premium)
    let dates = TestDates::standard();
    let market = setup_inverted_curve_market(dates.as_of);

    let swap = create_standard_fx_swap("INVERTED", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should handle inverted curves gracefully
    assert!(
        pv.amount().is_finite(),
        "PV with inverted curves should be finite"
    );
}

#[test]
fn test_pv_currency_consistency() {
    // Verify that PV is always returned in quote currency
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "CURRENCY_CHECK",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    assert_eq!(
        pv.currency(),
        Currency::USD,
        "PV must be in quote currency (USD)"
    );
}

#[test]
fn test_pv_with_only_near_rate() {
    // Test swap with only near rate specified (far rate from model)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = FxSwap::builder()
        .id("NEAR_RATE_ONLY".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(dates.near_date)
        .far_date(dates.far_date_1y)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .near_rate(1.10)
        .build()
        .unwrap();

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should produce valid PV using model forward
    assert!(pv.amount().is_finite(), "PV should be finite");
}

#[test]
fn test_pv_with_only_far_rate() {
    // Test swap with only far rate specified (near rate from market)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = FxSwap::builder()
        .id("FAR_RATE_ONLY".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(dates.near_date)
        .far_date(dates.far_date_1y)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .far_rate(1.20)
        .build()
        .unwrap();

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should produce valid PV using market spot
    assert!(pv.amount().is_finite(), "PV should be finite");
}

/// CIP parity regression test.
///
/// Validates covered interest parity formula: F = S × (DF_for/DF_dom)
/// Derivation: F = S × (1 + r_dom × T) / (1 + r_for × T) = S × DF_for / DF_dom
/// When domestic rate > foreign rate, forward should be at premium (F > S).
///
/// Test case:
/// - r_dom = 5%, r_for = 0.5%, S = 1
/// - Expected: F > 1 (forward at premium)
/// - PV at model forward should be near zero
#[test]
fn test_cip_parity_forward_at_premium() {
    use finstack_core::dates::Date;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use std::sync::Arc;

    // Mock FX provider with S = 1.0
    #[derive(Clone)]
    struct UnitSpotProvider;
    impl FxProvider for UnitSpotProvider {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            Ok(1.0)
        }
    }

    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let near_date = Date::from_calendar_date(2024, time::Month::January, 3).unwrap();
    let far_date_1m = Date::from_calendar_date(2024, time::Month::February, 3).unwrap();
    let far_date_2m = Date::from_calendar_date(2024, time::Month::March, 3).unwrap();

    // r_dom = 5% (USD), r_for = 0.5% (EUR)
    // For 5% over 1 year: DF ≈ 0.9512
    // For 0.5% over 1 year: DF ≈ 0.995
    let usd_curve = DiscountCurve::builder("USD-HIGH")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.9512)]) // ~5% rate
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-LOW")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.995)]) // ~0.5% rate
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fx_matrix = FxMatrix::new(Arc::new(UnitSpotProvider));

    let market = finstack_core::market_data::context::MarketContext::new()
        .insert(usd_curve.clone())
        .insert(eur_curve.clone())
        .insert_fx(fx_matrix);

    // Create 1M swap (spot = 1.0, no explicit rates - use model forward)
    let swap_1m = FxSwap::builder()
        .id("CIP_1M".to_string().into())
        .base_currency(Currency::EUR) // foreign
        .quote_currency(Currency::USD) // domestic
        .near_date(near_date)
        .far_date(far_date_1m)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-HIGH".into())
        .foreign_discount_curve_id("EUR-LOW".into())
        .build()
        .unwrap();

    // Create 2M swap
    let swap_2m = FxSwap::builder()
        .id("CIP_2M".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(near_date)
        .far_date(far_date_2m)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-HIGH".into())
        .foreign_discount_curve_id("EUR-LOW".into())
        .build()
        .unwrap();

    // Verify model forward is > 1 (forward at premium when r_dom > r_for)
    // CIP: F = S × DF_for / DF_dom
    // With S=1, DF_for > DF_dom when r_dom > r_for, so F > 1
    let pv_1m = swap_1m.value(&market, as_of).unwrap();
    let pv_2m = swap_2m.value(&market, as_of).unwrap();

    // At model rates (no explicit contract rates), PV should be near zero
    // Tolerance: < $100 on $1M notional (< 1bp)
    assert!(
        pv_1m.amount().abs() < 100.0,
        "1M CIP swap PV at model rates should be near zero, got: {} (expected < $100)",
        pv_1m.amount()
    );
    assert!(
        pv_2m.amount().abs() < 100.0,
        "2M CIP swap PV at model rates should be near zero, got: {} (expected < $100)",
        pv_2m.amount()
    );

    // Test with explicit forward rates to verify direction
    // Model forward ≈ 1 × 0.995 / 0.9512 ≈ 1.046 at 1Y (approx 1.004 at 2M)
    // Test that underpriced forward (F < S = 1) produces negative PV
    let swap_underpriced = FxSwap::builder()
        .id("CIP_UNDERPRICED".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(near_date)
        .far_date(far_date_2m)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-HIGH".into())
        .foreign_discount_curve_id("EUR-LOW".into())
        .near_rate(1.0)
        .far_rate(0.99) // Underpriced forward (< spot when it should be > spot)
        .build()
        .unwrap();

    let pv_underpriced = swap_underpriced.value(&market, as_of).unwrap();

    // Underpriced forward should produce material negative PV
    // because we contracted to receive EUR at far at 0.99 USD/EUR but model says > 1.0
    assert!(
        pv_underpriced.amount() < -1000.0,
        "Underpriced forward (0.99 vs model > 1.0) should have negative PV, got: {}",
        pv_underpriced.amount()
    );
}
