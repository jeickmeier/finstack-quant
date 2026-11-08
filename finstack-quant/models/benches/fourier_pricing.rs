//! Benchmarks for product-independent Fourier-pricing hot paths.
//!
//! These cover optimizations that the analytic `option_pricing` / `swaption_pricing`
//! benches do not exercise:
//!
//! - **COS strip pricing** (`fourier::cos`): the strike-independent coefficient
//!   `aₖ = Re[φ(uₖ)·exp(-i·uₖ·a)]` is precomputed once per strip instead of once
//!   per strike inside `put_price`.
//! - **Heston Fourier scalar batches** (`closed_form::heston_call_price_fourier`):
//!   each strike invokes the scalar API independently, including its adaptive
//!   grid construction and characteristic-function precomputation. This does
//!   not measure the internal shared-strip implementation.
//!
//! Fixtures and financial checks run only for selected Criterion cases, before
//! timing. The COS timer returns its full price vector; Heston consumes a single
//! scalar price directly or every scalar price and its sum, without adding a
//! batch output allocation.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_models::closed_form::{
    HestonFourierSettings, HestonPricingParams, bs_price, heston_call_price_fourier,
};
use finstack_quant_models::fourier::characteristic_function::BlackScholesCf;
use finstack_quant_models::fourier::cos::{CosConfig, CosPricer};
use finstack_quant_models::types::OptionType;
use std::hint::black_box;

/// Linearly spaced strikes from 70% to 130% of `spot`, including both endpoints.
fn make_strikes(spot: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| spot * (0.7 + 0.6 * (i as f64) / ((n - 1).max(1) as f64)))
        .collect()
}

/// Check every call against discounted no-arbitrage bounds and strike ordering.
fn validate_call_prices(spot: f64, strikes: &[f64], prices: &[f64], r: f64, q: f64, time: f64) {
    assert_eq!(prices.len(), strikes.len(), "one price per strike");
    let discounted_spot = spot * (-q * time).exp();
    let discount = (-r * time).exp();
    let tolerance = 1e-9 * discounted_spot;
    for (&strike, &price) in strikes.iter().zip(prices) {
        let lower_bound = (discounted_spot - strike * discount).max(0.0);
        assert!(price.is_finite(), "non-finite call price at K={strike}");
        assert!(
            price >= lower_bound - tolerance && price <= discounted_spot + tolerance,
            "call at K={strike} violates discounted bounds: {price}"
        );
    }
    assert!(
        strikes.windows(2).all(|pair| pair[0] < pair[1]),
        "fixture strikes must increase"
    );
    assert!(
        prices.windows(2).all(|pair| pair[0] + tolerance >= pair[1]),
        "calls must be non-increasing in strike"
    );
}

/// Compare fixture landmarks with a deliberately wider, finer initial grid.
fn validate_heston_convergence(
    spot: f64,
    strikes: &[f64],
    prices: &[f64],
    time: f64,
    params: &HestonPricingParams,
) {
    // This is a grid-convergence control using the canonical CF and checked
    // adaptive driver, not an independent Heston implementation. Three
    // landmarks bound untimed setup work even for the 256-strike case.
    let settings = HestonFourierSettings::new(400.0, 800, 16)
        .expect("valid Heston convergence-control settings");
    let mut landmarks = vec![0, strikes.len() / 2, strikes.len() - 1];
    landmarks.dedup();
    for index in landmarks {
        let strike = strikes[index];
        let reference = heston_call_price_fourier(spot, strike, time, params, Some(&settings))
            .expect("converged Heston landmark reference");
        assert!(reference.is_finite(), "non-finite Heston reference");
        // Both drivers require 1e-9 discounted-spot agreement between grids;
        // leave one order of magnitude for their different starting grids.
        assert!(
            (prices[index] - reference).abs() <= 1e-8 * spot,
            "Heston grid disagreement at K={strike}: {} vs {reference}",
            prices[index]
        );
    }
}

/// Check a resolved-grid reference and the independent Black-Scholes limit.
fn validate_heston_controls() {
    let params = HestonPricingParams::new(0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04)
        .expect("valid Heston short-expiry control parameters");
    // The reference in models/src/closed_form/heston/tests.rs comes from a
    // 20,000-frequency, 320,000-node integration, independent of default grids.
    let short_expiry = heston_call_price_fourier(100.0, 100.0, 0.001, &params, None)
        .expect("converged Heston short-expiry control");
    assert!(
        short_expiry.is_finite() && (short_expiry - 0.254_803_543_342).abs() < 1e-9,
        "Heston short-expiry reference disagreement: {short_expiry}"
    );

    let params = HestonPricingParams::new(0.03, 0.01, 1.5, 0.04, 1e-10, -0.6, 0.04)
        .expect("valid near-deterministic Heston control parameters");
    let heston = heston_call_price_fourier(100.0, 100.0, 1.0, &params, None)
        .expect("converged near-deterministic Heston control");
    let reference = bs_price(100.0, 100.0, 0.03, 0.01, 0.2, 1.0, OptionType::Call)
        .expect("valid Black-Scholes limit control");
    // This is an independent closed-form control, with the O(sigma_v)
    // correction bound used by the model's deterministic-limit regression.
    assert!(
        heston.is_finite() && (heston - reference).abs() < 20.0 * params.model.sigma_v + 1e-9,
        "Heston deterministic-limit disagreement: {heston} vs {reference}"
    );
}

// COS strip pricing (#13): strike-independent coefficient reuse.
fn bench_cos_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("cos_strip");

    for &n in &[16_usize, 64, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let spot = 100.0;
            let (r, q, sigma, t) = (0.03_f64, 0.01_f64, 0.20_f64, 1.0_f64);
            let cf = BlackScholesCf { r, q, sigma };
            let pricer = CosPricer::new(&cf, CosConfig::default());
            let strikes = make_strikes(spot, n);
            let prices = pricer
                .price_strip(spot, &strikes, r, t, OptionType::Call)
                .expect("valid COS preflight prices");
            validate_call_prices(spot, &strikes, &prices, r, q, t);
            for (&strike, &price) in strikes.iter().zip(&prices) {
                let reference = bs_price(spot, strike, r, q, sigma, t, OptionType::Call)
                    .expect("valid Black-Scholes reference");
                assert!(
                    reference.is_finite() && (price - reference).abs() < 1e-6,
                    "COS disagrees with Black-Scholes at K={strike}: {price} vs {reference}"
                );
            }

            b.iter(|| {
                black_box(
                    pricer
                        .price_strip(spot, black_box(&strikes), r, t, OptionType::Call)
                        .expect("COS call prices"),
                )
            });
        });
    }
    group.finish();
}

// Repeated scalar Heston pricing; no cross-strike cache is shared.
fn bench_heston_fourier_scalar_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("heston_fourier_scalar_batch");

    for &n in &[1_usize, 16, 64, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let spot = 100.0;
            let time = 1.0_f64;
            let params = HestonPricingParams::new(0.03, 0.01, 1.5, 0.04, 0.3, -0.6, 0.04)
                .expect("valid benchmark Heston parameters");
            let strikes = if n == 1 {
                vec![spot]
            } else {
                make_strikes(spot, n)
            };
            let prices: Vec<f64> = strikes
                .iter()
                .map(|&strike| {
                    heston_call_price_fourier(spot, strike, time, &params, None)
                        .expect("valid Heston preflight price")
                })
                .collect();
            validate_call_prices(spot, &strikes, &prices, params.r, params.q, time);
            validate_heston_convergence(spot, &strikes, &prices, time, &params);
            validate_heston_controls();

            if n == 1 {
                let strike = strikes[0];
                b.iter(|| {
                    black_box(
                        heston_call_price_fourier(spot, black_box(strike), time, &params, None)
                            .expect("Heston Fourier call price"),
                    )
                });
            } else {
                b.iter(|| {
                    let mut acc = 0.0_f64;
                    for &k in &strikes {
                        let price =
                            heston_call_price_fourier(spot, black_box(k), time, &params, None)
                                .expect("Heston Fourier call price");
                        acc += black_box(price);
                    }
                    black_box(acc)
                });
            }
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cos_strip, bench_heston_fourier_scalar_batch);
criterion_main!(benches);
