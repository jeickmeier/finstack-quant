//! Key-rate wing-bucket regression tests for Taylor attribution.
//!
//! REGRESSION (quant review B3): `key_rate_bump_spec` previously passed
//! `prev = 0.0` for the first bucket (understating sub-3M DV01 — a 1M knot
//! got triangular weight 1/3 instead of the flat 1.0 of the left-wing half
//! triangle, breaking the Σwᵢ(t)=1 partition) and `next = f64::INFINITY` for
//! the last bucket (NaN triangular weight for any knot beyond 30Y, which
//! errored the bump and silently dropped the ENTIRE rates factor for that
//! curve into the residual with only a tracing warn).
//!
//! These tests pin the fixed behavior with curves carrying real wing knots:
//! a 1M pillar on the short end and a 40Y pillar on the long end.

use finstack_attribution::{attribute_pnl_taylor, ExecutionPolicy, TaylorAttributionConfig};
use finstack_core::currency::Currency;
use finstack_core::dates::create_date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Instrument;
use std::sync::Arc;
use time::Month;

fn df_from_rate(rate: f64, years: f64) -> f64 {
    (-rate * years).exp()
}

/// Flat curve with explicit wing pillars: 1M on the short end, 40Y on the
/// long end (40Y/50Y pillars are routine on EUR/GBP/USD long curves).
fn build_wing_curve(curve_id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0 / 12.0, 0.25, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 40.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, df_from_rate(rate, t))).collect();
    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn attribute_bond_on_wing_curve(maturity_year: i32) -> finstack_attribution::PnlAttribution {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let bond = Bond::fixed(
        "WING-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.04,
        create_date(2025, Month::January, 1).unwrap(),
        create_date(maturity_year, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    let market_t0 = MarketContext::new().insert(build_wing_curve("USD-OIS", as_of_t0, 0.04));
    // Parallel +10bp move across the whole curve, including the wing pillars.
    let market_t1 = MarketContext::new().insert(build_wing_curve("USD-OIS", as_of_t1, 0.041));

    attribute_pnl_taylor(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &TaylorAttributionConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("Taylor attribution should succeed on a curve with wing knots")
}

/// A curve with a 40Y pillar must not lose its entire rates factor: before
/// the fix the last-bucket bump errored (NaN weight at t=40 from the ∞
/// sentinel) and the whole curve's rates P&L was dropped into the residual.
#[test]
fn taylor_long_end_wing_knot_does_not_drop_rates_factor() {
    let attribution = attribute_bond_on_wing_curve(2055);

    assert!(
        attribution.rates_curves_pnl.amount() < 0.0,
        "long bond + rates up must show a rates loss, got {}",
        attribution.rates_curves_pnl
    );
    // The rates factor must explain the move, not the residual: a dropped
    // factor would show rates = 0 and |residual| ≈ |total - carry|.
    assert!(
        attribution.rates_curves_pnl.amount().abs() > 5.0 * attribution.residual.amount().abs(),
        "rates P&L ({}) must be attributed, not left in residual ({})",
        attribution.rates_curves_pnl,
        attribution.residual,
    );
}

/// A short-dated bond whose PV lives entirely below the 3M bucket must have
/// its rates move fully explained: before the fix the first bucket's
/// rising-triangle weight (t/0.25) understated sub-3M DV01 by up to ~67%,
/// pushing the lost short-end P&L into the residual.
#[test]
fn taylor_short_end_wing_explains_sub_3m_rates_move() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // Zero-coupon style short bond: single principal cashflow ~2 months out,
    // discounted off the 1M/3M segment of the curve.
    let bond = Bond::fixed(
        "WING-BOND-SHORT",
        Money::new(1_000_000.0, Currency::USD),
        0.0,
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2025, Month::March, 15).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    let market_t0 = MarketContext::new().insert(build_wing_curve("USD-OIS", as_of_t0, 0.04));
    let market_t1 = MarketContext::new().insert(build_wing_curve("USD-OIS", as_of_t1, 0.041));

    let attribution = attribute_pnl_taylor(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &TaylorAttributionConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("Taylor attribution should succeed on a short-dated bond");

    let rates = attribution.rates_curves_pnl.amount();
    let residual = attribution.residual.amount();
    assert!(
        rates < 0.0,
        "short bond + rates up must show a rates loss, got {rates}"
    );
    // With the flat left-wing weight the explained P&L covers the move; the
    // old rising-triangle weight left ~2/3 of the sub-3M DV01 unexplained,
    // making |residual| comparable to |rates|.
    assert!(
        residual.abs() < 0.25 * rates.abs(),
        "sub-3M rates move must be explained by the first key-rate bucket: \
         rates={rates}, residual={residual}"
    );
}
