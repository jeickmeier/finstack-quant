//! Scaling guards for `finstack-quant-analytics`.
//!
//! Complements `analytics_hot_paths.rs` (absolute cost at one size) by measuring
//! how cost grows with series length or matrix dimension. Read ns-per-element
//! across sizes: flat is linear; rising means a super-linear term is back.
//!
//! ```sh
//! cargo bench -p finstack-quant-analytics --bench analytics_scaling
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_analytics::correlation::{
    nearest_correlation, validate_correlation_matrix, NearestCorrelationOpts,
};
use finstack_quant_analytics::regression::constrained_least_squares;
use finstack_quant_analytics::{Performance, ReturnKind};
use finstack_quant_core::dates::PeriodKind;
use fixtures::{
    constrained_ls_inputs, near_correlation_needs_repair, perf_from_returns, perf_panel,
    synthetic_dates, synthetic_returns,
};

fn scaling_value_at_risk(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_value_at_risk");
    for n in [2_500_usize, 10_000, 40_000, 100_000] {
        let perf = perf_from_returns(n, 42);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.value_at_risk(0.95).expect("valid confidence")));
        });
    }
    group.finish();
}

fn scaling_expected_shortfall(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_expected_shortfall");
    for n in [2_500_usize, 10_000, 40_000, 100_000] {
        let perf = perf_from_returns(n, 43);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.expected_shortfall(0.95).expect("valid confidence")));
        });
    }
    group.finish();
}

fn scaling_from_returns(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_from_returns");
    for n in [750_usize, 2_500, 10_000, 40_000] {
        let dates = synthetic_dates(n);
        let rets = synthetic_returns(n, 3);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(
                    Performance::from_returns(
                        dates.clone(),
                        vec![rets.clone()],
                        vec!["X".to_string()],
                        None,
                        PeriodKind::Daily,
                    )
                    .expect("from_returns"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_cumulative_returns(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_cumulative_returns");
    for n in [2_500_usize, 10_000, 40_000] {
        let perf = perf_from_returns(n, 53);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.cumulative_returns()));
        });
    }
    group.finish();
}

fn scaling_rolling_sharpe(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_sharpe");
    for n in [2_500_usize, 10_000, 40_000] {
        let perf = perf_from_returns(n, 17);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.rolling_sharpe(0, 63, 0.02).expect("rolling sharpe")));
        });
    }
    group.finish();
}

fn scaling_rolling_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_greeks");
    for n in [2_500_usize, 10_000, 25_000] {
        let perf = perf_panel(n, 2, 17);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.rolling_greeks(1, 63, 0.0).expect("rolling greeks")));
        });
    }
    group.finish();
}

fn scaling_correlation_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_correlation_matrix");
    // Fixed 1k observations; vary ticker count so ns-per-pair is comparable.
    let n_obs = 1_000_usize;
    for n_tickers in [10_usize, 25, 50, 100] {
        let perf = perf_panel(n_obs, n_tickers, 31);
        let pairs = (n_tickers * (n_tickers - 1) / 2) as u64;
        group.throughput(Throughput::Elements(pairs));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_tickers),
            &n_tickers,
            |b, _| {
                b.iter(|| black_box(perf.correlation_matrix().expect("correlation")));
            },
        );
    }
    group.finish();
}

fn scaling_nearest_correlation(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_nearest_correlation");
    let opts = NearestCorrelationOpts::default();
    for n in [20_usize, 40, 60, 80] {
        let input = near_correlation_needs_repair(n);
        assert!(
            validate_correlation_matrix(&input, n).is_err(),
            "needs_repair fixture must fail Cholesky so Higham iterates"
        );
        group.throughput(Throughput::Elements((n * n) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| black_box(nearest_correlation(&input, n, opts).expect("Higham converges")));
        });
    }
    group.finish();
}

fn scaling_constrained_least_squares(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_constrained_least_squares");
    for (n_assets, n_factors) in [(50_usize, 4_usize), (200, 8), (500, 8), (2_000, 8)] {
        let (exposures, returns, weights) = constrained_ls_inputs(n_assets, n_factors);
        let id = format!("{n_assets}x{n_factors}");
        group.throughput(Throughput::Elements(n_assets as u64));
        group.bench_with_input(BenchmarkId::from_parameter(&id), &n_assets, |b, _| {
            b.iter(|| {
                black_box(
                    constrained_least_squares(&exposures, n_factors, &returns, &weights)
                        .expect("full rank"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_drawdown_details(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_drawdown_details");
    for n in [2_500_usize, 10_000, 40_000] {
        let perf = perf_from_returns(n, 11);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(perf.drawdown_details(0, 10).expect("details")));
        });
    }
    group.finish();
}

fn scaling_period_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_period_stats");
    for n in [2_500_usize, 10_000, 40_000] {
        let perf = perf_from_returns(n, 47);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(
                    perf.period_stats(0, PeriodKind::Monthly, None)
                        .expect("period stats"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_multi_factor_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_multi_factor_greeks");
    for n in [500_usize, 2_500, 10_000] {
        let perf = perf_panel(n, 4, 23);
        let factor_a = synthetic_returns(n, 23);
        let factor_b = synthetic_returns(n, 24);
        let factor_c = synthetic_returns(n, 25);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let factors: [&[f64]; 3] = [&factor_a, &factor_b, &factor_c];
                black_box(
                    perf.multi_factor_greeks(3, &factors, ReturnKind::Excess)
                        .expect("multi factor"),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scaling_value_at_risk,
    scaling_expected_shortfall,
    scaling_from_returns,
    scaling_cumulative_returns,
    scaling_rolling_sharpe,
    scaling_rolling_greeks,
    scaling_correlation_matrix,
    scaling_nearest_correlation,
    scaling_constrained_least_squares,
    scaling_drawdown_details,
    scaling_period_stats,
    scaling_multi_factor_greeks,
);
criterion_main!(benches);
