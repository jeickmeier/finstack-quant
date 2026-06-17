//! Vectorized panel transform API tests.

use finstack_quant_features::{
    transform_cross_sectional, transform_cross_sectional_with_op, transform_panel,
    transform_panel_spec, transform_timeseries, transform_timeseries_with_op, CrossSectionalOp,
    PanelOperation, PanelTransformSpec, TimeSeriesOp,
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
