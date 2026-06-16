//! CDS pricing benchmarks.
//!
//! Measures performance of CDS operations:
//! - Present value (protection and premium legs)
//! - CS01 calculation
//! - Par spread calculation
//! - Risky PV01
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, Seniority};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::Instrument;
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

fn create_cds(tenor_years: i32) -> CreditDefaultSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    test_utils::cds_buy_protection(
        format!("CDS-{}Y", tenor_years),
        Money::new(10_000_000.0, Currency::USD),
        100.0, // 100bp spread
        start,
        maturity,
        "USD-OIS",
        "ACME-HAZARD",
    )
    .unwrap()
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("ACME-HAZARD")
        .issuer("ACME")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .recovery_rate(0.40)
        .base_date(base)
        .knots([
            (0.0, 0.015), // 150bp hazard
            (1.0, 0.016),
            (3.0, 0.018),
            (5.0, 0.020),
            (10.0, 0.025),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

fn bench_cds_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_pv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let cds = create_cds(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| cds.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_cs01");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let cds = create_cds(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    cds.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Cs01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_cds_cs01_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_cs01_variants");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cds = create_cds(5);

    let cases = [
        ("parallel_rebootstrap", MetricId::Cs01),
        ("bucketed_rebootstrap", MetricId::BucketedCs01),
        ("parallel_hazard", MetricId::Cs01Hazard),
        ("bucketed_hazard", MetricId::BucketedCs01Hazard),
    ];

    for (name, metric) in cases {
        let metrics = [metric];
        group.bench_function(name, |b| {
            b.iter(|| {
                cds.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(metrics.as_slice()),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
            });
        });
    }

    group.finish();
}

fn bench_cds_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_par_spread");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let cds = create_cds(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    cds.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::ParSpread]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cds_pv,
    bench_cds_cs01,
    bench_cds_cs01_variants,
    bench_cds_par_spread
);
criterion_main!(benches);
