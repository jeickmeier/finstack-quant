//! Regression coverage for priority allocation, paid-cash ECF and liabilities.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use crate::capital_structure::waterfall::execute_waterfall_same_period;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::{
    cashflow::CashFlowParams,
    currency::Currency,
    dates::{Date, DayCount, Period, PeriodId, PeriodKind},
    money::Money,
};
use finstack_quant_statements::capital_structure::{
    CapitalStructureState, CashflowBreakdown, EcfSweepSpec, PaymentClassSpec, PaymentPriority,
    PikToggleSpec, PrincipalClaim, WaterfallSpec,
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
    let mut context = EvaluationContext::new(
        period,
        &finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            Vec::new(),
        )
        .expect("empty plan"),
        Arc::new(columns),
        Arc::new(IndexMap::new()),
    );
    for (name, value) in values {
        context.set_value(name, *value).expect("declared node");
    }
    context
}

fn dated_period(start: Date, end: Date) -> Period {
    Period {
        id: PeriodId::from_date(start, PeriodKind::Daily, None).expect("daily period"),
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

fn waterfall_with_sweep(percentage: f64) -> WaterfallSpec {
    WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(sweep(percentage, None)),
        ..Default::default()
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
        let result = execute_waterfall_same_period(
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

/// A scheduled reduction is effective whether or not its cash is paid, so
/// the principal it removes cannot be swept. A sweep ordered ahead of
/// amortization therefore prepays the junior loan, and the senior payment it
/// starved is real arrears, reported apart from the debt balance.
#[test]
fn a_sweep_ahead_of_amortization_cannot_prepay_scheduled_principal() {
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
    let result = execute_waterfall_same_period(
        &context(&[("cash", 100.0), ("ebitda", 100.0)]),
        &spec,
        &mut state,
        &flows,
    )
    .expect("sweep before amortization");

    let (senior, junior) = (&result.flows["senior"], &result.flows["junior"]);
    assert_eq!(junior.principal_payment.amount(), 100.0);
    assert_eq!(junior.debt_balance.amount(), 0.0);
    assert_eq!(senior.principal_payment.amount(), 0.0);
    // The senior reduction took effect; its unpaid cash is arrears.
    assert_eq!(senior.debt_balance.amount(), 0.0);
    assert_eq!(senior.principal_arrears.map(|a| a.amount()), Some(100.0));
    assert_eq!(
        state.principal_shortfall["senior"][0].amount.amount(),
        100.0
    );
    assert_eq!(result.warnings.len(), 1);
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
        vec![PrincipalClaim::at_par(
            date!(2024 - 12 - 31),
            date!(2024 - 12 - 31),
            usd(20),
        )],
    );
    let spec = waterfall_with_sweep(0.5);
    let result = execute_waterfall_same_period(
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
    let result = execute_waterfall_same_period(
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
    let spec = waterfall_with_sweep(1.0);
    let result = execute_waterfall_same_period(
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
                target_instrument_ids: vec!["loan".into()],
                min_periods_in_pik: 0,
            }),
            ..Default::default()
        };
        let result =
            execute_waterfall_same_period(&context(&[("cash", 0.0)]), &spec, &mut state, &flows)
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
        let result = execute_waterfall_same_period(
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
        let arrears = |id: &str| result.flows[id].interest_arrears.map(|a| a.amount());
        assert_eq!(arrears("fees::A"), Some(20.0));
        assert_eq!(arrears("principal::A"), Some(40.0));
        assert_eq!(result.flows["fees::A"].accrued_interest.amount(), 0.0);
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
    let first = execute_waterfall_same_period(
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
            vec![PrincipalClaim::at_par(
                date!(2025 - 04 - 01),
                date!(2025 - 03 - 31),
                usd(10),
            )],
        )
        .expect("delayed cash claim");
    let second = execute_waterfall_same_period(
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
            vec![PrincipalClaim::at_par(
                date!(2025 - 04 - 01),
                date!(2025 - 03 - 31),
                usd(10),
            )],
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
    let result = execute_waterfall_same_period(
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
            vec![PrincipalClaim::at_par(
                date!(2025 - 03 - 28),
                date!(2025 - 04 - 02),
                usd(10),
            )],
        )
        .expect("early cash claim");
    let first = execute_waterfall_same_period(
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
    let second = execute_waterfall_same_period(
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
    let first = execute_waterfall_same_period(
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
    let second = execute_waterfall_same_period(
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
                vec![PrincipalClaim::at_par(
                    date!(2025 - 03 - 28),
                    date!(2025 - 04 - 02),
                    usd(10),
                )],
            )
            .expect("advance settlement");
        let first = execute_waterfall_same_period(
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
        let spec = waterfall_with_sweep(1.0);
        let middle = execute_waterfall_same_period(
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
        let last = execute_waterfall_same_period(
            &context_for(final_period.id, &[("cash", 100.0), ("ebitda", 100.0)]),
            &spec,
            &mut state,
            &flows,
        )
        .expect("retire the advance-payment timing item");
        assert_eq!(last.flows["loan"].principal_payment.amount(), 0.0);
        assert_eq!(last.flows["loan"].debt_balance.amount(), 0.0);
        assert!(state.principal_advance_payments.is_empty());
    }
}

#[test]
fn residual_rebuild_reserves_due_principal_ahead_of_future_installments() {
    for (initial_cash, sweep_first) in [(100, false), (75, true)] {
        let (mut state, mut flows) = loans(&["loan"], 100);
        let schedule = CashFlowSchedule::from_flows(
            vec![
                {
                    let cash = usd(50);
                    CashFlowParams::new(
                        date!(2025 - 04 - 04),
                        None,
                        cash,
                        CFKind::Amortization,
                        0.0,
                        None,
                    )
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(date!(2025 - 04 - 02))
                    .build()
                    .expect("valid cashflow")
                },
                {
                    let cash = usd(50);
                    CashFlowParams::new(
                        date!(2025 - 03 - 31),
                        None,
                        cash,
                        CFKind::Amortization,
                        0.0,
                        None,
                    )
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(date!(2025 - 04 - 03))
                    .build()
                    .expect("valid cashflow")
                },
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
                vec![PrincipalClaim::at_par(
                    date!(2025 - 03 - 31),
                    date!(2025 - 04 - 03),
                    usd(50),
                )],
            )
            .expect("advance is economically later than the other installment");
        let mut spec = waterfall_with_sweep(1.0);
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
        let first = execute_waterfall_same_period(
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
                .find(|flow| flow.get_date() == date!(2025 - 04 - 04))
                .expect("future installment row")
                .get_amount()
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
        let middle = execute_waterfall_same_period(
            &context_for(middle_period.id, &[("cash", 0.0)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("reserved advance reconciles with rebuilt economic principal");
        assert_eq!(middle.flows["loan"].debt_balance.amount(), 50.0);
        assert_eq!(middle.flows["loan"].principal_payment.amount(), 0.0);
        if sweep_first {
            assert_eq!(state.principal_shortfall["loan"][0].amount, usd(25));
        }

        state.advance_period();
        let last_period = dated_period(date!(2025 - 04 - 03), date!(2025 - 04 - 04));
        flows.get_mut("loan").expect("loan").debt_balance = usd(0);
        state
            .set_period_principal_flows("loan", &last_period, Vec::new())
            .expect("protected installment becomes economically effective");
        let last = execute_waterfall_same_period(
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
        assert!(state.principal_shortfall.is_empty());
        assert!(state.principal_advance_payments.is_empty());
    }
}

// Dated principal claims, arrears and advance reserves (STATEMENTS-CAPITAL-004)

fn amortize_then_sweep() -> WaterfallSpec {
    WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(sweep(1.0, None)),
        ..Default::default()
    }
}

/// The record's example: a 20 reduction effective 31 March on a 100 loan is
/// due on 15 April. Outstanding principal is 80 from 31 March whatever
/// happens to the cash; the missed cash is arrears until it is paid.
#[test]
fn a_reduction_takes_effect_on_its_date_and_missed_cash_is_arrears() {
    let (mut state, mut flows) = loans(&["loan"], 80);
    let april = dated_period(date!(2025 - 04 - 01), date!(2025 - 05 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(80);
    loan.principal_payment = usd(20);
    let claim = PrincipalClaim::at_par(date!(2025 - 04 - 15), date!(2025 - 03 - 31), usd(20));
    let claim_id = claim.id.clone();
    state
        .set_period_principal_flows("loan", &april, vec![claim])
        .expect("claim due in April");

    // April has 5 of cash: a partial payment on the due date.
    let paid = execute_waterfall_same_period(
        &context_for(april.id, &[("cash", 5.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("partial payment");
    let loan = &paid.flows["loan"];
    assert_eq!(loan.principal_payment.amount(), 5.0);
    assert_eq!(loan.debt_balance.amount(), 80.0);
    assert_eq!(loan.principal_arrears, Some(usd(15)));
    let carried = &state.principal_shortfall["loan"][0];
    assert_eq!(carried.id, claim_id);
    assert_eq!((carried.amount, carried.face), (usd(15), usd(15)));
    assert_eq!(carried.payments.len(), 1);
    assert_eq!(carried.payments[0].date, date!(2025 - 04 - 15));
    assert_eq!(carried.payments[0].amount, usd(5));

    // May pays the arrears late. The claim keeps its identity and dates, and
    // settling it does not reduce the 80 again.
    state.advance_period();
    let may = dated_period(date!(2025 - 05 - 01), date!(2025 - 06 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.principal_payment = usd(0);
    state
        .set_period_principal_flows("loan", &may, Vec::new())
        .expect("nothing new due");
    let settled = execute_waterfall_same_period(
        &context_for(may.id, &[("cash", 100.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("late payment");
    let loan = &settled.flows["loan"];
    assert_eq!(loan.principal_payment.amount(), 15.0);
    assert_eq!(loan.debt_balance.amount(), 80.0);
    assert_eq!(loan.principal_arrears, Some(usd(0)));
    assert!(state.principal_shortfall.is_empty());
    assert_eq!(
        settled.equity_distribution.expect("residual").amount(),
        85.0
    );
}

/// Cash paid before its reduction takes effect is reserved: the face stays
/// in outstanding principal until the effective date and cannot be prepaid
/// a second time.
#[test]
fn principal_paid_ahead_is_reserved_until_its_effective_date() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let march = dated_period(date!(2025 - 03 - 01), date!(2025 - 04 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(100);
    loan.principal_payment = usd(10);
    state
        .set_period_principal_flows(
            "loan",
            &march,
            vec![PrincipalClaim::at_par(
                date!(2025 - 03 - 20),
                date!(2025 - 04 - 10),
                usd(10),
            )],
        )
        .expect("early cash claim");
    let first = execute_waterfall_same_period(
        &context_for(march.id, &[("cash", 500.0), ("ebitda", 500.0)]),
        &amortize_then_sweep(),
        &mut state,
        &flows,
    )
    .expect("pay ahead, then sweep");
    let loan = &first.flows["loan"];
    // 10 scheduled and 90 swept: the reserved 10 cannot be swept as well.
    assert_eq!(loan.scheduled_principal, Some(usd(10)));
    assert_eq!(loan.sweep_prepayment, Some(usd(90)));
    assert_eq!(loan.principal_advance_reserve, Some(usd(10)));
    assert_eq!(loan.debt_balance.amount(), 10.0);
    assert_eq!(state.principal_advance_payments["loan"][0].face, usd(10));

    // On 10 April the reduction takes effect: the schedule closes at zero
    // and the reserve is released without any further cash.
    state.advance_period();
    let april = dated_period(date!(2025 - 04 - 01), date!(2025 - 05 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(0);
    loan.principal_payment = usd(0);
    state
        .set_period_principal_flows("loan", &april, Vec::new())
        .expect("nothing due");
    let second = execute_waterfall_same_period(
        &context_for(april.id, &[("cash", 500.0), ("ebitda", 500.0)]),
        &amortize_then_sweep(),
        &mut state,
        &flows,
    )
    .expect("effective date");
    let loan = &second.flows["loan"];
    assert_eq!(loan.principal_payment.amount(), 0.0);
    assert_eq!(loan.debt_balance.amount(), 0.0);
    assert_eq!(loan.principal_advance_reserve, Some(usd(0)));
    assert!(state.principal_advance_payments.is_empty());
}

/// Scheduled principal is always dated and reconciles exactly: nothing is
/// synthesized from an aggregate, and a claim list that misses the
/// contractual cash by a cent is rejected.
#[test]
fn scheduled_principal_needs_dated_claims_that_reconcile_exactly() {
    use crate::capital_structure::waterfall::execute_waterfall;
    let (mut state, mut flows) = loans(&["loan"], 80);
    let april = dated_period(date!(2025 - 04 - 01), date!(2025 - 05 - 01));
    let loan = flows.get_mut("loan").expect("loan");
    loan.debt_balance = usd(80);
    loan.principal_payment = usd(20);
    let context = context_for(april.id, &[("cash", 100.0)]);

    let error = execute_waterfall(
        &context,
        &WaterfallSpec::default(),
        &mut state,
        &flows,
        None,
    )
    .expect_err("no dated claims")
    .to_string();
    assert!(error.contains("no dated principal claims"), "{error}");

    let short = Money::new(19.99, Currency::USD).expect("money");
    state
        .set_period_principal_flows(
            "loan",
            &april,
            vec![PrincipalClaim::at_par(
                date!(2025 - 04 - 15),
                date!(2025 - 03 - 31),
                short,
            )],
        )
        .expect("claim");
    let error = execute_waterfall(
        &context,
        &WaterfallSpec::default(),
        &mut state,
        &flows,
        None,
    )
    .expect_err("claims a cent short")
    .to_string();
    assert!(error.contains("do not reconcile"), "{error}");
    assert!(state.closing_balances.is_empty());
}

// Exact allocation, residual treatment and conservation (STATEMENTS-CAPITAL-006)

fn cents(value: i64) -> Money {
    Money::from_decimal(rust_decimal::Decimal::new(value, 2), Currency::USD)
}

fn no_equity() -> WaterfallSpec {
    WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
        ],
        ..Default::default()
    }
}

/// The record's example: five cents across three equal two-cent claims pay
/// two, two and one by instrument id, whatever the row order, and the unpaid
/// cent is carried.
#[test]
fn five_cents_across_three_claims_are_paid_in_whole_cents_by_stable_id() {
    for order in [["a", "b", "c"], ["c", "a", "b"]] {
        let (mut state, mut flows) = loans(&order, 100);
        for flow in flows.values_mut() {
            flow.interest_expense_cash = cents(2);
        }
        let result = execute_waterfall_same_period(
            &context(&[("cash", 0.05)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("five cents");
        let paid = |id: &str| result.flows[id].interest_expense_cash;
        assert_eq!(
            (paid("a"), paid("b"), paid("c")),
            (cents(2), cents(2), cents(1))
        );
        assert_eq!(result.flows["c"].interest_arrears, Some(cents(1)));
        assert_eq!(state.interest_shortfall["c"], cents(1));
        assert_eq!(result.equity_distribution, Some(cents(0)));
    }
}

/// Every unpaid amount is carried, however small: a claim below one cent
/// that goes unpaid is not noise.
#[test]
fn a_subcent_claim_that_goes_unpaid_is_carried_exactly() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let tenth_of_a_cent = Money::from_decimal(rust_decimal::Decimal::new(1, 3), Currency::USD);
    flows.get_mut("loan").expect("loan").interest_expense_cash = tenth_of_a_cent;
    let result = execute_waterfall_same_period(
        &context(&[("cash", 0.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("no cash");
    assert_eq!(result.flows["loan"].interest_arrears, Some(tenth_of_a_cent));
    assert_eq!(state.interest_shortfall["loan"], tenth_of_a_cent);
    assert_eq!(result.warnings.len(), 1);
}

/// The record's example: a 15 residual is an equity distribution when
/// Equity is the terminal priority and retained cash when it is absent.
#[test]
fn the_residual_goes_to_equity_only_when_equity_is_the_terminal_priority() {
    for (spec, equity, retained) in [(WaterfallSpec::default(), 15, 0), (no_equity(), 0, 15)] {
        let (mut state, mut flows) = loans(&["loan"], 100);
        flows.get_mut("loan").expect("loan").interest_expense_cash = usd(5);
        let result =
            execute_waterfall_same_period(&context(&[("cash", 20.0)]), &spec, &mut state, &flows)
                .expect("residual");
        assert_eq!(result.equity_distribution, Some(usd(equity)));
        assert_eq!(result.retained_cash, Some(usd(retained)));
        assert_eq!(result.available_cash, Some(usd(20)));
    }
}

/// A negative pool is a funding shortfall: nothing is allocated and every
/// claim is carried.
#[test]
fn a_negative_pool_allocates_nothing_and_reports_the_shortfall() {
    let (mut state, mut flows) = loans(&["loan"], 100);
    let loan = flows.get_mut("loan").expect("loan");
    loan.interest_expense_cash = usd(5);
    loan.fees = usd(2);
    let result = execute_waterfall_same_period(
        &context(&[("cash", -10.0)]),
        &WaterfallSpec::default(),
        &mut state,
        &flows,
    )
    .expect("negative pool");
    assert_eq!(result.available_cash, Some(usd(0)));
    assert_eq!(result.equity_distribution, Some(usd(0)));
    assert_eq!(result.flows["loan"].interest_expense_cash, usd(0));
    assert_eq!(state.interest_shortfall["loan"], usd(5));
    assert_eq!(state.fee_shortfall["loan"], usd(2));
    // The funding shortfall and the two unpaid claims are all reported.
    assert_eq!(result.warnings.len(), 3);
}

/// The waterfall is evaluated without debt claims: the whole pool is
/// residual. Only a structure with no currency at all has nothing to run.
#[test]
fn a_debt_free_period_still_evaluates_the_pool() {
    use crate::capital_structure::waterfall::execute_waterfall;
    let empty = IndexMap::new();
    let context = context(&[("cash", 40.0)]);
    let mut state = CapitalStructureState::new();

    let with_equity = execute_waterfall(
        &context,
        &WaterfallSpec::default(),
        &mut state,
        &empty,
        Some(Currency::USD),
    )
    .expect("debt-free");
    assert_eq!(with_equity.available_cash, Some(usd(40)));
    assert_eq!(with_equity.equity_distribution, Some(usd(40)));
    assert_eq!(with_equity.retained_cash, Some(usd(0)));

    let retained = execute_waterfall(
        &context,
        &no_equity(),
        &mut state,
        &empty,
        Some(Currency::USD),
    )
    .expect("debt-free, no equity");
    assert_eq!(retained.retained_cash, Some(usd(40)));

    let nothing = execute_waterfall(
        &context,
        &WaterfallSpec::default(),
        &mut state,
        &empty,
        None,
    )
    .expect("no currency");
    assert!(nothing.available_cash.is_none() && nothing.equity_distribution.is_none());

    // Zero claims on existing instruments run the same way.
    let (mut state, flows) = loans(&["loan"], 100);
    let result =
        execute_waterfall_same_period(&context, &WaterfallSpec::default(), &mut state, &flows)
            .expect("no claims");
    assert_eq!(result.equity_distribution, Some(usd(40)));
}

/// Allocatable cash reconciles exactly to what was paid plus the residual,
/// and each instrument's payment does not depend on the order of the rows.
#[test]
fn allocation_is_exact_and_independent_of_row_order() {
    let claims = [
        ("alpha", 94_892_500_i64),
        ("beta", 14_753_112),
        ("gamma", 333),
    ];
    let mut by_order = Vec::new();
    for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
        let ids: Vec<&str> = order.iter().map(|index| claims[*index].0).collect();
        let (mut state, mut flows) = loans(&ids, 1_000_000_000);
        for (id, amount) in claims {
            let flow = flows.get_mut(id).expect("loan");
            flow.interest_expense_cash = cents(amount);
            flow.fees = cents(amount / 7);
        }
        let result = execute_waterfall_same_period(
            &context(&[("cash", 997_982.74)]),
            &WaterfallSpec::default(),
            &mut state,
            &flows,
        )
        .expect("short cash");
        let mut uses = result.equity_distribution.expect("equity");
        for flow in result.flows.values() {
            uses = uses
                .checked_add(flow.fees)
                .and_then(|sum| sum.checked_add(flow.interest_expense_cash))
                .and_then(|sum| sum.checked_add(flow.principal_payment))
                .expect("sum");
        }
        assert_eq!(Some(uses), result.available_cash);
        by_order.push(claims.map(|(id, _)| {
            (
                result.flows[id].fees,
                result.flows[id].interest_expense_cash,
            )
        }));
    }
    assert_eq!(by_order[0], by_order[1]);
    assert_eq!(by_order[0], by_order[2]);
}
