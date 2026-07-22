//! Unit tests for RoundingConvention behavior.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::RoundingConvention;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallContext;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PaymentCalculation, PaymentType, Recipient, RecipientType,
    TrancheStructure, WaterfallBuilder, WaterfallTier,
};

fn run_rounding_test(amount: f64, rounding: RoundingConvention) -> f64 {
    let currency = Currency::USD;
    let pool = AssetPool::new("TEST", DealType::CLO, currency);
    let tranches = TrancheStructure::new(vec![
        finstack_quant_valuations::instruments::fixed_income::structured_credit::Tranche::new(
            "A",
            0.0,
            100.0,
            finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheSeniority::Senior,
            Money::new(100.0, currency),
            finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCoupon::Fixed {
                rate: 0.05,
            },
            Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
        )
        .unwrap(),
    ])
    .unwrap();
    let market = MarketContext::new();

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "recipient",
                RecipientType::ServiceProvider("Test".into()),
                PaymentCalculation::PercentageOfCollateral {
                    rate: amount / 1_000_000.0,
                    annualized: false,
                    day_count: None,
                    rounding: Some(rounding),
                },
            )),
        )
        .build()
        .expect("build waterfall");

    let context = WaterfallContext {
        available_cash: Money::new(1_000_000.0, currency),
        interest_collections: Money::new(0.0, currency),
        principal_collections: Money::new(1_000_000.0, currency),
        payment_date: Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
        period_start: Date::from_calendar_date(2023, time::Month::October, 1).unwrap(),
        valuation_date: Date::from_calendar_date(2023, time::Month::October, 1).unwrap(),
        pool_balance: Money::new(1_000_000.0, currency),
        market: &market,
        tranche_balances: None,
        asset_balances: None,
        deferred_interest: None,
        reserve_balance: Money::new(0.0, currency),
        restricted_cash: Money::new(0.0, Currency::USD),
        recovery_proceeds: Money::new(0.0, currency),
        floating_rate_shift: 0.0,
    };

    let result =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::execute_waterfall(
            &waterfall, &tranches, &pool, context,
        )
        .unwrap();

    result
        .distributions
        .get(&RecipientType::ServiceProvider("Test".into()))
        .map(|m| m.amount())
        .unwrap_or(0.0)
}

#[test]
fn test_rounding_nearest() {
    // 100.125 -> 100.13 (Nearest)
    let val = run_rounding_test(100.125, RoundingConvention::Nearest);
    assert!((val - 100.13).abs() < 1e-10, "Expected 100.13, got {}", val);

    // 100.124 -> 100.12 (Nearest)
    let val = run_rounding_test(100.124, RoundingConvention::Nearest);
    assert!((val - 100.12).abs() < 1e-10, "Expected 100.12, got {}", val);
}

#[test]
fn test_rounding_floor() {
    // 100.129 -> 100.12 (Floor)
    let val = run_rounding_test(100.129, RoundingConvention::Floor);
    assert!((val - 100.12).abs() < 1e-10, "Expected 100.12, got {}", val);
}

#[test]
fn test_rounding_ceiling() {
    // 100.121 -> 100.13 (Ceiling)
    let val = run_rounding_test(100.121, RoundingConvention::Ceiling);
    assert!((val - 100.13).abs() < 1e-10, "Expected 100.13, got {}", val);
}
