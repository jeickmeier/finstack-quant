//! Criterion benchmarks for `finstack-quant-analytics` hot paths.
//!
//! Drives every benchmark through [`Performance`], which is the canonical
//! public entry point. Building-block functions are `pub(crate)` and not
//! intended for direct measurement.

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_analytics::Performance;
use finstack_quant_core::dates::PeriodKind;
use fixtures::{perf_from_returns, perf_panel, synthetic_dates, synthetic_returns};

fn bench_tail_risk(c: &mut Criterion) {
    let perf_small = perf_from_returns(2_500, 42);
    c.bench_function("Performance::value_at_risk 2.5k", |b| {
        b.iter(|| black_box(perf_small.value_at_risk(0.95).expect("valid confidence")));
    });
    c.bench_function("Performance::expected_shortfall 2.5k", |b| {
        b.iter(|| {
            black_box(
                perf_small
                    .expected_shortfall(0.95)
                    .expect("valid confidence"),
            )
        });
    });

    let perf_large = perf_from_returns(100_000, 43);
    c.bench_function("Performance::value_at_risk 100k", |b| {
        b.iter(|| black_box(perf_large.value_at_risk(0.95).expect("valid confidence")));
    });
    c.bench_function("Performance::expected_shortfall 100k", |b| {
        b.iter(|| {
            black_box(
                perf_large
                    .expected_shortfall(0.95)
                    .expect("valid confidence"),
            )
        });
    });
}

fn bench_return_based(c: &mut Criterion) {
    let perf = perf_from_returns(2_500, 7);
    c.bench_function("Performance::volatility 2.5k", |b| {
        b.iter(|| black_box(perf.volatility(true)));
    });
    c.bench_function("Performance::sharpe 2.5k", |b| {
        b.iter(|| black_box(perf.sharpe(0.02)));
    });
}

fn bench_drawdown(c: &mut Criterion) {
    let perf = perf_from_returns(10_000, 11);
    c.bench_function("Performance::get_drawdown_series 10k", |b| {
        b.iter(|| black_box(perf.get_drawdown_series()));
    });
}

fn bench_performance(c: &mut Criterion) {
    let n = 750;
    let dates = synthetic_dates(n);
    let prices_a: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 0.02).collect();
    let prices_b: Vec<f64> = (0..n).map(|i| 50.0 - i as f64 * 0.005).collect();

    c.bench_function("Performance::new 750x2 daily", |b| {
        b.iter(|| {
            black_box(
                Performance::new(
                    dates.clone(),
                    vec![prices_a.clone(), prices_b.clone()],
                    vec!["A".to_string(), "B".to_string()],
                    Some("B"),
                    PeriodKind::Daily,
                )
                .expect("perf"),
            )
        });
    });

    let perf = Performance::new(
        dates,
        vec![prices_a, prices_b],
        vec!["A".to_string(), "B".to_string()],
        Some("B"),
        PeriodKind::Daily,
    )
    .expect("perf");
    c.bench_function("Performance::sharpe 750x2", |b| {
        b.iter(|| black_box(perf.sharpe(0.02)));
    });
    c.bench_function("Performance::value_at_risk 750x2", |b| {
        b.iter(|| black_box(perf.value_at_risk(0.95).expect("valid confidence")));
    });
}

fn bench_rolling_greeks(c: &mut Criterion) {
    let perf = perf_panel(2_500, 2, 17);
    c.bench_function("Performance::rolling_greeks 2.5k window=63", |b| {
        b.iter(|| {
            black_box(
                perf.rolling_greeks(1, 63, 0.0)
                    .expect("rolling greeks bench input"),
            )
        });
    });
}

fn bench_multi_factor_greeks(c: &mut Criterion) {
    // Three factors plus the portfolio column gives a representative
    // OLS sized for a small risk-model regression.
    let perf = perf_panel(2_500, 4, 23);
    c.bench_function("Performance::multi_factor_greeks 2.5k k=3", |b| {
        // Pre-extract factor return slices. The portfolio column is index 3;
        // factors are columns 0 (benchmark), 1, and 2.
        let factor_a: Vec<f64> = synthetic_returns(2_500, 23);
        let factor_b: Vec<f64> = synthetic_returns(2_500, 24);
        let factor_c: Vec<f64> = synthetic_returns(2_500, 25);
        b.iter(|| {
            let factors: [&[f64]; 3] = [&factor_a, &factor_b, &factor_c];
            black_box(
                perf.multi_factor_greeks(3, &factors, finstack_quant_analytics::ReturnKind::Excess)
                    .expect("multi factor regression"),
            )
        });
    });
}

fn bench_correlation_matrix(c: &mut Criterion) {
    let perf = perf_panel(1_000, 50, 31);
    c.bench_function("Performance::correlation_matrix 1k x 50", |b| {
        b.iter(|| black_box(perf.correlation_matrix()));
    });
}

fn bench_period_stats(c: &mut Criterion) {
    let perf = perf_from_returns(2_500, 47);
    c.bench_function("Performance::period_stats 2.5k monthly", |b| {
        b.iter(|| {
            black_box(
                perf.period_stats(0, PeriodKind::Monthly, None)
                    .expect("period stats bench"),
            )
        });
    });
}

criterion_group!(
    benches,
    bench_tail_risk,
    bench_return_based,
    bench_drawdown,
    bench_performance,
    bench_rolling_greeks,
    bench_multi_factor_greeks,
    bench_correlation_matrix,
    bench_period_stats,
);
criterion_main!(benches);
