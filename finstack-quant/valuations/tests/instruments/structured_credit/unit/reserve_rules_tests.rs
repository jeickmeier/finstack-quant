//! Reserve account rules in the template waterfall: per-period target,
//! replenishment from excess interest, release of the excess above the
//! target, and the final-date principal cover. The 2026-09-17 audit found
//! the template had no reserve replenishment, target or release at all.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, DealType, PoolAsset, ReserveAccountSpec,
    ReserveTarget, SimulationRun, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure, WaterfallRules,
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

fn maturity() -> Date {
    d(2029, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=10)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Monthly ABS: ten 1M five-year 8% bullets (30/360), senior class `senior`
/// at 4%, equity for the rest, no fees, zero recovery paid immediately.
fn abs(
    senior: f64,
    cpr: f64,
    cdr: f64,
    initial_reserve: f64,
    spec: ReserveAccountSpec,
) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(1_000_000.0),
            0.08,
            maturity(),
            DayCount::Thirty360,
        ));
    }
    pool.reserve_account = usd(initial_reserve);
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            senior / 100_000.0,
            TrancheSeniority::Senior,
            usd(senior),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            senior / 100_000.0,
            100.0,
            TrancheSeniority::Equity,
            usd(10_000_000.0 - senior),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal = StructuredCredit::new_abs(
        "ABS-RESERVE-RULES",
        pool,
        tranches,
        close(),
        maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal.waterfall_rules = Some(WaterfallRules {
        reserve: Some(spec),
        ..Default::default()
    });
    deal
}

fn simulate(deal: &StructuredCredit) -> SimulationRun {
    run_simulation_with_diagnostics(deal, &market(), close()).expect("simulation")
}

fn two_pct_of_current_with_one_pct_floor() -> ReserveTarget {
    ReserveTarget::Max(
        Box::new(ReserveTarget::PctOfCurrent(0.02)),
        Box::new(ReserveTarget::PctOfOriginal(0.01)),
    )
}

/// Excess interest (8% on 10M less the 4% senior coupon, about 36k a month)
/// replenishes the reserve ahead of the residual until the 2%-of-current
/// target (200k) is met; equity receives nothing until then.
#[test]
fn replenishment_fills_the_reserve_from_excess_interest_to_the_target() {
    let deal = abs(
        9_000_000.0,
        0.0,
        0.0,
        0.0,
        ReserveAccountSpec::new(two_pct_of_current_with_one_pct_floor()),
    );
    let run = simulate(&deal);
    let periods = &run.diagnostics.periods;
    // Pool interest is 30/360 (66,667 a month); the notes accrue Act/360, so
    // the excess is read off the senior's actual coupon.
    let excess = |period: usize| {
        10_000_000.0 * 0.08 / 12.0 - run.tranches["A"].interest_flows[period].1.amount()
    };
    let monthly_excess = excess(0);
    assert!(
        (periods[0].reserve_balance.amount() - monthly_excess).abs() < 1.0,
        "period 1 captures the whole excess: {}",
        periods[0].reserve_balance.amount()
    );
    assert!(
        (periods[5].reserve_balance.amount() - 200_000.0).abs() < 1.0,
        "the target is reached by period 6: {}",
        periods[5].reserve_balance.amount()
    );
    assert!(
        (periods[6].reserve_balance.amount() - 200_000.0).abs() < 1.0,
        "the account stays at the target: {}",
        periods[6].reserve_balance.amount()
    );
    // Equity's flow list starts only when it first receives cash: nothing
    // before the target is met (July), then the whole excess.
    let equity_flows = &run.tranches["E"].interest_flows;
    assert!(
        equity_flows.iter().all(|(date, _)| *date >= d(2024, 7, 1)),
        "equity is behind the replenishment: {equity_flows:?}"
    );
    let august_equity = equity_flows
        .iter()
        .find(|(date, _)| *date == d(2024, 8, 1))
        .map(|(_, amount)| amount.amount())
        .expect("August residual");
    assert!(
        (august_equity - excess(6)).abs() < 1.0,
        "once the target is met the excess reaches equity: {august_equity}"
    );
}

/// A reserve funded above its target releases the excess into interest
/// proceeds (to the residual holder once coupons are covered), and as the
/// pool amortizes the 2%-of-current target falls to the 1%-of-original floor.
#[test]
fn excess_above_the_target_is_released_and_the_target_amortizes_to_its_floor() {
    let releasing = abs(
        9_000_000.0,
        0.30,
        0.0,
        500_000.0,
        ReserveAccountSpec::new(two_pct_of_current_with_one_pct_floor()),
    );
    let run = simulate(&releasing);
    let periods = &run.diagnostics.periods;
    assert!(
        (periods[0].reserve_balance.amount() - 200_000.0).abs() < 1.0,
        "the 300k above target is released in period 1: {}",
        periods[0].reserve_balance.amount()
    );
    let monthly_excess =
        10_000_000.0 * 0.08 / 12.0 - run.tranches["A"].interest_flows[0].1.amount();
    let first_equity = run.tranches["E"].interest_flows[0].1.amount();
    assert!(
        (first_equity - (300_000.0 + monthly_excess)).abs() < 1.0,
        "equity receives the release plus the period's excess: {first_equity}"
    );
    // 2% of the amortizing pool falls below the 1%-of-original floor once the
    // pool is under 5M; the balance tracks the target down and never below
    // the floor.
    // The target is resolved on the pool balance at the period's opening
    // (the previous period's closing balance).
    let floor_hit = periods
        .windows(2)
        .find(|pair| pair[0].pool_balance.amount() < 5_000_000.0)
        .expect("the pool amortizes below 5M at 30% CPR");
    assert!(
        (floor_hit[1].reserve_balance.amount() - 100_000.0).abs() < 1.0,
        "at the floor the reserve holds 1% of the original pool: {}",
        floor_hit[1].reserve_balance.amount()
    );
    let above_floor = periods
        .windows(2)
        .find(|pair| pair[0].pool_balance.amount() < 8_000_000.0)
        .expect("period with the pool under 8M");
    assert!(
        (above_floor[1].reserve_balance.amount() - 0.02 * above_floor[0].pool_balance.amount())
            .abs()
            < 1.0,
        "above the floor the reserve tracks 2% of the opening pool: {} vs {}",
        above_floor[1].reserve_balance.amount(),
        0.02 * above_floor[0].pool_balance.amount()
    );

    let mut keeping = ReserveAccountSpec::new(two_pct_of_current_with_one_pct_floor());
    keeping.release_excess = false;
    let kept = simulate(&abs(9_000_000.0, 0.30, 0.0, 500_000.0, keeping));
    assert!(
        (kept.diagnostics.periods[0].reserve_balance.amount() - 500_000.0).abs() < 1.0,
        "without release the balance stays put: {}",
        kept.diagnostics.periods[0].reserve_balance.amount()
    );
}

/// With defaults the pool returns less principal than the 9.8M senior is
/// owed; at legal final the reserve covers the shortfall by default, or goes
/// to the residual holder when the rules say so.
#[test]
fn final_date_principal_shortfall_is_covered_by_the_reserve() {
    let covering = abs(
        9_800_000.0,
        0.0,
        0.01,
        0.0,
        ReserveAccountSpec::new(ReserveTarget::Fixed(usd(200_000.0))),
    );
    let covered = simulate(&covering);
    let mut releasing = ReserveAccountSpec::new(ReserveTarget::Fixed(usd(200_000.0)));
    releasing.covers_principal_at_final = false;
    let released = simulate(&abs(9_800_000.0, 0.0, 0.01, 0.0, releasing));

    let a_covered = covered.tranches["A"].total_principal.amount();
    let a_released = released.tranches["A"].total_principal.amount();
    assert!(
        a_released < 9_800_000.0 - 100_000.0,
        "defaults leave the senior short of its 9.8M without the reserve: {a_released}"
    );
    assert!(
        (a_covered - a_released - 200_000.0).abs() < 1.0,
        "the 200k reserve retires senior principal at legal final: {a_covered} vs {a_released}"
    );
    let e_covered = covered.tranches["E"].total_principal.amount();
    let e_released = released.tranches["E"].total_principal.amount();
    assert!(
        (e_released - e_covered - 200_000.0).abs() < 1.0,
        "without the cover the reserve reaches the residual holder: {e_released} vs {e_covered}"
    );
}
