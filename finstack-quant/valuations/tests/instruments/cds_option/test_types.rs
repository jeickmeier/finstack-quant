//! Unit tests for CDSOption type construction and basic methods.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOptionParams;
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::ExerciseStyle;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::instruments::SettlementType;
use finstack_quant_valuations::pricer::InstrumentType;
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn test_cds_option_construction() {
    let _as_of = date!(2025 - 01 - 01);
    let expiry = date!(2026 - 01 - 01);
    let maturity = date!(2031 - 01 - 01);

    let option_params = CDSOptionParams::call(
        Decimal::new(1, 2), // 0.01 = 100bp
        expiry,
        maturity,
        Money::new(10_000_000.0, Currency::USD),
    )
    .expect("valid call params");
    let credit_params = CreditParams::corporate_standard("CORP", "HZ-CORP");

    let option = CDSOption::new(
        "TEST-CDSOPT",
        &option_params,
        &credit_params,
        "USD-OIS",
        "CDS-VOL",
    )
    .expect("valid CDS option");

    assert_eq!(option.id(), "TEST-CDSOPT");
    assert_eq!(option.strike, Decimal::new(1, 2));
    assert!(matches!(option.option_type, OptionType::Call));
    assert_eq!(option.expiry, expiry);
    assert_eq!(option.cds_maturity, maturity);
    assert_eq!(option.notional.amount(), 10_000_000.0);
    assert_eq!(option.notional.currency(), Currency::USD);
    assert_eq!(option.discount_curve_id.as_str(), "USD-OIS");
    assert_eq!(option.credit_curve_id.as_str(), "HZ-CORP");
    assert_eq!(option.vol_surface_id.as_str(), "CDS-VOL");
}

#[test]
fn test_instrument_trait_implementation() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().build(as_of);

    assert_eq!(option.id(), "CDSOPT-TEST");
    assert_eq!(option.key(), InstrumentType::CDSOption);
}

#[test]
fn test_instrument_clone_box() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().build(as_of);

    let boxed = option.clone_box();
    assert_eq!(boxed.id(), option.id());
    assert_eq!(boxed.key(), option.key());
}

#[test]
fn test_single_name_option_defaults() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().build(as_of);

    assert!(!option.underlying_is_index);
    assert_eq!(option.index_factor, None);
    assert_eq!(option.realized_index_loss, None);
    assert_eq!(option.underlying_cds_coupon, None);
}

#[test]
fn test_index_option_construction() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new()
        .with_index(0.88)
        .underlying_cds_coupon_bp(100.0)
        .build(as_of);

    assert!(option.underlying_is_index);
    assert_eq!(option.index_factor, Some(0.88));
    // Standard CDX coupon = 100 bp = 0.01 decimal.
    let expected = Decimal::try_from(100.0 / 10000.0).unwrap();
    assert_eq!(option.underlying_cds_coupon, Some(expected));
}

#[test]
fn test_call_option() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().call().build(as_of);

    assert!(matches!(option.option_type, OptionType::Call));
}

#[test]
fn test_put_option() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().put().build(as_of);

    assert!(matches!(option.option_type, OptionType::Put));
}

#[test]
fn test_various_strikes() {
    let as_of = date!(2025 - 01 - 01);

    for strike_bp in [25.0, 50.0, 100.0, 200.0, 500.0] {
        let option = CDSOptionBuilder::new().strike(strike_bp).build(as_of);
        let expected = Decimal::try_from(strike_bp / 10000.0).unwrap();
        assert_eq!(option.strike, expected);
    }
}

#[test]
fn test_various_maturities() {
    let as_of = date!(2025 - 01 - 01);

    for expiry_months in [3, 6, 12, 24] {
        for cds_months in [36, 60, 84, 120] {
            let option = CDSOptionBuilder::new()
                .expiry_months(expiry_months)
                .cds_maturity_months(cds_months)
                .build(as_of);

            // Verify dates are correctly set
            assert!(option.expiry > as_of);
            assert!(option.cds_maturity > option.expiry);
        }
    }
}

#[test]
fn test_pricing_overrides() {
    let as_of = date!(2025 - 01 - 01);
    let mut option = CDSOptionBuilder::new().implied_vol(0.45).build(as_of);

    assert_eq!(
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility,
        Some(0.45)
    );

    // Test modification
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.25);
    assert_eq!(
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility,
        Some(0.25)
    );
}

#[test]
fn test_attributes_access() {
    let as_of = date!(2025 - 01 - 01);
    let mut option = CDSOptionBuilder::new().build(as_of);

    // Test mutable access
    option
        .attributes_mut()
        .meta
        .insert("test_key".to_string(), "test_value".to_string());

    assert_eq!(
        option.attributes().meta.get("test_key"),
        Some(&"test_value".to_string())
    );
}

#[test]
fn test_american_exercise_is_rejected_at_pricing_time() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let mut option = CDSOptionBuilder::new().build(as_of);
    option.exercise_style = ExerciseStyle::American;

    let err = option
        .value(&market, as_of)
        .expect_err("American CDS options should be rejected");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn test_physical_settlement_is_rejected_at_pricing_time() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let mut option = CDSOptionBuilder::new().build(as_of);
    option.settlement = SettlementType::Physical;

    let err = option
        .value(&market, as_of)
        .expect_err("Physical CDS option settlement should be rejected");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}
