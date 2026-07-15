//! Common test utilities and fixtures for swaption tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::pricing_overrides::VolSurfaceExtrapolation;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::rates::swaption::{Swaption, SwaptionParams};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides, OptionType};
use rust_decimal::Decimal;
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

/// Build a flat volatility surface
pub fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.02, 0.03, 0.05, 0.07])
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .build()
        .unwrap()
}

/// Build a vol surface with realistic smile (lower vol for OTM puts, higher for OTM calls)
pub fn build_smile_vol_surface(_base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0])
        .strikes(&[0.02, 0.03, 0.05, 0.07])
        .row(&[0.25, 0.22, 0.20, 0.22]) // 3M expiry
        .row(&[0.28, 0.24, 0.20, 0.24]) // 1Y expiry
        .row(&[0.32, 0.26, 0.22, 0.28]) // 5Y expiry
        .build()
        .unwrap()
}

/// Create a standard ATM payer swaption for testing
pub fn create_standard_payer_swaption(
    expiry: Date,
    swap_start: Date,
    swap_end: Date,
    strike: f64,
) -> Swaption {
    let params = SwaptionParams::payer(
        Money::new(1_000_000.0, Currency::USD),
        strike,
        expiry,
        swap_start,
        swap_end,
    )
    .expect("valid swaption params")
    .with_fixed_frequency(Tenor::semi_annual())
    .with_float_frequency(Tenor::quarterly())
    .with_day_count(DayCount::Thirty360);
    let mut swaption = Swaption::new_payer(
        "SWAPTION_TEST",
        &params,
        "USD_OIS",
        "USD_LIBOR_3M",
        "USD_SWAPTION_VOL",
    );
    // Tests intentionally exercise OTM/ITM strikes; opt in to flat extrapolation
    // to avoid making results depend on the surface strike grid.
    swaption.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_vol_surface_extrapolation(VolSurfaceExtrapolation::Clamp);
    swaption
}

/// Create a standard ATM receiver swaption for testing
pub fn create_standard_receiver_swaption(
    expiry: Date,
    swap_start: Date,
    swap_end: Date,
    strike: f64,
) -> Swaption {
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
    swaption.option_type = OptionType::Put;
    swaption.id = "RECEIVER_SWAPTION_TEST".into();
    swaption
}

/// Create a complete market context with flat curves and vol surface
pub fn create_flat_market(as_of: Date, rate: f64, vol: f64) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(rate, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(vol, as_of, "USD_SWAPTION_VOL");

    MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface)
}

/// Compute the par rate of the vanilla IRS equivalent to a legacy swaption underlier.
pub fn equivalent_vanilla_irs_par_rate(
    swaption: &Swaption,
    market: &MarketContext,
    as_of: Date,
) -> f64 {
    let fixed = |rate| {
        let mut fixed = swaption.underlying_fixed_leg.clone();
        fixed.rate = Decimal::try_from(rate).unwrap();
        fixed
    };
    let float = swaption.underlying_float_leg.clone();
    let value_at_fixed_rate = |rate| {
        InterestRateSwap::builder()
            .id(format!("IRS-{rate:.0}").into())
            .notional(swaption.notional)
            .side(PayReceive::Receive)
            .fixed(fixed(rate))
            .float(float.clone())
            .build()
            .unwrap()
            .value(market, as_of)
            .unwrap()
            .amount()
    };

    let value_zero = value_at_fixed_rate(0.0);
    let value_unit = value_at_fixed_rate(1.0);
    -value_zero / (value_unit - value_zero)
}

/// Standard test dates
pub fn standard_dates() -> (Date, Date, Date, Date) {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let swap_start = date!(2025 - 01 - 01);
    let swap_end = date!(2030 - 01 - 01);
    (as_of, expiry, swap_start, swap_end)
}

/// Assert two floats are approximately equal (relative tolerance)
pub fn assert_approx_eq(actual: f64, expected: f64, rel_tol: f64, msg: &str) {
    let diff = (actual - expected).abs();
    let scale = expected.abs().max(1.0);
    let rel_err = diff / scale;
    assert!(
        rel_err < rel_tol,
        "{}: actual={:.6}, expected={:.6}, rel_err={:.10}",
        msg,
        actual,
        expected,
        rel_err
    );
}

/// Assert a value is finite and within reasonable bounds
pub fn assert_reasonable(value: f64, lower: f64, upper: f64, name: &str) {
    assert!(
        value.is_finite(),
        "{} should be finite, got: {}",
        name,
        value
    );
    assert!(
        value >= lower && value <= upper,
        "{} should be in [{}, {}], got: {}",
        name,
        lower,
        upper,
        value
    );
}
