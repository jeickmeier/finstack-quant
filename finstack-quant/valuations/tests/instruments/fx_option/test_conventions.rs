//! Convention-level tests for FX ATM strike helpers.

use finstack_quant_valuations::instruments::fx::fx_option::{FxAtmDeltaConvention, FxOption};

#[test]
fn test_atm_dns_unadjusted_spot_and_forward_share_same_strike() {
    let forward = 1.12;
    let vol = 0.18;
    let time_to_expiry = 0.75;
    let variance = vol * vol * time_to_expiry;
    let expected = forward * (0.5_f64 * variance).exp();

    let spot_dns = FxOption::atm_dns_strike_for_convention(
        forward,
        vol,
        time_to_expiry,
        FxAtmDeltaConvention::Spot,
    );
    let forward_dns = FxOption::atm_dns_strike_for_convention(
        forward,
        vol,
        time_to_expiry,
        FxAtmDeltaConvention::Forward,
    );

    assert!(
        (spot_dns - expected).abs() < 1e-12,
        "spot-delta DNS strike mismatch: expected {expected}, got {spot_dns}"
    );
    assert!(
        (forward_dns - expected).abs() < 1e-12,
        "forward-delta DNS strike mismatch: expected {expected}, got {forward_dns}"
    );
}

#[test]
fn test_atm_dns_premium_adjusted_uses_negative_half_variance() {
    let forward = 1.12;
    let vol = 0.18;
    let time_to_expiry = 0.75;
    let variance = vol * vol * time_to_expiry;
    let expected = forward * (-0.5_f64 * variance).exp();

    let pa_spot_dns = FxOption::atm_dns_strike_for_convention(
        forward,
        vol,
        time_to_expiry,
        FxAtmDeltaConvention::PremiumAdjustedSpot,
    );
    let pa_forward_dns = FxOption::atm_dns_strike_for_convention(
        forward,
        vol,
        time_to_expiry,
        FxAtmDeltaConvention::PremiumAdjustedForward,
    );

    assert!(
        (pa_spot_dns - expected).abs() < 1e-12,
        "premium-adjusted spot DNS strike mismatch: expected {expected}, got {pa_spot_dns}"
    );
    assert!(
        (pa_forward_dns - expected).abs() < 1e-12,
        "premium-adjusted forward DNS strike mismatch: expected {expected}, got {pa_forward_dns}"
    );
}
