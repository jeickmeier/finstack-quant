//! Financial regressions for residual coupon and principal-date reconstruction.

use finstack_quant_cashflows::builder::{
    CashFlowMeta, CashFlowSchedule, CouponType, FixedCouponSpec, Notional, ScheduleParams,
};
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{
    CapitalStructureState, EcfSweepSpec, PrincipalClaim, WaterfallSpec,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
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

fn split_schedule(maturity: Date) -> CashFlowSchedule {
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(usd(100.0), date!(2025 - 01 - 01), maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Split {
                cash_fraction: Decimal::new(4, 1),
                pik_fraction: Decimal::new(6, 1),
            },
            rate: Decimal::new(1, 1),
            schedule: ScheduleParams {
                frequency: Tenor::annual(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Following,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: Default::default(),
            },
        });
    builder
        .build(None)
        .expect("canonical split-coupon schedule")
}

fn state_with_balance(schedule: CashFlowSchedule, balance: f64) -> CapitalStructureState {
    let mut state = CapitalStructureState::new();
    state.residual_schedules.insert(INSTRUMENT.into(), schedule);
    state
        .closing_balances
        .insert(INSTRUMENT.into(), usd(balance));
    state
}

fn amount_on(schedule: &CashFlowSchedule, kind: CFKind, payment_date: Date) -> f64 {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == kind && flow.date == payment_date)
        .map(|flow| flow.amount.amount())
        .sum()
}

fn assert_matures_to_zero(schedule: &CashFlowSchedule, maturity: Date) {
    let path = schedule
        .outstanding_by_date()
        .expect("valid principal path");
    let (last_date, last_balance) = path.last().expect("nonempty principal path");
    assert_eq!(*last_date, maturity);
    assert_close(last_balance.amount(), 0.0);
}

fn assert_repeated_rebuild_is_unchanged(state: &mut CapitalStructureState, from_date: Date) {
    let before = serde_json::to_value(&state.residual_schedules[INSTRUMENT])
        .expect("serializable residual schedule");
    state
        .rebuild_residuals(from_date)
        .expect("repeated residual rebuild");
    let after = serde_json::to_value(&state.residual_schedules[INSTRUMENT])
        .expect("serializable repeated schedule");
    assert_eq!(
        before, after,
        "rebuilding the same closing must be idempotent"
    );
}

#[test]
fn split_coupon_partial_paydown_preserves_shares_and_pik_compounding() {
    let first_coupon = date!(2026 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);
    let cutoff = date!(2025 - 06 - 30);
    let original = split_schedule(maturity);
    assert_close(amount_on(&original, CFKind::Fixed, first_coupon), 4.0);
    assert_close(amount_on(&original, CFKind::Pik, first_coupon), 6.0);

    let mut state = state_with_balance(original, 50.0);
    state
        .rebuild_residuals(cutoff)
        .expect("partial repayment rebuild");
    let rebuilt = &state.residual_schedules[INSTRUMENT];

    // Jan 1--Jul 1 is 181 days; Jul 1--Jan 1 is 184 days. The first
    // coupon retains earned interest and applies the smaller face thereafter.
    let first_interest = 0.10 * (100.0 * 181.0 + 50.0 * 184.0) / 365.0;
    assert_close(
        amount_on(rebuilt, CFKind::Fixed, first_coupon),
        first_interest * 0.4,
    );
    assert_close(
        amount_on(rebuilt, CFKind::Pik, first_coupon),
        first_interest * 0.6,
    );

    // The next annual coupon earns interest on remaining face plus only the
    // PIK component of the first coupon; its cash component never capitalizes.
    let second_year_face = 50.0 + first_interest * 0.6;
    let second_interest = second_year_face * 0.10;
    assert_close(
        amount_on(rebuilt, CFKind::Fixed, maturity),
        second_interest * 0.4,
    );
    assert_close(
        amount_on(rebuilt, CFKind::Pik, maturity),
        second_interest * 0.6,
    );
    assert_close(
        amount_on(rebuilt, CFKind::Notional, maturity),
        second_year_face + second_interest * 0.6,
    );
    assert_matures_to_zero(rebuilt, maturity);
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
}

#[test]
fn split_coupon_full_paydown_preserves_earned_cash_and_pik_interest() {
    let maturity = date!(2026 - 01 - 01);
    let cutoff = date!(2025 - 06 - 30);
    let mut state = state_with_balance(split_schedule(maturity), 0.0);
    state
        .rebuild_residuals(cutoff)
        .expect("full repayment rebuild");
    let rebuilt = &state.residual_schedules[INSTRUMENT];

    // Paying principal does not extinguish the 181 days of earned interest.
    // No interest accrues on the repaid face during the remaining 184 days.
    let earned_interest = 100.0 * 0.10 * 181.0 / 365.0;
    assert_close(
        amount_on(rebuilt, CFKind::Fixed, maturity),
        earned_interest * 0.4,
    );
    assert_close(
        amount_on(rebuilt, CFKind::Pik, maturity),
        earned_interest * 0.6,
    );
    assert_close(
        amount_on(rebuilt, CFKind::Notional, maturity),
        earned_interest * 0.6,
    );
    assert!(
        rebuilt
            .get_flows()
            .iter()
            .filter(|flow| matches!(flow.kind, CFKind::Fixed | CFKind::Pik))
            .all(|flow| flow.amount.amount() >= 0.0),
        "repayment must not create negative positive-rate coupons"
    );
    assert_matures_to_zero(rebuilt, maturity);
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
}

#[test]
fn zeroed_split_coupon_retains_component_rates_when_closing_is_revised() {
    let maturity = date!(2026 - 01 - 01);
    let cutoff = date!(2025 - 06 - 30);
    let effective = date!(2025 - 07 - 01);
    let mut state = state_with_balance(split_schedule(maturity), 0.0);
    state
        .rebuild_residuals(cutoff)
        .expect("full repayment rebuild");
    let zeroed = &state.residual_schedules[INSTRUMENT];
    for kind in [CFKind::Fixed, CFKind::Pik] {
        let future_component: f64 = zeroed
            .get_flows()
            .iter()
            .filter(|flow| {
                flow.kind == kind
                    && flow
                        .accrual
                        .as_ref()
                        .is_some_and(|accrual| accrual.start == effective)
            })
            .map(|flow| flow.amount.amount())
            .sum();
        assert_close(future_component, 0.0);
    }

    // Correct the same closing snapshot from a full payoff to a 75 repayment.
    // The zeroed rows must still carry their individual coupon sensitivity.
    state.closing_balances.insert(INSTRUMENT.into(), usd(25.0));
    state
        .rebuild_residuals(cutoff)
        .expect("a revised closing must restore each zeroed coupon component");
    let rebuilt = &state.residual_schedules[INSTRUMENT];
    let earned_interest = 100.0 * 0.10 * 181.0 / 365.0;
    let remaining_interest = 25.0 * 0.10 * 184.0 / 365.0;
    for (kind, share) in [(CFKind::Fixed, 0.4), (CFKind::Pik, 0.6)] {
        let earned_component: f64 = rebuilt
            .get_flows()
            .iter()
            .filter(|flow| {
                flow.kind == kind
                    && flow
                        .accrual
                        .as_ref()
                        .is_some_and(|accrual| accrual.end == effective)
            })
            .map(|flow| flow.amount.amount())
            .sum();
        assert_close(earned_component, earned_interest * share);
        assert_close(
            amount_on(rebuilt, kind, maturity),
            (earned_interest + remaining_interest) * share,
        );
    }
    assert_close(
        amount_on(rebuilt, CFKind::Notional, maturity),
        25.0 + (earned_interest + remaining_interest) * 0.6,
    );
    assert_matures_to_zero(rebuilt, maturity);
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
}

fn assert_separate_principal_and_settlement_dates(
    economic_date: Date,
    payment_date: Date,
    closing_at_cutoff: f64,
) {
    let cutoff = date!(2025 - 03 - 31);
    let maturity = date!(2026 - 01 - 01);
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(usd(100.0), date!(2025 - 01 - 01), maturity)
        .add_principal_event(
            economic_date,
            payment_date,
            usd(-10.0),
            Some(usd(10.0)),
            CFKind::Amortization,
        );
    let original = builder.build(None).expect("dated principal schedule");
    assert_close(amount_on(&original, CFKind::Notional, maturity), 90.0);
    let mut state = state_with_balance(original, closing_at_cutoff);
    state
        .rebuild_residuals(cutoff)
        .expect("dated principal rebuild");
    let rebuilt = &state.residual_schedules[INSTRUMENT];

    // Cash and economic movement each survive once, irrespective of which
    // side of the reporting cutoff contains the contractual settlement.
    let amortization: Vec<_> = rebuilt
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization)
        .collect();
    assert_eq!(amortization.len(), 1);
    assert_eq!(amortization[0].date, payment_date);
    assert_eq!(amortization[0].get_balance_date(), economic_date);
    assert_close(amortization[0].amount.amount(), 10.0);
    assert_close(
        amortization[0]
            .principal_delta
            .expect("explicit principal delta")
            .amount(),
        -10.0,
    );

    // Original face 100 less contractual amortization 10 less the additional
    // period-end repayment 20 leaves exactly 70 for maturity redemption.
    assert_close(amount_on(rebuilt, CFKind::Notional, maturity), 70.0);
    assert_matures_to_zero(rebuilt, maturity);
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
}

#[test]
fn principal_effective_before_later_settlement_is_not_replayed_twice() {
    assert_separate_principal_and_settlement_dates(
        date!(2025 - 03 - 31),
        date!(2025 - 04 - 02),
        70.0,
    );
}

#[test]
fn principal_effective_after_earlier_settlement_is_replayed_once() {
    assert_separate_principal_and_settlement_dates(
        date!(2025 - 04 - 02),
        date!(2025 - 03 - 28),
        80.0,
    );
}

#[test]
fn full_paydown_preserves_paid_cash_and_suppresses_its_later_principal_movement() {
    let cutoff = date!(2025 - 03 - 31);
    let payment_date = date!(2025 - 03 - 28);
    let economic_date = date!(2025 - 04 - 02);
    let maturity = date!(2026 - 01 - 01);
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(usd(100.0), date!(2025 - 01 - 01), maturity)
        .add_principal_event(
            economic_date,
            payment_date,
            usd(-10.0),
            Some(usd(10.0)),
            CFKind::Amortization,
        );
    let mut state = state_with_balance(
        builder
            .build(None)
            .expect("early-settled principal schedule"),
        0.0,
    );
    state
        .rebuild_residuals(cutoff)
        .expect("full repayment rebuild");
    let rebuilt = &state.residual_schedules[INSTRUMENT];
    let paid_amortization = rebuilt
        .get_flows()
        .iter()
        .find(|flow| flow.kind == CFKind::Amortization)
        .expect("historical cash payment remains in the schedule");
    assert_eq!(paid_amortization.date, payment_date);
    assert_eq!(paid_amortization.get_balance_date(), economic_date);
    assert_close(paid_amortization.amount.amount(), 10.0);
    assert_close(
        paid_amortization
            .principal_delta
            .expect("explicit capped principal delta")
            .amount(),
        0.0,
    );
    assert_close(amount_on(rebuilt, CFKind::Notional, maturity), 0.0);
    assert_matures_to_zero(rebuilt, maturity);
    assert!(rebuilt
        .outstanding_by_date()
        .expect("valid principal path")
        .iter()
        .all(|(_, balance)| balance.amount() >= 0.0));
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
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
            .find(|flow| flow.kind == CFKind::Amortization && flow.get_balance_date() == economic)
            .expect("scheduled quarter-end principal movement");
        assert_eq!(amortization.date, settlement);
        assert_close(amortization.amount.amount(), 10.0);
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
        let builder = ModelBuilder::new("quarter-end-principal")
            .periods("2024Q1..2024Q4", None)
            .expect("reporting quarters")
            .value("cash", &cash)
            .value("ebitda", &ebitda)
            .add_debt(
                INSTRUMENT,
                FinancialStatementInstrument::TermLoan(loan.clone()),
            );
        let builder = if waterfall {
            builder.waterfall(WaterfallSpec {
                ecf_sweep: sweep.then_some(EcfSweepSpec {
                    ebitda_node: "ebitda".into(),
                    taxes_node: None,
                    capex_node: None,
                    working_capital_node: None,
                    cash_interest_node: None,
                    sweep_percentage: 0.5,
                    target_instrument_id: Some(INSTRUMENT.into()),
                }),
                ..Default::default()
            })
        } else {
            builder
        };
        let model = builder
            .compute("balance", "cs.debt_balance.LOAN")
            .expect("balance formula")
            .compute("principal", "cs.principal_payment.LOAN")
            .expect("principal formula")
            .build()
            .expect("canonical amortizing-loan model");
        let result = Evaluator::new()
            .evaluate_with_market(&model, &market, issue)
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

#[test]
fn previously_paid_redemption_cannot_extinguish_later_pik_principal() {
    let issue = date!(2025 - 03 - 01);
    let payment_date = date!(2025 - 03 - 28);
    let cutoff = date!(2025 - 03 - 31);
    let economic_date = date!(2025 - 04 - 02);
    let schedule = CashFlowSchedule::from_parts(
        vec![
            CashFlow::new(payment_date, None, usd(100.0), CFKind::Notional, 0.0, None)
                .with_principal_delta(usd(-100.0))
                .with_principal_date(economic_date),
        ],
        Notional::par(100.0, Currency::USD).expect("initial principal"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    // Redemption cash of 100 was already paid. A subsequent PIK toggle adds
    // 10 to closing principal before that redemption becomes economic.
    let mut state = state_with_balance(schedule, 110.0);
    state.principal_advance_payments.insert(
        INSTRUMENT.into(),
        vec![PrincipalClaim {
            payment_date,
            balance_date: economic_date,
            amount: usd(100.0),
        }],
    );
    state
        .rebuild_residuals(cutoff)
        .expect("new PIK principal rebuild");
    let rebuilt = &state.residual_schedules[INSTRUMENT];
    let redemption = rebuilt
        .get_flows()
        .iter()
        .find(|flow| flow.kind == CFKind::Notional)
        .expect("historical redemption cash remains");
    assert_eq!(redemption.date, payment_date);
    assert_eq!(redemption.get_balance_date(), economic_date);
    assert_close(redemption.amount.amount(), 100.0);
    assert_close(
        redemption
            .principal_delta
            .expect("original economic repayment")
            .amount(),
        -100.0,
    );
    let path = rebuilt.outstanding_by_date().expect("principal path");
    let balance = path
        .iter()
        .find(|(date, _)| *date == economic_date)
        .expect("redemption economic date")
        .1;
    assert_close(balance.amount(), 10.0);
    assert_repeated_rebuild_is_unchanged(&mut state, cutoff);
}
