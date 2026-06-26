//! Benchmarks for Fourier-pricing and Taylor-VaR hot paths.
//!
//! These cover optimizations that the analytic `option_pricing` / `swaption_pricing`
//! benches do not exercise:
//!
//! - **COS strip pricing** (`pricer::cos`): the strike-independent coefficient
//!   `aₖ = Re[φ(uₖ)·exp(-i·uₖ·a)]` is precomputed once per strip instead of once
//!   per strike inside `put_price`.
//! - **Heston Fourier scalar pricing** (`models::closed_form::heston`): the
//!   composite Gauss-Legendre grid is built once and shared across the two
//!   Gil-Pelaez probabilities (j = 1, 2) rather than rebuilt twice per price.
//! - **Taylor-approximation Historical VaR** (`metrics::risk::var_calculator`):
//!   the per-scenario `scenario.apply` market rebuild is skipped when the
//!   sensitivities are already in the reporting currency.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::characteristic_function::BlackScholesCf;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::metrics::risk::{
    calculate_var, MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType, VarConfig,
    VarMethod,
};
use finstack_quant_valuations::models::closed_form::{heston_call_price_fourier, HestonParams};
use finstack_quant_valuations::pricer::cos::{CosConfig, CosPricer};
use std::hint::black_box;
use time::Month;

/// A log-spaced-ish strip of `n` strikes around `spot` (70%–130% moneyness).
fn make_strikes(spot: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| spot * (0.7 + 0.6 * (i as f64) / ((n - 1).max(1) as f64)))
        .collect()
}

// ---------------------------------------------------------------------------
// COS strip pricing (#13): strike-independent coefficient reuse.
// ---------------------------------------------------------------------------
fn bench_cos_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("cos_strip");
    let spot = 100.0;
    let (r, q, sigma, t) = (0.03_f64, 0.01_f64, 0.20_f64, 1.0_f64);
    let cf = BlackScholesCf { r, q, sigma };
    let pricer = CosPricer::new(&cf, CosConfig::default());

    for &n in &[16_usize, 64, 256] {
        let strikes = make_strikes(spot, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &strikes, |b, strikes| {
            b.iter(|| black_box(pricer.price_calls(spot, black_box(strikes), r, t).unwrap()));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Heston Fourier scalar pricing (#14): shared GL grid across j = 1, 2.
// ---------------------------------------------------------------------------
fn bench_heston_fourier_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("heston_fourier_strip");
    let spot = 100.0;
    let time = 1.0_f64;
    let params = HestonParams {
        r: 0.03,
        q: 0.01,
        kappa: 1.5,
        theta: 0.04,
        sigma_v: 0.3,
        rho: -0.6,
        v0: 0.04,
    };

    for &n in &[16_usize, 64, 256] {
        let strikes = make_strikes(spot, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &strikes, |b, strikes| {
            b.iter(|| {
                let mut acc = 0.0_f64;
                for &k in strikes {
                    acc += heston_call_price_fourier(spot, black_box(k), time, &params);
                }
                black_box(acc)
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Taylor-approximation Historical VaR (#11): same-currency scenario skip.
// ---------------------------------------------------------------------------
fn make_var_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.970),
            (2.0, 0.940),
            (3.0, 0.910),
            (5.0, 0.850),
            (7.0, 0.800),
            (10.0, 0.730),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

fn make_var_bond(base: Date) -> Bond {
    Bond::fixed(
        "BOND-10Y",
        Money::new(10_000_000.0, Currency::USD),
        0.045,
        base,
        base + time::Duration::days(365 * 10),
        "USD-OIS",
    )
    .unwrap()
}

fn make_history(as_of: Date, n: usize) -> MarketHistory {
    let scenarios: Vec<MarketScenario> = (0..n)
        .map(|i| {
            // Deterministic per-scenario parallel rate shift (no wall-clock).
            let bump = ((i as f64) - (n as f64) / 2.0) * 0.0002;
            MarketScenario::new(
                as_of,
                vec![RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 5.0,
                    },
                    shift: bump,
                }],
            )
        })
        .collect();
    MarketHistory::new(as_of, 252, scenarios)
}

fn bench_taylor_var(c: &mut Criterion) {
    let mut group = c.benchmark_group("taylor_var");
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = make_var_market();
    let bond = make_var_bond(base);
    // Reporting currency == instrument currency exercises the same-currency
    // fast path that skips the per-scenario `scenario.apply` market rebuild.
    let config = VarConfig::var_99()
        .with_method(VarMethod::TaylorApproximation)
        .with_reporting_currency(Currency::USD);

    for &n in &[250_usize, 1000] {
        let history = make_history(base, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &history, |b, history| {
            b.iter(|| {
                black_box(
                    calculate_var(black_box(&[&bond]), &market, history, base, &config).unwrap(),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cos_strip,
    bench_heston_fourier_strip,
    bench_taylor_var
);
criterion_main!(benches);
