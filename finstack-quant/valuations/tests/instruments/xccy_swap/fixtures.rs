//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::FxMatrix;
use finstack_quant_core::money::fx::SimpleFxProvider;
pub use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::Month;

pub fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

pub const USD_CAL: &str = "usny";
pub const EUR_CAL: &str = "target2";

pub fn market_with_fx() -> MarketContext {
    let base = d(2025, 1, 2);

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.985), (2.0, 0.97)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots(vec![(0.0, 0.02), (2.0, 0.02)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
        .base_date(base)
        .knots(vec![(0.0, 0.015), (2.0, 0.015)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        p
    };
    let fx = FxMatrix::new(provider);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
        .insert_fx(fx)
}

pub fn market_without_fx() -> MarketContext {
    let base = d(2025, 1, 2);

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.985), (2.0, 0.97)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots(vec![(0.0, 0.02), (2.0, 0.02)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
        .base_date(base)
        .knots(vec![(0.0, 0.015), (2.0, 0.015)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
}

pub fn leg_usd_receive(
    start: Date,
    end: Date,
) -> finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
    finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
        currency: Currency::USD,
        notional: finstack_quant_core::money::Money::new(1_000_000.0, Currency::USD),
        side: finstack_quant_valuations::instruments::rates::xccy_swap::LegSide::Receive,
        forward_curve_id: finstack_quant_core::types::CurveId::new("USD-SOFR-3M"),
        discount_curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: Some(USD_CAL.to_string()),
        allow_calendar_fallback: false,
        reset_lag_days: None,
    }
}

pub fn leg_eur_pay(
    start: Date,
    end: Date,
) -> finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
    finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
        currency: Currency::EUR,
        notional: finstack_quant_core::money::Money::new(900_000.0, Currency::EUR),
        side: finstack_quant_valuations::instruments::rates::xccy_swap::LegSide::Pay,
        forward_curve_id: finstack_quant_core::types::CurveId::new("EUR-EURIBOR-3M"),
        discount_curve_id: finstack_quant_core::types::CurveId::new("EUR-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: Some(EUR_CAL.to_string()),
        allow_calendar_fallback: false,
        reset_lag_days: None,
    }
}

/// Market context with curves extending to 15 years for long-dated swap tests.
pub fn market_with_extended_curves() -> MarketContext {
    let base = d(2025, 1, 2);

    // Extended discount curves out to 15 years
    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.90),
            (10.0, 0.80),
            (15.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.985),
            (2.0, 0.97),
            (5.0, 0.92),
            (10.0, 0.85),
            (15.0, 0.78),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Extended forward curves
    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots(vec![(0.0, 0.02), (5.0, 0.025), (10.0, 0.03), (15.0, 0.032)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
        .base_date(base)
        .knots(vec![
            (0.0, 0.015),
            (5.0, 0.02),
            (10.0, 0.025),
            (15.0, 0.028),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        p
    };
    let fx = FxMatrix::new(provider);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
        .insert_fx(fx)
}
