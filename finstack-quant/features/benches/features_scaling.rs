//! Scaling guards for `finstack-quant-features`.
//!
//! Complements `features_hot_paths.rs` (absolute cost at one size) by measuring
//! how cost grows with row count, window length, cross-section width, and
//! factor count. Read ns-per-element across sizes: flat is linear; rising
//! means a super-linear term is back.
//!
//! ```sh
//! cargo bench -p finstack-quant-features --bench features_scaling
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_features::{
    neutralize, rolling_regression_residual, transform_cross_sectional,
    transform_cross_sectional_grouped, transform_timeseries, transform_timeseries_pairwise,
    CrossSectionalOp, PairwiseOp, TimeSeriesOp,
};
use fixtures::{feature_panel, window_params, HOT_FACTORS, HOT_OBS, HOT_WINDOW};
use serde_json::json;

fn scaling_returns(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_returns");
    for n_entities in [50_usize, 100, 200, 400] {
        let panel = feature_panel(n_entities, HOT_OBS, HOT_FACTORS, 7);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(panel.n_rows),
            &panel,
            |b, panel| {
                b.iter(|| {
                    black_box(
                        transform_timeseries(
                            &panel.values,
                            &panel.entity,
                            &panel.order,
                            TimeSeriesOp::Returns,
                            None,
                        )
                        .expect("returns"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn scaling_rolling_mean_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_mean_rows");
    let params = window_params(HOT_WINDOW);
    for n_entities in [50_usize, 100, 200] {
        let panel = feature_panel(n_entities, HOT_OBS, HOT_FACTORS, 11);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(panel.n_rows),
            &panel,
            |b, panel| {
                b.iter(|| {
                    black_box(
                        transform_timeseries(
                            &panel.values,
                            &panel.entity,
                            &panel.order,
                            TimeSeriesOp::RollingMean,
                            Some(&params),
                        )
                        .expect("mean"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn scaling_rolling_mean_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_mean_window");
    let panel = feature_panel(100, HOT_OBS, HOT_FACTORS, 13);
    for window in [21_usize, 63, 126, 252] {
        let params = window_params(window);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    transform_timeseries(
                        &panel.values,
                        &panel.entity,
                        &panel.order,
                        TimeSeriesOp::RollingMean,
                        Some(&params),
                    )
                    .expect("mean"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_rolling_rank_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_rank_window");
    let panel = feature_panel(100, HOT_OBS, HOT_FACTORS, 17);
    for window in [21_usize, 63, 126] {
        let params = window_params(window);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    transform_timeseries(
                        &panel.values,
                        &panel.entity,
                        &panel.order,
                        TimeSeriesOp::RollingRank,
                        Some(&params),
                    )
                    .expect("rank"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_hampel_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_hampel_window");
    let panel = feature_panel(100, HOT_OBS, HOT_FACTORS, 19);
    for window in [21_usize, 63, 126] {
        let params = window_params(window);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    transform_timeseries(
                        &panel.values,
                        &panel.entity,
                        &panel.order,
                        TimeSeriesOp::HampelFilter,
                        Some(&params),
                    )
                    .expect("hampel"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_zscore_names(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_zscore_names");
    for n_entities in [50_usize, 100, 200, 400] {
        let panel = feature_panel(n_entities, HOT_OBS, HOT_FACTORS, 23);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_entities),
            &panel,
            |b, panel| {
                b.iter(|| {
                    black_box(
                        transform_cross_sectional(
                            &panel.values,
                            &panel.time_key,
                            CrossSectionalOp::Zscore,
                            None,
                        )
                        .expect("zscore"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn scaling_grouped_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_grouped_zscore");
    for n_entities in [50_usize, 100, 200] {
        let panel = feature_panel(n_entities, HOT_OBS, HOT_FACTORS, 29);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(panel.n_rows),
            &panel,
            |b, panel| {
                b.iter(|| {
                    black_box(
                        transform_cross_sectional_grouped(
                            &panel.values,
                            &panel.time_key,
                            &panel.groups,
                            CrossSectionalOp::Zscore,
                            None,
                        )
                        .expect("grouped"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn scaling_neutralize_factors(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_neutralize_factors");
    for n_factors in [1_usize, 2, 3, 5] {
        let panel = feature_panel(100, HOT_OBS, n_factors, 31);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_factors),
            &panel,
            |b, panel| {
                b.iter(|| {
                    black_box(
                        neutralize(&panel.values, &panel.time_key, &panel.exposures, None)
                            .expect("neutralize"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn scaling_pairwise_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_pairwise_corr_window");
    let panel = feature_panel(100, HOT_OBS, HOT_FACTORS, 37);
    for window in [21_usize, 63, 126] {
        let params = window_params(window);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    transform_timeseries_pairwise(
                        &panel.values,
                        &panel.other,
                        &panel.entity,
                        &panel.order,
                        PairwiseOp::RollingCorr,
                        Some(&params),
                    )
                    .expect("corr"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_rolling_regression_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_rolling_regression_window");
    let panel = feature_panel(50, 126, HOT_FACTORS, 41);
    for window in [21_usize, 63] {
        let params = window_params(window);
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    rolling_regression_residual(
                        &panel.values,
                        &panel.exposures,
                        &panel.entity,
                        &panel.order,
                        Some(&params),
                    )
                    .expect("resid"),
                )
            });
        });
    }
    group.finish();
}

fn scaling_exp_decay_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_exp_decay_window");
    let panel = feature_panel(100, HOT_OBS, HOT_FACTORS, 43);
    for window in [21_usize, 63, 126, 252] {
        let params = json!({ "window": window, "half_life": 21.0 });
        group.throughput(Throughput::Elements(panel.n_rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                black_box(
                    transform_timeseries(
                        &panel.values,
                        &panel.entity,
                        &panel.order,
                        TimeSeriesOp::ExponentialDecayWeights,
                        Some(&params),
                    )
                    .expect("decay"),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scaling_returns,
    scaling_rolling_mean_rows,
    scaling_rolling_mean_window,
    scaling_rolling_rank_window,
    scaling_hampel_window,
    scaling_zscore_names,
    scaling_grouped_rows,
    scaling_neutralize_factors,
    scaling_pairwise_window,
    scaling_rolling_regression_window,
    scaling_exp_decay_window,
);
criterion_main!(benches);
