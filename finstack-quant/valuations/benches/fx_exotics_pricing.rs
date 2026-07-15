#![allow(clippy::unwrap_used)]

//! FX exotic instrument pricing benchmarks.
//!
//! Covers the analytical hot paths for:
//! - [`FxBarrierOption`]: closed-form single-barrier pricing (Rubinstein & Reiner 1991).
//! - [`FxTouchOption`]: closed-form one-touch / no-touch pricing.
//! - [`FxDigitalOption`]: cash-or-nothing / asset-or-nothing digital options.
//! - [`FxVarianceSwap`]: fair-strike via Carr-Madan replication across vol surface.
//! - [`QuantoOption`]: quanto drift-adjusted Black-Scholes.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::stats::RealizedVarMethod;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::exotics::BarrierType;
use finstack_quant_valuations::instruments::fx::fx_barrier_option::FxBarrierOption;
use finstack_quant_valuations::instruments::fx::fx_digital_option::{
    DigitalPayoutType, FxDigitalOption,
};
use finstack_quant_valuations::instruments::fx::fx_touch_option::{
    BarrierDirection, FxTouchOption, PayoutTiming, TouchType,
};
use finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap;
use finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{Attributes, InstrumentPricingOverrides, OptionType};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

// ================================================================================================
// Market setup
// ================================================================================================

fn base_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 2).unwrap()
}

/// Build a shared `MarketContext` for EUR/USD FX exotics.
///
/// Provides:
/// - `USD-OIS` domestic discount curve (5%)
/// - `EUR-OIS` foreign discount curve (3%)
/// - `JPY-OIS` third-currency discount curve (0.1%) for quanto benchmarks
/// - `NKY-DIV` dividend yield curve (1%) for quanto benchmarks
/// - Flat `EURUSD-VOL` vol surface at 10%
/// - Flat `NKY-VOL` equity vol surface at 20% (strikes in JPY levels)
/// - Flat `JPYUSD-VOL` FX vol surface at 8% (quote-per-base: USD per JPY)
/// - EUR/USD spot = 1.10; JPY/USD spot = 1/150; NKY-SPOT = 35 000
fn create_market(as_of: Date) -> MarketContext {
    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.9512),
            (2.0, 0.9048),
            (5.0, 0.7788),
            (10.0, 0.6065),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.9704),
            (2.0, 0.9418),
            (5.0, 0.8607),
            (10.0, 0.7408),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let jpy_disc = DiscountCurve::builder("JPY-OIS")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.9990),
            (5.0, 0.9950),
            (10.0, 0.9900),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let nky_div = DiscountCurve::builder("NKY-DIV")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.9900),
            (5.0, 0.9512),
            (10.0, 0.9048),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Flat EURUSD vol surface — strikes as dimensionless FX rates
    let eurusd_strikes = vec![0.90, 1.00, 1.10, 1.20, 1.30];
    let eurusd_tenors = vec![0.25, 0.5, 1.0, 2.0, 5.0];
    let flat_eurusd_row = vec![0.10_f64; eurusd_strikes.len()];
    let mut eurusd_vol_builder = VolSurface::builder(CurveId::new("EURUSD-VOL"))
        .expiries(&eurusd_tenors)
        .strikes(&eurusd_strikes);
    for _ in 0..eurusd_tenors.len() {
        eurusd_vol_builder = eurusd_vol_builder.row(&flat_eurusd_row);
    }
    let eurusd_vol = eurusd_vol_builder.build().unwrap();

    // Flat NKY equity vol surface — strikes as index levels (JPY)
    let nky_strikes = vec![25_000.0, 30_000.0, 35_000.0, 40_000.0, 45_000.0];
    let nky_tenors = vec![0.25, 0.5, 1.0, 2.0, 5.0];
    let flat_nky_row = vec![0.20_f64; nky_strikes.len()];
    let mut nky_vol_builder = VolSurface::builder(CurveId::new("NKY-VOL"))
        .expiries(&nky_tenors)
        .strikes(&nky_strikes);
    for _ in 0..nky_tenors.len() {
        nky_vol_builder = nky_vol_builder.row(&flat_nky_row);
    }
    let nky_vol = nky_vol_builder.build().unwrap();

    // Flat JPYUSD vol surface — strikes as dimensionless FX rates quoted
    // quote-per-base (USD per JPY), the direction the quanto pricer requires.
    let jpyusd_strikes = vec![
        1.0 / 170.0,
        1.0 / 160.0,
        1.0 / 150.0,
        1.0 / 140.0,
        1.0 / 130.0,
    ];
    let jpyusd_tenors = vec![0.25, 0.5, 1.0, 2.0, 5.0];
    let flat_jpyusd_row = vec![0.08_f64; jpyusd_strikes.len()];
    let mut jpyusd_vol_builder = VolSurface::builder(CurveId::new("JPYUSD-VOL"))
        .expiries(&jpyusd_tenors)
        .strikes(&jpyusd_strikes);
    for _ in 0..jpyusd_tenors.len() {
        jpyusd_vol_builder = jpyusd_vol_builder.row(&flat_jpyusd_row);
    }
    let jpyusd_vol = jpyusd_vol_builder.build().unwrap();

    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .unwrap();
    fx_provider
        .set_quote(Currency::JPY, Currency::USD, 1.0 / 150.0)
        .unwrap();
    fx_provider
        .set_quote(Currency::USD, Currency::USD, 1.0)
        .unwrap();
    let fx = FxMatrix::new(fx_provider);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(jpy_disc)
        .insert(nky_div)
        .insert_surface(eurusd_vol)
        .insert_surface(nky_vol)
        .insert_surface(jpyusd_vol)
        .insert_fx(fx)
        .insert_price("EURUSD", MarketScalar::Unitless(1.10))
        .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10))
        .insert_price("NKY-SPOT", MarketScalar::Unitless(35_000.0))
        .insert_price("JPYUSD-SPOT", MarketScalar::Unitless(1.0 / 150.0))
}

// ================================================================================================
// Instrument factories
// ================================================================================================

fn make_barrier(as_of: Date, tenor_years: i32, barrier_type: BarrierType) -> FxBarrierOption {
    let expiry =
        Date::from_calendar_date(as_of.year() + tenor_years, as_of.month(), as_of.day()).unwrap();
    FxBarrierOption::builder()
        .id(InstrumentId::new("FXBAR-BENCH"))
        .strike(1.10)
        .barrier(1.20)
        .option_type(OptionType::Call)
        .barrier_type(barrier_type)
        .expiry(expiry)
        .observed_barrier_breached_opt(None)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .use_gobet_miri(false)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn make_touch(as_of: Date, tenor_years: i32, touch_type: TouchType) -> FxTouchOption {
    let expiry =
        Date::from_calendar_date(as_of.year() + tenor_years, as_of.month(), as_of.day()).unwrap();
    FxTouchOption::builder()
        .id(InstrumentId::new("FXTOUCH-BENCH"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .barrier_level(1.05)
        .touch_type(touch_type)
        .barrier_direction(BarrierDirection::Down)
        .payout_amount(Money::new(1_000_000.0, Currency::USD))
        .payout_timing(PayoutTiming::AtExpiry)
        .expiry(expiry)
        .day_count(DayCount::Act365F)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .observed_touch_opt(None)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn make_digital(as_of: Date, tenor_years: i32, payout_type: DigitalPayoutType) -> FxDigitalOption {
    let expiry =
        Date::from_calendar_date(as_of.year() + tenor_years, as_of.month(), as_of.day()).unwrap();
    FxDigitalOption::builder()
        .id(InstrumentId::new("FXDIG-BENCH"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .strike(1.10)
        .option_type(OptionType::Call)
        .payout_type(payout_type)
        .payout_amount(Money::new(1_000_000.0, Currency::USD))
        .expiry(expiry)
        .day_count(DayCount::Act365F)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn make_variance_swap(as_of: Date, tenor_years: i32) -> FxVarianceSwap {
    use finstack_quant_valuations::instruments::fx::fx_variance_swap::PayReceive;
    let maturity =
        Date::from_calendar_date(as_of.year() + tenor_years, as_of.month(), as_of.day()).unwrap();
    FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-BENCH"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".to_string())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .strike_variance(0.04)
        .start_date(as_of)
        .maturity(maturity)
        .observation_freq(Tenor::daily())
        .base_calendar_id("TARGET2".to_string())
        .quote_calendar_id("USNY".to_string())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn make_quanto(as_of: Date, tenor_years: i32) -> QuantoOption {
    let expiry =
        Date::from_calendar_date(as_of.year() + tenor_years, as_of.month(), as_of.day()).unwrap();
    QuantoOption::builder()
        .id(InstrumentId::new("QUANTO-BENCH"))
        .underlying_ticker("NKY".to_string())
        .equity_strike(Money::new(35_000.0, Currency::JPY))
        .option_type(OptionType::Call)
        .expiry(expiry)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .underlying_quantity_opt(Some(100.0))
        .payoff_fx_rate_opt(Some(1.0 / 150.0))
        .base_currency(Currency::JPY)
        .quote_currency(Currency::USD)
        .correlation(-0.2)
        .day_count(DayCount::Act365F)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
        .spot_id("NKY-SPOT".into())
        .vol_surface_id(CurveId::new("NKY-VOL"))
        .div_yield_id_opt(Some(CurveId::new("NKY-DIV")))
        .fx_rate_id_opt(Some("JPYUSD-SPOT".to_string()))
        .fx_vol_id_opt(Some(CurveId::new("JPYUSD-VOL")))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

// ================================================================================================
// Benchmarks
// ================================================================================================

/// Scale barrier option pricing vs tenor (1Y, 2Y, 5Y).
///
/// Measures the Rubinstein-Reiner closed-form single-barrier pricer including
/// spot and vol-surface lookups and discount-factor interpolation.
fn bench_barrier_option_tenor(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_barrier_option/tenor");
    let years = 2;
    let inst = make_barrier(as_of, years, BarrierType::UpAndOut);
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{years}Y")),
        &years,
        |b, _| b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap()),
    );
    group.finish();
}

/// Compare all four barrier types: UpAndOut, UpAndIn, DownAndOut, DownAndIn.
fn bench_barrier_option_type(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let cases = [
        ("UpAndOut", BarrierType::UpAndOut),
        ("UpAndIn", BarrierType::UpAndIn),
        ("DownAndOut", BarrierType::DownAndOut),
        ("DownAndIn", BarrierType::DownAndIn),
    ];

    let mut group = c.benchmark_group("fx_barrier_option/barrier_type");
    for (name, barrier_type) in &cases {
        let inst = make_barrier(as_of, 1, *barrier_type);
        group.bench_with_input(BenchmarkId::from_parameter(*name), name, |b, _| {
            b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

/// Scale touch option pricing vs tenor (1Y, 2Y, 5Y).
fn bench_touch_option_tenor(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_touch_option/tenor");
    let years = 2;
    let inst = make_touch(as_of, years, TouchType::OneTouch);
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{years}Y")),
        &years,
        |b, _| b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap()),
    );
    group.finish();
}

/// Compare one-touch vs no-touch pricing.
fn bench_touch_option_type(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_touch_option/touch_type");
    for (name, touch_type) in [
        ("OneTouch", TouchType::OneTouch),
        ("NoTouch", TouchType::NoTouch),
    ] {
        let inst = make_touch(as_of, 1, touch_type);
        group.bench_with_input(BenchmarkId::from_parameter(name), &name, |b, _| {
            b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

/// Compare cash-or-nothing vs asset-or-nothing digital options.
fn bench_digital_option_payout_type(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_digital_option/payout_type");
    for (name, payout_type) in [
        ("CashOrNothing", DigitalPayoutType::CashOrNothing),
        ("AssetOrNothing", DigitalPayoutType::AssetOrNothing),
    ] {
        let inst = make_digital(as_of, 1, payout_type);
        group.bench_with_input(BenchmarkId::from_parameter(name), &name, |b, _| {
            b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

/// Scale variance swap replication vs tenor (1Y, 2Y, 5Y).
///
/// Fair-strike is computed via Carr-Madan replication integrating over the vol
/// surface. Longer tenors require wider strike ranges and more interpolation calls.
fn bench_variance_swap_tenor(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_variance_swap/tenor");
    let years = 2;
    let inst = make_variance_swap(as_of, years);
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{years}Y")),
        &years,
        |b, _| b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap()),
    );
    group.finish();
}

/// Scale quanto option pricing vs tenor (1Y, 2Y, 5Y).
///
/// Measures quanto drift adjustment and BS pricing.
fn bench_quanto_option_tenor(c: &mut Criterion) {
    let as_of = base_date();
    let market = create_market(as_of);

    let mut group = c.benchmark_group("fx_quanto_option/tenor");
    let years = 2;
    let inst = make_quanto(as_of, years);
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{years}Y")),
        &years,
        |b, _| b.iter(|| black_box(inst.value(black_box(&market), black_box(as_of))).unwrap()),
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_barrier_option_tenor,
    bench_barrier_option_type,
    bench_touch_option_tenor,
    bench_touch_option_type,
    bench_digital_option_payout_type,
    bench_variance_swap_tenor,
    bench_quanto_option_tenor,
);
criterion_main!(benches);
