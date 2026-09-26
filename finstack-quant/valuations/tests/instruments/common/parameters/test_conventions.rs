//! Tests for market conventions.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_valuations::instruments::BondConvention;
use std::str::FromStr;

#[test]
fn test_bond_convention_us_treasury() {
    // Arrange
    let conv = BondConvention::UsTreasury;

    // Assert
    assert_eq!(conv.day_count(), DayCount::ActActIsma);
    assert_eq!(conv.frequency(), Tenor::semi_annual());
    assert_eq!(
        conv.business_day_convention(),
        BusinessDayConvention::Following
    );
    assert_eq!(conv.discount_curve_id(), "USD-TREASURY");
}

#[test]
fn test_bond_convention_german_bund() {
    // Arrange
    let conv = BondConvention::GermanBund;

    // Assert
    assert_eq!(conv.day_count(), DayCount::ActActIsma);
    assert_eq!(conv.frequency(), Tenor::annual());
}

#[test]
fn test_bond_convention_from_str() {
    // Arrange & Act & Assert
    assert_eq!(
        BondConvention::from_str("us_treasury").unwrap(),
        BondConvention::UsTreasury
    );
    assert_eq!(
        BondConvention::from_str("german_bund").unwrap(),
        BondConvention::GermanBund
    );
    assert_eq!(
        BondConvention::from_str("us_corporate").unwrap(),
        BondConvention::UsCorporate
    );
    assert!(BondConvention::from_str("UST").is_err());
}
