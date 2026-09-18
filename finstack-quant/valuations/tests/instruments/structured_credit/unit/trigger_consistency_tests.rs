//! Coverage tests are evaluated on the cash and waterfall the executor uses
//! (hedge payments ranked as fees, hedge receipts, call premia and reserve
//! interest in the interest proceeds), so the reinvestment decision follows
//! the executor's own test result; `divert_pct` caps what a failing test
//! diverts, and `after_junior_fees` places a test ahead of the residual.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, run_simulation_with_diagnostics, AssetPool, CoverageTestDiagnostic,
    CoverageTestSpec, DealType, HedgeSwap, PaymentCalculation, PaymentType, PeriodDiagnostics,
    PoolAsset, Recipient, RecipientType, ReinvestmentCriteria, ReinvestmentPeriod,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallBuilder,
    WaterfallContext, WaterfallTier,
};
use finstack_quant_valuations::instruments::PayReceive;
use time::macros::date;

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};
use crate::test_support::rates::usd_irs_swap;

const CLOSE: Date = date!(2024 - 01 - 01);
const MATURITY: Date = date!(2030 - 01 - 01);
const INDEX: &str = "USD-SOFR-3M";

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(forward: f64) -> MarketContext {
    let fixings: Vec<(Date, f64)> = (0..25)
        .map(|days| (CLOSE - time::Duration::days(days), forward))
        .collect();
    MarketContext::new()
        .insert(flat_discount_curve(0.03, CLOSE, "USD-OIS"))
        .insert(flat_forward_curve(forward, CLOSE, INDEX))
        .insert_series(
            ScalarTimeSeries::new(format!("FIXING:{INDEX}"), fixings, None).expect("fixings"),
        )
}

/// Quarterly CLO: ten 10M 8% bullets, A 60M at 5%, equity 40M, reinvesting
/// 20% CPR proceeds through legal final, with `tests` on the template.
fn clo(tests: Vec<CoverageTestSpec>) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            MATURITY,
            DayCount::Act360,
        ));
    }
    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: MATURITY,
        is_active: true,
        amortizing_tranches: Vec::new(),
        assumptions: None,
        criteria: ReinvestmentCriteria::default(),
    });
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            60.0,
            TrancheSeniority::Senior,
            usd(60_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            MATURITY,
        )
        .expect("A"),
        Tranche::new(
            "E",
            60.0,
            100.0,
            TrancheSeniority::Equity,
            usd(40_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            MATURITY,
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-TRIGGERS", pool, tranches, CLOSE, MATURITY, "USD-OIS")
            .with_payment_calendar("nyse")
            .with_coverage_triggers(tests)
            .expect("tests");
    deal.fees = None;
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

/// Pay 6% fixed against SOFR on the senior par: with SOFR at 2% the net
/// payment is about 600k a quarter, ranked as a senior fee.
fn hedged(deal: StructuredCredit) -> StructuredCredit {
    let mut swap = usd_irs_swap(
        InstrumentId::new("HEDGE-A"),
        usd(60_000_000.0),
        0.06,
        CLOSE,
        MATURITY,
        PayReceive::Pay,
    )
    .expect("swap");
    swap.fixed.frequency = Tenor::quarterly();
    swap.float.frequency = Tenor::quarterly();
    deal.with_hedge_swap(HedgeSwap::new(swap).on_tranche_par("A"))
}

fn periods(deal: &StructuredCredit) -> Vec<PeriodDiagnostics> {
    run_simulation_with_diagnostics(deal, &market(0.02), CLOSE)
        .expect("simulation")
        .diagnostics
        .periods
}

/// Pool interest is about 2M a quarter against 750k of senior coupon, so an
/// IC test at 2.2x passes on the collateral alone but fails once the 600k
/// hedge payment ranks as a senior fee. The reinvestment decision must
/// follow the executor's result: the hedged deal stops buying collateral
/// in the periods the executor reports the test failing.
#[test]
fn reinvestment_suspends_on_the_executors_test_result() {
    let unhedged = periods(&clo(vec![CoverageTestSpec::ic("A", 2.2)]));
    let hedged = periods(&hedged(clo(vec![CoverageTestSpec::ic("A", 2.2)])));
    fn ic(period: &PeriodDiagnostics) -> &CoverageTestDiagnostic {
        period
            .coverage_tests
            .iter()
            .find(|test| test.test_id == "IC_A")
            .expect("the executor evaluated the IC test")
    }
    assert!(ic(&unhedged[0]).passing, "{:?}", ic(&unhedged[0]));
    assert!(
        unhedged[0].reinvested_par.amount() > 0.0,
        "the unhedged deal reinvests its prepayments"
    );
    assert!(!ic(&hedged[0]).passing, "{:?}", ic(&hedged[0]));
    assert_eq!(
        hedged[0].reinvested_par.amount(),
        0.0,
        "reinvestment stops in the period the executor's test fails"
    );
    for period in hedged.iter().take(4) {
        assert_eq!(
            period.reinvested_par.amount() > 0.0,
            ic(period).passing,
            "{}: reinvestment must agree with the executor's test",
            period.payment_date
        );
    }
}

/// A test placed after the junior fees with `divert_pct = 50` diverts half
/// of what would otherwise reach equity, and the other half still does.
#[test]
fn a_fifty_percent_diversion_after_the_sub_fee_diverts_half_the_residual() {
    let deal = clo(Vec::new());
    let mut waterfall = WaterfallBuilder::new(Currency::USD)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee).add_recipient(Recipient::fixed_fee(
                "trustee",
                "Trustee",
                usd(50_000.0),
            )),
        )
        .add_tier(
            WaterfallTier::new("a_interest", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("a_int", "A")),
        )
        .add_tier(
            WaterfallTier::new("junior_fees", 3, PaymentType::Fee)
                .add_recipient(Recipient::fixed_fee("sub_mgmt", "Manager", usd(100_000.0))),
        )
        .add_tier(
            WaterfallTier::new("principal", 4, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("a_prin", "A", None)),
        )
        .add_tier(
            WaterfallTier::new("equity", 5, PaymentType::Residual).add_recipient(Recipient::new(
                "equity_dist",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("waterfall");
    waterfall.insert_coverage_test(
        CoverageTestSpec::ic("A", 5.0)
            .after_junior_fees()
            .with_divert_pct(50.0),
    );
    let ids: Vec<&str> = waterfall.tiers.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "fees",
            "a_interest",
            "junior_fees",
            "junior_fees_coverage",
            "principal",
            "equity"
        ]
    );

    let market = market(0.02);
    let context = WaterfallContext {
        available_cash: usd(1_500_000.0),
        interest_collections: usd(1_500_000.0),
        principal_collections: usd(0.0),
        payment_date: date!(2024 - 04 - 01),
        period_start: CLOSE,
        valuation_date: CLOSE,
        pool_balance: usd(100_000_000.0),
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
    };
    let result =
        execute_waterfall(&waterfall, &deal.tranches, &deal.pool, context).expect("waterfall");
    let equity = result
        .distributions
        .get(&RecipientType::Equity)
        .map_or(0.0, |amount| amount.amount());
    let diverted = result.diverted_cash.amount();
    assert!(
        diverted > 100_000.0,
        "the failing 5x IC test diverts: {diverted}"
    );
    assert!(
        (diverted - equity).abs() < 1e-6,
        "half of the residual is diverted ({diverted}) and half reaches equity ({equity})"
    );
}
