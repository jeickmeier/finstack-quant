//! Shared test utilities for IR Future tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture, Position,
};
pub use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

/// Build a flat forward curve with constant rate
pub fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap()
}

/// Build a flat discount curve with constant rate
pub fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

/// Build a standard market context for testing
pub fn build_standard_market(as_of: Date, rate: f64) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(rate, as_of, "USD_LIBOR_3M");

    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

/// Create a standard IR future for testing
pub fn create_standard_future(start: Date, end: Date) -> InterestRateFuture {
    InterestRateFuture {
        id: "IRF_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        expiry: start,
        fixing_date: Some(start),
        period_start: Some(start),
        period_end: Some(end),
        quoted_price: 97.50, // Implies 2.5% rate
        day_count: DayCount::Act360,
        position: Position::Long,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Create a custom future with specified parameters
pub fn create_custom_future(
    id: &str,
    notional: f64,
    expiry: Date,
    period_start: Date,
    period_end: Date,
    quoted_price: f64,
    position: Position,
) -> InterestRateFuture {
    InterestRateFuture {
        id: id.into(),
        notional: Money::new(notional, Currency::USD),
        expiry,
        fixing_date: Some(expiry),
        period_start: Some(period_start),
        period_end: Some(period_end),
        quoted_price,
        day_count: DayCount::Act360,
        position,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Create SOFR-style contract specs (standard CME SOFR future)
pub fn create_sofr_specs() -> FutureContractSpecs {
    FutureContractSpecs {
        face_value: 1_000_000.0,
        tick_size: 0.0025, // 0.25 bp
        tick_value: 6.25,  // $6.25 per tick for 3M
        delivery_months: 3,
        convexity_adjustment: Some(0.0),
    }
}

/// Create Eurodollar-style contract specs
pub fn create_eurodollar_specs() -> FutureContractSpecs {
    FutureContractSpecs {
        face_value: 1_000_000.0,
        tick_size: 0.0025,
        tick_value: 6.25,
        delivery_months: 3,
        convexity_adjustment: Some(0.0),
    }
}

/// Standard test dates
pub fn standard_dates() -> (Date, Date, Date) {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 07 - 01);
    let end = date!(2024 - 10 - 01);
    (as_of, start, end)
}

/// Near-term test dates (short dated)
pub fn near_term_dates() -> (Date, Date, Date) {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 15);
    let end = date!(2024 - 02 - 15);
    (as_of, start, end)
}

/// Far forward test dates
pub fn far_forward_dates() -> (Date, Date, Date) {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2026 - 01 - 01);
    let end = date!(2026 - 04 - 01);
    (as_of, start, end)
}
