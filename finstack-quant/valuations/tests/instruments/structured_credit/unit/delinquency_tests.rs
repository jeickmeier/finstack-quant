//! Roll-rate delinquency, servicer advancing and the `MaxDelinquency`
//! step-down trigger through the deterministic engine, checked against
//! hand-computed bucket recursions.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AdvancingPolicy, AssetPool, DealType, DelinquencyModel, PoolAsset,
    StepDownSpec, StepDownTrigger, StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon,
    TrancheSeniority, TrancheStructure, WaterfallRules,
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

/// Monthly ABS: one 1M five-year 6% bullet (30/360) entering delinquency at
/// 1% of the performing balance a month, A 900k at 6% / E 100k, no fees, no
/// prepayments, zero recovery paid immediately.
fn abs(model: DelinquencyModel) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    let mut loan = PoolAsset::fixed_rate_bond(
        "L1",
        usd(1_000_000.0),
        0.06,
        maturity(),
        DayCount::Thirty360,
    );
    loan.mdr_override = Some(0.01);
    pool.assets.push(loan);
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(900_000.0),
            TrancheCoupon::Fixed { rate: 0.06 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(100_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_abs("ABS-DQ", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal.credit_model.delinquency = Some(model);
    deal
}

fn simulate(deal: &StructuredCredit) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market(), close()).expect("simulation")
}

/// Pool interest collections per payment date: everything the interest
/// waterfall distributes (the deal has no fees), in payment order.
fn collections(results: &HashMap<String, TrancheCashflows>) -> Vec<(Date, f64)> {
    let mut by_date: std::collections::BTreeMap<Date, f64> = std::collections::BTreeMap::new();
    for id in ["A", "E"] {
        for (date, amount) in &results[id].interest_flows {
            *by_date.entry(*date).or_default() += amount.amount();
        }
    }
    by_date.into_iter().collect()
}

/// 30/360 accrual between two business-day-adjusted payment dates, which is
/// what the pool earns: a payment date rolled off a weekend lengthens one
/// period and shortens the next.
fn accrual(from: Date, to: Date) -> f64 {
    DayCount::Thirty360
        .year_fraction(from, to, DayCountContext::default())
        .expect("accrual")
}

fn first_deferral(results: &HashMap<String, TrancheCashflows>) -> Option<Date> {
    results["A"].deferred_flows.first().map(|(date, _)| *date)
}

/// With every bucket rolling and nothing curing, a balance that turns
/// delinquent in month t charges off in month t + 3. The performing balance
/// is `0.99^t`, interest is earned on it (entries earn half a month), and
/// the 1M − 0.99^60 charged off by legal final is the senior note's
/// shortfall.
#[test]
fn roll_rates_reproduce_the_hand_computed_schedule() {
    let deal = abs(DelinquencyModel::new(
        vec![1.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0],
    ));
    let results = simulate(&deal);

    let collected = collections(&results);
    let mut performing = 1_000_000.0_f64;
    let mut prev = close();
    for (month, (date, actual)) in collected.iter().take(12).enumerate() {
        let expected = performing * 0.06 * accrual(prev, *date) * (1.0 - 0.5 * 0.01);
        assert!(
            (actual - expected).abs() < 0.02,
            "month {}: collected {actual} vs hand {expected}",
            month + 1
        );
        performing *= 0.99;
        prev = *date;
    }

    let survivor = 1_000_000.0 * 0.99_f64.powi(60);
    let principal = results["A"].total_principal.amount();
    let writedown = results["A"].total_writedown.amount();
    assert!(
        (principal - survivor).abs() < 1.0,
        "the surviving balance is the senior balloon: {principal} vs {survivor}"
    );
    assert!(
        (writedown - (900_000.0 - survivor)).abs() < 1.0,
        "charge-offs are the senior shortfall at legal final: {writedown}"
    );
    assert_eq!(results["E"].total_principal.amount(), 0.0);
}

/// Half of every bucket cures and half rolls each month: the cured balance
/// resumes paying, so interest follows the recursion, not the no-cure path.
#[test]
fn cures_return_balance_to_current_in_the_period_they_happen() {
    let deal = abs(DelinquencyModel::new(
        vec![0.5, 0.5, 0.5],
        vec![0.5, 0.5, 0.5],
    ));
    let results = simulate(&deal);

    let collected = collections(&results);
    let mut performing = 1_000_000.0_f64;
    let mut buckets = [0.0_f64; 3];
    let mut prev = close();
    for (month, (date, actual)) in collected.iter().take(12).enumerate() {
        let entry = performing * 0.01;
        let expected = (performing - 0.5 * entry) * 0.06 * accrual(prev, *date);
        prev = *date;
        assert!(
            (actual - expected).abs() < 0.02,
            "month {}: collected {actual} vs hand {expected}",
            month + 1
        );
        performing -= entry;
        let mut inflow = entry;
        let mut cured = 0.0;
        let mut next = [0.0_f64; 3];
        for (bucket, opening) in buckets.iter().enumerate() {
            let roll = opening * 0.5;
            cured += opening * 0.5;
            next[bucket] = inflow;
            inflow = roll;
        }
        performing += cured;
        buckets = next;
    }
    assert!(
        results["A"].total_writedown.amount() < 900_000.0 - 1_000_000.0 * 0.99_f64.powi(60),
        "cures must leave more collateral than the roll-everything schedule"
    );
}

/// Advancing the P&I delinquent balances miss keeps the senior coupon
/// current while collections fall; a zero recoverability cap advances
/// nothing and reproduces the no-advancing deal.
#[test]
fn servicer_advancing_keeps_senior_interest_current() {
    let no_advancing = abs(DelinquencyModel::new(
        vec![1.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0],
    ));
    let advancing = abs(
        DelinquencyModel::new(vec![1.0, 1.0, 1.0], vec![0.0, 0.0, 0.0]).with_advancing(
            AdvancingPolicy::PrincipalAndInterest {
                recoverability_cap_pct: 100.0,
            },
        ),
    );
    let capped_out = abs(
        DelinquencyModel::new(vec![1.0, 1.0, 1.0], vec![0.0, 0.0, 0.0]).with_advancing(
            AdvancingPolicy::PrincipalAndInterest {
                recoverability_cap_pct: 0.0,
            },
        ),
    );

    let without = simulate(&no_advancing);
    let with = simulate(&advancing);
    let capped = simulate(&capped_out);

    let deferral_without = first_deferral(&without).expect("collections fall below the coupon");
    let deferral_with = first_deferral(&with).expect("advances run out eventually");
    assert!(
        deferral_with > deferral_without,
        "advancing must defer the first shortfall: {deferral_with} vs {deferral_without}"
    );
    assert!(
        with["A"]
            .deferred_flows
            .iter()
            .all(|(date, _)| *date > d(2025, 1, 1)),
        "no senior shortfall in the first year with advancing"
    );
    assert!(with["A"].total_interest.amount() > without["A"].total_interest.amount());
    assert_eq!(capped["A"].interest_flows, without["A"].interest_flows);
}

/// A `MaxDelinquency` step-down trigger blocks pro-rata principal while the
/// delinquent share is above the limit and releases it once cures bring the
/// share back down: the subordinate note receives more principal than under
/// a sequential deal and less than under an always-passing trigger.
#[test]
fn max_delinquency_step_down_trigger_fires_and_reverts() {
    fn deal(trigger: Option<StepDownTrigger>) -> StructuredCredit {
        let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L1",
            usd(1_000_000.0),
            0.06,
            maturity(),
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                70.0,
                TrancheSeniority::Senior,
                usd(700_000.0),
                TrancheCoupon::Fixed { rate: 0.04 },
                maturity(),
            )
            .expect("SR"),
            Tranche::new(
                "SUB",
                70.0,
                90.0,
                TrancheSeniority::Mezzanine,
                usd(200_000.0),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .expect("SUB"),
            Tranche::new(
                "EQ",
                90.0,
                100.0,
                TrancheSeniority::Equity,
                usd(100_000.0),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .expect("EQ"),
        ])
        .expect("structure");
        let mut deal =
            StructuredCredit::new_abs("ABS-DQ-SD", pool, tranches, close(), maturity(), "USD-OIS")
                .with_payment_calendar("nyse");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
        // Heavy entries for one year, then none: delinquency climbs past 5%
        // and decays back below it as balances cure or charge off.
        let mut curve = vec![0.50; 12];
        curve.push(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::vector(curve);
        deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        deal.credit_model.delinquency = Some(DelinquencyModel::new(
            vec![0.3, 0.3, 0.3],
            vec![0.5, 0.5, 0.5],
        ));
        if let Some(trigger) = trigger {
            deal.waterfall_rules = Some(WaterfallRules {
                afc: None,
                excess_spread: None,
                step_down: Some(StepDownSpec {
                    step_down_date: d(2024, 7, 1),
                    triggers: vec![trigger],
                }),
                shifting_interest: None,
                early_amortization: None,
                controlled_accumulation: None,
            });
        }
        deal
    }
    fn sub_principal_before(deal: &StructuredCredit, cutoff: Date) -> f64 {
        simulate(deal)["SUB"]
            .principal_flows
            .iter()
            .filter(|(date, _)| *date < cutoff)
            .map(|(_, amount)| amount.amount())
            .sum()
    }

    let cutoff = d(2026, 1, 1);
    let sequential = sub_principal_before(&deal(None), cutoff);
    let always = sub_principal_before(&deal(Some(StepDownTrigger::MaxDelinquency(1.0))), cutoff);
    let gated = sub_principal_before(&deal(Some(StepDownTrigger::MaxDelinquency(0.05))), cutoff);

    assert!(
        sequential < gated && gated < always,
        "trigger must fire while delinquency > 5% and revert afterwards: sequential {sequential}, gated {gated}, always {always}"
    );
}
