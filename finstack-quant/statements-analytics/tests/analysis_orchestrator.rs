//! Integration tests for the corporate analysis orchestrator.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;

fn flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ]);

    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.interp(InterpStyle::Linear).validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().expect("valid flat discount curve")
}

#[test]
fn test_full_lbo_analysis() {
    // Build a simple LBO model:
    // Revenue -> EBITDA -> UFCF with a bond in capital structure
    let model = ModelBuilder::new("lbo-test")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(10_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(10_500_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(11_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(11_500_000.0),
                ),
            ],
        )
        .compute("ebitda", "revenue * 0.25")
        .expect("ebitda formula")
        .compute("ufcf", "ebitda * 0.6")
        .expect("ufcf formula")
        .add_bond(
            "SENIOR-BOND",
            Money::new(20_000_000.0, Currency::USD),
            0.06,
            time::macros::date!(2025 - 01 - 01),
            time::macros::date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("valid bond")
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let market = MarketContext::new();

    let analysis = CorporateAnalysisBuilder::new(model)
        .market(market)
        .as_of(time::macros::date!(2025 - 01 - 01))
        .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
        .net_debt_override(20_000_000.0)
        .coverage_node("ebitda")
        .analyze()
        .expect("analysis should succeed");

    // Statement results populated
    let ebitda_q1 = analysis
        .statement
        .get("ebitda", &PeriodId::quarter(2025, 1));
    assert!(ebitda_q1.is_some());
    assert!(
        (ebitda_q1.unwrap() - 2_500_000.0).abs() < 1.0,
        "EBITDA Q1 should be ~2.5M (25% of 10M), got {}",
        ebitda_q1.unwrap()
    );

    // Equity populated
    assert!(analysis.equity.is_some());
    let equity = analysis.equity.as_ref().expect("equity result");
    assert!(
        equity.equity_value.amount() != 0.0,
        "equity value should be non-zero"
    );
    assert_eq!(equity.equity_value.currency(), Currency::USD);

    // Credit populated (bond should have coverage metrics)
    if !analysis.credit.is_empty() {
        let bond_analysis = analysis.credit.get("SENIOR-BOND");
        assert!(
            bond_analysis.is_some(),
            "SENIOR-BOND should be in credit results"
        );
        let bond = bond_analysis.expect("bond analysis");
        // DSCR should be positive (EBITDA > debt service)
        if !bond.coverage.dscr.is_empty() {
            assert!(
                bond.coverage.dscr[0].1 > 0.0,
                "DSCR should be positive, got {}",
                bond.coverage.dscr[0].1
            );
        }
    }
}

#[test]
fn test_statement_only_no_equity_no_credit() {
    let model = ModelBuilder::new("simple")
        .periods("2025Q1..Q2", None)
        .expect("valid periods")
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(105_000.0),
                ),
            ],
        )
        .build()
        .expect("valid model");

    let analysis = CorporateAnalysisBuilder::new(model)
        .analyze()
        .expect("should succeed");

    assert!(analysis.equity.is_none());
    assert!(analysis.credit.is_empty());
    assert!(analysis
        .statement
        .get("revenue", &PeriodId::quarter(2025, 1))
        .is_some());
}

#[test]
fn test_mixed_currency_capital_structure_does_not_panic_in_dynamic_evaluator() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let model = ModelBuilder::new("mixed-currency")
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
            "USD-BOND",
            Money::new(10_000_000.0, Currency::USD),
            0.05,
            as_of,
            time::macros::date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("usd bond")
        .add_bond(
            "EUR-BOND",
            Money::new(8_000_000.0, Currency::EUR),
            0.04,
            as_of,
            time::macros::date!(2030 - 01 - 01),
            "EUR-OIS",
        )
        .expect("eur bond")
        .with_meta("currency", serde_json::json!("USD"))
        .build()
        .expect("valid model");

    let market = MarketContext::new()
        .insert(flat_discount_curve(0.05, as_of, "USD-OIS"))
        .insert(flat_discount_curve(0.03, as_of, "EUR-OIS"));
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market, as_of)
        .expect("evaluation should succeed");

    let cs = results.cs_cashflows.expect("capital structure cashflows");
    assert_eq!(cs.totals_by_currency.len(), 2);
    assert!(cs.get_total_interest(&PeriodId::quarter(2025, 1)).is_err());
}
