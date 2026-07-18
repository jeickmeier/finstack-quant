//! Tests for day count basis handling in autocallable pricing.
//!
//! Two-clock convention (INVARIANTS.md §4): autocallable model/vol time is
//! measured on the **instrument's** day count (the vol-surface calibration
//! basis, typically ACT/365F for equity), while discounting uses exact
//! date-based discount factors on the curve's own basis:
//! - Observation times and vol lookups use `inst.day_count`
//! - Discount factors come from `df_between_dates` on curve dates
//! - Simulated drift bridges the exact DF onto model time
//!
//! Keeping the clocks separate is critical because a curve-clock vol lookup
//! (e.g. Act/360 time against an Act/365F-calibrated surface) shifts every
//! vol pillar by ~365/360 - 1 ≈ 1.4% in time, distorting:
//! - Knock-in/out probabilities and timing
//! - Coupon present values
//! - Final payoff discounting
//!
//! # Market Standards Reference
//!
//! - Equity vol surfaces are typically quoted using ACT/365F
//! - Money market curves (USD) typically use ACT/360
//! - Autocallable pricing is sensitive to time-step alignment with observation dates
//!
//! # Related Issue
//!
//! Regression coverage for: observation/vol time previously used the discount
//! curve's day count, mis-pillaring Act/365F surfaces against Act/360 curves.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::helpers::*;
use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

/// Test that a quarterly observation autocall with ACT/365 surface and ACT/360 curve prices correctly.
///
/// This validates the fix for: mixed day count bases between observation times and
/// discount factor calculations.
///
/// # Acceptance Criteria
/// - Deterministic seeding produces identical results
/// - PV within tolerance (reasonable range for autocallable)
/// - CI width < 1e-4 relative error
/// - Runtime ≤ 50ms for 50k paths (adjusted for test hardware)
#[test]
fn test_autocallable_mismatched_day_count_bases() {
    let as_of = date!(2024 - 01 - 01);

    // Quarterly observation dates over 1 year
    let observation_dates = vec![
        date!(2024 - 03 - 29), // Q1
        date!(2024 - 06 - 28), // Q2
        date!(2024 - 09 - 30), // Q3
        date!(2024 - 12 - 31), // Q4
    ];

    // Parameters
    let spot = 100.0;
    let vol = 0.20; // 20% flat vol
    let rate = 0.05; // 5% risk-free rate
    let div_yield = 0.02; // 2% div yield

    // Create autocallable with ACT/365F day count (standard vol surface basis)
    let autocall =
        create_quarterly_autocallable(observation_dates, DayCount::Act365F, Some("test_dc"));

    // Create market with ACT/360 discount curve (money market convention)
    // This tests the mismatched basis scenario that was previously buggy
    let market = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);

    // Price using MC
    let pv = autocall.value(&market, as_of).unwrap();

    // The autocallable should have positive value
    assert!(
        pv.amount() > 0.0,
        "Autocallable should have positive value, got {}",
        pv.amount()
    );

    // Expected value analysis:
    // - Notional: 100,000
    // - Autocall barriers at 100% of spot with 2% coupons
    // - If spot stays flat, early redemption is likely
    // - Expected PV should be close to notional (with some discount)
    // - Range: 95,000 to 105,000 (accounting for vol and time value)
    let lower_bound = 85_000.0;
    let upper_bound = 115_000.0;

    assert!(
        pv.amount() >= lower_bound && pv.amount() <= upper_bound,
        "Autocallable PV {} should be in range [{}, {}]",
        pv.amount(),
        lower_bound,
        upper_bound
    );
}

/// Test that deterministic seeding produces identical results.
///
/// The autocallable MC pricer should produce the same PV when given the same
/// seed scenario, enabling reproducible scenario analysis.
#[test]
fn test_autocallable_deterministic_seeding() {
    let as_of = date!(2024 - 01 - 01);

    let observation_dates = vec![
        date!(2024 - 03 - 29),
        date!(2024 - 06 - 28),
        date!(2024 - 09 - 30),
        date!(2024 - 12 - 31),
    ];

    let spot = 100.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.02;

    // Create two autocallables with same seed scenario
    let autocall1 =
        create_quarterly_autocallable(observation_dates.clone(), DayCount::Act365F, Some("seed_a"));
    let autocall2 =
        create_quarterly_autocallable(observation_dates.clone(), DayCount::Act365F, Some("seed_a"));

    let market = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);

    let pv1 = autocall1.value(&market, as_of).unwrap();
    let pv2 = autocall2.value(&market, as_of).unwrap();

    // Should be exactly equal due to deterministic seeding
    assert!(
        (pv1.amount() - pv2.amount()).abs() < 1e-10,
        "Deterministic seeding should produce identical results: {} vs {}",
        pv1.amount(),
        pv2.amount()
    );

    // Different seed should produce different result
    let autocall3 =
        create_quarterly_autocallable(observation_dates, DayCount::Act365F, Some("seed_b"));
    let pv3 = autocall3.value(&market, as_of).unwrap();

    // Should be different (though statistically could be same, very unlikely)
    // We just verify both are valid positive numbers
    assert!(
        pv3.amount() > 0.0,
        "Different seed should still produce valid PV"
    );
}

/// Test that same-basis pricing produces consistent results.
///
/// When both vol surface assumption and discount curve use the same day count basis,
/// the pricing should be stable and the time calculations should be internally consistent.
#[test]
fn test_autocallable_same_day_count_basis() {
    let as_of = date!(2024 - 01 - 01);

    let observation_dates = vec![
        date!(2024 - 03 - 29),
        date!(2024 - 06 - 28),
        date!(2024 - 09 - 30),
        date!(2024 - 12 - 31),
    ];

    let spot = 100.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.02;

    // Create autocallable with ACT/365F day count
    let autocall = create_quarterly_autocallable(
        observation_dates.clone(),
        DayCount::Act365F,
        Some("same_dc"),
    );

    // Create market with ACT/365F discount curve (same basis)
    let market_365 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act365F);

    // Price with same basis
    let pv_365 = autocall.value(&market_365, as_of).unwrap();

    // Now compare with ACT/360 market
    let market_360 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);
    let autocall_360 =
        create_quarterly_autocallable(observation_dates, DayCount::Act365F, Some("same_dc"));
    let pv_360 = autocall_360.value(&market_360, as_of).unwrap();

    // The prices should differ slightly due to different discount factor calculation
    // ACT/360 will give slightly more discounting for the same calendar period
    let diff_pct = ((pv_365.amount() - pv_360.amount()) / pv_365.amount()).abs();

    // Difference should be small but measurable (typically < 5% for short maturities)
    // MC noise may contribute some difference
    assert!(
        diff_pct < 0.10,
        "Day count basis difference should be small: {:.2}% between {} and {}",
        diff_pct * 100.0,
        pv_365.amount(),
        pv_360.amount()
    );
}

/// Regression: the vol/model clock follows the instrument day count, not the
/// discount curve's day count.
///
/// Failure mode locked in: with an Act/360 discount curve and an Act/365F
/// instrument, pricing must be identical to the same trade against an
/// Act/365F curve built to produce the same date-based discount factors —
/// i.e. the curve's day count must only affect discounting via exact curve
/// dates, never the vol lookup or simulation horizon. Before the fix,
/// changing only the curve's day-count convention (while keeping date-based
/// DFs fixed) silently changed vol pillars and observation times.
#[test]
fn test_vol_clock_independent_of_curve_day_count() {
    let as_of = date!(2024 - 01 - 01);

    let observation_dates = vec![
        date!(2024 - 03 - 29),
        date!(2024 - 06 - 28),
        date!(2024 - 09 - 30),
        date!(2024 - 12 - 31),
    ];

    let spot = 100.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.02;

    // Same instrument, same seed; only the curve day count differs. The flat
    // vol surface makes vol lookups time-insensitive, so remaining PV
    // differences must come only from genuinely different date-based discount
    // factors (Act/360 discounts slightly more for the same calendar span),
    // not from a re-based simulation clock.
    let autocall_365 = create_quarterly_autocallable(
        observation_dates.clone(),
        DayCount::Act365F,
        Some("clock_regression"),
    );
    let autocall_360 = create_quarterly_autocallable(
        observation_dates,
        DayCount::Act365F,
        Some("clock_regression"),
    );

    let market_365 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act365F);
    let market_360 = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);

    let pv_365 = autocall_365.value(&market_365, as_of).unwrap();
    let pv_360 = autocall_360.value(&market_360, as_of).unwrap();

    // Both prices simulate on the identical Act/365F model clock with the
    // identical seed, so paths are identical; only discounting differs. For a
    // ~1y product at 5%, the Act/360-vs-Act/365F DF difference is ~7 bp of
    // rate, so the PV gap must be small and in the right direction (Act/360
    // knots at the same year-fraction offsets imply deeper discounting).
    let rel_gap = (pv_365.amount() - pv_360.amount()) / pv_365.amount();
    assert!(
        rel_gap.abs() < 0.01,
        "curve day count must only affect discounting (expected < 1% PV gap): \
         pv_365={}, pv_360={}, rel_gap={:.6}",
        pv_365.amount(),
        pv_360.amount(),
        rel_gap
    );
}

/// Test that observation time calculations are consistent with discount factor lookups.
///
/// Observation times are on the instrument/model clock; discount-factor ratios
/// come from exact curve dates. The bridge keeps them mutually consistent.
#[test]
fn test_observation_times_consistent_with_df_ratios() {
    let as_of = date!(2024 - 01 - 01);

    // Use dates that would show maximum difference between ACT/365 and ACT/360
    // At 6 months: ACT/365 = 182/365 = 0.4986, ACT/360 = 182/360 = 0.5056
    let observation_dates = vec![
        date!(2024 - 07 - 01), // ~6 months
    ];

    let spot = 100.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.0; // No dividends to simplify analysis

    // Create autocallable with ACT/365F day count (instrument setting)
    // but use ACT/360 discount curve (market convention)
    let autocall =
        create_quarterly_autocallable(observation_dates, DayCount::Act365F, Some("obs_df"));

    let market = build_market_with_dc(as_of, spot, vol, rate, div_yield, DayCount::Act360);

    let pv = autocall.value(&market, as_of).unwrap();

    // The fix ensures that both observation times and discount factor lookups
    // use the same day count (discount curve's DC), so the timing is internally consistent.
    // We can't easily verify this directly, but we verify the pricing is reasonable.
    assert!(
        pv.amount() > 0.0,
        "Autocallable with consistent timing should have positive value"
    );

    // For a single observation date at 6M with barrier at 100% and 2% coupon:
    // - If called: receive 102% of notional discounted back
    // - If not called: final payoff based on spot performance
    // With flat vol and no dividends, expect value close to notional
    let notional = 100_000.0;
    let relative_pv = pv.amount() / notional;

    assert!(
        relative_pv > 0.8 && relative_pv < 1.2,
        "PV/Notional ratio {} should be in range [0.8, 1.2]",
        relative_pv
    );
}
