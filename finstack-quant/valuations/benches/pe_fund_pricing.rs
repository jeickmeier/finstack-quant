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
//! - Waterfall complexity: simple (RoC + residual LP split) vs. standard
//!   (RoC + pref + catch-up + promote) vs. clawback (RoC + promote + clawback)
//! - Style: European vs. American waterfall
//! - IRR iteration: direct `run_waterfall()` vs. full `value()` with NAV discounting

mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::pe_fund::{
    AllocationLedger, ClawbackSpec, FundEvent, FundEventKind, PeFundWaterfallSpec,
    PrivateMarketsFund, WaterfallStyle,
};
use std::hint::black_box;
use time::Month;
use time::macros::date;

// Market context

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

// Waterfall specs

fn simple_waterfall() -> PeFundWaterfallSpec {
    PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .promote_tier(0.0, 1.0, 0.0)
        .build()
        .unwrap()
}

fn standard_waterfall() -> PeFundWaterfallSpec {
    PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(0.5)
        .promote_tier(0.12, 0.80, 0.20)
        .build()
        .unwrap()
}

fn full_waterfall_with_clawback() -> PeFundWaterfallSpec {
    PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .promote_tier(0.0, 0.80, 0.20)
        .clawback(ClawbackSpec {
            holdback_decimal: Some(0.20),
        })
        .build()
        .unwrap()
}

fn american_waterfall() -> PeFundWaterfallSpec {
    PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(0.5)
        .promote_tier(0.12, 0.80, 0.20)
        .build()
        .unwrap()
}

// Event set builder

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
        let amount =
            Money::new(1_000_000.0 + i as f64 * 100_000.0, currency).expect("valid money fixture");
        events.push(FundEvent::contribution(date, amount));
    }

    // Distributions in years 4–10
    for i in 0..distribution_count {
        let days_offset = 365 * 4 + (i as i64 * 365 * 6) / distribution_count.max(1) as i64;
        let date = Date::from_calendar_date(2020, Month::January, 1).unwrap()
            + time::Duration::days(days_offset);
        let amount =
            Money::new(2_000_000.0 + i as f64 * 150_000.0, currency).expect("valid money fixture");
        events.push(FundEvent::distribution(date, amount));
    }

    events
}

/// Build a clawback workload with one aggregate contribution and many
/// distributions so the fund-end entitlement replay has a unique IRR root.
fn make_clawback_events(n: usize) -> Vec<FundEvent> {
    let mut events = Vec::with_capacity(n);
    events.push(FundEvent::contribution(
        Date::from_calendar_date(2020, Month::January, 1).unwrap(),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
    ));
    for i in 0..n.saturating_sub(1) {
        let date = Date::from_calendar_date(2024, Month::January, 1).unwrap()
            + time::Duration::days((i as i64 * 365 * 6) / n.saturating_sub(1).max(1) as i64);
        events.push(FundEvent::distribution(
            date,
            Money::new(500_000.0, Currency::USD).expect("valid money fixture"),
        ));
    }
    events
}

fn make_fund(
    events: Vec<FundEvent>,
    spec: PeFundWaterfallSpec,
    with_discount: bool,
) -> PrivateMarketsFund {
    let fund =
        PrivateMarketsFund::new("PMF-BENCH", Currency::USD, spec, events).expect("valid fund");
    if with_discount {
        fund.with_discount_curve_id("USD-OIS")
    } else {
        fund
    }
}

fn validate_money(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn validate_ledger(ledger: &AllocationLedger, fund: &PrivateMarketsFund) {
    assert!(!ledger.rows.is_empty());
    for row in &ledger.rows {
        for value in [row.to_lp, row.to_gp, row.lp_unreturned, row.gp_carry_cum] {
            validate_money(&value);
        }
        if let Some(irr) = row.lp_irr_to_date {
            assert!(irr.is_finite());
        }
        if fund.waterfall_spec.style == WaterfallStyle::American {
            assert_eq!(row.deal_id.as_deref(), Some("BENCH-DEAL"));
        }
    }

    let cashflows = ledger.lp_cashflows().expect("reconstructed LP cashflows");
    assert_eq!(cashflows.len(), fund.events.len());
    for (date, value) in &cashflows {
        validate_money(value);
        assert!(fund.events.iter().any(|event| event.date == *date));
    }
    for event in &fund.events {
        let (_, amount) = cashflows
            .iter()
            .find(|(date, _)| *date == event.date)
            .expect("every fund event contributes an LP cashflow");
        if event.kind == FundEventKind::Contribution {
            assert_eq!(amount.amount(), -event.amount.amount());
        } else {
            assert!(amount.amount() > 0.0);
            assert!(ledger.rows.iter().any(|row| row.date == event.date));
        }
    }
}

// Benchmark: event count scaling (run_waterfall only, no discounting)

fn bench_pe_waterfall_event_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_events");
    let n = 30;

    group.throughput(Throughput::Elements(n as u64));
    group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
        let fund = make_fund(make_events(n), standard_waterfall(), false);
        support::bench(
            b,
            || {
                black_box(&fund)
                    .run_waterfall()
                    .expect("PE event-count waterfall")
            },
            |ledger| validate_ledger(ledger, &fund),
        );
    });

    group.finish();
}

// Benchmark: waterfall complexity comparison (30 events)

fn bench_pe_waterfall_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_complexity");

    for label in ["simple", "standard", "full_clawback"] {
        group.bench_function(label, |b| {
            let (events, spec) = match label {
                "simple" => (make_events(30), simple_waterfall()),
                "standard" => (make_events(30), standard_waterfall()),
                _ => (make_clawback_events(30), full_waterfall_with_clawback()),
            };
            let fund = make_fund(events, spec, false);
            support::bench(
                b,
                || {
                    black_box(&fund)
                        .run_waterfall()
                        .expect("PE complexity waterfall")
                },
                |ledger| validate_ledger(ledger, &fund),
            );
        });
    }

    group.finish();
}

// Benchmark: European vs. American waterfall (30 events)

fn bench_pe_waterfall_style(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_waterfall_style");

    group.bench_function("european", |b| {
        let european = make_fund(make_events(30), standard_waterfall(), false);
        assert_eq!(european.waterfall_spec.style, WaterfallStyle::European);
        support::bench(
            b,
            || {
                black_box(&european)
                    .run_waterfall()
                    .expect("European PE waterfall")
            },
            |ledger| validate_ledger(ledger, &european),
        );
    });
    group.bench_function("american", |b| {
        let american_events = make_events(30)
            .into_iter()
            .map(|event| event.with_deal_id("BENCH-DEAL"))
            .collect();
        let american = make_fund(american_events, american_waterfall(), false);
        assert_eq!(american.waterfall_spec.style, WaterfallStyle::American);
        support::bench(
            b,
            || {
                black_box(&american)
                    .run_waterfall()
                    .expect("American PE waterfall")
            },
            |ledger| validate_ledger(ledger, &american),
        );
    });

    group.finish();
}

// Benchmark: full value() pipeline (waterfall + IRR + NAV discounting)

fn bench_pe_fund_full_pricing(c: &mut Criterion) {
    let mut group = c.benchmark_group("pe_fund_full_pricing");
    let n = 30;

    group.throughput(Throughput::Elements(n as u64));
    group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
        let market = create_market();
        let as_of = date!(2025 - 01 - 01);
        let fund = make_fund(make_events(n), standard_waterfall(), true);
        support::bench(
            b,
            || {
                black_box(&fund)
                    .value(black_box(&market), black_box(as_of))
                    .expect("full PE fund price")
            },
            validate_money,
        );
    });

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
