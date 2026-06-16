//! Volatility surface tests

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_flat_surface_pricing_consistency() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let vol = 0.25;
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, vol);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should produce consistent results
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Flat surface pricing should work"
    );
}

#[test]
fn test_smile_surface_pricing() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let smile_surface = build_smile_vol_surface(as_of, "USD_SWAPTION_VOL");

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(smile_surface);

    // Test different strikes pick up smile
    let otm_put = create_standard_receiver_swaption(expiry, swap_start, swap_end, 0.02);
    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let otm_call = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07);

    let pv_otm_put = otm_put.value(&market, as_of).unwrap().amount();
    let pv_atm = atm.value(&market, as_of).unwrap().amount();
    let pv_otm_call = otm_call.value(&market, as_of).unwrap().amount();

    // All should price successfully with smile
    assert!(pv_otm_put > 0.0, "OTM put should have positive value");
    assert!(pv_atm > 0.0, "ATM should have positive value");
    assert!(pv_otm_call > 0.0, "OTM call should have positive value");
}

#[test]
fn test_surface_interpolation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();

    // Surface with specific vols
    let surface =
        finstack_quant_core::market_data::surfaces::VolSurface::builder("USD_SWAPTION_VOL")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[0.03, 0.05, 0.07])
            .row(&[0.20, 0.18, 0.22]) // 6M
            .row(&[0.25, 0.20, 0.25]) // 1Y
            .row(&[0.30, 0.25, 0.30]) // 2Y
            .build()
            .unwrap();

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(surface);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should interpolate and price successfully
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Surface interpolation should work"
    );
}

#[test]
fn test_extrapolation_stability() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();

    // Surface with limited strikes
    let surface =
        finstack_quant_core::market_data::surfaces::VolSurface::builder("USD_SWAPTION_VOL")
            .expiries(&[0.5, 1.0, 5.0])
            .strikes(&[0.03, 0.05, 0.07])
            .row(&[0.25, 0.25, 0.25])
            .row(&[0.25, 0.25, 0.25])
            .row(&[0.25, 0.25, 0.25])
            .build()
            .unwrap();

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(surface);

    // Test strike outside surface range
    let deep_otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.15);
    let pv = deep_otm.value(&market, as_of).unwrap().amount();

    // Should handle extrapolation gracefully (clamped or flat)
    assert!(
        pv >= 0.0 && pv.is_finite(),
        "Extrapolation should be stable"
    );
}
