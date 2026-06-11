//! Scenario set integration tests.
#![allow(clippy::expect_used)]

use finstack_core::dates::PeriodId;
use finstack_statements::builder::ModelBuilder;
use finstack_statements::types::{AmountOrScalar, FinancialModelSpec};
use finstack_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
use indexmap::IndexMap;

fn build_simple_model() -> FinancialModelSpec {
    let period_q1 = PeriodId::quarter(2025, 1);
    let period_q2 = PeriodId::quarter(2025, 2);

    ModelBuilder::new("scenario_demo")
        .periods("2025Q1..Q2", None)
        .expect("valid period range")
        .value(
            "revenue",
            &[
                (period_q1, AmountOrScalar::scalar(100_000.0)),
                (period_q2, AmountOrScalar::scalar(100_000.0)),
            ],
        )
        .compute("cogs", "revenue * 0.4")
        .expect("valid formula")
        .compute("ebitda", "revenue - cogs")
        .expect("valid formula")
        .build()
        .expect("valid model")
}

#[test]
fn evaluate_all_applies_overrides_and_evaluates() {
    let model = build_simple_model();
    let period = PeriodId::quarter(2025, 1);

    let mut scenarios = IndexMap::new();

    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: None,
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), 90_000.0);
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: Some("base".to_string()),
            overrides: downside_overrides,
        },
    );

    let set = ScenarioSet { scenarios };
    let results = set
        .evaluate_all(&model)
        .expect("scenario evaluation should succeed");

    assert_eq!(results.len(), 2);
    let base_results = results
        .scenarios
        .get("base")
        .expect("base scenario should be present");
    let downside_results = results
        .scenarios
        .get("downside")
        .expect("downside scenario should be present");

    let base_revenue = base_results
        .get("revenue", &period)
        .expect("base revenue should exist");
    let downside_revenue = downside_results
        .get("revenue", &period)
        .expect("downside revenue should exist");

    assert_eq!(base_revenue, 100_000.0);
    assert_eq!(downside_revenue, 90_000.0);

    let base_ebitda = base_results
        .get("ebitda", &period)
        .expect("base ebitda should exist");
    let downside_ebitda = downside_results
        .get("ebitda", &period)
        .expect("downside ebitda should exist");

    assert_eq!(base_ebitda, 60_000.0);
    assert_eq!(downside_ebitda, 54_000.0);
}

#[test]
fn diff_uses_variance_analyzer() {
    let model = build_simple_model();
    let period = PeriodId::quarter(2025, 1);

    let mut scenarios = IndexMap::new();

    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: None,
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), 90_000.0);
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: Some("base".to_string()),
            overrides: downside_overrides,
        },
    );

    let set = ScenarioSet { scenarios };
    let results = set
        .evaluate_all(&model)
        .expect("scenario evaluation should succeed");

    let metrics = vec!["revenue".to_string(), "ebitda".to_string()];
    let periods = vec![period];

    let diff = set
        .diff(&results, "base", "downside", &metrics, &periods)
        .expect("diff should succeed");

    assert_eq!(diff.baseline, "base");
    assert_eq!(diff.comparison, "downside");
    assert_eq!(diff.variance.rows.len(), 2);

    let mut revenue_row = None;
    let mut ebitda_row = None;
    for row in &diff.variance.rows {
        match row.metric.as_str() {
            "revenue" => revenue_row = Some(row),
            "ebitda" => ebitda_row = Some(row),
            _ => {}
        }
    }

    let revenue_row = revenue_row.expect("revenue row should be present");
    assert_eq!(revenue_row.baseline, 100_000.0);
    assert_eq!(revenue_row.comparison, 90_000.0);
    assert_eq!(revenue_row.abs_var, -10_000.0);

    let ebitda_row = ebitda_row.expect("ebitda row should be present");
    assert_eq!(ebitda_row.baseline, 60_000.0);
    assert_eq!(ebitda_row.comparison, 54_000.0);
    assert_eq!(ebitda_row.abs_var, -6_000.0);
}

#[test]
fn evaluate_all_preserves_actual_history_when_applying_overrides() {
    let model = ModelBuilder::new("scenario_actuals")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid period range")
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .build()
        .expect("valid model");

    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: None,
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), 90_000.0);
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: Some("base".to_string()),
            overrides: downside_overrides,
        },
    );

    let set = ScenarioSet { scenarios };
    let results = set
        .evaluate_all(&model)
        .expect("scenario evaluation should succeed");

    let downside = results
        .scenarios
        .get("downside")
        .expect("downside scenario should be present");

    assert_eq!(
        downside.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
    assert_eq!(
        downside.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(110_000.0)
    );
    assert_eq!(
        downside.get("revenue", &PeriodId::quarter(2025, 3)),
        Some(90_000.0)
    );
    assert_eq!(
        downside.get("revenue", &PeriodId::quarter(2025, 4)),
        Some(90_000.0)
    );
}

#[test]
fn comparison_table_emits_null_pct_on_zero_baseline() {
    use finstack_core::table::TableColumnData;

    let period_q1 = PeriodId::quarter(2025, 1);
    let period_q2 = PeriodId::quarter(2025, 2);

    // Baseline metric is exactly zero in both periods.
    let model = ModelBuilder::new("zero_base")
        .periods("2025Q1..Q2", None)
        .expect("valid period range")
        .value(
            "fcf",
            &[
                (period_q1, AmountOrScalar::scalar(0.0)),
                (period_q2, AmountOrScalar::scalar(0.0)),
            ],
        )
        .build()
        .expect("valid model");

    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: None,
            overrides: IndexMap::new(),
        },
    );
    let mut upside_overrides = IndexMap::new();
    upside_overrides.insert("fcf".to_string(), 10_000.0);
    scenarios.insert(
        "upside".to_string(),
        ScenarioDefinition {
            model_id: Some(model.id.clone()),
            parent: Some("base".to_string()),
            overrides: upside_overrides,
        },
    );

    let set = ScenarioSet { scenarios };
    let results = set.evaluate_all(&model).expect("scenario evaluation");
    let table = results
        .to_comparison_table(&["fcf"])
        .expect("comparison table");

    let pct_col = table
        .columns
        .iter()
        .find(|c| c.name.contains("_pct"))
        .expect("pct column present");
    match &pct_col.data {
        TableColumnData::NullableFloat64(values) => {
            assert!(
                values.iter().all(|v| v.is_none()),
                "pct change on a zero baseline must be null, got {values:?}"
            );
        }
        other => panic!("pct column should be nullable float, got {other:?}"),
    }
}
