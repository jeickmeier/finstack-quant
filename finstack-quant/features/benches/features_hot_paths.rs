//! Criterion benchmarks for `finstack-quant-features` hot paths.
//!
//! Absolute cost of each public entry point at one representative panel
//! (100 names × 252 days). Building-block functions that are `pub(crate)`
//! are not measured directly.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_features::{
    neutralize, neutralize_and_zscore, rank_to_weights, risk_scaled_weights,
    rolling_regression_residual, transform_cross_sectional,
    transform_cross_sectional_grouped_with_op, transform_cross_sectional_with_op, transform_panel,
    transform_panel_json, transform_timeseries_pairwise_with_op, transform_timeseries_with_op,
    CrossSectionalOp, PairwiseOp, PanelOperation, PanelTransformSpec, TimeSeriesOp,
};
use fixtures::{hot_panel, span_params, window_params, FeaturePanel, HOT_WINDOW};
use serde_json::{json, Value};

fn ts(
    panel: &FeaturePanel,
    values: &[Option<f64>],
    op: TimeSeriesOp,
    params: Option<&Value>,
) -> Vec<Option<f64>> {
    transform_timeseries_with_op(values, &panel.entity, &panel.order, op, params).expect("ts")
}

fn cs(panel: &FeaturePanel, op: CrossSectionalOp, params: Option<&Value>) -> Vec<Option<f64>> {
    transform_cross_sectional_with_op(&panel.values, &panel.time_key, op, params).expect("cs")
}

fn bench_timeseries_linear(c: &mut Criterion) {
    let panel = hot_panel();
    let span = span_params(20.0);

    c.bench_function("timeseries::returns 100x252", |b| {
        b.iter(|| black_box(ts(&panel, &panel.values, TimeSeriesOp::Returns, None)));
    });
    c.bench_function("timeseries::log_returns 100x252", |b| {
        b.iter(|| black_box(ts(&panel, &panel.values, TimeSeriesOp::LogReturns, None)));
    });
    c.bench_function("timeseries::lag 100x252", |b| {
        b.iter(|| black_box(ts(&panel, &panel.values, TimeSeriesOp::Lag, None)));
    });
    c.bench_function("timeseries::drawdown 100x252", |b| {
        b.iter(|| black_box(ts(&panel, &panel.levels, TimeSeriesOp::Drawdown, None)));
    });
    c.bench_function("timeseries::ewma_mean 100x252 span=20", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::EwmaMean,
                Some(&span),
            ))
        });
    });
    c.bench_function("timeseries::ewma_zscore 100x252 span=20", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::EwmaZscore,
                Some(&span),
            ))
        });
    });
}

fn bench_timeseries_rolling(c: &mut Criterion) {
    let panel = hot_panel();
    let w21 = window_params(21);
    let w63 = window_params(HOT_WINDOW);

    c.bench_function("timeseries::rolling_mean 100x252 w=21", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingMean,
                Some(&w21),
            ))
        });
    });
    c.bench_function("timeseries::rolling_mean 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingMean,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_std 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingStd,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_zscore 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingZscore,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_min 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingMin,
                Some(&w63),
            ))
        });
    });
}

fn bench_timeseries_advanced(c: &mut Criterion) {
    let panel = hot_panel();
    let w63 = window_params(HOT_WINDOW);
    let quantile = json!({ "window": HOT_WINDOW, "min_periods": HOT_WINDOW, "quantile": 0.5 });
    let winsor = json!({
        "window": HOT_WINDOW,
        "min_periods": HOT_WINDOW,
        "lower": 0.01,
        "upper": 0.99
    });
    let decay = json!({ "window": HOT_WINDOW, "half_life": 21.0 });

    c.bench_function("timeseries::rolling_rank 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingRank,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_quantile 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingQuantile,
                Some(&quantile),
            ))
        });
    });
    c.bench_function("timeseries::rolling_skew 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingSkew,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_sharpe 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingSharpe,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::rolling_winsorize 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::RollingWinsorize,
                Some(&winsor),
            ))
        });
    });
    c.bench_function("timeseries::hampel_filter 100x252 w=63", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::HampelFilter,
                Some(&w63),
            ))
        });
    });
    c.bench_function("timeseries::exp_decay_weights 100x252 w=63 hl=21", |b| {
        b.iter(|| {
            black_box(ts(
                &panel,
                &panel.values,
                TimeSeriesOp::ExponentialDecayWeights,
                Some(&decay),
            ))
        });
    });
}

fn bench_cross_sectional(c: &mut Criterion) {
    let panel = hot_panel();
    let winsor = json!({ "lower": 0.01, "upper": 0.99 });

    c.bench_function("cross_section::zscore 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::Zscore, None)));
    });
    c.bench_function("cross_section::rank 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::Rank, None)));
    });
    c.bench_function("cross_section::winsorize 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::Winsorize, Some(&winsor))));
    });
    c.bench_function("cross_section::robust_zscore 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::RobustZscore, None)));
    });
    c.bench_function("cross_section::long_short_weights 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::LongShortWeights, None)));
    });
    c.bench_function("cross_section::normal_score 100x252", |b| {
        b.iter(|| black_box(cs(&panel, CrossSectionalOp::NormalScoreTransform, None)));
    });
}

fn bench_multi(c: &mut Criterion) {
    let panel = hot_panel();
    let w63 = window_params(HOT_WINDOW);

    c.bench_function("pairwise::rolling_corr 100x252 w=63", |b| {
        b.iter(|| {
            black_box(
                transform_timeseries_pairwise_with_op(
                    &panel.values,
                    &panel.other,
                    &panel.entity,
                    &panel.order,
                    PairwiseOp::RollingCorr,
                    Some(&w63),
                )
                .expect("corr"),
            )
        });
    });
    c.bench_function("pairwise::rolling_beta 100x252 w=63", |b| {
        b.iter(|| {
            black_box(
                transform_timeseries_pairwise_with_op(
                    &panel.values,
                    &panel.other,
                    &panel.entity,
                    &panel.order,
                    PairwiseOp::RollingBeta,
                    Some(&w63),
                )
                .expect("beta"),
            )
        });
    });
    c.bench_function("grouped::zscore 100x252 g=10", |b| {
        b.iter(|| {
            black_box(
                transform_cross_sectional_grouped_with_op(
                    &panel.values,
                    &panel.time_key,
                    &panel.groups,
                    CrossSectionalOp::Zscore,
                    None,
                )
                .expect("grouped"),
            )
        });
    });
    c.bench_function("neutralize 100x252 k=3", |b| {
        b.iter(|| {
            black_box(
                neutralize(&panel.values, &panel.time_key, &panel.exposures, None)
                    .expect("neutralize"),
            )
        });
    });
    c.bench_function("neutralize_and_zscore 100x252 k=3", |b| {
        b.iter(|| {
            black_box(
                neutralize_and_zscore(&panel.values, &panel.time_key, &panel.exposures, None)
                    .expect("naz"),
            )
        });
    });
    c.bench_function("rolling_regression_residual 100x252 w=63 k=3", |b| {
        b.iter(|| {
            black_box(
                rolling_regression_residual(
                    &panel.values,
                    &panel.exposures,
                    &panel.entity,
                    &panel.order,
                    Some(&w63),
                )
                .expect("resid"),
            )
        });
    });
    c.bench_function("rank_to_weights 100x252", |b| {
        b.iter(|| black_box(rank_to_weights(&panel.values, &panel.time_key).expect("rtw")));
    });
    c.bench_function("risk_scaled_weights 100x252", |b| {
        b.iter(|| {
            black_box(
                risk_scaled_weights(&panel.values, &panel.time_key, &panel.volatility)
                    .expect("rsw"),
            )
        });
    });
    c.bench_function("cross_sectional winsorize 100x252", |b| {
        b.iter(|| {
            black_box(
                transform_cross_sectional(&panel.values, &panel.time_key, "winsorize", None)
                    .expect("clean"),
            )
        });
    });
    c.bench_function("cross_sectional zscore 100x252", |b| {
        b.iter(|| {
            black_box(
                transform_cross_sectional(&panel.values, &panel.time_key, "zscore", None)
                    .expect("norm"),
            )
        });
    });
}

fn bench_panel_pipeline(c: &mut Criterion) {
    let panel = hot_panel();
    let spec = PanelTransformSpec {
        values: panel.values,
        entity: Some(panel.entity),
        order: Some(panel.order),
        time_key: Some(panel.time_key),
        operations: vec![
            PanelOperation::Timeseries {
                name: "ret1".to_string(),
                op: TimeSeriesOp::Returns,
                params: None,
                input: Some("values".to_string()),
            },
            PanelOperation::Timeseries {
                name: "vol63".to_string(),
                op: TimeSeriesOp::RollingStd,
                params: Some(window_params(HOT_WINDOW)),
                input: Some("ret1".to_string()),
            },
            PanelOperation::CrossSectional {
                name: "rank".to_string(),
                op: CrossSectionalOp::Rank,
                params: None,
                input: Some("ret1".to_string()),
            },
        ],
    };
    let spec_json = serde_json::to_string(&spec).expect("spec json");

    c.bench_function("panel_spec returns+std+rank 100x252", |b| {
        b.iter(|| black_box(transform_panel(&spec).expect("panel spec")));
    });
    c.bench_function("panel_json returns+std+rank 100x252", |b| {
        b.iter(|| black_box(transform_panel_json(&spec_json).expect("panel json")));
    });
}

criterion_group!(
    benches,
    bench_timeseries_linear,
    bench_timeseries_rolling,
    bench_timeseries_advanced,
    bench_cross_sectional,
    bench_multi,
    bench_panel_pipeline,
);
criterion_main!(benches);
