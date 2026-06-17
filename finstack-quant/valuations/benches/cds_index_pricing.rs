//! CDS Index pricing benchmarks.
//!
//! Measures performance of CDS index operations:
//! - NPV calculation (single curve mode)
//! - NPV calculation (constituents mode)
//! - Par spread calculation
//! - CS01 (credit spread 01)
//! - Risky PV01
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams, IndexPricing,
};
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

fn create_cds_index_single_curve(tenor_years: i32) -> CDSIndex {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let preset = CDSIndexParams::cdx_na_ig(42, 1, 100.0);

    CDSIndex::from_preset(
        &preset,
        format!("CDX-{}Y", tenor_years),
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        0.40,
        "USD-OIS",
        "CDX-HAZARD",
    )
    .expect("valid benchmark parameters")
}

fn create_cds_index_constituents(tenor_years: i32, num_names: usize) -> CDSIndex {
    let mut index = create_cds_index_single_curve(tenor_years);

    let weight = 1.0 / (num_names as f64);
    let mut constituents = Vec::with_capacity(num_names);

    for i in 0..num_names {
        constituents.push(CDSIndexConstituent {
            credit: CreditParams {
                reference_entity: format!("ISSUER-{:03}", i + 1),
                credit_curve_id: format!("HAZARD-{:03}", i + 1).into(),
                recovery_rate: 0.40,
            },
            weight,
            defaulted: false,
        });
    }

    index.constituents = constituents;
    index.pricing = IndexPricing::Constituents;
    index
}

fn create_market_single_curve() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CDX-HAZARD")
        .base_date(base)
        .knots([
            (0.0, 0.0),
            (1.0, 0.02),
            (3.0, 0.025),
            (5.0, 0.03),
            (10.0, 0.035),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

fn create_market_constituents(num_names: usize) -> MarketContext {
    let mut market = create_market_single_curve();
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Add hazard curves for each constituent
    for i in 0..num_names {
        // Vary the hazard rates slightly for each name
        let scale = 1.0 + ((i as f64) / (num_names as f64) - 0.5) * 0.2;
        let hazard = HazardCurve::builder(format!("HAZARD-{:03}", i + 1))
            .base_date(base)
            .knots([
                (0.0, 0.0),
                (1.0, 0.02 * scale),
                (3.0, 0.025 * scale),
                (5.0, 0.03 * scale),
                (10.0, 0.035 * scale),
            ])
            .build()
            .unwrap();
        market = market.insert(hazard);
    }

    market
}

fn bench_cds_index_pv_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_pv_single");
    let market = create_market_single_curve();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let index = create_cds_index_single_curve(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| index.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_index_pv_constituents(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_pv_constituents");
    let tenor = 5;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    {
        let num_names = &50;
        let index = create_cds_index_constituents(tenor, *num_names);
        let market = create_market_constituents(*num_names);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}names", num_names)),
            num_names,
            |b, _| {
                b.iter(|| index.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_index_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_par_spread");
    let market = create_market_single_curve();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let index = create_cds_index_single_curve(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| index.par_spread(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_index_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_cs01");
    let market = create_market_single_curve();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let index = create_cds_index_single_curve(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| index.cs01(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_index_risky_pv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_risky_pv01");
    let market = create_market_single_curve();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        let index = create_cds_index_single_curve(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| index.risky_pv01(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_index_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_metrics");
    let market = create_market_single_curve();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let metrics = vec![MetricId::ParSpread, MetricId::Cs01, MetricId::RiskyPv01];

    for tenor in [3, 5, 10].iter() {
        let index = create_cds_index_single_curve(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    index.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
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
    bench_cds_index_pv_single,
    bench_cds_index_pv_constituents,
    bench_cds_index_par_spread,
    bench_cds_index_cs01,
    bench_cds_index_risky_pv01,
    bench_cds_index_metrics
);
criterion_main!(benches);
