//! Equity option pricing benchmarks.
//!
//! Measures performance of Black-Scholes pricing and Greeks:
//! - Present value (call and put)
//! - Full Greeks calculation (delta, gamma, vega, theta, rho)
//! - Individual Greeks
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::exotics::barrier_option::{BarrierOption, BarrierType};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides, OptionType};
use finstack_quant_valuations::metrics::MetricId;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}
use std::hint::black_box;
use time::Month;

fn create_call_option(expiry_months: i64) -> EquityOption {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base + time::Duration::days(expiry_months * 30);

    test_utils::equity_option_european_call(
        format!("CALL-{}M", expiry_months),
        "AAPL",
        100.0, // ATM strike
        expiry,
        100.0, // 100 shares
    )
    .expect("equity option should build in benchmarks")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let vol_surface = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20], // Flat 25% vol
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price(
            "EQUITY-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        )
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02))
}

fn create_barrier_option() -> BarrierOption {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    BarrierOption {
        expiry_fixing: None,
        id: "BARRIER-12M".into(),
        underlying_ticker: "SPX".into(),
        strike: 100.0,
        barrier: Money::new(80.0, Currency::USD),
        rebate: None,
        rebate_timing: Default::default(),
        option_type: OptionType::Call,
        barrier_type: BarrierType::DownAndOut,
        expiry: as_of + time::Duration::days(365),
        observed_barrier_breached: None,
        notional: Money::new(100.0, Currency::USD),
        day_count: finstack_quant_core::dates::DayCount::Act365F,
        use_gobet_miri: false,
        discount_curve_id: "USD_DISC".into(),
        spot_id: "SPX".into(),
        vol_surface_id: "SPX_VOL".into(),
        div_yield_id: Some("SPX_DIV".into()),
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
        monitoring_frequency: None,
    }
}

fn create_barrier_market() -> MarketContext {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD_DISC")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let vol_surface = VolSurface::from_grid(
        "SPX_VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20],
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price("SPX", MarketScalar::Price(Money::new(100.0, Currency::USD)))
        .insert_price("SPX_DIV", MarketScalar::Unitless(0.02))
}

fn bench_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("option_pv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for months in [3, 6, 12, 24].iter() {
        let option = create_call_option(*months);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}M", months)),
            months,
            |b, _| {
                b.iter(|| option.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("option_greeks");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let greeks = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
    ];

    for months in [3, 6, 12].iter() {
        let option = create_call_option(*months);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}M", months)),
            months,
            |b, _| {
                b.iter(|| {
                    option.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&greeks),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_barrier_option_fd_greeks(c: &mut Criterion) {
    let market = create_barrier_market();
    let option = create_barrier_option();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let greeks = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Vanna,
        MetricId::Volga,
    ];

    c.bench_function("barrier_option_fd_greeks_5_metrics", |b| {
        b.iter(|| {
            option.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&greeks),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });
}

criterion_group!(
    benches,
    bench_option_pv,
    bench_option_greeks,
    bench_barrier_option_fd_greeks
);
criterion_main!(benches);
