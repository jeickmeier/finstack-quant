//! Hedge swaps settled through the waterfall: a balance-guaranteed
//! pay-fixed/receive-float swap turns a fixed-rate pool's interest into
//! floating cash for floating-rate notes, and the swap's net payments rank as
//! fees, so they reduce interest coverage. The 2026-09-15 audit found hedges
//! valued only as an NPV overlay outside the waterfall and the IC test.

use finstack_quant_cashflows::builder::{
    DefaultModelSpec, FloatingRateSpec, PrepaymentModelSpec, RecoveryModelSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, run_simulation, AssetPool, CoverageTestSpec, DealType, HedgeSwap,
    PaymentType, PoolAsset, Recipient, StructuredCredit, SwapPriority, Tranche, TrancheCashflows,
    TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallBuilder, WaterfallContext,
    WaterfallTier,
};
use finstack_quant_valuations::instruments::PayReceive;
use time::macros::date;

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};
use crate::test_support::rates::usd_irs_swap;

const CLOSE: Date = date!(2024 - 01 - 01);
const MATURITY: Date = date!(2032 - 01 - 01);
const INDEX: &str = "USD-SOFR-3M";

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

/// Flat 3% discounting with the SOFR forward curve and fixings at `forward`.
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

fn floating(spread_bp: f64) -> FloatingRateSpec {
    let dec = |value: f64| rust_decimal::Decimal::try_from(value).expect("decimal");
    FloatingRateSpec {
        index_id: CurveId::new(INDEX),
        spread_bp: dec(spread_bp),
        gearing: dec(1.0),
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    }
}

/// Auto ABS: ten 10M fixed-rate loans at 5%, an 80M floating senior note at
/// SOFR + 150 bp and 20M of equity; no prepayments or defaults.
fn abs(hedged: bool) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Auto, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.05,
            MATURITY,
            DayCount::Act360,
        ));
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            usd(80_000_000.0),
            TrancheCoupon::Floating(floating(150.0)),
            MATURITY,
        )
        .expect("senior"),
        Tranche::new(
            "E",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            usd(20_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            MATURITY,
        )
        .expect("equity"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_abs("ABS-HEDGE", pool, tranches, CLOSE, MATURITY, "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    if hedged {
        // Pay 4% fixed, receive SOFR, monthly on both legs (the ABS pays
        // monthly), on the senior par.
        let mut swap = usd_irs_swap(
            InstrumentId::new("HEDGE-A"),
            usd(80_000_000.0),
            0.04,
            CLOSE,
            MATURITY,
            PayReceive::Pay,
        )
        .expect("swap");
        swap.fixed.frequency = Tenor::monthly();
        swap.float.frequency = Tenor::monthly();
        deal = deal.with_hedge_swap(HedgeSwap::new(swap).on_tranche_par("A"));
    }
    deal
}

fn simulate(deal: &StructuredCredit, forward: f64) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market(forward), CLOSE).expect("simulation")
}

#[test]
fn balance_guaranteed_swap_keeps_the_floating_senior_note_current_under_a_rate_shock() {
    // Base curve: SOFR 4%, A owes 5.5% on 80M (4.4M/yr) against 5M/yr of pool
    // interest, so A is current with or without the swap.
    let base_unhedged = simulate(&abs(false), 0.04);
    let base_hedged = simulate(&abs(true), 0.04);
    for run in [&base_unhedged, &base_hedged] {
        assert!(
            run["A"].deferred_flows.is_empty(),
            "A is current on the base curve, got {:?}",
            run["A"].deferred_flows.first()
        );
    }

    // +200 bp: A owes 7.5% (6M/yr) against 5M/yr of fixed pool interest.
    let shocked_unhedged = simulate(&abs(false), 0.06);
    let shocked_hedged = simulate(&abs(true), 0.06);
    assert!(
        !shocked_unhedged["A"].deferred_flows.is_empty(),
        "without the swap the fixed-rate pool cannot cover the floating coupon"
    );
    assert!(
        shocked_hedged["A"].deferred_flows.is_empty(),
        "the swap's 2% receipt on the senior par tops up interest proceeds, got {:?}",
        shocked_hedged["A"].deferred_flows.first()
    );
    // Interest available moves with the forward curve: the hedged note earns
    // its whole floating coupon under the shock.
    assert!(
        shocked_hedged["A"].total_interest.amount()
            > base_hedged["A"].total_interest.amount() + 1e6,
        "shocked {} vs base {}",
        shocked_hedged["A"].total_interest.amount(),
        base_hedged["A"].total_interest.amount()
    );
    let first = date!(2024 - 02 - 01);
    let a_coupon = |run: &HashMap<String, TrancheCashflows>| {
        run["A"]
            .interest_flows
            .iter()
            .filter(|(date, _)| *date == first)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>()
    };
    // 80M × 7.5% × 31/360 on the first monthly payment date.
    let expected = 80_000_000.0 * 0.075 * 31.0 / 360.0;
    assert!(
        (a_coupon(&shocked_hedged) - expected).abs() < 1.0,
        "first hedged coupon {} vs {expected}",
        a_coupon(&shocked_hedged)
    );
    assert!(a_coupon(&shocked_unhedged) < expected - 50_000.0);
}

fn ic_context(market: &MarketContext) -> WaterfallContext<'_> {
    WaterfallContext {
        available_cash: usd(1_500_000.0),
        interest_collections: usd(1_500_000.0),
        principal_collections: usd(0.0),
        payment_date: date!(2024 - 04 - 01),
        period_start: CLOSE,
        valuation_date: CLOSE,
        pool_balance: usd(100_000_000.0),
        market,
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
    }
}

#[test]
fn swap_payments_rank_as_fees_and_tighten_the_ic_test() {
    let deal = abs(false);
    let market = MarketContext::new();
    let waterfall = WaterfallBuilder::new(Currency::USD)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee)
                .add_recipient(Recipient::fixed_fee("trustee", "Trustee", usd(50_000.0))),
        )
        .add_tier(
            WaterfallTier::new("a_interest", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("a_int", "A")),
        )
        .add_tier(WaterfallTier::coverage_tests(
            "a_coverage",
            3,
            vec![CoverageTestSpec::ic("A", 1.20)],
        ))
        .add_tier(
            WaterfallTier::new("principal", 4, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("a_prin", "A", None)),
        )
        .add_tier(
            WaterfallTier::new("equity", 5, PaymentType::Residual).add_recipient(Recipient::new(
                "equity_dist",
                finstack_quant_valuations::instruments::fixed_income::structured_credit::RecipientType::Equity,
                finstack_quant_valuations::instruments::fixed_income::structured_credit::PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("waterfall");
    let mut with_swap = waterfall.clone();
    with_swap.insert_hedge_payment(
        Recipient::fixed_fee("swap_1", "SwapCounterparty", usd(200_000.0)),
        SwapPriority::SeniorFee,
    );
    assert_eq!(
        with_swap.tiers[0].recipients.len(),
        2,
        "a senior swap payment joins the leading fee tier"
    );
    let mut junior = waterfall.clone();
    junior.insert_hedge_payment(
        Recipient::fixed_fee("swap_1", "SwapCounterparty", usd(200_000.0)),
        SwapPriority::JuniorFee,
    );
    let ids: Vec<&str> = junior.tiers.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "fees",
            "a_interest",
            "a_coverage",
            "junior_hedge_fees",
            "principal",
            "equity"
        ],
        "a junior swap payment opens a fee tier after the coupons, ahead of principal"
    );
    assert!(junior
        .tiers
        .iter()
        .enumerate()
        .all(|(index, tier)| tier.priority == index + 1));

    // A's coupon needs the SOFR fixing; use a fixed-coupon twin of the deal
    // for the executor-level comparison.
    let mut fixed_deal = deal;
    fixed_deal.tranches.tranches[0].coupon = TrancheCoupon::Fixed { rate: 0.055 };
    let without = execute_waterfall(
        &waterfall,
        &fixed_deal.tranches,
        &fixed_deal.pool,
        ic_context(&market),
    )
    .expect("without swap");
    let with = execute_waterfall(
        &with_swap,
        &fixed_deal.tranches,
        &fixed_deal.pool,
        ic_context(&market),
    )
    .expect("with swap");
    let ic = |result: &finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallDistribution| {
        result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "IC_A")
            .map(|(_, ratio, _)| *ratio)
            .expect("IC_A")
    };
    let a_due = 80_000_000.0 * 0.055 * 91.0 / 360.0;
    assert!((ic(&without) - (1_500_000.0 - 50_000.0) / a_due).abs() < 1e-9);
    assert!(
        (ic(&with) - (1_500_000.0 - 250_000.0) / a_due).abs() < 1e-9,
        "the swap payment nets out of the IC numerator like any senior fee: {} vs {}",
        ic(&with),
        (1_500_000.0 - 250_000.0) / a_due
    );
}
