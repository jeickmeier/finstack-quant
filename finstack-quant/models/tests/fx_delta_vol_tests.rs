use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::surfaces::{
    FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
    VolInterpolationMode, interpolate_fx_wing_vols,
};
use finstack_quant_core::money::fx::CurrencyPair;
use finstack_quant_models::volatility::{
    VolSource, delta_to_strike, get_fx_delta_pillar_vols, get_fx_delta_vol,
    get_surface_vol_clamped, materialize_fx_delta_surface, strike_to_delta,
};
use std::sync::Arc;

fn eurusd_pair() -> CurrencyPair {
    CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("EUR/USD pair must be valid")
}

fn build_surface_3pt(
    id: &str,
    expiries: &[f64],
    atm_vols: &[f64],
    rr_25d: &[f64],
    bf_25d: &[f64],
) -> FxDeltaVolSurface {
    FxDeltaVolSurface::builder(id, eurusd_pair())
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(expiries)
        .atm_vols(atm_vols)
        .rr_25d(rr_25d)
        .bf_25d(bf_25d)
        .build()
        .expect("3-point FX delta surface must validate")
}

fn delta_surface_5pt() -> FxDeltaVolSurface {
    FxDeltaVolSurface::builder("EURUSD-VOL", eurusd_pair())
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(&[0.5, 1.0])
        .atm_vols(&[0.10, 0.11])
        .rr_25d(&[0.01, 0.015])
        .bf_25d(&[0.002, 0.003])
        .rr_10d(&[0.02, 0.028])
        .bf_10d(&[0.004, 0.006])
        .build()
        .expect("5-point FX delta surface must validate")
}

#[test]
fn five_point_smile_evaluates_and_clamps_wings() {
    let delta_surface = delta_surface_5pt();
    let spot: f64 = 1.10;
    let r_dom: f64 = 0.03;
    let r_for: f64 = 0.01;
    let fwd_05 = spot * ((r_dom - r_for) * 0.5).exp();
    let fwd_10 = spot * ((r_dom - r_for) * 1.0).exp();
    let materialized = materialize_fx_delta_surface(&delta_surface, &[fwd_05, fwd_10])
        .expect("materialization must succeed");
    assert_eq!(materialized.expiries(), &[0.5, 1.0]);
    assert_eq!(materialized.interpolation_mode(), VolInterpolationMode::Vol);

    let pillars_1y = get_fx_delta_pillar_vols(&delta_surface, 1).expect("pillar vols at t=1");
    let atm = pillars_1y[0];
    let sp25 = pillars_1y[1];
    let sc25 = pillars_1y[2];
    let sp10 = pillars_1y[3];
    let sc10 = pillars_1y[4];
    assert!((atm - 0.11).abs() < 1e-12);
    assert!((sp25 - (0.11 + 0.003 - 0.0075)).abs() < 1e-12);
    assert!((sc25 - (0.11 + 0.003 + 0.0075)).abs() < 1e-12);
    assert!((sp10 - (0.11 + 0.006 - 0.014)).abs() < 1e-12);
    assert!((sc10 - (0.11 + 0.006 + 0.014)).abs() < 1e-12);

    let k_p10 = delta_to_strike(false, 0.10, fwd_10, sp10, 1.0).expect("put 10d strike");
    let k_p25 = delta_to_strike(false, 0.25, fwd_10, sp25, 1.0).expect("put 25d strike");
    let k_atm = fwd_10 * (0.5 * atm * atm * 1.0).exp();
    let k_c25 = delta_to_strike(true, 0.25, fwd_10, sc25, 1.0).expect("call 25d strike");
    let k_c10 = delta_to_strike(true, 0.10, fwd_10, sc10, 1.0).expect("call 10d strike");

    for &(k, expected) in &[
        (k_p10, sp10),
        (k_p25, sp25),
        (k_atm, atm),
        (k_c25, sc25),
        (k_c10, sc10),
    ] {
        let got = get_fx_delta_vol(&delta_surface, 1.0, k, fwd_10).expect("direct vol at pillar");
        assert!((got - expected).abs() < 1e-12);
        let mat_got = get_surface_vol_clamped(&materialized, 1.0, k);
        assert!((mat_got - expected).abs() < 1e-12);
    }

    // Flat wing extrapolation outside extreme 10d strikes.
    let far_down =
        get_fx_delta_vol(&delta_surface, 1.0, k_p10 * 0.5, fwd_10).expect("downside wing");
    let far_up = get_fx_delta_vol(&delta_surface, 1.0, k_c10 * 1.8, fwd_10).expect("upside wing");
    assert!((far_down - sp10).abs() < 1e-12);
    assert!((far_up - sc10).abs() < 1e-12);
}

#[test]
fn three_point_smile_evaluates_and_roundtrips_delta() {
    let delta_surface = build_surface_3pt("EURUSD-VOL-3PT", &[1.0], &[0.12], &[0.01], &[0.002]);
    let forward = 1.12;
    let pillars = get_fx_delta_pillar_vols(&delta_surface, 0).expect("3-pt pillars");
    assert_eq!(pillars.len(), 3);
    let atm = pillars[0];
    let sp25 = pillars[1];
    let sc25 = pillars[2];

    let k_p25 = delta_to_strike(false, 0.25, forward, sp25, 1.0).expect("put 25d strike");
    let k_atm = forward * (0.5 * atm * atm).exp();
    let k_c25 = delta_to_strike(true, 0.25, forward, sc25, 1.0).expect("call 25d strike");

    assert!(
        (strike_to_delta(false, k_p25, forward, sp25, 1.0).expect("put delta") - 0.25).abs() < 1e-9
    );
    assert!(
        (strike_to_delta(true, k_c25, forward, sc25, 1.0).expect("call delta") - 0.25).abs() < 1e-9
    );

    let vol_put = get_fx_delta_vol(&delta_surface, 1.0, k_p25, forward).expect("put 25d vol");
    let vol_atm = get_fx_delta_vol(&delta_surface, 1.0, k_atm, forward).expect("atm vol");
    let vol_call = get_fx_delta_vol(&delta_surface, 1.0, k_c25, forward).expect("call 25d vol");
    assert!((vol_put - sp25).abs() < 1e-12);
    assert!((vol_atm - atm).abs() < 1e-12);
    assert!((vol_call - sc25).abs() < 1e-12);
}

#[test]
fn strict_expiry_bounds_and_singleton_expiry_behavior() {
    let multi = build_surface_3pt(
        "EURUSD-BOUNDS",
        &[0.5, 1.0],
        &[0.10, 0.12],
        &[0.01, 0.015],
        &[0.002, 0.003],
    );
    let forward = 1.10;
    assert!(get_fx_delta_vol(&multi, 0.5, 1.10, forward).is_ok());
    assert!(get_fx_delta_vol(&multi, 1.0, 1.10, forward).is_ok());
    assert!(get_fx_delta_vol(&multi, 0.49, 1.10, forward).is_err());
    assert!(get_fx_delta_vol(&multi, 1.01, 1.10, forward).is_err());

    let vol_source = VolSource::FxDelta(Arc::new(multi));
    // Clamped lookup extrapolates strike flat beyond wings, but still enforces strict expiry bounds.
    assert!(vol_source.get_fx_vol_clamped(0.5, 0.10, forward).is_ok());
    assert!(vol_source.get_fx_vol_clamped(0.49, 1.10, forward).is_err());
    assert!(vol_source.get_fx_vol_clamped(1.01, 1.10, forward).is_err());

    let single = build_surface_3pt("EURUSD-SINGLE", &[1.0], &[0.12], &[0.01], &[0.002]);
    assert!(get_fx_delta_vol(&single, 1.0, 1.10, forward).is_ok());
    assert!(get_fx_delta_vol(&single, 0.99, 1.10, forward).is_err());
    assert!(get_fx_delta_vol(&single, 1.01, 1.10, forward).is_err());
}

#[test]
fn midpoint_cancellation_and_extreme_scalar_delta_checks() {
    // 5 * 2^-55 cancellation test across tight bracket around 0.125 (= 2^-3, ulp = 2^-55).
    let step = 2.0_f64.powi(-55);
    let t0 = 0.125;
    let t1 = t0 + 8.0 * step;
    let t_query = t0 + 5.0 * step;
    let atm0 = 0.10;
    let atm1 = 0.18;
    let surface = build_surface_3pt(
        "EURUSD-CANCEL",
        &[t0, t1],
        &[atm0, atm1],
        &[0.0, 0.0],
        &[0.0, 0.0],
    );
    let expected_atm = 0.375 * atm0 + 0.625 * atm1;
    let forward = 1.25;
    let k_dns = forward * (0.5 * expected_atm * expected_atm * t_query).exp();
    let got = get_fx_delta_vol(&surface, t_query, k_dns, forward).expect("tight bracket interp");
    assert!((got - expected_atm).abs() < 1e-15);

    // Extreme put tail (|delta_p| = 1e-20) round-trips cleanly within tolerance.
    let k_tail = delta_to_strike(false, 1e-20, 1.0, 0.2, 1.0).expect("deep tail put strike");
    let d_tail = strike_to_delta(false, k_tail, 1.0, 0.2, 1.0).expect("deep tail put delta");
    assert!((d_tail - 1e-20).abs() / 1e-20 < 1e-3);

    // Tiny vol*sqrt(T) = 1e-17 fails delta-roundtrip tolerance when K collapses to F.
    assert!(delta_to_strike(true, 0.25, 1.0, 1e-17, 1.0).is_err());

    // Extreme small forward & large std dev (F = 1e-300, w = 40, put 0.50) avoids overflow/underflow.
    let k_scaled = delta_to_strike(false, 0.5, 1e-300, 40.0, 1.0).expect("extreme forward strike");
    assert!(k_scaled.is_finite() && k_scaled > 0.0);
    let d_scaled =
        strike_to_delta(false, k_scaled, 1e-300, 40.0, 1.0).expect("extreme forward delta");
    assert!((d_scaled - 0.5).abs() < 1e-12);
}

#[test]
fn crossed_interior_bracket_fails_smile_monotonicity() {
    // At t0 = 1.0: 0.5*(1.08^2 - 0.60^2)*1 = 0.3888 < 0.67449*0.60 = 0.4047 (monotone).
    // At t1 = 3.0: 0.5*(0.28^2 - 0.10^2)*3 = 0.1026 < 0.67449*0.10*sqrt(3) = 0.1168 (monotone).
    // At t  = 2.0: 0.5*(0.68^2 - 0.35^2)*2 = 0.3399 > 0.67449*0.35*sqrt(2) = 0.3338 (K_c25 <= K_atm crossed).
    let surface = build_surface_3pt(
        "EURUSD-CROSSED-MID",
        &[1.0, 3.0],
        &[1.08, 0.28],
        &[0.0, 0.0],
        &[-0.48, -0.18],
    );
    let fwd = 1.0;
    assert!(get_fx_delta_vol(&surface, 1.0, 1.0, fwd).is_ok());
    assert!(get_fx_delta_vol(&surface, 3.0, 1.0, fwd).is_ok());
    assert!(get_fx_delta_vol(&surface, 2.0, 1.0, fwd).is_err());
}

#[test]
fn materialized_strike_union_does_not_coalesce_close_strikes() {
    let surface = build_surface_3pt(
        "EURUSD-UNION",
        &[1.0, 2.0],
        &[0.10, 0.10],
        &[0.0, 0.0],
        &[0.0, 0.0],
    );
    let fwd0 = 1.0;
    // Choose fwd1 so K_atm(2.0) differs from K_atm(1.0) by ~1e-12 (relative),
    // verifying exact IEEE-754 strike union without tolerance coalescing.
    let k_atm_0 = fwd0 * (0.5 * 0.10 * 0.10 * 1.0_f64).exp();
    let target_k_atm_1 = k_atm_0 * (1.0 + 1e-12);
    let fwd1 = target_k_atm_1 * (-0.5 * 0.10 * 0.10 * 2.0_f64).exp();
    let materialized =
        materialize_fx_delta_surface(&surface, &[fwd0, fwd1]).expect("materialization succeeds");
    let strikes = materialized.secondary_values();
    let near_k0 = strikes
        .iter()
        .filter(|&&k| (k - k_atm_0).abs() / k_atm_0 < 1e-10)
        .count();
    assert_eq!(
        near_k0, 2,
        "1e-12-separated strikes must both survive union: {strikes:?}"
    );
}

#[test]
fn materialized_vol_grid_diverges_off_knot_from_direct_delta_eval() {
    let surface = build_surface_3pt(
        "EURUSD-DIVERGE",
        &[0.5, 1.5],
        &[0.08, 0.22],
        &[-0.02, 0.03],
        &[0.005, 0.015],
    );
    let fwd0 = 1.10;
    let fwd1 = 1.20;
    let materialized =
        materialize_fx_delta_surface(&surface, &[fwd0, fwd1]).expect("materialization succeeds");
    assert_eq!(materialized.interpolation_mode(), VolInterpolationMode::Vol);

    // Exact match on stored expiry knot with matching delivery forward.
    let k_on_knot = 1.14;
    let on_knot_direct = get_fx_delta_vol(&surface, 0.5, k_on_knot, fwd0).expect("knot direct");
    let on_knot_grid = get_surface_vol_clamped(&materialized, 0.5, k_on_knot);
    assert!((on_knot_direct - on_knot_grid).abs() < 1e-12);

    // Off-knot linear quote interpolation diverges from bilinear Vol-grid interpolation.
    let t_mid = 1.0;
    let fwd_mid = 1.15;
    let k_mid = 1.25;
    let direct_mid = get_fx_delta_vol(&surface, t_mid, k_mid, fwd_mid).expect("direct mid");
    let grid_mid = get_surface_vol_clamped(&materialized, t_mid, k_mid);
    assert!((direct_mid - grid_mid).abs() > 1e-4);
}

#[test]
fn models_analytic_17_table_fixtures_hold() {
    // Fixture 1: exact compensated wing recovery avoids cancellation to zero.
    let (atm, put_wing, call_wing) = interpolate_fx_wing_vols(
        0.5,
        (1.0, -2.5, 0.25_f64.next_up()),
        0.5,
        (1.0_f64.next_up(), -2.5, 0.25),
    )
    .expect("exact compensated wing recovery succeeds");
    assert!((atm - 1.0).abs() <= 2.0_f64.powi(-52));
    assert!((put_wing - 2.5).abs() <= 1e-15);
    assert_eq!(call_wing, 5.0 * 2.0_f64.powi(-55));

    // Fixture 2: endpoint smiles are ordered at T=1 and T=3, but K_put > K_DNS at T=2.
    let crossed = build_surface_3pt(
        "EURUSD-ADR-FIXTURE2",
        &[1.0, 3.0],
        &[0.1, 0.2],
        &[-1.0, -0.6],
        &[0.6, 0.3],
    );
    assert!(get_fx_delta_vol(&crossed, 1.0, 1.0, 1.0).is_ok());
    assert!(get_fx_delta_vol(&crossed, 3.0, 1.0, 1.0).is_ok());
    assert!(get_fx_delta_vol(&crossed, 2.0, 1.0, 1.0).is_err());

    // Fixture 3: direct quote interpolation at (T=2, K=exp(0.04)) returns flat ATM 0.20,
    // while the materialized strike grid returns ~0.20473735.
    let div_surface = build_surface_3pt(
        "EURUSD-ADR-FIXTURE3",
        &[1.0, 3.0],
        &[0.2, 0.2],
        &[0.0, 0.0],
        &[0.05, 0.05],
    );
    let query_k = 0.04_f64.exp();
    let direct = get_fx_delta_vol(&div_surface, 2.0, query_k, 1.0).expect("direct eval at T=2");
    assert!((direct - 0.20).abs() < 1e-12);
    let mat = materialize_fx_delta_surface(&div_surface, &[1.0, 1.0])
        .expect("materialization for ADR fixture 3 succeeds");
    let grid = get_surface_vol_clamped(&mat, 2.0, query_k);
    assert!((grid - 0.20473735).abs() < 1e-6);
}
