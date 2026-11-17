//! Financial-model input boundaries that must hold before execution or import.

use finstack_quant_core::contract::{ContractError, LoadLimits, LoadPhase, Severity};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::NodeValueType;
use finstack_quant_statements::types::{FinancialModelSpec, ForecastSpec};
use serde_json::json;
use time::macros::date;

fn quarter(number: u8) -> PeriodId {
    PeriodId::quarter(2025, number).unwrap()
}

fn forecast_model() -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("forecast-identity");
        model_builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        model_builder.value("revenue", &[(quarter(1), 100.0.into())]);
        model_builder.forecast("revenue", ForecastSpec::growth(0.1));
        model_builder.value("cost", &[(quarter(1), 40.0.into())]);
        model_builder.forecast("cost", ForecastSpec::growth(0.0));
        model_builder.compute("profit", "revenue - cost").unwrap();
        model_builder.build().unwrap()
    }
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
    assert!(
        evaluation_error
            .to_string()
            .contains("does not match its embedded node_id")
    );
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
    let error = {
        let mut model_builder = ModelBuilder::new("outside-timeline");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (quarter(1), 100.0.into()),
                (quarter(2), 200.0.into()),
                (quarter(3), 300.0.into()),
            ],
        );
        model_builder
            .build()
            .expect_err("an explicit value outside the timeline cannot reach evaluation")
    };
    assert!(error.to_string().contains("2025Q3"));
    assert!(
        error
            .to_string()
            .contains("not present in the model timeline")
    );
}

#[test]
fn period_columns_validate_null_only_columns_without_changing_builder_state() {
    let builder = {
        let mut model_builder = ModelBuilder::new("period-import");
        model_builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        model_builder.value("existing", &[(quarter(1), 7.0.into())]);
        model_builder
    };
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

    assert!(
        builder
            .resolve_period_columns(&[quarter(1), quarter(4), quarter(3)], None)
            .is_err()
    );
    assert!(builder.resolve_period_columns(&[], None).is_err());
}

#[test]
fn reserved_export_index_cannot_be_used_as_a_node_identifier() {
    let error = {
        let mut model_builder = ModelBuilder::new("index-collision");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("period_id", &[(quarter(1), 100.0.into())]);
        model_builder
            .build()
            .expect_err("wide exports reserve period_id for their timeline column")
    };
    assert!(error.to_string().contains("period_id"));
    assert!(error.to_string().contains("reserved"));
}

#[test]
fn invalid_forecast_parameters_are_rejected_even_when_actual_values_win() {
    let mut model = {
        let mut model_builder = ModelBuilder::new("unused-forecast");
        model_builder.periods("2025Q1..Q1", Some("2025Q1")).unwrap();
        model_builder.value("revenue", &[(quarter(1), 100.0.into())]);
        model_builder.build().unwrap()
    };
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
    assert!(
        error
            .to_string()
            .contains("no explicit value for that period")
    );
}

#[test]
fn named_capital_structure_formulas_keep_native_instrument_currency() {
    let model = {
        let mut model_builder = ModelBuilder::new("native-versus-reporting");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder
            .add_bond(
                "euro",
                Money::from((1_000_i64, Currency::EUR)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2030 - 01 - 01),
                "EUR-OIS",
            )
            .unwrap();
        model_builder.reporting_currency(Currency::USD);
        model_builder.value_money(
            "expected_usd",
            &[(quarter(1), Money::from((25_i64, Currency::USD)))],
        );
        model_builder
            .compute("native_interest", "cs.interest_expense.euro")
            .unwrap();
        model_builder
            .compute("reporting_interest", "cs.interest_expense.total")
            .unwrap();
        model_builder.build().unwrap()
    };
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

    let error = {
        let mut model_builder = ModelBuilder::from_spec(model).unwrap();
        model_builder
            .compute(
                "invalid_comparison",
                "cs.interest_expense.euro == expected_usd",
            )
            .unwrap();
        model_builder
            .build()
            .expect_err("native EUR interest cannot be compared as USD just because totals use USD")
    };
    assert!(error.to_string().contains("Dimensional mismatch"));
}

#[test]
fn model_period_intervals_must_match_their_identifiers_and_be_contiguous() {
    let mut empty = forecast_model();
    empty.periods.periods[0].end = empty.periods.periods[0].start;
    assert!(
        empty
            .validate_semantics()
            .unwrap_err()
            .to_string()
            .contains("not canonical")
    );

    let mut overlapping = forecast_model();
    overlapping.periods.periods[1].start = date!(2025 - 03 - 31);
    assert!(
        overlapping
            .validate_semantics()
            .unwrap_err()
            .to_string()
            .contains("not canonical")
    );

    let mut sparse = forecast_model();
    sparse.periods.periods[1].start = date!(2025 - 05 - 01);
    assert!(
        sparse
            .validate_semantics()
            .unwrap_err()
            .to_string()
            .contains("not canonical")
    );
}

/// STATEMENTS-CONTRACTS-001 worked example: a quarterly USD series on a
/// canonical Q1–Q3 plan may omit its Q2 observation. Omitting Q2 from the
/// plan, or referencing a misspelled node, fails runnable-model validation.
#[test]
fn runnable_model_keeps_missing_cells_and_rejects_plan_gaps_and_open_references() {
    use finstack_quant_statements::evaluator::CellSource;

    let usd = |amount: i64| Money::from((amount, Currency::USD)).into();
    let mut builder = ModelBuilder::new("admission");
    builder.periods("2025Q1..Q3", None).unwrap();
    builder.value("sales", &[(quarter(1), usd(100)), (quarter(3), usd(120))]);
    builder.compute("double", "sales * 2").unwrap();
    let model = builder
        .build()
        .expect("an omitted observation is a valid model");
    assert_eq!(
        model.nodes["sales"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::USD
        })
    );

    let result = Evaluator::new().evaluate(&model).unwrap();
    assert_eq!(result.get("sales", &quarter(1)), Some(100.0));
    assert!(result.get("sales", &quarter(2)).unwrap().is_nan());
    assert_eq!(
        result.get_source("sales", &quarter(2)),
        Some(CellSource::Missing)
    );
    assert!(result.get("double", &quarter(2)).unwrap().is_nan());
    assert_eq!(result.get("double", &quarter(3)), Some(240.0));

    // The same model without Q2 in its plan is rejected by every load path.
    let mut gap = serde_json::to_value(&model).unwrap();
    gap["periods"]["periods"].as_array_mut().unwrap().remove(1);
    let error = FinancialModelSpec::from_value(gap.clone()).expect_err("a plan gap is rejected");
    assert!(error.to_string().contains("contiguous"), "{error}");
    FinancialModelSpec::from_json(&gap.to_string()).expect_err("from_json rejects the gap");
    FinancialModelSpec::from_slice_strict(gap.to_string().as_bytes(), &LoadLimits::default())
        .expect_err("the strict loader rejects the gap");
    let mut direct = model;
    direct.periods.periods.remove(1);
    Evaluator::new()
        .evaluate(&direct)
        .expect_err("evaluation revalidates a mutated model");

    let mut misspelled = ModelBuilder::new("admission");
    misspelled.periods("2025Q1..Q3", None).unwrap();
    misspelled.value("sales", &[]);
    misspelled.compute("double", "slaes * 2").unwrap();
    let error = misspelled
        .build()
        .expect_err("an open reference is rejected");
    assert!(
        error.to_string().contains("Unknown identifier 'slaes'"),
        "{error}"
    );
}

/// Clause 2: each node kind admits only its own value sources, and every
/// configured source is validated.
#[test]
fn node_kinds_admit_only_their_own_sources() {
    use finstack_quant_statements::types::{NodeSpec, NodeType};

    let base = || {
        let mut builder = ModelBuilder::new("kinds");
        builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        builder.value("revenue", &[(quarter(1), 100.0.into())]);
        builder.build().unwrap()
    };
    let rejected = |node: NodeSpec, expected: &str| {
        let mut model = base();
        model.add_node(node);
        let error = model.validate_semantics().expect_err(expected);
        assert!(error.to_string().contains(expected), "{error}");
    };
    rejected(
        NodeSpec::new("v", NodeType::Value).with_formula("revenue"),
        "cannot have a formula",
    );
    rejected(
        NodeSpec::new("v", NodeType::Value).with_forecast(ForecastSpec::growth(0.1)),
        "cannot have a forecast",
    );
    rejected(NodeSpec::new("c", NodeType::Calculated), "needs a formula");
    rejected(
        NodeSpec::new("c", NodeType::Calculated)
            .with_formula("revenue")
            .with_forecast(ForecastSpec::growth(0.1)),
        "cannot have a forecast",
    );
    rejected(
        NodeSpec::new("m", NodeType::Mixed).with_formula("revenue +"),
        "Invalid formula",
    );
    rejected(
        NodeSpec::new("a-b", NodeType::Value),
        "not a valid identifier",
    );

    let mut mixed = base();
    mixed.add_node(
        NodeSpec::new("m", NodeType::Mixed)
            .with_formula("revenue")
            .with_forecast(ForecastSpec::growth(0.1)),
    );
    mixed
        .validate_semantics()
        .expect("a Mixed node takes every source");

    // The builder promotes a Value node to Mixed when a forecast is attached.
    assert_eq!(forecast_model().nodes["revenue"].node_type, NodeType::Mixed);
}

/// Clause 3: observations are finite; a missing cell is absent. Clause 4: a
/// runnable model needs a node or an evaluable capital structure.
#[test]
fn observations_are_finite_and_a_runnable_model_is_not_empty() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut builder = ModelBuilder::new("finite");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.value("x", &[(quarter(1), value.into())]);
        let error = builder
            .build()
            .expect_err("a non-finite observation is rejected");
        assert!(error.to_string().contains("non-finite"), "{error}");

        let mut direct = forecast_model();
        direct
            .nodes
            .get_mut("revenue")
            .unwrap()
            .values
            .as_mut()
            .unwrap()
            .insert(quarter(1), value.into());
        direct
            .validate_semantics()
            .expect_err("a directly mutated model is revalidated");
    }
    assert!(Money::new(f64::NAN, Currency::USD).is_err());

    let mut typed_empty = ModelBuilder::new("typed-empty");
    typed_empty.periods("2025Q1..Q2", None).unwrap();
    typed_empty.value_money("cash", &[]);
    typed_empty
        .build()
        .expect("an empty series is a valid node");

    let mut empty = ModelBuilder::new("empty");
    empty.periods("2025Q1..Q2", None).unwrap();
    let error = empty
        .build()
        .expect_err("a model with no node is not runnable");
    assert!(error.to_string().contains("at least one node"), "{error}");
}
