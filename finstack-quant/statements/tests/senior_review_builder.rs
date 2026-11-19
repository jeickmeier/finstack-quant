//! Failure-atomic model construction regressions.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;

#[test]
fn failed_model_setters_preserve_state_and_allow_retry() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut retry = ModelBuilder::new("recoverable");
    let mut clean = ModelBuilder::new("recoverable");
    for builder in [&mut retry, &mut clean] {
        builder.value_scalar("revenue", &[(q1, 100.0)]);
        builder.with_meta("marker", serde_json::json!({"retained": true}));
    }
    assert!(
        retry
            .periods_explicit(
                finstack_quant_core::dates::PeriodPlan::try_new(
                    finstack_quant_core::dates::PeriodCalendar::Gregorian,
                    Vec::new()
                )
                .expect("valid period plan")
            )
            .is_err()
    );
    assert!(retry.periods("not a period range", None).is_err());
    retry
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("retry timeline");
    assert!(retry.periods("2026Q1..Q2", None).is_err());
    assert!(retry.compute("revenue", "(").is_err());
    assert!(retry.compute("__reserved", "revenue").is_err());
    assert!(retry.availability_dates("unknown", &[]).is_err());
    assert!(
        retry
            .add_metric_from_registry(
                "missing.metric",
                &finstack_quant_statements::registry::Registry::new()
            )
            .is_err()
    );
    clean
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("clean timeline");
    for builder in [&mut retry, &mut clean] {
        builder.forecast(
            "revenue",
            finstack_quant_statements::types::ForecastSpec::forward_fill(),
        );
        builder
            .compute("profit", "revenue * 0.5")
            .expect("valid formula");
    }
    let actual = retry.build().expect("build after rejected setters");
    let expected = clean.build().expect("build without rejected setters");
    assert_eq!(
        serde_json::to_value(actual).expect("serialize actual"),
        serde_json::to_value(expected).expect("serialize expected")
    );
}

#[test]
fn failed_mixed_formula_preserves_values_forecast_name_and_parent() {
    use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut parent = ModelBuilder::new("mixed-recovery");
    parent
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("timeline");
    parent.value_scalar("base", &[(q1, 100.0)]);
    parent.with_meta("marker", serde_json::json!("retained"));
    let mut mixed = parent.mixed("mixed");
    mixed.values(&[(q1, AmountOrScalar::scalar(40.0))]);
    mixed.forecast(ForecastSpec::forward_fill());
    mixed.name("Retained name");
    assert!(mixed.formula("(").is_err());
    mixed.formula("base * 0.5").expect("retry formula");
    let ready = mixed.build().expect("finish mixed node");
    let model = ready.build().expect("finish parent model");
    let node = model.nodes.get("mixed").expect("mixed node retained");
    assert_eq!(node.name.as_deref(), Some("Retained name"));
    assert_eq!(node.formula_text.as_deref(), Some("base * 0.5"));
    assert!(node.forecast.is_some());
    assert_eq!(
        node.values.as_ref().expect("values retained").get(&q1),
        Some(&AmountOrScalar::scalar(40.0))
    );
    assert!(model.has_node("base"));
    assert_eq!(
        model.meta.get("marker"),
        Some(&serde_json::json!("retained"))
    );
}
