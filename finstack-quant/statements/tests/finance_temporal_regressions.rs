//! Calendar offsets, annual growth, and parser-produced cashflow references.

use finstack_quant_core::dates::{
    build_fiscal_periods, build_periods, DayCount, DayCountContext, FiscalConfig, Period,
};
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::dsl::parse_and_compile;
use finstack_quant_statements::evaluator::formula::evaluate_formula;
use finstack_quant_statements::evaluator::EvaluationContext;
use finstack_quant_statements::prelude::*;
use indexmap::IndexMap;
use std::sync::Arc;

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

fn offset_model(periods: Vec<Period>, observations: &[f64]) -> FinancialModelSpec {
    let values: Vec<_> = periods
        .iter()
        .zip(observations)
        .map(|(period, value)| (period.id, AmountOrScalar::scalar(*value)))
        .collect();
    ModelBuilder::new("fiscal-offsets")
        .periods_explicit(periods)
        .unwrap()
        .value("x", &values)
        .compute("prior", "lag(x,1)")
        .unwrap()
        .compute("shifted", "shift(x,1)")
        .unwrap()
        .compute("difference", "diff(x)")
        .unwrap()
        .compute("change", "pct_change(x)")
        .unwrap()
        .compute("nested", "lag(rolling_mean(x,2),1)")
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn fiscal_weekly_offsets_include_the_short_final_week() {
    let periods = build_fiscal_periods("FY2024W52..FY2025W2", FiscalConfig::us_federal(), None)
        .unwrap()
        .periods;
    assert_eq!(periods[1].id.index, 53);
    assert_eq!((periods[1].end - periods[1].start).whole_days(), 2);
    let model = offset_model(periods.clone(), &[100.0, 110.0, 121.0, 133.1]);
    let result = Evaluator::new().evaluate(&model).unwrap();

    for (index, previous) in [(1, 100.0), (2, 110.0), (3, 121.0)] {
        close(result.get("prior", &periods[index].id).unwrap(), previous);
        close(result.get("shifted", &periods[index].id).unwrap(), previous);
        close(result.get("change", &periods[index].id).unwrap(), 0.1);
    }
    close(result.get("difference", &periods[2].id).unwrap(), 11.0);
    close(result.get("nested", &periods[2].id).unwrap(), 105.0);

    let sparse = offset_model(
        vec![periods[0].clone(), periods[2].clone()],
        &[100.0, 121.0],
    );
    let sparse_result = Evaluator::new().evaluate(&sparse).unwrap();
    assert!(sparse_result.get("prior", &periods[2].id).unwrap().is_nan());
}

#[test]
fn fiscal_daily_offsets_cross_leap_and_nonleap_fiscal_years() {
    for (range, expected_final_ordinal) in
        [("FY2024D365..FY2025D2", 366), ("FY2025D364..FY2026D2", 365)]
    {
        let periods = build_fiscal_periods(range, FiscalConfig::us_federal(), None)
            .unwrap()
            .periods;
        assert_eq!(periods[1].id.index, expected_final_ordinal);
        assert_eq!(periods[2].id.index, 1);
        let model = offset_model(periods.clone(), &[100.0, 110.0, 121.0, 133.1]);
        let result = Evaluator::new().evaluate(&model).unwrap();
        close(result.get("prior", &periods[1].id).unwrap(), 100.0);
        close(result.get("prior", &periods[2].id).unwrap(), 110.0);
        close(result.get("difference", &periods[2].id).unwrap(), 11.0);
        close(result.get("change", &periods[2].id).unwrap(), 0.1);
        close(result.get("nested", &periods[2].id).unwrap(), 105.0);

        let sparse = offset_model(
            vec![periods[0].clone(), periods[2].clone()],
            &[100.0, 121.0],
        );
        let sparse_result = Evaluator::new().evaluate(&sparse).unwrap();
        assert!(sparse_result.get("prior", &periods[2].id).unwrap().is_nan());
    }
}

#[test]
fn growth_rate_annualizes_quarterly_and_monthly_observations() {
    for (range, frequency) in [("2024Q1..2025Q1", 4), ("2024M1..2025M1", 12)] {
        let periods = build_periods(range, None).unwrap().periods;
        let values: Vec<_> = periods
            .iter()
            .enumerate()
            .map(|(index, period)| {
                (
                    period.id,
                    AmountOrScalar::scalar(
                        100.0 * 1.4641_f64.powf(index as f64 / frequency as f64),
                    ),
                )
            })
            .collect();
        let model = ModelBuilder::new("annual-growth")
            .periods_explicit(periods.clone())
            .unwrap()
            .value("x", &values)
            .compute("default", "growth_rate(x)")
            .unwrap()
            .compute("one_period", "growth_rate(x,1)")
            .unwrap()
            .compute("expression", "growth_rate(x * 2,2)")
            .unwrap()
            .build()
            .unwrap();
        let result = Evaluator::new().evaluate(&model).unwrap();
        let final_period = periods.last().unwrap().id;
        for node in ["default", "one_period", "expression"] {
            close(result.get(node, &final_period).unwrap(), 0.4641);
        }
    }
}

#[test]
fn daily_and_weekly_growth_use_elapsed_period_end_dates() {
    let fiscal_weekly =
        build_fiscal_periods("FY2024W52..FY2025W1", FiscalConfig::us_federal(), None)
            .unwrap()
            .periods;
    let fiscal_daily =
        build_fiscal_periods("FY2024D366..FY2025D1", FiscalConfig::us_federal(), None)
            .unwrap()
            .periods;
    let calendar_daily = build_periods("2023D365..2024D2", None).unwrap().periods;
    for periods in [fiscal_weekly, fiscal_daily, calendar_daily] {
        let origin = periods[0].end.previous_day().unwrap();
        let values: Vec<_> = periods
            .iter()
            .map(|period| {
                let years = DayCount::ActAct
                    .year_fraction(
                        origin,
                        period.end.previous_day().unwrap(),
                        DayCountContext::default(),
                    )
                    .unwrap();
                (
                    period.id,
                    AmountOrScalar::scalar(100.0 * 1.2_f64.powf(years)),
                )
            })
            .collect();
        let model = ModelBuilder::new("dated-growth")
            .periods_explicit(periods.clone())
            .unwrap()
            .value("x", &values)
            .compute("growth", "growth_rate(x,1)")
            .unwrap()
            .build()
            .unwrap();
        let result = Evaluator::new().evaluate(&model).unwrap();
        for period in periods.iter().skip(1) {
            close(result.get("growth", &period.id).unwrap(), 0.2);
        }
    }
}

fn snapshot(period: PeriodId, value: f64) -> CapitalStructureCashflows {
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::new(value, Currency::USD).unwrap();
    let mut snapshot = CapitalStructureCashflows::new();
    snapshot.reporting_currency = Some(Currency::USD);
    snapshot.totals.insert(period, breakdown.clone());
    snapshot
        .totals_by_currency
        .insert(Currency::USD, snapshot.totals.clone());
    snapshot
        .by_instrument
        .entry("LOAN".to_string())
        .or_default()
        .insert(period, breakdown);
    snapshot
}

#[test]
fn parser_produced_cs_references_work_in_historical_functions() {
    let periods = build_periods("2025M1..M6", None).unwrap().periods;
    let current = periods.last().unwrap().id;
    // A standalone capital-structure context need not also manufacture empty
    // statement rows for every cashflow snapshot.
    let mut context = EvaluationContext::new(
        current,
        Arc::new(IndexMap::new()),
        Arc::new(IndexMap::new()),
    );
    context.historical_capital_structure_cashflows = Arc::new(
        periods
            .iter()
            .take(5)
            .enumerate()
            .map(|(index, period)| (period.id, snapshot(period.id, (index + 1) as f64 * 10.0)))
            .collect(),
    );
    context.capital_structure_cashflows = Some(snapshot(current, 60.0));
    for reference in ["cs.interest_expense.total", "cs.interest_expense.LOAN"] {
        for (formula, expected) in [
            (format!("ytd({reference})"), 210.0),
            (format!("qtd({reference})"), 150.0),
            (format!("fiscal_ytd({reference},4)"), 150.0),
            (format!("lag({reference},1)"), 50.0),
            (format!("shift({reference},1)"), 50.0),
            (format!("diff({reference})"), 10.0),
            (format!("pct_change({reference})"), 0.2),
            (format!("growth_rate({reference},3)"), 15.0),
            (format!("rank({reference})"), 6.0),
            (format!("rank({reference},0)"), 1.0),
            (format!("quantile({reference},0.5)"), 35.0),
            (format!("rolling_mean({reference},2)"), 55.0),
            (format!("lag(rolling_mean({reference},2),1)"), 45.0),
            (format!("lag(rolling_mean({reference} * 2,2),1)"), 90.0),
        ] {
            let compiled = parse_and_compile(&formula).unwrap();
            let actual = evaluate_formula(&compiled, &mut context, Some("test")).unwrap();
            close(actual, expected);
        }
    }
    let missing = parse_and_compile("ytd(cs.interest_expense.MISSING)").unwrap();
    assert!(evaluate_formula(&missing, &mut context, Some("test")).is_err());
}

#[test]
fn cashflow_period_aggregates_resolve_only_the_requested_window() {
    let current = PeriodId::month(2025, 6).unwrap();
    let old = PeriodId::month(2024, 12).unwrap();
    let mut context = EvaluationContext::new(
        current,
        Arc::new(IndexMap::new()),
        Arc::new(IndexMap::new()),
    );
    let mut snapshots: IndexMap<_, _> = (1..=5)
        .map(|month| {
            let period = PeriodId::month(2025, month).unwrap();
            (period, snapshot(period, f64::from(month) * 10.0))
        })
        .collect();
    snapshots.insert(old, CapitalStructureCashflows::new());
    context.historical_capital_structure_cashflows = Arc::new(snapshots);
    context.capital_structure_cashflows = Some(snapshot(current, 60.0));

    for reference in ["cs.interest_expense.total", "cs.interest_expense.LOAN"] {
        for (formula, expected) in [
            (format!("ytd({reference})"), 210.0),
            (format!("qtd({reference})"), 150.0),
            (format!("fiscal_ytd({reference},4)"), 150.0),
        ] {
            let expression = parse_and_compile(&formula).unwrap();
            close(
                evaluate_formula(&expression, &mut context, None).unwrap(),
                expected,
            );
        }
    }

    // A missing claim in this year's first quarter remains outside QTD, but
    // must be diagnosed by YTD because that longer window includes it.
    Arc::make_mut(&mut context.historical_capital_structure_cashflows)
        .get_mut(&PeriodId::month(2025, 3).unwrap())
        .unwrap()
        .by_instrument
        .clear();
    let qtd = parse_and_compile("qtd(cs.interest_expense.LOAN)").unwrap();
    close(evaluate_formula(&qtd, &mut context, None).unwrap(), 150.0);
    let ytd = parse_and_compile("ytd(cs.interest_expense.LOAN)").unwrap();
    assert!(evaluate_formula(&ytd, &mut context, None).is_err());
}
