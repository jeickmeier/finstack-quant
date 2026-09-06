//! Economic and typed-data boundaries found by the statements analytics audit.
use finstack_quant_core::{currency::Currency, dates::PeriodId};
use finstack_quant_statements::{
    builder::ModelBuilder,
    evaluator::{Evaluator, StatementResult},
    types::{AmountOrScalar, FinancialModelSpec},
};
use finstack_quant_statements_analytics::analysis::*;
use finstack_quant_statements_analytics::extensions::{corkscrew::*, scorecards::*};

fn monetary_model(currency: Currency) -> FinancialModelSpec {
    ModelBuilder::new("money")
        .periods("2025..2026", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::annual(2025),
                    AmountOrScalar::amount(100.0, currency).unwrap(),
                ),
                (
                    PeriodId::annual(2026),
                    AmountOrScalar::amount(110.0, currency).unwrap(),
                ),
            ],
        )
        .compute("profit", "revenue * 0.5")
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn monetary_goal_seek_preserves_round_trip_and_currency() {
    let mut model = monetary_model(Currency::USD);
    let period = PeriodId::annual(2025);
    assert!(
        (goal_seek(
            &mut model,
            "profit",
            period,
            60.0,
            "revenue",
            period,
            true,
            Some((1.0, 200.0))
        )
        .unwrap()
            - 120.0)
            .abs()
            < 1e-8
    );
    let model = ModelBuilder::from_spec(model).unwrap().build().unwrap();
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
    let analyzer = VarianceAnalyzer::new(&base, &comparison);
    let period = PeriodId::annual(2025);
    assert!(analyzer
        .compute(&VarianceConfig::new(
            "usd",
            "eur",
            vec!["revenue"],
            vec![period]
        ))
        .is_err());
    assert!(analyzer
        .bridge_decomposition("revenue", period, &["profit"], "usd", "eur")
        .is_err());
}

#[test]
fn trailing_year_requires_consecutive_calendar_periods() {
    let mut results = StatementResult::default();
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
        CreditAssessment::compute(&results, "2025Q4".parse().unwrap()).leverage_ratio,
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
    results
        .nodes
        .get_mut("revenue")
        .unwrap()
        .insert(PeriodId::annual(2026), f64::NAN);
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
    let report = CreditScorecardExtension::new(ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![],
        min_rating: None,
        period: None,
    })
    .execute(&model, &results)
    .unwrap();
    assert_eq!(report.status, ScorecardStatus::Failed);
    assert!(report.data["rating"].is_null());
    assert!(report.data["total_score"].is_null());
    assert_eq!(report.data["partial"], true);
}

fn dcf_model(debt_currency: Currency) -> FinancialModelSpec {
    let opening = PeriodId::annual(2024);
    let forecast = PeriodId::annual(2025);
    ModelBuilder::new("dcf-currencies")
        .periods("2024..2025", Some("2024"))
        .unwrap()
        .value(
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
        )
        .value(
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
        )
        .value(
            "cash",
            &[
                (opening, AmountOrScalar::amount(0.0, Currency::USD).unwrap()),
                (
                    forecast,
                    AmountOrScalar::amount(0.0, Currency::USD).unwrap(),
                ),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .unwrap()
}

#[test]
fn dcf_opening_debt_is_available_on_the_inclusive_reporting_date() {
    use finstack_quant_valuations::instruments::TerminalValueSpec;
    let model = dcf_model(Currency::USD);
    for as_of in [
        time::macros::date!(2024 - 12 - 31),
        time::macros::date!(2025 - 01 - 01),
    ] {
        let result = evaluate_dcf_with_market(
            &model,
            0.10,
            TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
            "ufcf",
            None,
            &DcfOptions::default(),
            None,
            Some(as_of),
        )
        .unwrap();
        assert_eq!(result.net_debt.amount(), 100.0);
    }
    let mixed = dcf_model(Currency::EUR);
    let result = evaluate_dcf_with_market(
        &mixed,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        None,
        &DcfOptions::default(),
        None,
        Some(time::macros::date!(2024 - 12 - 31)),
    );
    assert!(result.unwrap_err().to_string().contains("USD"));
}
