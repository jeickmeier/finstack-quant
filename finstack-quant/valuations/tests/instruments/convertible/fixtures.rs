//! Common test fixtures and helpers for convertible bond testing.
//!
//! Provides standardized market contexts, bond configurations, and utility
//! functions to support comprehensive unit testing across all convertible
//! bond features.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use time::Month;

use finstack_quant_cashflows::builder::specs::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateSpec,
};
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};

/// Standard test dates
pub mod dates {
    use super::*;

    pub fn issue() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    pub fn maturity_1y() -> Date {
        Date::from_calendar_date(2026, Month::January, 1).unwrap()
    }

    pub fn maturity_5y() -> Date {
        Date::from_calendar_date(2030, Month::January, 1).unwrap()
    }

    pub fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    pub fn mid_date() -> Date {
        Date::from_calendar_date(2027, Month::July, 1).unwrap()
    }
}

/// Standard market parameters
pub mod market_params {
    /// Standard spot price for equity
    pub const SPOT_PRICE: f64 = 150.0;

    /// Low spot price (out of the money)
    pub const SPOT_LOW: f64 = 50.0;

    /// High spot price (deep in the money)
    pub const SPOT_HIGH: f64 = 250.0;

    /// Standard volatility
    pub const VOL_STANDARD: f64 = 0.25;

    /// Low volatility
    pub const VOL_LOW: f64 = 0.05;

    /// High volatility
    pub const VOL_HIGH: f64 = 0.50;

    /// Standard dividend yield
    pub const DIV_YIELD: f64 = 0.02;

    /// Risk-free rate (implied by discount curve)
    pub const RISK_FREE_RATE: f64 = 0.03;
}

/// Standard bond parameters
pub mod bond_params {
    /// Standard notional
    pub const NOTIONAL: f64 = 1000.0;

    /// Standard conversion ratio (shares per bond)
    pub const CONVERSION_RATIO: f64 = 10.0;

    /// Standard conversion price ($/share)
    pub const CONVERSION_PRICE: f64 = 100.0;

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

/// Create market context with specific risk-free rate
pub fn create_market_context_with_rate(rate: f64) -> MarketContext {
    let base_date = dates::base_date();

    let df_10y = (-rate * 10.0).exp();
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, df_10y)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(market_params::SPOT_PRICE))
        .insert_price(
            "AAPL-VOL",
            MarketScalar::Unitless(market_params::VOL_STANDARD),
        )
        .insert_price(
            "AAPL-DIVYIELD",
            MarketScalar::Unitless(market_params::DIV_YIELD),
        )
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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    ConvertibleBond {
        id: "TEST_CONVERTIBLE".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Create convertible bond with conversion price instead of ratio
pub fn create_convertible_with_conversion_price() -> ConvertibleBond {
    let issue = dates::issue();
    let maturity = dates::maturity_5y();

    let conversion_spec = ConversionSpec {
        ratio: None,
        price: Some(bond_params::CONVERSION_PRICE),
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::from_f64_retain(bond_params::COUPON_RATE).unwrap_or_default(),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    ConvertibleBond {
        id: "TEST_CONVERTIBLE_PRICE".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Create convertible bond with floating coupon
pub fn create_floating_convertible() -> ConvertibleBond {
    let issue = dates::issue();
    let maturity = dates::maturity_1y();

    let conversion_spec = ConversionSpec {
        ratio: Some(bond_params::CONVERSION_RATIO),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let floating = FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: finstack_quant_cashflows::builder::FloatingRateFallback::SpreadOnly,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    };

    ConvertibleBond {
        id: "TEST_FLOATING_CONVERTIBLE".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: None,
        floating_coupon: Some(floating),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Create convertible bond with call schedule
pub fn create_callable_convertible(call_date: Date, call_price_pct: f64) -> ConvertibleBond {
    let mut bond = create_standard_convertible();

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: call_price_pct,
        make_whole: None,
    });

    bond.call_put = Some(call_put);
    bond
}

/// Create convertible bond with put schedule
pub fn create_puttable_convertible(put_date: Date, put_price_pct: f64) -> ConvertibleBond {
    let mut bond = create_standard_convertible();

    let mut call_put = CallPutSchedule::default();
    call_put.puts.push(CallPut {
        start_date: put_date,
        end_date: put_date,
        price_pct_of_par: put_price_pct,
        make_whole: None,
    });

    bond.call_put = Some(call_put);
    bond
}

/// Create convertible bond with both call and put schedules
pub fn create_callable_puttable_convertible(
    call_date: Date,
    call_price_pct: f64,
    put_date: Date,
    put_price_pct: f64,
) -> ConvertibleBond {
    let mut bond = create_standard_convertible();

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: call_price_pct,
        make_whole: None,
    });
    call_put.puts.push(CallPut {
        start_date: put_date,
        end_date: put_date,
        price_pct_of_par: put_price_pct,
        make_whole: None,
    });

    bond.call_put = Some(call_put);
    bond
}

/// Create zero-coupon convertible bond
pub fn create_zero_coupon_convertible() -> ConvertibleBond {
    let issue = dates::issue();
    let maturity = dates::maturity_5y();

    let conversion_spec = ConversionSpec {
        ratio: Some(bond_params::CONVERSION_RATIO),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    ConvertibleBond {
        id: "TEST_ZERO_COUPON".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: None,
        floating_coupon: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Calculate theoretical bond floor (PV of debt cashflows without conversion)
pub fn calculate_bond_floor(coupon_rate: f64, maturity_years: f64, risk_free_rate: f64) -> f64 {
    let periods = (maturity_years * 2.0) as usize; // semi-annual
    let coupon = coupon_rate / 2.0; // semi-annual coupon
    let discount_rate = risk_free_rate / 2.0; // semi-annual discount

    let mut pv = 0.0;
    for i in 1..=periods {
        pv += coupon / (1.0 + discount_rate).powi(i as i32);
    }
    pv += 1.0 / (1.0 + discount_rate).powi(periods as i32); // principal

    pv
}

/// Calculate theoretical conversion value
pub fn theoretical_conversion_value(spot: f64, conversion_ratio: f64) -> f64 {
    spot * conversion_ratio
}

/// Calculate theoretical parity
pub fn theoretical_parity(spot: f64, conversion_ratio: f64, notional: f64) -> f64 {
    theoretical_conversion_value(spot, conversion_ratio) / notional
}

/// Create market context with a separate credit curve (higher spread than risk-free).
///
/// The credit curve is constructed with a spread above the risk-free curve to
/// exercise the TZ credit/equity decomposition path, which is skipped when
/// `credit_curve_id` is `None`.
pub fn create_market_context_with_credit(credit_spread_bps: f64) -> MarketContext {
    let base_date = dates::base_date();
    let rf_rate = market_params::RISK_FREE_RATE;
    let credit_rate = rf_rate + credit_spread_bps / 10_000.0;

    let rf_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, (-rf_rate * 10.0).exp())])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let credit_curve = DiscountCurve::builder("USD-CREDIT")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, (-credit_rate * 10.0).exp())])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(rf_curve)
        .insert(credit_curve)
        .insert_price("AAPL", MarketScalar::Unitless(market_params::SPOT_PRICE))
        .insert_price(
            "AAPL-VOL",
            MarketScalar::Unitless(market_params::VOL_STANDARD),
        )
        .insert_price(
            "AAPL-DIVYIELD",
            MarketScalar::Unitless(market_params::DIV_YIELD),
        )
}

/// Create convertible bond with a separate credit curve.
pub fn create_convertible_with_credit() -> ConvertibleBond {
    let mut bond = create_standard_convertible();
    bond.credit_curve_id = Some("USD-CREDIT".into());
    bond
}

/// Tolerance for floating point comparisons
pub const TOLERANCE: f64 = 1e-9;

/// Relative tolerance for price comparisons (1%)
pub const PRICE_TOLERANCE_PCT: f64 = 0.01;
