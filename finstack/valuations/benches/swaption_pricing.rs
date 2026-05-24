//! Swaption pricing benchmarks.
//!
//! Measures performance of swaption operations:
//! - Present value calculation (Black76 model)
//! - SABR-implied volatility pricing
//! - Greeks calculation (delta, vega, gamma)
//! - Forward swap rate and annuity calculation
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::PayReceive;
use finstack_valuations::instruments::rates::swaption::SABRParameters;
use finstack_valuations::instruments::rates::swaption::Swaption;
use finstack_valuations::instruments::rates::swaption::SwaptionParams;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

fn create_swaption(expiry_months: i64, swap_tenor_years: i32) -> Swaption {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base + time::Duration::days(expiry_months * 30);
    let swap_start = expiry;
    let swap_end = Date::from_calendar_date(2025 + swap_tenor_years, Month::January, 1).unwrap();

    let params = SwaptionParams {
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.04).expect("valid literal"), // 4% strike
        expiry,
        swap_start,
        swap_end,
        side: PayReceive::Pay,
        fixed_freq: Some(Tenor::semi_annual()),
        float_freq: Some(Tenor::quarterly()),
        day_count: Some(DayCount::Thirty360),
        vol_model: None,
    };

    Swaption::new_payer(
        format!("SWAPTION-{}Mx{}Y", expiry_months, swap_tenor_years),
        &params,
        "USD-OIS",
        "USD-SOFR-3M",
        "SWAPTION-VOL",
    )
}

fn create_swaption_with_sabr(expiry_months: i64, swap_tenor_years: i32) -> Swaption {
    let mut swaption = create_swaption(expiry_months, swap_tenor_years);
    swaption.sabr_params = Some(SABRParameters {
        alpha: 0.15,
        beta: 0.5,
        nu: 0.40,
        rho: -0.25,
        shift: None,
    });
    swaption
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.035),
            (1.0, 0.038),
            (2.0, 0.040),
            (5.0, 0.045),
            (10.0, 0.050),
            (30.0, 0.055),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Create a vol surface for swaption volatility
    let vol_surface = VolSurface::from_grid(
        "SWAPTION-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0],     // Expiry times
        &[0.02, 0.03, 0.04, 0.05, 0.06], // Strike rates
        &[0.20; 25],                     // Flat 20% vol
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_surface(vol_surface)
}

fn bench_swaption_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_pv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        let swaption = create_swaption(*expiry_months, *swap_tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                b.iter(|| swaption.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_swaption_sabr(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_sabr");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        let swaption = create_swaption_with_sabr(*expiry_months, *swap_tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                b.iter(|| swaption.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_swaption_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_greeks");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let greeks = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
    ];

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 10)].iter() {
        let swaption = create_swaption(*expiry_months, *swap_tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                b.iter(|| {
                    swaption.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&greeks),
                        finstack_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_forward_swap_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_forward_rate");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        let swaption = create_swaption(*expiry_months, *swap_tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                b.iter(|| swaption.forward_swap_rate(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_swap_annuity(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_annuity");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        let swaption = create_swaption(*expiry_months, *swap_tenor);
        let disc = market.get_discount(&swaption.discount_curve_id).unwrap();
        let disc = disc.as_ref();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                b.iter(|| swaption.swap_annuity(black_box(disc), black_box(as_of)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_swaption_pv,
    bench_swaption_sabr,
    bench_swaption_greeks,
    bench_forward_swap_rate,
    bench_swap_annuity
);
criterion_main!(benches);
