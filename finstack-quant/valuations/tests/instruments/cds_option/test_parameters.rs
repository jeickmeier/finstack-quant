//! Unit tests for CDSOptionParams builder and validation.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOptionParams;
use finstack_quant_valuations::instruments::OptionType;
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn test_call_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD);

    let params = CDSOptionParams::call(Decimal::new(1, 2), expiry, maturity, notional)
        .expect("valid call params");

    assert_eq!(params.strike, Decimal::new(1, 2)); // 0.01 = 100bp
    assert_eq!(params.expiry, expiry);
    assert_eq!(params.cds_maturity, maturity);
    assert_eq!(params.notional.amount(), 10_000_000.0);
    assert_eq!(params.notional.currency(), Currency::USD);
    assert!(matches!(params.option_type, OptionType::Call));
    assert!(!params.underlying_is_index);
    assert_eq!(params.index_factor, None);
    assert_eq!(params.underlying_cds_coupon, None);
}

#[test]
fn test_put_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(5_000_000.0, Currency::EUR);

    let params = CDSOptionParams::put(Decimal::new(15, 3), expiry, maturity, notional)
        .expect("valid put params"); // 0.015 = 150bp

    assert_eq!(params.strike, Decimal::new(15, 3));
    assert!(matches!(params.option_type, OptionType::Put));
    assert_eq!(params.notional.currency(), Currency::EUR);
}

#[test]
fn test_index_option_builder() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD);

    let params = CDSOptionParams::call(Decimal::new(1, 2), expiry, maturity, notional)
        .expect("valid call params")
        .as_index(0.85)
        .expect("valid index factor");

    assert!(params.underlying_is_index);
    assert_eq!(params.index_factor, Some(0.85));
}

#[test]
fn test_underlying_cds_coupon_for_index() {
    // CDX/iTraxx index options have a contractual coupon distinct from
    // the option strike (e.g. 100 bp standard CDX). The Bloomberg CDSO
    // strike-adjustment term H(K) = ξN(c − K)A(K) is non-zero only when
    // c ≠ K, so this is the field index fixtures must populate.
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD);

    let params = CDSOptionParams::call(Decimal::new(55, 4), expiry, maturity, notional) // 0.0055 = 55bp
        .expect("valid call params")
        .as_index(1.0)
        .expect("valid index factor")
        .with_underlying_cds_coupon(Decimal::new(1, 2)); // 0.01 = 100 bp standard CDX

    assert_eq!(params.underlying_cds_coupon, Some(Decimal::new(1, 2)));
    assert!(params.underlying_is_index);
}

#[test]
fn test_chained_builders() {
    let expiry = date!(2025 - 06 - 30);
    let maturity = date!(2028 - 06 - 30);
    let notional = Money::new(20_000_000.0, Currency::GBP);

    let params = CDSOptionParams::put(Decimal::new(2, 2), expiry, maturity, notional) // 0.02 = 200bp
        .expect("valid put params")
        .as_index(0.75)
        .expect("valid index factor")
        .with_underlying_cds_coupon(Decimal::new(1, 2));

    assert!(matches!(params.option_type, OptionType::Put));
    assert_eq!(params.strike, Decimal::new(2, 2));
    assert!(params.underlying_is_index);
    assert_eq!(params.index_factor, Some(0.75));
    assert_eq!(params.underlying_cds_coupon, Some(Decimal::new(1, 2)));
}

#[test]
fn test_various_strikes() {
    let expiry = date!(2026 - 01 - 01);
    let maturity = date!(2031 - 01 - 01);
    let notional = Money::new(10_000_000.0, Currency::USD);

    // Strikes in bp -> decimal: 25bp=0.0025, 50bp=0.005, 100bp=0.01, etc.
    let strikes_decimal = [
        Decimal::new(25, 4), // 0.0025 = 25bp
        Decimal::new(5, 3),  // 0.005 = 50bp
        Decimal::new(1, 2),  // 0.01 = 100bp
        Decimal::new(2, 2),  // 0.02 = 200bp
        Decimal::new(5, 2),  // 0.05 = 500bp
        Decimal::new(1, 1),  // 0.1 = 1000bp
    ];

    for strike in strikes_decimal {
        let params =
            CDSOptionParams::call(strike, expiry, maturity, notional).expect("valid call params");
        assert_eq!(params.strike, strike);
    }
}

#[test]
fn test_various_currencies() {
    let expiry = date!(2026 - 01 - 01);
    let maturity = date!(2031 - 01 - 01);

    for currency in [Currency::USD, Currency::EUR, Currency::GBP, Currency::JPY] {
        let notional = Money::new(10_000_000.0, currency);
        let params = CDSOptionParams::call(Decimal::new(1, 2), expiry, maturity, notional)
            .expect("valid call params");
        assert_eq!(params.notional.currency(), currency);
    }
}
