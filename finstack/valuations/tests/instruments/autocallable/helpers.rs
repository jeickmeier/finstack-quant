//! Test helpers for autocallable tests.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(dead_code)] // Helper functions may not all be used in every test configuration

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::equity::autocallable::{Autocallable, FinalPayoffType};
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::PricingOverrides;

/// Standard curve IDs
pub const DISC_ID: &str = "USD_DISC";
pub const SPOT_ID: &str = "SPX";
pub const VOL_ID: &str = "SPX_VOL";
pub const DIV_ID: &str = "SPX_DIV";

/// Build a flat discount curve with specified day count basis.
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
            finstack_core::market_data::term_structures::ValidationMode::Raw {
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
        .expiries(&[0.25, 0.5, 0.75, 1.0, 2.0, 5.0])
        .strikes(&[50.0, 80.0, 100.0, 120.0, 150.0, 200.0])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

/// Create a quarterly observation autocallable.
///
/// # Arguments
/// * `observation_dates` - Observation dates for autocall checking
/// * `day_count` - Day count convention for the instrument
/// * `seed_scenario` - MC seed scenario for deterministic pricing
pub fn create_quarterly_autocallable(
    observation_dates: Vec<Date>,
    day_count: DayCount,
    seed_scenario: Option<&str>,
) -> Autocallable {
    let n = observation_dates.len();
    let autocall_barriers = vec![1.0; n]; // 100% of initial spot
    let coupons = vec![0.02; n]; // 2% per observation

    Autocallable {
        id: "AUTO_DC_TEST".into(),
        underlying_ticker: SPOT_ID.into(),
        expiry: *observation_dates
            .last()
            .expect("observation_dates cannot be empty"),
        observation_dates,
        autocall_barriers,
        coupons,
        memory_coupons: false, // non-memory autocallable
        final_barrier: 0.6,    // 60% knock-in barrier
        final_payoff_type: FinalPayoffType::Participation { rate: 1.0 },
        participation_rate: 1.0,
        cap_level: 1.5, // 150% cap
        notional: Money::new(100_000.0, Currency::USD),
        day_count,
        discount_curve_id: CurveId::new(DISC_ID),
        spot_id: SPOT_ID.into(),
        vol_surface_id: CurveId::new(VOL_ID),
        div_yield_id: Some(CurveId::new(DIV_ID)),
        pricing_overrides: seed_scenario
            .map(|s| PricingOverrides::default().with_mc_seed_scenario(s))
            .unwrap_or_default(),
        attributes: Attributes::new(),
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
