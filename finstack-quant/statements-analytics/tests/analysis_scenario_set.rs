//! Scenario set integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec};
use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
use indexmap::IndexMap;

fn build_simple_model() -> FinancialModelSpec {
    let period_q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let period_q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

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
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    let mut scenarios = IndexMap::new();

    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            parent: None,
            period_overrides: IndexMap::new(),
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), AmountOrScalar::scalar(90_000.0));
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            parent: Some("base".to_string()),
            period_overrides: IndexMap::new(),
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
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    let mut scenarios = IndexMap::new();

    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            parent: None,
            period_overrides: IndexMap::new(),
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), AmountOrScalar::scalar(90_000.0));
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            parent: Some("base".to_string()),
            period_overrides: IndexMap::new(),
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
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
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
            parent: None,
            period_overrides: IndexMap::new(),
            overrides: IndexMap::new(),
        },
    );

    let mut downside_overrides = IndexMap::new();
    downside_overrides.insert("revenue".to_string(), AmountOrScalar::scalar(90_000.0));
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            parent: Some("base".to_string()),
            period_overrides: IndexMap::new(),
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
        downside.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100_000.0)
    );
    assert_eq!(
        downside.get(
            "revenue",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(110_000.0)
    );
    assert_eq!(
        downside.get(
            "revenue",
            &PeriodId::quarter(2025, 3).expect("valid period fixture")
        ),
        Some(90_000.0)
    );
    assert_eq!(
        downside.get(
            "revenue",
            &PeriodId::quarter(2025, 4).expect("valid period fixture")
        ),
        Some(90_000.0)
    );
}

#[test]
fn comparison_table_emits_null_frac_on_zero_baseline() {
    use finstack_quant_core::table::TableColumnData;

    let period_q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let period_q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

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
            parent: None,
            period_overrides: IndexMap::new(),
            overrides: IndexMap::new(),
        },
    );
    let mut upside_overrides = IndexMap::new();
    upside_overrides.insert("fcf".to_string(), AmountOrScalar::scalar(10_000.0));
    scenarios.insert(
        "upside".to_string(),
        ScenarioDefinition {
            parent: Some("base".to_string()),
            period_overrides: IndexMap::new(),
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
        .find(|c| c.name.contains("_frac"))
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

#[test]
fn monetary_scenario_overrides_preserve_and_validate_currency() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = ModelBuilder::new("money-scenario")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .value_money(
            "revenue",
            &[(
                period,
                Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
            )],
        )
        .build()
        .expect("model");

    let valid = ScenarioSet {
        scenarios: IndexMap::from([(
            "upside".to_string(),
            ScenarioDefinition {
                parent: None,
                period_overrides: IndexMap::new(),
                overrides: IndexMap::from([(
                    "revenue".to_string(),
                    AmountOrScalar::amount(110_000.0, Currency::USD).expect("valid amount fixture"),
                )]),
            },
        )]),
    };
    let results = valid.evaluate_all(&model).expect("same-currency override");
    assert_eq!(
        results.scenarios["upside"]
            .get_money("revenue", &period)
            .expect("money")
            .amount(),
        110_000.0
    );

    let invalid = ScenarioSet {
        scenarios: IndexMap::from([(
            "invalid".to_string(),
            ScenarioDefinition {
                parent: None,
                period_overrides: IndexMap::new(),
                overrides: IndexMap::from([(
                    "revenue".to_string(),
                    AmountOrScalar::amount(110_000.0, Currency::EUR).expect("valid amount fixture"),
                )]),
            },
        )]),
    };
    let error = invalid
        .evaluate_all(&model)
        .expect_err("cross-currency override must fail");
    assert!(error.to_string().contains("incompatible"));
}

#[test]
fn per_period_overrides_apply_to_one_forecast_period_and_win_over_model_wide() {
    let model = build_simple_model();
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            parent: None,
            overrides: IndexMap::new(),
            period_overrides: IndexMap::new(),
        },
    );
    let mut q2_only = IndexMap::new();
    q2_only.insert(q2, AmountOrScalar::scalar(80_000.0));
    let mut period_overrides = IndexMap::new();
    period_overrides.insert("revenue".to_string(), q2_only);
    let mut overrides = IndexMap::new();
    overrides.insert("revenue".to_string(), AmountOrScalar::scalar(90_000.0));
    scenarios.insert(
        "downside".to_string(),
        ScenarioDefinition {
            parent: Some("base".to_string()),
            overrides,
            period_overrides,
        },
    );

    let set = ScenarioSet { scenarios };
    let results = set.evaluate_all(&model).expect("evaluation succeeds");
    let downside = results.scenarios.get("downside").expect("downside present");
    assert_eq!(downside.get("revenue", &q1), Some(90_000.0));
    assert_eq!(downside.get("revenue", &q2), Some(80_000.0));

    // The legacy wire form (no `period_overrides`) still deserializes.
    let json = serde_json::to_string(&set).expect("serialize");
    let restored: ScenarioSet = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        restored.scenarios["downside"].period_overrides["revenue"].len(),
        1
    );
    let legacy: ScenarioSet =
        serde_json::from_str(r#"{"scenarios":{"base":{"overrides":{"revenue":90.0}}}}"#)
            .expect("legacy wire form");
    assert!(legacy.scenarios["base"].period_overrides.is_empty());

    // A per-period override on a non-forecast period is rejected.
    let mut bad_period = IndexMap::new();
    bad_period.insert(
        PeriodId::quarter(2030, 1).expect("valid period fixture"),
        AmountOrScalar::scalar(1.0),
    );
    let mut bad = IndexMap::new();
    bad.insert("revenue".to_string(), bad_period);
    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "bad".to_string(),
        ScenarioDefinition {
            parent: None,
            overrides: IndexMap::new(),
            period_overrides: bad,
        },
    );
    let err = ScenarioSet { scenarios }
        .evaluate_all(&model)
        .expect_err("non-forecast period is rejected");
    assert!(err.to_string().contains("not a forecast period"));
}
