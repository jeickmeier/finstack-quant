// Common test fixtures and helpers for convertible bond testing.
//
// Provides standardized market contexts, bond configurations, and utility
// functions to support comprehensive unit testing across all convertible
// bond features.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use time::Month;

use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
use crate::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};

/// Standard test dates
pub mod dates {
    use super::*;

    pub fn issue() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    pub fn maturity_5y() -> Date {
        Date::from_calendar_date(2030, Month::January, 1).unwrap()
    }

    pub fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }
}

/// Standard market parameters
pub mod market_params {
    /// Standard spot price for equity
    pub const SPOT_PRICE: f64 = 150.0;

    /// Low spot price (out of the money)
    pub const SPOT_LOW: f64 = 50.0;

    /// Standard volatility
    pub const VOL_STANDARD: f64 = 0.25;

    /// Low volatility
    pub const VOL_LOW: f64 = 0.05;

    /// High volatility
    pub const VOL_HIGH: f64 = 0.50;

    /// Standard dividend yield
    pub const DIV_YIELD: f64 = 0.02;
}

/// Standard bond parameters
pub mod bond_params {
    /// Standard notional
    pub const NOTIONAL: f64 = 1000.0;

    /// Standard conversion ratio (shares per bond)
    pub const CONVERSION_RATIO: f64 = 10.0;

    /// Standard coupon rate
    pub const COUPON_RATE: f64 = 0.05;
}

/// Create standard market context with configurable parameters
pub fn create_market_context() -> MarketContext {
    create_market_context_with_params(
        market_params::SPOT_PRICE,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    )
}

/// Create market context with custom spot, volatility, and dividend yield
pub fn create_market_context_with_params(spot: f64, vol: f64, div_yield: f64) -> MarketContext {
    let base_date = dates::base_date();

    // Create flat discount curve at ~3% (df = e^(-0.03*t))
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.741)]) // e^(-0.03*10) = 0.741
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(spot))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(vol))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(div_yield))
}

/// Create standard convertible bond with voluntary conversion
pub fn create_standard_convertible() -> ConvertibleBond {
    create_convertible_with_policy(ConversionPolicy::Voluntary)
}

/// Create convertible bond with specific conversion policy
pub fn create_convertible_with_policy(policy: ConversionPolicy) -> ConvertibleBond {
    let issue = dates::issue();
    let maturity = dates::maturity_5y();

    let conversion_spec = ConversionSpec {
        ratio: Some(bond_params::CONVERSION_RATIO),
        price: None,
        policy,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::from_f64_retain(bond_params::COUPON_RATE).unwrap_or_default(),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    ConvertibleBond {
        id: "TEST_CONVERTIBLE".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        attributes: Default::default(),
    }
}

/// Relaxed relative tolerance for convergence tests (5%)
pub const CONVERGENCE_TOLERANCE_PCT: f64 = 0.05;
