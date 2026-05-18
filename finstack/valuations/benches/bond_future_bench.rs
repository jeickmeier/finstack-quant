//! Bond future pricing benchmarks.
//!
//! Measures performance of bond future operations:
//! - Conversion factor calculation (target: <1ms)
//! - NPV calculation (target: <5ms)
//! - DV01 calculation (target: <50ms)
//! - Bucketed DV01 (target: <200ms)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFutureSpecs, DeliverableBond, Position,
};
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

/// Create a realistic UST 10Y bond future with deliverable basket
fn create_ust_10y_future() -> BondFuture {
    let expiry = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let delivery_start = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let delivery_end = Date::from_calendar_date(2025, Month::March, 31).unwrap();

    // Deliverable basket with 5 bonds
    let basket = vec![
        DeliverableBond {
            bond_id: InstrumentId::from("US912828XG33"),
            conversion_factor: 0.8234,
        },
        DeliverableBond {
            bond_id: InstrumentId::from("US912828XH16"),
            conversion_factor: 0.8567,
        },
        DeliverableBond {
            bond_id: InstrumentId::from("US912828XJ71"),
            conversion_factor: 0.7892,
        },
        DeliverableBond {
            bond_id: InstrumentId::from("US912828XK54"),
            conversion_factor: 0.8123,
        },
        DeliverableBond {
            bond_id: InstrumentId::from("US912828XL38"),
            conversion_factor: 0.7765,
        },
    ];

    BondFuture::builder()
        .id(InstrumentId::from("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD)) // 10 contracts
        .expiry(expiry)
        .delivery_start(delivery_start)
        .delivery_end(delivery_end)
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::ust_10y())
        .deliverable_basket(basket)
        .ctd_bond_id(InstrumentId::from("US912828XG33")) // CTD bond
        .discount_curve_id(CurveId::from("USD-TREASURY"))
        .attributes(Attributes::new())
        .build_validated()
        .unwrap()
}

/// Create a CTD bond for testing
fn create_ctd_bond() -> Bond {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2029, Month::January, 15).unwrap();

    Bond::fixed(
        InstrumentId::from("US912828XG33"),
        Money::new(1_000_000.0, Currency::USD),
        0.035, // 3.5% coupon
        issue,
        maturity,
        CurveId::from("USD-TREASURY"),
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

/// Create a realistic market with USD Treasury curve
fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // Realistic USD Treasury discount curve
    let disc = DiscountCurve::builder("USD-TREASURY")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9890),
            (0.5, 0.9780),
            (1.0, 0.9560),
            (2.0, 0.9140),
            (3.0, 0.8730),
            (5.0, 0.7950),
            (7.0, 0.7210),
            (10.0, 0.6230),
            (15.0, 0.4980),
            (20.0, 0.3920),
            (30.0, 0.2710),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc)
}

/// Benchmark conversion factor calculation
fn bench_conversion_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_conversion_factor");

    let ctd_bond = create_ctd_bond();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let specs = BondFutureSpecs::ust_10y();

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            finstack_valuations::instruments::fixed_income::bond_future::BondFuturePricer::calculate_conversion_factor(
                black_box(&ctd_bond),
                black_box(specs.standard_coupon),
                black_box(specs.standard_maturity_years),
                black_box(as_of),
            )
        });
    });

    group.finish();
}

/// Benchmark model futures price calculation
fn bench_model_price(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_model_price");

    let ctd_bond = create_ctd_bond();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let delivery_date = Date::from_calendar_date(2025, Month::March, 31).unwrap();
    let cf = 0.8234;

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            finstack_valuations::instruments::fixed_income::bond_future::BondFuturePricer::calculate_model_price(
                black_box(&ctd_bond),
                black_box(cf),
                black_box(&market),
                black_box(as_of),
                black_box(delivery_date),
            )
        });
    });

    group.finish();
}

/// Benchmark NPV calculation
fn bench_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_npv");

    let future = create_ust_10y_future();
    let ctd_bond = create_ctd_bond();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let cf = 0.8234; // Pre-calculated conversion factor

    // Test different position sizes
    for num_contracts in [1, 10, 100].iter() {
        let mut sized_future = future.clone();
        sized_future.notional = Money::new(*num_contracts as f64 * 100_000.0, Currency::USD);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}contracts", num_contracts)),
            num_contracts,
            |b, _| {
                b.iter(|| {
                    finstack_valuations::instruments::fixed_income::bond_future::BondFuturePricer::calculate_npv(
                        black_box(&sized_future),
                        black_box(&ctd_bond),
                        black_box(cf),
                        black_box(&market),
                        black_box(as_of),
                    )
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Instrument trait value() method
fn bench_instrument_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_instrument_value");

    let future = create_ust_10y_future();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            let _ = future.value(black_box(&market), black_box(as_of));
        });
    });

    group.finish();
}

/// Benchmark DV01 calculation via metrics registry
fn bench_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_dv01");
    group.sample_size(20); // Fewer samples due to longer runtime

    let future = create_ust_10y_future();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            let metrics = vec![MetricId::Dv01];
            let _ = future.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&metrics),
                finstack_valuations::instruments::PricingOptions::default(),
            );
        });
    });

    group.finish();
}

/// Benchmark bucketed DV01 calculation
fn bench_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_bucketed_dv01");
    group.sample_size(10); // Fewer samples due to much longer runtime

    let future = create_ust_10y_future();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            let metrics = vec![MetricId::BucketedDv01];
            let _ = future.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&metrics),
                finstack_valuations::instruments::PricingOptions::default(),
            );
        });
    });

    group.finish();
}

/// Benchmark invoice price calculation
fn bench_invoice_price(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_invoice_price");

    let future = create_ust_10y_future();
    let ctd_bond = create_ctd_bond();
    let market = create_market();
    let settlement = Date::from_calendar_date(2025, Month::March, 23).unwrap();

    group.bench_function("ust_10y", |b| {
        b.iter(|| {
            future.invoice_price(
                black_box(&ctd_bond),
                black_box(&market),
                black_box(settlement),
            )
        });
    });

    group.finish();
}

/// Benchmark full pricing with all metrics
fn bench_full_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_full_metrics");
    group.sample_size(10); // Fewer samples due to longest runtime

    let future = create_ust_10y_future();
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    let metrics = vec![MetricId::Dv01, MetricId::BucketedDv01, MetricId::Theta];

    group.bench_function("ust_10y_all", |b| {
        b.iter(|| {
            let _ = future.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&metrics),
                finstack_valuations::instruments::PricingOptions::default(),
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_conversion_factor,
    bench_model_price,
    bench_npv,
    bench_instrument_value,
    bench_dv01,
    bench_bucketed_dv01,
    bench_invoice_price,
    bench_full_metrics
);
criterion_main!(benches);
