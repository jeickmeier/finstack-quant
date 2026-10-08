//! Construction and parameter validation tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Creation via builder pattern
//! - TIPS and UK Gilt conventions
//! - Various indexation methods
//! - Deflation protection settings

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod, InflationLinkedBond,
};
use rust_decimal::Decimal;

/// `ILB-TEST` with US TIPS conventions: 3-month lag, principal floor at
/// maturity, base date on the issue date, USD-OIS / US-CPI-U curves.
fn tips(
    notional: Money,
    real_coupon: f64,
    issue: Date,
    maturity: Date,
    base_cpi: f64,
    frequency: Tenor,
    day_count: DayCount,
) -> InflationLinkedBond {
    InflationLinkedBond::builder()
        .id(InstrumentId::new("ILB-TEST"))
        .notional(notional)
        .real_coupon(Decimal::try_from(real_coupon).expect("valid decimal"))
        .frequency(frequency)
        .day_count(day_count)
        .issue_date(issue)
        .maturity(maturity)
        .base_cpi(base_cpi)
        .base_date(issue)
        .indexation_method(IndexationMethod::Tips)
        .lag(InflationLag::Months(3))
        .deflation_protection(DeflationProtection::MaturityOnly)
        .business_day_convention(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .build()
        .expect("valid TIPS")
}

#[test]
fn test_tips_creation() {
    // Arrange
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let issue = d(2020, 1, 15);
    let maturity = d(2030, 1, 15);

    // Act
    let mut tips = tips(
        notional,
        0.0125, // 1.25% real coupon
        issue,
        maturity,
        250.0, // Base CPI
        Tenor::semi_annual(),
        DayCount::ActActIsma,
    );
    tips.id = InstrumentId::new("US_TIPS_2030");

    // Assert
    assert_eq!(tips.id.as_str(), "US_TIPS_2030");
    assert_eq!(tips.indexation_method, IndexationMethod::Tips);
    assert_eq!(
        tips.real_coupon,
        Decimal::try_from(0.0125).expect("valid decimal")
    );
    assert_eq!(tips.base_cpi, 250.0);
    assert_eq!(tips.notional.amount(), 1_000_000.0);
    assert_eq!(tips.notional.currency(), Currency::USD);
    assert_eq!(tips.frequency, Tenor::semi_annual());
    assert_eq!(tips.day_count, DayCount::ActActIsma);
    assert_eq!(tips.deflation_protection, DeflationProtection::MaturityOnly);
}

#[test]
fn test_uk_linker_creation() {
    // Arrange
    let notional = Money::new(1_000_000.0, Currency::GBP).expect("valid money fixture");
    let issue = d(2020, 3, 22);
    let maturity = d(2040, 3, 22);
    let base_date = d(2019, 7, 1);

    // Act: legacy (pre-September 2005) UK linker conventions — 8-month lag,
    // no deflation floor.
    let uk_gilt = InflationLinkedBond::builder()
        .id(InstrumentId::new("UK_GILT_2040"))
        .notional(notional)
        .real_coupon(Decimal::try_from(0.00625).expect("valid decimal")) // 0.625% real coupon
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::ActActIsma)
        .issue_date(issue)
        .maturity(maturity)
        .base_cpi(280.0) // Base RPI
        .base_date(base_date)
        .indexation_method(IndexationMethod::Uk)
        .lag(IndexationMethod::Uk.standard_lag())
        .deflation_protection(DeflationProtection::None)
        .business_day_convention(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("GBP-NOMINAL"))
        .inflation_index_id(CurveId::new("UK-RPI"))
        .build()
        .expect("valid UK linker");

    // Assert
    assert_eq!(uk_gilt.id.as_str(), "UK_GILT_2040");
    assert_eq!(uk_gilt.indexation_method, IndexationMethod::Uk);
    assert_eq!(
        uk_gilt.real_coupon,
        Decimal::try_from(0.00625).expect("valid decimal")
    );
    assert_eq!(uk_gilt.base_cpi, 280.0);
    assert_eq!(uk_gilt.base_date, base_date);
    assert_eq!(uk_gilt.notional.currency(), Currency::GBP);
    assert_eq!(uk_gilt.deflation_protection, DeflationProtection::None);
}

#[test]
fn test_builder_pattern_full_customization() {
    // Arrange & Act
    let bond = sample_tips();

    // Assert - verify all fields are set correctly
    assert_eq!(bond.id.as_str(), "TIPS-TEST");
    assert_eq!(bond.notional.amount(), 1_000_000.0);
    assert_eq!(
        bond.real_coupon,
        Decimal::try_from(0.0125).expect("valid decimal")
    );
    assert_eq!(bond.issue_date, d(2020, 1, 15));
    assert_eq!(bond.maturity, d(2030, 1, 15));
    assert_eq!(bond.base_cpi, 250.0);
    assert_eq!(bond.indexation_method, IndexationMethod::Tips);
}

#[test]
fn test_indexation_method_display() {
    // Arrange & Act & Assert
    assert_eq!(IndexationMethod::Tips.to_string(), "tips");
    assert_eq!(IndexationMethod::Uk.to_string(), "uk");
    assert_eq!(IndexationMethod::Canadian.to_string(), "canadian");
    assert_eq!(IndexationMethod::French.to_string(), "french");
    assert_eq!(IndexationMethod::Japanese.to_string(), "japanese");
}

#[test]
fn test_indexation_method_from_str() {
    // Arrange & Act & Assert
    use std::str::FromStr;

    assert_eq!(
        IndexationMethod::from_str("tips").unwrap(),
        IndexationMethod::Tips
    );
    assert_eq!(
        IndexationMethod::from_str("uk").unwrap(),
        IndexationMethod::Uk
    );
    assert_eq!(
        IndexationMethod::from_str("canadian").unwrap(),
        IndexationMethod::Canadian
    );
    assert_eq!(
        IndexationMethod::from_str("french").unwrap(),
        IndexationMethod::French
    );
    assert_eq!(
        IndexationMethod::from_str("japanese").unwrap(),
        IndexationMethod::Japanese
    );
    for retired in ["us", "UK", "jgb", "invalid"] {
        assert!(IndexationMethod::from_str(retired).is_err());
    }
}

#[test]
fn test_indexation_method_standard_lags() {
    // Arrange & Act
    use finstack_quant_core::market_data::scalars::InflationLag;

    // Assert
    assert_eq!(
        IndexationMethod::Tips.standard_lag(),
        InflationLag::Months(3)
    );
    assert_eq!(
        IndexationMethod::Canadian.standard_lag(),
        InflationLag::Months(3)
    );
    assert_eq!(IndexationMethod::Uk.standard_lag(), InflationLag::Months(8));
    assert_eq!(
        IndexationMethod::French.standard_lag(),
        InflationLag::Months(3)
    );
    assert_eq!(
        IndexationMethod::Japanese.standard_lag(),
        InflationLag::Months(3)
    );
}

#[test]
fn test_deflation_protection_display() {
    // Arrange & Act & Assert
    assert_eq!(DeflationProtection::None.to_string(), "none");
    assert_eq!(
        DeflationProtection::MaturityOnly.to_string(),
        "maturity_only"
    );
    assert_eq!(DeflationProtection::AllPayments.to_string(), "all_payments");
}

#[test]
fn test_deflation_protection_from_str() {
    // Arrange & Act & Assert
    use std::str::FromStr;

    assert_eq!(
        DeflationProtection::from_str("none").unwrap(),
        DeflationProtection::None
    );
    assert_eq!(
        DeflationProtection::from_str("maturity_only").unwrap(),
        DeflationProtection::MaturityOnly
    );
    assert_eq!(
        DeflationProtection::from_str("all_payments").unwrap(),
        DeflationProtection::AllPayments
    );
    for retired in ["maturity", "all", "MATURITY-ONLY", "invalid"] {
        assert!(DeflationProtection::from_str(retired).is_err());
    }
}

#[test]
fn test_various_currencies() {
    // Arrange
    let issue = d(2020, 1, 1);
    let maturity = d(2030, 1, 1);

    // Act & Assert - Test various currencies
    for (ccy, base_cpi) in [
        (Currency::USD, 250.0),
        (Currency::GBP, 280.0),
        (Currency::EUR, 100.0),
        (Currency::CAD, 140.0),
        (Currency::JPY, 100.0),
    ] {
        let notional = Money::new(1_000_000.0, ccy).expect("valid money fixture");
        let mut bond = tips(
            notional,
            0.01,
            issue,
            maturity,
            base_cpi,
            Tenor::semi_annual(),
            DayCount::ActAct,
        );
        bond.id = InstrumentId::new(format!("ILB-{}", ccy));
        bond.discount_curve_id = CurveId::new(format!("{}-REAL", ccy));
        bond.inflation_index_id = CurveId::new(format!("{}-CPI", ccy));

        assert_eq!(bond.notional.currency(), ccy);
    }
}

#[test]
fn test_various_frequencies() {
    // Arrange
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let issue = d(2020, 1, 1);
    let maturity = d(2030, 1, 1);

    // Act & Assert - Test various payment frequencies
    for frequency in [Tenor::annual(), Tenor::semi_annual(), Tenor::quarterly()] {
        let bond = tips(
            notional,
            0.01,
            issue,
            maturity,
            250.0,
            frequency,
            DayCount::ActAct,
        );

        assert_eq!(bond.frequency, frequency);
    }
}

#[test]
fn test_various_day_count_conventions() {
    // Arrange
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let issue = d(2020, 1, 1);
    let maturity = d(2030, 1, 1);

    // Act & Assert - Test various day count conventions
    for day_count in [DayCount::ActAct, DayCount::Act360, DayCount::Thirty360] {
        let bond = tips(
            notional,
            0.01,
            issue,
            maturity,
            250.0,
            Tenor::semi_annual(),
            day_count,
        );

        assert_eq!(bond.day_count, day_count);
    }
}

#[test]
fn test_quoted_clean_price_pct() {
    // Arrange & Act
    let mut bond = sample_tips();

    // Assert - quoted price can be set and cleared
    // Note: sample_tips() may or may not have a default quoted_clean_price_pct

    // Act - update quoted price
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(105.5);

    // Assert
    assert_eq!(
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct,
        Some(105.5)
    );

    // Act - clear quoted price
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = None;

    // Assert
    assert_eq!(
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct,
        None
    );
}
