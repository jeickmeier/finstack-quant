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
    base_date, create_attribution_portfolio, create_market_context, create_t1_market_context,
    t1_date,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_attribution::{
    default_waterfall_order, AttributionMethod, TaylorAttributionConfig,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_portfolio::attribution::attribute_portfolio_pnl;
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

    for num_positions in [40usize, 120, 250] {
        let portfolio = create_attribution_portfolio(num_positions);

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

    for num_positions in [40usize, 120, 250] {
        let portfolio = create_attribution_portfolio(num_positions);

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

// ============================================================================
// Method-owned controls
//
// Waterfall and Taylor intentionally keep financially distinct state-building
// paths. Retain explicit controls so endpoint reuse in MetricsBased cannot hide
// a regression in either method.
// ============================================================================

fn bench_attribution_method_owned_controls(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_attribution_method_owned_controls");
    group.sample_size(10);
    let market_t0 = create_market_context();
    let market_t1 = create_t1_market_context();
    let config = FinstackConfig::default();
    let as_of_t0 = base_date();
    let as_of_t1 = t1_date();
    let methods = [
        (
            "waterfall",
            AttributionMethod::Waterfall(default_waterfall_order()),
        ),
        (
            "taylor",
            AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        ),
    ];

    for num_positions in [40usize, 120] {
        let portfolio = create_attribution_portfolio(num_positions);
        for (label, method) in &methods {
            group.bench_with_input(
                BenchmarkId::new(*label, format!("{num_positions}pos")),
                method,
                |b, method| {
                    b.iter(|| {
                        attribute_portfolio_pnl(
                            black_box(&portfolio),
                            black_box(&market_t0),
                            black_box(&market_t1),
                            black_box(as_of_t0),
                            black_box(as_of_t1),
                            black_box(&config),
                            black_box(method.clone()),
                        )
                        .unwrap()
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_attribution_parallel,
    bench_attribution_metrics_based,
    bench_attribution_method_owned_controls,
);
criterion_main!(benches);
