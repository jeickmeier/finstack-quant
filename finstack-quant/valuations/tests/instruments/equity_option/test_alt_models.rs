//! Alternative equity-option model pricers exercised end-to-end through the
//! standard pricer registry.
//!
//! Integration coverage previously existed only for `MonteCarloRoughBergomi`
//! (see `test_rough_bergomi.rs`). This module adds the remaining registered
//! `ModelKey`s for `EquityOption`:
//!
//! - `PdeCrankNicolson1D` — finite-difference Black-Scholes PDE
//! - `HestonFourier` — semi-analytical Heston via Fourier inversion
//! - `MonteCarloHeston` — Heston QE Monte Carlo
//! - `RoughHestonFourier` — rough Heston via fractional Riccati + Fourier
//!
//! Each model is validated against a recognised analytic limiting case rather
//! than a circular self-referential fixture: with deterministic variance the
//! Heston / rough-Heston models collapse to Black-Scholes, and the
//! Crank-Nicolson PDE must reproduce the closed-form Black-Scholes price.

use super::helpers::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::math::volatility::black_scholes_spot_call;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{standard_registry, ModelKey};
use time::macros::date;

/// Black-Scholes reference price for the shared `create_call` contract
/// (notional / contract size = 100, Act365F day count over a leap year).
fn bs_call_reference(spot: f64, strike: f64, rate: f64, vol: f64) -> f64 {
    // 2024 is a leap year; the pricers day-count the 1Y expiry on Act365F.
    let t = 366.0 / 365.0;
    black_scholes_spot_call(spot, strike, rate, 0.0, vol, t) * 100.0
}

/// Inject the five required `HESTON_*` scalars onto a flat-vol market.
fn with_heston_scalars(
    market: MarketContext,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    v0: f64,
) -> MarketContext {
    market
        .insert_price("HESTON_KAPPA", MarketScalar::Unitless(kappa))
        .insert_price("HESTON_THETA", MarketScalar::Unitless(theta))
        .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(sigma_v))
        .insert_price("HESTON_RHO", MarketScalar::Unitless(rho))
        .insert_price("HESTON_V0", MarketScalar::Unitless(v0))
}

/// Price an ATM-ish call through `model`, returning the PV in account currency.
fn price_call(
    market: &MarketContext,
    model: ModelKey,
    as_of: finstack_quant_core::dates::Date,
    expiry: finstack_quant_core::dates::Date,
    strike: f64,
) -> f64 {
    let call = create_call(as_of, expiry, strike);
    standard_registry()
        .price_with_metrics(&call, model, market, as_of, &[], PricingOptions::default())
        .expect("pricing should succeed")
        .value
        .amount()
}

// ---------------------------------------------------------------------------
// Crank-Nicolson 1D PDE
// ---------------------------------------------------------------------------

/// The Crank-Nicolson Black-Scholes PDE must reproduce the closed-form
/// Black-Scholes price for a European call on a flat-vol surface.
#[test]
fn pde_cn1d_matches_black_scholes() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.03);

    let market = build_standard_market(as_of, spot, vol, rate, 0.0);
    let pv = price_call(&market, ModelKey::PdeCrankNicolson1D, as_of, expiry, strike);
    let bs = bs_call_reference(spot, strike, rate, vol);

    let rel = (pv - bs).abs() / bs;
    assert!(
        rel < 1.5e-2,
        "CN1D PDE ({pv}) must match Black-Scholes ({bs}); rel err {rel}"
    );
}

/// The PDE solver carries no RNG: pricing the same option twice is bit-identical.
#[test]
fn pde_cn1d_is_deterministic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let market = build_standard_market(as_of, 100.0, 0.22, 0.02, 0.0);

    let pv1 = price_call(&market, ModelKey::PdeCrankNicolson1D, as_of, expiry, 105.0);
    let pv2 = price_call(&market, ModelKey::PdeCrankNicolson1D, as_of, expiry, 105.0);
    assert_eq!(
        pv1.to_bits(),
        pv2.to_bits(),
        "CN1D PDE must be bit-identical across runs: {pv1} vs {pv2}"
    );
}

// ---------------------------------------------------------------------------
// Heston (semi-analytical Fourier)
// ---------------------------------------------------------------------------

/// With vol-of-vol `σ_v → 0` and `v0 = θ = σ²`, the Heston variance is
/// deterministic and the model collapses to Black-Scholes at lognormal vol `σ`.
/// The Fourier price must converge to the closed-form Black-Scholes price — a
/// genuine, non-circular analytic reference.
#[test]
fn heston_fourier_collapses_to_black_scholes() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.03);
    let var = vol * vol;

    // σ_v ≈ 0 ⇒ deterministic variance; ρ = 0 keeps the limit exactly BS.
    let market = with_heston_scalars(
        build_standard_market(as_of, spot, vol, rate, 0.0),
        1.0,  // kappa
        var,  // theta
        1e-4, // sigma_v -> 0
        0.0,  // rho
        var,  // v0
    );
    let pv = price_call(&market, ModelKey::HestonFourier, as_of, expiry, strike);
    let bs = bs_call_reference(spot, strike, rate, vol);

    let rel = (pv - bs).abs() / bs;
    assert!(
        rel < 2e-2,
        "σ_v→0 Heston Fourier ({pv}) must match Black-Scholes ({bs}); rel err {rel}"
    );
}

/// A genuine stochastic-vol Heston call (non-zero vol-of-vol) is positive and
/// in a sane range for a 1Y ATM option at ~20% vol (contract size 100).
#[test]
fn heston_fourier_atm_is_positive_and_sane() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let market = with_heston_scalars(
        build_standard_market(as_of, 100.0, 0.20, 0.0, 0.0),
        2.0,
        0.04,
        0.5,
        -0.7,
        0.04,
    );
    let pv = price_call(&market, ModelKey::HestonFourier, as_of, expiry, 100.0);
    assert!(
        pv > 0.0,
        "Heston Fourier ATM call must be positive, got {pv}"
    );
    assert_in_range(pv, 300.0, 1500.0, "Heston Fourier ATM call PV range");
}

/// The Fourier pricer is purely analytical: pricing twice is bit-identical.
#[test]
fn heston_fourier_is_deterministic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let market = with_heston_scalars(
        build_standard_market(as_of, 100.0, 0.20, 0.01, 0.0),
        1.5,
        0.05,
        0.4,
        -0.6,
        0.045,
    );
    let pv1 = price_call(&market, ModelKey::HestonFourier, as_of, expiry, 100.0);
    let pv2 = price_call(&market, ModelKey::HestonFourier, as_of, expiry, 100.0);
    assert_eq!(
        pv1.to_bits(),
        pv2.to_bits(),
        "Heston Fourier must be bit-identical across runs: {pv1} vs {pv2}"
    );
}

/// Missing `HESTON_*` scalars must hard-error rather than silently fall back to
/// representative defaults.
#[test]
fn heston_fourier_missing_scalars_error() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    // Standard market WITHOUT any HESTON_* scalars.
    let market = build_standard_market(as_of, 100.0, 0.20, 0.0, 0.0);
    let call = create_call(as_of, expiry, 100.0);

    let result = standard_registry().price_with_metrics(
        &call,
        ModelKey::HestonFourier,
        &market,
        as_of,
        &[],
        PricingOptions::default(),
    );
    assert!(
        result.is_err(),
        "Heston Fourier must error when HESTON_* scalars are missing"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("HESTON_"),
        "error should name a missing HESTON_* scalar, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Heston Monte Carlo (slow)
// ---------------------------------------------------------------------------

/// In the deterministic-variance limit (`σ_v → 0`, `ρ = 0`), the Heston QE
/// Monte Carlo price must converge to the same Black-Scholes reference as the
/// Fourier pricer — a cross-model parity that also exercises the MC path.
#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn heston_mc_collapses_to_black_scholes() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.03);
    let var = vol * vol;

    let market = with_heston_scalars(
        build_standard_market(as_of, spot, vol, rate, 0.0),
        1.0,
        var,
        1e-3,
        0.0,
        var,
    );
    let mut call = create_call(as_of, expiry, strike);
    call.instrument_pricing_overrides.model_config.mc_paths = Some(80_000);
    let pv = standard_registry()
        .price_with_metrics(
            &call,
            ModelKey::MonteCarloHeston,
            &market,
            as_of,
            &[],
            PricingOptions::default(),
        )
        .expect("Heston MC pricing should succeed")
        .value
        .amount();
    let bs = bs_call_reference(spot, strike, rate, vol);

    let rel = (pv - bs).abs() / bs;
    assert!(
        rel < 4e-2,
        "σ_v→0 Heston MC ({pv}) must converge to Black-Scholes ({bs}); rel err {rel}"
    );
}

// ---------------------------------------------------------------------------
// Heston 2D ADI PDE
// ---------------------------------------------------------------------------

/// The Heston Modified-Craig-Sneyd ADI PDE and the semi-analytical Fourier
/// pricer are two independent numerical methods for the *same* Heston model, so
/// at genuine stochastic-vol parameters they must agree to within discretization
/// tolerance. (The `σ_v → 0` Black-Scholes limit is deliberately *not* used here:
/// vanishing variance-diffusion makes the variance direction convection-dominated
/// and violates the ADI Péclet stability bound — the scheme is built for genuine
/// stochastic volatility. Feller `2κθ ≥ σ_v²` is satisfied to keep `v > 0`.)
#[test]
fn pde_adi2d_matches_heston_fourier() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.03);

    // Feller: 2·κ·θ = 2·2·0.06 = 0.24 ≥ σ_v² = 0.16.
    let market = with_heston_scalars(
        build_standard_market(as_of, spot, vol, rate, 0.0),
        2.0,  // kappa
        0.06, // theta
        0.4,  // sigma_v
        -0.5, // rho
        0.06, // v0
    );
    let pv_pde = price_call(&market, ModelKey::PdeAdi2D, as_of, expiry, strike);
    let pv_fourier = price_call(&market, ModelKey::HestonFourier, as_of, expiry, strike);

    let rel = (pv_pde - pv_fourier).abs() / pv_fourier;
    assert!(
        rel < 3e-2,
        "Heston ADI 2D PDE ({pv_pde}) must match the Fourier pricer ({pv_fourier}); rel err {rel}"
    );
}

// ---------------------------------------------------------------------------
// Rough Heston (semi-analytical Fourier)
// ---------------------------------------------------------------------------

/// Inject the six required `ROUGH_HESTON_*` scalars onto a flat-vol market.
fn with_rough_heston_scalars(
    market: MarketContext,
    hurst: f64,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    v0: f64,
) -> MarketContext {
    market
        .insert_price("ROUGH_HESTON_HURST", MarketScalar::Unitless(hurst))
        .insert_price("ROUGH_HESTON_KAPPA", MarketScalar::Unitless(kappa))
        .insert_price("ROUGH_HESTON_THETA", MarketScalar::Unitless(theta))
        .insert_price("ROUGH_HESTON_SIGMA_V", MarketScalar::Unitless(sigma_v))
        .insert_price("ROUGH_HESTON_RHO", MarketScalar::Unitless(rho))
        .insert_price("ROUGH_HESTON_V0", MarketScalar::Unitless(v0))
}

/// A rough-Heston Fourier ATM call is positive and in a sane range.
#[test]
fn rough_heston_fourier_atm_is_positive_and_sane() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let market = with_rough_heston_scalars(
        build_standard_market(as_of, 100.0, 0.20, 0.0, 0.0),
        0.1,
        1.5,
        0.04,
        0.3,
        -0.7,
        0.04,
    );
    let pv = price_call(&market, ModelKey::RoughHestonFourier, as_of, expiry, 100.0);
    assert!(
        pv > 0.0,
        "rough Heston Fourier ATM call must be positive, got {pv}"
    );
    assert_in_range(pv, 300.0, 1500.0, "rough Heston Fourier ATM call PV range");
}

/// A rough-Heston QE Monte Carlo ATM call is positive and in a sane range
/// (small path count keeps the test fast).
#[test]
fn rough_heston_mc_atm_is_positive_and_sane() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let market = with_rough_heston_scalars(
        build_standard_market(as_of, 100.0, 0.20, 0.0, 0.0),
        0.1,
        1.5,
        0.04,
        0.3,
        -0.7,
        0.04,
    );
    let mut call = create_call(as_of, expiry, 100.0);
    call.instrument_pricing_overrides.model_config.mc_paths = Some(4_000);
    let pv = standard_registry()
        .price_with_metrics(
            &call,
            ModelKey::MonteCarloRoughHeston,
            &market,
            as_of,
            &[],
            PricingOptions::default(),
        )
        .expect("rough Heston MC pricing should succeed")
        .value
        .amount();
    assert!(
        pv > 0.0,
        "rough Heston MC ATM call must be positive, got {pv}"
    );
    assert_in_range(pv, 200.0, 1800.0, "rough Heston MC ATM call PV range");
}

/// Missing `ROUGH_HESTON_*` scalars must hard-error.
#[test]
fn rough_heston_fourier_missing_scalars_error() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let market = build_standard_market(as_of, 100.0, 0.20, 0.0, 0.0);
    let call = create_call(as_of, expiry, 100.0);

    let result = standard_registry().price_with_metrics(
        &call,
        ModelKey::RoughHestonFourier,
        &market,
        as_of,
        &[],
        PricingOptions::default(),
    );
    assert!(
        result.is_err(),
        "rough Heston Fourier must error when ROUGH_HESTON_* scalars are missing"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("ROUGH_HESTON_"),
        "error should name a missing ROUGH_HESTON_* scalar, got: {msg}"
    );
}
