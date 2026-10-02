//! Regression coverage for priority allocation, paid-cash ECF and liabilities.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_core::{
    cashflow::CashFlow,
    currency::Currency,
    dates::{Date, DayCount, Period, PeriodId, PeriodKind},
    money::Money,
};
use finstack_quant_statements::capital_structure::{
    execute_waterfall, CapitalStructureState, CashflowBreakdown, EcfSweepSpec, PaymentClassSpec,
    PaymentPriority, PikToggleSpec, PrincipalClaim, WaterfallSpec,
};
use finstack_quant_statements::{evaluator::EvaluationContext, types::NodeId};
use indexmap::IndexMap;
use std::sync::Arc;
use time::macros::date;

fn quarter() -> PeriodId {
    PeriodId::quarter(2025, 1).expect("valid period")
}

fn usd(amount: i64) -> Money {
    Money::from((amount, Currency::USD))
}

fn context(values: &[(&str, f64)]) -> EvaluationContext {
    context_for(quarter(), values)
}

fn context_for(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
    let columns = values
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (NodeId::new(*name), index))
        .collect();
    let mut context = EvaluationContext::new(period, Arc::new(columns), Arc::new(IndexMap::new()));
    for (name, value) in values {
        context.set_value(name, *value).expect("declared node");
    }
    context
}

fn dated_period(start: Date, end: Date) -> Period {
    Period {
        id: PeriodId::from_date(start, PeriodKind::Daily),
        start,
        end,
        is_actual: false,
    }
}

fn loans(
    ids: &[&str],
    balance: i64,
) -> (CapitalStructureState, IndexMap<String, CashflowBreakdown>) {
    let mut state = CapitalStructureState::new();
    let mut flows = IndexMap::new();
    for id in ids {
        state.opening_balances.insert((*id).into(), usd(balance));
        flows.insert(
            (*id).into(),
            CashflowBreakdown::with_currency(Currency::USD),
        );
    }
    (state, flows)
}

fn sweep(percentage: f64, target: Option<&str>) -> EcfSweepSpec {
    EcfSweepSpec {
        ebitda_node: "ebitda".into(),
        taxes_node: None,
        capex_node: None,
        working_capital_node: None,
        cash_interest_node: None,
        sweep_percentage: percentage,
        target_instrument_id: target.map(str::to_owned),
    }
}

fn payment_classes() -> Vec<PaymentClassSpec> {
    vec![
        PaymentClassSpec {
            id: "first-lien".into(),
            rank: 0,
            instrument_ids: vec!["senior".into()],
        },
        PaymentClassSpec {
            id: "second-lien".into(),
            rank: 1,
            instrument_ids: vec!["junior".into()],
        },
    ]
}

#[test]
fn prepayment_rungs_follow_configured_order_and_class_rank() {
    for (first, second, target, paid_id) in [
        (
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Sweep,
            "senior",
            "senior",
        ),
        (
            PaymentPriority::Sweep,
            PaymentPriority::MandatoryPrepayment,
            "junior",
            "junior",
        ),
    ] {
        let (mut state, flows) = loans(&["senior", "junior"], 100);
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                first,
                second,
                PaymentPriority::Equity,
            ],
            ecf_sweep: Some(sweep(1.0, Some(target))),
            mandatory_prepay_node: Some("mandatory".into()),
            payment_classes: payment_classes(),
            ..Default::default()
        };
        let result = execute_waterfall(
            &quarter(),
            &context(&[("cash", 100.0), ("ebitda", 100.0), ("mandatory", 100.0)]),
            &spec,
            &mut state,
            &flows,
        )
        .expect("ordered waterfall");

        for id in ["senior", "junior"] {
            let expected = if id == paid_id { 100.0 } else { 0.0 };
            assert_eq!(result.flows[id].principal_payment.amount(), expected);
        }
        assert_eq!(result.equity_distribution.expect("residual").amount(), 0.0);
    }
}

#[test]
fn sweep_before_amortization_consumes_senior_capacity_without_phantom_arrears() {
    let (mut state, mut flows) = loans(&["senior", "junior"], 100);
    flows.get_mut("senior").expect("senior").principal_payment = usd(100);
    let spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Sweep,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(sweep(1.0, None)),
        payment_classes: payment_classes(),
        ..Default::default()
    };
    let result = execute_waterfall(
        &quarter(),
        &context(&[("cash", 100.0), ("ebitda", 100.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("sweep before amortization");

    assert_eq!(result.flows["senior"].principal_payment.amount(), 100.0);
    assert_eq!(result.flows["junior"].principal_payment.amount(), 0.0);
    assert!(state.principal_shortfall.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn ecf_deducts_cash_interest_fees_and_scheduled_arrears_actually_paid() {
    let (mut state, mut flows) = loans(&["loan"], 1_000);
    let loan = flows.get_mut("loan").expect("loan");
    loan.interest_expense_cash = usd(10);
    loan.fees = usd(5);
    loan.principal_payment = usd(10);
    state.interest_shortfall.insert("loan".into(), usd(90));
    state.fee_shortfall.insert("loan".into(), usd(15));
    state.principal_shortfall.insert(
        "loan".into(),
        vec![PrincipalClaim {
            payment_date: date!(2024 - 12 - 31),
            balance_date: date!(2024 - 12 - 31),
            amount: usd(20),
        }],
    );
    let spec = WaterfallSpec {
        ecf_sweep: Some(sweep(0.5, None)),
        ..Default::default()
    };
    let result = execute_waterfall(
        &quarter(),
        &context(&[("cash", 300.0), ("ebitda", 300.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("arrears funded before sweep");

    let loan = &result.flows["loan"];
    assert_eq!(loan.interest_expense_cash.amount(), 100.0);
    assert_eq!(loan.fees.amount(), 20.0);
    // ECF = 300 - 100 interest - 20 fees - 30 amortization = 150.
    // Principal = 30 scheduled + 75 sweep; the other 75 remains for equity.
    assert_eq!(loan.principal_payment.amount(), 105.0);
    assert_eq!(result.equity_distribution.expect("residual").amount(), 75.0);
    assert!(state.interest_shortfall.is_empty());
    assert!(state.fee_shortfall.is_empty());
    assert!(state.principal_shortfall.is_empty());
}

#[test]
fn ecf_does_not_reserve_cash_for_a_later_interest_priority() {
    let (mut state, mut flows) = loans(&["loan"], 1_000);
    flows.get_mut("loan").expect("loan").interest_expense_cash = usd(100);
    let spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Sweep,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(sweep(0.5, None)),
        ..Default::default()
    };
    let result = execute_waterfall(
        &quarter(),
        &context(&[("cash", 200.0), ("ebitda", 200.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("sweep precedes interest");
    assert_eq!(result.flows["loan"].principal_payment.amount(), 100.0);
    assert_eq!(result.flows["loan"].interest_expense_cash.amount(), 100.0);
    assert_eq!(result.equity_distribution.expect("residual").amount(), 0.0);
}

#[test]
fn full_principal_repayment_preserves_earned_unpaid_interest() {
    let (mut state, mut flows) = loans(&["loan"], 1_000);
    flows.get_mut("loan").expect("loan").accrued_interest = usd(25);
    let spec = WaterfallSpec {
        ecf_sweep: Some(sweep(1.0, None)),
        ..Default::default()
    };
    let result = execute_waterfall(
        &quarter(),
        &context(&[("cash", 1_000.0), ("ebitda", 1_000.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("full principal repayment");
    let loan = &result.flows["loan"];
    assert_eq!(loan.debt_balance.amount(), 0.0);
    assert_eq!(loan.principal_payment.amount(), 1_000.0);
    assert_eq!(loan.interest_expense_cash.amount(), 0.0);
    assert_eq!(loan.interest_expense_pik.amount(), 0.0);
    assert_eq!(loan.accrued_interest.amount(), 25.0);
}

#[test]
fn pik_preserves_earned_accrual_in_coupon_and_noncoupon_periods() {
    for due_coupon in [0, 50] {
        let (mut state, mut flows) = loans(&["loan"], 1_000);
        let loan = flows.get_mut("loan").expect("loan");
        loan.accrued_interest = usd(25);
        loan.interest_expense_cash = usd(due_coupon);
        let spec = WaterfallSpec {
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "cash".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["loan".into()]),
                min_periods_in_pik: 0,
            }),
            ..Default::default()
        };
        let result = execute_waterfall(
            &quarter(),
            &context(&[("cash", 0.0)]),
            &spec,
            &mut state,
            &flows,
        )
        .expect("PIK coupon allocation");
        let loan = &result.flows["loan"];
        assert_eq!(loan.debt_balance, usd(1_000 + due_coupon));
        assert_eq!(loan.interest_expense_cash.amount(), 0.0);
        assert_eq!(loan.interest_expense_pik, usd(due_coupon));
        assert_eq!(loan.accrued_interest.amount(), 25.0);
    }
}

#[test]
fn shortfall_categories_cannot_collide_with_instrument_ids() {
    for ids in [
        ["A", "fees::A", "principal::A"],
        ["principal::A", "fees::A", "A"],
    ] {
        let (mut state, mut flows) = loans(&ids, 1_000);
        let a = flows.get_mut("A").expect("A");
        a.fees = usd(10);
        a.principal_payment = usd(30);
        flows
            .get_mut("fees::A")
            .expect("namespaced id")
            .interest_expense_cash = usd(20);
        flows
            .get_mut("principal::A")
            .expect("namespaced id")
            .interest_expense_cash = usd(40);
        let result = execute_waterfall(
            &quarter(),
            &context(&[("cash", 0.0)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("unrestricted instrument ids");

        assert_eq!(state.fee_shortfall["A"].amount(), 10.0);
        assert_eq!(
            state.principal_shortfall["A"]
                .iter()
                .map(|claim| claim.amount.amount())
                .sum::<f64>(),
            30.0
        );
        assert_eq!(state.interest_shortfall["fees::A"].amount(), 20.0);
        assert_eq!(state.interest_shortfall["principal::A"].amount(), 40.0);
        assert_eq!(result.flows["fees::A"].accrued_interest.amount(), 20.0);
        assert_eq!(result.flows["principal::A"].accrued_interest.amount(), 40.0);
    }
}

#[test]
fn delayed_principal_settlement_does_not_reduce_economic_balance_twice() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let economic_period = dated_period(date!(2025 - 03 - 31), date!(2025 - 04 - 01));
    flows.get_mut("loan").expect("loan").debt_balance = usd(90);
    state
        .set_period_principal_flows("loan", &economic_period, Vec::new())
        .expect("economic movement without cash");
    let first = execute_waterfall(
        &economic_period.id,
        &context_for(economic_period.id, &[("cash", 0.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("principal date");
    assert_eq!(first.flows["loan"].debt_balance.amount(), 90.0);
    assert_eq!(first.flows["loan"].principal_payment.amount(), 0.0);

    state.advance_period();
    let cash_period = dated_period(date!(2025 - 04 - 01), date!(2025 - 04 - 02));
    flows.get_mut("loan").expect("loan").principal_payment = usd(10);
    state
        .set_period_principal_flows(
            "loan",
            &cash_period,
            vec![PrincipalClaim {
                payment_date: date!(2025 - 04 - 01),
                balance_date: date!(2025 - 03 - 31),
                amount: usd(10),
            }],
        )
        .expect("delayed cash claim");
    let second = execute_waterfall(
        &cash_period.id,
        &context_for(cash_period.id, &[("cash", 10.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("cash settlement");
    assert_eq!(second.flows["loan"].debt_balance.amount(), 90.0);
    assert_eq!(second.flows["loan"].principal_payment.amount(), 10.0);
}

#[test]
fn delayed_settlement_remains_payable_after_economic_principal_is_swept() {
    let (mut state, mut flows) = loans(&["loan"], 90);
    let period = dated_period(date!(2025 - 04 - 01), date!(2025 - 04 - 02));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(90);
    loan.principal_payment = usd(10);
    state
        .set_period_principal_flows(
            "loan",
            &period,
            vec![PrincipalClaim {
                payment_date: date!(2025 - 04 - 01),
                balance_date: date!(2025 - 03 - 31),
                amount: usd(10),
            }],
        )
        .expect("delayed cash claim");
    let spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Sweep,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(sweep(1.0, None)),
        ..Default::default()
    };
    let result = execute_waterfall(
        &period.id,
        &context_for(period.id, &[("cash", 100.0), ("ebitda", 100.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("prepay economic balance then settle dated claim");
    assert_eq!(result.flows["loan"].principal_payment.amount(), 100.0);
    assert_eq!(result.flows["loan"].debt_balance.amount(), 0.0);
    assert_eq!(result.equity_distribution.expect("residual").amount(), 0.0);
    assert!(state.principal_shortfall.is_empty());
}

#[test]
fn early_unpaid_principal_is_carried_without_creating_debt_before_its_economic_date() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let cash_period = dated_period(date!(2025 - 03 - 28), date!(2025 - 04 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(100);
    loan.principal_payment = usd(10);
    state
        .set_period_principal_flows(
            "loan",
            &cash_period,
            vec![PrincipalClaim {
                payment_date: date!(2025 - 03 - 28),
                balance_date: date!(2025 - 04 - 02),
                amount: usd(10),
            }],
        )
        .expect("early cash claim");
    let first = execute_waterfall(
        &cash_period.id,
        &context_for(cash_period.id, &[("cash", 0.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("unpaid early cash claim");
    assert_eq!(first.flows["loan"].debt_balance.amount(), 100.0);
    assert_eq!(state.principal_shortfall["loan"][0].amount, usd(10));

    state.advance_period();
    let economic_period = dated_period(date!(2025 - 04 - 01), date!(2025 - 04 - 03));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(90);
    loan.principal_payment = usd(0);
    state
        .set_period_principal_flows("loan", &economic_period, Vec::new())
        .expect("economic date of carried claim");
    let second = execute_waterfall(
        &economic_period.id,
        &context_for(economic_period.id, &[("cash", 10.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("settle carried claim after economic date");
    assert_eq!(second.flows["loan"].debt_balance.amount(), 90.0);
    assert_eq!(second.flows["loan"].principal_payment.amount(), 10.0);
    assert!(state.principal_shortfall.is_empty());
}

#[test]
fn contractual_pik_follows_its_economic_date_without_double_capitalization() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let economic_period = dated_period(date!(2025 - 03 - 31), date!(2025 - 04 - 01));
    flows.get_mut("loan").expect("loan").debt_balance = usd(110);
    state
        .set_period_principal_flows("loan", &economic_period, Vec::new())
        .expect("PIK economic date");
    let first = execute_waterfall(
        &economic_period.id,
        &context_for(economic_period.id, &[("cash", 0.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("economic PIK capitalization");
    assert_eq!(first.flows["loan"].debt_balance.amount(), 110.0);

    state.advance_period();
    let payment_period = dated_period(date!(2025 - 04 - 01), date!(2025 - 04 - 02));
    flows.get_mut("loan").expect("loan").interest_expense_pik = usd(10);
    state
        .set_period_principal_flows("loan", &payment_period, Vec::new())
        .expect("later PIK reporting date");
    let second = execute_waterfall(
        &payment_period.id,
        &context_for(payment_period.id, &[("cash", 0.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("PIK already capitalized");
    assert_eq!(second.flows["loan"].debt_balance.amount(), 110.0);
    assert_eq!(second.flows["loan"].interest_expense_pik.amount(), 10.0);
}

#[test]
fn advance_principal_cannot_be_repaid_again_before_its_economic_date() {
    for initial_cash in [5, 10] {
        let (mut state, mut flows) = loans(&["loan"], 100);
        let first_period = dated_period(date!(2025 - 03 - 28), date!(2025 - 04 - 01));
        let loan = flows.get_mut("loan").expect("loan");
        loan.debt_balance = usd(100);
        loan.principal_payment = usd(10);
        state
            .set_period_principal_flows(
                "loan",
                &first_period,
                vec![PrincipalClaim {
                    payment_date: date!(2025 - 03 - 28),
                    balance_date: date!(2025 - 04 - 02),
                    amount: usd(10),
                }],
            )
            .expect("advance settlement");
        let first = execute_waterfall(
            &first_period.id,
            &context_for(first_period.id, &[("cash", initial_cash as f64)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("partially or fully paid advance principal");
        assert_eq!(first.flows["loan"].debt_balance.amount(), 100.0);
        assert_eq!(first.flows["loan"].principal_payment, usd(initial_cash));

        state.advance_period();
        let middle_period = dated_period(date!(2025 - 04 - 01), date!(2025 - 04 - 02));
        flows.get_mut("loan").expect("loan").principal_payment = usd(0);
        state
            .set_period_principal_flows("loan", &middle_period, Vec::new())
            .expect("economic date still in the future");
        let spec = WaterfallSpec {
            ecf_sweep: Some(sweep(1.0, None)),
            ..Default::default()
        };
        let middle = execute_waterfall(
            &middle_period.id,
            &context_for(middle_period.id, &[("cash", 100.0), ("ebitda", 100.0)]),
            &spec,
            &mut state,
            &flows,
        )
        .expect("sweep only principal not already settled in cash");
        assert_eq!(
            middle.flows["loan"].principal_payment,
            usd(100 - initial_cash)
        );
        assert_eq!(middle.flows["loan"].debt_balance.amount(), 10.0);
        assert_eq!(
            middle.equity_distribution.expect("residual"),
            usd(initial_cash)
        );
        assert!(state.principal_shortfall.is_empty());

        state.advance_period();
        let final_period = dated_period(date!(2025 - 04 - 02), date!(2025 - 04 - 03));
        flows.get_mut("loan").expect("loan").debt_balance = usd(0);
        state
            .set_period_principal_flows("loan", &final_period, Vec::new())
            .expect("economic date of already settled principal");
        let last = execute_waterfall(
            &final_period.id,
            &context_for(final_period.id, &[("cash", 100.0), ("ebitda", 100.0)]),
            &spec,
            &mut state,
            &flows,
        )
        .expect("retire the advance-payment timing item");
        assert_eq!(last.flows["loan"].principal_payment.amount(), 0.0);
        assert_eq!(last.flows["loan"].debt_balance.amount(), 0.0);
        assert_eq!(state.cumulative_principal["loan"].amount(), 100.0);
        assert!(state.principal_advance_payments.is_empty());
    }
}

#[test]
fn residual_rebuild_reserves_due_principal_ahead_of_future_installments() {
    for (initial_cash, sweep_first) in [(100, false), (75, true)] {
        let (mut state, mut flows) = loans(&["loan"], 100);
        let schedule = CashFlowSchedule::from_parts(
            vec![
                CashFlow::new(
                    date!(2025 - 04 - 04),
                    None,
                    usd(50),
                    CFKind::Amortization,
                    0.0,
                    None,
                )
                .with_principal_date(date!(2025 - 04 - 02)),
                CashFlow::new(
                    date!(2025 - 03 - 31),
                    None,
                    usd(50),
                    CFKind::Amortization,
                    0.0,
                    None,
                )
                .with_principal_date(date!(2025 - 04 - 03)),
            ],
            Notional::par(100.0, Currency::USD).expect("principal"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(date!(2025 - 01 - 01)),
                ..Default::default()
            },
        );
        state.residual_schedules.insert("loan".into(), schedule);
        let first_period = dated_period(date!(2025 - 03 - 31), date!(2025 - 04 - 01));
        let loan = flows.get_mut("loan").expect("loan");
        loan.debt_balance = usd(100);
        loan.principal_payment = usd(50);
        state
            .set_period_principal_flows(
                "loan",
                &first_period,
                vec![PrincipalClaim {
                    payment_date: date!(2025 - 03 - 31),
                    balance_date: date!(2025 - 04 - 03),
                    amount: usd(50),
                }],
            )
            .expect("advance is economically later than the other installment");
        let mut spec = WaterfallSpec {
            ecf_sweep: Some(sweep(1.0, None)),
            ..Default::default()
        };
        if sweep_first {
            spec.priority_of_payments = vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ];
        }
        let ebitda = if sweep_first { 50.0 } else { 100.0 };
        let first = execute_waterfall(
            &first_period.id,
            &context_for(
                first_period.id,
                &[("cash", initial_cash as f64), ("ebitda", ebitda)],
            ),
            &spec,
            &mut state,
            &flows,
        )
        .expect("settle advance and prepay remaining debt");
        assert_eq!(first.flows["loan"].principal_payment, usd(initial_cash));
        assert_eq!(first.flows["loan"].debt_balance.amount(), 50.0);
        state
            .rebuild_residuals(date!(2025 - 03 - 31))
            .expect("rebuild preserves already settled economic principal");

        let schedule = &state.residual_schedules["loan"];
        let path = schedule.outstanding_by_date().expect("economic path");
        let balance_before_advance = path
            .iter()
            .rfind(|(date, _)| *date <= date!(2025 - 04 - 02))
            .expect("balance before the paid advance becomes effective")
            .1;
        assert_eq!(balance_before_advance.amount(), 50.0);
        assert_eq!(
            path.last().expect("final principal movement").1.amount(),
            0.0
        );
        assert_eq!(
            schedule
                .get_flows()
                .iter()
                .find(|flow| flow.date == date!(2025 - 04 - 04))
                .expect("future installment row")
                .amount
                .amount(),
            0.0,
            "the unpaid installment is extinguished by the prepayment"
        );

        state.advance_period();
        let middle_period = dated_period(date!(2025 - 04 - 02), date!(2025 - 04 - 03));
        let loan = flows.get_mut("loan").expect("loan");
        loan.debt_balance = balance_before_advance;
        loan.principal_payment = usd(0);
        state
            .set_period_principal_flows("loan", &middle_period, Vec::new())
            .expect("no cash due before the retained advance date");
        let middle = execute_waterfall(
            &middle_period.id,
            &context_for(middle_period.id, &[("cash", 0.0)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("reserved advance reconciles with rebuilt economic principal");
        assert_eq!(middle.flows["loan"].debt_balance.amount(), 50.0);
        assert_eq!(middle.flows["loan"].principal_payment.amount(), 0.0);
        assert_eq!(state.cumulative_principal["loan"], usd(initial_cash));
        if sweep_first {
            assert_eq!(state.principal_shortfall["loan"][0].amount, usd(25));
        }

        state.advance_period();
        let last_period = dated_period(date!(2025 - 04 - 03), date!(2025 - 04 - 04));
        flows.get_mut("loan").expect("loan").debt_balance = usd(0);
        state
            .set_period_principal_flows("loan", &last_period, Vec::new())
            .expect("protected installment becomes economically effective");
        let last = execute_waterfall(
            &last_period.id,
            &context_for(last_period.id, &[("cash", 25.0)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("settle any carried arrears without duplicating principal");
        assert_eq!(last.flows["loan"].debt_balance.amount(), 0.0);
        assert_eq!(
            last.flows["loan"].principal_payment,
            usd(100 - initial_cash)
        );
        assert_eq!(state.cumulative_principal["loan"].amount(), 100.0);
        assert!(state.principal_shortfall.is_empty());
        assert!(state.principal_advance_payments.is_empty());
    }
}
