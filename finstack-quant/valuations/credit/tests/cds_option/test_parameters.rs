//! Unit tests for `CdsOption::builder()` construction and validation.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};
use finstack_quant_valuations_credit::cds_option::{CdsOption, CdsOptionStrike};
use rust_decimal::Decimal;
use time::macros::date;

fn base_builder(
    strike: CdsOptionStrike,
    expiry: finstack_quant_core::dates::Date,
    maturity: finstack_quant_core::dates::Date,
    notional: Money,
    option_type: OptionType,
) -> finstack_quant_valuations_credit::cds_option::CdsOptionBuilder {
    CdsOption::builder()
        .id(InstrumentId::new("TEST-CDSOPT"))
        .strike(strike)
        .option_type(option_type)
        .expiry(expiry)
        .underlying_maturity(maturity)
        .notional(notional)
        .settlement(SettlementType::Cash)
        .recovery_rate(0.40)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id(CurveId::new("TEST-HAZARD"))
        .vol_surface_id(CurveId::new("CDSO-VOL"))
}

#[test]
fn test_call_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");

    let option = base_builder(
        CdsOptionStrike::Spread(Decimal::new(1, 2)),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .build()
    .expect("valid call option");
    option.validate_for_pricing().expect("valid invariants");

    assert_eq!(option.strike.spread_decimal(), Some(Decimal::new(1, 2)));
    assert_eq!(option.expiry, expiry);
    assert_eq!(option.underlying_maturity, maturity);
    assert_eq!(option.notional.amount(), 10_000_000.0);
    assert_eq!(option.notional.currency(), Currency::USD);
    assert!(matches!(option.option_type, OptionType::Call));
    assert!(!option.underlying_is_index);
    assert_eq!(option.index_factor, 1.0);
    assert_eq!(option.coupon_bp, None);
}

#[test]
fn test_put_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(5_000_000.0, Currency::EUR).expect("valid money fixture");

    let option = base_builder(
        CdsOptionStrike::Spread(Decimal::new(15, 3)),
        expiry,
        maturity,
        notional,
        OptionType::Put,
    )
    .build()
    .expect("valid put option");
    option.validate_for_pricing().expect("valid invariants");

    assert_eq!(option.strike.spread_decimal(), Some(Decimal::new(15, 3)));
    assert!(matches!(option.option_type, OptionType::Put));
    assert_eq!(option.notional.currency(), Currency::EUR);
}

#[test]
fn test_index_option_builder() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");

    let option = base_builder(
        CdsOptionStrike::Spread(Decimal::new(1, 2)),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .underlying_is_index(true)
    .index_factor(0.85)
    .coupon_bp(Decimal::new(100, 0))
    .build()
    .expect("valid index option");
    option.validate_for_pricing().expect("valid invariants");

    assert!(option.underlying_is_index);
    assert_eq!(option.index_factor, 0.85);
}

#[test]
fn test_coupon_bp_for_index() {
    let expiry = date!(2025 - 12 - 31);
    let maturity = date!(2030 - 12 - 31);
    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");

    let option = base_builder(
        CdsOptionStrike::Spread(Decimal::new(55, 4)),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .underlying_is_index(true)
    .index_factor(1.0)
    .coupon_bp(Decimal::new(100, 0))
    .build()
    .expect("valid index option");
    option.validate_for_pricing().expect("valid invariants");

    assert_eq!(option.coupon_bp, Some(Decimal::new(100, 0)));
    assert!(option.underlying_is_index);
}

#[test]
fn test_chained_builders() {
    let expiry = date!(2025 - 06 - 30);
    let maturity = date!(2028 - 06 - 30);
    let notional = Money::new(20_000_000.0, Currency::GBP).expect("valid money fixture");

    let option = base_builder(
        CdsOptionStrike::Spread(Decimal::new(2, 2)),
        expiry,
        maturity,
        notional,
        OptionType::Put,
    )
    .underlying_is_index(true)
    .index_factor(0.75)
    .coupon_bp(Decimal::new(100, 0))
    .build()
    .expect("valid index option");
    option.validate_for_pricing().expect("valid invariants");

    assert!(matches!(option.option_type, OptionType::Put));
    assert_eq!(option.strike.spread_decimal(), Some(Decimal::new(2, 2)));
    assert!(option.underlying_is_index);
    assert_eq!(option.index_factor, 0.75);
    assert_eq!(option.coupon_bp, Some(Decimal::new(100, 0)));
}

#[test]
fn test_various_strikes() {
    let expiry = date!(2026 - 01 - 01);
    let maturity = date!(2031 - 01 - 01);
    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");

    let strikes_decimal = [
        Decimal::new(25, 4),
        Decimal::new(5, 3),
        Decimal::new(1, 2),
        Decimal::new(2, 2),
        Decimal::new(5, 2),
        Decimal::new(1, 1),
    ];

    for strike in strikes_decimal {
        let option = base_builder(
            CdsOptionStrike::Spread(strike),
            expiry,
            maturity,
            notional,
            OptionType::Call,
        )
        .build()
        .expect("valid strike");
        option.validate_for_pricing().expect("valid invariants");
        assert_eq!(option.strike.spread_decimal(), Some(strike));
    }
}

#[test]
fn test_various_currencies() {
    let expiry = date!(2026 - 01 - 01);
    let maturity = date!(2031 - 01 - 01);

    for currency in [Currency::USD, Currency::EUR, Currency::GBP, Currency::JPY] {
        let notional = Money::new(10_000_000.0, currency).expect("valid money fixture");
        let option = base_builder(
            CdsOptionStrike::Spread(Decimal::new(1, 2)),
            expiry,
            maturity,
            notional,
            OptionType::Call,
        )
        .build()
        .expect("valid option");
        option.validate_for_pricing().expect("valid invariants");
        assert_eq!(option.notional.currency(), currency);
    }
}

#[test]
fn test_validation_rejects_invalid_terms() {
    let expiry = date!(2025 - 06 - 20);
    let maturity = date!(2030 - 06 - 20);
    let notional = Money::from((10_000_000_i64, Currency::USD));

    let zero_err = base_builder(
        CdsOptionStrike::Spread(Decimal::ZERO),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .build()
    .unwrap_err();
    assert!(zero_err.to_string().contains("strike must be positive"));

    assert!(
        base_builder(
            CdsOptionStrike::Spread(Decimal::new(-5, 3)),
            expiry,
            maturity,
            notional,
            OptionType::Call,
        )
        .build()
        .is_err()
    );

    let expiry_err = base_builder(
        CdsOptionStrike::Spread(Decimal::new(1, 2)),
        date!(2030 - 06 - 21),
        maturity,
        notional,
        OptionType::Call,
    )
    .build()
    .unwrap_err();
    assert!(
        expiry_err
            .to_string()
            .contains("must be before CDS maturity")
    );

    assert!(
        base_builder(
            CdsOptionStrike::Spread(Decimal::new(1, 2)),
            expiry,
            maturity,
            notional,
            OptionType::Call,
        )
        .underlying_is_index(true)
        .index_factor(1.5)
        .coupon_bp(Decimal::new(100, 0))
        .build()
        .is_err()
    );

    let big_spread = base_builder(
        CdsOptionStrike::Spread(Decimal::new(1070, 1)),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .build()
    .unwrap_err();
    assert!(big_spread.to_string().contains("exceeds maximum"));

    let clean_opt = base_builder(
        CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1)),
        expiry,
        maturity,
        notional,
        OptionType::Call,
    )
    .underlying_is_index(true)
    .index_factor(1.0)
    .strike_index_factor(1.0)
    .coupon_bp(Decimal::new(500, 0))
    .build()
    .unwrap();
    clean_opt
        .validate_for_pricing()
        .expect("clean strike > 100 valid");
}
