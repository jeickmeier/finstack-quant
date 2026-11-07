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

#[path = "support/validity.rs"]
mod validity;

use bench_common::{create_institutional_portfolio, create_market_context};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_portfolio::metrics::{PortfolioMetrics, aggregate_metrics};
use finstack_quant_portfolio::valuation::{PortfolioValuationOptions, value_portfolio};
use finstack_quant_portfolio::{dataframe::metrics_to_table, dataframe::positions_to_table};
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;

/// Require finite aggregates and a complete position-level metric export.
///
/// # Arguments
///
/// * `metrics` - Aggregated result inspected outside the timing loop.
/// * `position_count` - Number of positions in the successfully valued source book.
fn check_metrics(metrics: &PortfolioMetrics, position_count: usize) {
    assert_eq!(metrics.by_position.len(), position_count);
    assert!(metrics.degraded_positions.is_empty());
    assert!(metrics.skipped_metrics.is_empty());
    assert!(
        metrics
            .get_total("dv01")
            .is_some_and(|value| value.is_finite())
    );
    for metric in metrics.aggregated.values() {
        assert!(metric.total.is_finite());
        assert!(metric.by_entity.values().all(|value| value.is_finite()));
    }
    for position in metrics.by_position.values() {
        assert!(position.metrics.values().all(|value| value.is_finite()));
    }
}

// aggregate_metrics in isolation (valuation pre-computed outside bench loop)

fn bench_metrics_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_metrics_only");

    // 63/64 straddles POSITION_PARALLEL_MIN_POSITIONS; 250 is the workflow
    // default; 3,000 is the institutional control. Build and value only the
    // selected case, before measuring aggregation.
    for num_positions in [63_usize, 64, 250, 3_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, &num_positions| {
                let market = create_market_context();
                let config = FinstackConfig::default();
                let portfolio = create_institutional_portfolio(num_positions);
                let valuation = value_portfolio(
                    &portfolio,
                    &market,
                    &config,
                    &PortfolioValuationOptions::default(),
                )
                .expect("complete standard-risk benchmark valuation");
                validity::valuation(&portfolio, &valuation, &[MetricId::Dv01]);
                validity::bench(
                    b,
                    || {
                        aggregate_metrics(black_box(&valuation), black_box(&market))
                            .expect("complete benchmark metric aggregation")
                    },
                    |metrics| check_metrics(metrics, num_positions),
                );
            },
        );
    }
    group.finish();
}

// Full pipeline: value_portfolio + aggregate_metrics

fn bench_value_and_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_value_metrics");
    let num_positions = 250usize;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}pos", num_positions)),
        &num_positions,
        |b, &num_positions| {
            let market = create_market_context();
            let config = FinstackConfig::default();
            let portfolio = create_institutional_portfolio(num_positions);
            let options = PortfolioValuationOptions::default();
            let preflight = value_portfolio(&portfolio, &market, &config, &options)
                .expect("complete standard-risk benchmark valuation");
            validity::valuation(&portfolio, &preflight, &[MetricId::Dv01]);
            drop(preflight);
            validity::bench(
                b,
                || {
                    let valuation = value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &options,
                    )
                    .expect("complete standard-risk benchmark valuation");
                    aggregate_metrics(black_box(&valuation), black_box(&market))
                        .expect("complete benchmark metric aggregation")
                },
                |metrics| check_metrics(metrics, num_positions),
            );
        },
    );
    group.finish();
}

fn bench_metrics_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_metrics_export");

    group.bench_function("metrics_to_table_250pos", |b| {
        let market = create_market_context();
        let config = FinstackConfig::default();
        let portfolio = create_institutional_portfolio(250);
        let valuation = value_portfolio(
            &portfolio,
            &market,
            &config,
            &PortfolioValuationOptions::default(),
        )
        .expect("complete standard-risk export fixture");
        validity::valuation(&portfolio, &valuation, &[MetricId::Dv01]);
        let metrics = aggregate_metrics(&valuation, &market).expect("complete export metrics");
        check_metrics(&metrics, 250);
        let expected_rows: usize = metrics.by_position.values().map(|p| p.metrics.len()).sum();
        assert!(expected_rows > 0);
        validity::bench(
            b,
            || metrics_to_table(black_box(&metrics)).expect("metrics table export"),
            |table| assert_eq!(table.row_count, expected_rows),
        );
    });
    group.bench_function("positions_to_table_250pos", |b| {
        let market = create_market_context();
        let config = FinstackConfig::default();
        let portfolio = create_institutional_portfolio(250);
        let valuation = value_portfolio(
            &portfolio,
            &market,
            &config,
            &PortfolioValuationOptions::default(),
        )
        .expect("complete standard-risk export fixture");
        validity::valuation(&portfolio, &valuation, &[MetricId::Dv01]);
        validity::bench(
            b,
            || positions_to_table(black_box(&valuation)).expect("positions table export"),
            |table| assert_eq!(table.row_count, 250),
        );
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
