//! Edge case and boundary condition tests for repo instruments.

use super::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, Repo, RepoType};
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use rust_decimal::Decimal;

#[test]
fn test_very_small_notional() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "SMALL_NOTIONAL",
        Money::new(1.0, Currency::USD), // $1
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();
    assert!(interest.amount() > 0.0);
    assert!(interest.amount() < 0.1);
}

#[test]
fn test_very_large_notional() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "LARGE_NOTIONAL",
        Money::new(1_000_000_000_000.0, Currency::USD), // $1 trillion
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();
    assert!(interest.amount() > 1_000_000_000.0); // > $1B interest
}

#[test]
fn test_extremely_high_haircut() {
    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("EXTREME_HAIRCUT".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.50) // 50% haircut
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

    // Required collateral = Cash / (1 - haircut) = 1M / (1 - 0.50) = 2M
    assert_money_approx_eq(
        required.unwrap(),
        Money::new(2_000_000.0, Currency::USD),
        1.0,
    );
}

#[test]
fn test_very_short_term() {
    let collateral = treasury_collateral();

    // Same day maturity should fail
    let same_day = date(2025, 1, 15);
    let result = Repo::builder()
        .id("SAME_DAY".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(same_day)
        .maturity(same_day)
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err());
}

#[test]
fn test_very_long_term() {
    let collateral = treasury_collateral();

    // 10-year repo
    let repo = Repo::term(
        "LONG_TERM",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 1),
        date(2035, 1, 1),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();

    // 10 years * 5% * 1M = 500K
    assert!(interest.amount() > 400_000.0);
    assert!(interest.amount() < 600_000.0);
}

#[test]
fn test_zero_rate_repo() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "ZERO_RATE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.0,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();
    let total = repo.total_repayment().unwrap();

    assert_approx_eq(interest.amount(), 0.0, 1e-6);
    assert_money_approx_eq(total, Money::new(1_000_000.0, Currency::USD), 0.01);
}

#[test]
fn test_extreme_rate() {
    let collateral = treasury_collateral();

    // 100% annual rate
    let repo = Repo::term(
        "EXTREME_RATE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        1.0,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();

    // ~100% * 0.25 years * 1M = 250K
    assert!(interest.amount() > 200_000.0);
    assert!(interest.amount() < 300_000.0);
}

#[test]
fn test_valuation_far_before_start() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "FAR_BEFORE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 6, 1),
        date(2025, 12, 1),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Value 6 months before start
    let pv = repo.value(&context, date(2025, 1, 1)).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    // Should have some meaningful value due to future cashflows
}

#[test]
fn test_valuation_after_maturity() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "AFTER_MAT",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Value after maturity
    let pv = repo.value(&context, date(2025, 5, 1)).unwrap();

    // Past maturity, cashflows are in the past
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn test_missing_discount_curve() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "MISSING_CURVE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "NONEXISTENT-CURVE",
    )
    .expect("Repo construction should succeed");

    let result = repo.value(&context, date(2025, 1, 10));

    assert!(result.is_err());
}

#[test]
fn test_collateral_with_zero_quantity() {
    let collateral = CollateralSpec::new("TREASURY", 0.0, "TREASURY_BOND_PRICE");

    let context = create_standard_market_context();
    let market_value = collateral.market_value(&context).unwrap();

    assert_money_approx_eq(market_value, Money::new(0.0, Currency::USD), 1e-6);
}

#[test]
fn test_collateral_with_negative_price() {
    let context = create_standard_market_context().insert_price(
        "NEGATIVE_PRICE",
        MarketScalar::Price(Money::new(-1.0, Currency::USD)),
    );

    let collateral = CollateralSpec::new("WEIRD_BOND", 1_000_000.0, "NEGATIVE_PRICE");

    let market_value = collateral.market_value(&context).unwrap();

    // Negative price * positive quantity = negative value
    assert!(market_value.amount() < 0.0);
}

#[test]
fn test_repo_with_multiple_currencies() {
    let collateral = treasury_collateral();

    // GBP repo
    let gbp_repo = Repo::term(
        "GBP_REPO",
        Money::new(1_000_000.0, Currency::GBP),
        collateral,
        0.045,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = gbp_repo.interest_amount().unwrap();
    assert_eq!(interest.currency(), Currency::GBP);

    let total = gbp_repo.total_repayment().unwrap();
    assert_eq!(total.currency(), Currency::GBP);
}

#[test]
fn test_special_collateral_without_rate_adjustment() {
    let collateral = CollateralSpec::special(
        "SPECIAL_ID",
        "SPECIAL_BOND",
        500_000.0,
        "SPECIAL_BOND_PRICE",
        None, // No rate adjustment
    );

    let repo = Repo::term(
        "SPECIAL_NO_ADJ",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let effective_rate = repo.effective_rate().expect("effective rate");

    // No adjustment, so should equal base rate
    assert_approx_eq(effective_rate, 0.05, 1e-9);
}

#[test]
fn test_business_day_conventions() {
    let collateral = treasury_collateral();

    // Following
    let following = Repo::builder()
        .id("FOLLOWING".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral.clone())
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    assert_eq!(following.bdc, BusinessDayConvention::Following);

    // Modified Following
    let mod_following = Repo::builder()
        .id("MOD_FOLLOWING".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    assert_eq!(mod_following.bdc, BusinessDayConvention::ModifiedFollowing);
}

#[test]
fn test_leap_year_date_handling() {
    let collateral = treasury_collateral();

    // Leap day 2024-02-29
    let repo = Repo::term(
        "LEAP_YEAR",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2024, 2, 29),
        date(2024, 5, 29),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    assert_eq!(repo.start_date, date(2024, 2, 29));

    let interest = repo.interest_amount().unwrap();
    assert!(interest.amount() > 0.0);
}

#[test]
fn test_precision_with_small_amounts() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "SMALL_PRECISION",
        Money::new(0.01, Currency::USD), // 1 cent
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();

    // Interest on 1 cent should be extremely small but computable
    assert!(interest.amount() >= 0.0);
    assert!(interest.amount() < 0.01);
}
