//! Implied volatility tests for interest rate options.
//!
//! Validates solving for Black volatility from market prices.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, SettlementType};
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap()
}

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
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

fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

/// Round-trip: pricing a caplet with a known flat surface vol and then solving
/// for the implied vol from that price must recover the input vol. This locks in
/// the dating consistency between the pricer and the implied-vol metric (same
/// fixing date, payment date, forward period, and accrual).
#[test]
fn test_implied_vol_round_trips_pricing_vol() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01);
    let end = date!(2024 - 06 - 01);
    let surface_vol = 0.30;

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(surface_vol, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let mut caplet = CapFloor {
        id: "CAPLET_RT".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Price the caplet with the surface vol, then feed that price back as the
    // market quote the implied-vol solver must match.
    let pv = caplet.value(&market, as_of).expect("caplet should price");
    caplet.pricing_overrides.market_quotes.quoted_clean_price = Some(pv.amount());

    let result = caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("implied vol should solve");
    let implied_vol = *result
        .measures
        .get("implied_vol")
        .expect("implied_vol measure present");

    assert!(
        (implied_vol - surface_vol).abs() < 1e-4,
        "implied vol {implied_vol} should recover surface vol {surface_vol}"
    );
}

/// Implied vol calculation for CapFloor requires a market price
/// passed through pricing overrides on the MetricContext. Since CapFloor
/// does not carry pricing_overrides at the struct level, the implied vol metric
/// needs overrides to be set externally (e.g., via the pricing engine).
/// This test verifies that the metric fails gracefully when no market price is available.
#[test]
fn test_implied_vol_fails_without_market_price_override() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Future start to get t_fix > 0
    let end = date!(2024 - 06 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let caplet = CapFloor {
        id: "CAPLET_TEST".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,

        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // CapFloor does not carry pricing_overrides, so implied vol
    // requires the market price to be provided through the MetricContext.
    // Without it, the metric should fail.
    let result = caplet.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ImpliedVol],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Should fail because no market price is available
    assert!(
        result.is_err(),
        "ImpliedVol should fail without market price in pricing overrides"
    );
}
