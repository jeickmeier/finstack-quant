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
            max_cumulative_loss_pct: 1.0,
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

/// Controlled accumulation is unchanged by the card model: nothing is paid
/// between the revolving end and the bullet date, then the accumulated
/// payment-rate principal is released at once. Without charge-offs the
/// revolving period keeps the receivables at 100M; the seven payment dates
/// from the revolving end (inclusive) through the bullet date accumulate
/// `100M × (1 − 0.85^7)`.
#[test]
fn controlled_accumulation_releases_the_accumulated_payment_rate_principal_as_a_bullet() {
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

    let results = simulate(&accumulating(0.0));
    let flows: Vec<(Date, f64)> = results["A"]
        .principal_flows
        .iter()
        .filter(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, amount)| (*date, amount.amount()))
        .collect();

    assert!(!flows.is_empty());
    assert!(
        flows[0].0 >= bullet_date,
        "no principal before the bullet date: {:?}",
        flows[0]
    );
    assert!(
        flows[0].0 < d(2026, 8, 1),
        "the bullet is released on the first payment date at or after the bullet date: {:?}",
        flows[0]
    );
    let accumulated = 100_000_000.0 * (1.0 - 0.85_f64.powi(7));
    assert!(
        (flows[0].1 - accumulated).abs() < 1.0,
        "bullet {} should be the accumulated payment-rate principal {accumulated}",
        flows[0].1
    );

    // Charge-offs shrink the receivables and therefore the bullet.
    let with_charge_offs = simulate(&accumulating(0.05));
    let bullet = with_charge_offs["A"]
        .principal_flows
        .iter()
        .find(|(_, amount)| amount.amount() > 0.0)
        .map(|(_, amount)| amount.amount())
        .expect("bullet");
    assert!(
        bullet < accumulated,
        "charge-offs must reduce the bullet: {bullet}"
    );
}
