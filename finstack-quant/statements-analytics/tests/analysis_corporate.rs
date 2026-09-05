//! Corporate analysis integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::NonFiniteCheck;
use finstack_quant_statements::checks::CheckSuite;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
use finstack_quant_valuations::instruments::TerminalValueSpec;
use time::Month;

fn non_finite_suite() -> CheckSuite {
    CheckSuite::builder("corporate-test")
        .add_check(NonFiniteCheck { nodes: vec![] })
        .build()
}

#[test]
fn test_dcf_evaluation_gordon_growth() {
    let model = ModelBuilder::new("test-corp")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(110_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(120_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(130_000.0, Currency::USD).expect("valid money fixture"),
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
        None,
    )
    .expect("DCF evaluation should succeed");

    assert!(result.equity_value.amount() > 0.0);
    assert_eq!(result.equity_value.currency(), Currency::USD);
}

#[test]
fn test_dcf_rejects_scalar_ufcf_node() {
    let model = ModelBuilder::new("scalar-ufcf")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .value_scalar(
            "ufcf",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                100_000.0,
            )],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("model");

    let error = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .expect_err("scalar UFCF must fail");
    assert!(error.to_string().contains("must be monetary"));
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
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .add_bond(
            "BOND-001",
            Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
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
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
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
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let options = DcfOptions::default();

    // Test with None market context
    let result_no_market = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &options,
        None,
        None,
    )
    .expect("should succeed without market context");

    assert!(result_no_market.equity_value.amount() > 0.0);
    assert_eq!(result_no_market.equity_value.currency(), Currency::USD);

    // A market without as_of must not silently drop curve lookups.
    let market = MarketContext::new();
    let missing_as_of = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &options,
        Some(&market),
        None,
    );
    assert!(
        missing_as_of.is_err(),
        "market without as_of must error rather than evaluate without curves"
    );

    let result_with_market = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &options,
        Some(&market),
        Some(as_of),
    )
    .expect("should succeed with market context and as_of");

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
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(110_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(120_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(130_000.0, Currency::USD).expect("valid money fixture"),
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
fn test_dcf_uses_as_of_for_valuation_date_and_auto_net_debt() {
    let model = ModelBuilder::new("hist-boundary-dcf")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(110_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(120_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(130_000.0, Currency::USD).expect("valid money fixture"),
                ),
            ],
        )
        .value(
            "total_debt",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(40.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
            ],
        )
        .value(
            "cash",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let as_of = Date::from_calendar_date(2025, Month::August, 15).expect("valid date");
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
    .expect("DCF evaluation should succeed");

    let last_available_period = model
        .periods
        .iter()
        .rev()
        .find(|period| period.end <= as_of)
        .expect("balance-sheet period before valuation date");
    let dcf = result
        .dcf_instrument
        .as_ref()
        .expect("dcf instrument should be returned");

    assert_eq!(
        dcf.valuation_date, as_of,
        "DCF should discount from the supplied as-of date"
    );
    assert!(
        (result.net_debt.amount() - 100.0).abs() < 1e-9,
        "auto net debt should come from the latest balance sheet before as-of"
    );
    assert_eq!(
        last_available_period.id,
        PeriodId::quarter(2025, 2).expect("valid period fixture")
    );
}

#[test]
fn test_dcf_forecast_only_uses_first_forecast_boundary_for_net_debt() {
    let model = ModelBuilder::new("forecast-only-dcf")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(110_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(120_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(130_000.0, Currency::USD).expect("valid money fixture"),
                ),
            ],
        )
        .value(
            "total_debt",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(75.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(40.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
            ],
        )
        .value(
            "cash",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
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
        None,
        &DcfOptions::default(),
        None,
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

fn make_simple_dcf_model() -> finstack_quant_statements::types::FinancialModelSpec {
    ModelBuilder::new("parity-dcf")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(110_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(120_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(130_000.0, Currency::USD).expect("valid money fixture"),
                ),
            ],
        )
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model")
}

#[test]
fn parity_orchestrator_dcf_matches_standalone() {
    use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
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
        None,
    )
    .expect("standalone evaluate_dcf_with_market");

    let orchestrated = CorporateAnalysisBuilder::new(model)
        .dcf(0.10, tv)
        .net_debt_override(50_000.0)
        .checks(non_finite_suite())
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

// --- Quant review fixes: terminal-flow annualization, discounting basis ---

/// Quarterly grid + Gordon Growth: the terminal flow must be annualized as
/// the trailing sum of the final year's quarters, so the TV matches the
/// hand-computed Gordon value on the annual flow.
#[test]
fn quarterly_gordon_terminal_value_uses_annualized_flow() {
    let model = make_simple_dcf_model(); // quarterly UFCF: 100k,110k,120k,130k

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    )
    .expect("DCF evaluation");

    let dcf = result.dcf_instrument.expect("instrument");
    let annual_flow = 100_000.0 + 110_000.0 + 120_000.0 + 130_000.0;
    assert_eq!(dcf.terminal_flow_override, Some(annual_flow));

    let tv = dcf.calculate_terminal_value().expect("terminal value");
    let expected_tv = annual_flow * 1.02 / (0.10 - 0.02);
    assert!(
        (tv - expected_tv).abs() < 1e-6,
        "quarterly-grid TV must capitalize the trailing annual flow: expected {expected_tv}, got {tv}"
    );
}

/// Annual grid: terminal value behavior is unchanged (last flow used as-is).
#[test]
fn annual_gordon_terminal_value_unchanged() {
    let model = ModelBuilder::new("annual-dcf")
        .periods("2025..2027", None)
        .expect("valid periods")
        .value_money(
            "ufcf",
            &[
                (
                    PeriodId::annual(2025),
                    Money::new(400_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::annual(2026),
                    Money::new(440_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::annual(2027),
                    Money::new(480_000.0, Currency::USD).expect("valid money fixture"),
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
        None,
    )
    .expect("DCF evaluation");

    let dcf = result.dcf_instrument.expect("instrument");
    assert_eq!(dcf.terminal_flow_override, None);

    let tv = dcf.calculate_terminal_value().expect("terminal value");
    let expected_tv = 480_000.0 * 1.02 / (0.10 - 0.02);
    assert!((tv - expected_tv).abs() < 1e-6);
}

/// A "USD-DISCOUNT" market curve must not change the discounting basis:
/// all components stay on WACC and equity = EV - net debt holds exactly.
#[test]
fn usd_discount_curve_does_not_mix_discounting_bases() {
    let model = make_simple_dcf_model();
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let usd_discount = DiscountCurve::builder("USD-DISCOUNT")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.8)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(usd_discount);

    let net_debt = 50_000.0;
    let with_market = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(net_debt),
        &DcfOptions::default(),
        Some(&market),
        Some(base_date),
    )
    .expect("DCF with market");

    let without_market = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(net_debt),
        &DcfOptions::default(),
        None,
        None,
    )
    .expect("DCF without market");

    // Internal consistency of the envelope: equity = EV - net debt.
    assert!(
        (with_market.equity_value.amount()
            - (with_market.enterprise_value.amount() - with_market.net_debt.amount()))
        .abs()
            < 1e-6,
        "equity must reconcile to EV - net debt with a market curve loaded"
    );

    // Loading a conventionally-named curve must not change the valuation.
    assert!(
        (with_market.equity_value.amount() - without_market.equity_value.amount()).abs() < 1e-6,
        "a USD-DISCOUNT market curve must not alter the WACC discounting basis"
    );

    // The DCF carries no discount-curve reference at all: it discounts at its
    // own WACC, so a market curve cannot change the basis.
    assert!(with_market.dcf_instrument.is_some());
}

/// NaN terminal-value parameters must error instead of producing NaN values.
#[test]
fn nan_terminal_value_parameters_error() {
    let model = make_simple_dcf_model();

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth {
            growth_rate: f64::NAN,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    );
    assert!(result.is_err(), "NaN growth_rate must fail closed");
}

#[test]
fn stable_growth_above_policy_ceiling_is_rejected() {
    let model = make_simple_dcf_model();
    let options = DcfOptions {
        max_stable_growth_rate: 0.03,
        ..DcfOptions::default()
    };
    let error = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.04 },
        "ufcf",
        Some(0.0),
        &options,
        None,
        None,
    )
    .expect_err("growth above policy ceiling must fail");
    assert!(error.to_string().contains("ceiling"));
}

/// Standalone DCF must evaluate statements with market + as_of so
/// curve-dependent capital-structure nodes such as `cs.interest` resolve.
#[test]
fn evaluate_dcf_with_market_uses_curve_for_cs_interest() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.8)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc_curve);

    let model = ModelBuilder::new("dcf-cs-interest")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .expect("periods")
        .value_money(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Money::new(1_100_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    Money::new(1_200_000.0, Currency::USD).expect("valid money fixture"),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    Money::new(1_300_000.0, Currency::USD).expect("valid money fixture"),
                ),
            ],
        )
        .availability_dates(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                as_of,
            )],
        )
        .expect("availability")
        .add_bond(
            "BOND-001",
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            0.05,
            as_of,
            Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
            "USD-OIS",
        )
        .expect("bond")
        .compute("ufcf", "revenue - cs.interest_expense.total")
        .expect("formula")
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("model");

    let no_market = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        None,
        None,
    );
    assert!(
        no_market.is_err(),
        "cs.interest requires market-backed statement evaluation"
    );

    let missing_as_of = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        Some(&market),
        None,
    );
    assert!(
        missing_as_of.is_err(),
        "market without as_of must not silently drop CS curve lookups"
    );

    let result = evaluate_dcf_with_market(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
        Some(&market),
        Some(as_of),
    )
    .expect("standalone DCF must evaluate CS interest from the market curve");

    assert!(result.enterprise_value.amount() > 0.0);
    let dcf = result.dcf_instrument.expect("instrument");
    assert_eq!(dcf.flows.len(), 3, "three forecast quarters are explicit");
    // Semi-annual US corporate coupon from 2025-01-01 pays in Q3; that
    // period's UFCF must be revenue minus curve-priced interest.
    let q3_ufcf = dcf.flows[1].1;
    assert!(
        q3_ufcf < 1_200_000.0,
        "Q3 UFCF must deduct the July coupon, got {q3_ufcf}"
    );
}
