//! Financial-model input boundaries that must hold before execution or import.

use finstack_quant_core::contract::{ContractError, LoadLimits};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::NodeValueType;
use finstack_quant_statements::types::{FinancialModelSpec, ForecastFadeShape, ForecastSpec};
use indexmap::IndexMap;
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
    let ContractError::Report(report) = &error else {
        panic!("expected a located diagnostic, got {error:?}");
    };
    let diagnostic = &report.diagnostics[0];
    assert_eq!(diagnostic.code, "statements/build");
    assert_eq!(diagnostic.pointer.as_deref(), Some("/nodes/cost"));
    assert!(diagnostic.message.contains("Node map key 'cost'"));
    assert!(diagnostic.message.contains("embedded node_id 'revenue'"));
    // The error text alone names the failure.
    assert!(error.to_string().contains("Node map key 'cost'"), "{error}");

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

/// STATEMENTS-CONTRACTS-002 #3: an import needs a plan, maps unique columns
/// by period label in any order, keeps sparse cells missing and known zeros
/// distinct, and rejects a column outside the plan.
#[test]
fn observation_import_is_plan_validated_sparse_and_atomic() {
    use finstack_quant_statements::types::{AmountOrScalar, NodeId};

    let usd = |amount: i64| Some(AmountOrScalar::from(Money::from((amount, Currency::USD))));
    let scalar = |value: f64| Some(AmountOrScalar::scalar(value));
    let row = |id: &str, cells: Vec<Option<AmountOrScalar>>| (NodeId::new(id), cells);
    let base = || {
        let mut builder = ModelBuilder::new("period-import");
        builder.periods("2025Q1..Q3", Some("2025Q1")).unwrap();
        builder.value("existing", &[(quarter(1), 7.0.into())]);
        builder
    };

    // Unordered columns, a sparse row, a known zero and an all-missing column.
    let mut builder = base();
    builder
        .import_observations(
            &["2025Q3", "2025Q1", "2025Q2"],
            &[
                row("revenue", vec![scalar(300.0), scalar(100.0), None]),
                row("cash", vec![usd(0), usd(50), None]),
            ],
        )
        .unwrap();
    let model = builder.build().unwrap();
    let values = |id: &str| model.nodes[id].values.clone().unwrap();
    assert_eq!(values("revenue").len(), 2);
    assert_eq!(values("revenue")[&quarter(1)].value(), 100.0);
    assert_eq!(values("revenue")[&quarter(3)].value(), 300.0);
    assert!(!values("revenue").contains_key(&quarter(2)));
    assert_eq!(
        values("cash")[&quarter(3)].value(),
        0.0,
        "a zero is a known zero"
    );
    assert_eq!(
        model.nodes["cash"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::USD
        })
    );

    // A row with no observation has no units; the model needs them declared.
    let mut unresolved = base();
    unresolved
        .import_observations(&["2025Q1"], &[row("empty", vec![None])])
        .unwrap();
    let error = unresolved
        .build()
        .expect_err("an empty undeclared series has no units");
    assert!(
        error
            .to_string()
            .contains("Cannot resolve the units of node 'empty'"),
        "{error}"
    );
    assert!(
        model.periods[0].is_actual,
        "the plan keeps its actuals flags"
    );

    let rejected =
        |columns: &[&str], rows: Vec<(NodeId, Vec<Option<AmountOrScalar>>)>, expected: &str| {
            let mut builder = base();
            let error = builder
                .import_observations(columns, &rows)
                .map(|_| ())
                .expect_err(expected);
            assert!(error.to_string().contains(expected), "{error}");
            assert_eq!(
                builder.nodes().len(),
                1,
                "a rejected import changes nothing"
            );
        };
    let good = || vec![row("revenue", vec![scalar(1.0), scalar(2.0)])];
    rejected(
        &["2025Q1", "2025Q4"],
        good(),
        "not present in the model timeline",
    );
    rejected(&["2025Q1", "2025Q1"], good(), "more than once");
    rejected(&["2025Q1", "typo"], good(), "Period column 'typo'");
    rejected(&[], vec![], "at least one period column");
    rejected(
        &["2025Q1", "2025Q2"],
        vec![row("revenue", vec![scalar(1.0)])],
        "has 1 cells for 2 period columns",
    );
    rejected(
        &["2025Q1", "2025Q2"],
        vec![row("revenue", vec![scalar(1.0), usd(2)])],
        "",
    );
    rejected(
        &["2025Q1", "2025Q2"],
        vec![row("revenue", vec![scalar(f64::NAN), None])],
        "not finite",
    );
    rejected(
        &["2025Q1", "2025Q2"],
        vec![row("existing", vec![scalar(1.0), None])],
        "defined more than once",
    );
    rejected(
        &["2025Q1", "2025Q2"],
        vec![good().remove(0), good().remove(0)],
        "defined more than once",
    );
    rejected(
        &["2025Q1", "2025Q2"],
        vec![row("not-valid", vec![None, None])],
        "not a valid identifier",
    );

    let mut no_plan = ModelBuilder::new("no-plan");
    let error = no_plan
        .import_observations(&["2025Q1"], &good())
        .map(|_| ())
        .expect_err("an import never infers the timeline");
    assert!(
        error.to_string().contains("Set the model timeline"),
        "{error}"
    );
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
    // The only period is an actual with an observation, so none of these
    // forecasts would ever run. Their static requirements still hold.
    model.nodes.get_mut("revenue").unwrap().node_type =
        finstack_quant_statements::types::NodeType::Mixed;
    for (forecast, cause) in [
        (
            ForecastSpec::growth(0.1)
                .with_param("min", json!(5.0))
                .unwrap()
                .with_param("max", json!(1.0))
                .unwrap(),
            "'min' (5) is above 'max' (1)",
        ),
        (
            ForecastSpec::fade_to_target(1.0, ForecastFadeShape::Linear)
                .with_param("half_life", json!(2.0))
                .unwrap(),
            "'half_life' applies only to the exponential shape",
        ),
        (
            ForecastSpec::normal(0.0, -1.0, 7),
            "'std_dev' must not be negative",
        ),
        (
            ForecastSpec::growth(f64::INFINITY),
            "'rate' must be a finite number",
        ),
        (
            ForecastSpec::normal(0.0, 1.0, 7)
                .with_param("correlation_with", json!("nobody"))
                .unwrap()
                .with_param("correlation", json!(0.5))
                .unwrap(),
            "correlation peer 'nobody'",
        ),
        (
            ForecastSpec::overrides(IndexMap::from([(quarter(3), 1.0)])),
            "override key '2025Q3' is not a period of the model",
        ),
    ] {
        model.nodes.get_mut("revenue").unwrap().forecast = Some(forecast);
        let error = model.validate_semantics().expect_err(cause);
        assert!(error.to_string().contains(cause), "{cause}: {error}");
    }
    model.nodes.get_mut("revenue").unwrap().forecast = Some(ForecastSpec::growth(0.1));
    model.validate_semantics().unwrap();
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
        model_builder.add_debt(
            "euro",
            fixed_bond(
                "euro",
                Money::from((1_000_i64, Currency::EUR)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2030 - 01 - 01),
                "EUR-OIS",
            ),
        );
        model_builder.reporting_currency(Currency::USD);
        model_builder.fx_policy(finstack_quant_statements::types::CapitalFxPolicy::PeriodEnd);
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

/// A plan's intervals cannot disagree with their identifiers: the plan
/// constructor refuses them, so only a gap can reach model validation.
#[test]
fn model_period_intervals_must_match_their_identifiers_and_be_contiguous() {
    use finstack_quant_core::dates::PeriodPlan;
    let model = forecast_model();
    let calendar = model.periods.get_calendar();

    let mut empty = model.periods.to_vec();
    empty[0].end = empty[0].start;
    let mut overlapping = model.periods.to_vec();
    overlapping[1].start = date!(2025 - 03 - 31);
    for rows in [empty, overlapping] {
        let error = PeriodPlan::try_new(calendar, rows).unwrap_err();
        assert!(error.to_string().contains("calendar bounds"), "{error}");
    }

    let mut sparse = model;
    let mut rows = finstack_quant_core::dates::build_periods("2025Q1..Q3", Some("2025Q1"))
        .unwrap()
        .into_periods();
    rows.remove(1);
    sparse.periods = PeriodPlan::try_new(calendar, rows).unwrap();
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
    assert!(result.get("sales", &quarter(2)).is_none());
    assert_eq!(result.get_source("sales", &quarter(2)), None);
    assert!(result.get("double", &quarter(2)).is_none());
    assert_eq!(result.get("double", &quarter(3)), Some(240.0));

    // The same model without Q2 in its plan is rejected by every load path.
    let mut gap = serde_json::to_value(&model).unwrap();
    gap["periods"]["periods"].as_array_mut().unwrap().remove(1);
    let error =
        FinancialModelSpec::from_json(&gap.to_string()).expect_err("a plan gap is rejected");
    assert!(error.to_string().contains("contiguous"), "{error}");
    let mut direct = model;
    let mut rows = direct.periods.to_vec();
    rows.remove(1);
    direct.periods =
        finstack_quant_core::dates::PeriodPlan::try_new(direct.periods.get_calendar(), rows)
            .unwrap();
    Evaluator::new()
        .evaluate(&direct)
        .expect_err("evaluation revalidates a mutated model");

    let mut misspelled = ModelBuilder::new("admission");
    misspelled.periods("2025Q1..Q3", None).unwrap();
    misspelled.value_scalar("sales", &[]);
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

    // A declared type resolves an empty series; an undeclared one does not.
    let empty_node = |declared: Option<NodeValueType>| {
        let mut node = finstack_quant_statements::types::NodeSpec::new(
            "cash",
            finstack_quant_statements::types::NodeType::Value,
        );
        node.values = Some(Default::default());
        node.value_type = declared;
        let mut builder = ModelBuilder::new("typed-empty");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.insert_node(node);
        builder.build()
    };
    let typed = empty_node(Some(NodeValueType::Monetary {
        currency: Currency::USD,
    }))
    .expect("a declared type resolves an empty series");
    assert_eq!(
        typed.nodes["cash"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::USD
        })
    );
    let error = empty_node(None).expect_err("an undeclared empty series is unresolved");
    assert!(
        error
            .to_string()
            .contains("Cannot resolve the units of node 'cash'"),
        "{error}"
    );

    let mut empty = ModelBuilder::new("empty");
    empty.periods("2025Q1..Q2", None).unwrap();
    let error = empty
        .build()
        .expect_err("a model with no node is not runnable");
    assert!(error.to_string().contains("at least one node"), "{error}");
}

/// STATEMENTS-CONTRACTS-002 #1–#2: named edits commit atomically, a forecast
/// attaches to an existing node preserving its formula and observations, and
/// a rejected batch leaves the model unchanged.
#[test]
fn named_edits_commit_atomically() {
    use finstack_quant_statements::types::{ModelEdit, NodeId, NodeSpec, NodeType};

    let mut model = forecast_model();
    let before = serde_json::to_value(&model).unwrap();
    let node = |id: &str| NodeId::new(id);

    model
        .apply_edits(&[
            ModelEdit::SetObservation {
                node_id: node("cost"),
                period: quarter(1),
                value: 45.0.into(),
                availability_date: Some(date!(2025 - 04 - 25)),
            },
            ModelEdit::SetForecast {
                node_id: node("profit"),
                forecast: ForecastSpec::growth(0.0),
            },
            ModelEdit::AddNode {
                node: NodeSpec::new("margin", NodeType::Calculated)
                    .with_formula("profit / revenue"),
            },
            ModelEdit::SetWhere {
                node_id: node("margin"),
                predicate: "revenue > 0".into(),
            },
        ])
        .expect("a valid batch commits");
    assert_eq!(
        model.nodes["cost"].values.as_ref().unwrap()[&quarter(1)].value(),
        45.0
    );
    assert_eq!(
        model.nodes["cost"].availability_dates[&quarter(1)],
        date!(2025 - 04 - 25)
    );
    let profit = &model.nodes["profit"];
    assert_eq!(
        profit.node_type,
        NodeType::Mixed,
        "a forecast promotes to Mixed"
    );
    assert_eq!(profit.formula_text.as_deref(), Some("revenue - cost"));
    assert!(profit.forecast.is_some());
    assert_eq!(
        model.nodes["margin"].where_text.as_deref(),
        Some("revenue > 0")
    );

    // Each batch below fails on its last edit or on revalidation; none commits.
    let committed = serde_json::to_value(&model).unwrap();
    assert_ne!(committed, before);
    let good = ModelEdit::SetObservation {
        node_id: node("cost"),
        period: quarter(1),
        value: 1.0.into(),
        availability_date: None,
    };
    for (bad, expected) in [
        (
            ModelEdit::AddNode {
                node: NodeSpec::new("cost", NodeType::Value),
            },
            "already defined",
        ),
        (
            ModelEdit::ReplaceNode {
                node: NodeSpec::new("absent", NodeType::Value),
            },
            "unknown node 'absent'",
        ),
        (
            ModelEdit::RemoveNode {
                node_id: node("absent"),
            },
            "unknown node 'absent'",
        ),
        (
            ModelEdit::RemoveNode {
                node_id: node("revenue"),
            },
            "Unknown identifier 'revenue'",
        ),
        (
            ModelEdit::RemoveObservation {
                node_id: node("cost"),
                period: quarter(2),
            },
            "no observation for period 2025Q2",
        ),
        (
            ModelEdit::SetObservation {
                node_id: node("margin"),
                period: quarter(1),
                value: 1.0.into(),
                availability_date: None,
            },
            "cannot have explicit values",
        ),
        (
            ModelEdit::SetFormula {
                node_id: node("profit"),
                formula: "revenue -".into(),
            },
            "Invalid formula",
        ),
        (
            ModelEdit::SetWhere {
                node_id: node("profit"),
                predicate: "revenue >".into(),
            },
            "Invalid where clause",
        ),
    ] {
        let error = model
            .apply_edits(&[good.clone(), bad])
            .expect_err("the batch is rejected");
        assert!(error.to_string().contains(expected), "{expected}: {error}");
        assert_eq!(
            serde_json::to_value(&model).unwrap(),
            committed,
            "a rejected batch leaves the model unchanged"
        );
    }

    // Edits round-trip through their wire form.
    let wire = serde_json::json!([
        {"edit": "remove_where", "node_id": "margin"},
        {"edit": "remove_forecast", "node_id": "profit"},
        {"edit": "remove_observation", "node_id": "cost", "period": "2025Q1"},
    ]);
    let edits: Vec<ModelEdit> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&edits).unwrap(), wire);
    model.apply_edits(&edits).unwrap();
    assert!(model.nodes["margin"].where_text.is_none());
    assert!(model.nodes["cost"].availability_dates.is_empty());
    serde_json::from_value::<ModelEdit>(serde_json::json!({"edit": "rename_node"}))
        .expect_err("an unknown edit is rejected");
}

/// STATEMENTS-CONTRACTS-002 #4 worked example: a $100 observation published
/// on April 25 is visible at that day's cutoff. A revision to $110 published
/// on May 15 is visible in the revised snapshot at the May 15 cutoff. An
/// April 25 replay uses the original snapshot and obtains $100.
#[test]
fn point_in_time_evaluation_replays_identified_snapshots() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_statements::evaluator::ObservationCutoff;

    let snapshot = |id: Option<&str>, amount: i64, published: Option<Date>| {
        let mut builder = ModelBuilder::new("revision-replay");
        builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        builder.value_money(
            "revenue",
            &[
                (quarter(1), Money::from((amount, Currency::USD))),
                // An undated value in a forecast period is an assumption.
                (quarter(2), Money::from((125_i64, Currency::USD))),
            ],
        );
        if let Some(published) = published {
            builder
                .availability_dates("revenue", &[(quarter(1), published)])
                .unwrap();
        }
        if let Some(id) = id {
            builder.source_snapshot(id);
        }
        builder.build().unwrap()
    };
    let original = snapshot(Some("original"), 100, Some(date!(2025 - 04 - 25)));
    let revised = snapshot(Some("revised"), 110, Some(date!(2025 - 05 - 15)));
    let read = |model: &FinancialModelSpec, cutoff: ObservationCutoff| {
        Evaluator::new()
            .evaluate_with_market(model, &MarketContext::new(), date!(2025 - 06 - 30), cutoff)
            .map(|result| {
                (
                    result.get("revenue", &quarter(1)).unwrap_or(f64::NAN),
                    result.get("revenue", &quarter(2)).unwrap_or(f64::NAN),
                )
            })
    };
    let through = ObservationCutoff::Through;

    // Inclusive through the end of the publication day.
    assert_eq!(
        read(&original, through(date!(2025 - 04 - 25))).unwrap(),
        (100.0, 125.0)
    );
    // Before publication the cell is unavailable, not an error and not zero.
    let (before, assumption) = read(&original, through(date!(2025 - 04 - 24))).unwrap();
    assert!(before.is_nan(), "an unpublished observation has no value");
    assert_eq!(assumption, 125.0);

    assert_eq!(
        read(&revised, through(date!(2025 - 05 - 15))).unwrap(),
        (110.0, 125.0)
    );
    assert!(
        read(&revised, through(date!(2025 - 05 - 14)))
            .unwrap()
            .0
            .is_nan(),
        "the revision is not yet published"
    );
    // The April 25 replay uses the original snapshot.
    assert_eq!(
        read(&original, through(date!(2025 - 04 - 25))).unwrap().0,
        100.0
    );
    assert_eq!(original.source_snapshot.as_deref(), Some("original"));
    assert_eq!(revised.source_snapshot.as_deref(), Some("revised"));

    // Full-snapshot mode reads the selected snapshot with no cutoff.
    assert_eq!(
        read(&revised, ObservationCutoff::FullSnapshot).unwrap(),
        (110.0, 125.0)
    );
    assert_eq!(
        Evaluator::new()
            .evaluate(&original)
            .unwrap()
            .get("revenue", &quarter(1)),
        Some(100.0)
    );

    // A cutoff needs an identified snapshot and a date on every observation
    // in an actual period.
    let unidentified = snapshot(None, 100, Some(date!(2025 - 04 - 25)));
    let error = read(&unidentified, through(date!(2025 - 04 - 25)))
        .expect_err("a cutoff needs an identified snapshot");
    assert!(
        error.to_string().contains("identified source snapshot"),
        "{error}"
    );
    read(&unidentified, ObservationCutoff::FullSnapshot).expect("full snapshot needs no identity");
    let undated = snapshot(Some("undated"), 100, None);
    let error = read(&undated, through(date!(2025 - 04 - 25)))
        .expect_err("a cutoff needs explicit publication dates");
    assert!(
        error.to_string().contains("no availability date"),
        "{error}"
    );

    let mut blank = original;
    blank.source_snapshot = Some("  ".to_string());
    blank
        .validate_semantics()
        .expect_err("a snapshot identity is not blank");
    let round_trip =
        FinancialModelSpec::from_json(&serde_json::to_string(&revised).unwrap()).unwrap();
    assert_eq!(round_trip.source_snapshot.as_deref(), Some("revised"));
}

/// A fixed-rate bond from the canonical Valuations constructor (US corporate
/// conventions: semi-annual 30/360), ready to attach with `add_debt`.
fn fixed_bond(
    id: &str,
    notional: finstack_quant_core::money::Money,
    coupon_rate: f64,
    issue: finstack_quant_core::dates::Date,
    maturity: finstack_quant_core::dates::Date,
    curve: &str,
) -> finstack_quant_statements::types::FinancialStatementInstrument {
    use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
    finstack_quant_statements::types::FinancialStatementInstrument::Bond(
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
            InstrumentId::new(id),
            notional,
            Rate::from_decimal(coupon_rate).expect("valid coupon"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            CurveId::new(curve),
        )
        .expect("valid bond"),
    )
}

// Typed capital selectors and reporting scopes (STATEMENTS-CAPITAL-009)

fn two_currency_builder(reporting: Option<Currency>) -> ModelBuilder {
    two_currency_builder_over("2025Q1..Q1", reporting)
}

fn two_currency_builder_over(periods: &str, reporting: Option<Currency>) -> ModelBuilder {
    let mut builder = ModelBuilder::new("capital-scopes");
    builder.periods(periods, None).unwrap();
    for (id, currency, curve) in [
        ("euro", Currency::EUR, "EUR-OIS"),
        ("dollar", Currency::USD, "USD-OIS"),
    ] {
        builder.add_debt(
            id,
            fixed_bond(
                id,
                Money::from((1_000_i64, currency)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2030 - 01 - 01),
                curve,
            ),
        );
    }
    if let Some(currency) = reporting {
        builder.reporting_currency(currency);
        builder.fx_policy(finstack_quant_statements::types::CapitalFxPolicy::PeriodEnd);
    }
    builder
}

/// With the period-close quote missing, a lagged consolidated reference is
/// unavailable like the same-period one, the run completes, and the cash
/// flows still list every period.
#[test]
fn a_missing_close_quote_leaves_lagged_totals_unavailable_and_periods_listed() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_statements::evaluator::{ObservationCutoff, UnavailableReason};
    use std::sync::Arc;

    let mut builder = two_currency_builder_over("2025Q1..Q2", Some(Currency::USD));
    builder
        .compute("prior_total", "lag(cs.debt_balance.total, 1)")
        .unwrap();
    builder
        .compute("prior_euro", "lag(cs.debt_balance.total.EUR, 1)")
        .unwrap();
    let model = builder.build().unwrap();
    // A matrix that cannot convert EUR to USD.
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::GBP, Currency::USD, 1.3)])
        .unwrap();
    let market = MarketContext::new().insert_fx(Arc::new(FxMatrix::new(Arc::new(provider), None)));
    let result = Evaluator::new()
        .evaluate_with_market(
            &model,
            &market,
            date!(2025 - 01 - 01),
            ObservationCutoff::FullSnapshot,
        )
        .expect("a missing quote does not fail the run");

    let cell = result.get_cell("prior_total", &quarter(2)).expect("cell");
    assert_eq!(cell.value, None);
    assert_eq!(cell.reason, Some(UnavailableReason::MissingFxQuote));
    assert_eq!(
        result.get_money("prior_euro", &quarter(2)),
        Some(Money::from((1_000_i64, Currency::EUR)))
    );
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");
    assert!(cashflows.totals.is_empty());
    assert_eq!(cashflows.periods(), vec![quarter(1), quarter(2)]);
}

/// Each selector parses, renders and compiles, and an unknown component,
/// instrument or scope is rejected when the model is built.
#[test]
fn capital_selectors_are_validated_when_the_model_is_built() {
    use finstack_quant_statements::dsl::{compile, parse_formula};
    for text in [
        "cs.debt_balance.euro",
        "cs.debt_balance.total",
        "cs.debt_balance.total.EUR",
        "cs.scheduled_principal.total.USD",
        "cs.net_interest_expense_cash.dollar",
    ] {
        let ast = parse_formula(text).expect("selector");
        assert_eq!(ast.to_string(), text);
        assert_eq!(parse_formula(&ast.to_string()).expect("round trip"), ast);
        compile(&ast).expect("compiles");
    }
    let error = parse_formula("cs.coupon.euro").expect_err("unknown component");
    assert!(
        error
            .to_string()
            .contains("Unknown capital structure component"),
        "{error}"
    );

    let build = |reporting: Option<Currency>, formula: &str| {
        let mut builder = two_currency_builder(reporting);
        builder.compute("probe", formula)?;
        builder.build()
    };
    let rejected = |reporting: Option<Currency>, formula: &str| {
        build(reporting, formula)
            .expect_err("rejected selector")
            .to_string()
    };
    let error = rejected(Some(Currency::USD), "cs.debt_balance.sterling");
    assert!(error.contains("'sterling'"), "{error}");
    let error = rejected(Some(Currency::USD), "cs.debt_balance.total.GBP");
    assert!(error.contains("GBP"), "{error}");
    // Two native currencies and no target: only native scopes exist.
    let error = rejected(None, "cs.debt_balance.total");
    assert!(error.contains("reporting currency"), "{error}");
    let model = build(None, "cs.debt_balance.total.EUR").expect("native total");
    assert_eq!(
        model.nodes["probe"].value_type,
        Some(NodeValueType::Monetary {
            currency: Currency::EUR
        })
    );
    // A model without a capital structure has nothing to read.
    let mut bare = ModelBuilder::new("no-capital");
    bare.periods("2025Q1..Q1", None).unwrap();
    bare.compute("probe", "cs.debt_balance.total").unwrap();
    let error = bare.build().expect_err("no capital structure").to_string();
    assert!(error.contains("has none"), "{error}");

    // Consolidating two currencies needs the policy chosen explicitly.
    let mut builder = two_currency_builder(None);
    builder.reporting_currency(Currency::USD);
    let error = builder.build().expect_err("no FX policy").to_string();
    assert!(error.contains("fx_policy"), "{error}");
    assert!(
        "cashflow_date"
            .parse::<finstack_quant_statements::types::CapitalFxPolicy>()
            .is_err()
    );
}

/// Each scope evaluates to exact money in its own currency. With a missing
/// period-close quote the consolidated cell is unavailable and the native
/// cells keep their values.
#[test]
fn capital_scopes_evaluate_natively_and_consolidate_at_the_close() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_statements::evaluator::{ObservationCutoff, UnavailableReason};
    use std::sync::Arc;

    let mut builder = two_currency_builder(Some(Currency::USD));
    for (node, formula) in [
        ("euro_balance", "cs.debt_balance.euro"),
        ("euro_total", "cs.debt_balance.total.EUR"),
        ("dollar_total", "cs.debt_balance.total.USD"),
        ("consolidated", "cs.debt_balance.total"),
    ] {
        builder.compute(node, formula).unwrap();
    }
    let model = builder.build().unwrap();
    let evaluate = |quotes: &[(Currency, Currency, f64)]| {
        let provider = SimpleFxProvider::new();
        provider.set_quotes(quotes).unwrap();
        let market =
            MarketContext::new().insert_fx(Arc::new(FxMatrix::new(Arc::new(provider), None)));
        Evaluator::new()
            .evaluate_with_market(
                &model,
                &market,
                date!(2025 - 01 - 01),
                ObservationCutoff::FullSnapshot,
            )
            .expect("evaluation")
    };

    let result = evaluate(&[(Currency::EUR, Currency::USD, 1.2)]);
    let money = |node: &str| result.get_money(node, &quarter(1)).expect("money cell");
    assert_eq!(
        money("euro_balance"),
        Money::from((1_000_i64, Currency::EUR))
    );
    assert_eq!(money("euro_total"), Money::from((1_000_i64, Currency::EUR)));
    assert_eq!(
        money("dollar_total"),
        Money::from((1_000_i64, Currency::USD))
    );
    assert_eq!(
        money("consolidated"),
        Money::from((2_200_i64, Currency::USD))
    );
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");
    assert_eq!(cashflows.fx_conversions[&quarter(1)][0].rate, 1.2);
    assert_eq!(
        cashflows.fx_conversions[&quarter(1)][0].date,
        date!(2025 - 03 - 31)
    );

    let result = evaluate(&[(Currency::GBP, Currency::USD, 1.3)]);
    let cell = result.get_cell("consolidated", &quarter(1)).expect("cell");
    assert_eq!(cell.value, None);
    assert_eq!(cell.reason, Some(UnavailableReason::MissingFxQuote));
    assert_eq!(
        result.get_money("euro_total", &quarter(1)),
        Some(Money::from((1_000_i64, Currency::EUR)))
    );
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");
    assert!(
        cashflows
            .consolidation_unavailable
            .contains_key(&quarter(1))
    );
}

/// The capital long table carries each amount exactly as decimal text beside
/// its float view.
#[test]
fn the_capital_table_reports_exact_amounts() {
    use finstack_quant_core::table::TableColumnData;
    let model = two_currency_builder(None).build().unwrap();
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    let table = result
        .cs_cashflows
        .as_ref()
        .expect("capital cash flows")
        .to_table()
        .expect("table");
    let column = |name: &str| {
        table
            .columns
            .iter()
            .find(|column| column.name == name)
            .unwrap_or_else(|| panic!("column {name}"))
    };
    let (TableColumnData::Float64(values), TableColumnData::String(amounts)) =
        (&column("value").data, &column("amount").data)
    else {
        panic!("value is a float column and amount a text column");
    };
    assert_eq!(values.len(), amounts.len());
    for (value, amount) in values.iter().zip(amounts) {
        let exact: rust_decimal::Decimal = amount.parse().expect("decimal text");
        assert_eq!(exact.to_string(), *amount);
        assert!((value - f64::try_from(exact).expect("float view")).abs() < 1e-9);
    }
    assert!(amounts.iter().any(|amount| amount == "1000"));
}
