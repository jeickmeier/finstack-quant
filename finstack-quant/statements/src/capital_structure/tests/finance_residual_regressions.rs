//! Financial regressions for residual coupon and principal-date reconstruction.

use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_cashflows::builder::{AmortizationSpec, RateSpec};
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, PeriodId, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, PaymentPriority, WaterfallSpec};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use rust_decimal::Decimal;
use time::macros::date;

const INSTRUMENT: &str = "LOAN";

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("finite fixture amount")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected:.12}, got {actual:.12}"
    );
}

#[test]
fn model_preserves_economic_amortization_across_quarter_end_with_optional_sweep() {
    let issue = date!(2023 - 12 - 31);
    let maturity = date!(2024 - 12 - 31);
    let mut loan = TermLoan::example().expect("canonical term loan");
    loan.id = INSTRUMENT.into();
    loan.notional_limit = usd(100.0);
    loan.issue_date = issue;
    loan.maturity = maturity;
    loan.rate = RateSpec::Fixed { rate: 0.10 };
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act365F;
    loan.business_day_convention = BusinessDayConvention::Following;
    loan.calendar_id = Some("weekends_only".into());
    loan.amortization = AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.10 };
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.9)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(discount);
    let schedule = loan
        .raw_cashflow_schedule(&market, issue)
        .expect("loan cashflows");
    for (economic, settlement) in [
        (date!(2024 - 03 - 31), date!(2024 - 04 - 01)),
        (date!(2024 - 06 - 30), date!(2024 - 07 - 01)),
    ] {
        let amortization = schedule
            .get_flows()
            .iter()
            .find(|flow| {
                flow.get_kind() == CFKind::Amortization
                    && flow.get_balance_date().expect("principal row balance date") == economic
            })
            .expect("scheduled quarter-end principal movement");
        assert_eq!(amortization.get_date(), settlement);
        assert_close(amortization.get_amount().amount(), 10.0);
    }

    let quarters: Vec<_> = (1..=4)
        .map(|quarter| PeriodId::quarter(2024, quarter).expect("valid quarter"))
        .collect();
    let cash: Vec<_> = quarters
        .iter()
        .map(|period| (*period, AmountOrScalar::scalar(1_000.0)))
        .collect();
    let ebitda: Vec<_> = quarters
        .iter()
        .enumerate()
        .map(|(index, period)| {
            (
                *period,
                AmountOrScalar::scalar(if index == 0 { 40.0 } else { 0.0 }),
            )
        })
        .collect();
    for (waterfall, sweep) in [(false, false), (true, false), (true, true)] {
        let mut builder = {
            let mut model_builder = ModelBuilder::new("quarter-end-principal");
            model_builder
                .periods("2024Q1..2024Q4", None)
                .expect("reporting quarters");
            model_builder.value("cash", &cash);
            model_builder.value("ebitda", &ebitda);
            model_builder.add_debt(
                INSTRUMENT,
                FinancialStatementInstrument::TermLoan(loan.clone()),
            );
            model_builder
        };
        if sweep {
            builder.waterfall(WaterfallSpec {
                priority_of_payments: vec![
                    PaymentPriority::Fees,
                    PaymentPriority::Interest,
                    PaymentPriority::Amortization,
                    PaymentPriority::Sweep,
                    PaymentPriority::Equity,
                ],
                ecf_sweep: Some(EcfSweepSpec {
                    cash_measure: "ebitda".into(),
                    deductions: vec![
                        crate::capital_structure::EcfTerm::PaidCashInterest,
                        crate::capital_structure::EcfTerm::PaidFees,
                        crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                    ],
                    credits: Vec::new(),
                    sweep_percentage: 0.5,
                    facilities: crate::capital_structure::FacilitySelection::Named(vec![
                        INSTRUMENT.into(),
                    ]),
                }),
                ..Default::default()
            });
        } else if waterfall {
            builder.waterfall(WaterfallSpec::default());
        }
        let model = {
            let mut model_builder_1 = builder;
            model_builder_1
                .compute("balance", "cs.debt_balance.LOAN")
                .expect("balance formula");
            model_builder_1
                .compute("principal", "cs.principal_payment.LOAN")
                .expect("principal formula");
            model_builder_1
                .build()
                .expect("canonical amortizing-loan model")
        };
        let result = Evaluator::new()
            .evaluate_with_market(
                &model,
                &market,
                issue,
                crate::evaluator::ObservationCutoff::FullSnapshot,
            )
            .expect("amortizing-loan model evaluation");

        // March/June economic amortization belongs to Q1/Q2 respectively,
        // while its April/July cash settlement belongs to Q2/Q3. The Q1-only
        // sweep reduces face by another 20, with no second reduction when
        // April's contractual cash settles, and lowers the final redemption.
        let expected_balances = if sweep {
            [70.0, 60.0, 50.0, 0.0]
        } else {
            [90.0, 80.0, 70.0, 0.0]
        };
        let expected_principal = if sweep {
            [20.0, 10.0, 20.0, 50.0]
        } else {
            [0.0, 10.0, 20.0, 70.0]
        };
        let mut total_principal = 0.0;
        for (index, period) in quarters.iter().enumerate() {
            assert_close(
                result.get("balance", period).expect("quarterly balance"),
                expected_balances[index],
            );
            let paid = result
                .get("principal", period)
                .expect("quarterly principal cash");
            assert_close(paid, expected_principal[index]);
            total_principal += paid;
        }
        assert_close(total_principal, 100.0);
    }
}

/// A balance opened by this period's funding closes on the economic clock,
/// exactly like a balance carried in from the prior period.
#[test]
fn newly_funded_balance_closes_on_economic_principal_dates() {
    use crate::support::ScheduleInstrument;
    use finstack_quant_statements::capital_structure::calculate_period_flows;
    let periods = finstack_quant_core::dates::build_periods("2025Q1..2025Q2", None)
        .expect("valid periods")
        .periods;
    let market = MarketContext::new();
    for (economic, payment, expected_close) in [
        // Amortized in the quarter, settled after it: the balance is already lower.
        (date!(2025 - 03 - 31), date!(2025 - 04 - 02), 90.0),
        // Settled in the quarter, amortized after it: the balance is still whole.
        (date!(2025 - 04 - 02), date!(2025 - 03 - 28), 100.0),
        (date!(2025 - 03 - 15), date!(2025 - 03 - 15), 90.0),
    ] {
        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(usd(100.0), date!(2025 - 01 - 15), date!(2026 - 01 - 15))
            .add_principal_event(
                economic,
                payment,
                usd(-10.0),
                Some(usd(10.0)),
                CFKind::Amortization,
            );
        let instrument = ScheduleInstrument::new(builder.build(None).expect("schedule"));
        let (_, closing, funding, _) = calculate_period_flows(
            &instrument,
            &periods[0],
            usd(0.0),
            &market,
            periods[0].start,
            None,
        )
        .expect("newly funded period flows");
        assert_close(funding.amount(), 100.0);
        assert_close(closing.amount(), expected_close);
    }
}

/// A loan funded on or after the model start repays exactly what it borrowed
/// when quarter-end amortization settles on the next business day.
#[test]
fn loan_funded_inside_the_model_repays_its_face_once() {
    let as_of = date!(2023 - 12 - 31);
    let quarters: Vec<_> = (1..=4)
        .map(|quarter| PeriodId::quarter(2024, quarter).expect("valid quarter"))
        .collect();
    let cash: Vec<_> = quarters
        .iter()
        .map(|period| (*period, AmountOrScalar::scalar(1_000.0)))
        .collect();
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, 0.9)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(discount);
    for issue in [date!(2024 - 01 - 01), date!(2024 - 01 - 15)] {
        let mut loan = TermLoan::example().expect("canonical term loan");
        loan.id = INSTRUMENT.into();
        loan.notional_limit = usd(100.0);
        loan.issue_date = issue;
        loan.maturity = date!(2024 - 12 - 31);
        loan.rate = RateSpec::Fixed { rate: 0.10 };
        loan.frequency = Tenor::quarterly();
        loan.day_count = DayCount::Act365F;
        loan.stub = StubKind::ShortFront;
        loan.business_day_convention = BusinessDayConvention::Following;
        loan.calendar_id = Some("weekends_only".into());
        loan.amortization = AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.10 };
        let schedule = loan
            .raw_cashflow_schedule(&market, as_of)
            .expect("loan cashflows");
        let first = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.get_kind() == CFKind::Amortization)
            .expect("first amortization");
        assert_eq!(
            first
                .get_balance_date()
                .expect("principal row balance date"),
            date!(2024 - 03 - 31)
        );
        assert_eq!(first.get_date(), date!(2024 - 04 - 01));

        for waterfall in [false, true] {
            let mut builder = ModelBuilder::new("funded-inside-model");
            builder
                .periods("2024Q1..2024Q4", None)
                .expect("reporting quarters");
            builder.value("cash", &cash);
            builder.add_debt(
                INSTRUMENT,
                FinancialStatementInstrument::TermLoan(loan.clone()),
            );
            if waterfall {
                builder.waterfall(WaterfallSpec::default());
            }
            builder
                .compute("balance", "cs.debt_balance.LOAN")
                .expect("balance formula");
            builder
                .compute("principal", "cs.principal_payment.LOAN")
                .expect("principal formula");
            let model = builder.build().expect("amortizing-loan model");
            let result = Evaluator::new()
                .evaluate_with_market(
                    &model,
                    &market,
                    as_of,
                    crate::evaluator::ObservationCutoff::FullSnapshot,
                )
                .expect("amortizing-loan model evaluation");
            let mut total_principal = 0.0;
            for (period, (balance, principal)) in
                quarters
                    .iter()
                    .zip([(90.0, 0.0), (80.0, 10.0), (70.0, 20.0), (0.0, 70.0)])
            {
                assert_close(result.get("balance", period).expect("balance"), balance);
                let paid = result.get("principal", period).expect("principal cash");
                assert_close(paid, principal);
                total_principal += paid;
            }
            assert_close(total_principal, 100.0);
        }
    }
}

/// A mandatory prepayment under `all_eligible` is paid across a real term
/// loan and a real revolver through the evaluator, the rung's requested, paid
/// and unpaid amounts are recorded, and a sizing input that reaches a
/// same-period `cs` output through another node is rejected before the run.
#[test]
fn evaluator_prepays_a_term_loan_and_a_revolver_and_records_the_outcome() {
    use finstack_quant_statements::capital_structure::{FacilitySelection, PrepaymentSpec};
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
        DrawRepaySpec, RevolvingCredit,
    };

    let issue = date!(2023 - 12 - 31);
    let maturity = date!(2024 - 12 - 31);
    let mut loan = TermLoan::example().expect("canonical term loan");
    loan.id = INSTRUMENT.into();
    loan.notional_limit = usd(100.0);
    loan.issue_date = issue;
    loan.maturity = maturity;
    loan.rate = RateSpec::Fixed { rate: 0.10 };
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act365F;
    loan.business_day_convention = BusinessDayConvention::Following;
    loan.calendar_id = Some("weekends_only".into());
    loan.amortization = AmortizationSpec::None;
    let mut revolver = RevolvingCredit::example().expect("canonical revolver");
    revolver.id = "RCF".into();
    revolver.commitment = usd(200.0);
    revolver.drawn = usd(100.0);
    revolver.issue_date = issue;
    revolver.maturity = maturity;
    revolver.rate = RateSpec::Fixed { rate: 0.08 };
    revolver.draw_repay_spec = DrawRepaySpec::Deterministic(Vec::new());
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.9)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(discount);

    let quarters: Vec<_> = (1..=4)
        .map(|quarter| PeriodId::quarter(2024, quarter).expect("valid quarter"))
        .collect();
    let series = |amount: fn(usize) -> f64| -> Vec<_> {
        quarters
            .iter()
            .enumerate()
            .map(|(index, period)| (*period, AmountOrScalar::scalar(amount(index))))
            .collect()
    };
    let model = |amount: &str| {
        let mut builder = ModelBuilder::new("mixed-prepayment");
        builder
            .periods("2024Q1..2024Q4", None)
            .expect("reporting quarters");
        builder.value("cash", &series(|_| 1_000.0));
        builder.value(
            "request",
            &series(|index| if index == 0 { 60.0 } else { 0.0 }),
        );
        builder
            .compute("interest_alias", "cs.interest_expense_cash.total")
            .expect("alias of a waterfall output");
        builder.add_debt(
            INSTRUMENT,
            FinancialStatementInstrument::TermLoan(loan.clone()),
        );
        builder.add_debt(
            "RCF",
            FinancialStatementInstrument::RevolvingCredit(revolver.clone()),
        );
        builder.waterfall(WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::MandatoryPrepayment,
                PaymentPriority::Equity,
            ],
            mandatory_prepayment: Some(PrepaymentSpec {
                amount: amount.into(),
                facilities: FacilitySelection::AllEligible,
            }),
            ..Default::default()
        });
        builder.build()
    };
    let evaluate = |amount: &str| {
        Evaluator::new().evaluate_with_market(
            &model(amount).expect("mixed capital structure"),
            &market,
            issue,
            crate::evaluator::ObservationCutoff::FullSnapshot,
        )
    };

    let result = evaluate("request").expect("mixed prepayment evaluation");
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");
    let first = &cashflows.prepayments[&quarters[0]];
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].kind, PaymentPriority::MandatoryPrepayment);
    assert_eq!(first[0].requested, usd(60.0));
    assert_eq!(first[0].paid, usd(60.0));
    assert_eq!(first[0].unpaid, usd(0.0));
    // Equal principal, one class: the request is shared equally.
    for id in [INSTRUMENT, "RCF"] {
        assert_eq!(
            cashflows.by_instrument[id][&quarters[0]].mandatory_prepayment,
            Some(usd(30.0))
        );
        assert_close(
            cashflows
                .get_component("debt_balance", id, &quarters[0])
                .expect("closing balance"),
            70.0,
        );
    }
    assert_eq!(cashflows.prepayments[&quarters[1]][0].requested, usd(0.0));

    let error = evaluate("interest_alias")
        .expect_err("a request sized from this period's interest")
        .to_string();
    assert!(error.contains("mandatory_prepayment"), "{error}");
    assert!(error.contains("interest_alias"), "{error}");
}

// Residual coupon rebuilding through the instrument owner (STATEMENTS-CAPITAL-005)

/// A 10% loan of 100 on 30/360 with semiannual coupons, issued 1 January 2025.
fn semiannual_loan(coupon_type: CouponType) -> TermLoan {
    let mut loan = TermLoan::example().expect("canonical term loan");
    loan.id = INSTRUMENT.into();
    loan.notional_limit = usd(100.0);
    loan.issue_date = date!(2025 - 01 - 01);
    loan.maturity = date!(2026 - 01 - 01);
    loan.rate = RateSpec::Fixed { rate: 0.10 };
    loan.frequency = Tenor::semi_annual();
    loan.day_count = DayCount::Thirty360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.amortization = AmortizationSpec::None;
    loan.coupon_type = coupon_type;
    loan
}

fn prepay_then_equity(amount: &str) -> WaterfallSpec {
    WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Equity,
        ],
        mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: amount.into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        ..Default::default()
    }
}

/// Evaluate `periods` of 2025 with `instrument`, ample cash and a `request`
/// node that is `first` in the first period and zero afterwards.
fn evaluate_2025(
    periods: &str,
    instrument: FinancialStatementInstrument,
    waterfall: WaterfallSpec,
    first: f64,
    extra: &[(&str, f64)],
) -> crate::Result<crate::evaluator::StatementResult> {
    evaluate_2025_with(
        periods,
        instrument,
        waterfall,
        &|_| 1_000.0,
        &|index| if index == 0 { first } else { 0.0 },
        extra,
        &MarketContext::new(),
    )
}

/// As [`evaluate_2025`], with the cash pool and the request stated per
/// period (by index) and an explicit market.
fn evaluate_2025_with(
    periods: &str,
    instrument: FinancialStatementInstrument,
    waterfall: WaterfallSpec,
    cash: &dyn Fn(usize) -> f64,
    request: &dyn Fn(usize) -> f64,
    extra: &[(&str, f64)],
    market: &MarketContext,
) -> crate::Result<crate::evaluator::StatementResult> {
    let mut builder = ModelBuilder::new("residual-rebuild");
    builder.periods(periods, None).expect("periods");
    let model_periods: Vec<PeriodId> = if periods.contains('M') {
        (1..=6)
            .map(|month| PeriodId::month(2025, month).expect("month"))
            .collect()
    } else {
        (1..=4)
            .map(|quarter| PeriodId::quarter(2025, quarter).expect("quarter"))
            .collect()
    };
    let series = |value: &dyn Fn(usize) -> f64| -> Vec<_> {
        model_periods
            .iter()
            .enumerate()
            .map(|(index, period)| (*period, AmountOrScalar::scalar(value(index))))
            .collect()
    };
    builder.value("cash", &series(cash));
    builder.value("request", &series(request));
    for (name, value) in extra {
        builder.value(*name, &series(&|_| *value));
    }
    let id = match &instrument {
        FinancialStatementInstrument::RevolvingCredit(_) => "RCF",
        _ => INSTRUMENT,
    };
    builder.add_debt(id, instrument);
    builder.waterfall(waterfall);
    let model = builder.build()?;
    Evaluator::new().evaluate_with_market(
        &model,
        market,
        date!(2025 - 01 - 01),
        crate::evaluator::ObservationCutoff::FullSnapshot,
    )
}

fn component(
    result: &crate::evaluator::StatementResult,
    name: &str,
    id: &str,
    period: &str,
) -> f64 {
    let period: PeriodId = period.parse().expect("period id");
    result
        .cs_cashflows
        .as_ref()
        .expect("capital cash flows")
        .get_component(name, id, &period)
        .expect("component")
}

/// The record's example: 10% on 100 for an exact quarter and on 60 for the
/// next earns 2.50 and 1.50 inside one coupon, and the interest earned
/// before the sweep is preserved at each close.
#[test]
fn a_sweep_inside_a_coupon_earns_on_the_dated_principal() {
    let result = evaluate_2025(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(semiannual_loan(CouponType::Cash)),
        prepay_then_equity("request"),
        40.0,
        &[],
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    assert_close(get("debt_balance", "2025Q1"), 60.0);
    // Earned at the first close on 100, before the sweep takes effect.
    assert_close(get("accrued_interest", "2025Q1"), 2.5);
    // 2.50 on 100 and 89 of the second quarter's 90 days on 60.
    assert_close(get("accrued_interest", "2025Q2"), 2.5 + 1.5 * 89.0 / 90.0);
    // The 1 July coupon pays 2.50 + 1.50.
    assert_close(get("interest_expense_cash", "2025Q3"), 4.0);
    assert_close(get("debt_balance", "2025Q4"), 60.0);
}

/// The contractual 75% cash and 25% PIK split applies once to the rebuilt
/// coupon: 3 of cash and 1 capitalized on the coupon date.
#[test]
fn the_contractual_split_applies_once_to_a_rebuilt_coupon() {
    let result = evaluate_2025(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(semiannual_loan(CouponType::Split {
            cash_fraction: Decimal::new(75, 2),
            pik_fraction: Decimal::new(25, 2),
        })),
        prepay_then_equity("request"),
        40.0,
        &[],
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    assert_close(get("interest_expense_cash", "2025Q3"), 3.0);
    assert_close(get("interest_expense_pik", "2025Q3"), 1.0);
    // Scheduled cash service excludes the capitalized share and the sweep.
    assert_close(get("scheduled_cash_interest_due", "2025Q3"), 3.0);
    assert_close(get("scheduled_principal_due", "2025Q1"), 0.0);
    assert_close(get("debt_balance", "2025Q2"), 60.0);
    assert_close(get("debt_balance", "2025Q3"), 61.0);
}

fn pik_toggle(fraction: Decimal, min_coupon_periods: usize) -> WaterfallSpec {
    WaterfallSpec {
        pik_toggle: Some(crate::capital_structure::PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 100.0,
            elections: vec![crate::capital_structure::PikElectionSpec {
                instrument_id: INSTRUMENT.into(),
                pik_fraction: fraction,
                min_coupon_periods,
            }],
        }),
        ..Default::default()
    }
}

/// An 8% quarterly loan of 100 on 30/360: 2.00 a quarter.
fn quarterly_loan() -> TermLoan {
    let mut loan = semiannual_loan(CouponType::Cash);
    loan.frequency = Tenor::quarterly();
    loan.rate = RateSpec::Fixed { rate: 0.08 };
    loan
}

/// A PIK request takes effect at the start of the next coupon period, never
/// on a coupon already running, and capitalizes once on each coupon date.
#[test]
fn a_pik_election_starts_with_the_next_coupon_period() {
    let result = evaluate_2025(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(quarterly_loan()),
        pik_toggle(Decimal::ONE, 0),
        0.0,
        &[("liquidity", 10.0)],
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    // The first quarter's request covers coupon periods from 1 April. The
    // January to March coupon, paid on 1 April, stays cash.
    assert_close(get("interest_expense_cash", "2025Q2"), 2.0);
    assert_close(get("interest_expense_pik", "2025Q2"), 0.0);
    assert_close(get("debt_balance", "2025Q2"), 100.0);
    // The April to June coupon capitalizes on 1 July, the next on 1 October.
    assert_close(get("interest_expense_cash", "2025Q3"), 0.0);
    assert_close(get("interest_expense_pik", "2025Q3"), 2.0);
    assert_close(get("debt_balance", "2025Q3"), 102.0);
    assert_close(get("interest_expense_pik", "2025Q4"), 2.04);
    assert_close(get("debt_balance", "2025Q4"), 104.04);
}

/// A 200 commitment with 100 drawn at 8% on ACT/360, a 50 bp commitment fee
/// and a 25 bp usage fee, quarterly.
fn fee_revolver(
    events: Vec<
        finstack_quant_valuations::instruments::fixed_income::revolving_credit::DrawRepayEvent,
    >,
) -> FinancialStatementInstrument {
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
        DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
    };
    let mut facility = RevolvingCredit::example().expect("canonical revolver");
    facility.id = "RCF".into();
    facility.commitment = usd(200.0);
    facility.drawn = usd(100.0);
    facility.issue_date = date!(2025 - 01 - 01);
    facility.maturity = date!(2026 - 01 - 01);
    facility.rate = RateSpec::Fixed { rate: 0.08 };
    facility.day_count = DayCount::Act360;
    facility.frequency = Tenor::quarterly();
    facility.fees = RevolvingCreditFees::flat(50.0, 25.0, 0.0).expect("fees");
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(events);
    FinancialStatementInstrument::RevolvingCredit(facility)
}

/// A swept revolver's interest, usage fee and commitment fee follow the
/// drawn and undrawn balances from the day after the sweep, on actual days.
#[test]
fn a_swept_revolver_rebuilds_interest_and_fees_on_actual_days() {
    let result = evaluate_2025(
        "2025M01..2025M06",
        fee_revolver(Vec::new()),
        prepay_then_equity("request"),
        40.0,
        &[],
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, "RCF", period);
    assert_close(get("debt_balance", "2025M01"), 60.0);
    // The quarter's rows are paid in April: 31 days on 100 drawn, then 59 on
    // 60 drawn and 140 undrawn.
    let paid: f64 = ["2025M04"]
        .iter()
        .map(|period| get("interest_expense_cash", period))
        .sum();
    assert_close(paid, 0.08 * (100.0 * 31.0 + 60.0 * 59.0) / 360.0);
    assert_close(
        get("fees", "2025M04"),
        (0.005 * (100.0 * 31.0 + 140.0 * 59.0) + 0.0025 * (100.0 * 31.0 + 60.0 * 59.0)) / 360.0,
    );
    // Fees are not scheduled debt service.
    assert_close(get("scheduled_cash_interest_due", "2025M04"), paid);
    assert!(get("fees", "2025M04") > 0.0);
    assert_close(get("scheduled_principal_due", "2025M01"), 0.0);
    // Interest earned in January on 100 is preserved at the January close.
    assert_close(
        get("accrued_interest", "2025M01"),
        100.0 * 0.08 * 30.0 / 360.0,
    );
    // At the February close: 31 days on 100 and 27 on 60.
    assert_close(
        get("accrued_interest", "2025M02"),
        0.08 * (100.0 * 31.0 + 60.0 * 27.0) / 360.0,
    );
}

/// The facility's terms survive a zero balance: a contractual redraw after a
/// full sweep earns the facility's rate again.
#[test]
fn a_fully_swept_revolver_earns_again_on_a_contractual_redraw() {
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::DrawRepayEvent;
    let result = evaluate_2025(
        "2025Q1..2025Q4",
        fee_revolver(vec![DrawRepayEvent {
            date: date!(2025 - 07 - 01),
            amount: usd(80.0),
            is_draw: true,
        }]),
        prepay_then_equity("request"),
        100.0,
        &[],
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, "RCF", period);
    assert_close(get("debt_balance", "2025Q1"), 0.0);
    assert_close(get("debt_balance", "2025Q2"), 0.0);
    assert_close(get("debt_balance", "2025Q3"), 80.0);
    // Nothing was drawn in the second quarter; the third (92 days) earns 8%
    // on the 80 redrawn and is paid on 1 October.
    assert_close(get("interest_expense_cash", "2025Q3"), 0.0);
    assert_close(
        get("interest_expense_cash", "2025Q4"),
        80.0 * 0.08 * 92.0 / 360.0,
    );
}

/// Unpaid scheduled principal is carried as arrears while the schedule keeps
/// its dated reductions, and a later sweep is posted on top: the loan repays
/// its face exactly once.
#[test]
fn arrears_and_a_later_sweep_repay_the_face_once() {
    let mut loan = semiannual_loan(CouponType::Cash);
    loan.issue_date = date!(2024 - 12 - 31);
    loan.maturity = date!(2025 - 12 - 31);
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act365F;
    loan.business_day_convention = BusinessDayConvention::Following;
    loan.calendar_id = Some("weekends_only".into());
    loan.amortization = AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.10 };
    // No cash in the second quarter, when the 30 June installment falls due;
    // a sweep of 20 in the third.
    let result = evaluate_2025_with(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(loan),
        prepay_then_equity("request"),
        &|index| if index == 1 { 0.0 } else { 1_000.0 },
        &|index| if index == 2 { 20.0 } else { 0.0 },
        &[],
        &MarketContext::new(),
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    let quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"];
    let balances: Vec<f64> = quarters.iter().map(|q| get("debt_balance", q)).collect();
    let paid: Vec<f64> = quarters
        .iter()
        .map(|q| get("principal_payment", q))
        .collect();
    let arrears: Vec<f64> = quarters
        .iter()
        .map(|q| get("principal_arrears", q))
        .collect();
    // Economic principal follows the contract dates and the sweep.
    assert_eq!(balances, vec![90.0, 80.0, 50.0, 0.0], "{balances:?}");
    // The installment due on 30 June is unpaid in the second quarter and
    // settled with the 30 September installment and the sweep in the third.
    assert_eq!(arrears, vec![0.0, 10.0, 0.0, 0.0], "{arrears:?}");
    assert_eq!(paid[1], 0.0);
    assert_close(paid[2], 40.0);
    assert_close(paid.iter().sum::<f64>(), 100.0);

    // Scheduled service is recognized once, on its contractual due date
    // (STATEMENTS-CONTRACTS-008): the missed 30 June installment is due in
    // the second quarter although nothing is paid, and the third quarter's
    // due amount excludes both the arrears it settles and the sweep.
    let due: Vec<f64> = quarters
        .iter()
        .map(|q| get("scheduled_principal_due", q))
        .collect();
    assert_eq!(due, vec![10.0, 10.0, 10.0, 50.0], "{due:?}");
    // What falls due over the life is the face less the 20 swept.
    assert_close(due.iter().sum::<f64>(), 80.0);
    let interest_due: Vec<f64> = quarters
        .iter()
        .map(|q| get("scheduled_cash_interest_due", q))
        .collect();
    let interest_paid: Vec<f64> = quarters
        .iter()
        .map(|q| get("interest_expense_cash", q))
        .collect();
    // The 30 June coupon is due and unpaid; the third quarter pays it with
    // its own coupon but only its own coupon falls due then.
    assert!(interest_due[1] > 0.0 && interest_paid[1] == 0.0);
    assert_close(interest_paid[2], interest_due[1] + interest_due[2]);
}

#[test]
fn an_election_with_no_coupon_period_left_changes_nothing() {
    // The loan matures on 1 July: requests made in the third and fourth
    // quarters, after the full payoff, have no coupon period to cover.
    let mut loan = quarterly_loan();
    loan.maturity = date!(2025 - 07 - 01);
    let result = evaluate_2025_with(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(loan),
        pik_toggle(Decimal::ONE, 0),
        &|_| 1_000.0,
        &|_| 0.0,
        &[("liquidity", 10.0)],
        &MarketContext::new(),
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    // The first quarter's request covers the April to June coupon, which
    // capitalizes at maturity and is redeemed with the principal.
    assert_close(get("interest_expense_pik", "2025Q3"), 2.0);
    assert_close(get("principal_payment", "2025Q3"), 102.0);
    assert_close(get("debt_balance", "2025Q3"), 0.0);
    assert_close(get("debt_balance", "2025Q4"), 0.0);
    assert_close(get("interest_expense_pik", "2025Q4"), 0.0);
}

/// A floating coupon rebuilt after a sweep keeps its locked observation: the
/// 5% fixing set before the run, not the 3% forward curve, and only the
/// principal it earns on changes.
#[test]
fn a_floating_coupon_keeps_its_rate_when_rebuilt_after_a_sweep() {
    let mut loan = semiannual_loan(CouponType::Cash);
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act360;
    loan.rate = TermLoan::example_floating_with_ddtl()
        .expect("floating loan")
        .rate;
    let market = MarketContext::new().insert(
        finstack_quant_core::market_data::term_structures::ForwardCurve::flat(
            "USD-SOFR-3M",
            date!(2025 - 01 - 01),
            0.03,
            DayCount::Act360,
        )
        .expect("forward curve"),
    );
    use finstack_quant_core::market_data::scalars::{
        ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    let market = market.insert_series(
        ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(date!(2024 - 12 - 30), 0.05)],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("fixing history"),
    );
    let april_coupon = |swept: f64| {
        let result = evaluate_2025_with(
            "2025M01..2025M06",
            FinancialStatementInstrument::TermLoan(loan.clone()),
            prepay_then_equity("request"),
            &|_| 1_000.0,
            &|index| if index == 0 { swept } else { 0.0 },
            &[],
            &market,
        )
        .expect("evaluation");
        component(&result, "interest_expense_cash", INSTRUMENT, "2025M04")
    };
    let plain = april_coupon(0.0);
    // The locked 5% fixing plus the 400 bp margin for 90 days on 100.
    assert_close(plain, 100.0 * 0.09 * 90.0 / 360.0);
    // 31 days on 100 and 59 on 60, at the same projected rate.
    assert_close(
        april_coupon(40.0),
        plain * (100.0 * 31.0 + 60.0 * 59.0) / (100.0 * 90.0),
    );
}

// Contractual PIK elections (STATEMENTS-CAPITAL-008)

/// The record's example: a 10 coupon with a 40% PIK election pays 6 in cash
/// and capitalizes 4 on the coupon date. Overdue cash interest of 3 stays a
/// cash claim: 9 of cash interest is due and nothing more capitalizes.
#[test]
fn an_elected_split_capitalizes_its_share_and_leaves_arrears_in_cash() {
    // 40% on 100 with quarterly coupons: 10 a coupon.
    let mut loan = quarterly_loan();
    loan.rate = RateSpec::Fixed { rate: 0.40 };
    // Cash covers only 7 of the first coupon, paid on 1 April, so 3 is
    // overdue; the first quarter's request covers the coupon from 1 April.
    let result = evaluate_2025_with(
        "2025Q1..2025Q4",
        FinancialStatementInstrument::TermLoan(loan),
        pik_toggle(Decimal::new(4, 1), 0),
        &|index| if index == 1 { 7.0 } else { 1_000.0 },
        &|_| 0.0,
        &[("liquidity", 10.0)],
        &MarketContext::new(),
    )
    .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    assert_close(get("interest_expense_cash", "2025Q2"), 7.0);
    assert_close(get("interest_arrears", "2025Q2"), 3.0);
    assert_close(get("debt_balance", "2025Q2"), 100.0);
    // The elected coupon, paid on 1 July: 6 in cash plus the 3 overdue, and
    // 4 capitalized once.
    assert_close(get("interest_expense_cash", "2025Q3"), 9.0);
    assert_close(get("interest_expense_pik", "2025Q3"), 4.0);
    assert_close(get("interest_arrears", "2025Q3"), 0.0);
    assert_close(get("debt_balance", "2025Q3"), 104.0);
}

/// A PIK election lasts at least its minimum number of coupon periods,
/// counted in coupon periods of the loan and not in reporting periods.
#[test]
fn a_pik_election_lasts_its_minimum_number_of_coupon_periods() {
    // Liquidity is short in January only, in a monthly model of a
    // quarterly-coupon loan.
    let run = |min_coupon_periods: usize| {
        let mut builder_periods = Vec::new();
        for month in 1..=12 {
            builder_periods.push(PeriodId::month(2025, month).expect("month"));
        }
        let mut loan = quarterly_loan();
        loan.maturity = date!(2026 - 01 - 01);
        let mut builder = ModelBuilder::new("pik-duration");
        builder.periods("2025M01..2025M12", None).expect("periods");
        let series = |value: &dyn Fn(usize) -> f64| -> Vec<_> {
            builder_periods
                .iter()
                .enumerate()
                .map(|(index, period)| (*period, AmountOrScalar::scalar(value(index))))
                .collect()
        };
        builder.value("cash", &series(&|_| 1_000.0));
        builder.value(
            "liquidity",
            &series(&|index| if index == 0 { 10.0 } else { 500.0 }),
        );
        builder.add_debt(INSTRUMENT, FinancialStatementInstrument::TermLoan(loan));
        builder.waterfall(pik_toggle(Decimal::ONE, min_coupon_periods));
        let model = builder.build().expect("model");
        let result = Evaluator::new()
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                date!(2025 - 01 - 01),
                crate::evaluator::ObservationCutoff::FullSnapshot,
            )
            .expect("evaluation");
        // PIK capitalized on the 1 July, 1 October and 1 January coupons.
        ["2025M07", "2025M10"]
            .map(|period| component(&result, "interest_expense_pik", INSTRUMENT, period))
    };
    // January's request covers the coupon period from 1 April. February's
    // request for cash ends it at once when no minimum applies: the election
    // is replaced before its first period begins.
    assert_eq!(run(0), [0.0, 0.0]);
    // With a minimum of one coupon period the April to June coupon is PIK
    // and the next is cash; with two, both are PIK.
    assert_eq!(run(1), [2.0, 0.0]);
    let two = run(2);
    assert_close(two[0], 2.0);
    assert_close(two[1], 2.04);
}

/// Actual periods are not exercised: a historical election is recorded on
/// the loan itself.
#[test]
fn the_toggle_is_not_exercised_in_actual_periods() {
    let mut builder = ModelBuilder::new("pik-actuals");
    builder
        .periods("2025Q1..2025Q4", Some("2025Q2"))
        .expect("periods");
    let quarters: Vec<PeriodId> = (1..=4)
        .map(|quarter| PeriodId::quarter(2025, quarter).expect("quarter"))
        .collect();
    let series = |value: f64| -> Vec<_> {
        quarters
            .iter()
            .map(|period| (*period, AmountOrScalar::scalar(value)))
            .collect()
    };
    builder.value("cash", &series(1_000.0));
    builder.value("liquidity", &series(10.0));
    let mut loan = quarterly_loan();
    loan.maturity = date!(2026 - 04 - 01);
    builder.add_debt(INSTRUMENT, FinancialStatementInstrument::TermLoan(loan));
    builder.waterfall(pik_toggle(Decimal::ONE, 0));
    let model = builder.build().expect("model");
    let result = Evaluator::new()
        .evaluate_with_market(
            &model,
            &MarketContext::new(),
            date!(2025 - 01 - 01),
            crate::evaluator::ObservationCutoff::FullSnapshot,
        )
        .expect("evaluation");
    let get = |name: &str, period: &str| component(&result, name, INSTRUMENT, period);
    // The first forecast period is the third quarter; its request covers
    // coupon periods from 1 October, so nothing capitalizes in 2025 except
    // nothing at all: the 1 October coupon was earned before the election.
    for period in ["2025Q2", "2025Q3", "2025Q4"] {
        assert_close(get("interest_expense_pik", period), 0.0);
        assert_close(get("interest_expense_cash", period), 2.0);
    }
    assert_close(get("debt_balance", "2025Q4"), 100.0);
}
