//! Economic invariants and boundary regressions from the features audit.
use finstack_quant_features::*;
use serde_json::{json, Value};

fn keys(n: usize) -> (Vec<String>, Vec<String>) {
    (
        vec!["A".into(); n],
        (0..n).map(|i| format!("{i:04}")).collect(),
    )
}
fn ts(
    values: &[Option<f64>],
    op: &str,
    params: Value,
) -> finstack_quant_core::Result<Vec<Option<f64>>> {
    let (entity, order) = keys(values.len());
    transform_timeseries(values, &entity, &order, op, Some(&params))
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-11 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn dimensionless_features_do_not_depend_on_measurement_units() {
    let data = [0.0, 1.0, 1.0, 4.0];
    let (entity, order) = keys(data.len());
    for scale in [1e-150, 1e-7, 1.0, 1e7, 1e150] {
        let values: Vec<_> = data.iter().map(|x| Some(x * scale)).collect();
        let base: Vec<_> = data.iter().copied().map(Some).collect();
        for op in ["rolling_skew", "rolling_kurtosis", "rolling_zscore"] {
            close(
                ts(&values, op, json!({"window":4})).unwrap()[3].unwrap(),
                ts(&base, op, json!({"window":4})).unwrap()[3].unwrap(),
            );
        }
        for op in [
            "zscore",
            "robust_zscore",
            "minmax_scale",
            "long_short_weights",
        ] {
            let actual = transform_cross_sectional(&values, &entity, op, None).unwrap();
            let expected = transform_cross_sectional(&base, &entity, op, None).unwrap();
            for (a, b) in actual.iter().zip(expected) {
                close(a.unwrap(), b.unwrap());
            }
        }
        let corr = transform_timeseries_pairwise(
            &values,
            &values,
            &entity,
            &order,
            "rolling_corr",
            Some(&json!({"window":4})),
        )
        .unwrap();
        close(corr[3].unwrap(), 1.0);
        let beta = transform_timeseries_pairwise(
            &values,
            &values,
            &entity,
            &order,
            "rolling_beta",
            Some(&json!({"window":4})),
        )
        .unwrap();
        close(beta[3].unwrap(), 1.0);
        let vol = ts(&values, "ewma_vol", json!({"span":3})).unwrap();
        let base_vol = ts(&base, "ewma_vol", json!({"span":3})).unwrap();
        close(vol[3].unwrap() / scale, base_vol[3].unwrap());
    }
    close(
        ts(
            &[Some(0.), Some(0.), Some(1e-7)],
            "rolling_skew",
            json!({"window":3}),
        )
        .unwrap()[2]
            .unwrap(),
        3.0_f64.sqrt(),
    );
}

#[test]
fn invalid_parameters_fail_before_any_window_is_ready() {
    for values in [vec![], vec![None], vec![Some(1.)], vec![Some(1.); 4]] {
        for (op, params) in [
            ("ewma_mean", json!({"span":0.5})),
            ("rolling_quantile", json!({"window":4,"quantile":2})),
            (
                "rolling_winsorize",
                json!({"window":4,"lower":0.9,"upper":0.1}),
            ),
            ("hampel_filter", json!({"window":4,"threshold":-1})),
            ("rolling_mean", json!({"window":1,"min_periods":2})),
        ] {
            assert!(ts(&values, op, params).is_err(), "{op}: {values:?}");
        }
        let (time, _) = keys(values.len());
        for (op, params) in [
            ("winsorize", json!({"lower":2})),
            ("cap_weights", json!({"max_abs":0})),
            ("clip", json!({"lower":2,"upper":1})),
        ] {
            assert!(transform_cross_sectional(&values, &time, op, Some(&params)).is_err());
            assert!(
                transform_cross_sectional_grouped(&values, &time, &time, op, Some(&params))
                    .is_err()
            );
        }
    }
}

#[test]
fn ewma_distinguishes_initialization_from_zero_volatility() {
    let values = [Some(0.01), None, Some(0.01), Some(0.01)];
    assert_eq!(
        ts(&values, "ewma_vol", json!({"span":3})).unwrap(),
        vec![None, None, Some(0.), Some(0.)]
    );
    assert_eq!(
        ts(&values, "ewma_zscore", json!({"span":3})).unwrap(),
        vec![Some(0.), None, Some(0.), Some(0.)]
    );
    assert_eq!(
        ts(&[Some(1.), Some(2.)], "ewma_mean", json!({"span":1})).unwrap(),
        vec![Some(1.), Some(2.)]
    );
    assert_eq!(
        ts(&[Some(1.), Some(2.)], "ewma_vol", json!({"span":1})).unwrap(),
        vec![None, Some(0.)]
    );
}

#[test]
fn hampel_replaces_a_spike_in_a_flat_history_at_any_scale() {
    for scale in [1e-150, 1.0, 1e150] {
        let result = ts(
            &[Some(scale), Some(scale), Some(100. * scale)],
            "hampel_filter",
            json!({"window":3}),
        )
        .unwrap();
        close(result[2].unwrap() / scale, 1.0);
    }
}

#[test]
fn rolling_windows_and_slope_preserve_row_gaps() {
    assert_eq!(
        ts(
            &[Some(1.), None],
            "rolling_mean",
            json!({"window":2,"min_periods":1})
        )
        .unwrap(),
        vec![Some(1.), Some(1.)]
    );
    close(
        ts(
            &[Some(0.), None, Some(2.)],
            "rolling_slope",
            json!({"window":3,"min_periods":2}),
        )
        .unwrap()[2]
            .unwrap(),
        1.0,
    );
    let (entity, order) = keys(3);
    let values = [Some(1.), None, Some(2.)];
    assert_eq!(
        transform_timeseries_pairwise(
            &values,
            &values,
            &entity,
            &order,
            "rolling_corr",
            Some(&json!({"window":2}))
        )
        .unwrap(),
        vec![None; 3]
    );
    close(
        transform_timeseries_pairwise(
            &values,
            &values,
            &entity,
            &order,
            "rolling_corr",
            Some(&json!({"window":3,"min_periods":2})),
        )
        .unwrap()[2]
            .unwrap(),
        1.0,
    );
    assert_eq!(
        ts(&values, "lag", json!({})).unwrap(),
        vec![None, None, Some(1.)]
    );
}

#[test]
fn signed_zero_never_creates_a_rank_signal() {
    let values = [Some(-0.0), Some(0.0)];
    let (time, _) = keys(2);
    assert_eq!(rank_to_weights(&values, &time).unwrap(), vec![Some(0.0); 2]);
    for op in [
        "rank",
        "percentile_rank",
        "normal_score_transform",
        "quantile_bucket",
    ] {
        let result = transform_cross_sectional(&values, &time, op, None).unwrap();
        assert_eq!(result[0], result[1]);
    }
    assert_eq!(
        ts(&values, "rolling_rank", json!({"window":2})).unwrap()[1],
        Some(0.)
    );
}

#[test]
fn ols_residuals_are_invariant_to_exposure_units() {
    let (time, order) = keys(3);
    let y = [Some(1.), Some(0.), Some(0.)];
    for scale in [1e-150, 1e-7, 1., 1e7, 1e150] {
        let x = vec![vec![Some(scale), Some(2. * scale), Some(3. * scale)]];
        let result = neutralize(&y, &time, &x, None).unwrap();
        for (actual, expected) in result.iter().zip([1. / 6., -1. / 3., 1. / 6.]) {
            close(actual.unwrap(), expected);
        }
        let residual =
            rolling_regression_residual(&y, &x, &time, &order, Some(&json!({"window":3}))).unwrap();
        close(residual[2].unwrap(), 1. / 6.);
        assert!(neutralize(&y, &time, &[x[0].clone(), x[0].clone()], None).is_err());
    }
}

#[test]
fn standardized_residuals_preserve_neutrality_and_do_not_amplify_roundoff() {
    let (time, _) = keys(3);
    let x = vec![vec![Some(1.), Some(2.), Some(3.)]];
    assert!(neutralize_and_zscore(
        &[Some(1.), Some(0.), Some(0.)],
        &time,
        &x,
        Some(&json!({"fit_intercept":false}))
    )
    .is_err());
    let residual = neutralize_and_zscore(&[Some(1.), Some(0.), Some(0.)], &time, &x, None).unwrap();
    close(residual.iter().map(|v| v.unwrap()).sum(), 0.);
    close(
        residual
            .iter()
            .zip([1., 2., 3.])
            .map(|(v, x)| v.unwrap() * x)
            .sum(),
        0.,
    );
    assert_eq!(
        neutralize_and_zscore(&[Some(3.), Some(5.), Some(7.)], &time, &x, None).unwrap(),
        vec![Some(0.); 3]
    );
}

#[test]
fn final_weight_caps_preserve_neutrality_and_reject_infeasible_allocations() {
    let (time, _) = keys(4);
    let params = json!({"max_abs":0.3});
    let result = transform_cross_sectional(
        &[Some(-10.), Some(-1.), Some(1.), Some(10.)],
        &time,
        "cap_weights",
        Some(&params),
    )
    .unwrap();
    for (actual, expected) in result.iter().zip([-0.3, -0.2, 0.2, 0.3]) {
        close(actual.unwrap(), expected);
    }
    close(result.iter().map(|v| v.unwrap()).sum(), 0.);
    close(result.iter().map(|v| v.unwrap().abs()).sum(), 1.);
    assert!(result.iter().all(|v| v.unwrap().abs() <= 0.3));
    assert!(transform_cross_sectional(
        &[Some(-1.), Some(1.)],
        &time[..2],
        "cap_weights",
        Some(&params)
    )
    .is_err());
    assert_eq!(
        transform_cross_sectional(&[Some(2.); 4], &time, "cap_weights", Some(&params)).unwrap(),
        vec![Some(0.); 4]
    );
    assert!(
        risk_scaled_weights(&[Some(1.), Some(2.)], &time[..2], &[Some(1.), Some(-1.)]).is_err()
    );
}

#[test]
fn finite_results_survive_json_and_overflow_is_an_error() {
    let values = [Some(1e308), Some(1e308)];
    assert_eq!(
        ts(&values, "rolling_mean", json!({"window":2})).unwrap()[1],
        Some(1e308)
    );
    assert!(ts(&values, "rolling_sum", json!({"window":2})).is_err());
    let mut spec = json!({"values":[1e308,1e308],"entity":["a","a"],"order":["1","2"],"operations":[{"name":"m","family":"timeseries","op":"rolling_mean","params":{"window":2}}]});
    let result: Value =
        serde_json::from_str(&transform_panel_json(&spec.to_string()).unwrap()).unwrap();
    assert_eq!(result["columns"][0]["values"][1].as_f64(), Some(1e308));
    spec["operations"][0]["op"] = json!("rolling_sum");
    assert!(transform_panel_json(&spec.to_string()).is_err());
    let (time, _) = keys(2);
    let opposite = [Some(-1e308), Some(1e308)];
    assert_eq!(
        transform_cross_sectional(&opposite, &time, "zscore", None).unwrap(),
        vec![Some(-1.), Some(1.)]
    );
    assert_eq!(
        ts(&opposite, "rolling_quantile", json!({"window":2})).unwrap()[1],
        Some(0.)
    );
}
