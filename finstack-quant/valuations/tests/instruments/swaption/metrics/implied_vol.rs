//! Implied volatility tests
//!
//! The implied vol calculator solves for Black vol that reproduces the PV.
//! When the same pricing path is used (price → implied vol → price), inversion
//! should be very precise. Market standard tolerance is 1bp of vol (0.01% relative).

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

/// Tolerance for implied vol round-trip (same pricing path): 1e-6 relative
/// This corresponds to ~0.1bp of vol at 20% vol.
const IMPLIED_VOL_ROUNDTRIP_TOL: f64 = 1e-6;

/// Tolerance for implied vol cross-strike consistency: 1e-4 relative (1bp)
/// Surface interpolation may introduce small errors across strikes.
const IMPLIED_VOL_SURFACE_TOL: f64 = 1e-4;

#[test]
fn test_implied_vol_matches_surface() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let vol_input = 0.30;
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, vol_input);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Should recover the input vol from surface
    assert_approx_eq(
        implied_vol,
        vol_input,
        IMPLIED_VOL_ROUNDTRIP_TOL,
        "Implied vol should match surface vol",
    );
}

#[test]
fn test_implied_vol_positive() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.25);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    assert!(implied_vol > 0.0, "Implied vol should be positive");
    assert!(
        implied_vol < 5.0,
        "Implied vol should be reasonable (< 500%)"
    );
}

#[test]
fn test_implied_vol_inversion() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.40);

    // Solve for implied vol given market PV
    // The implied vol calculator solves: price_black(sigma) = target_pv
    // where target_pv is the base_value computed by the same pricing function.
    // Since both use identical pricing paths, inversion should be very precise.
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Should recover input vol very precisely - solver uses 1e-10 tolerance
    assert_approx_eq(
        implied_vol,
        0.40,
        IMPLIED_VOL_ROUNDTRIP_TOL,
        "Implied vol inversion",
    );
}

#[test]
fn test_implied_vol_consistency_across_strikes() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let vol_input = 0.25;
    let market = create_flat_market(as_of, 0.05, vol_input);

    for strike in [0.03, 0.05, 0.07] {
        let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);

        let result = swaption
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::ImpliedVol],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let implied_vol = *result.measures.get("implied_vol").unwrap();

        // With flat surface, all strikes should recover same vol
        // Use slightly looser tolerance for cross-strike due to surface interpolation
        assert_approx_eq(
            implied_vol,
            vol_input,
            IMPLIED_VOL_SURFACE_TOL,
            &format!("Implied vol at strike {}", strike),
        );
    }
}
