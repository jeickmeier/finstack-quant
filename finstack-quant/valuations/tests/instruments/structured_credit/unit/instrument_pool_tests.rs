//! Deterministic pools of real instruments: schedule-driven flows, reserve
//! funding of draws, replenishment, and call/put exercise policies.

use finstack_quant_cashflows::traits::CashflowScheduleSource;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, CashflowSpec,
};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, CallExercisePolicy, DealType, DefaultModelSpec,
    InstrumentCollateral, PrepaymentModelSpec, PutExercisePolicy, SimulationRun, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use time::macros::date;

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};

pub(crate) fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

/// Fixed-rate revolver with one draw and one repayment inside its life.
pub(crate) fn fixed_revolver(commitment_date: Date, maturity: Date) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id(InstrumentId::new("RCF"))
        .commitment_amount(usd(50_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(commitment_date)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
            DrawRepayEvent {
                date: date!(2024 - 03 - 01),
                amount: usd(5_000_000.0),
                is_draw: true,
            },
            DrawRepayEvent {
                date: date!(2025 - 06 - 01),
                amount: usd(3_000_000.0),
                is_draw: false,
            },
        ]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("revolver")
}

pub(crate) struct DealSpec {
    pub(crate) collateral: InstrumentCollateral,
    pub(crate) reserve: f64,
    pub(crate) reserve_target: Option<f64>,
    pub(crate) closing: Date,
    pub(crate) maturity: Date,
    pub(crate) senior: f64,
    pub(crate) equity: f64,
}

/// CLO shell with a senior and an equity tranche, no fees, no prepayment or
/// default noise, and the given collateral and reserve.
pub(crate) fn deal_with(spec: DealSpec) -> StructuredCredit {
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.instruments = Some(spec.collateral);
    pool.reserve_account = usd(spec.reserve);
    pool.reserve_target = spec.reserve_target.map(usd);
    // Attachment order (equity first) is what the stochastic validator expects.
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "EQ",
            0.0,
            10.0,
            TrancheSeniority::Equity,
            usd(spec.equity),
            TrancheCoupon::Fixed { rate: 0.0 },
            spec.maturity,
        )
        .expect("equity"),
        Tranche::new(
            "A",
            10.0,
            100.0,
            TrancheSeniority::Senior,
            usd(spec.senior),
            TrancheCoupon::Fixed { rate: 0.05 },
            spec.maturity,
        )
        .expect("senior"),
    ])
    .expect("tranches");
    let mut deal = StructuredCredit::new_clo(
        "POOL-DEAL",
        pool,
        tranches,
        spec.closing,
        spec.maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal
}

pub(crate) fn totals(run: &SimulationRun) -> (f64, f64) {
    let interest: f64 = run
        .tranches
        .values()
        .map(|r| r.total_interest.amount())
        .sum();
    let principal: f64 = run
        .tranches
        .values()
        .map(|r| r.total_principal.amount())
        .sum();
    (interest, principal)
}

pub(crate) fn market_with_curves(base: Date) -> MarketContext {
    let fixings: Vec<(Date, f64)> = (0..25)
        .map(|days| (base - time::Duration::days(days), 0.04))
        .collect();
    MarketContext::new()
        .insert(flat_discount_curve(0.03, base, "USD-OIS"))
        .insert(flat_forward_curve(0.04, base, "USD-SOFR-3M"))
        .insert_series(ScalarTimeSeries::new("FIXING:USD-SOFR-3M", fixings, None).expect("fixings"))
}

/// A pool of one revolver, with a reserve at least equal to its commitment
/// and a pass-through waterfall, distributes exactly the facility's own
/// interest and fees, and returns every dollar of drawn balance and reserve.
#[test]
fn single_revolver_pool_reproduces_the_standalone_schedule() {
    let closing = date!(2024 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);
    let facility = fixed_revolver(closing, maturity);
    let market = MarketContext::new();

    let standalone = facility
        .raw_cashflow_schedule(&market, closing)
        .expect("standalone schedule");
    let expected_interest: f64 = standalone
        .get_flows()
        .iter()
        .filter(|cf| cf.date > closing)
        .filter(|cf| {
            matches!(
                cf.kind,
                CFKind::Fixed
                    | CFKind::FloatReset
                    | CFKind::Fee
                    | CFKind::CommitmentFee
                    | CFKind::UsageFee
                    | CFKind::FacilityFee
            )
        })
        .map(|cf| cf.amount.amount())
        .sum();
    assert!(expected_interest > 0.0);

    let deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            revolvers: vec![facility],
            ..Default::default()
        },
        reserve: 50_000_000.0,
        reserve_target: None,
        closing,
        maturity,
        senior: 9_000_000.0,
        equity: 1_000_000.0,
    });
    let run = run_simulation_with_diagnostics(&deal, &market, closing).expect("pool run");
    // The registry valuation path routes through the same engine.
    let market_with_discount =
        MarketContext::new().insert(flat_discount_curve(0.03, closing, "USD-OIS"));
    let pv = finstack_quant_valuations::instruments::Instrument::value(
        &deal,
        &market_with_discount,
        closing,
    )
    .expect("deal value");
    assert!(pv.amount().is_finite() && pv.amount() > 0.0);

    let (interest, principal) = totals(&run);
    // Total cash to the notes = facility interest and fees + drawn balance
    // repaid + reserve released at deal end.
    let expected_total = expected_interest + 10_000_000.0 + 50_000_000.0;
    assert!(
        (interest + principal - expected_total).abs() < 1.0,
        "tranche cash {} vs facility cash {expected_total}",
        interest + principal
    );

    let d = &run.diagnostics;
    assert_eq!(d.draws_from_reserve, usd(5_000_000.0));
    assert_eq!(d.draws_from_principal, usd(0.0));
    assert_eq!(d.unfunded_draws, usd(0.0));
    assert_eq!(d.reserve_replenished, usd(0.0));
    // The draw on 2024-03-01 takes the reserve to 45M, where it stays.
    let after_draw = d
        .reserve_balance_path
        .iter()
        .find(|(date, _)| *date >= date!(2024 - 03 - 01))
        .expect("period after the draw");
    assert_eq!(after_draw.1, usd(45_000_000.0));
    assert_eq!(
        d.reserve_balance_path.last().expect("last period").1,
        usd(45_000_000.0)
    );
}

/// Revolver repayments replenish the reserve toward its target before
/// counting as principal collections.
#[test]
fn revolver_repayments_replenish_the_reserve_toward_target() {
    let closing = date!(2024 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);
    let facility = fixed_revolver(closing, maturity);
    let market = MarketContext::new();
    let deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            revolvers: vec![facility],
            ..Default::default()
        },
        reserve: 50_000_000.0,
        reserve_target: Some(50_000_000.0),
        closing,
        maturity,
        senior: 9_000_000.0,
        equity: 1_000_000.0,
    });
    let run = run_simulation_with_diagnostics(&deal, &market, closing).expect("pool run");
    let d = &run.diagnostics;
    // The 3M repayment tops the reserve back up from 45M to 48M; the terminal
    // repayment then fills the remaining 2M of room, so 5M is diverted in all.
    assert_eq!(d.reserve_replenished, usd(5_000_000.0));
    let after_repayment = d
        .reserve_balance_path
        .iter()
        .find(|(date, _)| *date >= date!(2025 - 06 - 01))
        .expect("period after the repayment");
    assert_eq!(after_repayment.1, usd(48_000_000.0));
    assert_eq!(
        d.reserve_balance_path.last().expect("last period").1,
        usd(50_000_000.0)
    );
    // Replenished cash is released at deal end, so total note cash is unchanged.
    let (interest, principal) = totals(&run);
    let standalone_interest: f64 = fixed_revolver(closing, maturity)
        .raw_cashflow_schedule(&market, closing)
        .expect("schedule")
        .get_flows()
        .iter()
        .filter(|cf| cf.date > closing && !matches!(cf.kind, CFKind::Notional))
        .map(|cf| cf.amount.amount())
        .sum();
    assert!((interest + principal - (standalone_interest + 60_000_000.0)).abs() < 1.0);
}

/// A mixed pool (fixed, floating and callable bonds, a delayed-draw term loan
/// and a revolver) runs with every draw funded from the reserve; the
/// first-call policy redeems the callable bond early.
#[test]
fn mixed_pool_funds_draws_and_exercises_calls() {
    let closing = date!(2024 - 01 - 15);
    let maturity = date!(2034 - 01 - 15);
    let market = market_with_curves(closing);
    let collateral = |policy: CallExercisePolicy| InstrumentCollateral {
        bonds: vec![
            Bond::example().expect("fixed bond"),
            Bond::example_floating().expect("floating bond"),
            Bond::example_callable().expect("callable bond"),
        ],
        term_loans: vec![TermLoan::example_floating_with_ddtl().expect("ddtl")],
        revolvers: vec![{
            let mut facility = fixed_revolver(closing, date!(2027 - 01 - 15));
            facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![
                DrawRepayEvent {
                    date: date!(2024 - 03 - 01),
                    amount: usd(5_000_000.0),
                    is_draw: true,
                },
                DrawRepayEvent {
                    date: date!(2025 - 06 - 01),
                    amount: usd(3_000_000.0),
                    is_draw: false,
                },
            ]);
            facility
        }],
        call_exercise: policy,
        ..Default::default()
    };
    let build = |policy| {
        deal_with(DealSpec {
            collateral: collateral(policy),
            reserve: 30_000_000.0,
            reserve_target: None,
            closing,
            maturity,
            senior: 11_700_000.0,
            equity: 1_300_000.0,
        })
    };

    let contractual =
        run_simulation_with_diagnostics(&build(CallExercisePolicy::Contractual), &market, closing)
            .expect("contractual run");
    let first_call =
        run_simulation_with_diagnostics(&build(CallExercisePolicy::FirstCall), &market, closing)
            .expect("first-call run");

    for run in [&contractual, &first_call] {
        let d = &run.diagnostics;
        // Revolver draw 5M plus the two delayed draws (10M + 5M) after closing.
        assert_eq!(d.draws_from_reserve, usd(20_000_000.0));
        assert_eq!(d.draws_from_principal, usd(0.0));
        assert_eq!(d.unfunded_draws, usd(0.0));
        let final_reserve = d.reserve_balance_path.last().expect("last").1.amount();
        assert!(
            (final_reserve - 10_000_000.0).abs() < 1e-6,
            "final reserve {final_reserve}"
        );
    }

    // Calling the 4% bond at 103 in 2027 forgoes seven years of coupon for a
    // 3% premium, so the notes receive less cash in total under first-call.
    let (ic, pc) = totals(&contractual);
    let (if_, pf) = totals(&first_call);
    assert!(
        if_ + pf < ic + pc,
        "first call {} vs contractual {}",
        if_ + pf,
        ic + pc
    );
}

fn callable_eight_percent_bond(calls: Vec<CallPut>, puts: Vec<CallPut>) -> Bond {
    let mut bond = Bond::example().expect("bond");
    bond.id = InstrumentId::new("B-8");
    bond.cashflow_spec =
        CashflowSpec::fixed(0.08, Tenor::semi_annual(), DayCount::Thirty360).expect("coupon");
    bond.call_put = Some(CallPutSchedule { calls, puts });
    bond
}

fn one_day(date: Date, price: f64) -> CallPut {
    CallPut {
        start_date: date,
        end_date: date,
        price_pct_of_par: price,
        make_whole: None,
    }
}

fn first_senior_principal_date(run: &SimulationRun) -> Date {
    run.tranches["A"]
        .principal_flows
        .first()
        .map(|(date, _)| *date)
        .expect("senior principal")
}

/// Exercise policies on a single 8% callable bond against a 3% curve.
#[test]
fn exercise_policies_redeem_when_they_should() {
    let closing = date!(2024 - 01 - 15);
    let maturity = date!(2034 - 01 - 15);
    let call_date = date!(2026 - 01 - 15);
    let put_date = date!(2027 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve(0.03, closing, "USD-OIS"));

    let run_with = |policy: CallExercisePolicy, put: PutExercisePolicy, price: Option<f64>| {
        let mut bond = callable_eight_percent_bond(
            vec![one_day(call_date, 101.0)],
            vec![one_day(put_date, 100.0)],
        );
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = price;
        let deal = deal_with(DealSpec {
            collateral: InstrumentCollateral {
                bonds: vec![bond],
                call_exercise: policy,
                put_exercise: put,
                ..Default::default()
            },
            reserve: 0.0,
            reserve_target: None,
            closing,
            maturity,
            senior: 900_000.0,
            equity: 100_000.0,
        });
        run_simulation_with_diagnostics(&deal, &market, closing).expect("run")
    };

    // Contractual: principal only at maturity.
    let run = run_with(
        CallExercisePolicy::Contractual,
        PutExercisePolicy::Never,
        None,
    );
    assert!(first_senior_principal_date(&run) >= maturity);

    // Refinancing incentive: 8% coupon vs ~3% refinancing = 500 bp saving.
    let run = run_with(
        CallExercisePolicy::RefinancingIncentive {
            threshold_bp: 100.0,
        },
        PutExercisePolicy::Never,
        None,
    );
    assert_eq!(first_senior_principal_date(&run), call_date);
    let run = run_with(
        CallExercisePolicy::RefinancingIncentive {
            threshold_bp: 600.0,
        },
        PutExercisePolicy::Never,
        None,
    );
    assert!(first_senior_principal_date(&run) >= maturity);

    // First call redeems at the first window regardless of rates.
    let run = run_with(
        CallExercisePolicy::FirstCall,
        PutExercisePolicy::Never,
        None,
    );
    assert_eq!(first_senior_principal_date(&run), call_date);

    // Yield-to-worst: a 120 price makes the 101 call the worst outcome; a 90
    // price makes maturity the worst outcome.
    let run = run_with(
        CallExercisePolicy::Worst,
        PutExercisePolicy::Never,
        Some(120.0),
    );
    assert_eq!(first_senior_principal_date(&run), call_date);
    let run = run_with(
        CallExercisePolicy::Worst,
        PutExercisePolicy::Never,
        Some(90.0),
    );
    assert!(first_senior_principal_date(&run) >= maturity);

    // First put redeems at the put window.
    let run = run_with(
        CallExercisePolicy::Contractual,
        PutExercisePolicy::FirstPut,
        None,
    );
    assert_eq!(first_senior_principal_date(&run), put_date);
}

/// A delayed-draw calendar the reserve cannot fund fails at preparation
/// naming the shortfall date.
#[test]
fn unfundable_draw_calendar_fails_at_preparation() {
    let closing = date!(2024 - 01 - 15);
    let market = market_with_curves(closing);
    let deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            term_loans: vec![TermLoan::example_floating_with_ddtl().expect("ddtl")],
            ..Default::default()
        },
        reserve: 1_000_000.0,
        reserve_target: None,
        closing,
        maturity: date!(2031 - 01 - 15),
        senior: 9_000_000.0,
        equity: 1_000_000.0,
    });
    let err = run_simulation_with_diagnostics(&deal, &market, closing)
        .expect_err("10M draw against a 1M reserve must fail");
    let message = err.to_string();
    assert!(
        message.contains("exceed the reserve") && message.contains("2024-04-15"),
        "{message}"
    );
}
