//! Scenario set integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, ForecastSpec};
use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
use indexmap::IndexMap;

fn build_simple_model() -> FinancialModelSpec {
    let period_q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let period_q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

    {
        let mut model_builder = ModelBuilder::new("scenario_demo");
        model_builder
            .periods("2025Q1..Q2", None)
            .expect("valid period range");
        model_builder.value(
            "revenue",
            &[
                (period_q1, AmountOrScalar::scalar(100_000.0)),
                (period_q2, AmountOrScalar::scalar(100_000.0)),
            ],
        );
        model_builder
            .compute("cogs", "revenue * 0.4")
            .expect("valid formula");
        model_builder
            .compute("ebitda", "revenue - cogs")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    }
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
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
    let model = {
        let mut model_builder = ModelBuilder::new("scenario_actuals");
        model_builder
            .periods("2025Q1..Q4", Some("2025Q2"))
            .expect("valid period range");
        model_builder.value(
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
        );
        model_builder.build().expect("valid model")
    };

    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            parent: None,
            period_overrides: IndexMap::new(),
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
    let model = {
        let mut model_builder = ModelBuilder::new("zero_base");
        model_builder
            .periods("2025Q1..Q2", None)
            .expect("valid period range");
        model_builder.value(
            "fcf",
            &[
                (period_q1, AmountOrScalar::scalar(0.0)),
                (period_q2, AmountOrScalar::scalar(0.0)),
            ],
        );
        model_builder.build().expect("valid model")
    };

    let mut scenarios = IndexMap::new();
    scenarios.insert(
        "base".to_string(),
        ScenarioDefinition {
            parent: None,
            period_overrides: IndexMap::new(),
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
    let model = {
        let mut model_builder = ModelBuilder::new("money-scenario");
        model_builder.periods("2025Q1..Q1", None).expect("periods");
        model_builder.value_money(
            "revenue",
            &[(
                period,
                Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
            )],
        );
        model_builder.build().expect("model")
    };

    let valid = ScenarioSet {
        scenarios: IndexMap::from([(
            "upside".to_string(),
            ScenarioDefinition {
                parent: None,
                period_overrides: IndexMap::new(),
                forecast_overrides: IndexMap::new(),
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
                forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
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
            forecast_overrides: IndexMap::new(),
        },
    );
    let err = ScenarioSet { scenarios }
        .evaluate_all(&model)
        .expect_err("non-forecast period is rejected");
    assert!(err.to_string().contains("not a forecast period"));
}

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

/// Revenue is actual in Q1 and grows 5% a quarter after; COGS is a formula.
fn forecast_model() -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("forecast_scenarios");
        model_builder
            .periods("2025Q1..Q4", Some("2025Q1"))
            .expect("valid period range");
        model_builder.value("revenue", &[(q(1), AmountOrScalar::scalar(100.0))]);
        model_builder.forecast("revenue", ForecastSpec::growth(0.05));
        model_builder
            .compute("cogs", "revenue * 0.4")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    }
}

fn scenario(parent: Option<&str>, forecasts: &[(&str, ForecastSpec)]) -> ScenarioDefinition {
    ScenarioDefinition {
        parent: parent.map(str::to_string),
        forecast_overrides: forecasts
            .iter()
            .map(|(node, spec)| ((*node).to_string(), spec.clone()))
            .collect(),
        ..ScenarioDefinition::default()
    }
}

fn revenue_path(results: &finstack_quant_statements::evaluator::StatementResult) -> Vec<f64> {
    (1..=4)
        .map(|quarter| results.get("revenue", &q(quarter)).expect("revenue"))
        .collect()
}

#[test]
fn forecast_overrides_replace_methods_and_parameters_along_the_parent_chain() {
    let model = forecast_model();
    let set = ScenarioSet {
        scenarios: IndexMap::from([
            ("base".to_string(), scenario(None, &[])),
            (
                "curve".to_string(),
                scenario(
                    Some("base"),
                    &[
                        ("revenue", ForecastSpec::curve(vec![0.10, 0.0, -0.10])),
                        // A formula node gains a forecast and becomes Mixed.
                        ("cogs", ForecastSpec::forward_fill()),
                    ],
                ),
            ),
            (
                "stochastic".to_string(),
                scenario(
                    Some("curve"),
                    &[("revenue", ForecastSpec::normal(0.0, 5.0, 7))],
                ),
            ),
            (
                "reseeded".to_string(),
                scenario(
                    Some("curve"),
                    &[("revenue", ForecastSpec::normal(0.0, 5.0, 8))],
                ),
            ),
        ]),
    };

    let results = set.evaluate_all(&model).expect("evaluation succeeds");

    let base = revenue_path(&results.scenarios["base"]);
    assert!((base[3] - 100.0 * 1.05_f64.powi(3)).abs() < 1e-9);

    let curve = &results.scenarios["curve"];
    let expected = [100.0, 110.0, 110.0, 99.0];
    for (actual, expected) in revenue_path(curve).iter().zip(expected) {
        assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
    }
    // COGS carries its Q1 formula value forward instead of tracking revenue.
    for quarter in 1..=4 {
        assert_eq!(curve.get("cogs", &q(quarter)), Some(40.0));
    }

    // The child's stochastic spec replaces the parent's curve for revenue
    // while the inherited COGS override still applies; the seed is a
    // stochastic parameter the scenario controls.
    let stochastic = &results.scenarios["stochastic"];
    let stochastic_path = revenue_path(stochastic);
    assert_eq!(stochastic_path[0], 100.0);
    assert_ne!(stochastic_path, revenue_path(curve));
    assert_ne!(
        stochastic_path,
        revenue_path(&results.scenarios["reseeded"])
    );
    assert_eq!(stochastic.get("cogs", &q(4)), Some(40.0));
    let again = set.evaluate_all(&model).expect("evaluation succeeds");
    assert_eq!(
        revenue_path(&again.scenarios["stochastic"]),
        stochastic_path
    );

    // The base model is untouched.
    assert!(model.nodes["cogs"].forecast.is_none());
}

#[test]
fn value_overrides_win_over_forecast_overrides() {
    let model = forecast_model();
    let mut definition = scenario(None, &[("revenue", ForecastSpec::curve(vec![0.5; 3]))]);
    definition.period_overrides = IndexMap::from([(
        "revenue".to_string(),
        IndexMap::from([(q(3), AmountOrScalar::scalar(50.0))]),
    )]);
    let set = ScenarioSet {
        scenarios: IndexMap::from([("mixed".to_string(), definition)]),
    };

    let results = set.evaluate_all(&model).expect("evaluation succeeds");
    let path = revenue_path(&results.scenarios["mixed"]);

    // Q2 and Q4 follow the overridden curve; Q3 is the explicit value, which
    // replaces that output without restarting the forecast path.
    assert_eq!(path, vec![100.0, 150.0, 50.0, 337.5]);
}

#[test]
fn invalid_forecast_override_parameters_are_rejected_with_the_node() {
    let model = forecast_model();
    let mut typo = ForecastSpec::growth(0.05);
    typo.params
        .insert("rat".to_string(), serde_json::json!(0.1));
    let set = ScenarioSet {
        scenarios: IndexMap::from([("typo".to_string(), scenario(None, &[("revenue", typo)]))]),
    };

    let err = set
        .evaluate_all(&model)
        .expect_err("unknown forecast parameter");
    let message = err.to_string();
    assert!(message.contains("'revenue'"), "{message}");
    assert!(message.contains("rat"), "{message}");

    let missing = ScenarioSet {
        scenarios: IndexMap::from([(
            "missing".to_string(),
            scenario(None, &[("nope", ForecastSpec::forward_fill())]),
        )]),
    };
    let err = missing.evaluate_all(&model).expect_err("unknown node");
    assert!(err.to_string().contains("'nope'"));

    // The new field round-trips through the strict wire format.
    let json = serde_json::to_string(&set).expect("serialize");
    assert!(json.contains("forecast_overrides"));
    let restored: ScenarioSet = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        restored.scenarios["typo"].forecast_overrides["revenue"],
        set.scenarios["typo"].forecast_overrides["revenue"]
    );
}

#[test]
fn scenarios_evaluate_the_capital_structure() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::Month;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let model = {
        let mut model_builder = ModelBuilder::new("cs_scenarios");
        model_builder
            .periods("2025Q1..Q4", None)
            .expect("valid periods");
        model_builder.value(
            "revenue",
            &(1..=4)
                .map(|quarter| (q(quarter), AmountOrScalar::scalar(1_000.0)))
                .collect::<Vec<_>>(),
        );
        model_builder
            .add_bond(
                "BOND",
                Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue,
                Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
                "USD-OIS",
            )
            .expect("valid bond");
        model_builder
            .compute("interest", "cs.interest_expense.total")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };
    let set = ScenarioSet {
        scenarios: IndexMap::from([
            ("base".to_string(), ScenarioDefinition::default()),
            (
                "downside".to_string(),
                ScenarioDefinition {
                    parent: Some("base".to_string()),
                    overrides: IndexMap::from([(
                        "revenue".to_string(),
                        AmountOrScalar::scalar(900.0),
                    )]),
                    ..ScenarioDefinition::default()
                },
            ),
        ]),
    };
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (5.0, 0.9)])
            .build()
            .expect("curve should build"),
    );

    let plain = set.evaluate_all(&model).expect("plain scenario evaluation");
    let priced = set
        .evaluate_all_with_market(&model, &market, issue)
        .expect("market scenario evaluation");

    for results in [&plain, &priced] {
        for name in ["base", "downside"] {
            let result = &results.scenarios[name];
            let cashflows = result.cs_cashflows.as_ref().expect("cs_cashflows");
            assert!(cashflows.by_instrument.contains_key("BOND"));
            // The semi-annual coupon from 2025-01-01 pays on 1 July (Q3).
            let interest = result.get("interest", &q(3)).expect("interest");
            assert!(interest > 240.0 && interest < 260.0, "{name}: {interest}");
        }
        assert_eq!(
            results.scenarios["downside"].get("revenue", &q(1)),
            Some(900.0)
        );
    }
    assert_eq!(
        plain.scenarios["base"].get("interest", &q(3)),
        priced.scenarios["base"].get("interest", &q(3))
    );
}
