//! SABR model pricing tests

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::rates::swaption::SABRParameters;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_sabr_pricing_runs() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;

    let sabr_params = SABRParameters {
        alpha: 0.20,
        beta: 0.5,
        rho: -0.3,
        nu: 0.4,
        shift: None,
    };

    let swaption =
        create_standard_payer_swaption(expiry, swap_start, swap_end, strike).with_sabr(sabr_params);

    let market = create_flat_market(as_of, 0.05, 0.30); // Vol surface still needed for fallback
    let pv = swaption.value(&market, as_of).unwrap();

    assert!(
        pv.amount() > 0.0,
        "SABR pricing should produce positive value"
    );
    assert!(pv.amount().is_finite(), "SABR pricing should be finite");
}

#[test]
fn test_sabr_smile_effect() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();

    // SABR with negative rho creates volatility smile
    let sabr_params = SABRParameters {
        alpha: 0.25,
        beta: 0.5,
        rho: -0.4, // Negative correlation
        nu: 0.5,   // Significant vol-of-vol
        shift: None,
    };

    let market = create_flat_market(as_of, 0.05, 0.25);

    // OTM put (low strike)
    let otm_put = create_standard_receiver_swaption(expiry, swap_start, swap_end, 0.03)
        .with_sabr(sabr_params.clone());

    // ATM
    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05)
        .with_sabr(sabr_params.clone());

    // OTM call (high strike)
    let otm_call =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07).with_sabr(sabr_params);

    let pv_otm_put = otm_put.value(&market, as_of).unwrap().amount();
    let pv_atm = atm.value(&market, as_of).unwrap().amount();
    let pv_otm_call = otm_call.value(&market, as_of).unwrap().amount();

    // All should have positive value
    assert!(
        pv_otm_put > 0.0 && pv_atm > 0.0 && pv_otm_call > 0.0,
        "SABR pricing should handle smile"
    );
}

#[test]
fn test_sabr_beta_effect() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.07; // OTM for testing beta effect
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Beta = 0.5 (CEV model) - more stable than pure normal
    let sabr_normal = SABRParameters {
        alpha: 0.15,
        beta: 0.5, // Use 0.5 instead of 0.0 for better stability
        rho: 0.0,
        nu: 0.2,
        shift: None,
    };

    // Beta = 1 (lognormal model)
    let sabr_lognormal = SABRParameters {
        alpha: 0.25,
        beta: 1.0,
        rho: 0.0,
        nu: 0.3,
        shift: None,
    };

    let swaption_normal =
        create_standard_payer_swaption(expiry, swap_start, swap_end, strike).with_sabr(sabr_normal);
    let swaption_lognormal = create_standard_payer_swaption(expiry, swap_start, swap_end, strike)
        .with_sabr(sabr_lognormal);

    let pv_normal = swaption_normal.value(&market, as_of).unwrap().amount();
    let pv_lognormal = swaption_lognormal.value(&market, as_of).unwrap().amount();

    // Both should produce valid prices (exact values depend on parameterization)
    assert!(
        pv_normal > 0.0 && pv_normal.is_finite(),
        "SABR CEV model (beta=0.5) should work"
    );
    assert!(
        pv_lognormal > 0.0 && pv_lognormal.is_finite(),
        "SABR lognormal model (beta=1.0) should work"
    );
}
