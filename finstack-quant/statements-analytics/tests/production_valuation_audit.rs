//! Regression coverage for valuation acceptance and monetary input contracts.
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::{builder::ModelBuilder, types::AmountOrScalar};
use finstack_quant_statements_analytics::analysis::goal_seek;

#[test]
fn discontinuous_objective_is_rejected_without_mutation() {
    let q = PeriodId::quarter(2025, 1).unwrap();
    for bounds in [None, Some((-1., 1.))] {
        let mut model = ModelBuilder::new("discontinuous")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("driver", &[(q, AmountOrScalar::scalar(1.))])
            .compute("target", "if(driver < 0, 0, 1)")
            .unwrap()
            .build()
            .unwrap();
        let before = serde_json::to_value(&model).unwrap();
        let error = goal_seek(&mut model, "target", q, 0.5, "driver", q, true, bounds)
            .expect_err("no attainable target");
        assert!(error.to_string().contains("residual"), "{error}");
        assert_eq!(serde_json::to_value(&model).unwrap(), before);
    }
}

#[test]
fn credit_reports_and_checks_reject_mixed_currency_ratios() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_statements::checks::{Check, CheckContext};
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements_analytics::analysis::{
        checks::{CoverageFloorCheck, LeverageRangeCheck},
        CreditAssessment,
    };
    let period = PeriodId::annual(2025);
    let model = ModelBuilder::new("mixed credit units")
        .periods("2025..2025", None)
        .unwrap()
        .value(
            "total_debt",
            &[(
                period,
                AmountOrScalar::amount(100.0, Currency::USD).unwrap(),
            )],
        )
        .value(
            "ebitda",
            &[(period, AmountOrScalar::amount(10.0, Currency::EUR).unwrap())],
        )
        .value(
            "interest_expense",
            &[(period, AmountOrScalar::amount(2.0, Currency::GBP).unwrap())],
        )
        .build()
        .unwrap();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    let report = CreditAssessment::compute(&results, period);
    assert_eq!(report.leverage_ratio, None);
    assert_eq!(report.interest_coverage, None);
    let context = CheckContext::new(&model, &results);
    let leverage = LeverageRangeCheck {
        debt_node: "total_debt".into(),
        ebitda_node: "ebitda".into(),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 8.0),
    };
    let coverage = CoverageFloorCheck {
        numerator_node: "ebitda".into(),
        denominator_node: "interest_expense".into(),
        min_warning: 2.0,
        min_error: 1.0,
    };
    assert!(leverage.execute(&context).is_err());
    assert!(coverage.execute(&context).is_err());
    results.node_value_types.clear();
    let report = CreditAssessment::compute(&results, period);
    assert_eq!(report.leverage_ratio, None);
    assert_eq!(report.interest_coverage, None);
    let context = CheckContext::new(&model, &results);
    assert!(leverage.execute(&context).is_err());
    assert!(coverage.execute(&context).is_err());
}

#[test]
fn credit_checks_reject_overflowing_ratios() {
    use finstack_quant_statements::checks::{Check, CheckContext};
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements_analytics::analysis::checks::{
        CoverageFloorCheck, LeverageRangeCheck,
    };
    let period = PeriodId::annual(2025);
    let model = ModelBuilder::new("overflowing credit ratio")
        .periods("2025..2025", None)
        .unwrap()
        .value("large", &[(period, AmountOrScalar::scalar(100.0))])
        .value("small", &[(period, AmountOrScalar::scalar(1e-320))])
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let context = CheckContext::new(&model, &results);
    assert!(LeverageRangeCheck {
        debt_node: "large".into(),
        ebitda_node: "small".into(),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 8.0),
    }
    .execute(&context)
    .is_err());
    assert!(CoverageFloorCheck {
        numerator_node: "large".into(),
        denominator_node: "small".into(),
        min_warning: 2.0,
        min_error: 1.0,
    }
    .execute(&context)
    .is_err());
}

#[test]
fn credit_reports_reject_contradictory_numeric_and_monetary_values() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements_analytics::analysis::CreditAssessment;
    let period = PeriodId::annual(2025);
    let model = ModelBuilder::new("contradictory monetary projection")
        .periods("2025..2025", None)
        .unwrap()
        .value_money(
            "total_debt",
            &[(period, Money::from((500_i64, Currency::USD)))],
        )
        .value_money("ebitda", &[(period, Money::from((100_i64, Currency::USD)))])
        .build()
        .unwrap();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    assert_eq!(
        CreditAssessment::compute(&results, period).leverage_ratio,
        Some(5.0)
    );
    results
        .nodes
        .get_mut("ebitda")
        .unwrap()
        .insert(period, 1_000.0);
    assert_eq!(
        CreditAssessment::compute(&results, period).leverage_ratio,
        None
    );
    let precise = Money::from_decimal_str("100.12345", Currency::USD).unwrap();
    results
        .monetary_nodes
        .get_mut("ebitda")
        .unwrap()
        .insert(period, precise);
    results
        .nodes
        .get_mut("ebitda")
        .unwrap()
        .insert(period, precise.amount());
    assert_eq!(
        CreditAssessment::compute(&results, period).leverage_ratio,
        Some(500.0 / precise.amount())
    );
}

#[test]
fn working_capital_rejects_currency_change_without_metadata() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_statements::checks::{Check, CheckContext};
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements_analytics::analysis::checks::WorkingCapitalConsistency;
    let first = PeriodId::quarter(2025, 1).unwrap();
    let second = PeriodId::quarter(2025, 2).unwrap();
    let model = ModelBuilder::new("changed working capital currency")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value_money(
            "wc",
            &[
                (first, Money::from((0_i64, Currency::USD))),
                (second, Money::from((-10_i64, Currency::USD))),
            ],
        )
        .value_money(
            "assets",
            &[
                (first, Money::from((100_i64, Currency::USD))),
                (second, Money::from((110_i64, Currency::USD))),
            ],
        )
        .build()
        .unwrap();
    let mut results = Evaluator::new().evaluate(&model).unwrap();
    results.node_value_types.clear();
    results
        .monetary_nodes
        .get_mut("assets")
        .unwrap()
        .insert(first, Money::from((100_i64, Currency::EUR)));
    let context = CheckContext::new(&model, &results);
    let error = WorkingCapitalConsistency {
        wc_change_cf_node: "wc".into(),
        current_assets_nodes: vec!["assets".into()],
        current_liabilities_nodes: Vec::new(),
        tolerance: None,
    }
    .execute(&context)
    .unwrap_err();
    assert!(error.to_string().contains("retain its type and currency"));
}

#[test]
fn growth_terminal_flow_rejects_missing_calendar_quarters() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
    use finstack_quant_valuations::instruments::TerminalValueSpec;
    let values: Vec<_> = (2025..=2026)
        .flat_map(|year| {
            (1..=4).map(move |quarter| {
                (
                    PeriodId::quarter(year, quarter).unwrap(),
                    Money::from((25_i64, Currency::USD)),
                )
            })
        })
        .collect();
    let mut model = ModelBuilder::new("sparse terminal year")
        .periods("2025Q1..2026Q4", None)
        .unwrap()
        .value_money("ufcf", &values)
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .unwrap();
    let terminal = TerminalValueSpec::GordonGrowth {
        stable_growth_rate: 0.02,
    };
    let valid = evaluate_dcf_with_market(
        &model,
        0.10,
        terminal.clone(),
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        valid.dcf_instrument.unwrap().terminal_flow_override,
        Some(100.0)
    );
    let missing_period = PeriodId::quarter(2026, 1).unwrap();
    model.periods.retain(|period| period.id != missing_period);
    model
        .nodes
        .get_mut("ufcf")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .shift_remove(&missing_period);
    model.validate_semantics().unwrap();
    let error = evaluate_dcf_with_market(
        &model,
        0.10,
        terminal,
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("complete contiguous history"));
}

#[test]
fn growth_terminal_flow_rejects_partial_week_at_calendar_year_boundary() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
    use finstack_quant_valuations::instruments::TerminalValueSpec;
    let values: Vec<_> = (1..=52)
        .map(|week| {
            (
                PeriodId::week(2025, week).unwrap(),
                Money::from((25_i64, Currency::USD)),
            )
        })
        .collect();
    let model = ModelBuilder::new("unaligned weekly terminal year")
        .periods("2025W01..W52", None)
        .unwrap()
        .value_money("ufcf", &values)
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .unwrap();
    let error = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("complete contiguous history"));
}

#[test]
fn growth_terminal_flow_rejects_annual_identifier_on_partial_year() {
    use finstack_quant_core::{currency::Currency, dates::Period, money::Money};
    use finstack_quant_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
    use finstack_quant_valuations::instruments::TerminalValueSpec;
    let period = PeriodId::annual(2025);
    let model = ModelBuilder::new("annual stub")
        .periods_explicit(vec![Period {
            id: period,
            start: time::macros::date!(2025 - 07 - 01),
            end: time::macros::date!(2026 - 01 - 01),
            is_actual: false,
        }])
        .unwrap()
        .value_money("ufcf", &[(period, Money::from((100_i64, Currency::USD)))])
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .unwrap();
    let error = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("complete contiguous history"));
}

#[test]
fn growth_terminal_flow_uses_complete_year_across_annual_stub_identifiers() {
    use finstack_quant_core::{currency::Currency, dates::Period, money::Money};
    use finstack_quant_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
    use finstack_quant_valuations::instruments::TerminalValueSpec;
    let actual = PeriodId::annual(2024);
    let forecast = PeriodId::annual(2025);
    let model = ModelBuilder::new("complete year across annual stubs")
        .periods_explicit(vec![
            Period {
                id: actual,
                start: time::macros::date!(2024 - 07 - 01),
                end: time::macros::date!(2025 - 01 - 01),
                is_actual: true,
            },
            Period {
                id: forecast,
                start: time::macros::date!(2025 - 01 - 01),
                end: time::macros::date!(2025 - 07 - 01),
                is_actual: false,
            },
        ])
        .unwrap()
        .value_money(
            "ufcf",
            &[
                (actual, Money::from((100_i64, Currency::USD))),
                (forecast, Money::from((100_i64, Currency::USD))),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .unwrap();
    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .unwrap();
    let dcf = result.dcf_instrument.unwrap();
    assert_eq!(dcf.terminal_flow_override, Some(200.0));
    let terminal_value = dcf.calculate_terminal_value().unwrap();
    assert!((terminal_value - 200.0 * 1.02 / (0.10 - 0.02)).abs() < 1e-10);
}
