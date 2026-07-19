//! Cross-feature portfolio workflow benchmarks.
//!
//! These cases measure request-level reuse that is not visible in a single
//! instrument or single-state valuation benchmark.

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{
    base_date, create_attribution_portfolio, create_institutional_portfolio, create_market_context,
    create_t1_market_context,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_attribution::AttributionMethod;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_portfolio::replay::{
    replay_portfolio, ReplayConfig, ReplayErrorPolicy, ReplayMode, ReplayTimeline,
};
use finstack_quant_portfolio::scenarios::{scenario_pnl, scenario_pnl_batch};
use finstack_quant_portfolio::valuation::{PortfolioValuationOptions, RequestedMetrics};
use finstack_quant_scenarios::spec::{CurveKind, OperationSpec, ScenarioSpec};
use std::hint::black_box;

fn full_workflow_matrix_enabled() -> bool {
    std::env::var("FINSTACK_PORTFOLIO_BENCH_FULL").is_ok_and(|value| value == "1")
}

fn market_scenarios(count: usize) -> Vec<ScenarioSpec> {
    (0..count)
        .map(|index| ScenarioSpec {
            id: format!("rates_{index}"),
            name: None,
            description: None,
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: (index as f64 + 1.0) * 2.5,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        })
        .collect()
}

fn bench_scenario_pnl_workflows(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("portfolio_scenario_pnl_workflows");
    group.sample_size(10);
    let portfolio = create_attribution_portfolio(120);
    let market = create_market_context();
    let config = FinstackConfig::default();

    let single = market_scenarios(1);
    group.bench_function("single/120pos", |bencher| {
        bencher.iter(|| {
            scenario_pnl(
                black_box(&portfolio),
                black_box(&single[0]),
                black_box(&market),
                black_box(&config),
            )
            .expect("single scenario P&L benchmark")
        });
    });

    for scenario_count in [10usize, 100] {
        let scenarios = market_scenarios(scenario_count);

        group.bench_with_input(
            BenchmarkId::new("repeated_single", scenario_count),
            &scenario_count,
            |bencher, _| {
                bencher.iter(|| {
                    scenarios
                        .iter()
                        .map(|scenario| {
                            scenario_pnl(
                                black_box(&portfolio),
                                black_box(scenario),
                                black_box(&market),
                                black_box(&config),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .expect("scenario P&L benchmark")
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("bounded_batch", scenario_count),
            &scenario_count,
            |bencher, _| {
                bencher.iter(|| {
                    scenario_pnl_batch(
                        black_box(&portfolio),
                        black_box(&scenarios),
                        black_box(&market),
                        black_box(&config),
                    )
                    .expect("scenario batch benchmark")
                });
            },
        );
    }

    group.finish();
}

fn bench_scenario_pnl_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("portfolio_scenario_pnl_scale");
    group.sample_size(10);
    let market = create_market_context();
    let config = FinstackConfig::default();
    let scenario = market_scenarios(1)
        .pop()
        .expect("one scale benchmark scenario");
    let mut position_counts = vec![120usize];
    if full_workflow_matrix_enabled() {
        position_counts.push(3_000);
    }

    for position_count in position_counts {
        let portfolio = create_institutional_portfolio(position_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{position_count}pos")),
            &position_count,
            |bencher, _| {
                bencher.iter(|| {
                    scenario_pnl(
                        black_box(&portfolio),
                        black_box(&scenario),
                        black_box(&market),
                        black_box(&config),
                    )
                    .expect("scenario P&L scale benchmark")
                });
            },
        );
    }
    group.finish();
}

fn replay_timeline(snapshot_count: usize) -> ReplayTimeline {
    let base = create_market_context();
    let shifted = create_t1_market_context();
    let snapshots = (0..snapshot_count)
        .map(|index| {
            let market = if index % 2 == 0 {
                base.clone()
            } else {
                shifted.clone()
            };
            (base_date() + time::Duration::days(index as i64), market)
        })
        .collect();
    ReplayTimeline::new(snapshots).expect("ordered benchmark timeline")
}

fn bench_replay_workflows(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("portfolio_replay_workflows");
    group.sample_size(10);
    let config = FinstackConfig::default();
    let pv_only = ReplayConfig {
        mode: ReplayMode::PvOnly,
        attribution_method: AttributionMethod::MetricsBased,
        valuation_options: PortfolioValuationOptions {
            strict_risk: false,
            metrics: RequestedMetrics::Only(Vec::new()),
        },
        on_error: ReplayErrorPolicy::Strict,
    };
    let metrics_attribution = ReplayConfig {
        mode: ReplayMode::FullAttribution,
        attribution_method: AttributionMethod::MetricsBased,
        valuation_options: PortfolioValuationOptions::default(),
        on_error: ReplayErrorPolicy::Strict,
    };

    let mut book_sizes = vec![40usize];
    if full_workflow_matrix_enabled() {
        book_sizes.push(300);
    }

    for position_count in book_sizes {
        let portfolio = create_attribution_portfolio(position_count);
        for snapshot_count in [20usize, 250] {
            let timeline = replay_timeline(snapshot_count);
            let shape = format!("{position_count}pos_{snapshot_count}snap");
            group.bench_with_input(
                BenchmarkId::new("pv_only", &shape),
                &snapshot_count,
                |bencher, _| {
                    bencher.iter(|| {
                        replay_portfolio(
                            black_box(&portfolio),
                            black_box(&timeline),
                            black_box(&pv_only),
                            black_box(&config),
                        )
                        .expect("PV-only replay benchmark")
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new("metrics_attribution", &shape),
                &snapshot_count,
                |bencher, _| {
                    bencher.iter(|| {
                        replay_portfolio(
                            black_box(&portfolio),
                            black_box(&timeline),
                            black_box(&metrics_attribution),
                            black_box(&config),
                        )
                        .expect("metrics-attribution replay benchmark")
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_scenario_pnl_workflows,
    bench_scenario_pnl_scale,
    bench_replay_workflows
);
criterion_main!(benches);
