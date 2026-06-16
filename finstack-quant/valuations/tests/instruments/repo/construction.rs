//! Tests for repo construction, builders, and validation.

use super::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::repo::{Repo, RepoBuilder, RepoType};
use finstack_quant_valuations::instruments::Attributes;
use rust_decimal::Decimal;

#[test]
fn test_overnight_repo_factory() {
    let collateral = treasury_collateral();
    let start_date = date(2025, 1, 15);

    let repo = Repo::overnight(
        "REPO_ON_001",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        start_date,
        "usny",    // Calendar ID for business day adjustment
        "USD-OIS", // Discount curve ID
    )
    .unwrap();

    assert_eq!(repo.id.as_str(), "REPO_ON_001");
    assert_eq!(repo.repo_type, RepoType::Overnight);
    assert_eq!(repo.start_date, start_date);
    assert!(repo.maturity > start_date, "Maturity must be after start");
    assert_eq!(repo.cash_amount.amount(), 1_000_000.0);
    assert_eq!(
        repo.repo_rate,
        Decimal::try_from(0.05).expect("valid decimal")
    );
    assert_eq!(repo.day_count, DayCount::Act360);
}

#[test]
fn test_term_repo_factory() {
    let collateral = treasury_collateral();
    let start_date = date(2025, 1, 15);
    let maturity = date(2025, 4, 15);

    let repo = Repo::term(
        "REPO_TERM_001",
        Money::new(2_000_000.0, Currency::USD),
        collateral,
        0.045,
        start_date,
        maturity,
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    assert_eq!(repo.id.as_str(), "REPO_TERM_001");
    assert_eq!(repo.repo_type, RepoType::Term);
    assert_eq!(repo.start_date, start_date);
    assert_eq!(repo.maturity, maturity);
    assert_eq!(repo.cash_amount.amount(), 2_000_000.0);
    assert_eq!(
        repo.repo_rate,
        Decimal::try_from(0.045).expect("valid decimal")
    );
}

#[test]
fn test_open_repo_factory() {
    let collateral = treasury_collateral();
    let start_date = date(2025, 1, 15);
    let initial_maturity = date(2025, 12, 15);

    let repo = Repo::open(
        "REPO_OPEN_001",
        Money::new(1_500_000.0, Currency::USD),
        collateral,
        0.05,
        start_date,
        initial_maturity,
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    assert_eq!(repo.repo_type, RepoType::Open);
    assert_eq!(repo.start_date, start_date);
    assert_eq!(repo.maturity, initial_maturity);
}

#[test]
fn test_builder_with_all_fields() {
    let collateral = treasury_collateral();

    let repo = RepoBuilder::new()
        .id("REPO_FULL".into())
        .cash_amount(Money::new(5_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.055).expect("valid decimal"))
        .start_date(date(2025, 1, 10))
        .maturity(date(2025, 7, 10))
        .repo_type(RepoType::Term)
        .haircut(0.025)
        .day_count(DayCount::Act365F)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("nyc".into()))
        .triparty(true)
        .discount_curve_id("USD-OIS".into())
        .attributes(
            Attributes::new()
                .with_tag("funding")
                .with_tag("bilateral")
                .with_meta("desk", "rates_trading")
                .with_meta("book", "REPO_DESK_01"),
        )
        .build()
        .unwrap();

    assert_eq!(repo.id.as_str(), "REPO_FULL");
    assert_eq!(repo.cash_amount.amount(), 5_000_000.0);
    assert_eq!(
        repo.repo_rate,
        Decimal::try_from(0.055).expect("valid decimal")
    );
    assert_eq!(repo.haircut, 0.025);
    assert_eq!(repo.day_count, DayCount::Act365F);
    assert_eq!(repo.bdc, BusinessDayConvention::ModifiedFollowing);
    assert!(repo.triparty);
    assert!(repo.attributes.has_tag("funding"));
    assert!(repo.attributes.has_tag("bilateral"));
    assert_eq!(repo.attributes.get_meta("desk"), Some("rates_trading"));
}

#[test]
fn test_builder_minimal_fields() {
    let collateral = treasury_collateral();

    let repo = RepoBuilder::new()
        .id("MINIMAL".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
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

    // Check values are set correctly
    assert_eq!(repo.haircut, 0.02);
    assert_eq!(repo.repo_type, RepoType::Term);
    assert!(!repo.triparty);
}

#[test]
fn test_builder_missing_required_field_fails() {
    let result = RepoBuilder::new()
        .id("INCOMPLETE".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        // Missing collateral, rate, dates, etc.
        .build();

    assert!(result.is_err(), "Builder should fail with missing fields");
}

#[test]
fn test_validation_maturity_before_start() {
    let collateral = treasury_collateral();

    let result = RepoBuilder::new()
        .id("INVALID_DATES".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 4, 15))
        .maturity(date(2025, 1, 15)) // Maturity before start
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err(), "Should reject maturity before start date");
}

#[test]
fn test_validation_same_day_maturity() {
    let collateral = treasury_collateral();
    let same_date = date(2025, 1, 15);

    let result = RepoBuilder::new()
        .id("SAME_DAY".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(same_date)
        .maturity(same_date)
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err(), "Should reject same-day start and maturity");
}

#[test]
fn test_validation_negative_rate() {
    let collateral = treasury_collateral();

    let result = RepoBuilder::new()
        .id("NEGATIVE_RATE".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(-0.01).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err(), "Should reject negative repo rate");
}

#[test]
fn test_validation_negative_haircut() {
    let collateral = treasury_collateral();

    let result = RepoBuilder::new()
        .id("NEGATIVE_HAIRCUT".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(-0.05)
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err(), "Should reject negative haircut");
}

#[test]
fn test_validation_zero_or_negative_cash() {
    let collateral = treasury_collateral();

    let result = RepoBuilder::new()
        .id("ZERO_CASH".into())
        .cash_amount(Money::new(0.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .discount_curve_id("USD-OIS".into())
        .build();

    assert!(result.is_err(), "Should reject zero cash amount");
}

#[test]
fn test_overnight_repo_business_day_adjustment() {
    let collateral = treasury_collateral();

    // Friday Jan 17, 2025
    let friday = date(2025, 1, 17);

    let repo = Repo::overnight(
        "REPO_WEEKEND",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        friday,
        "usny",    // Calendar ID for business day adjustment
        "USD-OIS", // Discount curve ID
    )
    .unwrap();

    // Should mature on Tuesday Jan 21 (skipping weekend AND MLK Day on Jan 20)
    // Note: January 20, 2025 is Martin Luther King Jr. Day, a US federal holiday
    let tuesday = date(2025, 1, 21);
    assert_eq!(
        repo.maturity, tuesday,
        "Overnight repo starting Friday should mature Tuesday (MLK Day on Jan 20)"
    );
}

#[test]
fn test_repo_type_display() {
    assert_eq!(RepoType::Term.to_string(), "term");
    assert_eq!(RepoType::Open.to_string(), "open");
    assert_eq!(RepoType::Overnight.to_string(), "overnight");
}

#[test]
fn test_repo_type_from_str() {
    use std::str::FromStr;

    assert_eq!(RepoType::from_str("term").unwrap(), RepoType::Term);
    assert_eq!(RepoType::from_str("TERM").unwrap(), RepoType::Term);
    assert_eq!(RepoType::from_str("open").unwrap(), RepoType::Open);
    assert_eq!(
        RepoType::from_str("overnight").unwrap(),
        RepoType::Overnight
    );

    assert!(RepoType::from_str("invalid").is_err());
}

#[test]
fn test_attributes_tagging() {
    let collateral = treasury_collateral();

    let repo = RepoBuilder::new()
        .id("TAGGED".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
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
        .attributes(
            Attributes::new()
                .with_tag("treasury")
                .with_tag("short_term")
                .with_tag("funding")
                .with_meta("counterparty", "DEALER_A")
                .with_meta("settlement", "DVP"),
        )
        .build()
        .unwrap();

    assert!(repo.attributes.has_tag("treasury"));
    assert!(repo.attributes.has_tag("short_term"));
    assert!(repo.attributes.has_tag("funding"));
    assert!(!repo.attributes.has_tag("nonexistent"));

    assert_eq!(repo.attributes.get_meta("counterparty"), Some("DEALER_A"));
    assert_eq!(repo.attributes.get_meta("settlement"), Some("DVP"));
    assert_eq!(repo.attributes.get_meta("missing"), None);
}
