use finstack_quant_core::market_data::surfaces::{VolCube, VolInterpolationMode};
use finstack_quant_core::math::volatility::sabr::SabrParams;

#[test]
fn test_vol_cube_builder_basic() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::builder("USD-SWAPTION")
        .expiries(&[1.0, 5.0])
        .tenors(&[2.0, 10.0])
        .node(p, 0.03)
        .node(p, 0.035)
        .node(p, 0.04)
        .node(p, 0.045)
        .build()
        .unwrap();

    assert_eq!(cube.id().as_str(), "USD-SWAPTION");
    assert_eq!(cube.expiries(), &[1.0, 5.0]);
    assert_eq!(cube.tenors(), &[2.0, 10.0]);
    assert_eq!(cube.grid_shape(), (2, 2));
}

#[test]
fn test_vol_cube_from_grid() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let params = vec![p; 4];
    let forwards = vec![0.03, 0.035, 0.04, 0.045];
    let cube = VolCube::from_grid("TEST", &[1.0, 5.0], &[2.0, 10.0], &params, &forwards).unwrap();
    assert_eq!(cube.grid_shape(), (2, 2));
}

#[test]
fn test_vol_cube_validation_rejects_bad_input() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    // Wrong number of params
    let result = VolCube::from_grid("BAD", &[1.0, 5.0], &[2.0, 10.0], &[p; 3], &[0.03; 3]);
    assert!(result.is_err());
    // Unsorted expiries
    let result = VolCube::from_grid("BAD", &[5.0, 1.0], &[2.0, 10.0], &[p; 4], &[0.03; 4]);
    assert!(result.is_err());
}

#[test]
fn test_vol_cube_serde_roundtrips_interpolation_mode_and_grid_state() {
    let p0 = SabrParams::new(0.030, 0.5, -0.2, 0.4).unwrap();
    let p1 = SabrParams::new(0.040, 0.5, -0.1, 0.5).unwrap();
    let cube = VolCube::from_grid(
        "USD-SWAPTION",
        &[1.0, 5.0],
        &[2.0, 10.0],
        &[p0, p1, p0, p1],
        &[0.030, 0.035, 0.040, 0.045],
    )
    .unwrap()
    .with_interpolation_mode(VolInterpolationMode::TotalVariance);

    let json = serde_json::to_string(&cube).unwrap();
    assert!(
        json.contains("\"interpolation_mode\":\"total_variance\""),
        "serialized cube should preserve the interpolation mode: {json}"
    );

    let roundtrip: VolCube = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.id(), cube.id());
    assert_eq!(roundtrip.expiries(), cube.expiries());
    assert_eq!(roundtrip.tenors(), cube.tenors());
    assert_eq!(roundtrip.grid_shape(), cube.grid_shape());
    assert_eq!(roundtrip.params_at(0, 1), cube.params_at(0, 1));
    assert_eq!(roundtrip.forward_at(1, 0), cube.forward_at(1, 0));

    let serialized = serde_json::to_value(roundtrip).unwrap();
    assert_eq!(serialized["interpolation_mode"], "total_variance");
}

#[test]
fn test_vol_cube_serde_defaults_legacy_missing_interpolation_mode_to_vol() {
    let json = r#"{
        "id": "LEGACY-CUBE",
        "expiries": [1.0],
        "tenors": [5.0],
        "params": [{"alpha": 0.035, "beta": 0.5, "rho": -0.2, "nu": 0.4}],
        "forwards": [0.03]
    }"#;

    let cube: VolCube = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(cube).unwrap();

    assert_eq!(serialized["interpolation_mode"], "vol");
}

#[test]
fn test_vol_cube_serde_rejects_unknown_fields() {
    let json = r#"{
        "id": "STRICT-CUBE",
        "expiries": [1.0],
        "tenors": [5.0],
        "params": [{"alpha": 0.035, "beta": 0.5, "rho": -0.2, "nu": 0.4}],
        "forwards": [0.03],
        "unknown": true
    }"#;

    let result = serde_json::from_str::<VolCube>(json);
    assert!(
        result.is_err(),
        "VolCube wire format should reject unknown fields"
    );
}

#[test]
fn test_vol_cube_vol_at_grid_node() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let fwd = 0.03;
    let cube = VolCube::from_grid("TEST", &[1.0], &[5.0], &[p], &[fwd]).unwrap();
    let strike = 0.03;
    let vol = cube.vol(1.0, 5.0, strike).unwrap();
    let expected = p.implied_vol_lognormal(fwd, strike, 1.0);
    assert!(
        (vol - expected).abs() < 1e-14,
        "grid-node vol {vol} != direct SABR {expected}"
    );
}

#[test]
fn test_vol_cube_vol_interpolated() {
    let p_lo = SabrParams::new(0.020, 0.5, -0.2, 0.3).unwrap();
    let p_hi = SabrParams::new(0.050, 0.5, -0.2, 0.5).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p_lo, p_lo, p_hi, p_hi],
        &[0.03, 0.04, 0.03, 0.04],
    )
    .unwrap();
    let strike = 0.035;
    let vol_mid = cube.vol(3.0, 7.5, strike).unwrap();
    assert!(vol_mid.is_finite() && vol_mid > 0.0);
}

#[test]
fn test_vol_cube_vol_clamped_extrapolation() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("TEST", &[1.0, 5.0], &[5.0, 10.0], &[p; 4], &[0.03; 4]).unwrap();
    let vol = cube.vol_clamped(0.1, 2.0, 0.03);
    assert!(vol.is_finite() && vol > 0.0);
}

#[test]
fn test_vol_cube_vol_checked_out_of_bounds() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("TEST", &[1.0, 5.0], &[5.0, 10.0], &[p; 4], &[0.03; 4]).unwrap();
    assert!(cube.vol(0.1, 7.0, 0.03).is_err());
    assert!(cube.vol(3.0, 2.0, 0.03).is_err());
}

#[test]
fn test_vol_cube_materialize_tenor_slice() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p; 4],
        &[0.03, 0.035, 0.04, 0.045],
    )
    .unwrap();
    let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
    let surface = cube.materialize_tenor_slice(5.0, &strikes).unwrap();
    assert_eq!(surface.expiries(), &[1.0, 5.0]);
    assert_eq!(surface.strikes(), &strikes[..]);
    let cube_vol = cube.vol(1.0, 5.0, 0.03).unwrap();
    let surf_vol = surface.value_checked(1.0, 0.03).unwrap();
    assert!(
        (cube_vol - surf_vol).abs() < 1e-14,
        "materialized surface vol {surf_vol} != cube vol {cube_vol}"
    );
}

#[test]
fn test_vol_cube_materialize_expiry_slice() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p; 4],
        &[0.03, 0.035, 0.04, 0.045],
    )
    .unwrap();
    let strikes = vec![0.02, 0.03, 0.04];
    let surface = cube.materialize_expiry_slice(1.0, &strikes).unwrap();
    assert_eq!(surface.expiries(), &[5.0, 10.0]);
    assert_eq!(surface.strikes(), &strikes[..]);
}

#[test]
fn test_vol_cube_materialize_grid_flattens_expiry_tenor_strike_order() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p; 4],
        &[0.03, 0.035, 0.04, 0.045],
    )
    .unwrap();
    let strikes = [0.02, 0.03, 0.04];

    let grid = cube.materialize_grid(&strikes).expect("materializes");

    assert_eq!(grid.len(), 2 * 2 * strikes.len());
    let expected_first = cube.vol(1.0, 5.0, strikes[0]).expect("cube vol");
    let expected_second_strike = cube.vol(1.0, 5.0, strikes[1]).expect("cube vol");
    let expected_next_tenor = cube.vol(1.0, 10.0, strikes[0]).expect("cube vol");
    assert!((grid[0] - expected_first).abs() < 1e-14);
    assert!((grid[1] - expected_second_strike).abs() < 1e-14);
    assert!((grid[strikes.len()] - expected_next_tenor).abs() < 1e-14);

    assert!(cube.materialize_grid(&[]).is_err());
}

// ---------------------------------------------------------------------------
// VolProvider trait tests (Task 5)
// ---------------------------------------------------------------------------

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::traits::VolProvider;

#[test]
fn test_vol_provider_cube() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("TEST", &[1.0, 5.0], &[5.0, 10.0], &[p; 4], &[0.03; 4]).unwrap();
    let provider: &dyn VolProvider = &cube;
    let vol = provider.vol(1.0, 5.0, 0.03).unwrap();
    assert!(vol.is_finite() && vol > 0.0);
}

#[test]
fn test_vol_provider_surface_ignores_tenor() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[0.02, 0.03, 0.04])
        .row(&[0.20, 0.21, 0.22])
        .row(&[0.19, 0.20, 0.21])
        .build()
        .unwrap();
    let provider: &dyn VolProvider = &surface;
    let vol_a = provider.vol(1.5, 5.0, 0.03).unwrap();
    let vol_b = provider.vol(1.5, 999.0, 0.03).unwrap();
    assert!(
        (vol_a - vol_b).abs() < 1e-14,
        "VolSurface should ignore tenor"
    );
}

// ---------------------------------------------------------------------------
// MarketContext integration tests (Task 6)
// ---------------------------------------------------------------------------

#[test]
fn test_market_context_vol_cube_insert_and_get() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("USD-SWPT", &[1.0], &[5.0], &[p], &[0.03]).unwrap();
    let ctx = MarketContext::new().insert_vol_cube(cube);
    let retrieved = ctx.get_vol_cube("USD-SWPT").unwrap();
    assert_eq!(retrieved.id().as_str(), "USD-SWPT");
}

#[test]
fn test_market_context_vol_provider_finds_cube() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("USD-SWPT", &[1.0], &[5.0], &[p], &[0.03]).unwrap();
    let ctx = MarketContext::new().insert_vol_cube(cube);
    let provider = ctx.get_vol_provider("USD-SWPT").unwrap();
    let vol = provider.vol(1.0, 5.0, 0.03).unwrap();
    assert!(vol.is_finite() && vol > 0.0);
}

#[test]
fn test_market_context_vol_provider_falls_back_to_surface() {
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2, 0.2])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert_surface(surface);
    let provider = ctx.get_vol_provider("EQ-VOL").unwrap();
    let vol = provider.vol_clamped(1.5, 999.0, 95.0);
    assert!(vol > 0.0);
}

#[test]
fn test_market_context_stats_includes_vol_cubes() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid("TEST", &[1.0], &[5.0], &[p], &[0.03]).unwrap();
    let ctx = MarketContext::new().insert_vol_cube(cube);
    assert_eq!(ctx.stats().vol_cube_count, 1);
}

// ---------------------------------------------------------------------------
// Normal (Bachelier) vol quoting tests
// ---------------------------------------------------------------------------

use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::math::volatility::{
    black_call, black_shifted_call, implied_vol_bachelier,
};

/// Positive-rates consistency: the Hagan normal vol from the cube must agree
/// with the exact Bachelier vol implied from the Black price computed with the
/// cube's lognormal quote. Both Hagan expansions are O(T) asymptotic
/// approximations of the same SABR dynamics truncated differently, so the two
/// quotes are price-equivalent only up to the expansion error — a 1% relative
/// tolerance comfortably covers it for moderate vols/expiries.
#[test]
fn test_vol_cube_vol_normal_price_consistency_positive_rates() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let fwd = 0.03;
    let cube = VolCube::from_grid("TEST", &[1.0], &[5.0], &[p], &[fwd]).unwrap();
    let t = 1.0;

    for strike in [0.02, 0.025, 0.03, 0.035, 0.045] {
        let sigma_ln = cube.vol(1.0, 5.0, strike).unwrap();
        let sigma_n = cube.vol_normal(1.0, 5.0, strike).unwrap();
        assert!(sigma_n.is_finite() && sigma_n > 0.0);

        // Exact conversion through price space
        let price = black_call(fwd, strike, sigma_ln, t);
        let sigma_n_exact = implied_vol_bachelier(price, fwd, strike, t, true).unwrap();

        let rel = (sigma_n - sigma_n_exact).abs() / sigma_n_exact;
        assert!(
            rel < 0.01,
            "strike {strike}: Hagan normal {sigma_n} vs price-implied {sigma_n_exact} (rel {rel})"
        );
    }
}

/// Negative-rate point with shifted SABR: the normal vol is finite/positive
/// and price-consistent with the lognormal quote evaluated on the shifted
/// forward/strike (shifted-Black price -> exact Bachelier implied vol).
/// Tolerance as above: the two Hagan expansions differ at their truncation
/// order, ~1% relative for these inputs.
#[test]
fn test_vol_cube_vol_normal_negative_rates_with_shift() {
    let shift = 0.02;
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4)
        .unwrap()
        .with_shift(shift);
    let fwd = -0.005;
    let cube = VolCube::from_grid("EUR-SWPT", &[1.0], &[5.0], &[p], &[fwd]).unwrap();
    let t = 1.0;
    let strike = -0.002;

    let sigma_n = cube.vol_normal(1.0, 5.0, strike).unwrap();
    assert!(sigma_n.is_finite() && sigma_n > 0.0);

    // Lognormal quote applies to the shifted model: price with shifted Black.
    let sigma_ln = cube.vol(1.0, 5.0, strike).unwrap();
    let price = black_shifted_call(fwd, strike, sigma_ln, t, shift);
    let sigma_n_exact = implied_vol_bachelier(price, fwd, strike, t, true).unwrap();

    let rel = (sigma_n - sigma_n_exact).abs() / sigma_n_exact;
    assert!(
        rel < 0.01,
        "Hagan normal {sigma_n} vs shifted-Black price-implied {sigma_n_exact} (rel {rel})"
    );
}

/// ATM: sigma_N ~= sigma_LN * F (standard first-order ATM approximation; the
/// exact relation carries a 1 - sigma_LN^2 T / 24 correction, ~0.2% here, so
/// a 1% relative tolerance is ample).
#[test]
fn test_vol_cube_vol_normal_atm_approximation() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let fwd = 0.03;
    let cube = VolCube::from_grid("TEST", &[1.0], &[5.0], &[p], &[fwd]).unwrap();

    let sigma_ln = cube.vol(1.0, 5.0, fwd).unwrap();
    let sigma_n = cube.vol_normal(1.0, 5.0, fwd).unwrap();

    let approx = sigma_ln * fwd;
    let rel = (sigma_n - approx).abs() / approx;
    assert!(
        rel < 0.01,
        "ATM normal {sigma_n} vs lognormal*F {approx} (rel {rel})"
    );
}

/// Degenerate point: beta > 0 with a cross-zero (negative forward, positive
/// strike) quote and no shift is refused by the checked path and floored by
/// the clamped path. The normal floor is in absolute rate units:
/// 1e-8 * max(|F|, 1) = 1e-8 here.
#[test]
fn test_vol_cube_vol_normal_floor_on_degenerate_point() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let fwd = -0.01; // cross-zero vs positive strike, beta = 0.5, no shift
    let cube = VolCube::from_grid("BAD", &[1.0], &[5.0], &[p], &[fwd]).unwrap();

    assert!(cube.vol_normal(1.0, 5.0, 0.02).is_err());

    let floored = cube.vol_normal_clamped(1.0, 5.0, 0.02);
    assert!(
        (floored - 1e-8).abs() < 1e-20,
        "expected normal-vol floor 1e-8, got {floored}"
    );
}

#[test]
fn test_vol_cube_materialize_tenor_slice_normal() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p; 4],
        &[0.03, 0.035, 0.04, 0.045],
    )
    .unwrap();
    let strikes = vec![0.02, 0.03, 0.04];
    let surface = cube.materialize_tenor_slice_normal(5.0, &strikes).unwrap();
    assert_eq!(surface.quote_type(), VolQuoteType::Normal);
    assert_eq!(surface.expiries(), &[1.0, 5.0]);

    let cube_vol = cube.vol_normal(1.0, 5.0, 0.03).unwrap();
    let surf_vol = surface.value_checked(1.0, 0.03).unwrap();
    assert!(
        (cube_vol - surf_vol).abs() < 1e-14,
        "materialized normal surface vol {surf_vol} != cube normal vol {cube_vol}"
    );
}

#[test]
fn test_vol_cube_materialize_expiry_slice_normal() {
    let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::from_grid(
        "TEST",
        &[1.0, 5.0],
        &[5.0, 10.0],
        &[p; 4],
        &[0.03, 0.035, 0.04, 0.045],
    )
    .unwrap();
    let strikes = vec![0.02, 0.03, 0.04];
    let surface = cube.materialize_expiry_slice_normal(1.0, &strikes).unwrap();
    assert_eq!(surface.quote_type(), VolQuoteType::Normal);
    assert_eq!(surface.expiries(), &[5.0, 10.0]);
}
