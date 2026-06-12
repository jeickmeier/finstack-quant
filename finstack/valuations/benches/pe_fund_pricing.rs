//! Private equity fund waterfall and IRR benchmarks.
//!
//! The PE fund pricer is CPU-heavy because:
//! 1. `run_waterfall()` iterates every event against every waterfall tranche,
//!    computing time-weighted IRR tests and accruals.
//! 2. `calculate_irr()` runs Brent root-finding on an NPV function that iterates
//!    over all cashflows per trial rate.
//!
//! These are the core hot paths not covered by any existing bench.
//!
//! Scenarios:
//! - Event count scaling (10 / 30 / 60 / 100 events) — waterfall + IRR cost
//! - Waterfall complexity: simple (RoC only) vs. standard (RoC + pref + catchup + promote)
//!   vs. full (same + clawback)
//! - Style: European vs. American waterfall
//! - IRR iteration: direct `run_waterfall()` vs. full `value()` with NAV discounting

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::equity::pe_fund::{
    ClawbackSettle, ClawbackSpec, FundEvent, PrivateMarketsFund, WaterfallSpec, WaterfallStyle,
};
use finstack_valuations::instruments::Instrument;
use std::hint::black_box;
use time::macros::date;
use time::{Date, Month};

// ---------------------------------------------------------------------------
// Market context
// ---------------------------------------------------------------------------

fn create_market() -> MarketContext {
    let as_of = date!(2025 - 01 - 01);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, 0.916), (5.0, 0.790), (10.0, 0.608)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

// ---------------------------------------------------------------------------
// Waterfall specs
// ---------------------------------------------------------------------------

fn simple_waterfall() -> WaterfallSpec {
    WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .build()
        .unwrap()
}

fn standard_waterfall() -> WaterfallSpec {
    WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catchup(0.5)
        .promote_tier(0.12, 0.80, 0.20)
        .build()
        .unwrap()
}

fn full_waterfall_with_clawback() -> WaterfallSpec {
    WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catchup(0.5)
        .promote_tier(0.12, 0.80, 0.20)
        .clawback(ClawbackSpec {
            enable: true,
            holdback_pct: Some(0.20),
            settle_on: ClawbackSettle::FundEnd,
        })
        .build()
        .unwrap()
}

fn american_waterfall() -> WaterfallSpec {
    WaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .preferred_irr(0.08)
        .catchup(0.5)
        .promote_tier(0.12, 0.80, 0.20)
        .build()
        .unwrap()
}

// ---------------------------------------------------------------------------
// Event set builder
// ---------------------------------------------------------------------------

/// Build n fund events evenly spread across a 10-year fund life.
/// Roughly half are contributions (early), half are distributions (late).
fn make_events(n: usize) -> Vec<FundEvent> {
    let currency = Currency::USD;
    let contribution_count = n / 2;
    let distribution_count = n - contribution_count;

    let mut events = Vec::with_capacity(n);

    // Contributions in years 0–3
    for i in 0..contribution_count {
        let days_offset = (i as i64 * 365 * 3) / contribution_count.max(1) as i64;
        let date = Date::from_calendar_date(2020, Month::January, 1).unwrap()
            + time::Duration::days(days_offset);
        let amount = Money::new(1_000_000.0 + i as f64 * 100_000.0, currency);
        events.push(FundEvent::contribution(date, amount));
    }

    // Distributions in years 4–10
    for i in 0..distribution_count {
        let days_offset = 365 * 4 + (i as i64 * 365 * 6) / distribution_count.max(1) as i64;
        let date = Date::from_calendar_date(2020, Month::January, 1).unwrap()
            + time::Duration::days(days_offset);
        let amount = Money::new(2_000_000.0 + i as f64 * 150_000.0, currency);
        events.push(FundEvent::distribution(date, amount));
    }

    events
}

fn make_fund(
    events: Vec<FundEvent>,
    spec: WaterfallSpec,
    with_discount: bool,
) -> PrivateMarketsFund {
    let fund = PrivateMarketsFund::new("PMF-BENCH", Currency::USD, spec, events);
    if with_discount {
        fund.with_discount_curve("USD-OIS")
    } else {
        fund
    }
}

// ---------------------------------------------------------------------------
// Benchmark: event count scaling (run_waterfall only, no discounting)
// ---------------------------------------------------------------------------

fn bench_pe_waterfall_event_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_events");
    let spec = standard_waterfall();

    for n in [10usize, 30, 60, 100] {
        let fund = make_fund(make_events(n), spec.clone(), false);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(&fund).run_waterfall().unwrap());
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: waterfall complexity comparison (30 events)
// ---------------------------------------------------------------------------

fn bench_pe_waterfall_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_complexity");
    let events = make_events(30);

    let simple = make_fund(events.clone(), simple_waterfall(), false);
    let standard = make_fund(events.clone(), standard_waterfall(), false);
    let full = make_fund(events, full_waterfall_with_clawback(), false);

    for (label, fund) in [
        ("simple", &simple),
        ("standard", &standard),
        ("full_clawback", &full),
    ] {
        group.bench_function(label, |b| {
            b.iter(|| black_box(fund).run_waterfall().unwrap());
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: European vs. American waterfall (30 events)
// ---------------------------------------------------------------------------

fn bench_pe_waterfall_style(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_style");
    let events = make_events(30);

    let european = make_fund(events.clone(), standard_waterfall(), false);
    let american = make_fund(events, american_waterfall(), false);

    group.bench_function("european", |b| {
        b.iter(|| black_box(&european).run_waterfall().unwrap());
    });
    group.bench_function("american", |b| {
        b.iter(|| black_box(&american).run_waterfall().unwrap());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: full value() pipeline (waterfall + IRR + NAV discounting)
// ---------------------------------------------------------------------------

fn bench_pe_fund_full_pricing(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_full_pricing");
    let market = create_market();
    let as_of = date!(2025 - 01 - 01);
    let spec = standard_waterfall();

    for n in [10usize, 30, 60] {
        let fund = make_fund(make_events(n), spec.clone(), true);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(&fund)
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
    bench_pe_waterfall_event_count,
    bench_pe_waterfall_complexity,
    bench_pe_waterfall_style,
    bench_pe_fund_full_pricing,
);
criterion_main!(benches);
