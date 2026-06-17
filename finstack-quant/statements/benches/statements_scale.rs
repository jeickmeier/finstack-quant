//! Production-scale benchmarks for `finstack-quant-statements`.
//!
//! Complements `statements_operations.rs` (which targets correctness-sized
//! models, 4–24 periods × ≤50 nodes) with the workload sizes encountered in
//! real LBO and credit work:
//!
//! - **`monte_carlo_scaling`** — Monte Carlo path count sweeps. Surfaces
//!   per-path overhead (forecast cache rebuilds, accumulator merges) that is
//!   invisible at small path counts.
//! - **`rolling_window_scaling`** — N rolling-aggregate formulas referencing
//!   the same node. Catches regressions in the per-call BTreeMap rebuild that
//!   `formula_helpers::collect_historical_values_sorted` previously incurred
//!   before per-context memoization landed.
//! - **`large_lbo_model`** — 100 nodes × 60 monthly periods, a realistic
//!   five-year LBO model size. Validates the assumption that the period × node
//!   evaluation loop stays roughly linear at production scale.
//!
//! Run with `cargo bench -p finstack-quant-statements --bench statements_scale`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_statements::evaluator::{Evaluator, MonteCarloConfig};
use finstack_quant_statements::prelude::*;
use indexmap::IndexMap;
use std::hint::black_box;

// ============================================================================
// Monte Carlo scaling
// ============================================================================

/// Build a small forecast model that exercises the Monte Carlo path loop.
fn build_mc_model() -> FinancialModelSpec {
    let actual_q = PeriodId::quarter(2024, 4);
    let mut params = IndexMap::new();
    params.insert("mean".into(), serde_json::json!(0.05));
    params.insert("std_dev".into(), serde_json::json!(0.02));
    params.insert("anchor".into(), serde_json::json!(100.0));
    params.insert("seed".into(), serde_json::json!(42));

    ModelBuilder::new("mc-model")
        .periods("2024Q4..2026Q4", Some("2024Q4"))
        .unwrap()
        .value("revenue", &[(actual_q, AmountOrScalar::scalar(100.0))])
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Normal,
                params,
            },
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap()
}

fn bench_monte_carlo_scaling(c: &mut Criterion) {
    let model = build_mc_model();
    let mut group = c.benchmark_group("monte_carlo_scaling");
    group.sample_size(10); // long runs — keep the matrix wall-clock manageable

    {
        let &n_paths = &1_000usize;
        group.throughput(Throughput::Elements(n_paths as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_paths),
            &n_paths,
            |b, &paths| {
                b.iter(|| {
                    let mut evaluator = Evaluator::new();
                    let config = MonteCarloConfig::new(paths, 42);
                    black_box(
                        evaluator
                            .evaluate_monte_carlo(black_box(&model), &config)
                            .unwrap(),
                    )
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Rolling-window scaling
// ============================================================================

/// Build a model with a single value series plus N rolling-aggregate formulas
/// that all reference it. Catches regressions in shared-history caching.
fn build_rolling_model(n_rolling: usize, n_periods: usize) -> FinancialModelSpec {
    let revenue_values: Vec<(PeriodId, AmountOrScalar)> = (0..n_periods)
        .map(|i| {
            let period = PeriodId::quarter(2020 + (i / 4) as i32, ((i % 4) + 1) as u8);
            (period, AmountOrScalar::scalar(100.0 + i as f64))
        })
        .collect();

    let period_range = format!(
        "2020Q1..{}Q{}",
        2020 + ((n_periods - 1) / 4) as i32,
        ((n_periods - 1) % 4) + 1
    );

    let mut builder = ModelBuilder::new("rolling")
        .periods(&period_range, None)
        .unwrap()
        .value("revenue", &revenue_values);

    for i in 0..n_rolling {
        let window = (i % 6) + 2; // mix of window sizes 2..=7
        builder = builder
            .compute(
                format!("rolling_{}", i),
                format!("rolling_mean(revenue, {})", window),
            )
            .unwrap();
    }

    builder.build().unwrap()
}

fn bench_rolling_window_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_window_scaling");

    {
        let &n_rolling = &25usize;
        let model = build_rolling_model(n_rolling, 24);
        group.throughput(Throughput::Elements(n_rolling as u64));
        group.bench_with_input(
            BenchmarkId::new("rolling_count", n_rolling),
            &model,
            |b, model| {
                b.iter(|| {
                    let mut evaluator = Evaluator::new();
                    black_box(evaluator.evaluate(black_box(model)).unwrap())
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Large LBO-shaped model
// ============================================================================

/// Build a realistic-shaped LBO/operating model: monthly periods over five
/// years, ~100 derived nodes referencing a small shared driver set.
fn build_large_lbo_model(n_nodes: usize, n_months: usize) -> FinancialModelSpec {
    // Build a monthly period range like "2024M01..2028M12".
    let last_year = 2024 + ((n_months - 1) / 12) as i32;
    let last_month = ((n_months - 1) % 12) + 1;
    let period_range = format!("2024M01..{}M{:02}", last_year, last_month);

    let revenue_values: Vec<(PeriodId, AmountOrScalar)> = (0..n_months)
        .map(|i| {
            let period = PeriodId::month(2024 + (i / 12) as i32, ((i % 12) + 1) as u8);
            (
                period,
                AmountOrScalar::scalar(1_000_000.0 + i as f64 * 1_000.0),
            )
        })
        .collect();

    let mut builder = ModelBuilder::new("lbo")
        .periods(&period_range, None)
        .unwrap()
        .value("revenue", &revenue_values);

    // Three shared drivers + (n_nodes - 4) derived metrics that fan out from
    // the driver set. Mirrors the typical LBO shape: a handful of P&L drivers
    // feeding many ratio / margin / coverage outputs.
    builder = builder
        .compute("cogs", "revenue * 0.55")
        .unwrap()
        .compute("opex", "revenue * 0.20")
        .unwrap()
        .compute("ebitda", "revenue - cogs - opex")
        .unwrap();

    for i in 0..n_nodes.saturating_sub(4) {
        let formula = match i % 5 {
            0 => format!("ebitda * {}", 0.01 + 0.001 * i as f64),
            1 => format!("revenue / {}", 1.0 + 0.001 * i as f64),
            2 => format!("rolling_mean(ebitda, 3) + {}", i as f64),
            3 => format!("lag(ebitda, 1) * {}", 0.5 + 0.001 * i as f64),
            _ => format!("ebitda - cogs * {}", 0.001 * i as f64),
        };
        builder = builder.compute(format!("derived_{}", i), formula).unwrap();
    }

    builder.build().unwrap()
}

fn bench_large_lbo_model(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_lbo_model");
    group.sample_size(10);

    {
        let &(n_nodes, n_months) = &(100usize, 60usize);
        let model = build_large_lbo_model(n_nodes, n_months);
        let label = format!("{}x{}", n_nodes, n_months);
        group.throughput(Throughput::Elements((n_nodes * n_months) as u64));
        group.bench_with_input(BenchmarkId::new("evaluate", &label), &model, |b, model| {
            b.iter(|| {
                let mut evaluator = Evaluator::new();
                black_box(evaluator.evaluate(black_box(model)).unwrap())
            });
        });
    }
    group.finish();
}

// ============================================================================
// Criterion configuration
// ============================================================================

criterion_group!(
    benches,
    bench_monte_carlo_scaling,
    bench_rolling_window_scaling,
    bench_large_lbo_model
);
criterion_main!(benches);
