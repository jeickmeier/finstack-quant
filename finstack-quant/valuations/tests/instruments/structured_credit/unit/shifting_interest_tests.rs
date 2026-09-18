//! Shifting interest with the prospectus reading of the schedule: each step
//! is the share of the subordinates' pro-rata unscheduled principal that
//! shifts to the senior, and a failing performance trigger reverts to the
//! full lockout. The 2026-09-17 audit found the schedule read as the
//! senior's share outright and no gating at all.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, DealType, PoolAsset, ShiftMode,
    ShiftingInterestSpec, ShiftingInterestStep, SimulationRun, StepDownTrigger, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallRules,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// 100M bullet pool at 6%, CPR 20%, no defaults; A 90M / M 10M; full
/// lockout for 36 months then the given step, read per `mode`.
fn rmbs(step: f64, mode: ShiftMode, triggers: Vec<StepDownTrigger>) -> StructuredCredit {
    let maturity = d(2034, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Rmbs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.06,
        maturity,
        DayCount::Thirty360,
    ));
    let tr = |id: &str, a: f64, b: f64, sen: TrancheSeniority, bal: f64, cpn: f64| {
        Tranche::new(
            id,
            a,
            b,
            sen,
            usd(bal),
            TrancheCoupon::Fixed { rate: cpn },
            maturity,
        )
        .expect("tranche")
    };
    let tranches = TrancheStructure::new(vec![
        tr("A", 0.0, 90.0, TrancheSeniority::Senior, 90_000_000.0, 0.05),
        tr(
            "M",
            90.0,
            100.0,
            TrancheSeniority::Mezzanine,
            10_000_000.0,
            0.07,
        ),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_rmbs("RMBS-SHIFT", pool, tranches, close(), maturity, "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal.waterfall_rules = Some(WaterfallRules {
        shifting_interest: Some(
            ShiftingInterestSpec::new(
                "A",
                vec![
                    ShiftingInterestStep {
                        months_from_closing: 0,
                        senior_pct: 1.0,
                    },
                    ShiftingInterestStep {
                        months_from_closing: 36,
                        senior_pct: step,
                    },
                ],
            )
            .with_mode(mode)
            .with_triggers(triggers),
        ),
        ..Default::default()
    });
    deal
}

fn simulate(deal: &StructuredCredit) -> SimulationRun {
    run_simulation_with_diagnostics(deal, &market(), close()).expect("simulation")
}

/// Month-37 principal to A and the pool prepayment that period, plus A's
/// live share of the notes at the period's opening.
fn month_37(run: &SimulationRun) -> (f64, f64, f64) {
    let period = &run.diagnostics.periods[36];
    assert!(
        period.payment_date >= d(2027, 2, 1) && period.payment_date <= d(2027, 2, 3),
        "month 37 is the February 2027 date: {}",
        period.payment_date
    );
    let paid_before = |id: &str| -> f64 {
        run.tranches[id]
            .principal_flows
            .iter()
            .filter(|(date, _)| *date < period.payment_date)
            .map(|(_, amount)| amount.amount())
            .sum()
    };
    // The 36-month step already applies to the January 2027 date, so M's
    // live balance at month 37 is a touch below par.
    let a_opening = 90_000_000.0 - paid_before("A");
    let m_opening = 10_000_000.0 - paid_before("M");
    let senior_share = a_opening / (a_opening + m_opening);
    let a_paid = run.tranches["A"]
        .principal_flows
        .iter()
        .filter(|(date, _)| *date == period.payment_date)
        .map(|(_, amount)| amount.amount())
        .sum();
    (a_paid, period.principal_collections.amount(), senior_share)
}

/// Prospectus reading: a 70% step shifts 70% of the subordinate's pro-rata
/// share to the senior, so A's month-37 principal is
/// `(senior_pct + 0.7 × (1 − senior_pct)) × prepayment` on live balances
/// (about `0.805 + 0.7 × 0.195` after three years of full lockout).
#[test]
fn shift_of_subordinate_reads_the_step_as_the_shifted_share() {
    let run = simulate(&rmbs(0.70, ShiftMode::ShiftOfSubordinate, Vec::new()));
    let (a_paid, prepayment, senior_share) = month_37(&run);
    assert!(
        (senior_share - 0.805).abs() < 0.01,
        "three years of lockout leave A at about 80.5% of the notes: {senior_share}"
    );
    let expected = (senior_share + 0.7 * (1.0 - senior_share)) * prepayment;
    assert!(
        (a_paid - expected).abs() < 1.0,
        "A takes its pro-rata share plus 70% of M's: {a_paid} vs {expected}"
    );
    let m_paid = prepayment - a_paid;
    assert!(
        m_paid > 1_000.0,
        "M receives the unshifted 30% of its share"
    );

    // The senior-share reading pays A 70% of the prepayment outright.
    let run = simulate(&rmbs(0.70, ShiftMode::SeniorShare, Vec::new()));
    let (a_paid, prepayment, _) = month_37(&run);
    assert!(
        (a_paid - 0.7 * prepayment).abs() < 1.0,
        "senior-share mode: {a_paid} vs 0.7 × {prepayment}"
    );
}

/// While a performance trigger fails the shift reverts to the full lockout;
/// a passing trigger leaves the schedule in force.
#[test]
fn a_failing_trigger_reverts_to_the_full_lockout() {
    // Senior credit enhancement is (pool − A) / pool ≈ 20%: a 99% floor fails
    // every period, a 1% floor passes.
    let locked = simulate(&rmbs(
        0.70,
        ShiftMode::ShiftOfSubordinate,
        vec![StepDownTrigger::MinCreditEnhancement(0.99)],
    ));
    let (a_paid, prepayment, _) = month_37(&locked);
    assert!(
        (a_paid - prepayment).abs() < 1.0,
        "a failing trigger locks the subordinate out: {a_paid} vs {prepayment}"
    );
    let m_total: f64 = locked.tranches["M"]
        .principal_flows
        .iter()
        .filter(|(date, _)| *date < d(2033, 12, 1))
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!(
        m_total < 1.0,
        "M receives nothing before maturity: {m_total}"
    );

    let passing = simulate(&rmbs(
        0.70,
        ShiftMode::ShiftOfSubordinate,
        vec![StepDownTrigger::MinCreditEnhancement(0.01)],
    ));
    let (a_paid, prepayment, senior_share) = month_37(&passing);
    let expected = (senior_share + 0.7 * (1.0 - senior_share)) * prepayment;
    assert!(
        (a_paid - expected).abs() < 1.0,
        "a passing trigger leaves the schedule in force: {a_paid} vs {expected}"
    );
}
