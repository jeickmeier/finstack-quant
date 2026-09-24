//! Regression baseline for the 2026-09-17 structured-credit analyst audit
//! (plan `2026-09-17-structured-credit-analyst-remediation.md`, Task 0).
//!
//! Every test asserts the economically correct behaviour the audit expected.
//! Tests start `#[ignore]`d with the task that fixes them; each task removes
//! its ignore attribute. Fixture conventions: flat 5% continuously
//! compounded USD-OIS curve, `with_payment_calendar("nyse")`, fees `None`,
//! `constant_cpr(0)`, `constant_cdr(0)`, `RecoveryModelSpec::with_lag(0.40, 0)`
//! unless a test says otherwise.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, run_simulation, run_simulation_with_diagnostics, AllocationMode, AssetPool,
    AssetType, CallAssumption, CardPortfolioSpec, ControlledAccumulationSpec, CoverageRules,
    CoverageTestSpec, DealType, LiquidationSpec, PaymentCalculation, PaymentType, PoolAsset,
    PricingMode, Recipient, RecipientType, ReinvestmentCriteria, ReinvestmentPeriod, StepDownSpec,
    StepDownTrigger, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    Waterfall, WaterfallContext, WaterfallRules, WaterfallTier,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(base: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=15)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

fn tranche(
    id: &str,
    attachment: f64,
    detachment: f64,
    seniority: TrancheSeniority,
    balance: f64,
    coupon: f64,
    maturity: Date,
) -> Tranche {
    Tranche::new(
        id,
        attachment,
        detachment,
        seniority,
        usd(balance),
        TrancheCoupon::Fixed { rate: coupon },
        maturity,
    )
    .expect("tranche")
}

fn quiet(mut deal: StructuredCredit, cpr: f64) -> StructuredCredit {
    deal.fees = None;
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    deal.with_payment_calendar("nyse")
}

fn flows_on(flows: &[(Date, Money)], date: Date) -> f64 {
    flows
        .iter()
        .filter(|(dt, _)| *dt == date)
        .map(|(_, m)| m.amount())
        .sum()
}

/// P1: a deal-scope call realises the collateral's stub interest for the
/// period ending on the call date.
#[test]
fn p1_deal_call_proceeds_include_final_stub_collateral_interest() {
    let close = d(2024, 1, 1);
    let legal = d(2034, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.08,
        legal,
        DayCount::Act360,
    ));
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            90_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "EQ",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_clo("P1", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    deal.call_assumption = Some(CallAssumption::new(d(2026, 1, 1), 100.0));

    let results = run_simulation(&deal, &market(close), close).expect("run");
    let a = &results["A"];
    let eq = &results["EQ"];
    let call_date = *a
        .cashflows
        .iter()
        .map(|(dt, _)| dt)
        .filter(|dt| **dt >= d(2026, 1, 1))
        .min()
        .expect("call date");
    let prev_pay = *a
        .cashflows
        .iter()
        .map(|(dt, _)| dt)
        .filter(|dt| **dt < call_date)
        .max()
        .expect("previous payment date");
    let stub = DayCount::Act360
        .year_fraction(prev_pay, call_date, DayCountContext::default())
        .expect("stub");
    let collateral_stub_interest = 100_000_000.0 * 0.08 * stub;
    let a_accrued = 90_000_000.0 * 0.05 * stub;
    let paid_a = flows_on(&a.cashflows, call_date);
    let paid_eq = flows_on(&eq.cashflows, call_date);
    let total = paid_a + paid_eq;
    let expected_total = 100_000_000.0 + collateral_stub_interest;
    let expected_eq = 10_000_000.0 + collateral_stub_interest - a_accrued;
    assert!(
        (total - expected_total).abs() < 1_000.0,
        "call on {call_date}: distributed {total:.0} = A {paid_a:.0} + EQ {paid_eq:.0}; expected \
         {expected_total:.0}; equity {paid_eq:.0} vs {expected_eq:.0}"
    );
    assert!(
        (paid_eq - expected_eq).abs() < 1_000.0,
        "equity residual {paid_eq:.0} vs {expected_eq:.0}"
    );
}

fn seasoned_abs(
    current_pool: f64,
    cum_prepay: f64,
    cum_defaults: f64,
    cum_recoveries: f64,
    cpr: f64,
) -> (StructuredCredit, Date) {
    let close = d(2024, 1, 1);
    let legal = d(2027, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(current_pool),
        0.06,
        legal,
        DayCount::Act360,
    ));
    pool.cumulative_prepayments = usd(cum_prepay);
    pool.cumulative_defaults = usd(cum_defaults);
    pool.cumulative_recoveries = usd(cum_recoveries);
    let tranches = TrancheStructure::new(vec![
        tranche(
            "SR",
            20.0,
            100.0,
            TrancheSeniority::Senior,
            current_pool * 0.80,
            0.05,
            legal,
        ),
        tranche(
            "SUB",
            4.0,
            20.0,
            TrancheSeniority::Subordinated,
            current_pool * 0.16,
            0.07,
            legal,
        ),
        tranche(
            "EQ",
            0.0,
            4.0,
            TrancheSeniority::Equity,
            current_pool * 0.04,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let deal = quiet(
        StructuredCredit::new_abs("P2", pool, tranches, close, legal, "USD-OIS"),
        cpr,
    );
    (deal, close)
}

/// P2a: cumulative-loss triggers are a fraction of the ORIGINAL pool. A
/// seasoned 100M pool at factor 0.5 with 1.8% lifetime loss passes a 3%
/// step-down trigger.
#[test]
fn p2a_step_down_loss_trigger_uses_original_pool_balance() {
    let (mut deal, close) =
        seasoned_abs(50_000_000.0, 50_000_000.0, 3_000_000.0, 1_200_000.0, 0.20);
    deal.waterfall_rules = Some(WaterfallRules {
        step_down: Some(StepDownSpec {
            step_down_date: close,
            triggers: vec![StepDownTrigger::MaxCumulativeLoss(0.03)],
        }),
        ..Default::default()
    });
    let results = run_simulation(&deal, &market(close), close).expect("run");
    let sub = &results["SUB"];
    let first_date = sub
        .cashflows
        .first()
        .map(|(dt, _)| *dt)
        .expect("first date");
    let sub_principal = flows_on(&sub.principal_flows, first_date);
    let sr_principal = flows_on(&results["SR"].principal_flows, first_date);
    assert!(
        sub_principal > 0.0,
        "step-down should pay SUB pro-rata on {first_date}; SUB {sub_principal:.0}, SR {sr_principal:.0}"
    );
}

/// P2b: the clean-up call factor is current over ORIGINAL balance; a pool at
/// factor 0.08 is cleaned up on the first payment date under a 10% threshold.
#[test]
fn p2b_cleanup_call_factor_uses_original_pool_balance() {
    let (mut deal, close) = seasoned_abs(8_000_000.0, 92_000_000.0, 0.0, 0.0, 0.0);
    deal.cleanup_call_pct = Some(0.10);
    let results = run_simulation(&deal, &market(close), close).expect("run");
    let sr = &results["SR"];
    let first_date = sr.cashflows.first().map(|(dt, _)| *dt).expect("first date");
    let sr_principal = flows_on(&sr.principal_flows, first_date);
    assert!(
        (sr_principal - 6_400_000.0).abs() < 1.0,
        "the deal should be cleaned up on {first_date}: SR principal {sr_principal:.0}, \
         payment dates {}",
        sr.cashflows.len()
    );
}

/// P3: a `CommercialMortgage` with a 30-year amortization term and a 10-year
/// maturity leaves a balloon; without the term it fully amortizes.
#[test]
fn p3_commercial_mortgage_amortization_term_produces_the_balloon() {
    let close = d(2024, 1, 1);
    let legal = d(2034, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Cmbs, Currency::USD);
    let mut loan =
        PoolAsset::fixed_rate_bond("L", usd(10_000_000.0), 0.06, legal, DayCount::Thirty360);
    loan.asset_type = AssetType::CommercialMortgage { ltv: None };
    loan.amortization_term_months = Some(360);
    pool.assets.push(loan);
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            10.0,
            100.0,
            TrancheSeniority::Senior,
            9_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "EQ",
            0.0,
            10.0,
            TrancheSeniority::Equity,
            1_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let deal = quiet(
        StructuredCredit::new_cmbs("P3", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    let run = run_simulation_with_diagnostics(&deal, &market(close), close).expect("run");
    let last = run.diagnostics.periods.last().expect("periods");
    assert!(
        (last.principal_collections.amount() - 8_386_595.0).abs() < 1_000.0,
        "balloon on {}: {:.0}",
        last.payment_date,
        last.principal_collections.amount()
    );
}

/// P5: an OC cure paid from interest does not leave the numerator, so the
/// paydown that restores the ratio is `D − N/r`.
#[test]
fn p5_oc_cure_is_sized_for_an_interest_funded_paydown() {
    let legal = d(2031, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(118_000_000.0),
        0.08,
        legal,
        DayCount::Act360,
    ));
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            23.08,
            100.0,
            TrancheSeniority::Senior,
            100_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "B",
            0.0,
            23.08,
            TrancheSeniority::Subordinated,
            30_000_000.0,
            0.08,
            legal,
        ),
    ])
    .expect("structure");
    let waterfall = Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("a_int", "A"))
                .add_recipient(Recipient::tranche_interest("b_int", "B")),
        )
        .add_tier(WaterfallTier::coverage_tests(
            "coverage",
            2,
            vec![CoverageTestSpec::oc("A", 1.20)],
        ))
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_principal("a_prin", "A", None))
                .add_recipient(Recipient::tranche_principal("b_prin", "B", None)),
        )
        .add_tier(
            WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(Recipient::new(
                "eq",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("waterfall");
    let market = MarketContext::new();
    let result = execute_waterfall(
        &waterfall,
        &tranches,
        &pool,
        WaterfallContext {
            available_cash: usd(25_000_000.0),
            interest_collections: usd(25_000_000.0),
            principal_collections: usd(0.0),
            payment_date: d(2024, 4, 1),
            period_start: d(2024, 1, 1),
            valuation_date: d(2024, 1, 1),
            pool_balance: usd(118_000_000.0),
            market: &market,
            tranche_balances: None,
            asset_balances: None,
            live_collateral: None,
            special_serviced: None,
            deferred_interest: None,
            reserve_balance: usd(0.0),
            restricted_cash: usd(0.0),
            defaulted_collateral_value: usd(0.0),
            recovery_proceeds: usd(0.0),
            floating_rate_shift: 0.0,
            equity_history: None,
        },
    )
    .expect("waterfall execution");
    let correct = 100_000_000.0 - 118_000_000.0 / 1.20;
    let diverted = result.diverted_cash.amount();
    assert!(
        (diverted - correct).abs() < 1.0,
        "diverted {diverted:.0} vs the interest-funded cure {correct:.0}; post-cure ratio {:.4}",
        118_000_000.0 / (100_000_000.0 - diverted)
    );
}

/// P6: `CoverageRules::clo_standard()` carries performing collateral at par
/// whatever its rating.
#[test]
fn p6_clo_standard_rules_carry_performing_collateral_at_par() {
    let close = d(2024, 1, 1);
    let legal = d(2030, 1, 1);
    let ratio_for = |rating: CreditRating| {
        let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
        for i in 0..10 {
            pool.assets.push(
                PoolAsset::fixed_rate_bond(
                    format!("L{i}"),
                    usd(10_000_000.0),
                    0.08,
                    legal,
                    DayCount::Act360,
                )
                .with_rating(rating),
            );
        }
        let tranches = TrancheStructure::new(vec![
            tranche(
                "A",
                50.0,
                100.0,
                TrancheSeniority::Senior,
                50_000_000.0,
                0.05,
                legal,
            ),
            tranche(
                "EQ",
                0.0,
                50.0,
                TrancheSeniority::Equity,
                50_000_000.0,
                0.0,
                legal,
            ),
        ])
        .expect("structure");
        let mut deal = quiet(
            StructuredCredit::new_clo("P6", pool, tranches, close, legal, "USD-OIS"),
            0.0,
        )
        .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.10)])
        .expect("triggers");
        deal.coverage_rules = Some(CoverageRules::clo_standard());
        let run = run_simulation_with_diagnostics(&deal, &market(close), close).expect("run");
        run.diagnostics.periods[0].coverage_tests[0].ratio
    };
    let b = ratio_for(CreditRating::B);
    let b_minus = ratio_for(CreditRating::BMinus);
    assert!(
        (b - 2.0).abs() < 1e-6 && (b_minus - 2.0).abs() < 1e-6,
        "par OC should be 2.00; clo_standard gives B => {b:.4}, B- => {b_minus:.4}"
    );
}

/// P7: tranche-level bump metrics are per tranche and sum to the deal;
/// Severity01 stays live under a recovery override.
#[test]
fn p7_tranche_bump_metrics_are_per_tranche_and_severity01_is_live() {
    let close = d(2024, 1, 1);
    let legal = d(2030, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.08,
        legal,
        DayCount::Act360,
    ));
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            10.0,
            100.0,
            TrancheSeniority::Senior,
            90_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "EQ",
            0.0,
            10.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_clo("P7", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
    let mkt = market(close);
    let a = deal
        .value_tranche_with_metrics("A", &mkt, close, &[MetricId::Default01])
        .expect("A metrics");
    let eq = deal
        .value_tranche_with_metrics("EQ", &mkt, close, &[MetricId::Default01])
        .expect("EQ metrics");
    let whole = deal
        .price_with_metrics(
            &mkt,
            close,
            &[MetricId::Default01],
            PricingOptions::default(),
        )
        .expect("deal metrics");
    let a01 = a.metrics[&MetricId::Default01];
    let eq01 = eq.metrics[&MetricId::Default01];
    let deal01 = whole.metric(MetricId::Default01).expect("deal Default01");
    assert!(
        (a01 - eq01).abs() > 1.0,
        "senior and equity Default01 must differ: A {a01:.2}, EQ {eq01:.2}"
    );
    assert!(
        ((a01 + eq01) - deal01).abs() < 1.0,
        "tranche sensitivities {a01:.2} + {eq01:.2} should sum to the deal's {deal01:.2}"
    );
    // At 1% CDR the lifetime loss stays inside the 10M equity, so the
    // senior's sensitivity is a small fraction of equity's.
    let mut mild = deal.clone();
    mild.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.01);
    let a_mild = mild
        .value_tranche_with_metrics("A", &mkt, close, &[MetricId::Default01])
        .expect("A metrics")
        .metrics[&MetricId::Default01];
    let eq_mild = mild
        .value_tranche_with_metrics("EQ", &mkt, close, &[MetricId::Default01])
        .expect("EQ metrics")
        .metrics[&MetricId::Default01];
    assert!(
        a_mild.abs() < eq_mild.abs() * 0.25,
        "with losses inside equity the senior Default01 {a_mild:.2} is far below equity's {eq_mild:.2}"
    );

    let mut overridden = deal;
    overridden.behavior_overrides.recovery_rate = Some(0.40);
    let sev = overridden
        .price_with_metrics(
            &mkt,
            close,
            &[MetricId::Severity01, MetricId::Recovery01],
            PricingOptions::default(),
        )
        .expect("severity metrics");
    let severity01 = sev.metric(MetricId::Severity01).expect("Severity01");
    let recovery01 = sev.metric(MetricId::Recovery01).expect("Recovery01");
    assert!(
        (severity01 + recovery01).abs() < recovery01.abs() * 0.05,
        "Severity01 {severity01:.2} should be about −Recovery01 {recovery01:.2}"
    );
}

/// P8: the stochastic deal price is quoted on the same face as the
/// deterministic one (the sum of current tranche balances).
#[test]
fn p8_stochastic_price_uses_the_same_face_as_deterministic() {
    let close = d(2024, 1, 1);
    let legal = d(2027, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.06,
        legal,
        DayCount::Act360,
    ));
    // Notes total 60M against a 100M pool: prices are quoted on the notes.
    let tranches = TrancheStructure::new(vec![
        tranche(
            "EQ",
            0.0,
            33.33,
            TrancheSeniority::Equity,
            20_000_000.0,
            0.0,
            legal,
        ),
        tranche(
            "A",
            33.33,
            100.0,
            TrancheSeniority::Senior,
            40_000_000.0,
            0.05,
            legal,
        ),
    ])
    .expect("structure");
    let deal = quiet(
        StructuredCredit::new_abs("P8", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    let mkt = market(close);
    let det = deal
        .price_with_metrics(
            &mkt,
            close,
            &[MetricId::DirtyPrice],
            PricingOptions::default(),
        )
        .expect("deterministic");
    let det_dirty = det.metric(MetricId::DirtyPrice).expect("DirtyPrice");
    let sto = deal
        .price_stochastic_with_mode(
            &mkt,
            close,
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect("stochastic");
    assert!(
        (sto.dirty_price - det_dirty).abs() < 0.05,
        "deterministic dirty price {det_dirty:.3} vs stochastic {:.3}",
        sto.dirty_price
    );
}

/// Card master trust: from the start of controlled accumulation the investor
/// receives a FIXED share of trust principal collections (payment rate on
/// receivables held level by the seller), so a 100M investor interest at a
/// 15% payment rate accumulates 15M a month: the six accumulation dates
/// after the revolving end (the end date itself still revolves) fund the
/// 90M Class A bullet exactly.
#[test]
fn card_accumulation_uses_a_fixed_investor_allocation() {
    let close = d(2024, 1, 1);
    let legal = d(2029, 1, 1);
    let revolving_end = d(2026, 1, 1);
    let bullet_date = d(2026, 7, 1);
    let mut pool = AssetPool::new("P", DealType::Card, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("R{i}"),
            usd(10_000_000.0),
            0.0,
            legal,
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
        tranche(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            90_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_abs("CARD", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal.credit_model.card = Some(CardPortfolioSpec::new(0.15, 0.18, 0.0));
    deal.waterfall_rules = Some(WaterfallRules {
        controlled_accumulation: Some(ControlledAccumulationSpec {
            start_date: revolving_end,
            bullet_date,
        }),
        ..Default::default()
    });
    let results = run_simulation(&deal, &market(close), close).expect("run");
    let bullet = results["A"]
        .principal_flows
        .iter()
        .find(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, amount)| (*date, amount.amount()))
        .expect("bullet");
    assert!(
        bullet.0 >= bullet_date && bullet.0 < d(2026, 8, 1),
        "bullet date {}",
        bullet.0
    );
    assert!(
        (bullet.1 - 90_000_000.0).abs() < 1.0,
        "six months of 15M fixed-allocation principal fund the 90M bullet; got {:.0}",
        bullet.1
    );
}

/// Task 1: a cumulative-loss curve is stated on the ORIGINAL balance. A pool
/// at factor 0.5 (10M current, 20M original, 24 months old) running a 3%
/// 36-month net-loss curve at 50% severity still owes months 25-36 of the
/// curve: `(3% − 2%) / 0.5 × 20M = 400,000` of defaults, not 200,000.
#[test]
fn cumulative_loss_curve_is_based_on_the_original_pool_balance() {
    let close = d(2024, 1, 1);
    let legal = d(2027, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    let mut loan =
        PoolAsset::fixed_rate_bond("L", usd(10_000_000.0), 0.06, legal, DayCount::Act360);
    loan.acquisition_date = Some(d(2022, 1, 1));
    pool.assets.push(loan);
    pool.cumulative_prepayments = usd(5_000_000.0);
    pool.cumulative_scheduled_amortization = usd(5_000_000.0);
    let tranches = TrancheStructure::new(vec![
        tranche(
            "SR",
            20.0,
            100.0,
            TrancheSeniority::Senior,
            8_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "EQ",
            0.0,
            20.0,
            TrancheSeniority::Equity,
            2_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_abs("CNL", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    let curve: Vec<f64> = (1..=36).map(|m| 3.0 * f64::from(m) / 36.0).collect();
    deal.credit_model.default_spec = DefaultModelSpec::cumulative_loss(curve, 0.5);
    let run = run_simulation_with_diagnostics(&deal, &market(close), close).expect("run");
    let defaults: f64 = run
        .diagnostics
        .periods
        .iter()
        .map(|period| period.defaults.amount())
        .sum();
    assert!(
        (defaults - 400_000.0).abs() < 2_000.0,
        "remaining curve defaults on the original balance should be 400,000, got {defaults:.0}"
    );
    assert!(
        (run.diagnostics.periods[0].pool_factor - 0.5).abs() < 0.01,
        "pool factor is quoted on the original balance: {}",
        run.diagnostics.periods[0].pool_factor
    );
}

/// Task 2: a deal called before an NPL row resolves realises that row at its
/// net liquidation proceeds, not at par.
#[test]
fn deal_call_values_unresolved_npl_rows_at_net_proceeds() {
    let close = d(2024, 1, 1);
    let legal = d(2030, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Rmbs, Currency::USD);
    let mut npl =
        PoolAsset::fixed_rate_bond("N", usd(100_000_000.0), 0.06, legal, DayCount::Thirty360);
    npl.liquidation = Some(LiquidationSpec {
        months_to_resolution: 36,
        proceeds_pct: 55.0,
        carry_cost_pct: 5.0,
        reperformance_prob: 0.0,
        modified_rate: None,
    });
    pool.assets.push(npl);
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            50.0,
            100.0,
            TrancheSeniority::Senior,
            50_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "E",
            0.0,
            50.0,
            TrancheSeniority::Equity,
            50_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_rmbs("NPL-CALL", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    deal.call_assumption = Some(CallAssumption::new(d(2026, 1, 1), 100.0));
    let results = run_simulation(&deal, &market(close), close).expect("run");
    let a_total: f64 = results["A"].cashflows.iter().map(|(_, m)| m.amount()).sum();
    let e_total: f64 = results["E"].cashflows.iter().map(|(_, m)| m.amount()).sum();
    // Net proceeds 50% of 100M = 50M: they cover A's deferred coupons first
    // and the rest of its principal; E gets nothing.
    assert!(
        (a_total - 50_000_000.0).abs() < 1.0,
        "A cash {a_total:.0} should equal the 50M net proceeds"
    );
    assert!(
        e_total < 1.0,
        "equity should receive nothing from a 50M liquidation against 50M of notes, got {e_total:.0}"
    );
}

/// Task 2: clean-up feasibility counts the redemption period's collections.
/// An 8M pool at factor 0.08 collects ~162k of stub interest; after paying
/// A's coupon the leftover lets an 8.05M note be redeemed.
#[test]
fn cleanup_call_feasibility_counts_the_stub_collections() {
    let close = d(2024, 1, 1);
    let legal = d(2027, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(8_000_000.0),
        0.08,
        legal,
        DayCount::Act360,
    ));
    pool.cumulative_prepayments = usd(92_000_000.0);
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            0.0,
            99.38,
            TrancheSeniority::Senior,
            8_050_000.0,
            0.05,
            legal,
        ),
        tranche(
            "E",
            99.38,
            100.0,
            TrancheSeniority::Equity,
            50_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let mut deal = quiet(
        StructuredCredit::new_clo("CLEANUP", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    deal.cleanup_call_pct = Some(0.10);
    let results = run_simulation(&deal, &market(close), close).expect("run");
    let a = &results["A"];
    let first_date = a.cashflows.first().map(|(dt, _)| *dt).expect("first date");
    let a_principal = flows_on(&a.principal_flows, first_date);
    assert!(
        (a_principal - 8_050_000.0).abs() < 1.0,
        "the clean-up call is feasible once the stub collections count: A principal on \
         {first_date} = {a_principal:.0} over {} payment dates",
        a.cashflows.len()
    );
}

/// Task 4: a user-supplied haircut map keys by rating bucket, so `B-` rows
/// read the `B` entry.
#[test]
fn rating_haircuts_key_by_rating_bucket() {
    let close = d(2024, 1, 1);
    let legal = d(2030, 1, 1);
    let ratio_for = |rating: CreditRating, map: Vec<(CreditRating, f64)>| {
        let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
        for i in 0..10 {
            pool.assets.push(
                PoolAsset::fixed_rate_bond(
                    format!("L{i}"),
                    usd(10_000_000.0),
                    0.08,
                    legal,
                    DayCount::Act360,
                )
                .with_rating(rating),
            );
        }
        let tranches = TrancheStructure::new(vec![
            tranche(
                "A",
                50.0,
                100.0,
                TrancheSeniority::Senior,
                50_000_000.0,
                0.05,
                legal,
            ),
            tranche(
                "EQ",
                0.0,
                50.0,
                TrancheSeniority::Equity,
                50_000_000.0,
                0.0,
                legal,
            ),
        ])
        .expect("structure");
        let mut deal = quiet(
            StructuredCredit::new_clo("HC", pool, tranches, close, legal, "USD-OIS"),
            0.0,
        )
        .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.10)])
        .expect("triggers");
        deal.coverage_rules = Some(CoverageRules {
            rating_haircuts: map.into_iter().collect(),
            ..CoverageRules::default()
        });
        let run = run_simulation_with_diagnostics(&deal, &market(close), close).expect("run");
        run.diagnostics.periods[0].coverage_tests[0].ratio
    };
    let b_minus = ratio_for(CreditRating::BMinus, vec![(CreditRating::B, 0.10)]);
    let b_minus_nr_only = ratio_for(CreditRating::BMinus, vec![(CreditRating::NR, 0.15)]);
    assert!(
        (b_minus - 1.80).abs() < 1e-6,
        "B- rows read the B entry (10% haircut): expected 1.80, got {b_minus:.4}"
    );
    assert!(
        (b_minus_nr_only - 2.0).abs() < 1e-6,
        "a rated row never takes the NR entry: expected 2.00, got {b_minus_nr_only:.4}"
    );
}

/// Task 5: the registry's generic rate sensitivities reprice the NOTE too:
/// the senior and the equity carry different DV01s that sum to the deal's.
#[test]
fn tranche_dv01_is_per_tranche_and_sums_to_the_deal() {
    let close = d(2024, 1, 1);
    let legal = d(2030, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.08,
        legal,
        DayCount::Act360,
    ));
    let tranches = TrancheStructure::new(vec![
        tranche(
            "A",
            10.0,
            100.0,
            TrancheSeniority::Senior,
            90_000_000.0,
            0.05,
            legal,
        ),
        tranche(
            "EQ",
            0.0,
            10.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
            legal,
        ),
    ])
    .expect("structure");
    let deal = quiet(
        StructuredCredit::new_clo("DV01", pool, tranches, close, legal, "USD-OIS"),
        0.0,
    );
    let mkt = market(close);
    let a = deal
        .value_tranche_with_metrics("A", &mkt, close, &[MetricId::Dv01])
        .expect("A metrics")
        .metrics[&MetricId::Dv01];
    let eq = deal
        .value_tranche_with_metrics("EQ", &mkt, close, &[MetricId::Dv01])
        .expect("EQ metrics")
        .metrics[&MetricId::Dv01];
    let whole = deal
        .price_with_metrics(&mkt, close, &[MetricId::Dv01], PricingOptions::default())
        .expect("deal metrics")
        .metric(MetricId::Dv01)
        .expect("deal Dv01");
    assert!(
        (a - eq).abs() > 1.0,
        "A DV01 {a:.2} and EQ DV01 {eq:.2} must differ"
    );
    assert!(
        ((a + eq) - whole).abs() < 1.0,
        "A {a:.2} + EQ {eq:.2} must sum to the deal DV01 {whole:.2}"
    );
}
