//! Financial-model input boundaries that must hold before execution or import.

use finstack_quant_core::contract::{ContractError, LoadLimits, LoadPhase, Severity};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::NodeValueType;
use serde_json::json;
use time::macros::date;

fn quarter(number: u8) -> PeriodId {
    PeriodId::quarter(2025, number).unwrap()
}

fn forecast_model() -> FinancialModelSpec {
    ModelBuilder::new("forecast-identity")
        .periods("2025Q1..Q2", Some("2025Q1"))
        .unwrap()
        .value("revenue", &[(quarter(1), 100.0.into())])
        .forecast("revenue", ForecastSpec::growth(0.1))
        .value("cost", &[(quarter(1), 40.0.into())])
        .forecast("cost", ForecastSpec::growth(0.0))
        .compute("profit", "revenue - cost")
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn strict_json_rejects_node_identity_that_would_share_another_forecast_cache() {
    let mut payload = serde_json::to_value(forecast_model()).unwrap();
    payload["nodes"]["cost"]["node_id"] = json!("revenue");
    let bytes = serde_json::to_vec(&payload).unwrap();

    let error = FinancialModelSpec::from_slice_strict(&bytes, &LoadLimits::default())
        .expect_err("two map entries must not share an embedded forecast-cache identity");
    let ContractError::Report(report) = error else {
        panic!("expected a structured semantic-validation report, got {error:?}");
    };
    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = &report.diagnostics[0];
    assert_eq!(diagnostic.code, "contract/semantic-invalid");
    assert_eq!(diagnostic.phase, LoadPhase::Semantic);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.contract.as_deref(),
        Some("finstack_quant.financial_model")
    );
    assert!(diagnostic.message.contains("Node map key 'cost'"));
    assert!(diagnostic.message.contains("embedded node_id 'revenue'"));

    let model: FinancialModelSpec = serde_json::from_value(payload).unwrap();
    let evaluation_error = Evaluator::new()
        .evaluate(&model)
        .expect_err("ordinary evaluation must reject the same node identity collision");
    assert!(evaluation_error
        .to_string()
        .contains("does not match its embedded node_id"));
}

#[test]
fn independent_node_forecasts_keep_distinct_economics() {
    let result = Evaluator::new().evaluate(&forecast_model()).unwrap();
    assert!((result.get("revenue", &quarter(2)).unwrap() - 110.0).abs() < 1e-12);
    assert_eq!(result.get("cost", &quarter(2)), Some(40.0));
    assert!((result.get("profit", &quarter(2)).unwrap() - 70.0).abs() < 1e-12);
}

#[test]
fn build_rejects_values_that_evaluation_would_silently_drop() {
    let error = ModelBuilder::new("outside-timeline")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (quarter(1), 100.0.into()),
                (quarter(2), 200.0.into()),
                (quarter(3), 300.0.into()),
            ],
        )
        .build()
        .expect_err("an explicit value outside the timeline cannot reach evaluation");
    assert!(error.to_string().contains("2025Q3"));
    assert!(error
        .to_string()
        .contains("not present in the model timeline"));
}

#[test]
fn period_columns_validate_null_only_columns_without_changing_builder_state() {
    let builder = ModelBuilder::new("period-import")
        .periods("2025Q1..Q2", Some("2025Q1"))
        .unwrap()
        .value("existing", &[(quarter(1), 7.0.into())]);
    let error = builder
        .resolve_period_columns(&[quarter(1), quarter(3)], None)
        .expect_err("column membership is independent of its cell values");
    assert!(error.to_string().contains("2025Q3"));
    let retained = builder.build().unwrap();
    assert!(retained.periods[0].is_actual);
    assert_eq!(retained.nodes["existing"].values.as_ref().unwrap().len(), 1);
}

#[test]
fn inferred_import_timeline_includes_gaps_and_checks_interior_columns() {
    let builder = ModelBuilder::new("inferred-import");
    let periods = builder
        .resolve_period_columns(&[quarter(1), quarter(3)], Some("2025Q1"))
        .unwrap();
    assert_eq!(periods.len(), 3);
    assert!(periods[0].is_actual);
    assert!(!periods[1].is_actual);

    assert!(builder
        .resolve_period_columns(&[quarter(1), quarter(4), quarter(3)], None)
        .is_err());
    assert!(builder.resolve_period_columns(&[], None).is_err());
}

#[test]
fn reserved_export_index_cannot_be_used_as_a_node_identifier() {
    let error = ModelBuilder::new("index-collision")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("period_id", &[(quarter(1), 100.0.into())])
        .build()
        .expect_err("wide exports reserve period_id for their timeline column");
    assert!(error.to_string().contains("period_id"));
    assert!(error.to_string().contains("reserved"));
}

#[test]
fn invalid_forecast_parameters_are_rejected_even_when_actual_values_win() {
    let mut model = ModelBuilder::new("unused-forecast")
        .periods("2025Q1..Q1", Some("2025Q1"))
        .unwrap()
        .value("revenue", &[(quarter(1), 100.0.into())])
        .build()
        .unwrap();
    let mut forecast = ForecastSpec::growth(0.1);
    forecast.params.insert("raet".into(), json!(0.2));
    model.nodes.get_mut("revenue").unwrap().forecast = Some(forecast);
    assert!(model.validate_semantics().is_err());
}

#[test]
fn removing_an_explicit_observation_requires_removing_its_availability_date() {
    let mut model = forecast_model();
    let revenue = model.nodes.get_mut("revenue").unwrap();
    revenue
        .availability_dates
        .insert(quarter(1), date!(2025 - 04 - 10));
    revenue.values.as_mut().unwrap().shift_remove(&quarter(1));
    let error = model
        .validate_semantics()
        .expect_err("a release date without its explicit observation is invalid");
    assert!(error
        .to_string()
        .contains("no explicit value for that period"));
}

#[test]
fn named_capital_structure_formulas_keep_native_instrument_currency() {
    let model = ModelBuilder::new("native-versus-reporting")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .add_bond(
            "euro",
            Money::from((1_000_i64, Currency::EUR)),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "EUR-OIS",
        )
        .unwrap()
        .reporting_currency(Currency::USD)
        .value_money(
            "expected_usd",
            &[(quarter(1), Money::from((25_i64, Currency::USD)))],
        )
        .compute("native_interest", "cs.interest_expense.euro")
        .unwrap()
        .compute("reporting_interest", "cs.interest_expense.total")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        model.nodes["native_interest"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::EUR
        })
    );
    assert_eq!(
        model.nodes["reporting_interest"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::USD
        })
    );

    let error = ModelBuilder::from_spec(model)
        .unwrap()
        .compute(
            "invalid_comparison",
            "cs.interest_expense.euro == expected_usd",
        )
        .unwrap()
        .build()
        .expect_err("native EUR interest cannot be compared as USD just because totals use USD");
    assert!(error.to_string().contains("Dimensional mismatch"));
}

#[test]
fn model_period_intervals_must_be_positive_and_disjoint() {
    let mut empty = forecast_model();
    empty.periods[0].end = empty.periods[0].start;
    assert!(empty
        .validate_semantics()
        .unwrap_err()
        .to_string()
        .contains("start before end"));

    let mut overlapping = forecast_model();
    overlapping.periods[1].start = date!(2025 - 03 - 31);
    assert!(overlapping
        .validate_semantics()
        .unwrap_err()
        .to_string()
        .contains("must not overlap"));

    let mut sparse = forecast_model();
    sparse.periods[1].start = date!(2025 - 05 - 01);
    assert!(sparse.validate_semantics().is_ok(), "gaps remain supported");
}
