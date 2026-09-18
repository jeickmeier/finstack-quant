//! Credit-card master trust: payment-rate principal, portfolio-yield
//! interest, charge-offs, the excess-spread early-amortization test and
//! controlled accumulation, checked against hand computations.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AbsExcessSpreadCalculator, AbsPaymentRateCalculator, AssetPool,
    CardPortfolioSpec, ControlledAccumulationSpec, DealFees, DealType, EarlyAmortizationSpec,
    PoolAsset, ReinvestmentCriteria, ReinvestmentPeriod, StructuredCredit, Tranche,
    TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallRules,
};
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
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

/// 15% monthly payment rate, 18% portfolio yield, `charge_off` annual charge-offs.
fn card(charge_off: f64) -> CardPortfolioSpec {
    CardPortfolioSpec::new(0.15, 0.18, charge_off)
}

/// Monthly card master trust: 100M of receivables as ten 10M lines, A 90M at
/// a 5% fixed coupon, E 10M, no fees, no recoveries, revolving until
/// `revolving_end` with every note held flat.
fn trust(spec: CardPortfolioSpec, revolving_end: Date) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Card, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("R{i}"),
            usd(10_000_000.0),
            0.0,
            maturity(),
            DayCount::Thirty360,
        ));
    }
    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: revolving_end,
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
        amortizing_tranches: Vec::new(),
        assumptions: None,
    });
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(90_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(10_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_abs("CARD-MT", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse")
            .with_fees(fees(0.0));
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal.credit_model.card = Some(spec);
    deal
}

fn fees(servicing_fee_bp: f64) -> DealFees {
    DealFees {
        trustee_fee_annual: usd(0.0),
        senior_mgmt_fee_bp: 0.0,
        subordinated_mgmt_fee_bp: 0.0,
        servicing_fee_bp,
        master_servicer_fee_bp: None,
        special_servicer_fee_bp: None,
        workout_fee_pct: None,
        liquidation_fee_pct: None,
        incentive_fee: None,
    }
}

fn with_rules(mut deal: StructuredCredit, rules: WaterfallRules) -> StructuredCredit {
    deal.waterfall_rules = Some(rules);
    deal
}

fn no_rules() -> WaterfallRules {
    WaterfallRules {
        afc: None,
        excess_spread: None,
        step_down: None,
        shifting_interest: None,
        early_amortization: None,
        controlled_accumulation: None,
        reserve: None,
        target_oc: None,
    }
}

fn simulate(deal: &StructuredCredit) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market(), close()).expect("simulation")
}

fn first_principal(results: &HashMap<String, TrancheCashflows>, id: &str) -> Option<Date> {
    results[id]
        .principal_flows
        .iter()
        .find(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, _)| *date)
}

fn metric_context(deal: StructuredCredit) -> MetricContext {
    MetricContext::new(
        Arc::new(deal),
        Arc::new(market()),
        close(),
        usd(0.0),
        MetricContext::default_config(),
    )
}

/// Excess spread = yield − weighted note coupon − servicing − charge-offs:
/// 18% − (90M × 5%) / 100M − 1% − 5% = 7.5% per annum; the payment rate
/// metric reports the monthly rate in percent.
#[test]
fn static_excess_spread_and_payment_rate_metrics_match_the_hand_computation() {
    let deal = trust(card(0.05), d(2027, 1, 1)).with_fees(fees(100.0));

    let excess = AbsExcessSpreadCalculator
        .calculate(&mut metric_context(deal.clone()))
        .expect("excess spread");
    let payment_rate = AbsPaymentRateCalculator
        .calculate(&mut metric_context(deal))
        .expect("payment rate");

    assert!((excess - 7.5).abs() < 1e-9, "excess spread {excess}");
    assert!(
        (payment_rate - 15.0).abs() < 1e-12,
        "payment rate {payment_rate}"
    );

    let mut plain = trust(card(0.05), d(2027, 1, 1));
    plain.credit_model.card = None;
    assert!(AbsExcessSpreadCalculator
        .calculate(&mut metric_context(plain))
        .is_err());
}

/// During the revolving period cardholder payments are recycled and the
/// notes are held flat; the investor interest earns the portfolio yield.
/// With a positive excess spread the `min_excess_spread_3m` test never fires
/// and the first note principal is the revolving-period end. With charge-offs
/// that push the realized excess spread negative, three periods below the
/// floor end the revolving period: the fourth payment date pays principal.
#[test]
fn three_periods_of_negative_excess_spread_trigger_early_amortization() {
    let rules = |floor: Option<f64>| WaterfallRules {
        early_amortization: Some(EarlyAmortizationSpec {
            max_cumulative_loss: Some(1.0),
            min_excess_spread_3m: floor,
        }),
        ..no_rules()
    };
    let revolving_end = d(2027, 1, 1);

    let healthy = simulate(&with_rules(
        trust(card(0.05), revolving_end),
        rules(Some(0.0)),
    ));
    let stressed = simulate(&with_rules(
        trust(card(0.25), revolving_end),
        rules(Some(0.0)),
    ));
    let untested = simulate(&with_rules(trust(card(0.25), revolving_end), rules(None)));

    // Portfolio yield on the flat 100M investor interest for a 30/360 month
    // (no fees in this trust, so the notes receive every dollar of it).
    let month_1_interest: f64 = ["A", "E"]
        .into_iter()
        .map(|id| healthy[id].interest_flows[0].1.amount())
        .sum();
    assert!(
        (month_1_interest
            - 100_000_000.0 * 0.18 / 12.0 * (1.0 - 0.5 * (1.0 - 0.95_f64.powf(1.0 / 12.0))))
        .abs()
            < 1.0,
        "month 1 interest is the portfolio yield net of the charge-off haircut: {month_1_interest}"
    );

    assert!(
        first_principal(&healthy, "A").expect("amortization after revolving") >= revolving_end,
        "positive excess spread keeps the deal revolving"
    );
    assert!(
        first_principal(&untested, "A").expect("amortization after revolving") >= revolving_end,
        "without the test the stressed deal still revolves"
    );
    assert_eq!(
        first_principal(&stressed, "A"),
        Some(d(2024, 5, 1)),
        "three periods below the floor end the revolving period at the fourth payment date"
    );
}

/// Once the revolving period ends the investor allocation is fixed on the
/// receivables held at that point (100M, no seller interest), so every
/// accumulation date collects `15% × 100M = 15M` of investor principal
/// regardless of how far the investor interest has amortized. With the NYSE
/// calendar the 2026-01-01 date rolls to 2026-01-02, past the revolving end,
/// so seven dates accumulate before the bullet: 105M capped at the 100M
/// investor interest, of which Class A takes its full 90M as one bullet on
/// the bullet date and the equity the rest. Charge-offs reduce the investor
/// interest on the fixed base and therefore only the equity's principal.
#[test]
fn controlled_accumulation_releases_the_fixed_allocation_principal_as_a_bullet() {
    let revolving_end = d(2026, 1, 1);
    let bullet_date = d(2026, 7, 1);
    let accumulating = |charge_off: f64| {
        with_rules(
            trust(card(charge_off), revolving_end),
            WaterfallRules {
                controlled_accumulation: Some(ControlledAccumulationSpec {
                    start_date: revolving_end,
                    bullet_date,
                }),
                ..no_rules()
            },
        )
    };
    let positive = |flows: &[(Date, Money)]| -> Vec<(Date, f64)> {
        flows
            .iter()
            .filter(|(_, amount)| amount.amount() > 0.0)
            .map(|(date, amount)| (*date, amount.amount()))
            .collect()
    };

    let results = simulate(&accumulating(0.0));
    let a_flows = positive(&results["A"].principal_flows);
    assert_eq!(a_flows.len(), 1, "one bullet: {a_flows:?}");
    assert!(
        a_flows[0].0 >= bullet_date && a_flows[0].0 < d(2026, 8, 1),
        "the bullet is released on the first payment date at or after the bullet date: {:?}",
        a_flows[0]
    );
    assert!(
        (a_flows[0].1 - 90_000_000.0).abs() < 1.0,
        "the fixed 15M monthly allocation funds the whole 90M class: {}",
        a_flows[0].1
    );
    let e_total: f64 = positive(&results["E"].principal_flows)
        .iter()
        .map(|(_, amount)| amount)
        .sum();
    assert!(
        (e_total - 10_000_000.0).abs() < 1.0,
        "the equity receives the remaining investor interest: {e_total}"
    );

    // Two years of revolving charge-offs leave the investor interest below
    // the 90M class at the freeze, and the accumulation months keep charging
    // off the fixed base, so the bullet is short by the unreimbursed
    // charge-offs (no excess-spread reimbursement in this trust): the class
    // takes a loss and the equity receives no principal.
    let with_charge_offs = simulate(&accumulating(0.05));
    let a_flows = positive(&with_charge_offs["A"].principal_flows);
    assert_eq!(a_flows.len(), 1, "one short bullet: {a_flows:?}");
    assert!(
        a_flows[0].1 < 90_000_000.0 && a_flows[0].1 > 85_000_000.0,
        "the bullet is the investor interest net of charge-offs: {}",
        a_flows[0].1
    );
    let e_with_charge_offs: f64 = positive(&with_charge_offs["E"].principal_flows)
        .iter()
        .map(|(_, amount)| amount)
        .sum();
    assert!(
        e_with_charge_offs < 1.0,
        "the equity is wiped out before the senior completes: {e_with_charge_offs}"
    );
}

/// Early amortization pays the investor interest down at the fixed
/// allocation of the payment rate on the receivables held when the event
/// fires: after the trigger period every paydown is `15% × opening balance
/// of the trigger period` until the investor interest is retired within
/// `⌈1 / MPR⌉ = 7` payment dates of the trigger.
#[test]
fn early_amortization_pays_down_at_the_fixed_allocation_of_the_payment_rate() {
    let deal = with_rules(
        trust(card(0.25), d(2027, 1, 1)),
        WaterfallRules {
            early_amortization: Some(EarlyAmortizationSpec {
                max_cumulative_loss: Some(1.0),
                min_excess_spread_3m: Some(0.0),
            }),
            ..no_rules()
        },
    );
    let run = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
        &deal,
        &market(),
        close(),
    )
    .expect("simulation");
    let a_flows: Vec<(Date, f64)> = run.tranches["A"]
        .principal_flows
        .iter()
        .filter(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, amount)| (*date, amount.amount()))
        .collect();
    assert_eq!(
        a_flows[0].0,
        d(2024, 5, 1),
        "early amortization starts at the fourth date"
    );
    // The event fires after the trigger period's flows, which ran on the
    // live balance (payment rate on the post-charge-off balance); the base
    // is then fixed at that period's opening balance for every later date.
    let opening = run.diagnostics.periods[2].pool_balance.amount();
    let trigger_period_defaults = run.diagnostics.periods[3].defaults.amount();
    assert!(
        (a_flows[0].1 - 0.15 * (opening - trigger_period_defaults)).abs() < 1.0,
        "trigger-period paydown is the payment rate on the live balance ({opening} less {trigger_period_defaults}): {}",
        a_flows[0].1
    );
    for flow in &a_flows[1..a_flows.len() - 1] {
        assert!(
            (flow.1 - 0.15 * opening).abs() < 1.0,
            "the fixed base repeats the payment rate on the trigger period's opening balance {opening}: {flow:?}"
        );
    }
    assert!(
        a_flows.len() <= 7,
        "the investor interest is retired within ⌈1 / MPR⌉ dates: {a_flows:?}"
    );
    // 25% charge-offs on the fixed base keep eroding the investor interest
    // while it pays down, and nothing reimburses them in this trust: the
    // class collects the opening balance less every later charge-off and
    // takes the rest as a loss; the equity receives nothing.
    let later_charge_offs: f64 = run.diagnostics.periods[3..]
        .iter()
        .map(|period| period.defaults.amount())
        .sum();
    let total_a: f64 = a_flows.iter().map(|(_, amount)| amount).sum();
    assert!(
        (total_a - (opening - later_charge_offs)).abs() < 1.0,
        "A collects the investor interest net of charge-offs {}: {total_a}",
        opening - later_charge_offs
    );
    assert!(
        total_a < 90_000_000.0,
        "A takes the unreimbursed charge-offs as a loss"
    );
    let e_principal: f64 = run.tranches["E"]
        .principal_flows
        .iter()
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!(e_principal < 1.0, "the equity is wiped out: {e_principal}");
}

/// With a seller interest and a supplied allocation the investor base is
/// the allocation of the level trust receivables: 100M investor + 50M seller
/// at a fixed 80% gives a 120M base, so accumulation collects 18M a month and
/// the investor interest is retired in six dates instead of seven.
#[test]
fn fixed_allocation_pct_applies_to_the_level_trust_receivables() {
    let revolving_end = d(2026, 1, 1);
    let bullet_date = d(2026, 7, 1);
    let spec = card(0.0)
        .with_seller_interest(usd(50_000_000.0))
        .with_fixed_allocation_pct(0.80);
    let deal = with_rules(
        trust(spec, revolving_end),
        WaterfallRules {
            controlled_accumulation: Some(ControlledAccumulationSpec {
                start_date: revolving_end,
                bullet_date,
            }),
            ..no_rules()
        },
    );
    let diagnostics = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
        &deal,
        &market(),
        close(),
    )
    .expect("simulation");
    let first_accumulating = diagnostics
        .diagnostics
        .periods
        .iter()
        .find(|period| period.payment_date > revolving_end)
        .expect("accumulation period");
    let principal = first_accumulating.principal_collections.amount();
    assert!(
        (principal - 0.80 * 0.15 * 150_000_000.0).abs() < 1.0,
        "investor principal is 80% of the payment rate on 150M of receivables: {principal}"
    );

    // Without a supplied allocation the floating share (100 / 150) applies and
    // the seller interest changes nothing: the base is the investor interest.
    let floating = trust(
        card(0.0).with_seller_interest(usd(50_000_000.0)),
        revolving_end,
    );
    let floating = with_rules(
        floating,
        WaterfallRules {
            controlled_accumulation: Some(ControlledAccumulationSpec {
                start_date: revolving_end,
                bullet_date,
            }),
            ..no_rules()
        },
    );
    let floating = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
        &floating,
        &market(),
        close(),
    )
    .expect("simulation");
    let principal = floating
        .diagnostics
        .periods
        .iter()
        .find(|period| period.payment_date > revolving_end)
        .expect("accumulation period")
        .principal_collections
        .amount();
    assert!(
        (principal - 0.15 * 100_000_000.0).abs() < 1.0,
        "floating share × level receivables = payment rate on the investor interest: {principal}"
    );
}
