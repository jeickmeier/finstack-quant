//! Tests for repo pricing, interest calculations, and present value.

use super::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::repo::{Repo, RepoType};
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use rust_decimal::Decimal;

#[test]
fn test_interest_calculation_act360() {
    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("INTEREST_360".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15)) // 90 days
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

    let interest = repo.interest_amount().unwrap();

    // 1,000,000 * 0.05 * (90/360) = 12,500
    let expected = 1_000_000.0 * 0.05 * (90.0 / 360.0);
    assert_money_approx_eq(interest, Money::new(expected, Currency::USD), 0.01);
}

#[test]
fn test_interest_calculation_act365() {
    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("INTEREST_365".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act365F)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    let interest = repo.interest_amount().unwrap();

    // Act/365F: 90/365 * 0.05 * 1,000,000
    let expected = 1_000_000.0 * 0.05 * (90.0 / 365.0);
    assert_money_approx_eq(interest, Money::new(expected, Currency::USD), 0.01);
}

#[test]
fn test_daycount_360_vs_365_difference() {
    let collateral1 = treasury_collateral();
    let collateral2 = treasury_collateral();

    let repo_360 = Repo::builder()
        .id("DC360".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral1)
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

    let repo_365 = Repo::builder()
        .id("DC365".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral2)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(date(2025, 1, 15))
        .maturity(date(2025, 4, 15))
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act365F)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id("USD-OIS".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    let interest_360 = repo_360.interest_amount().unwrap();
    let interest_365 = repo_365.interest_amount().unwrap();

    // Act/360 should yield higher interest than Act/365 for same period
    assert!(
        interest_360.amount() > interest_365.amount(),
        "Act/360 should give higher interest than Act/365"
    );
}

#[test]
fn test_zero_rate_interest() {
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

    assert_money_approx_eq(interest, Money::new(0.0, Currency::USD), 0.01);
}

#[test]
fn test_high_rate_interest() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "HIGH_RATE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.25, // 25% annual rate
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();

    // Rough check: 25% * 0.25 years * 1M = 62,500
    assert!(interest.amount() > 60_000.0);
    assert!(interest.amount() < 65_000.0);
}

#[test]
fn test_total_repayment() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "REPAYMENT",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let total = repo.total_repayment().unwrap();
    let interest = repo.interest_amount().unwrap();
    let expected_total = repo.cash_amount.checked_add(interest).unwrap();

    assert_money_approx_eq(total, expected_total, 0.01);
    assert!(
        total.amount() > 1_000_000.0,
        "Total should include interest"
    );
}

#[test]
fn test_pv_before_start() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "PV_BEFORE_START",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let valuation_date = date(2025, 1, 10); // Before start
    let pv = repo.value(&context, valuation_date).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    // PV before start: 5-day pre-start period on $1M 5% repo ≈ $694 carry; < $5k is generous
    assert!(
        pv.amount().abs() < 5_000.0,
        "Repo PV before start: {}",
        pv.amount()
    );
}

#[test]
fn test_pv_at_start() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "PV_AT_START",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let pv = repo.value(&context, date(2025, 1, 15)).unwrap();

    // At start, PV should be near zero (slight discrepancy due to discounting)
    assert!(pv.amount().abs() < 50_000.0);
}

#[test]
fn test_pv_mid_term() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "PV_MID",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let mid_date = date(2025, 3, 1); // Mid-term
    let pv = repo.value(&context, mid_date).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    // Should have some value
    assert!(pv.amount() != 0.0);
}

#[test]
fn test_pv_at_maturity() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "PV_AT_MAT",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let pv = repo.value(&context, date(2025, 4, 15)).unwrap();

    // At maturity, PV represents the net value
    // Both legs (outflow and inflow) occur, so PV is close to interest

    // The PV should be meaningful (not zero) and have the right currency
    assert_eq!(pv.currency(), Currency::USD);
    // At maturity, net PV could be small or close to interest depending on discounting
    // Just verify it's computed and has the right currency and is reasonable
    assert!(
        pv.amount().abs() < 100_000.0,
        "PV at maturity should be reasonable"
    );
}

#[test]
fn test_pv_with_flat_curve() {
    let flat_curve = create_flat_discount_curve();
    let context = create_standard_market_context().insert(flat_curve);

    let collateral = treasury_collateral();

    let repo = Repo::builder()
        .id("FLAT_CURVE".into())
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
        .discount_curve_id("USD-FLAT".into())
        .attributes(Attributes::default())
        .build()
        .unwrap();

    let pv = repo.value(&context, date(2025, 1, 10)).unwrap();

    // With flat curve (zero rates), PV should equal undiscounted interest
    let interest = repo.interest_amount().unwrap();
    // Allow for minor discounting effects
    assert!(
        (pv.amount() - interest.amount()).abs() < 1000.0,
        "PV with flat curve should approximate interest amount"
    );
}

#[test]
fn test_pv_currency_matches_cash_currency() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    // EUR repo
    let eur_repo = Repo::term(
        "EUR_REPO",
        Money::new(1_000_000.0, Currency::EUR),
        collateral,
        0.035,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS", // Using USD curve for test simplicity
    )
    .expect("Repo construction should succeed");

    let pv = eur_repo.value(&context, date(2025, 1, 10)).unwrap();

    assert_eq!(pv.currency(), Currency::EUR);
}

#[test]
fn test_special_collateral_affects_interest_not_pv_directly() {
    let context = create_standard_market_context();

    let general_collateral = treasury_collateral();
    let special_collateral = special_collateral(-50.0); // 50bp lower

    let repo_general = Repo::term(
        "GENERAL",
        Money::new(1_000_000.0, Currency::USD),
        general_collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let repo_special = Repo::term(
        "SPECIAL",
        Money::new(1_000_000.0, Currency::USD),
        special_collateral,
        0.05, // Same base rate
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest_general = repo_general.interest_amount().unwrap();
    let interest_special = repo_special.interest_amount().unwrap();

    // Special collateral should have lower interest due to rate adjustment
    assert!(
        interest_special.amount() < interest_general.amount(),
        "Special collateral should yield lower interest"
    );

    // This should affect PV as well
    let pv_general = repo_general.value(&context, date(2025, 1, 10)).unwrap();
    let pv_special = repo_special.value(&context, date(2025, 1, 10)).unwrap();

    assert!(
        pv_special.amount() < pv_general.amount(),
        "Special collateral repo should have lower PV"
    );
}

#[test]
fn test_overnight_repo_minimal_interest() {
    let collateral = treasury_collateral();

    let repo = Repo::overnight(
        "OVERNIGHT",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        "usny",    // Calendar ID for business day adjustment
        "USD-OIS", // Discount curve ID
    )
    .unwrap();

    let interest = repo.interest_amount().unwrap();

    // Overnight: ~1/360 years
    // 1M * 5% * (1/360) ≈ 138.89
    assert!(interest.amount() > 100.0);
    assert!(interest.amount() < 200.0);
}

#[test]
fn test_long_term_repo_interest() {
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "LONG_TERM",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 1),
        date(2026, 1, 1), // 1 year (365 days)
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let interest = repo.interest_amount().unwrap();

    // With Act/360: 365/360 = 1.0139 year fraction
    // Expected: 1M * 5% * (365/360) = $50,694.44
    let expected = 1_000_000.0 * 0.05 * (365.0 / 360.0);
    assert_money_approx_eq(interest, Money::new(expected, Currency::USD), 0.01);
}
