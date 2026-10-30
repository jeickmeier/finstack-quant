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
            .evaluate_with_market(&model, &market, as_of)
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
    assert!(evaluator
        .evaluate_prepared(&model, &plan)
        .unwrap_err()
        .to_string()
        .contains("prepared value type"));
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
    changed_period.periods[0].end = changed_period.periods[0].start;
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
    assert!(evaluator
        .evaluate_prepared(&calculated_override, &plan)
        .is_err());
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

#[test]
fn finite_money_overflow_retains_conversion_warning_in_both_paths() {
    let model = {
        let mut model_builder = ModelBuilder::new("money conversion diagnostics");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value_money("usd", &[(quarter(), Money::from((1_i64, Currency::USD)))]);
        model_builder.compute("large", "usd * 1e30").unwrap();
        model_builder.build().unwrap()
    };
    let mut evaluator = Evaluator::new();
    let plan = evaluator.prepare(&model).unwrap();
    for result in [
        evaluator.evaluate(&model).unwrap(),
        evaluator.evaluate_prepared(&model, &plan).unwrap(),
    ] {
        assert_eq!(result.get("large", &quarter()), Some(1e30));
        assert!(result.get_money("large", &quarter()).is_none());
        assert!(
            result.meta.warnings.iter().any(|warning| matches!(warning,
                EvalWarning::NonFiniteValue { node_id, period, value }
                    if node_id == "large" && *period == quarter() && *value == 1e30
            )),
            "monetary conversion diagnostics must survive finalization"
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
