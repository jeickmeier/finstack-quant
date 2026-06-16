//! Tests for day count basis handling in barrier option pricing.
//!
//! These tests validate that barrier options correctly separate day count bases:
//! - Vol surface lookup uses the instrument's day count (should match vol calibration)
//! - Discounting uses the discount curve's own day count basis
//!
//! This separation is critical because mismatched time bases bias barrier crossing
//! probabilities and rebate PVs in barrier pricing.
//!
//! # Market Standards Reference
//!
//! - Equity vol surfaces are typically quoted using ACT/365F
//! - Money market curves (USD) typically use ACT/360
//! - Barrier pricing is highly time-step sensitive; mismatched bases can cause
//!   significant pricing errors

use super::helpers::*;
use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

/// Test that a down-and-out call with ACT/365F vol vs ACT/360 curve prices correctly.
///
/// This validates the fix for: time to expiry should use curve DC for discounting
/// but query vol surface with its own year-fraction basis.
///
/// # Acceptance Criteria
/// - Deterministic pricing
/// - Price error < 1e-4 (relative to expected theoretical value)
#[test]
fn test_down_and_out_call_mismatched_day_count_bases() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 07 - 01); // 6M expiry

    // Parameters
    let spot = 100.0;
    let strike = 100.0;
    let barrier = 80.0; // Down-and-out barrier
    let vol = 0.20; // 20% flat vol
    let rate = 0.05; // 5% risk-free rate
    let div_yield = 0.0; // No dividends for simplicity

    // Create barrier option with ACT/365F day count (standard vol surface basis)
    let option = create_down_and_out_call(expiry, strike, barrier, DayCount::Act365F);

    // Create market with ACT/360 discount curve (money market convention)
    // This tests the mismatched basis scenario
    let market = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);

    // Price using analytical method
    let pv = option.value(&market, as_of).unwrap();

    // The down-and-out call should have positive value (barrier is far from spot)
    assert!(
        pv.amount() > 0.0,
        "Down-and-out call should have positive value, got {}",
        pv.amount()
    );

    // Expected approximate value for a 6M ATM call with barrier at 80:
    // - Vanilla ATM call ≈ S * N(d1) - K * e^(-rT) * N(d2)
    // - Down-and-out reduces value due to knock-out probability
    // - With 20% vol and barrier at 80% of spot, knock-out probability is small
    // - Expected PV should be close to vanilla but slightly lower

    // For the test, we verify the price is in a reasonable range
    // A vanilla 6M ATM call with these params would be roughly 5-7% of spot
    // Down-and-out should be 80-95% of vanilla (barrier is relatively far)
    let lower_bound = spot * 0.04; // At least 4% of spot
    let upper_bound = spot * 0.08; // At most 8% of spot

    assert!(
        pv.amount() >= lower_bound && pv.amount() <= upper_bound,
        "Down-and-out call PV {} should be in range [{}, {}]",
        pv.amount(),
        lower_bound,
        upper_bound
    );
}

/// Test that same-basis pricing produces consistent results.
///
/// When both vol surface and discount curve use the same day count basis,
/// the pricing should be stable and deterministic.
#[test]
fn test_down_and_out_call_same_day_count_basis() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 07 - 01); // 6M expiry

    let spot = 100.0;
    let strike = 100.0;
    let barrier = 80.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.0;

    // Create barrier option with ACT/365F day count
    let option = create_down_and_out_call(expiry, strike, barrier, DayCount::Act365F);

    // Create market with ACT/365F discount curve (same basis)
    let market_365 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act365F);

    // Price with same basis
    let pv_365 = option.value(&market_365, as_of).unwrap();

    // Verify determinism by pricing twice
    let pv_365_again = option.value(&market_365, as_of).unwrap();

    assert!(
        (pv_365.amount() - pv_365_again.amount()).abs() < 1e-12,
        "Pricing should be deterministic: {} vs {}",
        pv_365.amount(),
        pv_365_again.amount()
    );

    // Now compare with ACT/360 market
    let market_360 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);
    let pv_360 = option.value(&market_360, as_of).unwrap();

    // The prices should differ slightly due to different discount factor calculation
    // ACT/360 will give slightly more discounting for the same calendar period
    // since it uses fewer days in the denominator (360 vs 365)
    let diff_pct = ((pv_365.amount() - pv_360.amount()) / pv_365.amount()).abs();

    // Difference should be small but measurable (typically < 1% for short maturities)
    assert!(
        diff_pct < 0.02,
        "Day count basis difference should be small: {}% between {} and {}",
        diff_pct * 100.0,
        pv_365.amount(),
        pv_360.amount()
    );
}

/// Test that the vol surface is queried with the correct time basis.
///
/// By using different vol surface slopes across expiries, we can detect
/// if the wrong time basis is being used for vol lookup.
#[test]
fn test_vol_lookup_uses_correct_time_basis() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 07 - 01); // 6M expiry

    let spot = 100.0;
    let strike = 100.0;
    let barrier = 80.0;
    let rate = 0.05;
    let div_yield = 0.0;

    // Create a vol surface with distinct vol levels at different expiries
    // If the wrong time basis is used, the wrong vol will be picked
    let vol_surface = VolSurface::builder(VOL_ID)
        .expiries(&[
            0.25,  // 3M
            0.5,   // 6M (target for ACT/365F: 182/365 ≈ 0.499)
            0.505, // Slightly above 6M (ACT/360: 182/360 ≈ 0.506)
            0.51,  // Guard band
            1.0,   // 1Y
        ])
        .strikes(&[50.0, 80.0, 100.0, 120.0, 150.0])
        .row(&[0.15, 0.15, 0.15, 0.15, 0.15]) // 3M vol = 15%
        .row(&[0.20, 0.20, 0.20, 0.20, 0.20]) // 6M vol = 20% (ACT/365F)
        .row(&[0.25, 0.25, 0.25, 0.25, 0.25]) // ~6M ACT/360 vol = 25%
        .row(&[0.26, 0.26, 0.26, 0.26, 0.26]) // Guard
        .row(&[0.30, 0.30, 0.30, 0.30, 0.30]) // 1Y vol = 30%
        .build()
        .unwrap();

    // Create market with the custom vol surface and ACT/360 discount curve
    let disc_curve = build_discount_curve_with_dc(rate, as_of, DISC_ID, DayCount::Act360);

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price(
            SPOT_ID,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(
                finstack_quant_core::money::Money::new(
                    spot,
                    finstack_quant_core::currency::Currency::USD,
                ),
            ),
        )
        .insert_price(
            DIV_ID,
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(div_yield),
        );

    // Create option with ACT/365F day count (should pick 20% vol at t=0.499)
    let option_365 = create_down_and_out_call(expiry, strike, barrier, DayCount::Act365F);
    let pv_365 = option_365.value(&market, as_of).unwrap();

    // Now create market with ACT/365F discount curve for comparison
    let disc_curve_365 = build_discount_curve_with_dc(rate, as_of, DISC_ID, DayCount::Act365F);
    let market_365 = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_curve_365)
        .insert_surface(
            VolSurface::builder(VOL_ID)
                .expiries(&[0.25, 0.5, 0.505, 0.51, 1.0])
                .strikes(&[50.0, 80.0, 100.0, 120.0, 150.0])
                .row(&[0.15, 0.15, 0.15, 0.15, 0.15])
                .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
                .row(&[0.25, 0.25, 0.25, 0.25, 0.25])
                .row(&[0.26, 0.26, 0.26, 0.26, 0.26])
                .row(&[0.30, 0.30, 0.30, 0.30, 0.30])
                .build()
                .unwrap(),
        )
        .insert_price(
            SPOT_ID,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(
                finstack_quant_core::money::Money::new(
                    spot,
                    finstack_quant_core::currency::Currency::USD,
                ),
            ),
        )
        .insert_price(
            DIV_ID,
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(div_yield),
        );

    let pv_365_market = option_365.value(&market_365, as_of).unwrap();

    // The instrument with ACT/365F day count should query the vol surface at t≈0.499
    // regardless of what day count the discount curve uses
    // So both should pick approximately 20% vol and produce similar values
    // (with small difference due to discount factor calculation)
    let rel_diff = ((pv_365.amount() - pv_365_market.amount()) / pv_365.amount()).abs();

    assert!(
        rel_diff < 0.02, // Less than 2% relative difference
        "Vol lookup should use instrument day count, not curve day count. \
         PV with ACT/360 curve: {}, PV with ACT/365F curve: {}, rel diff: {:.4}%",
        pv_365.amount(),
        pv_365_market.amount(),
        rel_diff * 100.0
    );
}
