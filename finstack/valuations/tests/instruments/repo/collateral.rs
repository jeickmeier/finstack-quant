//! Tests for collateral specification, valuation, and adequacy.

use super::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::repo::{
    CollateralSpec, CollateralType, Repo, RepoType,
};
use finstack_valuations::instruments::Attributes;
use rust_decimal::Decimal;

#[allow(unused_imports)]
use finstack_valuations::instruments::rates::repo as _;

#[test]
fn test_general_collateral_creation() {
    let collateral = CollateralSpec::new("TREASURY", 1_000_000.0, "TREASURY_BOND_PRICE");

    assert_eq!(collateral.instrument_id, "TREASURY");
    assert_eq!(collateral.quantity, 1_000_000.0);
    assert_eq!(collateral.market_value_id, "TREASURY_BOND_PRICE");
    assert!(matches!(
        collateral.collateral_type,
        CollateralType::General
    ));
}

#[test]
fn test_special_collateral_creation() {
    let collateral = CollateralSpec::special(
        "ON_THE_RUN_10Y",
        "TREASURY_10Y",
        1_000_000.0,
        "TREASURY_10Y_PRICE",
        Some(-25.0),
    );

    assert_eq!(collateral.instrument_id, "TREASURY_10Y");

    match &collateral.collateral_type {
        CollateralType::Special {
            security_id,
            rate_adjustment_bp,
        } => {
            assert_eq!(security_id, "ON_THE_RUN_10Y");
            assert_eq!(*rate_adjustment_bp, Some(-25.0));
        }
        _ => panic!("Expected special collateral type"),
    }
}

#[test]
fn test_collateral_market_value_calculation() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let market_value = collateral.market_value(&context).unwrap();

    // 1,000,000 * 1.02 = 1,020,000
    assert_money_approx_eq(market_value, Money::new(1_020_000.0, Currency::USD), 1.0);
}

#[test]
fn test_collateral_value_different_prices() {
    let context = create_standard_market_context();

    // Corporate bond at 98%
    let corp_collateral = corporate_collateral();
    let corp_value = corp_collateral.market_value(&context).unwrap();
    assert_money_approx_eq(corp_value, Money::new(980_000.0, Currency::USD), 1.0);

    // Special bond at 105%
    let special = CollateralSpec::new("SPECIAL_BOND", 500_000.0, "SPECIAL_BOND_PRICE");
    let special_value = special.market_value(&context).unwrap();
    assert_money_approx_eq(special_value, Money::new(525_000.0, Currency::USD), 1.0);
}

#[test]
fn test_collateral_value_requires_currency_price() {
    use finstack_core::market_data::scalars::MarketScalar;

    let context =
        create_standard_market_context().insert_price("UNITLESS", MarketScalar::Unitless(1.0));

    let collateral = CollateralSpec::new("BOND", 1_000_000.0, "UNITLESS");

    // Should error because unitless prices don't provide currency
    assert!(collateral.market_value(&context).is_err());
}

#[test]
fn test_collateral_value_missing_price() {
    let context = create_standard_market_context();
    let collateral = CollateralSpec::new("MISSING", 1_000_000.0, "NONEXISTENT_PRICE");

    assert!(collateral.market_value(&context).is_err());
}

#[test]
fn test_required_collateral_with_haircut() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "HAIRCUT_TEST",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let required = repo.required_collateral_value();

    // 1,000,000 / (1 - 0.05) = 1,052,631.58 (Repo::term uses rate as haircut? No, Repo::term defaults haircut to 0.02)
    // Repo::term implementation: haircut(0.02)
    // 1,000,000 / (1 - 0.02) = 1,020,408.16
    assert_money_approx_eq(
        required.unwrap(),
        Money::new(1_020_408.16, Currency::USD),
        1.0,
    );
}

#[test]
fn test_required_collateral_high_haircut() {
    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("HIGH_HAIRCUT".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.20) // 20% haircut
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    let required = repo.required_collateral_value();

    // 1,000,000 / (1 - 0.20) = 1,250,000
    assert_money_approx_eq(
        required.unwrap(),
        Money::new(1_250_000.0, Currency::USD),
        1.0,
    );
}

#[test]
fn test_required_collateral_zero_haircut() {
    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("ZERO_HAIRCUT".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.0)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    let required = repo.required_collateral_value();

    assert_money_approx_eq(
        required.unwrap(),
        Money::new(1_000_000.0, Currency::USD),
        1.0,
    );
}

#[test]
fn test_adequately_collateralized() {
    let context = create_standard_market_context();
    let mut collateral = treasury_collateral();
    // Increase quantity slightly to cover the 1/(1-h) calculation which requires ~1,020,408
    collateral.quantity = 1_000_500.0; // Value = 1,020,510

    let repo = Repo::term(
        "ADEQUATE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Collateral value: 1,020,000
    // Required: 1,020,000
    // Should be adequate
    let is_adequate = repo.is_adequately_collateralized(&context).unwrap();
    assert!(is_adequate);
}

#[test]
fn test_undercollateralized() {
    let context = create_standard_market_context();
    let collateral = insufficient_collateral(); // High yield at 85%

    let repo = Repo::term(
        "INSUFFICIENT",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Collateral value: 850,000
    // Required: 1,020,000
    // Should be inadequate
    let is_adequate = repo.is_adequately_collateralized(&context).unwrap();
    assert!(!is_adequate);
}

#[test]
fn test_overcollateralized() {
    let context = create_standard_market_context();

    // Use special bond at 105% price
    let collateral = CollateralSpec::new("SPECIAL_BOND", 1_000_000.0, "SPECIAL_BOND_PRICE");

    let repo = Repo::term(
        "OVERCOLLATERALIZED",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Collateral value: 1,050,000
    // Required: 1,020,000
    // Should be adequate with cushion
    let is_adequate = repo.is_adequately_collateralized(&context).unwrap();
    assert!(is_adequate);
}

#[test]
fn test_special_collateral_rate_adjustment_negative() {
    let collateral = special_collateral(-25.0); // 25bp lower

    let repo = Repo::term(
        "SPECIAL_NEGATIVE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05, // Base 5%
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let effective_rate = repo.effective_rate().expect("effective rate");

    // 5% - 25bp = 4.75%
    assert_approx_eq(effective_rate, 0.0475, 1e-9);
}

#[test]
fn test_special_collateral_rate_adjustment_positive() {
    let collateral = special_collateral(10.0); // 10bp higher

    let repo = Repo::term(
        "SPECIAL_POSITIVE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05, // Base 5%
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let effective_rate = repo.effective_rate().expect("effective rate");

    // 5% + 10bp = 5.10%
    assert_approx_eq(effective_rate, 0.0510, 1e-9);
}

#[test]
fn test_general_collateral_effective_rate() {
    let collateral = treasury_collateral(); // General collateral

    let repo = Repo::term(
        "GENERAL",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let effective_rate = repo.effective_rate().expect("effective rate");

    // No adjustment for general collateral
    assert_approx_eq(effective_rate, 0.05, 1e-9);
}

#[test]
fn test_collateral_currency_safety() {
    use finstack_core::market_data::scalars::MarketScalar;

    // Mix EUR and USD
    let context = create_standard_market_context().insert_price(
        "EUR_BOND_PRICE",
        MarketScalar::Price(Money::new(1.0, Currency::EUR)),
    );

    let eur_collateral = CollateralSpec::new("EUR_BOND", 1_000_000.0, "EUR_BOND_PRICE");

    let repo = Repo::term(
        "CURRENCY_MIX",
        Money::new(1_000_000.0, Currency::USD), // USD cash
        eur_collateral,                         // EUR collateral
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Should error due to currency mismatch
    let result = repo.is_adequately_collateralized(&context);
    assert!(result.is_err());
}
