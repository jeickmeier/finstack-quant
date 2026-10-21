//! Portfolio metrics aggregation benchmarks.
//!
//! Measures `aggregate_metrics` independently from valuation so regressions in
//! the O(P × M) aggregation loop, FX conversion, and neumaier summation are
//! visible without being swamped by instrument-pricing cost.
//!
//! Benchmark groups:
//! - `portfolio_metrics_only`   — pre-valued portfolio, bench just `aggregate_metrics`
//! - `portfolio_value_metrics`  — full pipeline: `value_portfolio` + `aggregate_metrics`
//! - `portfolio_metrics_export` — `metrics_to_table` / `positions_to_table` on a valued book

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{create_institutional_portfolio, create_market_context};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_portfolio::metrics::aggregate_metrics;
use finstack_quant_portfolio::valuation::{
    value_portfolio, PortfolioValuationOptions, RequestedMetrics,
};
use finstack_quant_portfolio::{dataframe::metrics_to_table, dataframe::positions_to_table};
use std::hint::black_box;

fn best_effort_metrics() -> PortfolioValuationOptions {
    PortfolioValuationOptions {
        strict_risk: false,
        metrics: RequestedMetrics::Standard,
    }
}

// aggregate_metrics in isolation (valuation pre-computed outside bench loop)

fn bench_metrics_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_metrics_only");
    let market = create_market_context();
    let config = FinstackConfig::default();

    // 63/64 straddles POSITION_PARALLEL_MIN_POSITIONS; 250 is the workflow
    // default; 3,000 is the institutional control. Aggregation is cheap
    // relative to pricing, so the large case stays in the default matrix.
    for num_positions in [63_usize, 64, 250, 3_000] {
        let portfolio = create_institutional_portfolio(num_positions);
        let valuation =
            value_portfolio(&portfolio, &market, &config, &best_effort_metrics()).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| aggregate_metrics(black_box(&valuation), black_box(&market)).unwrap());
            },
        );
    }
    group.finish();
}

// Full pipeline: value_portfolio + aggregate_metrics

fn bench_value_and_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_value_metrics");
    let market = create_market_context();
    let config = FinstackConfig::default();

    {
        let num_positions = 250usize;
        let portfolio = create_institutional_portfolio(num_positions);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    let valuation = value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &best_effort_metrics(),
                    )
                    .unwrap();
                    aggregate_metrics(black_box(&valuation), black_box(&market)).unwrap()
                });
            },
        );
    }
    group.finish();
}

fn bench_metrics_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_metrics_export");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let portfolio = create_institutional_portfolio(250);
    let valuation = value_portfolio(&portfolio, &market, &config, &best_effort_metrics()).unwrap();
    let metrics = aggregate_metrics(&valuation, &market).unwrap();

    group.bench_function("metrics_to_table_250pos", |b| {
        b.iter(|| metrics_to_table(black_box(&metrics)).unwrap());
    });
    group.bench_function("positions_to_table_250pos", |b| {
        b.iter(|| positions_to_table(black_box(&valuation)).unwrap());
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_metrics_only,
    bench_value_and_metrics,
    bench_metrics_export
);
criterion_main!(benches);
