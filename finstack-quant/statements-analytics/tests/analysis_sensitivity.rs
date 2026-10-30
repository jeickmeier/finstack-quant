//! Statement sensitivity: shock semantics, tornado periods and parameter specs.
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::{
    builder::ModelBuilder,
    types::{AmountOrScalar, FinancialModelSpec, ForecastMethod, ForecastSpec},
};
use finstack_quant_statements_analytics::analysis::{
    run_sensitivity, ParameterSpec, SensitivityConfig, SensitivityMode,
};

fn quarter(q: u8) -> PeriodId {
    PeriodId::quarter(2025, q).expect("valid quarter")
}

/// 2025Q1 actual revenue grown by a `curve_pct` forecast through 2025Q4,
/// with a running total that reads every earlier quarter.
fn growth_model() -> FinancialModelSpec {
    let mut model_builder = ModelBuilder::new("growth");
    model_builder.periods("2025Q1..Q4", Some("2025Q1")).unwrap();
    model_builder.value("revenue", &[(quarter(1), AmountOrScalar::scalar(100.0))]);
    let mut params = indexmap::IndexMap::new();
    params.insert("curve".into(), serde_json::json!([0.05, 0.05, 0.05]));
    model_builder.forecast(
        "revenue",
        ForecastSpec {
            method: ForecastMethod::CurvePct,
            params,
        },
    );
    model_builder
        .compute("cumulative_revenue", "cumsum(revenue)")
        .unwrap();
    model_builder.build().unwrap()
}

/// A shock is a point override of one forecast cell: the shocked node's own
/// growth forecast keeps its unshocked anchor in later periods, while formulas
/// that read the shocked cell move.
#[test]
fn sensitivity_shock_is_a_point_override_of_one_forecast_cell() {
    let model = growth_model();
    let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
    config.add_parameter(ParameterSpec::new("revenue", quarter(2), 105.0, vec![50.0]));
    let result = run_sensitivity(&model, &config).unwrap();
    let baseline = result.baseline.as_ref().unwrap();
    let shocked = &result.scenarios[0].results;

    assert_eq!(shocked.get("revenue", &quarter(2)), Some(50.0));
    assert_eq!(
        shocked.get("revenue", &quarter(3)),
        baseline.get("revenue", &quarter(3))
    );
    let cumulative_delta = shocked.get("cumulative_revenue", &quarter(3)).unwrap()
        - baseline.get("cumulative_revenue", &quarter(3)).unwrap();
    assert!((cumulative_delta + 55.0).abs() < 1e-9, "{cumulative_delta}");
}
