//! Monte Carlo evaluation of models with a capital structure.
//!
//! Each path runs the deterministic evaluator on its sampled forecasts, so the
//! waterfall (here a 50% ECF sweep into a term loan) and every `cs.*`
//! reference must match a deterministic evaluation of the same inputs.
#![allow(clippy::expect_used)]

use finstack_quant_cashflows::builder::{AmortizationSpec, CouponType, RateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, WaterfallSpec};
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
            "cs.debt_balance.total / cs.interest_expense.total",
        )
        .expect("valid formula");
    builder.add_debt("TL-001", term_loan());
    builder.waterfall(WaterfallSpec {
        ecf_sweep: Some(EcfSweepSpec {
            ebitda_node: "ebitda".to_string(),
            taxes_node: None,
            capex_node: None,
            working_capital_node: None,
            cash_interest_node: None,
            sweep_percentage: 0.5,
            target_instrument_id: None,
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
    let config = MonteCarloConfig::new(64, 7).with_percentiles(vec![0.05, 0.5, 0.95]);
    let first = Evaluator::new()
        .evaluate_monte_carlo(&model, &config)
        .expect("Monte Carlo with a capital structure");
    let second = Evaluator::new()
        .evaluate_monte_carlo(&model, &config)
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
        .evaluate_monte_carlo(
            &model,
            &MonteCarloConfig::new(1, 99).with_percentiles(vec![0.5]),
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
        .evaluate_monte_carlo(&model, &MonteCarloConfig::new(8, 3))
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
