//! Benchmarks for product-independent Fourier-pricing hot paths.
//!
//! These cover optimizations that the analytic `option_pricing` / `swaption_pricing`
//! benches do not exercise:
//!
//! - **COS strip pricing** (`fourier::cos`): the strike-independent coefficient
//!   `aₖ = Re[φ(uₖ)·exp(-i·uₖ·a)]` is precomputed once per strip instead of once
//!   per strike inside `put_price`.
//! - **Heston Fourier scalar pricing** (`closed_form::heston_call_price_fourier`): the
//!   composite Gauss-Legendre grid is built once and shared across the two
//!   Gil-Pelaez probabilities (j = 1, 2) rather than rebuilt twice per price.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_models::closed_form::{heston_call_price_fourier, HestonPricingParams};
use finstack_quant_models::fourier::characteristic_function::BlackScholesCf;
use finstack_quant_models::fourier::cos::{CosConfig, CosPricer};
use finstack_quant_models::types::OptionType;
use std::hint::black_box;

/// A log-spaced-ish strip of `n` strikes around `spot` (70%–130% moneyness).
fn make_strikes(spot: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| spot * (0.7 + 0.6 * (i as f64) / ((n - 1).max(1) as f64)))
        .collect()
}

// COS strip pricing (#13): strike-independent coefficient reuse.
fn bench_cos_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("cos_strip");
    let spot = 100.0;
    let (r, q, sigma, t) = (0.03_f64, 0.01_f64, 0.20_f64, 1.0_f64);
    let cf = BlackScholesCf { r, q, sigma };
    let pricer = CosPricer::new(&cf, CosConfig::default());

    for &n in &[16_usize, 64, 256] {
        let strikes = make_strikes(spot, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &strikes, |b, strikes| {
            b.iter(|| {
                black_box(
                    pricer
                        .price_strip(spot, black_box(strikes), r, t, OptionType::Call)
                        .unwrap(),
                )
            });
        });
    }
    group.finish();
}

// Heston Fourier scalar pricing (#14): shared GL grid across j = 1, 2.
fn bench_heston_fourier_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("heston_fourier_strip");
    let spot = 100.0;
    let time = 1.0_f64;
    let params = HestonPricingParams::new(0.03, 0.01, 1.5, 0.04, 0.3, -0.6, 0.04)
        .expect("valid benchmark Heston parameters");

    for &n in &[16_usize, 64, 256] {
        let strikes = make_strikes(spot, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &strikes, |b, strikes| {
            b.iter(|| {
                let mut acc = 0.0_f64;
                for &k in strikes {
                    acc += heston_call_price_fourier(spot, black_box(k), time, &params, None)
                        .expect("Heston Fourier call price");
                }
                black_box(acc)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cos_strip, bench_heston_fourier_strip);
criterion_main!(benches);
