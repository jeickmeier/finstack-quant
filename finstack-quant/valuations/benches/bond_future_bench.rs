//! Bond future pricing benchmarks.
//!
//! Measures performance of bond future operations:
//! - Conversion factor calculation (target: <1ms)
//! - NPV calculation (target: <5ms)
//! - DV01 calculation (target: <50ms)
//! - Bucketed DV01 (target: <200ms)

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::contract_specs::BondFutureSpecs;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFuturePricer, DeliverableBond,
};
use finstack_quant_valuations::instruments::{ListedFutureTerms, Position};
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

fn check_money(value: &Money, currency: Currency) {
    assert_eq!(value.currency(), currency);
    assert!(value.amount().is_finite());
}

fn check_valuation(
    result: &finstack_quant_valuations::results::ValuationResult,
    currency: Currency,
    metrics: &[MetricId],
) {
    check_money(&result.value, currency);
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        let prefix = format!("{}::", metric.as_str());
        assert!(
            result.measures.contains_key(metric)
                || result
                    .measures
                    .keys()
                    .any(|key| key.as_str().starts_with(&prefix)),
            "missing requested metric {metric}"
        );
    }
}

/// Create a synthetic UST 10Y contract fixture with a specified CTD basket.
fn create_ust_10y_future() -> BondFuture {
    let expiry = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let delivery_start = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_delivery_date = Date::from_calendar_date(2025, Month::March, 31).unwrap();

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
        .terms(
            ListedFutureTerms::new(
                10.0,
                1_000.0,
                Currency::USD,
                125.50,
                expiry,
                last_delivery_date,
                Position::Long,
            )
            .expect("terms"),
        )
        .delivery_start(delivery_start)
        .contract_specs(BondFutureSpecs::ust_10y())
        .deliverable_basket(basket)
        .ctd_bond_id(InstrumentId::from("US912828XG33")) // CTD bond
        .ctd_bond(create_ctd_bond())
        .discount_curve_id(CurveId::from("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

/// Create a CTD bond for testing
fn create_ctd_bond() -> Bond {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2029, Month::January, 15).unwrap();

    Bond::fixed(
        InstrumentId::from("US912828XG33"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.035).expect("valid rate fixture"), // 3.5% coupon
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
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
    group.bench_function("ust_10y", |b| {
        let ctd_bond = create_ctd_bond();
        let delivery_month = Date::from_calendar_date(2025, Month::March, 1).unwrap();
        let specs = BondFutureSpecs::ust_10y();
        support::bench(
            b,
            || {
                BondFuturePricer::calculate_conversion_factor(
                    black_box(&ctd_bond),
                    black_box(&specs),
                    black_box(delivery_month),
                )
                .expect("CTD conversion factor")
            },
            |factor| {
                assert!(factor.is_finite() && *factor > 0.0);
                assert!((*factor * 10_000.0 - (*factor * 10_000.0).round()).abs() < 1e-8);
            },
        );
    });
    group.finish();
}

/// Benchmark model futures price calculation
fn bench_model_price(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_model_price");
    group.bench_function("ust_10y", |b| {
        let future = create_ust_10y_future();
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        support::bench(
            b,
            || {
                future
                    .fair_price(black_box(&market), black_box(as_of))
                    .expect("bond future fair price")
            },
            |price| {
                assert!(price.is_finite() && *price > 0.0);
                let value = future.value(&market, as_of).expect("instrument value");
                check_money(&value, Currency::USD);
                let expected = (*price - future.terms.entry_price)
                    * future.terms.contracts
                    * future.terms.multiplier;
                assert!((value.amount() - expected).abs() < 1e-8);
            },
        );
    });
    group.finish();
}

/// Benchmark NPV calculation
fn bench_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_npv");
    let contracts = 10;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}contracts", contracts)),
        &contracts,
        |b, _| {
            let mut future = create_ust_10y_future();
            future.terms.contracts = contracts as f64;
            let ctd_bond = future.ctd_bond.as_ref().expect("embedded CTD");
            let market = create_market();
            let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
            let cf = future.deliverable_basket[0].conversion_factor;
            support::bench(
                b,
                || {
                    BondFuturePricer::calculate_npv(
                        black_box(&future),
                        black_box(ctd_bond),
                        black_box(cf),
                        black_box(&market),
                        black_box(as_of),
                    )
                    .expect("direct bond future NPV")
                },
                |value| {
                    check_money(value, Currency::USD);
                    let instrument_value = future.value(&market, as_of).expect("instrument value");
                    assert!((value.amount() - instrument_value.amount()).abs() < 1e-8);
                },
            );
        },
    );
    group.finish();
}

/// Benchmark Instrument trait value() method
fn bench_instrument_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_instrument_value");
    group.bench_function("ust_10y", |b| {
        let future = create_ust_10y_future();
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        support::bench(
            b,
            || {
                future
                    .value(black_box(&market), black_box(as_of))
                    .expect("bond future instrument value")
            },
            |value| {
                check_money(value, Currency::USD);
                let price = future.fair_price(&market, as_of).expect("fair price");
                let expected = (price - future.terms.entry_price)
                    * future.terms.contracts
                    * future.terms.multiplier;
                assert!((value.amount() - expected).abs() < 1e-8);
            },
        );
    });
    group.finish();
}

/// Benchmark DV01 calculation via metrics registry
fn bench_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_dv01");
    group.bench_function("ust_10y", |b| {
        let future = create_ust_10y_future();
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let metrics = [MetricId::Dv01];
        support::bench(
            b,
            || {
                // Keep the original per-operation request allocation timed.
                let metrics = vec![MetricId::Dv01];
                future
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("bond future risk metrics")
            },
            |result| {
                check_valuation(result, Currency::USD, &metrics);
                let value = future.value(&market, as_of).expect("instrument value");
                assert!((result.value.amount() - value.amount()).abs() < 1e-8);
            },
        );
    });
    group.finish();
}

/// Benchmark bucketed DV01 calculation
fn bench_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_bucketed_dv01");
    group.bench_function("ust_10y", |b| {
        let future = create_ust_10y_future();
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let metrics = [MetricId::BucketedDv01];
        support::bench(
            b,
            || {
                // Keep the original per-operation request allocation timed.
                let metrics = vec![MetricId::BucketedDv01];
                future
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("bond future risk metrics")
            },
            |result| {
                check_valuation(result, Currency::USD, &metrics);
                let value = future.value(&market, as_of).expect("instrument value");
                assert!((result.value.amount() - value.amount()).abs() < 1e-8);

                let buckets = result.metric_series(&MetricId::BucketedDv01);
                assert!(
                    !buckets.is_empty(),
                    "bucketed DV01 must include curve/tenor entries"
                );
                assert!(buckets.iter().all(|(labels, value)| labels.len() == 2
                    && labels[0] == "USD-TREASURY"
                    && !labels[1].is_empty()
                    && value.is_finite()));
            },
        );
    });
    group.finish();
}

/// Benchmark full pricing with all metrics
fn bench_full_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_future_full_metrics");
    group.bench_function("ust_10y_all", |b| {
        let future = create_ust_10y_future();
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let metrics = [MetricId::Dv01, MetricId::BucketedDv01, MetricId::Theta];
        support::bench(
            b,
            || {
                future
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("bond future risk metrics")
            },
            |result| {
                check_valuation(result, Currency::USD, &metrics);
                let value = future.value(&market, as_of).expect("instrument value");
                assert!((result.value.amount() - value.amount()).abs() < 1e-8);

                let buckets = result.metric_series(&MetricId::BucketedDv01);
                assert!(
                    !buckets.is_empty(),
                    "bucketed DV01 must include curve/tenor entries"
                );
                assert!(buckets.iter().all(|(labels, value)| labels.len() == 2
                    && labels[0] == "USD-TREASURY"
                    && !labels[1].is_empty()
                    && value.is_finite()));
            },
        );
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
    bench_full_metrics
);
criterion_main!(benches);
