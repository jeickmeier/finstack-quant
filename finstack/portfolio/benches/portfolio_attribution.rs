//! Portfolio P&L attribution benchmarks.
//!
//! `attribute_portfolio_pnl` is the most expensive portfolio-level operation:
//! each position is repriced under T0 and T1 markets (or more, depending on
//! method), then factor contributions are FX-converted and aggregated.
//!
//! This bench uses realistic day-over-day markets (+10bp parallel shift) so
//! every code path — repricing, waterfall decomposition, FX translation, and
//! neumaier aggregation — is exercised.
//!
//! Benchmark groups:
//! - `portfolio_attribution_parallel`      — `AttributionMethod::Parallel`
//! - `portfolio_attribution_metrics_based` — `AttributionMethod::MetricsBased`

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{
    base_date, create_institutional_portfolio, create_market_context, create_t1_market_context,
    t1_date,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_attribution::AttributionMethod;
use finstack_core::config::FinstackConfig;
use finstack_portfolio::attribution::attribute_portfolio_pnl;
use std::hint::black_box;

// ============================================================================
// Parallel attribution
//
// Each position is independently repriced under T0 and T1 markets.  Cost is
// proportional to 2 × (valuation cost per position) × num_positions.
// ============================================================================

fn bench_attribution_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_attribution_parallel");
    let market_t0 = create_market_context();
    let market_t1 = create_t1_market_context();
    let config = FinstackConfig::default();
    let as_of_t0 = base_date();
    let as_of_t1 = t1_date();

    for num_positions in [20usize, 50, 100, 250, 500, 2000] {
        let portfolio = create_institutional_portfolio(num_positions);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    attribute_portfolio_pnl(
                        black_box(&portfolio),
                        black_box(&market_t0),
                        black_box(&market_t1),
                        black_box(as_of_t0),
                        black_box(as_of_t1),
                        black_box(&config),
                        AttributionMethod::Parallel,
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Metrics-based attribution
//
// Linear approximation using pre-computed sensitivities (theta, DV01, CS01).
// Much faster than Parallel but exercising the same aggregation / FX path.
// ============================================================================

fn bench_attribution_metrics_based(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_attribution_metrics_based");
    let market_t0 = create_market_context();
    let market_t1 = create_t1_market_context();
    let config = FinstackConfig::default();
    let as_of_t0 = base_date();
    let as_of_t1 = t1_date();

    for num_positions in [20usize, 50, 100, 250, 500, 2000] {
        let portfolio = create_institutional_portfolio(num_positions);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    attribute_portfolio_pnl(
                        black_box(&portfolio),
                        black_box(&market_t0),
                        black_box(&market_t1),
                        black_box(as_of_t0),
                        black_box(as_of_t1),
                        black_box(&config),
                        AttributionMethod::MetricsBased,
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_attribution_parallel,
    bench_attribution_metrics_based,
);
criterion_main!(benches);
