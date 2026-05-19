//! Corporate analysis integration tests.
#![allow(clippy::expect_used)]

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, PeriodId};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_statements::builder::ModelBuilder;
use finstack_statements::evaluator::Evaluator;
use finstack_statements::types::AmountOrScalar;
use finstack_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
use finstack_valuations::instruments::TerminalValueSpec;
use time::Month;

#[test]
fn test_dcf_evaluation_gordon_growth() {
    let model = ModelBuilder::new("test-corp")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "ufcf",
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
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(50_000.0),
        &DcfOptions::default(),
        None,
    )
    .expect("DCF evaluation should succeed");

    assert!(result.equity_value.amount() > 0.0);
    assert_eq!(result.equity_value.currency(), Currency::USD);
}

#[test]
fn test_cs_cashflows_populated_on_statement_result() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let model = ModelBuilder::new("cs-test")
        .periods("2025Q1..Q2", None)
        .expect("valid periods")
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .add_bond(
            "BOND-001",
            Money::new(10_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("valid bond")
        .build()
        .expect("model should build");

    // Market context with a simple discount curve
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .expect("curve should build");
    let market_ctx = MarketContext::new().insert(disc_curve);

    let mut evaluator = Evaluator::new();
    let result = evaluator
        .evaluate_with_market(&model, &market_ctx, issue)
        .expect("evaluation should succeed");

    // cs_cashflows should be populated when model has a capital structure
    assert!(
        result.cs_cashflows.is_some(),
        "cs_cashflows should be Some when model has capital_structure"
    );

    let cs = result.cs_cashflows.as_ref().expect("cs_cashflows present");

    // Should have per-instrument data for BOND-001
    assert!(
        cs.by_instrument.contains_key("BOND-001"),
        "by_instrument should contain BOND-001"
    );

    // Should have totals for both periods
    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);
    assert!(cs.totals.contains_key(&q1), "totals should contain Q1 2025");
    assert!(cs.totals.contains_key(&q2), "totals should contain Q2 2025");

    // Debt balance should be positive
    let total_balance_q1 = cs
        .get_total_debt_balance(&q1)
        .expect("total debt balance Q1");
    assert!(
        total_balance_q1 > 0.0,
        "debt balance should be positive, got {}",
        total_balance_q1
    );
}

#[test]
fn test_dcf_with_market_context() {
    let model = ModelBuilder::new("mkt-test")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(100_000.0),
                ),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    // Test with None market context
    let result_no_market = finstack_statements_analytics::analysis::evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &finstack_statements_analytics::analysis::DcfOptions::default(),
        None,
    )
    .expect("should succeed without market context");

    assert!(result_no_market.equity_value.amount() > 0.0);
    assert_eq!(result_no_market.equity_value.currency(), Currency::USD);

    // Test with explicit market context
    let market = MarketContext::new();
    let result_with_market = finstack_statements_analytics::analysis::evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &finstack_statements_analytics::analysis::DcfOptions::default(),
        Some(&market),
    )
    .expect("should succeed with market context");

    assert!(result_with_market.equity_value.amount() > 0.0);
    // With empty market, results should be the same
    assert!(
        (result_no_market.equity_value.amount() - result_with_market.equity_value.amount()).abs()
            < 0.01,
        "Results should match for empty vs None market context"
    );
}

#[test]
fn test_dcf_excludes_historical_periods_from_explicit_flows() {
    let model = ModelBuilder::new("hist-vs-forecast")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .value(
            "ufcf",
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
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
    )
    .expect("DCF evaluation should succeed");

    let dcf = result
        .dcf_instrument
        .expect("dcf instrument should be returned");
    assert_eq!(
        dcf.flows.len(),
        2,
        "only forecast periods should feed DCF flows"
    );
    assert_eq!(dcf.flows[0].1, 120_000.0);
    assert_eq!(dcf.flows[1].1, 130_000.0);
}

#[test]
fn test_dcf_uses_forecast_boundary_for_valuation_date_and_auto_net_debt() {
    let model = ModelBuilder::new("hist-boundary-dcf")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .value(
            "ufcf",
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
        .value(
            "total_debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(40.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(10.0)),
            ],
        )
        .value(
            "cash",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.0)),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        None,
        &DcfOptions::default(),
        None,
    )
    .expect("DCF evaluation should succeed");

    let first_forecast = model
        .periods
        .iter()
        .find(|p| !p.is_actual)
        .expect("forecast period should exist");
    let last_actual = model
        .periods
        .iter()
        .rfind(|p| p.is_actual)
        .expect("actual period should exist");
    let dcf = result
        .dcf_instrument
        .as_ref()
        .expect("dcf instrument should be returned");

    assert_eq!(
        dcf.valuation_date, first_forecast.start,
        "DCF should discount from the first forecast start date"
    );
    assert_eq!(
        dcf.valuation_date, last_actual.end,
        "valuation date should align with the last actual balance-sheet boundary"
    );
    assert!(
        (result.net_debt.amount() - 100.0).abs() < 1e-9,
        "auto net debt should come from the last actual balance sheet, not the terminal forecast period"
    );
}

#[test]
fn test_dcf_forecast_only_uses_first_forecast_boundary_for_net_debt() {
    let model = ModelBuilder::new("forecast-only-dcf")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "ufcf",
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
        .value(
            "total_debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(75.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(40.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(10.0)),
            ],
        )
        .value(
            "cash",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.0)),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        None,
        &DcfOptions::default(),
        None,
    )
    .expect("DCF evaluation should succeed");

    let dcf = result
        .dcf_instrument
        .as_ref()
        .expect("dcf instrument should be returned");
    let first_forecast = model.periods.first().expect("forecast period should exist");

    assert_eq!(dcf.valuation_date, first_forecast.start);
    assert!(
        (result.net_debt.amount() - 100.0).abs() < 1e-9,
        "forecast-only auto net debt should come from the valuation boundary, not the terminal period"
    );
}

// --- Parity tests: all wrapper entrypoints must produce identical results ---

fn make_simple_dcf_model() -> finstack_statements::types::FinancialModelSpec {
    ModelBuilder::new("parity-dcf")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "ufcf",
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
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model")
}

#[test]
fn parity_orchestrator_dcf_matches_standalone() {
    use finstack_statements_analytics::analysis::CorporateAnalysisBuilder;
    let model = make_simple_dcf_model();
    let tv = TerminalValueSpec::GordonGrowth { growth_rate: 0.02 };

    let standalone = evaluate_dcf_with_market(
        &model,
        0.10,
        tv.clone(),
        "ufcf",
        Some(50_000.0),
        &DcfOptions::default(),
        None,
    )
    .expect("standalone evaluate_dcf_with_market");

    let orchestrated = CorporateAnalysisBuilder::new(model)
        .dcf(0.10, tv)
        .net_debt_override(50_000.0)
        .analyze()
        .expect("orchestrated analysis")
        .equity
        .expect("equity result");

    assert!(
        (standalone.equity_value.amount() - orchestrated.equity_value.amount()).abs() < 1e-6,
        "standalone and orchestrated DCF must match: {} vs {}",
        standalone.equity_value.amount(),
        orchestrated.equity_value.amount()
    );
}
