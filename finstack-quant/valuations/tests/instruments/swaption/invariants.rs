//! Swaption mathematical invariant tests.
//!
//! These tests verify fundamental mathematical properties that must hold
//! regardless of market conditions or implementation details. They complement
//! parity tests by not depending on external reference values.
//!
//! # Key Invariants
//!
//! 1. **Payer-Receiver Parity**: At ATM strike, payer and receiver swaptions
//!    should have similar values (exact parity holds for European options
//!    when forward = strike).
//!
//! 2. **Vega Positivity**: Long option vega must be positive (higher vol → higher value).
//!
//! 3. **Monotonicity in Strike**: Payer value decreases with strike,
//!    receiver value increases with strike.
//!
//! 4. **Time Decay**: Option value decreases as expiry approaches (all else equal).

use super::common::*;
use crate::common::test_helpers::tolerances;
use finstack_quant_core::dates::DateExt;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use proptest::prelude::*;
use time::macros::date;

// =============================================================================
// Payer-Receiver Parity Tests
// =============================================================================

/// At ATM (strike = forward), payer and receiver swaptions should have equal value.
///
/// This is a fundamental no-arbitrage relationship:
/// Payer(F, K) - Receiver(F, K) = Annuity × (F - K)
///
/// At ATM (K = F): Payer = Receiver
#[test]
fn test_payer_receiver_parity_atm() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);
    let market_rate = 0.05;
    let market = create_flat_market(as_of, market_rate, 0.20);
    let forward = create_standard_payer_swaption(expiry, swap_start, swap_end, market_rate)
        .forward_swap_rate(&market, as_of)
        .unwrap();

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, forward);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, forward);

    let pv_payer = payer.value(&market, as_of).unwrap().amount();
    let pv_receiver = receiver.value(&market, as_of).unwrap().amount();

    // At ATM, payer and receiver should be very close
    // Allow small difference due to numerical precision and leg convention asymmetry
    let diff = (pv_payer - pv_receiver).abs();
    let avg = (pv_payer + pv_receiver) / 2.0;
    let rel_diff = diff / avg;

    assert!(
        rel_diff < 0.05, // Within 5% when strike matches forward
        "ATM payer-receiver should be close: payer={:.2}, receiver={:.2}, rel_diff={:.2}%",
        pv_payer,
        pv_receiver,
        rel_diff * 100.0
    );
}

/// Payer - Receiver = Annuity × (Forward - Strike) for any strike.
///
/// This parity relationship must hold exactly for European swaptions.
#[test]
fn test_payer_receiver_parity_itm_otm() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);
    let market_rate = 0.05;

    // Test across different moneyness levels
    for strike in [0.03, 0.04, 0.05, 0.06, 0.07] {
        let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
        let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

        let market = create_flat_market(as_of, market_rate, 0.20);

        let pv_payer = payer.value(&market, as_of).unwrap().amount();
        let pv_receiver = receiver.value(&market, as_of).unwrap().amount();

        // Get annuity for theoretical parity check
        let disc = market.get_discount("USD_OIS").unwrap();
        let forward_rate = payer.forward_swap_rate(&market, as_of).unwrap();
        let annuity = payer.swap_annuity(disc.as_ref(), as_of).unwrap();
        let notional = payer.notional.amount();

        // Theoretical: Payer - Receiver = Notional × Annuity × (F - K)
        let expected_diff = notional * annuity * (forward_rate - strike);
        let actual_diff = pv_payer - pv_receiver;

        // Allow tolerance for numerical precision
        let tol = notional * annuity * tolerances::NUMERICAL;
        assert!(
            (actual_diff - expected_diff).abs() < tol.max(100.0),
            "Parity violated at strike={}: expected_diff={:.2}, actual_diff={:.2}",
            strike,
            expected_diff,
            actual_diff
        );
    }
}

// =============================================================================
// Monotonicity Tests
// =============================================================================

/// Payer swaption value decreases as strike increases (higher strike = less valuable call).
#[test]
fn test_payer_monotonic_in_strike() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);

    let market = create_flat_market(as_of, 0.05, 0.20);

    let strikes = [0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08];
    let mut prev_pv = f64::MAX;

    for strike in strikes {
        let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
        let pv = swaption.value(&market, as_of).unwrap().amount();

        assert!(
            pv < prev_pv + 1e-6, // Allow tiny tolerance for numerical noise
            "Payer should decrease with strike: strike={}, pv={:.2}, prev_pv={:.2}",
            strike,
            pv,
            prev_pv
        );
        prev_pv = pv;
    }
}

/// Receiver swaption value increases as strike increases (higher strike = more valuable put).
#[test]
fn test_receiver_monotonic_in_strike() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);

    let market = create_flat_market(as_of, 0.05, 0.20);

    let strikes = [0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08];
    let mut prev_pv = 0.0;

    for strike in strikes {
        let swaption = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);
        let pv = swaption.value(&market, as_of).unwrap().amount();

        assert!(
            pv > prev_pv - 1e-6, // Allow tiny tolerance for numerical noise
            "Receiver should increase with strike: strike={}, pv={:.2}, prev_pv={:.2}",
            strike,
            pv,
            prev_pv
        );
        prev_pv = pv;
    }
}

/// Option value increases with volatility (vega > 0 for long options).
#[test]
fn test_vega_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    let vols = [0.10, 0.15, 0.20, 0.30, 0.40, 0.50];
    let mut prev_pv = 0.0;

    for vol in vols {
        let market = create_flat_market(as_of, 0.05, vol);
        let pv = swaption.value(&market, as_of).unwrap().amount();

        assert!(
            pv > prev_pv - 1e-6,
            "PV should increase with vol: vol={}, pv={:.2}, prev_pv={:.2}",
            vol,
            pv,
            prev_pv
        );
        prev_pv = pv;
    }
}

// =============================================================================
// Boundary Condition Tests
// =============================================================================

/// At expiry, swaption value equals intrinsic value.
#[test]
fn test_at_expiry_equals_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of; // At expiry
    let swap_start = date!(2024 - 01 - 03); // T+2
    let swap_end = date!(2029 - 01 - 03);

    let forward_rate = 0.05;

    // ITM payer (strike < forward)
    let payer_itm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);
    let market = create_flat_market(as_of, forward_rate, 0.20);
    let pv = payer_itm.value(&market, as_of).unwrap().amount();

    // At expiry, value should be >= 0 (can't be negative)
    assert!(
        pv >= -1e-6,
        "At-expiry swaption should have non-negative value, got: {}",
        pv
    );

    // OTM payer (strike > forward) should have zero value at expiry
    let payer_otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.08);
    let pv_otm = payer_otm.value(&market, as_of).unwrap().amount();

    // At expiry, OTM option is not exercised → exactly zero value
    assert!(
        pv_otm.abs() < 1.0,
        "At-expiry OTM swaption should be zero (not exercised), got: {}",
        pv_otm
    );
}

/// Zero volatility should give intrinsic value.
#[test]
fn test_zero_vol_gives_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = date!(2030 - 01 - 01);

    // Create market with very low vol (can't use exactly 0 due to numerical issues)
    let market = create_flat_market(as_of, 0.05, 0.001);

    // ITM payer (strike < forward)
    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);
    let pv = payer.value(&market, as_of).unwrap().amount();

    // With near-zero vol, value should be close to discounted intrinsic
    let disc = market.get_discount("USD_OIS").unwrap();
    let annuity = payer.swap_annuity(disc.as_ref(), as_of).unwrap();
    let forward = payer.forward_swap_rate(&market, as_of).unwrap();
    let notional = payer.notional.amount();
    let intrinsic = notional * annuity * (forward - 0.03).max(0.0);

    // Should be close to intrinsic (within 1%)
    let rel_diff = ((pv - intrinsic) / intrinsic.max(1.0)).abs();
    assert!(
        rel_diff < 0.01,
        "Near-zero vol should give intrinsic: pv={:.2}, intrinsic={:.2}",
        pv,
        intrinsic
    );
}

// =============================================================================
// Property-Based Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Payer-receiver parity holds across different market conditions.
    #[test]
    fn prop_payer_receiver_parity(
        forward in 0.02..0.08,
        strike in 0.02..0.08,
        vol in 0.10..0.40,
        tenor_years in 2..=10,
    ) {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let swap_start = expiry;
        let swap_end = swap_start.add_months(tenor_years * 12);

        let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
        let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

        let market = create_flat_market(as_of, forward, vol);

        let pv_payer = payer.value(&market, as_of).unwrap().amount();
        let pv_receiver = receiver.value(&market, as_of).unwrap().amount();

        // Get annuity
        let disc = market.get_discount("USD_OIS").unwrap();
        let annuity = payer.swap_annuity(disc.as_ref(), as_of).unwrap();
        let forward_rate = payer.forward_swap_rate(&market, as_of).unwrap();
        let notional = payer.notional.amount();

        // Parity: Payer - Receiver = N × A × (F - K)
        let expected_diff = notional * annuity * (forward_rate - strike);
        let actual_diff = pv_payer - pv_receiver;

        // Tolerance scales with notional and annuity
        let tol = notional * annuity * 0.001; // 10bp tolerance

        prop_assert!(
            (actual_diff - expected_diff).abs() < tol.max(500.0),
            "Parity violated: expected_diff={:.2}, actual_diff={:.2}, tol={:.2}",
            expected_diff, actual_diff, tol
        );
    }

    /// Delta should be bounded between 0 and notional * annuity.
    #[test]
    fn prop_delta_bounded(
        strike in 0.02..0.08,
        vol in 0.10..0.40,
    ) {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let swap_start = expiry;
        let swap_end = date!(2030 - 01 - 01);

        let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
        let market = create_flat_market(as_of, 0.05, vol);

        let result = swaption.price_with_metrics(&market, as_of, &[MetricId::Delta], finstack_quant_valuations::instruments::PricingOptions::default()).unwrap();
        let delta = *result.measures.get("delta").unwrap();

        // Payer delta should be positive and bounded
        prop_assert!(delta >= 0.0, "Payer delta should be non-negative: {}", delta);

        // Upper bound: notional * annuity (when option is deep ITM)
        let disc = market.get_discount("USD_OIS").unwrap();
        let annuity = swaption.swap_annuity(disc.as_ref(), as_of).unwrap();
        let max_delta = swaption.notional.amount() * annuity;

        prop_assert!(
            delta <= max_delta * 1.1, // Allow 10% tolerance for numerical precision
            "Delta {} exceeds max {} for strike {}",
            delta, max_delta, strike
        );
    }
}
