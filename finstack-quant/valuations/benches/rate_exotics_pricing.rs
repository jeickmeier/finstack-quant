//! Hull-White 1F and range-accrual exotic pricing benchmarks.
//!
//! Covers the rate-note engines that were previously unmeasured:
//! - [`RangeAccrual`]: default static-replication (digital call-spread) path
//! - [`CallableRangeAccrual`]: HW1F LSMC with Bermudan call dates
//! - [`Snowball`]: path-dependent HW1F Monte Carlo
//! - [`Tarn`]: target-redemption HW1F Monte Carlo
//!
//! Monte Carlo cases pin `mc_paths` to 2,500 so the target stays usable under
//! `mise run rust-bench` short sampling. Production defaults are 20,000 paths
//! for rate exotics.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::{
    CallableRangeAccrual, Instrument, InstrumentPricingOverrides, PricingOptions, RangeAccrual,
    Snowball, Tarn,
};
use std::hint::black_box;
use time::Month;

const MC_PATHS: u64 = 2_500;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

/// Discount plus projection forwards reconstructed from the same curve.
fn hw1f_market(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build()
        .unwrap();
    let fwd_6m = discount.to_forward_curve("USD-SOFR-6M", 0.5, None).unwrap();
    let fwd_3m = discount
        .to_forward_curve("USD-SOFR-3M", 0.25, None)
        .unwrap();
    MarketContext::new()
        .insert(discount)
        .insert(fwd_6m)
        .insert(fwd_3m)
        .insert_price("SOFR-RATE", MarketScalar::Unitless(0.03))
        .insert_price("USD-OIS_CAPFLOOR_HW1F_KAPPA", MarketScalar::Unitless(0.03))
        .insert_price("USD-OIS_CAPFLOOR_HW1F_SIGMA", MarketScalar::Unitless(0.01))
}

/// Equity-linked range-accrual market matching the instrument example IDs.
fn equity_range_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
        .build()
        .unwrap();
    let surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[80.0, 100.0, 120.0, 140.0])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .build()
        .unwrap();
    MarketContext::new()
        .insert(curve)
        .insert_surface(surface)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.02))
}

fn with_mc_paths<T>(mut inst: T, apply: impl FnOnce(&mut T, InstrumentPricingOverrides)) -> T {
    apply(
        &mut inst,
        InstrumentPricingOverrides::default().with_mc_paths(MC_PATHS as usize),
    );
    inst
}

fn bench_range_accrual_analytic(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_accrual_analytic");
    let as_of = date(2024, Month::January, 1);
    let market = equity_range_market(as_of);
    let inst = RangeAccrual::example();
    group.bench_function("12_monthly_obs", |b| {
        b.iter(|| {
            black_box(&inst)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });
    group.finish();
}

fn bench_callable_range_accrual_lsmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("callable_range_accrual_lsmc");
    group.throughput(Throughput::Elements(MC_PATHS));
    let as_of = date(2026, Month::January, 1);
    let market = hw1f_market(as_of);
    let inst = with_mc_paths(CallableRangeAccrual::example(), |inst, overrides| {
        inst.instrument_pricing_overrides = overrides;
    });
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        b.iter(|| {
            black_box(&inst)
                .price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[]),
                    PricingOptions::default(),
                )
                .unwrap()
                .value
                .amount()
        });
    });
    group.finish();
}

fn bench_snowball_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("snowball_hw1f_mc");
    group.throughput(Throughput::Elements(MC_PATHS));
    let as_of = date(2026, Month::January, 1);
    let market = hw1f_market(as_of);
    let inst = with_mc_paths(Snowball::example_snowball(), |inst, overrides| {
        inst.instrument_pricing_overrides = overrides;
    });
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        b.iter(|| {
            black_box(&inst)
                .price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[]),
                    PricingOptions::default(),
                )
                .unwrap()
                .value
                .amount()
        });
    });
    group.finish();
}

fn bench_tarn_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("tarn_hw1f_mc");
    group.throughput(Throughput::Elements(MC_PATHS));
    let as_of = date(2026, Month::January, 1);
    let market = hw1f_market(as_of);
    let inst = with_mc_paths(Tarn::example(), |inst, overrides| {
        inst.instrument_pricing_overrides = overrides;
    });
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        b.iter(|| {
            black_box(&inst)
                .price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[]),
                    PricingOptions::default(),
                )
                .unwrap()
                .value
                .amount()
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_range_accrual_analytic,
    bench_callable_range_accrual_lsmc,
    bench_snowball_mc,
    bench_tarn_mc,
);
criterion_main!(benches);
