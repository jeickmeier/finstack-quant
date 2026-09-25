//! Shared test utilities for IR Future tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture, RateAveragingMethod,
};
pub use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ListedFutureTerms, Position};
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

/// Listed terms for a term-rate SOFR-style future: `notional / 1,000,000`
/// contracts at $2,500 per price point, settling on the last trading date.
pub fn listed_terms(
    notional: f64,
    last_trading_date: Date,
    entry_price: f64,
    position: Position,
) -> ListedFutureTerms {
    ListedFutureTerms::new(
        notional / 1_000_000.0,
        2_500.0,
        Currency::USD,
        entry_price,
        last_trading_date,
        last_trading_date,
        position,
    )
    .expect("valid listed terms")
}

/// Create a standard IR future for testing
pub fn create_standard_future(start: Date, end: Date) -> InterestRateFuture {
    InterestRateFuture {
        id: "IRF_TEST".into(),
        terms: listed_terms(1_000_000.0, start, 97.50, Position::Long),
        fixing_date: Some(start),
        period_start: Some(start),
        period_end: Some(end),
        day_count: DayCount::Act360,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        rate_averaging: RateAveragingMethod::Term,
        fixing_index_id: None,
        fixing_calendar_id: None,
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
    entry_price: f64,
    position: Position,
) -> InterestRateFuture {
    InterestRateFuture {
        id: id.into(),
        terms: listed_terms(notional, expiry, entry_price, position),
        fixing_date: Some(expiry),
        period_start: Some(period_start),
        period_end: Some(period_end),
        day_count: DayCount::Act360,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        rate_averaging: RateAveragingMethod::Term,
        fixing_index_id: None,
        fixing_calendar_id: None,
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
        delivery_months: 3,
        convexity_adjustment: Some(0.0),
    }
}

/// Create Eurodollar-style contract specs
pub fn create_eurodollar_specs() -> FutureContractSpecs {
    FutureContractSpecs {
        face_value: 1_000_000.0,
        tick_size: 0.0025,
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
