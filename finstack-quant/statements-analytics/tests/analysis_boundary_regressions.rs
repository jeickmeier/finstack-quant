//! Economic and typed-data boundaries found by the statements analytics audit.
use finstack_quant_core::{currency::Currency, dates::PeriodId};
use finstack_quant_statements::{
    builder::ModelBuilder,
    evaluator::{Evaluator, StatementResult},
    types::{AmountOrScalar, FinancialModelSpec},
};
use finstack_quant_statements_analytics::analysis::*;
use finstack_quant_statements_analytics::extensions::*;

/// Caller-supplied test registry (agency-style labels, test-owned numbers).
fn agency_style_registry() -> finstack_quant_core::rating_scales::RatingScaleRegistry {
    finstack_quant_core::rating_scales::RatingScaleRegistry::from_json(include_str!(
        "data/agency_style_rating_scales.json"
    ))
    .expect("valid test rating-scale registry")
}

fn monetary_model(currency: Currency) -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("money");
        model_builder.periods("2025..2026", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::annual(2025).expect("valid annual period"),
                    AmountOrScalar::amount(100.0, currency).unwrap(),
                ),
                (
                    PeriodId::annual(2026).expect("valid annual period"),
                    AmountOrScalar::amount(110.0, currency).unwrap(),
                ),
            ],
        );
        model_builder.compute("profit", "revenue * 0.5").unwrap();
        model_builder.build().unwrap()
    }
}

#[test]
fn monetary_goal_seek_preserves_round_trip_and_currency() {
    let model = monetary_model(Currency::USD);
    let period = PeriodId::annual(2025).expect("valid annual period");
    let solved = goal_seek(
        &model,
        &GoalSeekSpec {
            target_node: "profit".to_string(),
            target_period: period,
            target_value: 60.0,
            driver_node: "revenue".to_string(),
            driver_period: period,
            update_model: true,
            bounds: Some((1.0, 200.0)),
        },
    )
    .unwrap();
    assert!((solved.solved_value - 120.0).abs() < 1e-8);
    let model = solved.model.expect("update_model=true returns the model");
    let model = {
        let model_builder = ModelBuilder::from_spec(model).unwrap();
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    let revenue = results.get_money("revenue", &period).unwrap();
    assert_eq!(revenue.currency(), Currency::USD);
    assert!((revenue.amount() - 120.0).abs() < 1e-8);
}

#[test]
fn variance_and_bridge_reject_currency_mismatch() {
    let base = Evaluator::new()
        .evaluate(&monetary_model(Currency::USD))
        .unwrap();
    let comparison = Evaluator::new()
        .evaluate(&monetary_model(Currency::EUR))
        .unwrap();
    let period = PeriodId::annual(2025).expect("valid annual period");
    assert!(
        run_variance(
            &base,
            &comparison,
            &VarianceConfig::new("usd", "eur", vec!["revenue"], vec![period])
        )
        .is_err()
    );
    assert!(
        variance_bridge(
            &base,
            &comparison,
            "revenue",
            period,
            &["profit"],
            "usd",
            "eur"
        )
        .is_err()
    );
}

#[test]
fn trailing_year_requires_consecutive_calendar_periods() {
    let mut results = StatementResult {
        periods: finstack_quant_core::dates::build_periods("2024Q1..2025Q4", None)
            .unwrap()
            .periods,
        ..StatementResult::default()
    };
    for p in ["2024Q1", "2024Q2", "2024Q3", "2025Q4"] {
        results
            .nodes
            .entry("ebitda".into())
            .or_default()
            .insert(p.parse().unwrap(), 100.0);
    }
    results
        .nodes
        .entry("total_debt".into())
        .or_default()
        .insert("2025Q4".parse().unwrap(), 800.0);
    assert!(
        CreditAssessment::compute(&results, "2025Q4".parse().unwrap())
            .unwrap()
            .leverage_ratio
            .is_none()
    );
    for p in ["2025Q1", "2025Q2", "2025Q3"] {
        results
            .nodes
            .entry("ebitda".into())
            .or_default()
            .insert(p.parse().unwrap(), 100.0);
    }
    assert_eq!(
        CreditAssessment::compute(&results, "2025Q4".parse().unwrap())
            .unwrap()
            .leverage_ratio,
        Some(2.0)
    );
}

#[test]
fn regression_rejects_unidentified_and_unaligned_samples() {
    assert!(regression_fair_value(&[1.0; 3], &[2.0, 4.0, 6.0], 2.0, 6.0).is_none());
    assert!(regression_fair_value(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0, 8.0], 2.0, 6.0).is_none());
    assert!(regression_fair_value(&[1.0, 2.0, f64::NAN], &[2.0, 4.0, 6.0], 2.0, 6.0).is_none());
    let fitted = regression_fair_value(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0], 4.0, 9.0).unwrap();
    assert!((fitted.fitted_value - 8.0).abs() < 1e-12);
}

#[test]
fn corkscrew_rejects_nan_in_strict_and_report_modes() {
    let model = monetary_model(Currency::USD);
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.nodes.get_mut("revenue").unwrap().insert(
        PeriodId::annual(2026).expect("valid annual period"),
        f64::NAN,
    );
    results
        .nodes
        .insert("liability".into(), results.nodes["revenue"].clone());
    for strict in [true, false] {
        let accounts = [
            ("revenue", AccountType::Asset),
            ("liability", AccountType::Liability),
        ]
        .into_iter()
        .map(|(id, account_type)| CorkscrewAccount {
            node_id: id.into(),
            account_type,
            changes: vec![],
            decreases: vec![],
            beginning_balance_node: None,
        })
        .collect();
        let result = CorkscrewExtension::new(CorkscrewConfig {
            accounts,
            tolerance: 0.01,
            fail_on_error: strict,
        })
        .execute(&model, &results);
        if strict {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().status, CorkscrewStatus::Failed);
        }
    }
}

#[test]
fn empty_scorecard_has_no_fabricated_rating() {
    let model = monetary_model(Currency::USD);
    let results = Evaluator::new().evaluate(&model).unwrap();
    let report = CreditScorecardExtension::new(
        ScorecardConfig {
            rating_scale: "sp".into(),
            metrics: vec![],
            min_rating: None,
            period: None,
        },
        &agency_style_registry(),
    )
    .expect("valid scorecard")
    .execute(&model, &results)
    .unwrap();
    assert_eq!(report.status, ScorecardStatus::Failed);
    assert!(report.data["rating"].is_null());
    assert!(report.data["total_score"].is_null());
    assert_eq!(report.data["partial"], true);
}

fn dcf_model(debt_currency: Currency) -> FinancialModelSpec {
    let opening = PeriodId::annual(2024).expect("valid annual period");
    let forecast = PeriodId::annual(2025).expect("valid annual period");
    {
        let mut model_builder = ModelBuilder::new("dcf-currencies");
        model_builder.periods("2024..2025", Some("2024")).unwrap();
        model_builder.value(
            "ufcf",
            &[
                (
                    opening,
                    AmountOrScalar::amount(90.0, Currency::USD).unwrap(),
                ),
                (
                    forecast,
                    AmountOrScalar::amount(100.0, Currency::USD).unwrap(),
                ),
            ],
        );
        model_builder.value(
            "total_debt",
            &[
                (
                    opening,
                    AmountOrScalar::amount(100.0, debt_currency).unwrap(),
                ),
                (
                    forecast,
                    AmountOrScalar::amount(10.0, debt_currency).unwrap(),
                ),
            ],
        );
        model_builder.value(
            "cash",
            &[
                (opening, AmountOrScalar::amount(0.0, Currency::USD).unwrap()),
                (
                    forecast,
                    AmountOrScalar::amount(0.0, Currency::USD).unwrap(),
                ),
            ],
        );
        model_builder.with_meta("currency", serde_json::json!("USD"));
        model_builder.build().unwrap()
    }
}

#[test]
fn dcf_opening_debt_is_available_on_the_inclusive_reporting_date() {
    use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
    let model = dcf_model(Currency::USD);
    for as_of in [
        time::macros::date!(2024 - 12 - 31),
        time::macros::date!(2025 - 01 - 01),
    ] {
        let result = evaluate_dcf(
            &model,
            &DcfInputs {
                wacc: 0.10,
                terminal_value: TerminalValueSpec::GordonGrowth {
                    stable_growth_rate: 0.02,
                },
                ufcf_node: "ufcf".to_string(),
                net_debt_override: None,
            },
            &DcfOptions::default(),
            None,
            Some(as_of),
        )
        .unwrap();
        assert_eq!(result.net_debt.amount(), 100.0);
    }
    let mixed = dcf_model(Currency::EUR);
    let result = evaluate_dcf(
        &mixed,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
        },
        &DcfOptions::default(),
        None,
        Some(time::macros::date!(2024 - 12 - 31)),
    );
    assert!(result.unwrap_err().to_string().contains("USD"));
}

#[test]
fn credit_assessment_rejects_a_period_outside_the_results() {
    let model = {
        let mut model_builder = ModelBuilder::new("quarterly credit");
        model_builder.periods("2025Q1..Q4", None).unwrap();
        model_builder.value(
            "total_debt",
            &(1..=4)
                .map(|q| {
                    (
                        PeriodId::quarter(2025, q).unwrap(),
                        AmountOrScalar::scalar(300.0),
                    )
                })
                .collect::<Vec<_>>(),
        );
        model_builder.build().unwrap()
    };
    let results = Evaluator::new().evaluate(&model).unwrap();
    assert!(CreditAssessment::compute(&results, PeriodId::quarter(2025, 4).unwrap()).is_ok());
    // A period past the model and an annual id on a quarterly model are both
    // unknown: neither may come back as an all-null or truncated assessment.
    for period in [
        PeriodId::quarter(2030, 1).unwrap(),
        PeriodId::annual(2025).expect("valid annual period"),
    ] {
        let error = CreditAssessment::compute(&results, period).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("is not a period of the statement results"),
            "{error}"
        );
    }
}
