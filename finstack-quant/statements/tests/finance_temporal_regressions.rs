//! Calendar offsets, annual growth, and parser-produced cashflow references.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::dates::{FiscalConfig, Period, build_fiscal_periods, build_periods};
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::dsl::parse_and_compile;
use finstack_quant_statements::evaluator::EvaluationContext;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::evaluator::evaluate_formula;
use finstack_quant_statements::evaluator::{
    CellStatus, EvalWarning, StatementResult, UnavailableReason,
};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec};
use indexmap::IndexMap;
use std::sync::Arc;

/// The plan of these fixtures: fiscal ids use the October 1 opening every
/// fiscal fixture in this file is built with; unprefixed ids are Gregorian.
fn fixture_plan(periods: Vec<Period>) -> finstack_quant_core::dates::PeriodPlan {
    let calendar = if periods.first().is_some_and(|period| period.id.is_fiscal()) {
        finstack_quant_core::dates::PeriodCalendar::Fiscal(
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
        )
    } else {
        finstack_quant_core::dates::PeriodCalendar::Gregorian
    };
    finstack_quant_core::dates::PeriodPlan::try_new(calendar, periods).expect("valid period plan")
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

fn offset_model(periods: Vec<Period>, observations: &[f64]) -> FinancialModelSpec {
    let values: Vec<_> = periods
        .iter()
        .zip(observations)
        .map(|(period, value)| (period.id, AmountOrScalar::scalar(*value)))
        .collect();
    {
        let mut model_builder = ModelBuilder::new("fiscal-offsets");
        model_builder
            .periods_explicit(fixture_plan(periods))
            .unwrap();
        model_builder.value("x", &values);
        model_builder.compute("prior", "lag(x,1)").unwrap();
        model_builder.compute("shifted", "shift(x,1)").unwrap();
        model_builder.compute("difference", "diff(x)").unwrap();
        model_builder.compute("change", "pct_change(x)").unwrap();
        model_builder
            .compute("nested", "lag(rolling_mean(x,2),1)")
            .unwrap();
        model_builder.build().unwrap()
    }
}

#[test]
fn fiscal_weekly_offsets_include_the_short_final_week() {
    let periods = build_fiscal_periods(
        "FY2024W52..FY2025W2",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
    .unwrap()
    .periods;
    assert_eq!(periods[1].id.index(), 53);
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

    fixture_plan(vec![periods[0].clone(), periods[2].clone()])
        .validate_canonical()
        .expect_err("a plan that skips the short final week is not canonical");
}

#[test]
fn fiscal_daily_offsets_cross_leap_and_nonleap_fiscal_years() {
    for (range, expected_final_ordinal) in
        [("FY2024D365..FY2025D2", 366), ("FY2025D364..FY2026D2", 365)]
    {
        let periods = build_fiscal_periods(
            range,
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
            None,
        )
        .unwrap()
        .periods;
        assert_eq!(periods[1].id.index(), expected_final_ordinal);
        assert_eq!(periods[2].id.index(), 1);
        let model = offset_model(periods.clone(), &[100.0, 110.0, 121.0, 133.1]);
        let result = Evaluator::new().evaluate(&model).unwrap();
        close(result.get("prior", &periods[1].id).unwrap(), 100.0);
        close(result.get("prior", &periods[2].id).unwrap(), 110.0);
        close(result.get("difference", &periods[2].id).unwrap(), 11.0);
        close(result.get("change", &periods[2].id).unwrap(), 0.1);
        close(result.get("nested", &periods[2].id).unwrap(), 105.0);

        fixture_plan(vec![periods[0].clone(), periods[2].clone()])
            .validate_canonical()
            .expect_err("a plan that skips a fiscal day is not canonical");
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
        let model = {
            let mut model_builder = ModelBuilder::new("annual-growth");
            model_builder
                .periods_explicit(fixture_plan(periods.clone()))
                .unwrap();
            model_builder.value("x", &values);
            model_builder
                .compute("one_year", format!("growth_rate(x,{frequency})"))
                .unwrap();
            model_builder
                .compute("one_period", "growth_rate(x,1)")
                .unwrap();
            model_builder
                .compute("expression", "growth_rate(x * 2,2)")
                .unwrap();
            model_builder.build().unwrap()
        };
        let result = Evaluator::new().evaluate(&model).unwrap();
        let final_period = periods.last().unwrap().id;
        for node in ["one_year", "one_period", "expression"] {
            close(result.get(node, &final_period).unwrap(), 0.4641);
        }
    }
}

#[test]
fn daily_and_weekly_growth_use_actual_365_fixed_between_period_end_dates() {
    let fiscal_weekly = build_fiscal_periods(
        "FY2024W52..FY2025W1",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
    .unwrap()
    .periods;
    let fiscal_daily = build_fiscal_periods(
        "FY2024D366..FY2025D1",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
    .unwrap()
    .periods;
    let calendar_daily = build_periods("2023D365..2024D2", None).unwrap().periods;
    for periods in [fiscal_weekly, fiscal_daily, calendar_daily] {
        let origin = periods[0].end.previous_day().unwrap();
        let values: Vec<_> = periods
            .iter()
            .map(|period| {
                // Actual/365 Fixed, computed here from the day count alone.
                let days = (period.end.previous_day().unwrap() - origin).whole_days();
                let years = days as f64 / 365.0;
                (
                    period.id,
                    AmountOrScalar::scalar(100.0 * 1.2_f64.powf(years)),
                )
            })
            .collect();
        let model = {
            let mut model_builder = ModelBuilder::new("dated-growth");
            model_builder
                .periods_explicit(fixture_plan(periods.clone()))
                .unwrap();
            model_builder.value("x", &values);
            model_builder.compute("growth", "growth_rate(x,1)").unwrap();
            model_builder.build().unwrap()
        };
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
        &finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            Vec::new(),
        )
        .expect("empty plan"),
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
        &finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            Vec::new(),
        )
        .expect("empty plan"),
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

#[test]
fn ytd_starts_at_the_first_fiscal_period_of_the_fiscal_year() {
    let periods = build_fiscal_periods(
        "FY2024Q3..FY2025Q2",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
    .unwrap()
    .periods;
    let values: Vec<_> = periods
        .iter()
        .zip([1.0, 2.0, 4.0, 8.0])
        .map(|(period, value)| (period.id, AmountOrScalar::scalar(value)))
        .collect();
    let model = {
        let mut model_builder = ModelBuilder::new("fiscal-ytd");
        model_builder
            .periods_explicit(fixture_plan(periods.clone()))
            .unwrap();
        model_builder.value("x", &values);
        model_builder.compute("to_date", "ytd(x)").unwrap();
        model_builder.build().unwrap()
    };
    let result = Evaluator::new().evaluate(&model).unwrap();
    // The plan starts in FY2024Q3, so the FY2024 windows lack Q1 and Q2 and
    // are unavailable; a partial sum would not be a year-to-date figure.
    for index in [0, 1] {
        assert_eq!(result.get("to_date", &periods[index].id), None);
    }
    for (index, expected) in [(2, 4.0), (3, 12.0)] {
        close(result.get("to_date", &periods[index].id).unwrap(), expected);
    }
}

#[test]
fn daily_ttm_sums_twelve_calendar_months_of_calendar_and_business_days() {
    let all_days = build_periods("2024D1..2025D365", None).unwrap().periods;
    for business_only in [false, true] {
        // The plan is every calendar day; a business-day model leaves the
        // weekend observations out.
        let values: Vec<_> = all_days
            .iter()
            .filter(|p| !business_only || p.start.weekday().number_days_from_monday() < 5)
            .map(|period| (period.id, AmountOrScalar::scalar(1.0)))
            .collect();
        let mut model_builder = ModelBuilder::new("daily-ttm");
        model_builder
            .periods_explicit(fixture_plan(all_days.clone()))
            .unwrap();
        model_builder.value("x", &values);
        model_builder.compute("ltm", "ttm(x)").unwrap();
        let result = Evaluator::new()
            .evaluate(&model_builder.build().unwrap())
            .unwrap();
        // A calendar-day model sums 2025's 365 days. Business-day
        // observations are not full calendar-day coverage (absent cells do
        // not prove weekends or holidays), so the window is incomplete
        // (record CORE-CONVENTIONS-014 #11).
        let year_end = PeriodId::day(2025, 365).unwrap();
        let ltm = result.get("ltm", &year_end);
        if business_only {
            assert_eq!(ltm, None, "business-day LTM must be incomplete");
        } else {
            close(ltm.unwrap(), 365.0);
        }
        // 2024-12-30 has one uncovered day (2023-12-31) in its window.
        assert!(
            result
                .get("ltm", &PeriodId::day(2024, 365).unwrap())
                .is_none()
        );
    }
}

/// A model holds one period kind in one calendar context: `PeriodId` order is
/// structural, not a chronology across kinds (record CORE-CONVENTIONS-014 #8),
/// so a timeline that changes frequency mid-year is rejected at build time.
#[test]
fn model_rejects_periods_of_mixed_kinds() {
    use finstack_quant_core::dates::build_periods;
    for (fine, coarse) in [
        ("2025M01..2025M06", "2025Q3..2025Q4"),
        ("2025M01..2025M06", "2025H2..2025H2"),
        ("2025Q1..2025Q2", "2025M07..2025M12"),
    ] {
        let mut periods = build_periods(fine, None).unwrap().periods;
        periods.extend(build_periods(coarse, None).unwrap().periods);
        let mut model_builder = ModelBuilder::new("mixed-frequency");
        let err = match model_builder
            .periods_explicit(fixture_plan(periods))
            .map(|_| ())
        {
            Err(err) => err,
            Ok(()) => model_builder
                .build()
                .expect_err("mixed period kinds must be rejected"),
        };
        assert!(err.to_string().contains("one period kind"), "{err}");
    }
}

/// Weekly TTM is not a twelve-month window: 52 weeks is 364 days and a
/// fiscal week 53 may be short (record CORE-CONVENTIONS-014 #11), so a
/// weekly model's `ttm` is an evaluation error rather than a value.
#[test]
fn weekly_ttm_is_an_evaluation_error() {
    for plan in [
        build_periods("2024W1..2025W10", None).unwrap(),
        build_fiscal_periods(
            "FY2024W1..FY2025W10",
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
            None,
        )
        .unwrap(),
    ] {
        let values: Vec<_> = plan
            .periods
            .iter()
            .map(|period| (period.id, AmountOrScalar::scalar(1.0)))
            .collect();
        let mut model_builder = ModelBuilder::new("weekly-ttm");
        model_builder.periods_explicit(plan).unwrap();
        model_builder.value("x", &values);
        model_builder.compute("trailing", "ttm(x)").unwrap();
        let err = Evaluator::new()
            .evaluate(&model_builder.build().unwrap())
            .expect_err("weekly TTM is rejected");
        assert!(err.to_string().contains("weekly"), "{err}");
    }
}

// STATEMENTS-EVALUATION-003 qualification: exact offsets, signed changes,
// the growth domain and complete reporting windows.

fn quarter(index: u8) -> PeriodId {
    PeriodId::quarter(2025, index).unwrap()
}

fn usd(amount: &str) -> Money {
    Money::from_decimal_str(amount, Currency::USD).unwrap()
}

/// Quarterly USD cash of 100.10, 0, -50 and 200.20, with the named formulas.
fn cash_model(formulas: &[(&str, &str)]) -> finstack_quant_statements::Result<StatementResult> {
    let mut builder = ModelBuilder::new("offsets");
    builder.periods("2025Q1..Q4", None)?;
    let cash: Vec<_> = ["100.10", "0", "-50", "200.20"]
        .iter()
        .zip(1..)
        .map(|(amount, index)| (quarter(index), usd(amount)))
        .collect();
    builder.value_money("cash", &cash);
    builder.value_scalar("steps", &[(quarter(1), -1.0), (quarter(2), 0.5)]);
    for (node, formula) in formulas {
        builder.compute(*node, *formula)?;
    }
    Evaluator::new().evaluate(&builder.build()?)
}

fn reason(result: &StatementResult, node: &str, period: PeriodId) -> Option<UnavailableReason> {
    let cell = result.get_cell(node, &period).expect("cell");
    assert_eq!(cell.status, CellStatus::Unavailable, "{node} {period}");
    cell.reason
}

#[test]
fn offsets_read_exact_typed_history() {
    let result = cash_model(&[
        ("previous", "lag(cash, 1)"),
        ("two_back", "shift(cash, 2)"),
        ("same", "lag(cash, 0)"),
        ("stacked", "lag(cash, 1) + cash"),
        ("change", "diff(cash)"),
    ])
    .unwrap();
    assert_eq!(
        result.get_money("previous", &quarter(2)),
        Some(usd("100.10"))
    );
    assert_eq!(
        result.get_money("two_back", &quarter(3)),
        Some(usd("100.10"))
    );
    assert_eq!(result.get_money("same", &quarter(4)), Some(usd("200.20")));
    assert_eq!(
        result.get_money("stacked", &quarter(4)),
        Some(usd("150.20"))
    );
    // Current minus base, in the input's units.
    assert_eq!(result.get_money("change", &quarter(4)), Some(usd("250.20")));
    assert_eq!(result.get_money("change", &quarter(3)), Some(usd("-50")));
    // Missing history is unavailable, not an error and not zero.
    for node in ["previous", "stacked", "change"] {
        assert_eq!(
            reason(&result, node, quarter(1)),
            Some(UnavailableReason::MissingInput)
        );
    }
    assert_eq!(
        reason(&result, "two_back", quarter(2)),
        Some(UnavailableReason::MissingInput)
    );
}

#[test]
fn fractional_and_future_offsets_are_errors() {
    for (formula, cause) in [
        ("lag(cash, steps)", "non-negative integer"),
        ("shift(cash, steps)", "future period"),
    ] {
        // `steps` is -1 in Q1.
        let error = cash_model(&[("bad", formula)]).expect_err(formula);
        assert!(error.to_string().contains(cause), "{formula}: {error}");
    }
    // `steps` is 0.5 in Q2; Q1 must pass first, so read it only from Q2 on.
    let error = cash_model(&[("bad", "if(steps > 0, diff(cash, steps), cash)")])
        .expect_err("fractional offset");
    assert!(error.to_string().contains("integer"), "{error}");
}

#[test]
fn pct_change_is_signed_and_a_zero_base_is_unavailable() {
    let result = cash_model(&[("change", "pct_change(cash)")]).unwrap();
    // Q2: (0 - 100.10) / 100.10.
    close(result.get("change", &quarter(2)).unwrap(), -1.0);
    // Q4: (200.20 - (-50)) / (-50), signed by the negative base.
    close(result.get("change", &quarter(4)).unwrap(), -5.004);
    // Q3 is measured from an exactly zero base.
    assert_eq!(
        reason(&result, "change", quarter(3)),
        Some(UnavailableReason::UndefinedBase)
    );
    assert!(result.meta.warnings.iter().any(|warning| matches!(
        warning,
        EvalWarning::DivisionByZero { node_id, period } if node_id == "change" && *period == quarter(3)
    )));
    assert_eq!(
        reason(&result, "change", quarter(1)),
        Some(UnavailableReason::MissingInput)
    );
}

#[test]
fn growth_rate_needs_a_positive_base_and_a_non_negative_ending_value() {
    let annual = |values: [f64; 3]| {
        let mut builder = ModelBuilder::new("growth");
        builder.periods("2023..2025", None).unwrap();
        let ids = [2023, 2024, 2025].map(|year| PeriodId::annual(year).unwrap());
        builder.value_scalar(
            "x",
            &[
                (ids[0], values[0]),
                (ids[1], values[1]),
                (ids[2], values[2]),
            ],
        );
        builder.compute("one", "growth_rate(x, 1)").unwrap();
        builder.compute("two", "growth_rate(x, 2)").unwrap();
        (
            Evaluator::new()
                .evaluate(&builder.build().unwrap())
                .unwrap(),
            ids,
        )
    };
    let (result, ids) = annual([100.0, 121.0, 0.0]);
    close(result.get("one", &ids[1]).unwrap(), 0.21);
    // A zero ending value is -100%, over one year or two.
    close(result.get("one", &ids[2]).unwrap(), -1.0);
    close(result.get("two", &ids[2]).unwrap(), -1.0);
    assert_eq!(
        reason(&result, "one", ids[0]),
        Some(UnavailableReason::MissingInput)
    );

    let (result, ids) = annual([-100.0, 0.0, -5.0]);
    // A negative base, a zero base and a negative ending value have no
    // compound growth rate.
    for (node, period) in [("one", ids[1]), ("one", ids[2]), ("two", ids[2])] {
        assert_eq!(
            reason(&result, node, period),
            Some(UnavailableReason::UndefinedBase),
            "{node} {period}"
        );
    }

    let error = {
        let mut builder = ModelBuilder::new("growth");
        builder.periods("2023..2025", None).unwrap();
        builder.value_scalar("x", &[(PeriodId::annual(2023).unwrap(), 1.0)]);
        builder
            .compute("zero", "growth_rate(x, lag(x, 0) - lag(x, 0))")
            .unwrap();
        Evaluator::new()
            .evaluate(&builder.build().unwrap())
            .expect_err("a zero lookback")
    };
    assert!(error.to_string().contains("positive integer"), "{error}");
}

#[test]
fn reporting_windows_need_every_slot() {
    let month = |index: u8| PeriodId::month(2025, index).unwrap();
    let mut builder = ModelBuilder::new("windows");
    builder.periods("2025M01..2025M06", None).unwrap();
    // March has no observation: an interior gap.
    let sales: Vec<_> = [1, 2, 4, 5, 6]
        .map(|index| (month(index), usd(&format!("{index}.01"))))
        .to_vec();
    builder.value_money("sales", &sales);
    builder.compute("year", "ytd(sales)").unwrap();
    builder.compute("quarter", "qtd(sales)").unwrap();
    builder.compute("fiscal", "fiscal_ytd(sales, 4)").unwrap();
    let result = Evaluator::new()
        .evaluate(&builder.build().unwrap())
        .unwrap();

    // Exact money sums over complete windows.
    assert_eq!(result.get_money("year", &month(2)), Some(usd("3.02")));
    assert_eq!(result.get_money("quarter", &month(6)), Some(usd("15.03")));
    assert_eq!(result.get_money("fiscal", &month(5)), Some(usd("9.02")));
    // A window that contains the March gap is unavailable.
    for (node, index) in [("year", 3), ("year", 6), ("quarter", 3), ("fiscal", 3)] {
        assert_eq!(
            reason(&result, node, month(index)),
            Some(UnavailableReason::MissingInput),
            "{node} M{index}"
        );
    }
}

#[test]
fn month_to_quarter_windows_follow_the_fiscal_quarter() {
    let periods = build_fiscal_periods(
        "FY2025M01..FY2025M06",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
    .unwrap()
    .periods;
    let values: Vec<_> = periods
        .iter()
        .zip(1..)
        .map(|(period, value)| (period.id, AmountOrScalar::scalar(f64::from(value))))
        .collect();
    let build = |formula: &str| {
        let mut builder = ModelBuilder::new("fiscal-months");
        builder
            .periods_explicit(fixture_plan(periods.clone()))
            .unwrap();
        builder.value("x", &values);
        builder.compute("window", formula).unwrap();
        Evaluator::new().evaluate(&builder.build().unwrap())
    };
    let result = build("qtd(x)").unwrap();
    // Fiscal month 4 opens the second fiscal quarter.
    for (index, expected) in [(0, 1.0), (2, 6.0), (3, 4.0), (5, 15.0)] {
        close(result.get("window", &periods[index].id).unwrap(), expected);
    }
    let result = build("ytd(x)").unwrap();
    close(result.get("window", &periods[5].id).unwrap(), 21.0);
    // A fiscal plan carries its own reporting year.
    let error = build("fiscal_ytd(x, 10)").expect_err("fiscal plan");
    assert!(
        error.to_string().contains("fiscal plan uses ytd()"),
        "{error}"
    );
}
