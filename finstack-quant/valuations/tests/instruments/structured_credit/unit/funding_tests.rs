//! Cross-account funding: a fee or interest tier may top up from principal
//! proceeds (`FundingSource::InterestThenPrincipal`), the CLO
//! principal-waterfall convention. The 2026-09-15 audit found the accounts
//! strictly separated by tier type, so a senior coupon shortfall deferred
//! even with ample principal in the deal.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, run_simulation, AssetPool, CoverageTestSpec, DealType, FundingSource,
    PaymentType, PoolAsset, Recipient, StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon,
    TrancheSeniority, TrancheStructure, WaterfallBuilder, WaterfallContext, WaterfallTier,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Five-class CLO: ten 10M bullet loans at `pool_rate`, A 60 / B 15 / C 10 /
/// D 5 / E 10 (equity), 20% CPR (ample principal), no defaults.
fn clo(pool_rate: f64, a_coupon: f64, covers: Option<bool>) -> (StructuredCredit, Date) {
    let close = d(2024, 1, 1);
    let maturity = d(2032, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            pool_rate,
            maturity,
            DayCount::Act360,
        ));
    }
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
        tr(
            "A",
            0.0,
            60.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            a_coupon,
        ),
        tr(
            "B",
            60.0,
            75.0,
            TrancheSeniority::Mezzanine,
            15_000_000.0,
            0.07,
        ),
        tr(
            "C",
            75.0,
            85.0,
            TrancheSeniority::Mezzanine,
            10_000_000.0,
            0.09,
        ),
        tr(
            "D",
            85.0,
            90.0,
            TrancheSeniority::Subordinated,
            5_000_000.0,
            0.12,
        ),
        tr(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
        ),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-FUNDING", pool, tranches, close, maturity, "USD-OIS")
            .with_payment_calendar("nyse");
    deal.principal_covers_senior_interest = covers;
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    (deal, close)
}

fn simulate(deal: &StructuredCredit, as_of: Date) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market(as_of), as_of).expect("simulation")
}

fn flow_on(flows: &[(Date, Money)], date: Date) -> f64 {
    flows
        .iter()
        .filter(|(flow_date, _)| *flow_date == date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

fn context<'a>(market: &'a MarketContext, interest: f64, principal: f64) -> WaterfallContext<'a> {
    WaterfallContext {
        available_cash: usd(interest + principal),
        interest_collections: usd(interest),
        principal_collections: usd(principal),
        payment_date: d(2024, 4, 1),
        period_start: d(2024, 1, 1),
        valuation_date: d(2024, 1, 1),
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
fn template_funds_senior_coupons_from_principal_only_for_clo_deals() {
    let (clo_deal, _) = clo(0.08, 0.05, None);
    let waterfall = clo_deal
        .with_standard_fees()
        .create_waterfall()
        .expect("waterfall");
    let funding = |id: &str| {
        waterfall
            .tiers
            .iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("tier {id}"))
            .effective_funding()
    };
    assert_eq!(funding("fees"), FundingSource::InterestThenPrincipal);
    assert_eq!(funding("A_interest"), FundingSource::InterestThenPrincipal);
    for junior in ["B_interest", "C_interest", "D_interest"] {
        assert_eq!(
            funding(junior),
            FundingSource::Interest,
            "{junior} is not a senior coupon"
        );
    }
    assert_eq!(funding("principal"), FundingSource::Principal);
    assert_eq!(funding("equity"), FundingSource::Interest);

    let (mut abs_deal, _) = clo(0.08, 0.05, None);
    abs_deal.deal_type = DealType::Abs;
    let waterfall = abs_deal.create_waterfall().expect("waterfall");
    assert!(
        waterfall
            .tiers
            .iter()
            .all(|t| t.effective_funding() == FundingSource::default_for(t.payment_type)),
        "an ABS template keeps every tier on its default account"
    );
}

#[test]
fn interest_then_principal_tier_tops_up_the_senior_coupon_from_principal() {
    let (deal, _) = clo(0.08, 0.05, None);
    let market = MarketContext::new();
    // Class A's quarterly coupon is 60M × 5% × 91/360 ≈ 758k; interest
    // collections are 20% short of it while principal is ample.
    let a_coupon = 60_000_000.0 * 0.05 * 91.0 / 360.0;
    let interest = 0.8 * a_coupon;
    let principal = 5_000_000.0;
    let build = |source: Option<FundingSource>| {
        let mut tier = WaterfallTier::new("a_interest", 1, PaymentType::Interest)
            .add_recipient(Recipient::tranche_interest("a_int", "A"));
        if let Some(source) = source {
            tier = tier.funding(source);
        }
        WaterfallBuilder::new(Currency::USD)
            .add_tier(tier)
            .add_tier(WaterfallTier::coverage_tests(
                "a_coverage",
                2,
                vec![CoverageTestSpec::ic("A", 1.10)],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .add_recipient(Recipient::tranche_principal("a_prin", "A", None)),
            )
            .build()
            .expect("waterfall")
    };

    let separate = execute_waterfall(
        &build(None),
        &deal.tranches,
        &deal.pool,
        context(&market, interest, principal),
    )
    .expect("separate accounts");
    let funded = execute_waterfall(
        &build(Some(FundingSource::InterestThenPrincipal)),
        &deal.tranches,
        &deal.pool,
        context(&market, interest, principal),
    )
    .expect("cross funding");

    let paid = |result: &finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallDistribution,
                id: &str| {
        result
            .payment_records
            .iter()
            .filter(|r| r.recipient_id == id)
            .map(|r| r.paid_amount.amount())
            .sum::<f64>()
    };
    assert!((paid(&separate, "a_int") - interest).abs() < 1.0);
    assert!(
        (paid(&funded, "a_int") - a_coupon).abs() < 1.0,
        "A's coupon is paid in full, got {}",
        paid(&funded, "a_int")
    );
    let shortfall = a_coupon - interest;
    assert!((funded.principal_used_for_interest.amount() - shortfall).abs() < 1.0);
    assert!((funded.remaining_interest.amount()).abs() < 1.0);
    // The principal tier receives what is left after the top-up.
    assert!((paid(&funded, "a_prin") - (principal - shortfall)).abs() < 1.0);
    assert!((paid(&separate, "a_prin") - principal).abs() < 1.0);
    // Both runs conserve cash and report the same IC ratio: the test is
    // evaluated on interest collections, not on what funded the coupon.
    for result in [&separate, &funded] {
        let distributed: f64 = result.distributions.values().map(|m| m.amount()).sum();
        assert!(
            (distributed + result.remaining_cash.amount() - (interest + principal)).abs() < 1.0
        );
    }
    assert_eq!(separate.coverage_tests, funded.coverage_tests);
}

#[test]
fn clo_senior_coupon_shortfall_is_funded_from_principal_instead_of_deferring() {
    // Pool interest 100M × 4% is below A's 60M × 8% coupon every period;
    // 20% CPR supplies ample principal.
    let (funded_deal, as_of) = clo(0.04, 0.08, None);
    let (separate_deal, _) = clo(0.04, 0.08, Some(false));
    let funded = simulate(&funded_deal, as_of);
    let separate = simulate(&separate_deal, as_of);
    let first = d(2024, 4, 1);

    assert!(
        funded["A"].deferred_flows.is_empty(),
        "A's coupon is topped up from principal, got {:?}",
        funded["A"].deferred_flows.first()
    );
    let separate_first_deferral = flow_on(&separate["A"].deferred_flows, first);
    assert!(
        separate_first_deferral > 0.0,
        "with separate accounts the same deal defers A's coupon"
    );
    // The top-up reclassifies cash A would have received as principal: A's
    // total cash on the first date is unchanged, its coupon is whole, and its
    // principal is reduced by exactly the deferred amount.
    let a_interest = flow_on(&funded["A"].interest_flows, first);
    let a_principal = flow_on(&funded["A"].principal_flows, first);
    let sep_interest = flow_on(&separate["A"].interest_flows, first);
    let sep_principal = flow_on(&separate["A"].principal_flows, first);
    assert!(
        (a_interest - (sep_interest + separate_first_deferral)).abs() < 1.0,
        "funded coupon {a_interest} vs separate {sep_interest} + deferral {separate_first_deferral}"
    );
    assert!(
        (a_principal - (sep_principal - separate_first_deferral)).abs() < 1.0,
        "funded principal {a_principal} vs separate {sep_principal} - deferral {separate_first_deferral}"
    );
    assert!(
        funded["A"].final_balance.amount() < 1.0 && separate["A"].final_balance.amount() < 1.0,
        "A is repaid in full either way"
    );
}
