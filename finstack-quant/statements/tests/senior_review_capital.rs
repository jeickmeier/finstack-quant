//! Regression coverage for capital-structure review findings.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{build_periods, Date, DayCount, StubKind};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_statements::capital_structure::{
    calculate_period_flows, execute_waterfall, CapitalStructureState, CashflowBreakdown,
    EcfSweepSpec, PaymentPriority, PikToggleSpec, WaterfallSpec,
};
use finstack_quant_statements::evaluator::EvaluationContext;
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::FinancialStatementInstrument;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, CommitmentFeeBase, DdtlSpec, DrawEvent, OidPolicy, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::PayReceive;
use indexmap::IndexMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::macros::date;

fn swap_market(forward_rate: f64) -> MarketContext {
    let as_of = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("valid discount curve");
    let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, forward_rate), (5.0, forward_rate)])
        .build()
        .expect("valid signed forward curve");
    MarketContext::new().insert(discount).insert(forward)
}

#[test]
fn declared_swap_kind_nets_negative_rates_and_zero_fixed_legs() {
    let as_of = date!(2025 - 01 - 01);
    let period = build_periods("2025..2025", None)
        .expect("annual period")
        .periods
        .remove(0);
    for (side, fixed_rate, forward_rate) in [
        (PayReceive::Receive, 0.01, -0.01),
        (PayReceive::Pay, 0.01, -0.01),
        (PayReceive::Receive, 0.0, -0.01),
        (PayReceive::Pay, 0.0, -0.01),
        (PayReceive::Pay, -0.02, 0.01),
    ] {
        let swap = crate::dsl_all::rates_support::usd_irs_swap(
            "IRS".into(),
            Money::from((100_000_i64, Currency::USD)),
            fixed_rate,
            as_of,
            date!(2027 - 01 - 01),
            side,
        )
        .expect("canonical signed IRS");
        let market = swap_market(forward_rate);
        let raw = swap
            .raw_cashflow_schedule(&market, as_of)
            .expect("raw IRS schedule");
        let net: f64 = raw
            .get_flows()
            .iter()
            .filter(|flow| flow.date >= period.start && flow.date < period.end)
            .filter(|flow| matches!(flow.kind, CFKind::Fixed | CFKind::Stub | CFKind::FloatReset))
            .map(|flow| flow.amount.amount())
            .sum();
        assert!(net != 0.0, "fixture must produce a net payment or receipt");
        if fixed_rate == 0.0 {
            assert!(raw
                .get_flows()
                .iter()
                .any(|flow| matches!(flow.kind, CFKind::Fixed | CFKind::Stub)
                    && flow.amount.amount() == 0.0));
        } else {
            // These canonical schedules carry one sign throughout their life,
            // so opposite-sign inference cannot recognize the instrument.
            let positive = raw
                .get_flows()
                .iter()
                .any(|flow| flow.amount.amount() > 0.0);
            let negative = raw
                .get_flows()
                .iter()
                .any(|flow| flow.amount.amount() < 0.0);
            assert_ne!(positive, negative);
        }
        let expected_expense = (-net).max(0.0);
        let expected_income = net.max(0.0);
        let (flows, closing, funding, _) = calculate_period_flows(
            &swap,
            &period,
            swap.notional,
            Money::from((0_i64, Currency::USD)),
            &market,
            as_of,
            None,
        )
        .expect("declared IRS period flows");
        assert_eq!(closing.amount(), 0.0);
        assert_eq!(funding.amount(), 0.0);
        assert_eq!(flows.debt_balance.amount(), 0.0);
        assert_eq!(flows.accrued_interest.amount(), 0.0);
        assert!((flows.interest_expense_cash.amount() - expected_expense).abs() < 1e-9);
        assert!((flows.interest_income_cash_or_zero().amount() - expected_income).abs() < 1e-9);

        // Explicit schedule dates still cannot turn a hedge notional into funding.
        let mut dated_meta = raw.get_meta().clone();
        dated_meta.issue_date = Some(period.start);
        let dated = CashFlowSchedule::from_parts(
            raw.get_flows().to_vec(),
            raw.get_notional().clone(),
            raw.get_day_count(),
            dated_meta,
        );
        let (_, closing, funding, _) = calculate_period_flows(
            &swap,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market,
            as_of,
            Some(&dated),
        )
        .expect("dated hedge schedule");
        assert_eq!(closing.amount(), 0.0);
        assert_eq!(funding.amount(), 0.0);

        for waterfall in [false, true] {
            let builder = ModelBuilder::new("signed-swap").add_debt(
                "IRS",
                FinancialStatementInstrument::InterestRateSwap(swap.clone()),
            );
            let builder = if waterfall {
                builder.waterfall(WaterfallSpec {
                    priority_of_payments: vec![
                        PaymentPriority::Fees,
                        PaymentPriority::Interest,
                        PaymentPriority::Amortization,
                        PaymentPriority::Equity,
                    ],
                    ..Default::default()
                })
            } else {
                builder
            };
            let model = builder
                .periods("2025..2025", None)
                .expect("valid annual period")
                .value("cash", &[(period.id, AmountOrScalar::scalar(10_000.0))])
                .compute("balance", "cs.debt_balance.IRS")
                .expect("hedge balance")
                .compute("expense", "cs.interest_expense_cash.IRS")
                .expect("expense formula")
                .compute("income", "cs.interest_income.IRS")
                .expect("income formula")
                .build()
                .expect("both swap sides are supported");
            let result = Evaluator::new()
                .evaluate_with_market(&model, &market, as_of)
                .expect("signed IRS evaluation");
            assert_eq!(result.get("balance", &period.id), Some(0.0));
            assert!(
                (result.get("expense", &period.id).expect("expense") - expected_expense).abs()
                    < 1e-9,
                "side={side:?}, fixed={fixed_rate}, forward={forward_rate}, waterfall={waterfall}"
            );
            assert!(
                (result.get("income", &period.id).expect("income") - expected_income).abs() < 1e-9
            );
            if waterfall {
                let cashflows = result.cs_cashflows.expect("capital structure");
                assert!(
                    (cashflows.equity_distribution[&period.id].amount()
                        - (10_000.0 - expected_expense))
                        .abs()
                        < 1e-9
                );
            }
        }
    }
}

#[test]
fn canonical_negative_debt_coupons_are_income() {
    let as_of = date!(2025 - 01 - 01);
    let period = build_periods("2025..2025", None)
        .expect("annual period")
        .periods
        .remove(0);
    let mut negative_loan = loan(as_of);
    let mut floating =
        finstack_quant_cashflows::builder::FloatingRateSpec::euribor_3m(Decimal::ZERO);
    floating.forward_curve_id = "USD-SOFR-3M".into();
    floating.reset_lag_days = 0;
    negative_loan.rate = RateSpec::Floating(floating);
    let market = swap_market(-0.01);
    let spec = FinancialStatementInstrument::TermLoan(negative_loan);
    let instrument: finstack_quant_valuations::instruments::InstrumentJson = spec.clone().into();
    let instrument = instrument.into_boxed().expect("valid negative-rate debt");
    let raw = instrument
        .raw_cashflow_schedule(&market, as_of)
        .expect("negative coupon schedule");
    let net: f64 = raw
        .get_flows()
        .iter()
        .filter(|flow| flow.date >= period.start && flow.date < period.end)
        .filter(|flow| matches!(flow.kind, CFKind::Fixed | CFKind::Stub | CFKind::FloatReset))
        .map(|flow| flow.amount.amount())
        .sum();
    assert!(
        net < 0.0,
        "negative debt coupon must survive canonical emission"
    );
    let model = ModelBuilder::new("negative-debt")
        .add_debt("DEBT", spec)
        .periods("2025..2025", None)
        .expect("annual period")
        .compute("expense", "cs.interest_expense_cash.DEBT")
        .expect("expense")
        .compute("income", "cs.interest_income.DEBT")
        .expect("income")
        .build()
        .expect("negative-rate debt model");
    let result = Evaluator::new()
        .evaluate_with_market(&model, &market, as_of)
        .expect("negative-rate debt evaluation");
    assert_eq!(result.get("expense", &period.id), Some(0.0));
    assert!((result.get("income", &period.id).expect("income") + net).abs() < 1e-9);
}

#[test]
fn placeholder_rate_option_schedule_is_rejected_explicitly() {
    let option = finstack_quant_valuations::instruments::CapFloor::example()
        .expect("canonical cap/floor fixture");
    let as_of = date!(2025 - 01 - 01);
    let period = build_periods("2025..2025", None)
        .expect("annual period")
        .periods
        .remove(0);
    let error = calculate_period_flows(
        &option,
        &period,
        Money::from((0_i64, Currency::USD)),
        Money::from((0_i64, Currency::USD)),
        &MarketContext::new(),
        as_of,
        None,
    )
    .expect_err("unmodeled contingent payouts must not appear as zero income");
    assert!(error.to_string().contains("placeholder cashflow schedule"));
    let model = ModelBuilder::new("placeholder-option")
        .add_debt("OPTION", FinancialStatementInstrument::CapFloor(option))
        .periods("2025..2025", None)
        .expect("annual period")
        .build()
        .expect("valid model specification");
    let error = Evaluator::new()
        .evaluate_with_market(&model, &MarketContext::new(), as_of)
        .expect_err("runtime must reject the placeholder too");
    assert!(error.to_string().contains("placeholder cashflow schedule"));
}

#[test]
fn pik_toggle_rejects_hedge_targets_and_keeps_explicit_debt_targets() {
    let as_of = date!(2025 - 01 - 01);
    let swap = crate::dsl_all::rates_support::usd_irs_swap(
        "IRS".into(),
        Money::from((100_000_i64, Currency::USD)),
        0.01,
        as_of,
        date!(2027 - 01 - 01),
        PayReceive::Pay,
    )
    .expect("canonical IRS");
    let waterfall = |targets| WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        pik_toggle: Some(PikToggleSpec {
            liquidity_metric: "cash".into(),
            threshold: 100.0,
            target_instrument_ids: targets,
            min_periods_in_pik: 0,
        }),
        ..Default::default()
    };
    for targets in [Some(vec!["IRS".into()]), None] {
        let error = ModelBuilder::new("hedge-pik")
            .add_debt(
                "IRS",
                FinancialStatementInstrument::InterestRateSwap(swap.clone()),
            )
            .waterfall(waterfall(targets.clone()))
            .periods("2025..2025", None)
            .expect("annual period")
            .value(
                "cash",
                &[(PeriodId::annual(2025), AmountOrScalar::scalar(10.0))],
            )
            .build()
            .expect_err("hedge payments cannot become borrowing principal");
        assert!(error.to_string().contains(if targets.is_some() {
            "hedge or option"
        } else {
            "must explicitly list"
        }));
    }
    ModelBuilder::new("debt-pik")
        .add_debt("TL", FinancialStatementInstrument::TermLoan(loan(as_of)))
        .waterfall(waterfall(Some(vec!["TL".into()])))
        .periods("2025..2025", None)
        .expect("annual period")
        .value(
            "cash",
            &[(PeriodId::annual(2025), AmountOrScalar::scalar(10.0))],
        )
        .build()
        .expect("explicit debt-only PIK target remains valid");
}

fn loan(issue: Date) -> TermLoan {
    let mut loan = TermLoan::example().expect("valid term-loan example");
    loan.id = InstrumentId::new("TL");
    loan.notional_limit = Money::from((100_i64, Currency::USD));
    loan.issue_date = issue;
    loan.maturity = date!(2027 - 01 - 01);
    loan.stub = StubKind::ShortFront;
    loan.rate = RateSpec::Fixed { rate: 0.0 };
    loan.amortization = AmortizationSpec::None;
    loan
}

fn oid_loan(issue: Date, oid_bp: u32) -> TermLoan {
    let mut loan = loan(issue);
    loan.ddtl = Some(DdtlSpec {
        commitment: loan.notional_limit,
        availability_start: issue,
        availability_end: date!(2025 - 12 - 31),
        draws: vec![DrawEvent {
            date: issue,
            amount: loan.notional_limit,
        }],
        commitment_steps: vec![],
        usage_fee_bp: Decimal::ZERO,
        commitment_fee_bp: Decimal::ZERO,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: Some(OidPolicy::WithheldBp(Decimal::from(oid_bp))),
    });
    loan
}

use crate::dsl_all::support::ScheduleInstrument;

fn no_funding_schedule(initial: f64, issue: Date) -> ScheduleInstrument {
    let flows = if initial > 0.0 {
        vec![CashFlow::new(
            date!(2027 - 01 - 01),
            None,
            Money::new(initial, Currency::USD).expect("valid redemption principal"),
            CFKind::Notional,
            0.0,
            None,
        )]
    } else {
        vec![]
    };
    ScheduleInstrument::new(CashFlowSchedule::from_parts(
        flows,
        Notional::par(initial, Currency::USD).expect("valid decimal principal"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    ))
}

fn empty_debt_context() -> (
    PeriodId,
    EvaluationContext,
    CapitalStructureState,
    IndexMap<String, CashflowBreakdown>,
) {
    let period = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut context = EvaluationContext::new(
        period,
        Arc::new([(NodeId::new("cash"), 0)].into_iter().collect()),
        Arc::new(IndexMap::new()),
    );
    context.set_value("cash", 10.0).expect("known node");
    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL".into(), Money::from((100_i64, Currency::USD)));
    let flows = [("TL".into(), CashflowBreakdown::with_currency(Currency::USD))]
        .into_iter()
        .collect();
    (period, context, state, flows)
}

fn ecf_sweep() -> EcfSweepSpec {
    EcfSweepSpec {
        ebitda_node: "cash".into(),
        taxes_node: None,
        capex_node: None,
        working_capital_node: None,
        cash_interest_node: None,
        sweep_percentage: 0.5,
        target_instrument_id: Some("TL".into()),
    }
}

#[test]
fn loan_issuance_is_funded_once_before_on_and_after_first_period_start() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let q2 = PeriodId::quarter(2025, 2).expect("valid quarter");
    for issue in [
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 15),
    ] {
        for waterfall in [false, true] {
            let builder = ModelBuilder::new("issuance")
                .add_debt("TL", FinancialStatementInstrument::TermLoan(loan(issue)));
            let builder = if waterfall {
                builder.waterfall(WaterfallSpec::default())
            } else {
                builder
            };
            let model = builder
                .periods("2025Q1..2025Q2", None)
                .expect("valid periods")
                .value(
                    "cash",
                    &[
                        (q1, AmountOrScalar::scalar(10.0)),
                        (q2, AmountOrScalar::scalar(10.0)),
                    ],
                )
                .compute("balance", "cs.debt_balance.TL")
                .expect("valid capital-structure formula")
                .build()
                .expect("valid model");
            let results = Evaluator::new()
                .evaluate_with_market(&model, &MarketContext::new(), date!(2025 - 01 - 01))
                .expect("issuance evaluation");
            for period in [q1, q2] {
                assert_eq!(
                    results.get("balance", &period),
                    Some(100.0),
                    "issue={issue}, waterfall={waterfall}, period={period}"
                );
            }
        }
    }
}

#[test]
fn bond_issuance_is_funded_once_before_on_and_after_first_period_start() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let q2 = PeriodId::quarter(2025, 2).expect("valid quarter");
    for issue in [
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 15),
    ] {
        let bond = Bond::fixed(
            InstrumentId::new("BOND"),
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.0).expect("valid coupon"),
            issue,
            date!(2027 - 01 - 01),
            StubKind::ShortFront,
            CurveId::new("USD-OIS"),
        )
        .expect("valid bond");
        for waterfall in [false, true] {
            let builder = ModelBuilder::new("bond opening")
                .add_debt("BOND", FinancialStatementInstrument::Bond(bond.clone()));
            let builder = if waterfall {
                builder.waterfall(WaterfallSpec {
                    priority_of_payments: vec![
                        PaymentPriority::Fees,
                        PaymentPriority::Interest,
                        PaymentPriority::Amortization,
                        PaymentPriority::Equity,
                    ],
                    ..Default::default()
                })
            } else {
                builder
            };
            let model = builder
                .periods("2025Q1..2025Q2", None)
                .expect("valid periods")
                .value(
                    "cash",
                    &[
                        (q1, AmountOrScalar::scalar(10.0)),
                        (q2, AmountOrScalar::scalar(10.0)),
                    ],
                )
                .compute("balance", "cs.debt_balance.BOND")
                .expect("valid capital-structure formula")
                .build()
                .expect("bond waterfall has no prepayment priority");
            let results = Evaluator::new()
                .evaluate_with_market(&model, &MarketContext::new(), date!(2025 - 01 - 01))
                .expect("bond evaluation");
            for period in [q1, q2] {
                assert_eq!(
                    results.get("balance", &period),
                    Some(100.0),
                    "issue={issue}, waterfall={waterfall}, period={period}"
                );
            }
        }
    }
}

#[test]
fn oid_funding_preserves_loan_face_and_cash_at_boundaries() {
    let periods = build_periods("2025Q1..2025Q2", None)
        .expect("valid periods")
        .periods;
    let market = MarketContext::new();
    let zero = Money::from((0_i64, Currency::USD));
    for issue in [
        date!(2024 - 12 - 31),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 15),
    ] {
        for oid_bp in [200, 10_000] {
            let loan = oid_loan(issue, oid_bp);
            let raw = loan
                .raw_cashflow_schedule(&market, periods[0].start)
                .expect("valid DDTL OID schedule");
            let draw = raw
                .get_flows()
                .iter()
                .find(|flow| {
                    flow.principal_delta
                        .is_some_and(|delta| delta.amount() > 0.0)
                })
                .expect("DDTL draw preserves principal separately from cash");
            let expected_cash = if oid_bp == 200 { -98.0 } else { 0.0 };
            assert_eq!(draw.amount.amount(), expected_cash);
            assert_eq!(draw.principal_delta.expect("draw face").amount(), 100.0);
            let opening = if issue < periods[0].start {
                Money::from((100_i64, Currency::USD))
            } else {
                zero
            };
            let (_, closing, funding, _) = calculate_period_flows(
                &loan,
                &periods[0],
                opening,
                zero,
                &market,
                periods[0].start,
                None,
            )
            .expect("OID does not reduce principal funding");
            assert_eq!(closing.amount(), 100.0, "issue={issue}, oid_bp={oid_bp}");
            assert_eq!(
                funding.amount(),
                if issue < periods[0].start { 0.0 } else { 100.0 }
            );
            for waterfall in [false, true] {
                let builder = ModelBuilder::new("OID funding")
                    .add_debt("TL", FinancialStatementInstrument::TermLoan(loan.clone()));
                let builder = if waterfall {
                    builder.waterfall(WaterfallSpec::default())
                } else {
                    builder
                };
                let model = builder
                    .periods("2025Q1..2025Q2", None)
                    .expect("valid periods")
                    .value(
                        "cash",
                        &[
                            (periods[0].id, AmountOrScalar::scalar(10.0)),
                            (periods[1].id, AmountOrScalar::scalar(10.0)),
                        ],
                    )
                    .compute("balance", "cs.debt_balance.TL")
                    .expect("balance formula")
                    .build()
                    .expect("valid OID model");
                let result = Evaluator::new()
                    .evaluate_with_market(&model, &market, periods[0].start)
                    .expect("OID model evaluation");
                for period in &periods {
                    assert_eq!(
                        result.get("balance", &period.id),
                        Some(100.0),
                        "issue={issue}, oid_bp={oid_bp}, waterfall={waterfall}"
                    );
                    if waterfall {
                        let cashflows = result.cs_cashflows.as_ref().expect("debt cashflows");
                        assert_eq!(cashflows.equity_distribution[&period.id].amount(), 10.0);
                    }
                }
            }
        }
    }
}

#[test]
fn oid_funding_preserves_custom_bond_face_with_separate_cash_dates() {
    let market = MarketContext::new();
    let periods = build_periods("2025Q1..2025Q2", None)
        .expect("valid periods")
        .periods;
    for issue in [
        date!(2024 - 12 - 31),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 15),
    ] {
        for initial_face in [0.0, 100.0] {
            let funding = CashFlow::new(
                issue.next_day().expect("cash settlement date"),
                None,
                Money::from((-98_i64, Currency::USD)),
                CFKind::Notional,
                0.0,
                None,
            )
            .with_principal_delta(
                Money::new(100.0 - initial_face, Currency::USD).expect("funded face"),
            )
            .with_principal_date(issue);
            let schedule = CashFlowSchedule::from_parts(
                vec![
                    funding,
                    CashFlow::new(
                        date!(2027 - 01 - 01),
                        None,
                        Money::from((100_i64, Currency::USD)),
                        CFKind::Notional,
                        0.0,
                        None,
                    ),
                ],
                Notional::par(initial_face, Currency::USD).expect("initial face"),
                DayCount::Act365F,
                CashFlowMeta {
                    issue_date: Some(issue),
                    ..Default::default()
                },
            );
            let bond = Bond::fixed(
                InstrumentId::new("BOND"),
                Money::from((100_i64, Currency::USD)),
                Rate::from_decimal(0.0).expect("zero coupon"),
                issue,
                date!(2027 - 01 - 01),
                StubKind::ShortFront,
                CurveId::new("USD-OIS"),
            )
            .expect("valid bond")
            .with_custom_cashflows(schedule);
            let raw = bond
                .raw_cashflow_schedule(&market, periods[0].start)
                .expect("custom discounted issuance");
            assert_eq!(raw.get_flows()[0].amount.amount(), -98.0);
            for waterfall in [false, true] {
                let builder = ModelBuilder::new("custom OID bond")
                    .add_debt("BOND", FinancialStatementInstrument::Bond(bond.clone()));
                let builder = if waterfall {
                    builder.waterfall(WaterfallSpec {
                        priority_of_payments: vec![
                            PaymentPriority::Fees,
                            PaymentPriority::Interest,
                            PaymentPriority::Amortization,
                            PaymentPriority::Equity,
                        ],
                        ..Default::default()
                    })
                } else {
                    builder
                };
                let model = builder
                    .periods("2025Q1..2025Q2", None)
                    .expect("valid periods")
                    .value(
                        "cash",
                        &[
                            (periods[0].id, AmountOrScalar::scalar(10.0)),
                            (periods[1].id, AmountOrScalar::scalar(10.0)),
                        ],
                    )
                    .compute("balance", "cs.debt_balance.BOND")
                    .expect("balance formula")
                    .build()
                    .expect("valid custom bond model");
                let result = Evaluator::new()
                    .evaluate_with_market(&model, &market, periods[0].start)
                    .expect("custom bond evaluation");
                for period in &periods {
                    assert_eq!(
                        result.get("balance", &period.id),
                        Some(100.0),
                        "issue={issue}, initial_face={initial_face}, waterfall={waterfall}"
                    );
                    if waterfall {
                        let cashflows = result.cs_cashflows.as_ref().expect("debt cashflows");
                        assert_eq!(cashflows.equity_distribution[&period.id].amount(), 10.0);
                    }
                }
            }
        }
    }
}

#[test]
fn oid_funding_rejects_cash_currency_different_from_principal() {
    let period = build_periods("2025Q1..2025Q1", None)
        .expect("valid period")
        .periods
        .remove(0);
    let instrument = ScheduleInstrument::new(CashFlowSchedule::from_parts(
        vec![CashFlow::new(
            date!(2025 - 04 - 01),
            None,
            Money::from((-98_i64, Currency::EUR)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((100_i64, Currency::USD)))
        .with_principal_date(date!(2025 - 01 - 15))],
        Notional::par(0.0, Currency::USD).expect("initial face"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(date!(2025 - 01 - 15)),
            ..Default::default()
        },
    ));
    let zero = Money::from((0_i64, Currency::USD));
    let error = calculate_period_flows(
        &instrument,
        &period,
        zero,
        zero,
        &MarketContext::new(),
        period.start,
        None,
    )
    .expect_err("funding cash and principal must use the instrument currency");
    assert!(error.to_string().contains("Currency mismatch"));
}

#[test]
fn schedule_without_funding_books_issuance_once_and_does_not_resurrect_paid_debt() {
    let periods = build_periods("2025Q1..2025Q2", None)
        .expect("valid periods")
        .periods;
    let market = MarketContext::new();
    let zero = Money::from((0_i64, Currency::USD));
    for issue in [
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 15),
    ] {
        let instrument = no_funding_schedule(100.0, issue);
        for waterfall in [false, true] {
            let opening = if issue <= periods[0].start {
                Money::from((100_i64, Currency::USD))
            } else {
                zero
            };
            let (flows, closing, funding, _) = calculate_period_flows(
                &instrument,
                &periods[0],
                opening,
                zero,
                &market,
                periods[0].start,
                None,
            )
            .expect("issuance flows");
            assert_eq!(closing.amount(), 100.0);
            assert_eq!(
                funding.amount(),
                if issue > periods[0].start { 100.0 } else { 0.0 }
            );
            if waterfall {
                let (period, context, mut state, _) = empty_debt_context();
                state.opening_balances.insert("TL".into(), opening);
                state.period_new_funding.insert("TL".into(), funding);
                let result = execute_waterfall(
                    &period,
                    &context,
                    &WaterfallSpec::default(),
                    &mut state,
                    &[("TL".into(), flows)].into_iter().collect(),
                )
                .expect("issuance waterfall");
                assert_eq!(result.flows["TL"].debt_balance.amount(), 100.0);
            }
        }
        let (_, closing, funding, _) = calculate_period_flows(
            &instrument,
            &periods[1],
            zero,
            zero,
            &market,
            periods[0].start,
            None,
        )
        .expect("paid-off debt stays zero after issuance period");
        assert_eq!(closing.amount(), 0.0);
        assert_eq!(funding.amount(), 0.0);
    }
}

#[test]
fn implicit_issuance_rejects_negative_principal_and_keeps_zero_at_zero() {
    let period = build_periods("2025Q1..2025Q1", None)
        .expect("valid period")
        .periods
        .remove(0);
    let market = MarketContext::new();
    let zero = Money::from((0_i64, Currency::USD));
    let error = calculate_period_flows(
        &no_funding_schedule(-100.0, date!(2025 - 01 - 15)),
        &period,
        zero,
        zero,
        &market,
        period.start,
        None,
    )
    .expect_err("negative principal cannot imply a funding receipt");
    assert!(error.to_string().contains("nonnegative initial principal"));
    let (_, closing, funding, _) = calculate_period_flows(
        &no_funding_schedule(0.0, date!(2025 - 01 - 15)),
        &period,
        zero,
        zero,
        &market,
        period.start,
        None,
    )
    .expect("zero principal");
    assert_eq!(closing.amount(), 0.0);
    assert_eq!(funding.amount(), 0.0);
}

#[test]
fn newly_funded_loan_preserves_contractual_pik_in_its_first_period() {
    let period = PeriodId::annual(2025);
    let issue = date!(2025 - 01 - 01);
    let mut loan = loan(issue);
    loan.rate = RateSpec::Fixed { rate: 0.08 };
    loan.coupon_type = CouponType::Pik;
    let market = MarketContext::new();
    let schedule = loan
        .raw_cashflow_schedule(&market, issue)
        .expect("valid PIK schedule");
    let expected = schedule
        .outstanding_by_date()
        .expect("valid outstanding path")
        .into_iter()
        .rfind(|(date, _)| *date <= date!(2025 - 12 - 31))
        .expect("funded loan has an outstanding snapshot")
        .1
        .amount();
    assert!(expected > 100.0, "in-period PIK must capitalize");
    for waterfall in [false, true] {
        let builder = ModelBuilder::new("PIK issuance")
            .add_debt("TL", FinancialStatementInstrument::TermLoan(loan.clone()));
        let builder = if waterfall {
            builder.waterfall(WaterfallSpec::default())
        } else {
            builder
        };
        let model = builder
            .periods("2025..2025", None)
            .expect("valid annual period")
            .value("cash", &[(period, AmountOrScalar::scalar(10.0))])
            .compute("balance", "cs.debt_balance.TL")
            .expect("valid capital-structure formula")
            .build()
            .expect("valid model");
        let results = Evaluator::new()
            .evaluate_with_market(&model, &market, issue)
            .expect("PIK issuance evaluation");
        let actual = results
            .get("balance", &period)
            .expect("evaluated debt balance");
        assert!(
            (actual - expected).abs() < 1e-9,
            "waterfall={waterfall}: actual={actual}, expected={expected}"
        );
    }
}

#[test]
fn contractual_pik_is_redeemed_at_maturity_and_capitalized_once() {
    let period = PeriodId::annual(2025);
    let next = PeriodId::annual(2026);
    let issue = date!(2025 - 01 - 01);
    let mut loan = loan(issue);
    loan.maturity = date!(2025 - 10 - 01);
    loan.rate = RateSpec::Fixed { rate: 0.08 };
    loan.coupon_type = CouponType::Pik;
    let market = MarketContext::new();
    let schedule = loan
        .raw_cashflow_schedule(&market, issue)
        .expect("PIK maturity schedule");
    let expected_pik: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Pik)
        .map(|flow| flow.amount.amount().abs())
        .sum();
    let expected_principal = 100.0 + expected_pik;
    assert!(expected_pik > 0.0);
    for waterfall in [false, true] {
        let builder = ModelBuilder::new("PIK maturity")
            .add_debt("TL", FinancialStatementInstrument::TermLoan(loan.clone()));
        let builder = if waterfall {
            builder.waterfall(WaterfallSpec::default())
        } else {
            builder
        };
        let model = builder
            .periods("2025..2026", None)
            .expect("valid annual periods")
            .value(
                "cash",
                &[
                    (period, AmountOrScalar::scalar(200.0)),
                    (next, AmountOrScalar::scalar(200.0)),
                ],
            )
            .compute("balance", "cs.debt_balance.TL")
            .expect("balance formula")
            .compute("principal", "cs.principal_payment.TL")
            .expect("principal formula")
            .compute("pik", "cs.interest_expense_pik.TL")
            .expect("PIK formula")
            .build()
            .expect("valid maturity model");
        let result = Evaluator::new()
            .evaluate_with_market(&model, &market, issue)
            .expect("PIK maturity evaluation");
        assert!(
            (result.get("principal", &period).expect("principal") - expected_principal).abs()
                < 1e-9,
            "waterfall={waterfall}"
        );
        assert!(
            (result.get("pik", &period).expect("PIK") - expected_pik).abs() < 1e-9,
            "waterfall={waterfall}"
        );
        assert_eq!(
            result.get("balance", &period),
            Some(0.0),
            "waterfall={waterfall}"
        );
        assert_eq!(
            result.get("balance", &next),
            Some(0.0),
            "waterfall={waterfall}"
        );
        assert_eq!(result.get("principal", &next), Some(0.0));
    }
}

#[test]
fn loan_swept_after_issuance_stays_repaid_in_later_periods() {
    let periods: Vec<_> = (1..=3)
        .map(|quarter| PeriodId::quarter(2025, quarter).expect("quarter"))
        .collect();
    let model = ModelBuilder::new("issuance then sweep")
        .add_debt(
            "TL",
            FinancialStatementInstrument::TermLoan(loan(date!(2025 - 01 - 15))),
        )
        .waterfall(WaterfallSpec {
            ecf_sweep: Some(EcfSweepSpec {
                sweep_percentage: 1.0,
                ..ecf_sweep()
            }),
            ..Default::default()
        })
        .periods("2025Q1..2025Q3", None)
        .expect("quarterly periods")
        .value(
            "cash",
            &[
                (periods[0], AmountOrScalar::scalar(0.0)),
                (periods[1], AmountOrScalar::scalar(100.0)),
                (periods[2], AmountOrScalar::scalar(100.0)),
            ],
        )
        .compute("balance", "cs.debt_balance.TL")
        .expect("balance formula")
        .compute("principal", "cs.principal_payment.TL")
        .expect("principal formula")
        .build()
        .expect("valid model");
    let result = Evaluator::new()
        .evaluate_with_market(&model, &MarketContext::new(), date!(2025 - 01 - 01))
        .expect("issuance and sweep");
    assert_eq!(result.get("balance", &periods[0]), Some(100.0));
    assert_eq!(result.get("balance", &periods[1]), Some(0.0));
    assert_eq!(result.get("principal", &periods[1]), Some(100.0));
    assert_eq!(result.get("balance", &periods[2]), Some(0.0));
    assert_eq!(result.get("principal", &periods[2]), Some(0.0));
}

#[test]
fn named_prepayment_without_its_priority_is_rejected_before_state_changes() {
    for mandatory in [true, false] {
        let (period, context, mut state, flows) = empty_debt_context();
        let spec = WaterfallSpec {
            mandatory_prepay_node: mandatory.then(|| "20".into()),
            voluntary_prepay_node: (!mandatory).then(|| "20".into()),
            ..Default::default()
        };
        let node_name = if mandatory {
            "mandatory_prepay_node"
        } else {
            "voluntary_prepay_node"
        };
        assert!(spec
            .validate()
            .expect_err("sizing nodes require their payment priority")
            .to_string()
            .contains(node_name));
        let error = execute_waterfall(&period, &context, &spec, &mut state, &flows)
            .expect_err("invalid waterfall must not allocate principal");
        assert!(error.to_string().contains(node_name));
        assert_eq!(state.opening_balances["TL"].amount(), 100.0);
        assert!(state.closing_balances.is_empty());
        assert!(state.cumulative_principal.is_empty());
    }
}

#[test]
fn named_prepayment_priorities_consume_the_shared_cash_pool() {
    let (period, context, mut state, flows) = empty_debt_context();
    let spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::VoluntaryPrepayment,
            PaymentPriority::Equity,
        ],
        mandatory_prepay_node: Some("20".into()),
        voluntary_prepay_node: Some("20".into()),
        ..Default::default()
    };
    let result = execute_waterfall(&period, &context, &spec, &mut state, &flows)
        .expect("configured priorities cap their payments");
    let debt = &result.flows["TL"];
    let equity = result.equity_distribution.expect("waterfall residual");
    assert_eq!(debt.principal_payment.amount(), 10.0);
    assert_eq!(debt.debt_balance.amount(), 90.0);
    assert_eq!(equity.amount(), 0.0);
    assert_eq!(debt.principal_payment.amount() + equity.amount(), 10.0);
}

#[test]
fn positive_ecf_requires_sweep_even_with_other_prepayment_priorities() {
    for priority in [
        PaymentPriority::MandatoryPrepayment,
        PaymentPriority::VoluntaryPrepayment,
    ] {
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                priority,
                PaymentPriority::Equity,
            ],
            mandatory_prepay_node: (priority == PaymentPriority::MandatoryPrepayment)
                .then(|| "0".into()),
            voluntary_prepay_node: (priority == PaymentPriority::VoluntaryPrepayment)
                .then(|| "0".into()),
            ecf_sweep: Some(ecf_sweep()),
            ..Default::default()
        };
        assert!(spec
            .validate()
            .expect_err("only Sweep executes the ECF bucket")
            .to_string()
            .contains("requires the `Sweep`"));
    }
}

#[test]
fn ecf_sweep_priority_pays_the_sized_sweep_and_preserves_cash() {
    let (period, context, mut state, flows) = empty_debt_context();
    let spec = WaterfallSpec {
        ecf_sweep: Some(ecf_sweep()),
        ..Default::default()
    };
    let result = execute_waterfall(&period, &context, &spec, &mut state, &flows)
        .expect("positive ECF has its Sweep priority");
    let debt = &result.flows["TL"];
    let equity = result.equity_distribution.expect("waterfall residual");
    assert_eq!(debt.principal_payment.amount(), 5.0);
    assert_eq!(debt.debt_balance.amount(), 95.0);
    assert_eq!(equity.amount(), 5.0);
    assert_eq!(debt.principal_payment.amount() + equity.amount(), 10.0);
}
