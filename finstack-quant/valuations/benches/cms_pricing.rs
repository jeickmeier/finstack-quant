//! CMS option and CMS swap pricing benchmarks.
//!
//! Both instruments use Gauss-Legendre quadrature via static replication to
//! compute the convexity-adjusted CMS rate, iterating over the full swaption
//! vol surface for each fixing date. The computational cost scales with:
//!   - Number of fixing dates  (outer loop)
//!   - CMS tenor              (annuity sum loop inside each GL node)
//!   - Number of GL nodes     (integration accuracy)
//!
//! Scenarios:
//! - CMS cap/floor: period count scaling (4 / 8 / 20 / 40 quarterly fixings)
//! - CMS cap: CMS tenor scaling (5Y / 10Y / 20Y)
//! - CmsSwap: period count scaling (same dims)

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::cms_option::CmsOption;
use finstack_quant_valuations::instruments::rates::cms_swap::{CmsSwap, FundingLegSpec};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{IRSConvention, PayReceive};
use finstack_quant_valuations::instruments::{OptionType, PricingOverrides};
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

// ---------------------------------------------------------------------------
// Market context
// ---------------------------------------------------------------------------

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = vec![
        (0.0, 1.0),
        (1.0, (-0.03_f64).exp()),
        (5.0, (-0.03 * 5.0_f64).exp()),
        (10.0, (-0.03 * 10.0_f64).exp()),
        (20.0, (-0.03 * 20.0_f64).exp()),
        (30.0, (-0.03 * 30.0_f64).exp()),
    ];

    let disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder(CurveId::new("USD-LIBOR-3M"), 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 0.035), (30.0, 0.035)])
        .build()
        .unwrap();

    // Flat 20% vol surface across strikes and expiries
    let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07];
    let expiries = vec![0.5, 1.0, 2.0, 5.0, 10.0, 20.0];
    let flat_row = vec![0.20_f64; strikes.len()];

    let mut vol_builder = VolSurface::builder(CurveId::new("USD-CMS10Y-VOL"))
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in 0..expiries.len() {
        vol_builder = vol_builder.row(&flat_row);
    }
    let vol_surface = vol_builder.build().unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_surface(vol_surface)
}

// ---------------------------------------------------------------------------
// CMS Option builder — n quarterly fixing periods
// ---------------------------------------------------------------------------

fn make_cms_option(as_of: Date, n_periods: usize, cms_tenor: f64) -> CmsOption {
    let mut fixing_dates = Vec::with_capacity(n_periods);
    let mut payment_dates = Vec::with_capacity(n_periods);
    let mut accrual_fractions = Vec::with_capacity(n_periods);

    for i in 1..=n_periods {
        let days_fix = (i as i64 * 91) - 2; // quarterly fixing, 2-day lag
        let days_pay = i as i64 * 91;
        let fix_date = as_of + time::Duration::days(days_fix);
        let pay_date = as_of + time::Duration::days(days_pay);
        fixing_dates.push(fix_date);
        payment_dates.push(pay_date);
        accrual_fractions.push(0.25_f64);
    }

    CmsOption {
        id: InstrumentId::new("CMS-BENCH"),
        option_type: OptionType::Call,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.03).unwrap(),
        fixing_dates,
        payment_dates,
        accrual_fractions,
        cms_tenor,
        swap_convention: None,
        swap_fixed_freq: Some(Tenor::semi_annual()),
        swap_float_freq: Some(Tenor::quarterly()),
        swap_day_count: Some(DayCount::Thirty360),
        swap_float_day_count: Some(DayCount::Act360),
        day_count: DayCount::Act360,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-LIBOR-3M"),
        vol_surface_id: CurveId::new("USD-CMS10Y-VOL"),
        pricing_overrides: PricingOverrides::default(),
        attributes: Default::default(),
    }
}

// ---------------------------------------------------------------------------
// Benchmark: CMS cap period count scaling (10Y CMS tenor)
// ---------------------------------------------------------------------------

fn bench_cms_option_period_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("cms_option_periods");
    let as_of = base_date();
    let market = create_market(as_of);

    for n in [4usize, 8, 20, 40] {
        let cap = make_cms_option(as_of, n, 10.0);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(&cap)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: CMS cap tenor scaling (20 quarterly periods)
// ---------------------------------------------------------------------------

fn bench_cms_option_cms_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("cms_option_cms_tenor");
    let as_of = base_date();
    let market = create_market(as_of);

    for (label, cms_tenor) in [("5Y", 5.0f64), ("10Y", 10.0), ("20Y", 20.0)] {
        let cap = make_cms_option(as_of, 20, cms_tenor);

        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&cap)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: CMS swap period count scaling
// ---------------------------------------------------------------------------

fn bench_cms_swap_period_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("cms_swap_periods");
    let as_of = base_date();
    let market = create_market(as_of);

    for n in [4usize, 8, 20, 40] {
        let end_days = n as i64 * 91;
        let start = as_of;
        let end = as_of + time::Duration::days(end_days);

        let swap = CmsSwap::from_schedule(
            "CMSSWAP-BENCH",
            start,
            end,
            Tenor::quarterly(),
            10.0,
            0.0,
            FundingLegSpec::Fixed {
                rate: 0.03,
                day_count: DayCount::Thirty360,
            },
            Money::new(10_000_000.0, Currency::USD),
            DayCount::Act360,
            IRSConvention::USDStandard,
            PayReceive::Receive,
            "USD-OIS",
            "USD-LIBOR-3M",
            "USD-CMS10Y-VOL",
        )
        .unwrap();

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(&swap)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_cms_option_period_count,
    bench_cms_option_cms_tenor,
    bench_cms_swap_period_count,
);
criterion_main!(benches);
