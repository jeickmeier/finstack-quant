//! Strict serialization tests for dynamic term-structure models.

use finstack_quant_models::rates::dtsm::{
    nelson_siegel_yields, DieboldLi, FactorTimeSeries, YieldForecast, YieldPanel, YieldPca,
    YieldPcaView,
};
use nalgebra::{DMatrix, DVector};

fn panel() -> YieldPanel {
    let tenors = vec![1.0, 5.0, 10.0, 20.0];
    let yields = (0..30)
        .map(|t| {
            let x = t as f64;
            nelson_siegel_yields(
                0.7308,
                [
                    0.02 + 0.001 * (0.3 * x).sin(),
                    -0.01 + 0.002 * (0.7 * x).cos(),
                    0.005 + 0.003 * (1.1 * x).sin(),
                ],
                &tenors,
            )
            .unwrap()
        })
        .collect();
    YieldPanel::from_rows(tenors, yields, None).unwrap()
}

fn assert_strict_inbound<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_value(value).unwrap();
    let _: T = serde_json::from_value(json.clone()).unwrap();

    let mut tampered = json;
    tampered
        .as_object_mut()
        .expect("serialized state should be a JSON object")
        .insert("typo_field".to_string(), serde_json::json!(1));
    assert!(serde_json::from_value::<T>(tampered).is_err());
}

#[test]
fn diebold_li_rejects_unknown_fields() {
    let model = DieboldLi::with_default_lambda();
    assert_strict_inbound(&model);
}

#[test]
fn diebold_li_rejects_invalid_lambda_on_deserialize() {
    let model = DieboldLi::with_default_lambda();
    let mut json = serde_json::to_value(&model).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("lambda".to_string(), serde_json::json!(-1.0));
    assert!(serde_json::from_value::<DieboldLi>(json).is_err());
}

#[test]
fn validated_dtsm_states_round_trip_without_changing_forecasts_or_scenarios() {
    let panel = panel();
    let model = DieboldLi::with_default_lambda().fit(&panel).unwrap();
    let forecast = model.forecast(2).unwrap();
    let pca = YieldPca::fit(&panel).unwrap();
    let view = pca.truncated(2).unwrap();
    assert_strict_inbound(&panel);
    assert_strict_inbound(model.factors().unwrap());
    assert_strict_inbound(&model);
    assert_strict_inbound(&forecast);
    assert_strict_inbound(&pca);
    assert_strict_inbound(&view);
    let restored: DieboldLi =
        serde_json::from_str(&serde_json::to_string(&model).unwrap()).unwrap();
    assert_eq!(restored.forecast(2).unwrap().yields, forecast.yields);
    let restored: YieldPca = serde_json::from_str(&serde_json::to_string(&pca).unwrap()).unwrap();
    assert_eq!(
        restored.scenario(&[1.0]).unwrap(),
        pca.scenario(&[1.0]).unwrap()
    );
}

#[test]
fn yield_panel_deserialization_rejects_constructor_invalid_shapes() {
    let empty = serde_json::json!({"yields": [[], 0, 0], "tenors": [1.0], "dates": null});
    assert!(serde_json::from_value::<YieldPanel>(empty).is_err());
    let mut value = serde_json::to_value(panel()).unwrap();
    value["tenors"] = serde_json::json!([1.0]);
    assert!(serde_json::from_value::<YieldPanel>(value).is_err());
}

#[test]
fn diebold_li_deserialization_rejects_incomplete_or_misaligned_fitted_state() {
    let model = DieboldLi::with_default_lambda().fit(&panel()).unwrap();
    let value = serde_json::to_value(model).unwrap();
    for (field, replacement) in [
        ("q_cov", serde_json::Value::Null),
        (
            "mu",
            serde_json::to_value(DVector::<f64>::zeros(2)).unwrap(),
        ),
        (
            "phi",
            serde_json::to_value(DMatrix::<f64>::zeros(2, 2)).unwrap(),
        ),
        ("tenors", serde_json::json!([1.0])),
    ] {
        let mut invalid = value.clone();
        invalid[field] = replacement;
        assert!(
            serde_json::from_value::<DieboldLi>(invalid).is_err(),
            "accepted invalid {field}"
        );
    }
    let mut invalid = value;
    invalid["factors"]["factors"] = serde_json::json!([[], 0, 3]);
    assert!(serde_json::from_value::<DieboldLi>(invalid).is_err());
}

#[test]
fn dtsm_results_reject_shapes_that_would_break_host_accessors() {
    let panel = panel();
    let model = DieboldLi::with_default_lambda().fit(&panel).unwrap();
    let mut factors = serde_json::to_value(model.factors().unwrap()).unwrap();
    factors["factors"] = serde_json::to_value(DMatrix::<f64>::zeros(30, 2)).unwrap();
    assert!(serde_json::from_value::<FactorTimeSeries>(factors).is_err());

    let mut forecast = serde_json::to_value(model.forecast(1).unwrap()).unwrap();
    forecast["lower_95"] = serde_json::json!([]);
    assert!(serde_json::from_value::<YieldForecast>(forecast).is_err());

    let pca = YieldPca::fit(&panel).unwrap();
    let mut state = serde_json::to_value(&pca).unwrap();
    state["scores"] = serde_json::to_value(DMatrix::<f64>::zeros(29, 1)).unwrap();
    assert!(serde_json::from_value::<YieldPca>(state).is_err());
    let mut view = serde_json::to_value(pca.truncated(2).unwrap()).unwrap();
    view["loadings"][0] = serde_json::json!([]);
    assert!(serde_json::from_value::<YieldPcaView>(view).is_err());
}
