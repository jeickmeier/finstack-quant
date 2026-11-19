//! Monte Carlo evaluation of models with a capital structure.
//!
//! Each path runs the deterministic evaluator on its sampled forecasts, so the
//! waterfall (here a 50% ECF sweep into a term loan) and every `cs.*`
//! reference must match a deterministic evaluation of the same inputs.
#![allow(clippy::expect_used)]

use crate::AllMetrics;
use finstack_quant_cashflows::builder::{AmortizationSpec, CouponType, RateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, PaymentPriority, WaterfallSpec};
use finstack_quant_statements::evaluator::{Evaluator, MonteCarloConfig};
use finstack_quant_statements::types::FinancialStatementInstrument;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, ForecastSpec};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use time::Month;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid quarter")
}

fn term_loan() -> FinancialStatementInstrument {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    FinancialStatementInstrument::TermLoan(
        TermLoan::builder()
            .id("TL-001".into())
            .currency(Currency::USD)
            .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money"))
            .issue_date(issue)
            .maturity(maturity)
            .rate(RateSpec::Fixed { rate: 0.05 })
            .frequency(Tenor::quarterly())
            .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
            .day_count(DayCount::Act360)
            .yield_basis(finstack_quant_valuations::metrics::returns::YieldBasis::Act360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(Default::default())
            .attributes(Default::default())
            .build()
            .expect("valid term loan"),
    )
}

/// A levered model whose EBITDA is either forecast (`Some(spec)`) after an
/// actual 2025Q1, or given explicitly for every quarter (`None` with
/// `explicit`). Cash equals EBITDA; half of EBITDA sweeps the term loan.
fn levered_model(
    forecast: Option<ForecastSpec>,
    explicit: &[(PeriodId, f64)],
) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("mc-cs");
    builder
        .periods("2025Q1..Q4", Some("2025Q1"))
        .expect("valid periods");
    let values: Vec<(PeriodId, AmountOrScalar)> = explicit
        .iter()
        .map(|(period, value)| (*period, AmountOrScalar::scalar(*value)))
        .collect();
    let mut builder = match forecast {
        Some(spec) => {
            let mut node = builder.mixed("ebitda");
            node.values(&values);
            node.forecast(spec);
            node.build().expect("valid mixed node")
        }
        None => {
            builder.value("ebitda", &values);
            builder
        }
    };
    builder.compute("cash", "ebitda").expect("valid formula");
    builder
        .compute("debt", "cs.debt_balance.total")
        .expect("valid formula");
    builder
        .compute("interest", "cs.interest_expense.total")
        .expect("valid formula");
    builder
        .compute(
            "coverage",
            "if(sign(cs.interest_expense.total) > 0, \
             cs.debt_balance.total / cs.interest_expense.total, 0)",
        )
        .expect("valid formula");
    builder.add_debt("TL-001", term_loan());
    builder.waterfall(WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".to_string(),
            deductions: vec![
                finstack_quant_statements::capital_structure::EcfTerm::PaidCashInterest,
                finstack_quant_statements::capital_structure::EcfTerm::PaidFees,
                finstack_quant_statements::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities:
                finstack_quant_statements::capital_structure::FacilitySelection::AllEligible,
            payment_lag_days: None,
        }),
        available_cash_node: "cash".into(),
        ..Default::default()
    });
    builder.build().expect("valid model")
}

const CS_NODES: [&str; 3] = ["debt", "interest", "coverage"];

#[test]
fn monte_carlo_runs_a_model_with_debt_and_varies_cs_nodes() {
    let model = levered_model(
        Some(ForecastSpec::normal(1_000_000.0, 300_000.0, 11)),
        &[(q(1), 1_000_000.0)],
    );
    let config = MonteCarloConfig::new(64, 7, [""; 0]).with_percentiles(vec![0.05, 0.5, 0.95]);
    let first = Evaluator::new()
        .evaluate_all_metrics(&model, &config)
        .expect("Monte Carlo with a capital structure");
    let second = Evaluator::new()
        .evaluate_all_metrics(&model, &config)
        .expect("Monte Carlo with a capital structure");

    for node in CS_NODES {
        assert_eq!(
            first.percentile_results.get(node).map(|s| &s.values),
            second.percentile_results.get(node).map(|s| &s.values),
            "{node} is reproducible for one seed"
        );
    }
    // The 2025Q3 balance reflects the 2025Q2 sweep, which depends on the
    // sampled EBITDA, so its distribution has width.
    let p5 = first.percentile_by_period("debt", 0.05).expect("debt p5")[&q(3)];
    let p95 = first.percentile_by_period("debt", 0.95).expect("debt p95")[&q(3)];
    assert!(
        p95 > p5,
        "debt balance varies across paths: p5 {p5}, p95 {p95}"
    );
}

#[test]
fn a_monte_carlo_path_equals_deterministic_evaluation_of_its_samples() {
    let model = levered_model(
        Some(ForecastSpec::normal(1_000_000.0, 300_000.0, 11)),
        &[(q(1), 1_000_000.0)],
    );
    // One path: every percentile is that path's value.
    let path = Evaluator::new()
        .evaluate_all_metrics(
            &model,
            &MonteCarloConfig::new(1, 99, [""; 0]).with_percentiles(vec![0.5]),
        )
        .expect("Monte Carlo with a capital structure");
    let sampled = path.percentile_by_period("ebitda", 0.5).expect("ebitda");

    let mut explicit = vec![(q(1), 1_000_000.0)];
    explicit.extend(sampled.iter().map(|(period, value)| (*period, *value)));
    let deterministic = Evaluator::new()
        .evaluate(&levered_model(None, &explicit))
        .expect("deterministic evaluation");

    for node in CS_NODES {
        let simulated = path
            .percentile_by_period(node, 0.5)
            .expect("simulated node");
        assert_eq!(simulated.len(), 3, "{node} has every forecast quarter");
        for (period, value) in simulated {
            let expected = deterministic
                .get(node, &period)
                .expect("deterministic value");
            assert_eq!(value, expected, "{node} {period}");
        }
    }
}

#[test]
fn zero_variance_monte_carlo_equals_deterministic_evaluation() {
    let model = levered_model(
        Some(ForecastSpec::normal(1_200_000.0, 0.0, 11)),
        &[(q(1), 1_000_000.0)],
    );
    let mc = Evaluator::new()
        .evaluate_all_metrics(&model, &MonteCarloConfig::new(8, 3, [""; 0]))
        .expect("Monte Carlo with a capital structure");
    let deterministic = Evaluator::new()
        .evaluate(&model)
        .expect("deterministic evaluation");

    for node in ["ebitda", "debt", "interest", "coverage"] {
        for percentile in &mc.percentiles {
            let series = mc.percentile_by_period(node, *percentile).expect("series");
            for (period, value) in series {
                let expected = deterministic
                    .get(node, &period)
                    .expect("deterministic value");
                assert_eq!(value, expected, "{node} {period} p{percentile}");
            }
        }
    }
}

/// A floating-rate loan needs a market. The one Monte Carlo entry point takes
/// the same market, valuation date and cutoff as a market-aware evaluation,
/// and every path's debt service is the deterministic one: only the
/// statement forecast is random.
#[test]
fn monte_carlo_evaluates_floating_debt_on_the_supplied_market() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::{
        ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_statements::evaluator::{MonteCarloContext, ObservationCutoff};

    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let FinancialStatementInstrument::TermLoan(mut loan) = term_loan() else {
        unreachable!("fixture is a term loan");
    };
    loan.rate = TermLoan::example_floating_with_ddtl()
        .expect("floating loan")
        .rate;
    let model = {
        let mut builder = ModelBuilder::new("mc-floating");
        builder
            .periods("2025Q1..Q4", Some("2025Q1"))
            .expect("periods");
        let mut ebitda = builder.mixed("ebitda");
        ebitda.values(&[(q(1), AmountOrScalar::scalar(1_000_000.0))]);
        ebitda.forecast(ForecastSpec::normal(1_000_000.0, 100_000.0, 11));
        let mut builder = ebitda.build().expect("mixed node");
        builder.add_debt("TL-001", FinancialStatementInstrument::TermLoan(loan));
        builder
            .compute("interest", "cs.interest_expense_cash.total")
            .expect("capital reference");
        builder.build().expect("model")
    };
    let market = MarketContext::new()
        .insert(ForwardCurve::flat("USD-SOFR-3M", as_of, 0.03, DayCount::Act360).expect("curve"))
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                vec![(
                    Date::from_calendar_date(2024, Month::December, 30).expect("valid date"),
                    0.05,
                )],
                None,
                SeriesInterpolation::Step,
                SeriesExtrapolation::Error,
            )
            .expect("fixing history"),
        );
    let config = MonteCarloConfig::new(32, 7, ["ebitda", "interest"])
        .with_percentiles(vec![0.05, 0.5, 0.95]);

    // Without a market the floating coupon cannot be projected.
    Evaluator::new()
        .evaluate_monte_carlo(&model, &config, None)
        .expect_err("a floating loan needs a market");

    let context = MonteCarloContext {
        market: &market,
        as_of,
        cutoff: ObservationCutoff::FullSnapshot,
    };
    let results = Evaluator::new()
        .evaluate_monte_carlo(&model, &config, Some(context))
        .expect("market-aware Monte Carlo");
    assert!(results.incomplete.is_none());
    let deterministic = Evaluator::new()
        .evaluate_with_market(&model, &market, as_of, ObservationCutoff::FullSnapshot)
        .expect("deterministic evaluation");
    for quarter in 2..=4 {
        let expected = deterministic
            .get("interest", &q(quarter))
            .expect("deterministic interest");
        assert!(expected > 0.0, "Q{quarter} pays a floating coupon");
        // Debt service does not vary across paths: every level is the same.
        for level in [0.05, 0.5, 0.95] {
            let value = results
                .percentile_by_period("interest", level)
                .expect("interest")[&q(quarter)];
            assert!(
                (value - expected).abs() < 1e-6,
                "Q{quarter} p{level}: {value}"
            );
        }
    }
    // The statement forecast does vary.
    let low = results
        .percentile_by_period("ebitda", 0.05)
        .expect("ebitda")[&q(4)];
    let high = results
        .percentile_by_period("ebitda", 0.95)
        .expect("ebitda")[&q(4)];
    assert!(low < high);
    // The same seed and context replay the same result.
    let again = Evaluator::new()
        .evaluate_monte_carlo(&model, &config, Some(context))
        .expect("replay");
    assert_eq!(
        serde_json::to_value(&results).expect("serialize"),
        serde_json::to_value(&again).expect("serialize")
    );
}
