//! Tests for equity option constructors and builders.

use super::helpers::*;
use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, SettlementType};
use time::macros::date;

#[test]
fn test_builder_creates_valid_option() {
    let expiry = date!(2025 - 12 - 31);

    let option = EquityOption::builder()
        .id(InstrumentId::new("TEST_CALL"))
        .underlying_ticker("AAPL".to_string())
        .strike(150.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(100.0, Currency::USD))
        .day_count(DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new(DISC_ID))
        .spot_id(SPOT_ID.into())
        .vol_surface_id(CurveId::new(VOL_ID))
        .div_yield_id_opt(Some(CurveId::new(DIV_ID)))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(option.id.as_str(), "TEST_CALL");
    assert_eq!(option.strike, 150.0);
    assert_eq!(option.option_type, OptionType::Call);
    assert_eq!(option.notional.amount(), 100.0);
}

#[test]
fn test_european_call_convenience_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let call =
        test_utils::equity_option_european_call("SPX-CALL", "SPX", 4500.0, expiry, 100.0).unwrap();

    assert_eq!(call.option_type, OptionType::Call);
    assert_eq!(call.exercise_style, ExerciseStyle::European);
    assert_eq!(call.strike, 4500.0);
    assert_eq!(call.notional.amount(), 100.0);
    assert_eq!(call.settlement, SettlementType::Cash);
}

#[test]
fn test_european_put_convenience_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let put =
        test_utils::equity_option_european_put("SPX-PUT", "SPX", 4200.0, expiry, 100.0).unwrap();

    assert_eq!(put.option_type, OptionType::Put);
    assert_eq!(put.exercise_style, ExerciseStyle::European);
    assert_eq!(put.strike, 4200.0);
    assert_eq!(put.notional.amount(), 100.0);
}

#[test]
fn test_american_call_convenience_constructor() {
    let expiry = date!(2025 - 12 - 31);
    let call =
        test_utils::equity_option_american_call("SPX-AMER", "SPX", 4500.0, expiry, 100.0).unwrap();

    assert_eq!(call.exercise_style, ExerciseStyle::American);
    assert_eq!(call.option_type, OptionType::Call);
}

#[test]
fn test_contract_size_variations() {
    let expiry = date!(2025 - 12 - 31);

    // Standard contract
    let standard =
        test_utils::equity_option_european_call("STD", "SPX", 100.0, expiry, 100.0).unwrap();
    assert_eq!(standard.notional.amount(), 100.0);

    // Mini contract
    let mini = test_utils::equity_option_european_call("MINI", "SPX", 100.0, expiry, 10.0).unwrap();
    assert_eq!(mini.notional.amount(), 10.0);

    // Custom size
    let custom =
        test_utils::equity_option_european_call("CUSTOM", "SPX", 100.0, expiry, 50.0).unwrap();
    assert_eq!(custom.notional.amount(), 50.0);
}

#[test]
fn test_settlement_type_variations() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let mut cash_settled = create_call(as_of, expiry, 100.0);
    cash_settled.settlement = SettlementType::Cash;
    assert_eq!(cash_settled.settlement, SettlementType::Cash);

    let mut physical_settled = create_call(as_of, expiry, 100.0);
    physical_settled.settlement = SettlementType::Physical;
    assert_eq!(physical_settled.settlement, SettlementType::Physical);
}

#[test]
fn test_exercise_style_variations() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let mut european = create_call(as_of, expiry, 100.0);
    european.exercise_style = ExerciseStyle::European;
    assert_eq!(european.exercise_style, ExerciseStyle::European);

    let mut american = create_call(as_of, expiry, 100.0);
    american.exercise_style = ExerciseStyle::American;
    assert_eq!(american.exercise_style, ExerciseStyle::American);
}

#[test]
fn test_convenience_constructors_keep_scalar_strike() {
    let expiry = date!(2025 - 12 - 31);
    let option =
        test_utils::equity_option_european_call("EUR-OPT", "SX5E", 4000.0, expiry, 100.0).unwrap();

    assert_eq!(option.strike, 4000.0);
}
