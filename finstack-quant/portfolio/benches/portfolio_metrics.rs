//! Portfolio metrics aggregation benchmarks.
//!
//! Measures `aggregate_metrics` independently from valuation so regressions in
//! the O(P × M) aggregation loop, FX conversion, and neumaier summation are
//! visible without being swamped by instrument-pricing cost.
//!
//! Benchmark groups:
//! - `portfolio_metrics_only`   — pre-valued portfolio, bench just `aggregate_metrics`
//! - `portfolio_value_metrics`  — full pipeline: `value_portfolio` + `aggregate_metrics`

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{base_date, create_institutional_portfolio, create_market_context};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_portfolio::metrics::aggregate_metrics;
use finstack_quant_portfolio::valuation::value_portfolio;
use std::hint::black_box;

// ============================================================================
// aggregate_metrics in isolation (valuation pre-computed outside bench loop)
// ============================================================================

fn bench_metrics_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_metrics_only");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let as_of = base_date();

    {
        let num_positions = 250usize;
        let portfolio = create_institutional_portfolio(num_positions);
        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default()).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    aggregate_metrics(
                        black_box(&valuation),
                        black_box(Currency::USD),
                        black_box(&market),
                        black_box(as_of),
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Full pipeline: value_portfolio + aggregate_metrics
// ============================================================================

fn bench_value_and_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_value_metrics");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let as_of = base_date();

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
                        &Default::default(),
                    )
                    .unwrap();
                    aggregate_metrics(
                        black_box(&valuation),
                        black_box(Currency::USD),
                        black_box(&market),
                        black_box(as_of),
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_metrics_only, bench_value_and_metrics);
criterion_main!(benches);
