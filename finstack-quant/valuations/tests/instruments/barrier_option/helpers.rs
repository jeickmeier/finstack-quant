//! Test helpers for barrier option tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::exotics::barrier_option::{BarrierOption, BarrierType};
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::instruments::PricingOverrides;

/// Standard curve IDs
pub const DISC_ID: &str = "USD_DISC";
pub const SPOT_ID: &str = "SPX";
pub const VOL_ID: &str = "SPX_VOL";
pub const DIV_ID: &str = "SPX_DIV";

/// Build a flat discount curve with specified day count basis.
///
/// This allows testing scenarios where the discount curve and vol surface
/// use different day count conventions.
pub fn build_discount_curve_with_dc(
    rate: f64,
    base_date: Date,
    curve_id: &str,
    day_count: DayCount,
) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(day_count)
        .knots([
            (0.0, 1.0),
            (0.25, (-rate * 0.25).exp()),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ]);

    // For zero or negative rates, the curve may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

/// Build a flat vol surface.
///
/// The vol surface expiries are year fractions computed assuming the standard
/// vol surface convention (typically ACT/365F for equity options).
pub fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 0.5, 1.0, 2.0, 5.0])
        .strikes(&[50.0, 80.0, 100.0, 120.0, 150.0, 200.0])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

/// Create a down-and-out call barrier option.
///
/// # Arguments
/// * `expiry` - Option expiry date
/// * `strike` - Strike price
/// * `barrier` - Barrier level (must be below spot for down-and-out)
/// * `day_count` - Day count convention for the instrument (should match vol surface)
pub fn create_down_and_out_call(
    expiry: Date,
    strike: f64,
    barrier: f64,
    day_count: DayCount,
) -> BarrierOption {
    BarrierOption {
        id: "BARRIER_DOC_TEST".into(),
        underlying_ticker: SPOT_ID.into(),
        strike,
        barrier: Money::new(barrier, Currency::USD),
        rebate: None,
        rebate_timing: Default::default(),
        option_type: OptionType::Call,
        barrier_type: BarrierType::DownAndOut,
        expiry,
        observed_barrier_breached: None,
        notional: Money::new(1.0, Currency::USD),
        day_count,
        use_gobet_miri: false,
        discount_curve_id: DISC_ID.into(),
        spot_id: SPOT_ID.into(),
        vol_surface_id: VOL_ID.into(),
        div_yield_id: Some(DIV_ID.into()),
        pricing_overrides: PricingOverrides::default(),
        monitoring_frequency: None,
        attributes: Default::default(),
    }
}

/// Create a market context with specified discount curve day count.
///
/// This allows testing scenarios where the discount curve uses a different
/// day count basis than the vol surface (vol surface is assumed ACT/365F).
pub fn build_market_with_dc(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
    disc_day_count: DayCount,
) -> MarketContext {
    let disc_curve = build_discount_curve_with_dc(rate, as_of, DISC_ID, disc_day_count);
    let vol_surface = build_flat_vol_surface(vol, as_of, VOL_ID);

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price(
            SPOT_ID,
            MarketScalar::Price(Money::new(spot, Currency::USD)),
        )
        .insert_price(DIV_ID, MarketScalar::Unitless(div_yield))
}
