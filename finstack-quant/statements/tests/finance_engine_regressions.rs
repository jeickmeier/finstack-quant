//! Regression coverage for validated execution plans and result diagnostics.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::evaluator::{EvalWarning, MonteCarloConfig};
use finstack_quant_statements::types::{FinancialModelSpec, NodeId};

fn quarter() -> PeriodId {
    PeriodId::quarter(2025, 1).expect("valid fixture period")
}

fn currency_model() -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("engine currency checks");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value_money("usd", &[(quarter(), Money::from((100_i64, Currency::USD)))]);
        model_builder.value_money("eur", &[(quarter(), Money::from((100_i64, Currency::EUR)))]);
        model_builder.compute("total", "usd * 2").unwrap();
        model_builder.build().unwrap()
    }
}

#[test]
fn all_plan_building_entry_points_revalidate_mutated_formulas() {
    let mut model = currency_model();
    model.get_node_mut("total").unwrap().formula_text = Some("usd + eur".into());
    let mut evaluator = Evaluator::new();
    let market = MarketContext::new();
    let as_of = model.periods[0].end;
    for error in [
        evaluator.evaluate(&model).unwrap_err(),
        evaluator
            .evaluate_with_market(
                &model,
                &market,
                as_of,
                finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            )
            .unwrap_err(),
        evaluator
            .evaluate_monte_carlo(&model, &MonteCarloConfig::new(2, 42))
            .unwrap_err(),
        evaluator
            .prepare(&model)
            .err()
            .expect("prepare must reject invalid dimensions"),
    ] {
        assert!(
            error.to_string().contains("cannot combine USD and EUR"),
            "{error}"
        );
    }
}

#[test]
fn direct_models_keep_inferred_currency_in_ordinary_and_prepared_results() {
    let mut model = currency_model();
    // Direct construction and serde may omit both input and formula units.
    for node in model.nodes.values_mut() {
        node.value_type = None;
    }
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    for result in [
        evaluator.evaluate(&model).unwrap(),
        evaluator.evaluate_prepared(&model, &plan).unwrap(),
    ] {
        assert_eq!(
            result.get_money("total", &quarter()),
            Some(Money::from((200_i64, Currency::USD)))
        );
        assert!(result.get_scalar("total", &quarter()).is_none());
    }
    assert!(
        model.nodes.values().all(|node| node.value_type.is_none()),
        "evaluation must not mutate its input"
    );

    // A missing declaration must not let input currencies drift between runs.
    model
        .get_node_mut("usd")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .insert(quarter(), Money::from((100_i64, Currency::EUR)).into());
    assert!(
        evaluator
            .evaluate_prepared(&model, &plan)
            .unwrap_err()
            .to_string()
            .contains("prepared value type")
    );
}

#[test]
fn prepared_plan_rejects_embedded_identity_and_timeline_mutations() {
    let model = currency_model();
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    let mut changed_id = model.clone();
    changed_id.get_node_mut("eur").unwrap().node_id = NodeId::new("usd");
    assert!(evaluator.evaluate_prepared(&changed_id, &plan).is_err());

    let mut changed_period = model.clone();
    changed_period.periods.periods[0].end = changed_period.periods.periods[0].start;
    assert!(evaluator.evaluate_prepared(&changed_period, &plan).is_err());
}

#[test]
fn prepared_plan_rejects_calculated_overrides_and_orphaned_availability() {
    let mut model = currency_model();
    let release_date = model.periods[0].end;
    model
        .get_node_mut("usd")
        .unwrap()
        .availability_dates
        .insert(quarter(), release_date);
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();

    let mut calculated_override = model.clone();
    calculated_override.get_node_mut("total").unwrap().values = Some(indexmap::indexmap! {
        quarter() => Money::from((999_i64, Currency::USD)).into(),
    });
    assert!(
        evaluator
            .evaluate_prepared(&calculated_override, &plan)
            .is_err()
    );
    assert!(evaluator.evaluate(&calculated_override).is_err());

    model
        .get_node_mut("usd")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .clear();
    assert!(evaluator.evaluate_prepared(&model, &plan).is_err());
    assert!(evaluator.evaluate(&model).is_err());
}

#[test]
fn monetary_nonfinite_results_emit_one_diagnostic_per_cell() {
    let model = {
        let mut model_builder = ModelBuilder::new("nonfinite monetary diagnostics");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value_money("usd", &[(quarter(), Money::from((1_i64, Currency::USD)))]);
        model_builder.compute("undefined", "usd / 0").unwrap();
        model_builder.build().unwrap()
    };
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    for result in [
        evaluator.evaluate(&model).unwrap(),
        evaluator.evaluate_prepared(&model, &plan).unwrap(),
    ] {
        assert!(!result.get("undefined", &quarter()).unwrap().is_finite());
        assert!(result.get_money("undefined", &quarter()).is_none());
        assert_eq!(
            result
                .meta
                .warnings
                .iter()
                .filter(|warning| matches!(warning,
                    EvalWarning::NonFiniteValue { node_id, period, .. }
                        if node_id == "undefined" && *period == quarter()
                ))
                .count(),
            1
        );
    }
}

/// STATEMENTS-EVALUATION-013 #1: unrepresentable money is an error naming
/// the node, period and cause, on both evaluation paths.
#[test]
fn unrepresentable_money_is_an_error_in_both_paths() {
    let model = {
        let mut model_builder = ModelBuilder::new("money conversion diagnostics");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value_money("usd", &[(quarter(), Money::from((1_i64, Currency::USD)))]);
        model_builder.compute("large", "usd * 1e30").unwrap();
        model_builder.build().unwrap()
    };
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    for error in [
        evaluator.evaluate(&model).unwrap_err(),
        evaluator.evaluate_prepared(&model, &plan).unwrap_err(),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("Node 'large' in period 2025Q1")
                && message.contains("cannot be represented as USD money"),
            "{message}"
        );
    }
}

/// Every Monte Carlo path re-evaluates the actual periods, so a warning there
/// used to be reported once per path (1,400 copies for 200 paths of the UI's
/// analyst model). Only simulated forecast periods are reported, each
/// distinct warning once.
#[test]
fn monte_carlo_reports_each_forecast_warning_once() {
    use finstack_quant_statements::types::{AmountOrScalar, ForecastMethod, ForecastSpec};
    let q = |n| PeriodId::quarter(2025, n).unwrap();
    let model = {
        let mut model_builder = ModelBuilder::new("mc warnings");
        model_builder.periods("2025Q1..Q4", Some("2025Q2")).unwrap();
        model_builder.value(
            "revenue",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(110.0)),
            ],
        );
        let mut params = indexmap::IndexMap::new();
        params.insert("mean".into(), serde_json::json!(120.0));
        params.insert("std_dev".into(), serde_json::json!(5.0));
        params.insert("seed".into(), serde_json::json!(42));
        model_builder.forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Normal,
                params,
            },
        );
        model_builder.value(
            "denominator",
            &(1..=4)
                .map(|n| (q(n), AmountOrScalar::scalar(if n == 3 { 0.0 } else { 1.0 })))
                .collect::<Vec<_>>(),
        );
        // NaN in the first (actual) quarter on every path.
        model_builder
            .compute("growth", "pct_change(revenue, 1)")
            .unwrap();
        // A finite 2025Q3 forecast cell that warns on every path: the division
        // by zero is skipped by `sum`.
        model_builder
            .compute("total", "sum(revenue, revenue / denominator)")
            .unwrap();
        model_builder.build().unwrap()
    };
    let results = Evaluator::new()
        .evaluate_monte_carlo(&model, &MonteCarloConfig::new(50, 7))
        .unwrap();

    assert!(!results.warnings.is_empty(), "the 2025Q3 warning is kept");
    let mut keys: Vec<String> = results
        .warnings
        .iter()
        .map(|warning| {
            let period = match warning {
                EvalWarning::DivisionByZero { period, .. }
                | EvalWarning::NonFiniteValue { period, .. }
                | EvalWarning::CapitalStructure { period, .. }
                | EvalWarning::NonFiniteSkipped { period, .. } => *period,
            };
            assert!(results.forecast_periods.contains(&period), "{warning:?}");
            format!("{warning:?}")
        })
        .collect();
    let reported = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), reported, "{:?}", results.warnings);
}

/// STATEMENTS-EVALUATION-001 #1: only a positive integer literal offset makes
/// a `lag`/`shift` value a past dependency. Zero and dynamic offsets keep the
/// same-period edge, and the offset's own dependencies stay in the graph.
#[test]
fn lag_edges_follow_the_positive_integer_literal_rule() {
    let build = |a: &str, b: &str| {
        let mut builder = ModelBuilder::new("lag-edges");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.value_scalar("seed", &[(quarter(), 1.0)]);
        builder.value_scalar("n", &[(quarter(), 1.0)]);
        builder.compute("a", a).unwrap();
        builder.compute("b", b).unwrap();
        builder.build()
    };

    // A literal past offset breaks the cycle: a corkscrew.
    for past in ["lag(b, 1) + seed", "shift(b, 2) + seed"] {
        build(past, "a + seed").unwrap_or_else(|error| panic!("{past}: {error}"));
    }
    // Zero, a non-integer literal and a dynamic offset can read the current
    // period, so the same-period cycle is rejected.
    for same_period in ["lag(b, 0) + seed", "lag(b, 1.5) + seed", "lag(b, n) + seed"] {
        let error = build(same_period, "a + seed").expect_err(same_period);
        assert!(
            error.to_string().contains("Circular dependency"),
            "{same_period}: {error}"
        );
    }
    // The offset expression is a control dependency even for a past value.
    let error = build("lag(seed, b) + seed", "a + seed").expect_err("offset dependency");
    assert!(error.to_string().contains("Circular dependency"), "{error}");
}

/// STATEMENTS-EVALUATION-001 #4: a prepared plan is reused only for payload
/// changes in existing observation slots.
#[test]
fn prepared_plans_are_reused_only_for_payload_edits_in_existing_slots() {
    use finstack_quant_statements::types::{CapitalStructureSpec, ModelEdit, NodeId};
    use time::macros::date;

    let q2 = PeriodId::quarter(2025, 2).unwrap();
    let model = {
        let mut builder = ModelBuilder::new("prepared");
        builder.periods("2025Q1..Q2", Some("2025Q1")).unwrap();
        builder.value_scalar("revenue", &[(quarter(), 100.0)]);
        builder
            .availability_dates("revenue", &[(quarter(), date!(2025 - 04 - 25))])
            .unwrap();
        builder.compute("double", "revenue * 2").unwrap();
        builder.build().unwrap()
    };
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    let observation = |period, value: f64, availability_date| ModelEdit::SetObservation {
        node_id: NodeId::new("revenue"),
        period,
        value: value.into(),
        availability_date,
    };

    // A new payload in the existing slot reuses the plan.
    let mut payload = model.clone();
    payload
        .apply_edits(&[observation(quarter(), 150.0, Some(date!(2025 - 04 - 25)))])
        .unwrap();
    let result = evaluator.evaluate_prepared(&payload, &plan).unwrap();
    assert_eq!(result.get("double", &quarter()), Some(300.0));

    let rejected = |evaluator: &mut Evaluator, changed: &FinancialModelSpec, what: &str| {
        let error = evaluator.evaluate_prepared(changed, &plan).expect_err(what);
        assert!(
            error.to_string().contains("Rebuild the plan"),
            "{what}: {error}"
        );
    };

    let mut new_slot = model.clone();
    new_slot.apply_edits(&[observation(q2, 1.0, None)]).unwrap();
    rejected(&mut evaluator, &new_slot, "a new observation slot");

    let mut removed_slot = model.clone();
    removed_slot
        .apply_edits(&[ModelEdit::RemoveObservation {
            node_id: NodeId::new("revenue"),
            period: quarter(),
        }])
        .unwrap();
    rejected(&mut evaluator, &removed_slot, "a removed observation slot");

    let mut republished = model.clone();
    republished
        .apply_edits(&[observation(quarter(), 100.0, Some(date!(2025 - 05 - 15)))])
        .unwrap();
    rejected(&mut evaluator, &republished, "a changed availability date");

    let mut formula = model.clone();
    formula
        .apply_edits(&[ModelEdit::SetFormula {
            node_id: NodeId::new("double"),
            formula: "revenue * 3".into(),
        }])
        .unwrap();
    rejected(&mut evaluator, &formula, "a changed formula");

    let mut masked = model.clone();
    masked
        .apply_edits(&[ModelEdit::SetWhere {
            node_id: NodeId::new("double"),
            predicate: "revenue > 0".into(),
        }])
        .unwrap();
    rejected(&mut evaluator, &masked, "a changed mask");

    let mut forecast = model.clone();
    forecast
        .apply_edits(&[ModelEdit::SetForecast {
            node_id: NodeId::new("revenue"),
            forecast: finstack_quant_statements::types::ForecastSpec::growth(0.1),
        }])
        .unwrap();
    rejected(&mut evaluator, &forecast, "a changed forecast");

    let mut capital = model.clone();
    capital.capital_structure = Some(CapitalStructureSpec {
        debt_instruments: Vec::new(),
        meta: Default::default(),
        reporting_currency: Some(Currency::USD),
        fx_policy: None,
        waterfall: None,
    });
    rejected(&mut evaluator, &capital, "a changed capital structure");

    let mut timeline = model.clone();
    timeline.periods.periods[1].is_actual = true;
    rejected(&mut evaluator, &timeline, "a changed timeline");
}
