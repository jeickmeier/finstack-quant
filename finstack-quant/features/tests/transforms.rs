//! Vectorized panel transform API tests.

use finstack_quant_features::{
    clean_signal, neutralize, neutralize_and_zscore, normalize_signal, rank_to_weights,
    risk_scaled_weights, rolling_regression_residual, transform_cross_sectional,
    transform_cross_sectional_grouped, transform_cross_sectional_with_op, transform_panel,
    transform_panel_spec, transform_timeseries, transform_timeseries_pairwise,
    transform_timeseries_with_op, CrossSectionalOp, PanelOperation, PanelTransformSpec,
    TimeSeriesOp,
};
use serde_json::json;

#[test]
fn transform_timeseries_aligns_unsorted_returns_lag_and_rolling_std() {
    let values = vec![Some(12.0), Some(10.0), Some(21.0), Some(20.0)];
    let entity = vec![
        "A".to_string(),
        "A".to_string(),
        "B".to_string(),
        "B".to_string(),
    ];
    let order = vec![
        "2026-01-02".to_string(),
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-01".to_string(),
    ];

    let returns = transform_timeseries(
        &values,
        &entity,
        &order,
        "returns",
        Some(&json!({"periods": 1})),
    )
    .expect("returns");
    assert!((returns[0].expect("A return") - 0.2).abs() < 1e-12);
    assert_eq!(returns[1], None);
    assert!((returns[2].expect("B return") - 0.05).abs() < 1e-12);
    assert_eq!(returns[3], None);

    let lag = transform_timeseries(
        &values,
        &entity,
        &order,
        "lag",
        Some(&json!({"periods": 1})),
    )
    .expect("lag");
    assert_eq!(lag, vec![Some(10.0), None, Some(20.0), None]);

    let rolling_std = transform_timeseries(
        &values,
        &entity,
        &order,
        "rolling_std",
        Some(&json!({"window": 2, "min_periods": 2})),
    )
    .expect("rolling std");
    assert!((rolling_std[0].expect("A std") - 2.0_f64.sqrt()).abs() < 1e-12);
    assert_eq!(rolling_std[1], None);
}

#[test]
fn transform_timeseries_supports_mvp_rolling_and_ewma_ops() {
    let values = vec![Some(10.0), Some(12.0), Some(13.0), None, Some(17.0)];
    let entity = vec![
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
    ];
    let order = vec![
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-03".to_string(),
        "2026-01-04".to_string(),
        "2026-01-05".to_string(),
    ];

    let diff = transform_timeseries(
        &values,
        &entity,
        &order,
        "diff",
        Some(&json!({"periods": 2})),
    )
    .expect("diff");
    assert_eq!(diff, vec![None, None, Some(3.0), None, Some(4.0)]);

    let rolling_zscore = transform_timeseries(
        &values,
        &entity,
        &order,
        "rolling_zscore",
        Some(&json!({"window": 3, "min_periods": 3})),
    )
    .expect("rolling zscore");
    assert_close_options(
        &rolling_zscore,
        &[None, None, Some(0.872_871_560_943_969_6), None, None],
    );

    let ewma_mean = transform_timeseries(
        &values,
        &entity,
        &order,
        "ewma_mean",
        Some(&json!({"span": 3.0})),
    )
    .expect("ewma mean");
    assert_close_options(
        &ewma_mean,
        &[Some(10.0), Some(11.0), Some(12.0), None, Some(14.5)],
    );

    let returns = vec![Some(1.0), Some(3.0), Some(5.0)];
    let short_entity = vec!["A".to_string(), "A".to_string(), "A".to_string()];
    let short_order = vec![
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-03".to_string(),
    ];
    let ewma_vol = transform_timeseries(
        &returns,
        &short_entity,
        &short_order,
        "ewma_vol",
        Some(&json!({"span": 3.0})),
    )
    .expect("ewma vol");
    assert_close_options(
        &ewma_vol,
        &[Some(1.0), Some(5.0_f64.sqrt()), Some(15.0_f64.sqrt())],
    );

    let ewma_zscore = transform_timeseries(
        &returns,
        &short_entity,
        &short_order,
        "ewma_zscore",
        Some(&json!({"span": 3.0})),
    )
    .expect("ewma zscore");
    assert_close_options(
        &ewma_zscore,
        &[Some(0.0), Some(1.0), Some(0.904_534_033_733_290_9)],
    );

    let old_ewma_alias = transform_timeseries(
        &returns,
        &short_entity,
        &short_order,
        "ewma",
        Some(&json!({"span": 3.0})),
    );
    assert!(old_ewma_alias.is_err());
}

#[test]
fn transform_timeseries_supports_advanced_rolling_signal_ops() {
    let values = vec![Some(1.0), Some(2.0), Some(3.0), Some(100.0), Some(4.0)];
    let entity = vec![
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
        "A".to_string(),
    ];
    let order = vec![
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-03".to_string(),
        "2026-01-04".to_string(),
        "2026-01-05".to_string(),
    ];
    let rolling_params = Some(&json!({"window": 3, "min_periods": 3}));

    let rolling_rank =
        transform_timeseries(&values, &entity, &order, "rolling_rank", rolling_params)
            .expect("rolling rank");
    assert_close_options(
        &rolling_rank,
        &[None, None, Some(1.0), Some(1.0), Some(0.5)],
    );

    let rolling_quantile = transform_timeseries(
        &values,
        &entity,
        &order,
        "rolling_quantile",
        Some(&json!({"window": 3, "min_periods": 3, "quantile": 0.5})),
    )
    .expect("rolling quantile");
    assert_close_options(
        &rolling_quantile,
        &[None, None, Some(2.0), Some(3.0), Some(4.0)],
    );

    let rolling_skew =
        transform_timeseries(&values, &entity, &order, "rolling_skew", rolling_params)
            .expect("rolling skew");
    assert_close_options(&rolling_skew[0..3], &[None, None, Some(0.0)]);

    let rolling_kurtosis =
        transform_timeseries(&values, &entity, &order, "rolling_kurtosis", rolling_params)
            .expect("rolling kurtosis");
    assert_close_options(&rolling_kurtosis[0..3], &[None, None, Some(-1.5)]);

    let rolling_slope =
        transform_timeseries(&values, &entity, &order, "rolling_slope", rolling_params)
            .expect("rolling slope");
    assert_close_options(&rolling_slope[0..3], &[None, None, Some(1.0)]);

    let rolling_sharpe =
        transform_timeseries(&values, &entity, &order, "rolling_sharpe", rolling_params)
            .expect("rolling sharpe");
    assert_close_options(&rolling_sharpe[0..3], &[None, None, Some(2.0)]);

    let rolling_winsorize = transform_timeseries(
        &values,
        &entity,
        &order,
        "rolling_winsorize",
        Some(&json!({"window": 3, "min_periods": 3, "lower": 0.0, "upper": 0.5})),
    )
    .expect("rolling winsorize");
    assert_close_options(
        &rolling_winsorize,
        &[None, None, Some(2.0), Some(3.0), Some(4.0)],
    );

    let hampel = transform_timeseries(
        &values,
        &entity,
        &order,
        "hampel_filter",
        Some(&json!({"window": 3, "min_periods": 3, "threshold": 3.0})),
    )
    .expect("hampel filter");
    assert_close_options(&hampel, &[None, None, Some(3.0), Some(3.0), Some(4.0)]);

    let drawdown = transform_timeseries(
        &[Some(100.0), Some(120.0), Some(90.0)],
        &["A".to_string(), "A".to_string(), "A".to_string()],
        &[
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            "2026-01-03".to_string(),
        ],
        "drawdown",
        None,
    )
    .expect("drawdown");
    assert_close_options(&drawdown, &[Some(0.0), Some(0.0), Some(-0.25)]);

    let decay = transform_timeseries(
        &[Some(1.0), Some(1.0), Some(1.0)],
        &["A".to_string(), "A".to_string(), "A".to_string()],
        &[
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            "2026-01-03".to_string(),
        ],
        "exponential_decay_weights",
        Some(&json!({"window": 3, "half_life": 1.0})),
    )
    .expect("decay weights");
    assert_close_options(&decay, &[Some(1.0), Some(2.0 / 3.0), Some(4.0 / 7.0)]);
}

#[test]
fn transform_cross_sectional_matches_zscore_rank_and_winsorize() {
    let values = vec![Some(1.0), Some(2.0), Some(100.0), Some(5.0)];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
    ];

    let rank = transform_cross_sectional(&values, &time_key, "rank", None).expect("rank");
    assert_eq!(rank[0], Some(0.0));
    assert_eq!(rank[1], Some(0.5));
    assert_eq!(rank[2], Some(1.0));
    assert_eq!(rank[3], Some(0.0));

    let zscore = transform_cross_sectional(&values, &time_key, "zscore", None).expect("zscore");
    assert_eq!(zscore[3], Some(0.0));

    let winsorized = transform_cross_sectional(
        &values,
        &time_key,
        "winsorize",
        Some(&json!({"lower": 0.0, "upper": 0.5})),
    )
    .expect("winsorize");
    assert_eq!(winsorized[2], Some(2.0));
}

#[test]
fn transform_cross_sectional_supports_mvp_signal_cleaning_ops() {
    let values = vec![
        Some(1.0),
        Some(2.0),
        Some(2.0),
        Some(4.0),
        None,
        Some(f64::NAN),
    ];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
    ];

    let percentile_rank = transform_cross_sectional(&values, &time_key, "percentile_rank", None)
        .expect("percentile rank");
    assert_close_options(
        &percentile_rank,
        &[Some(0.2), Some(0.5), Some(0.5), Some(0.8), None, None],
    );

    let quantile_bucket = transform_cross_sectional(
        &values,
        &time_key,
        "quantile_bucket",
        Some(&json!({"buckets": 4})),
    )
    .expect("quantile bucket");
    assert_eq!(
        quantile_bucket,
        vec![Some(0.0), Some(1.0), Some(1.0), Some(3.0), None, None]
    );

    let robust_zscore = transform_cross_sectional(&values, &time_key, "robust_zscore", None)
        .expect("robust zscore");
    assert_close_options(
        &robust_zscore,
        &[
            Some(-1.348_981_518_953_190_4),
            Some(0.0),
            Some(0.0),
            Some(2.697_963_037_906_381),
            None,
            None,
        ],
    );

    let minmax_scale =
        transform_cross_sectional(&values, &time_key, "minmax_scale", None).expect("minmax");
    assert_close_options(
        &minmax_scale,
        &[
            Some(0.0),
            Some(1.0 / 3.0),
            Some(1.0 / 3.0),
            Some(1.0),
            None,
            None,
        ],
    );

    let clip = transform_cross_sectional(
        &values,
        &time_key,
        "clip",
        Some(&json!({"lower": 1.5, "upper": 3.0})),
    )
    .expect("clip");
    assert_eq!(
        clip,
        vec![Some(1.5), Some(2.0), Some(2.0), Some(3.0), None, None]
    );

    let clip_by_sigma = transform_cross_sectional(
        &values,
        &time_key,
        "clip_by_sigma",
        Some(&json!({"sigma": 0.5})),
    )
    .expect("clip by sigma");
    assert_close_options(
        &clip_by_sigma,
        &[
            Some(1.705_137_632_057_415_9),
            Some(2.0),
            Some(2.0),
            Some(2.794_862_367_942_584_),
            None,
            None,
        ],
    );

    let clip_by_quantile = transform_cross_sectional(
        &values,
        &time_key,
        "clip_by_quantile",
        Some(&json!({"lower": 0.25, "upper": 0.75})),
    )
    .expect("clip by quantile");
    assert_eq!(
        clip_by_quantile,
        vec![Some(1.75), Some(2.0), Some(2.0), Some(2.5), None, None]
    );
}

#[test]
fn transform_cross_sectional_supports_normal_score_transform() {
    let values = vec![Some(1.0), Some(2.0), Some(3.0), None];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
    ];
    let normal_scores =
        transform_cross_sectional(&values, &time_key, "normal_score_transform", None)
            .expect("normal scores");
    assert_close_options(
        &normal_scores,
        &[
            Some(-0.674_489_750_196_081_8),
            Some(0.0),
            Some(0.674_489_750_196_081_8),
            None,
        ],
    );
}

#[test]
fn transform_cross_sectional_supports_weight_and_missing_ops() {
    let values = vec![Some(-1.0), Some(0.0), Some(3.0), None, Some(f64::NAN)];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
    ];

    let long_short = transform_cross_sectional(&values, &time_key, "long_short_weights", None)
        .expect("long short weights");
    assert_close_options(
        &long_short,
        &[
            Some(-0.357_142_857_142_857_15),
            Some(-0.142_857_142_857_142_85),
            Some(0.5),
            None,
            None,
        ],
    );

    let dollar_neutral =
        transform_cross_sectional(&values, &time_key, "dollar_neutral_weights", None)
            .expect("dollar neutral weights");
    assert_close_options(
        &dollar_neutral,
        &[
            Some(-0.357_142_857_142_857_15),
            Some(-0.142_857_142_857_142_85),
            Some(0.5),
            None,
            None,
        ],
    );

    let capped = transform_cross_sectional(
        &values,
        &time_key,
        "cap_weights",
        Some(&json!({"max_abs": 0.5})),
    )
    .expect("cap weights");
    assert_close_options(
        &capped,
        &[
            Some(-1.0 / 3.0),
            Some(-1.0 / 3.0),
            Some(1.0 / 3.0),
            None,
            None,
        ],
    );

    let filled = transform_cross_sectional(
        &values,
        &time_key,
        "fill_missing",
        Some(&json!({"value": 42.0})),
    )
    .expect("fill missing");
    assert_eq!(
        filled,
        vec![Some(-1.0), Some(0.0), Some(3.0), Some(42.0), Some(42.0)]
    );

    let is_finite =
        transform_cross_sectional(&values, &time_key, "is_finite", None).expect("is finite");
    assert_eq!(
        is_finite,
        vec![Some(1.0), Some(1.0), Some(1.0), Some(0.0), Some(0.0)]
    );

    let nan_mask =
        transform_cross_sectional(&values, &time_key, "nan_mask", None).expect("nan mask");
    assert_eq!(
        nan_mask,
        vec![Some(0.0), Some(0.0), Some(0.0), Some(1.0), Some(1.0)]
    );
}

#[test]
fn finance_specific_transforms_handle_grouping_neutralization_and_weights() {
    let values = vec![Some(1.0), Some(3.0), Some(10.0), Some(14.0)];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
    ];
    let groups = vec![
        "tech".to_string(),
        "tech".to_string(),
        "fin".to_string(),
        "fin".to_string(),
    ];
    let grouped = transform_cross_sectional_grouped(&values, &time_key, &groups, "zscore", None)
        .expect("grouped zscore");
    assert_close_options(&grouped, &[Some(-1.0), Some(1.0), Some(-1.0), Some(1.0)]);

    let signal = vec![Some(1.0), Some(2.0), Some(2.0), Some(4.0)];
    let beta = vec![Some(0.0), Some(1.0), Some(0.0), Some(1.0)];
    let residual =
        neutralize(&signal, &time_key, &[beta], None).expect("cross-sectional neutralize");
    assert_close_options(&residual, &[Some(-0.5), Some(-1.0), Some(0.5), Some(1.0)]);

    let volatility = vec![Some(1.0), Some(2.0), Some(1.0), Some(2.0)];
    let weights = risk_scaled_weights(&signal, &time_key, &volatility, None).expect("risk weights");
    assert_close_options(
        &weights,
        &[
            Some(1.0 / 6.0),
            Some(1.0 / 6.0),
            Some(1.0 / 3.0),
            Some(1.0 / 3.0),
        ],
    );
}

#[test]
fn grouped_cross_sectional_transform_does_not_collide_composite_keys() {
    let values = vec![Some(1.0), Some(3.0)];
    let time_key = vec!["a\u{1f}b".to_string(), "a".to_string()];
    let groups = vec!["c".to_string(), "b\u{1f}c".to_string()];

    let grouped = transform_cross_sectional_grouped(&values, &time_key, &groups, "zscore", None)
        .expect("grouped zscore");
    assert_eq!(grouped, vec![Some(0.0), Some(0.0)]);
}

#[test]
fn finance_specific_timeseries_transforms_handle_pairwise_and_regression_ops() {
    let y = vec![Some(1.0), Some(2.0), Some(3.0)];
    let x = vec![Some(1.0), Some(2.0), Some(4.0)];
    let entity = vec!["A".to_string(), "A".to_string(), "A".to_string()];
    let order = vec![
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-03".to_string(),
    ];

    let rolling_corr = transform_timeseries_pairwise(
        &y,
        &x,
        &entity,
        &order,
        "rolling_corr",
        Some(&json!({"window": 3, "min_periods": 3})),
    )
    .expect("rolling corr");
    assert_close_options(&rolling_corr, &[None, None, Some(0.981_980_506_061_965_7)]);

    let rolling_beta = transform_timeseries_pairwise(
        &y,
        &x,
        &entity,
        &order,
        "rolling_beta",
        Some(&json!({"window": 3, "min_periods": 3})),
    )
    .expect("rolling beta");
    assert_close_options(&rolling_beta, &[None, None, Some(9.0 / 14.0)]);

    let residual = rolling_regression_residual(
        &[Some(1.0), Some(2.0), Some(5.0)],
        &[vec![Some(0.0), Some(1.0), Some(2.0)]],
        &entity,
        &order,
        Some(&json!({"window": 3, "min_periods": 3})),
    )
    .expect("rolling regression residual");
    assert_close_options(&residual, &[None, None, Some(1.0 / 3.0)]);
}

#[test]
fn pipeline_helpers_compose_cleaning_normalization_and_neutralization() {
    let values = vec![Some(1.0), Some(2.0), Some(100.0)];
    let time_key = vec![
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
        "2026-01-01".to_string(),
    ];
    let cleaned = clean_signal(
        &values,
        &time_key,
        Some(&json!({"lower": 0.0, "upper": 0.5})),
    )
    .expect("clean signal");
    assert_eq!(cleaned, vec![Some(1.0), Some(2.0), Some(2.0)]);

    let normalized = normalize_signal(&values, &time_key, Some(&json!({"method": "rank"})))
        .expect("normalize signal");
    assert_eq!(normalized, vec![Some(0.0), Some(0.5), Some(1.0)]);

    let weights = rank_to_weights(&values, &time_key, None).expect("rank to weights");
    assert_close_options(&weights, &[Some(-0.5), Some(0.0), Some(0.5)]);

    let flat_weights = rank_to_weights(&[Some(1.0), Some(1.0), Some(1.0)], &time_key, None)
        .expect("flat rank weights");
    assert_eq!(flat_weights, vec![Some(0.0), Some(0.0), Some(0.0)]);

    let signal = vec![Some(1.0), Some(2.0), Some(2.0), Some(4.0)];
    let beta = vec![Some(0.0), Some(1.0), Some(0.0), Some(1.0)];
    let neutralized = neutralize_and_zscore(
        &signal,
        &[
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
        ],
        &[beta],
        None,
    )
    .expect("neutralize and zscore");
    assert_close_options(
        &neutralized,
        &[
            Some(-0.632_455_532_033_675_9),
            Some(-1.264_911_064_067_351_8),
            Some(0.632_455_532_033_675_9),
            Some(1.264_911_064_067_351_8),
        ],
    );
}

#[test]
fn typed_transform_entrypoints_avoid_string_dispatch() {
    let values = vec![Some(12.0), Some(10.0), Some(21.0), Some(20.0)];
    let entity = vec![
        "A".to_string(),
        "A".to_string(),
        "B".to_string(),
        "B".to_string(),
    ];
    let order = vec![
        "2026-01-02".to_string(),
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        "2026-01-01".to_string(),
    ];

    let returns = transform_timeseries_with_op(
        &values,
        &entity,
        &order,
        TimeSeriesOp::Returns,
        Some(&json!({"periods": 1})),
    )
    .expect("typed returns");
    assert!((returns[0].expect("A return") - 0.2).abs() < 1e-12);

    let ranks = transform_cross_sectional_with_op(
        &[Some(1.0), Some(2.0), Some(100.0)],
        &[
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
        ],
        CrossSectionalOp::Rank,
        None,
    )
    .expect("typed rank");
    assert_eq!(ranks, vec![Some(0.0), Some(0.5), Some(1.0)]);
}

#[test]
fn transform_panel_runs_multiple_named_operations() {
    let spec = json!({
        "values": [10.0, 12.0, 20.0, 21.0],
        "entity": ["A", "A", "B", "B"],
        "order": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "time_key": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "operations": [
            {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
            {"name": "rank", "family": "cross_sectional", "op": "rank"}
        ]
    });

    let out = transform_panel(&spec.to_string()).expect("panel");
    let result: serde_json::Value = serde_json::from_str(&out).expect("panel JSON");
    assert!((result["columns"]["ret1"][1].as_f64().expect("ret1") - 0.2).abs() < 1e-12);
    assert_eq!(result["columns"]["rank"][2], 1.0);
}

#[test]
fn typed_transform_panel_preserves_operation_order() {
    let spec = PanelTransformSpec {
        values: vec![Some(10.0), Some(12.0), Some(20.0), Some(21.0)],
        entity: Some(vec![
            "A".to_string(),
            "A".to_string(),
            "B".to_string(),
            "B".to_string(),
        ]),
        order: Some(vec![
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
        ]),
        time_key: Some(vec![
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
        ]),
        operations: vec![
            PanelOperation::CrossSectional {
                name: "rank".to_string(),
                op: CrossSectionalOp::Rank,
                params: None,
            },
            PanelOperation::Timeseries {
                name: "ret1".to_string(),
                op: TimeSeriesOp::Returns,
                params: Some(json!({"periods": 1})),
            },
        ],
    };

    let result = transform_panel_spec(&spec).expect("typed panel");
    assert_eq!(result.columns[0].name, "rank");
    assert_eq!(result.columns[1].name, "ret1");
    assert_eq!(result.get_column("rank").expect("rank")[2], Some(1.0));
}

#[test]
fn transform_panel_rejects_duplicate_operation_names() {
    let spec = json!({
        "values": [10.0, 12.0],
        "entity": ["A", "A"],
        "order": ["2026-01-01", "2026-01-02"],
        "operations": [
            {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
            {"name": "ret1", "family": "timeseries", "op": "lag", "params": {"periods": 1}}
        ]
    });

    let err = transform_panel(&spec.to_string()).expect_err("duplicate operation names");
    assert!(err.to_string().contains("duplicate"));
}

fn assert_close_options(actual: &[Option<f64>], expected: &[Option<f64>]) {
    assert_eq!(actual.len(), expected.len());
    for (idx, (actual_value, expected_value)) in actual.iter().zip(expected.iter()).enumerate() {
        match (actual_value, expected_value) {
            (Some(actual_value), Some(expected_value)) => assert!(
                (actual_value - expected_value).abs() < 1e-12,
                "idx {idx}: expected {expected_value}, got {actual_value}"
            ),
            (None, None) => {}
            _ => panic!("idx {idx}: expected {expected_value:?}, got {actual_value:?}"),
        }
    }
}

#[test]
fn transform_panel_rejects_duplicate_operation_names_before_evaluation() {
    let spec = json!({
        "values": [10.0, 12.0],
        "operations": [
            {"name": "dup", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
            {"name": "dup", "family": "cross_sectional", "op": "rank"}
        ]
    });

    let err = transform_panel(&spec.to_string()).expect_err("duplicate operation names");
    assert!(err.to_string().contains("duplicate"));
}
