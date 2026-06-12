//! Range accrual pricing benchmarks.
//!
//! The range accrual static replication pricer evaluates a binary call spread
//! (2 vanilla option evaluations using vol surface lookups) for each observation
//! date. Cost = O(n_observations × vol_surface_lookup).
//!
//! For MC pricing: cost = O(n_paths × n_time_steps × n_observations).
//! MC is triggered automatically when `mc_seed_scenario` is set in PricingOverrides.
//!
//! Scenarios:
//! - Observation count scaling: 12 / 52 / 252 observations (analytic static replication)
//! - Analytic bounds type: absolute vs. relative-to-initial-spot
//! - MC throughput: 252 observations, 1K paths (with mc_seed_scenario)

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};
use finstack_valuations::instruments::rates::range_accrual::{BoundsType, RangeAccrual};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PricingOverrides;
use std::hint::black_box;
use time::Month;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_market(as_of: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
        .build()
        .unwrap();

    let surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .row(&[0.22, 0.20, 0.18, 0.17, 0.16])
        .row(&[0.22, 0.20, 0.18, 0.17, 0.16])
        .row(&[0.22, 0.20, 0.18, 0.17, 0.16])
        .row(&[0.22, 0.20, 0.18, 0.17, 0.16])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(surface)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(4700.0))
}

fn make_range_accrual(
    as_of: Date,
    n_obs: usize,
    bounds_type: BoundsType,
    mc_seed: Option<String>,
) -> RangeAccrual {
    let mut obs_dates = Vec::with_capacity(n_obs);
    for i in 1..=n_obs {
        let days = (i as i64 * 365) / n_obs.max(1) as i64;
        obs_dates.push(as_of + time::Duration::days(days));
    }

    // Use relative bounds [90%–110%] or absolute [4230–5170] for SPX at 4700
    let (lower, upper) = match bounds_type {
        BoundsType::Absolute => (4230.0, 5170.0),
        _ => (0.90, 1.10),
    };

    let mut overrides = PricingOverrides::default();
    if let Some(scenario) = mc_seed {
        overrides.metrics.mc_seed_scenario = Some(scenario);
    }

    RangeAccrual {
        id: InstrumentId::new("RANGE-BENCH"),
        underlying_ticker: "SPX".to_string(),
        observation_dates: obs_dates,
        lower_bound: lower,
        upper_bound: upper,
        bounds_type,
        coupon_rate: 0.08,
        notional: Money::new(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        discount_curve_id: CurveId::new("USD-OIS"),
        spot_id: PriceId::new("SPX-SPOT"),
        vol_surface_id: CurveId::new("SPX-VOL"),
        div_yield_id: None,
        pricing_overrides: overrides,
        attributes: Default::default(),
        quanto: None,
        payment_date: None,
        past_fixings_in_range: None,
        total_past_observations: None,
    }
}

// ---------------------------------------------------------------------------
// Benchmark: observation count scaling — analytic static replication
// ---------------------------------------------------------------------------

fn bench_range_accrual_analytic_obs_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_accrual_analytic");
    let as_of = base_date();
    let market = create_market(as_of);

    for n in [12usize, 52, 252] {
        let ra = make_range_accrual(as_of, n, BoundsType::RelativeToInitialSpot, None);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(&ra)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: absolute vs. relative bounds (52 weekly observations)
// ---------------------------------------------------------------------------

fn bench_range_accrual_bounds_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_accrual_bounds_type");
    let as_of = base_date();
    let market = create_market(as_of);

    let ra_relative = make_range_accrual(as_of, 52, BoundsType::RelativeToInitialSpot, None);
    let ra_absolute = make_range_accrual(as_of, 52, BoundsType::Absolute, None);

    group.bench_function("relative_to_initial_spot", |b| {
        b.iter(|| {
            black_box(&ra_relative)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });

    group.bench_function("absolute", |b| {
        b.iter(|| {
            black_box(&ra_absolute)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: MC vs. analytic at 52 observations
// ---------------------------------------------------------------------------

fn bench_range_accrual_analytic_vs_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_accrual_analytic_vs_mc");
    let as_of = base_date();
    let market = create_market(as_of);

    let ra_analytic = make_range_accrual(as_of, 52, BoundsType::RelativeToInitialSpot, None);
    // MC is triggered by setting mc_seed_scenario
    let ra_mc = make_range_accrual(
        as_of,
        52,
        BoundsType::RelativeToInitialSpot,
        Some("bench".to_string()),
    );

    group.bench_function("analytic", |b| {
        b.iter(|| {
            black_box(&ra_analytic)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });

    group.bench_function("mc_default_paths", |b| {
        b.iter(|| {
            black_box(&ra_mc)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_range_accrual_analytic_obs_count,
    bench_range_accrual_bounds_type,
    bench_range_accrual_analytic_vs_mc,
);
criterion_main!(benches);
