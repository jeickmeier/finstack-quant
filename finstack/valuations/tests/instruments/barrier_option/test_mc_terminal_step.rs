//! Integration test: MC barrier pricer captures terminal spot at correct maturity step.
//!
//! The unit test (in `pricer.rs`) proves the off-by-one with a 2-step grid.
//! This file provides a complementary integration test over the public API using the
//! default 252-step grid: the degenerate up-and-out call (barrier >> spot) must price
//! within a generous tolerance of the Black-Scholes call.
//!
//! Note: the 252-step grid makes the off-by-one bias only ≈0.03, which is smaller than
//! one MC stderr (≈0.033). This test therefore cannot reliably distinguish buggy from
//! fixed code on its own. The definitive test is the unit test in `pricer.rs` which
//! uses `steps_per_year=2.0` to amplify the bias to a detectable ≈3.6 units.

use finstack_core::currency::Currency;
use finstack_core::dates::{DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::types::InstrumentId;
use finstack_valuations::instruments::exotics::barrier_option::{BarrierOption, BarrierType};
use finstack_valuations::instruments::{Attributes, OptionType, PricingOverrides};
use time::Month;

/// Cumulative standard normal distribution (Abramowitz & Stegun §26.2.17).
fn norm_cdf(x: f64) -> f64 {
    if x < -8.0 { return 0.0; }
    if x > 8.0 { return 1.0; }
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = t * (0.319_381_53
        + t * (-0.356_563_782
            + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    let phi = (-0.5 * x * x).exp() / (2.0_f64 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - phi * poly;
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

fn bs_call(spot: f64, strike: f64, t: f64, r: f64, q: f64, sigma: f64) -> f64 {
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    spot * (-q * t).exp() * norm_cdf(d1) - strike * (-r * t).exp() * norm_cdf(d2)
}

fn date(year: i32, month: u8, day: u8) -> finstack_core::dates::Date {
    finstack_core::dates::Date::from_calendar_date(
        year,
        Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

fn make_market(as_of: finstack_core::dates::Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    let disc = DiscountCurve::builder("USD_DISC")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
        .build()
        .expect("disc curve");

    let surface = VolSurface::builder("SPX_VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[50.0, 80.0, 100.0, 110.0, 150.0])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .expect("vol surface");

    MarketContext::new()
        .insert(disc)
        .insert_surface(surface)
        .insert_price("SPX", MarketScalar::Price(Money::new(spot, Currency::USD)))
        .insert_price("SPX_DIV", MarketScalar::Unitless(0.0))
}

/// Sanity check: MC up-and-out call with a far-above-spot barrier must price in
/// the same ballpark as the Black-Scholes vanilla call (within 5 MC standard errors).
///
/// This does NOT reliably detect the off-by-one over the default 252-step grid
/// (bias ≈ 0.03 ≈ 1 stderr); the definitive unit test lives in `pricer.rs`.
/// This test validates end-to-end wiring via the public `npv_mc()` API.
#[test]
fn barrier_uao_degenerate_matches_bs() {
    let as_of = date(2024, 1, 1);
    let expiry = date(2025, 1, 1);
    let spot = 100.0_f64;
    let strike = 100.0_f64;
    let barrier = 10_000.0_f64; // Far above spot
    let vol = 0.20_f64;
    let rate = 0.05_f64;

    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let bs_price = bs_call(spot, strike, t, rate, 0.0, vol);

    let option = BarrierOption {
        id: InstrumentId::new("BARRIER-UAO-DEGEN-INT"),
        underlying_ticker: "SPX".to_string(),
        strike,
        barrier: Money::new(barrier, Currency::USD),
        rebate: None,
        option_type: OptionType::Call,
        barrier_type: BarrierType::UpAndOut,
        expiry,
        observed_barrier_breached: None,
        notional: Money::new(1.0, Currency::USD),
        day_count: DayCount::Act365F,
        use_gobet_miri: true, // routes value() to npv_mc()
        discount_curve_id: "USD_DISC".into(),
        spot_id: "SPX".into(),
        vol_surface_id: "SPX_VOL".into(),
        div_yield_id: Some("SPX_DIV".into()),
        pricing_overrides: PricingOverrides::default(),
        monitoring_frequency: None,
        attributes: Attributes::new(),
    };

    let market = make_market(as_of, spot, vol, rate);
    let mc_pv = option
        .npv_mc(&market, as_of)
        .expect("mc price")
        .amount();

    // 5 × stderr bound: stderr ≈ BS / sqrt(100_000) ≈ 10.47 / 316 ≈ 0.033; 5σ ≈ 0.17.
    // Post-fix the MC price (252 steps) lands within ±0.17 of BS with high probability.
    // Pre-fix the bias is ≈ 0.03 (<< 0.17), so this test is NOT a reliable detector of
    // the bug — see the unit test in pricer.rs for the definitive check.
    let five_se_bound = 0.20_f64;

    println!("BS call price: {bs_price:.6}");
    println!("MC call price: {mc_pv:.6}");
    println!("Difference:    {:.6}", (mc_pv - bs_price).abs());
    println!("5σ bound:      {five_se_bound:.6}");

    assert!(
        (mc_pv - bs_price).abs() < five_se_bound,
        "MC up-and-out call (degenerate barrier) should match BS call within 5σ: \
         mc={mc_pv:.6}, bs={bs_price:.6}, diff={:.6}",
        (mc_pv - bs_price).abs()
    );
}
