//! Tests for market conventions.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_valuations::instruments::{BondConvention, IRSConvention};
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
    assert_eq!(conv.default_disc_curve(), "USD-TREASURY");
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

#[test]
fn test_irs_convention_usd() {
    // Arrange
    let conv = IRSConvention::UsdSofr;

    // Assert - USD SOFR OIS is the post-LIBOR standard
    assert_eq!(conv.fixed_day_count().expect("registry"), DayCount::Act360);
    assert_eq!(conv.float_day_count().expect("registry"), DayCount::Act360);
    assert_eq!(conv.fixed_frequency().expect("registry"), Tenor::annual());
    assert_eq!(conv.disc_curve_id(), "USD-SOFR");
}

#[test]
fn test_irs_convention_eur() {
    // Arrange - EUR Standard is ESTR OIS (annual float with daily compounding)
    let conv = IRSConvention::EurEstr;

    // Assert - ESTR OIS uses annual payment frequency (not semi-annual like EURIBOR)
    assert_eq!(conv.fixed_frequency().expect("registry"), Tenor::annual());
    assert_eq!(conv.float_frequency().expect("registry"), Tenor::annual());
}

#[test]
fn test_irs_convention_eur_euribor() {
    // Arrange - EUR EURIBOR is the legacy IBOR convention with semi-annual float
    let conv = IRSConvention::EurEuribor;

    // Assert - EURIBOR 6M uses semi-annual payment frequency
    assert_eq!(conv.fixed_frequency().expect("registry"), Tenor::annual());
    assert_eq!(
        conv.float_frequency().expect("registry"),
        Tenor::semi_annual()
    );
}
