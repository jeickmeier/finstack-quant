//! FX delta-quoted volatility surface tests.
//!
//! Regression coverage for merged-strike-grid behavior ("Merged global strike
//! grid flattens short-expiry FX smiles"): each expiry's smile must be built
//! from its *own* pillar strikes
//! (derived from that expiry's forward and vol scale), and queries at a
//! short expiry must reproduce that expiry's own 3/5-point smile exactly —
//! never flat-extrapolated wing vol injected by long-expiry strikes.

use finstack_quant_core::market_data::surfaces::{FxDeltaVolSurface, FxDeltaVolSurfaceBuilder};

const SPOT: f64 = 1.10;
const R_F: f64 = 0.0;
const T_SHORT: f64 = 1.0 / 52.0; // 1W
const T_LONG: f64 = 2.0; // 2Y

/// Domestic rate chosen so the 2Y forward is 1.25 while the 1W forward is
/// ~1.10 — forwards and vol scales differ strongly across pillars, which is
/// exactly the configuration where the old merged grid placed long-expiry
/// strikes far outside the short expiry's smile.
fn r_d() -> f64 {
    (1.25_f64 / SPOT).ln() / T_LONG
}

fn fwd(t: f64) -> f64 {
    SPOT * ((r_d() - R_F) * t).exp()
}

// Market-realistic EUR/USD-style quotes: short ATM 8%, long ATM 12%,
// negative risk reversals (put skew), positive butterflies.
const ATM: [f64; 2] = [0.08, 0.12];
const RR25: [f64; 2] = [-0.005, -0.02];
const BF25: [f64; 2] = [0.0015, 0.004];
const RR10: [f64; 2] = [-0.009, -0.036];
const BF10: [f64; 2] = [0.005, 0.013];

/// Hand-computed 5-point pillar smile (strikes, vols) for expiry index `i`,
/// using only the public delta/strike conversion and the documented smile
/// (broker) strangle convention `sigma_wing = ATM + BF ± RR/2`.
fn pillar_smile_5pt(i: usize) -> ([f64; 5], [f64; 5]) {
    let t = [T_SHORT, T_LONG][i];
    let f = fwd(t);
    let atm = ATM[i];
    let sp25 = atm + BF25[i] - 0.5 * RR25[i];
    let sc25 = atm + BF25[i] + 0.5 * RR25[i];
    let sp10 = atm + BF10[i] - 0.5 * RR10[i];
    let sc10 = atm + BF10[i] + 0.5 * RR10[i];
    // Put strike at |delta| = d corresponds to call delta 1 - d under the
    // premium-unadjusted forward-delta convention.
    let k_p10 = FxDeltaVolSurface::delta_to_strike(0.90, f, sp10, t, R_F);
    let k_p25 = FxDeltaVolSurface::delta_to_strike(0.75, f, sp25, t, R_F);
    let k_atm = f * (0.5 * atm * atm * t).exp();
    let k_c25 = FxDeltaVolSurface::delta_to_strike(0.25, f, sc25, t, R_F);
    let k_c10 = FxDeltaVolSurface::delta_to_strike(0.10, f, sc10, t, R_F);
    (
        [k_p10, k_p25, k_atm, k_c25, k_c10],
        [sp10, sp25, atm, sc25, sc10],
    )
}

fn built_surface_5pt() -> finstack_quant_core::market_data::surfaces::VolSurface {
    FxDeltaVolSurfaceBuilder::new("EURUSD-VOL")
        .spot(SPOT)
        .domestic_rate(r_d())
        .foreign_rate(R_F)
        .expiries(&[T_SHORT, T_LONG])
        .atm_vols(&ATM)
        .rr_25d(&RR25)
        .bf_25d(&BF25)
        .rr_10d(&RR10)
        .bf_10d(&BF10)
        .build()
        .expect("5-point FX delta surface should build")
}

fn delta_surface_5pt() -> FxDeltaVolSurface {
    FxDeltaVolSurface::with_10d(
        "EURUSD-DELTA-VOL",
        vec![T_SHORT, T_LONG],
        ATM.to_vec(),
        RR25.to_vec(),
        BF25.to_vec(),
        RR10.to_vec(),
        BF10.to_vec(),
    )
    .expect("5-point delta surface should build")
}

/// Hand linear interpolation on sorted knots with flat extrapolation —
/// the documented intra-smile scheme.
fn lin_interp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    let n = xs.len();
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let i = xs.partition_point(|&xi| xi < x);
    let t = (x - xs[i - 1]) / (xs[i] - xs[i - 1]);
    ys[i - 1] + t * (ys[i] - ys[i - 1])
}

// ---------------------------------------------------------------------------
// Regression: short-expiry smile is its own 3/5-point smile, not flattened
// wing vol from long-expiry strikes (merged strike grid).
// ---------------------------------------------------------------------------

#[test]
fn short_expiry_smile_not_flattened_by_long_expiry_strikes() {
    let surface = built_surface_5pt();
    let (ks, vs) = pillar_smile_5pt(0);

    // The grid contains strikes from the 2Y smile (around F=1.25, sigma 12%)
    // that lie far outside the 1W smile (around F=1.10, sigma 8%).
    assert!(
        surface.strikes().last().copied().unwrap_or(0.0) > 1.3,
        "test setup: long-expiry strikes should extend the grid well beyond \
         the short smile"
    );

    // The 25Δ-put vol must differ from the 10Δ-put wing vol: the smile has
    // structure between the wings.
    assert!(
        (vs[1] - vs[0]).abs() > 1e-4,
        "test setup: 25Δ-put vol should differ from the 10Δ wing vol"
    );

    // Querying the short expiry at moderate deltas reproduces the short
    // expiry's OWN 5-point linear smile exactly — not flat wing vol.
    let k_25dp = ks[1];
    let v = surface.value_clamped(T_SHORT, k_25dp);
    assert!(
        (v - vs[1]).abs() < 1e-12,
        "25Δ-put vol should be the pillar vol {}, got {v}",
        vs[1]
    );
    assert!(
        (v - vs[0]).abs() > 1e-4,
        "25Δ-put vol must not collapse to the 10Δ wing vol"
    );

    // Strikes strictly between the short expiry's pillars match the hand
    // interpolation of its own smile to 1e-12.
    for k in [
        0.5 * (ks[0] + ks[1]),
        0.5 * (ks[1] + ks[2]),
        0.5 * (ks[2] + ks[3]),
        0.5 * (ks[3] + ks[4]),
        0.25 * ks[1] + 0.75 * ks[2],
    ] {
        let got = surface.value_clamped(T_SHORT, k);
        let want = lin_interp(&ks, &vs, k);
        assert!(
            (got - want).abs() < 1e-12,
            "short-expiry smile at strike {k}: got {got}, want {want} \
             (per-expiry 5-point interpolation)"
        );
    }

    // Beyond the short expiry's own wings the smile is flat at the wing vol
    // (delta-space flat extrapolation), even at strikes contributed by the
    // long expiry.
    let got = surface.value_clamped(T_SHORT, 1.30);
    assert!(
        (got - vs[4]).abs() < 1e-12,
        "beyond the 10Δ call wing the short smile is flat at the wing vol"
    );
}

// ---------------------------------------------------------------------------
// Pillar reproduction: exact pillar strikes return pillar vols to 1e-12 at
// every expiry, on both the materialized surface and the delta-quote query
// path.
// ---------------------------------------------------------------------------

#[test]
fn pillar_strikes_reproduce_pillar_vols_exactly() {
    let surface = built_surface_5pt();
    let delta_surface = delta_surface_5pt();

    for i in 0..2 {
        let t = [T_SHORT, T_LONG][i];
        let f = fwd(t);
        let (ks, vs) = pillar_smile_5pt(i);
        for (k, v) in ks.iter().zip(vs.iter()) {
            let got = surface.value_clamped(t, *k);
            assert!(
                (got - v).abs() < 1e-12,
                "materialized surface, expiry {t}, pillar strike {k}: got {got}, want {v}"
            );

            let got = delta_surface
                .implied_vol(t, *k, f, r_d(), R_F)
                .expect("implied_vol at pillar should succeed");
            assert!(
                (got - v).abs() < 1e-12,
                "implied_vol, expiry {t}, pillar strike {k}: got {got}, want {v}"
            );
        }
    }
}

#[test]
fn pillar_strikes_reproduce_pillar_vols_three_point_smile() {
    // 3-point (25Δ-only) variant of the pillar-reproduction check.
    let surface = FxDeltaVolSurfaceBuilder::new("EURUSD-VOL-3PT")
        .spot(SPOT)
        .domestic_rate(r_d())
        .foreign_rate(R_F)
        .expiries(&[T_SHORT, T_LONG])
        .atm_vols(&ATM)
        .rr_25d(&RR25)
        .bf_25d(&BF25)
        .build()
        .expect("3-point FX delta surface should build");

    for i in 0..2 {
        let t = [T_SHORT, T_LONG][i];
        let f = fwd(t);
        let atm = ATM[i];
        let sp25 = atm + BF25[i] - 0.5 * RR25[i];
        let sc25 = atm + BF25[i] + 0.5 * RR25[i];
        let k_p25 = FxDeltaVolSurface::delta_to_strike(0.75, f, sp25, t, R_F);
        let k_atm = f * (0.5 * atm * atm * t).exp();
        let k_c25 = FxDeltaVolSurface::delta_to_strike(0.25, f, sc25, t, R_F);

        for (k, v) in [(k_p25, sp25), (k_atm, atm), (k_c25, sc25)] {
            let got = surface.value_clamped(t, k);
            assert!(
                (got - v).abs() < 1e-12,
                "3-point surface, expiry {t}, pillar strike {k}: got {got}, want {v}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Off-pillar expiries: the delta-quote query path rebuilds the smile at the
// query expiry's forward (smile-faithful), while the materialized rectangular
// grid blends rows at fixed strike — a documented limitation of the
// materialization, not of the query path.
// ---------------------------------------------------------------------------

#[test]
fn implied_vol_rebuilds_smile_at_intermediate_expiry() {
    let delta_surface = delta_surface_5pt();
    let t = 0.5;
    let f = fwd(t);

    // Quote-space linear interpolation by hand.
    let w = (t - T_SHORT) / (T_LONG - T_SHORT);
    let atm = ATM[0] + w * (ATM[1] - ATM[0]);
    let rr25 = RR25[0] + w * (RR25[1] - RR25[0]);
    let bf25 = BF25[0] + w * (BF25[1] - BF25[0]);
    let k_atm = f * (0.5 * atm * atm * t).exp();

    let got = delta_surface
        .implied_vol(t, k_atm, f, r_d(), R_F)
        .expect("intermediate-expiry implied_vol should succeed");
    assert!(
        (got - atm).abs() < 1e-12,
        "ATM vol at intermediate expiry should be the quote-interpolated ATM \
         {atm}, got {got}"
    );

    // The intermediate smile has genuine put skew around its own forward.
    let sp25 = atm + bf25 - 0.5 * rr25;
    let k_p25 = FxDeltaVolSurface::delta_to_strike(0.75, f, sp25, t, R_F);
    let got_put = delta_surface
        .implied_vol(t, k_p25, f, r_d(), R_F)
        .expect("intermediate-expiry 25Δ-put implied_vol should succeed");
    assert!(
        (got_put - sp25).abs() < 1e-12,
        "25Δ-put vol at intermediate expiry should be {sp25}, got {got_put}"
    );
    assert!(
        got_put > got,
        "negative RR implies put vol above ATM at the intermediate expiry"
    );

    // Documented limitation of the rectangular materialization (core quant
    // convention): off-pillar queries blend rows at fixed strike, so
    // the intermediate ATM vol differs from the smile-faithful query path.
    let surface = built_surface_5pt();
    let grid_atm = surface.value_clamped(t, k_atm);
    assert!(
        (grid_atm - got).abs() > 1e-4,
        "expected the fixed-strike grid blend ({grid_atm}) to differ from the \
         quote-interpolated smile ({got}); if these now agree, the \
         materialization gained quote-space expiry interpolation and this \
         assertion (and the module docs) should be updated"
    );
}
