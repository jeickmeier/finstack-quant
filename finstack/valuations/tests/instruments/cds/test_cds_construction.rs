//! CDS construction and conventions tests.
//!
//! Tests basic CDS creation, builder patterns, convention mappings,
//! and structural validation.

use crate::finstack_test_utils as test_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::money::Money;
use finstack_valuations::instruments::credit_derivatives::cds::{CDSConvention, PayReceive};
use rust_decimal::Decimal;
use time::Month;

/// Helper to create standard test date
fn test_date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

#[test]
fn test_buy_protection_constructor() {
    let notional = Money::new(10_000_000.0, Currency::USD);
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "CDS_BUY_TEST",
        notional,
        100.0, // 100bp
        start,
        end,
        "USD-OIS",
        "CORP-SENIOR",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.id.as_str(), "CDS_BUY_TEST");
    assert_eq!(cds.notional.amount(), 10_000_000.0);
    assert_eq!(cds.notional.currency(), Currency::USD);
    assert_eq!(cds.side, PayReceive::Pay);
    assert_eq!(cds.premium.spread_bp, Decimal::from(100));
    assert_eq!(cds.convention, CDSConvention::IsdaNa);
}

#[test]
fn test_sell_protection_constructor() {
    let notional = Money::new(5_000_000.0, Currency::EUR);
    let start = test_date(2025, Month::March, 20);
    let end = test_date(2028, Month::March, 20);

    let cds = test_utils::cds_sell_protection(
        "CDS_SELL_TEST",
        notional,
        150.0,
        start,
        end,
        "EUR-ESTR",
        "CORP-SUB",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.side, PayReceive::Receive);
    assert_eq!(cds.premium.spread_bp, Decimal::from(150));
    assert_eq!(cds.notional.currency(), Currency::EUR);
}

#[test]
fn test_convention_na_mappings() {
    let conv = CDSConvention::IsdaNa;
    assert_eq!(conv.day_count(), DayCount::Act360);
    assert_eq!(conv.frequency(), Tenor::quarterly());
    assert_eq!(conv.settlement_delay(), 3);
}

#[test]
fn test_convention_eu_mappings() {
    let conv = CDSConvention::IsdaEu;
    assert_eq!(conv.day_count(), DayCount::Act360);
    assert_eq!(conv.frequency(), Tenor::quarterly());
    // EU settlement changed from T+3 to T+1 on June 20, 2009 (ISDA Big Bang)
    assert_eq!(conv.settlement_delay(), 1);
}

#[test]
fn test_convention_as_mappings() {
    let conv = CDSConvention::IsdaAs;
    assert_eq!(conv.day_count(), DayCount::Act365F);
    assert_eq!(conv.frequency(), Tenor::quarterly());
    assert_eq!(conv.settlement_delay(), 3);
}

#[test]
fn test_convention_custom_defaults() {
    let conv = CDSConvention::Custom;
    assert_eq!(conv.day_count(), DayCount::Act360);
    assert_eq!(conv.frequency(), Tenor::quarterly());
    assert_eq!(conv.settlement_delay(), 3);
}

#[test]
fn test_builder_pattern() {
    use finstack_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
    use finstack_valuations::instruments::credit_derivatives::cds::{
        PremiumLegSpec, ProtectionLegSpec,
    };
    use finstack_valuations::instruments::Attributes;
    use finstack_valuations::instruments::PricingOverrides;

    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);
    let convention = CDSConvention::IsdaNa;

    let cds = CreditDefaultSwap::builder()
        .id("BUILDER_TEST".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .convention(convention)
        .premium(PremiumLegSpec {
            start,
            end,
            frequency: convention.frequency(),
            stub: convention.stub_convention(),
            bdc: convention.business_day_convention(),
            calendar_id: None,
            day_count: convention.day_count(),
            spread_bp: Decimal::from(200),
            discount_curve_id: "USD-OIS".into(),
        })
        .protection(ProtectionLegSpec {
            credit_curve_id: "CORP".into(),
            recovery_rate: 0.40,
            settlement_delay: 3,
        })
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(cds.id.as_str(), "BUILDER_TEST");
    assert_eq!(cds.premium.spread_bp, Decimal::from(200));
    assert_eq!(cds.protection.recovery_rate, 0.40);
}

#[test]
fn test_recovery_rate_applied() {
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let mut cds = test_utils::cds_buy_protection(
        "RECOVERY_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // Default recovery for senior unsecured
    assert_eq!(cds.protection.recovery_rate, 0.40);

    // Override recovery rate
    cds.protection.recovery_rate = 0.25;
    assert_eq!(cds.protection.recovery_rate, 0.25);
}

#[test]
fn test_notional_zero_allowed() {
    // Zero notional should be constructible (useful for testing)
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "ZERO_NOTIONAL",
        Money::new(0.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.notional.amount(), 0.0);
}

#[test]
fn test_spread_can_be_negative() {
    // Negative spreads are theoretically possible (though rare)
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "NEG_SPREAD",
        Money::new(10_000_000.0, Currency::USD),
        -50.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.premium.spread_bp, Decimal::from(-50));
}

#[test]
fn test_different_currencies() {
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    for currency in [Currency::USD, Currency::EUR, Currency::GBP, Currency::JPY] {
        let cds = test_utils::cds_buy_protection(
            format!("CDS_{}", currency),
            Money::new(10_000_000.0, currency),
            100.0,
            start,
            end,
            "DISC",
            "CREDIT",
        )
        .expect("CDS construction should succeed");

        assert_eq!(cds.notional.currency(), currency);
    }
}

#[test]
fn test_maturity_after_start() {
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "MATURITY_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert!(cds.premium.end > cds.premium.start);
}

#[test]
fn test_short_tenor_cds() {
    // 3-month CDS
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2025, Month::April, 1);

    let cds = test_utils::cds_buy_protection(
        "SHORT_TENOR",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.premium.start, start);
    assert_eq!(cds.premium.end, end);
}

#[test]
fn test_long_tenor_cds() {
    // 30-year CDS
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2055, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "LONG_TENOR",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.premium.end, end);
}

#[test]
fn test_premium_leg_spec_fields() {
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "PREMIUM_SPEC",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.premium.start, start);
    assert_eq!(cds.premium.end, end);
    assert_eq!(cds.premium.frequency, Tenor::quarterly());
    assert_eq!(cds.premium.day_count, DayCount::Act360);
    assert_eq!(cds.premium.discount_curve_id.as_str(), "USD-OIS");
}

#[test]
fn test_protection_leg_spec_fields() {
    let start = test_date(2025, Month::January, 1);
    let end = test_date(2030, Month::January, 1);

    let cds = test_utils::cds_buy_protection(
        "PROTECTION_SPEC",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD-OIS",
        "CORP-CREDIT",
    )
    .expect("CDS construction should succeed");

    assert_eq!(cds.protection.credit_curve_id.as_str(), "CORP-CREDIT");
    assert_eq!(cds.protection.recovery_rate, 0.40);
    assert_eq!(cds.protection.settlement_delay, 3);
}
