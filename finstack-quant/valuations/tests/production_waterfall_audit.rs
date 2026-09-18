//! Independent cash-account and priority contracts for structured credit.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{currency::Currency, money::Money};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, FundingSource, PaymentCalculation, PaymentType, Recipient, RecipientType,
    StructuredCredit, Waterfall, WaterfallContext, WaterfallDistribution, WaterfallTier,
};
use time::macros::date;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("finite USD")
}

fn fee(id: &str, priority: usize, amount: f64) -> WaterfallTier {
    WaterfallTier::new(id, priority, PaymentType::Fee).add_recipient(Recipient::new(
        id,
        RecipientType::ReserveAccount(id.into()),
        PaymentCalculation::FixedAmount {
            amount: usd(amount),
            rounding: None,
        },
    ))
}

fn execute(
    waterfall: &Waterfall,
    interest: f64,
    principal: f64,
) -> finstack_quant_core::Result<WaterfallDistribution> {
    let deal = StructuredCredit::example();
    let market = MarketContext::new();
    execute_waterfall(
        waterfall,
        &deal.tranches,
        &deal.pool,
        WaterfallContext {
            available_cash: usd(interest + principal),
            interest_collections: usd(interest),
            principal_collections: usd(principal),
            payment_date: date!(2024 - 04 - 01),
            period_start: date!(2024 - 01 - 01),
            valuation_date: date!(2024 - 01 - 01),
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
        },
    )
}

#[test]
fn production_waterfall_principal_cannot_fund_fees_unless_the_tier_draws_on_it() {
    let waterfall = Waterfall::new(Currency::USD).add_tier(fee("fee", 1, 50.0));
    let result = execute(&waterfall, 10.0, 100.0).expect("waterfall");
    assert_eq!(result.tier_allocations[0].1, usd(10.0));
    assert_eq!(result.remaining_cash, usd(100.0));
    assert_eq!(result.principal_used_for_interest, usd(0.0));

    // The same tier funded interest-then-principal tops up from principal.
    let waterfall = Waterfall::new(Currency::USD)
        .add_tier(fee("fee", 1, 50.0).funding(FundingSource::InterestThenPrincipal));
    let result = execute(&waterfall, 10.0, 100.0).expect("waterfall");
    assert_eq!(result.tier_allocations[0].1, usd(50.0));
    assert_eq!(result.principal_used_for_interest, usd(40.0));
    assert_eq!(result.remaining_interest, usd(0.0));
    assert_eq!(result.remaining_principal, usd(60.0));
    assert_eq!(result.remaining_cash, usd(60.0));
}

#[test]
fn production_waterfall_interest_cannot_repay_principal_without_diversion() {
    let deal = StructuredCredit::example();
    let waterfall = Waterfall::new(Currency::USD).add_tier(
        WaterfallTier::new("principal", 1, PaymentType::Principal).add_recipient(
            Recipient::tranche_principal("debt", deal.tranches.tranches[0].id.as_str(), None),
        ),
    );
    let result = execute(&waterfall, 100.0, 10.0).expect("waterfall");
    assert_eq!(result.tier_allocations[0].1, usd(10.0));
    assert_eq!(result.remaining_cash, usd(100.0));
}

#[test]
fn production_waterfall_deserialized_tiers_execute_by_priority() {
    let mut waterfall = Waterfall::new(Currency::USD);
    waterfall.tiers = vec![fee("junior", 2, 30.0), fee("senior", 1, 20.0)];
    let wire = serde_json::to_string(&waterfall).expect("serialize");
    let waterfall: Waterfall = serde_json::from_str(&wire).expect("deserialize");
    let result = execute(&waterfall, 25.0, 0.0).expect("waterfall");
    assert_eq!(
        result.tier_allocations,
        vec![("senior".into(), usd(20.0)), ("junior".into(), usd(5.0))]
    );
}

#[test]
fn production_waterfall_duplicate_priorities_are_rejected() {
    let mut waterfall = Waterfall::new(Currency::USD);
    waterfall.tiers = vec![fee("a", 1, 20.0), fee("b", 1, 20.0)];
    assert!(execute(&waterfall, 25.0, 0.0).is_err());
}
