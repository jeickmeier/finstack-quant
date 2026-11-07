//! Bucketed risk benchmarks for DV01 and CS01.
//!
//! Measures performance of bucketed risk calculations with varying bucket counts:
//! - Bucketed DV01 (interest rate risk) for bonds and swaps
//! - Bucketed CS01 (credit spread risk) for CDS
//! - Standard 11-point rate buckets and 11 calibrated credit-quote pillars
//!
//! Each bucket requires a separate curve bump and repricing, so performance
//! should scale linearly with the number of buckets.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/credit.rs"]
mod credit_support;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../rates/tests/support/rates.rs"]
mod rates_support;
mod support;
use std::hint::black_box;
use time::Month;

// Test Instrument Creation

fn create_bond(tenor_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    Bond::fixed(
        format!("BOND-{}Y", tenor_years),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"), // 5% coupon
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

fn create_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    rates_support::usd_irs_swap(
        InstrumentId::new(format!("IRS-{}Y", tenor_years)),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04, // 4% fixed rate
        start,
        end,
        PayReceive::Pay,
    )
    .unwrap()
}

fn create_cds(tenor_years: i32) -> CreditDefaultSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    credit_support::cds_buy_protection(
        format!("CDS-{}Y", tenor_years),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        200.0, // 200 bp spread
        start,
        maturity,
        "USD-OIS",
        "CORP-HAZARD",
    )
    .unwrap()
}

// Market Data Creation

fn create_ir_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9900),
            (0.5, 0.9800),
            (1.0, 0.9600),
            (2.0, 0.9200),
            (3.0, 0.8850),
            (5.0, 0.8200),
            (7.0, 0.7600),
            (10.0, 0.6800),
            (15.0, 0.5700),
            (20.0, 0.4800),
            (30.0, 0.3500),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.0350),
            (0.25, 0.0360),
            (0.5, 0.0370),
            (1.0, 0.0380),
            (2.0, 0.0400),
            (3.0, 0.0420),
            (5.0, 0.0450),
            (7.0, 0.0470),
            (10.0, 0.0500),
            (15.0, 0.0530),
            (20.0, 0.0550),
            (30.0, 0.0570),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd)
}

fn create_credit_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9900),
            (0.5, 0.9800),
            (1.0, 0.9600),
            (2.0, 0.9200),
            (3.0, 0.8850),
            (5.0, 0.8200),
            (7.0, 0.7600),
            (10.0, 0.6800),
            (15.0, 0.5700),
            (20.0, 0.4800),
            (30.0, 0.3500),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    // Replayable par-spread quotes are required for quote-space CS01.
    let source_market = MarketContext::new().insert(disc);
    let pillars =
        [91, 182, 365, 730, 1095, 1825, 2555, 3650, 5475, 7300, 10950].map(|days| (days, 200.0));
    let hazard = credit_support::calibrated_hazard_curve_with_pillars(
        &source_market,
        base,
        "CORP-HAZARD",
        "CORP",
        "USD-OIS",
        &pillars,
    )
    .expect("calibrated CDS benchmark hazard with replay metadata");

    source_market.insert(hazard)
}

fn validate_metrics(result: &ValuationResult, metrics: &[MetricId], curves: &[&str]) {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite());
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        assert!(result.measures.contains_key(metric), "missing {metric}");
        if metric == &MetricId::BucketedDv01 || metric == &MetricId::BucketedCs01 {
            let mut total = 0.0;
            for curve_id in curves {
                let prefix = format!("{metric}::{curve_id}::");
                let buckets: Vec<_> = result
                    .measures
                    .iter()
                    .filter(|(key, _)| key.as_str().starts_with(&prefix))
                    .collect();
                assert_eq!(buckets.len(), 11, "expected eleven {curve_id} buckets");
                assert!(buckets.iter().any(|(_, value)| **value != 0.0));
                total += buckets.iter().map(|(_, value)| **value).sum::<f64>();
            }
            let reported = result.measures[metric];
            assert!(
                (total - reported).abs() <= 1e-10 * reported.abs().max(1.0),
                "bucket sum does not match {metric}"
            );
        }
    }
}

// Bucketed DV01 Benchmarks

fn bench_bond_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_bucketed_dv01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // The standard buckets are: [0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20, 30] = 11 buckets
    // We can't easily control bucket count via the metric system, but we can
    // benchmark the standard bucketed DV01 calculation
    group.bench_function("10Y_bond_11_buckets", |b| {
        let market = create_ir_market();
        let bond = create_bond(10);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[MetricId::BucketedDv01]),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::BucketedDv01], &["USD-OIS"]),
        );
    });

    // For comparison: parallel DV01 (single bump)
    group.bench_function("10Y_bond_parallel", |b| {
        let market = create_ir_market();
        let bond = create_bond(10);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[MetricId::Dv01]),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::Dv01], &["USD-OIS"]),
        );
    });

    group.finish();
}

fn bench_swap_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_bucketed_dv01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test with different tenor swaps
    for tenor in [5, 10, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_bucketed", tenor)),
            tenor,
            |b, _| {
                let market = create_ir_market();
                let swap = create_swap(*tenor);
                support::bench(
                    b,
                    || {
                        swap.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::BucketedDv01]),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("swap requested risk metrics")
                    },
                    |result| {
                        validate_metrics(
                            result,
                            &[MetricId::BucketedDv01],
                            &["USD-OIS", "USD-SOFR-3M"],
                        )
                    },
                );
            },
        );

        // Parallel DV01 for comparison
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_parallel", tenor)),
            tenor,
            |b, _| {
                let market = create_ir_market();
                let swap = create_swap(*tenor);
                support::bench(
                    b,
                    || {
                        swap.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::Dv01]),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("swap requested risk metrics")
                    },
                    |result| {
                        validate_metrics(result, &[MetricId::Dv01], &["USD-OIS", "USD-SOFR-3M"])
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_bond_bucketed_dv01_by_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_bucketed_dv01_by_tenor");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Show how bucketed DV01 performance varies with instrument maturity
    for tenor in [2, 5, 10, 20, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_ir_market();
                let bond = create_bond(*tenor);
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::BucketedDv01]),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond requested risk metrics")
                    },
                    |result| validate_metrics(result, &[MetricId::BucketedDv01], &["USD-OIS"]),
                );
            },
        );
    }

    group.finish();
}

// Bucketed CS01 Benchmarks

fn bench_cds_bucketed_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_bucketed_cs01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Quote-space CS01 uses the eleven calibrated replay pillars.
    group.bench_function("10Y_cds_11_buckets", |b| {
        let market = create_credit_market();
        let cds = create_cds(10);
        support::bench(
            b,
            || {
                cds.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[MetricId::BucketedCs01]),
                    credit_support::pricing_options(),
                )
                .expect("cds requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::BucketedCs01], &["CORP-HAZARD"]),
        );
    });

    // Parallel CS01 for comparison
    group.bench_function("10Y_cds_parallel", |b| {
        let market = create_credit_market();
        let cds = create_cds(10);
        support::bench(
            b,
            || {
                cds.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&[MetricId::Cs01]),
                    credit_support::pricing_options(),
                )
                .expect("cds requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::Cs01], &["CORP-HAZARD"]),
        );
    });

    group.finish();
}

fn bench_cds_bucketed_cs01_by_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_bucketed_cs01_by_tenor");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Show how bucketed CS01 performance varies with CDS maturity
    for tenor in [2, 5, 10, 20, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_credit_market();
                let cds = create_cds(*tenor);
                support::bench(
                    b,
                    || {
                        cds.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::BucketedCs01]),
                            credit_support::pricing_options(),
                        )
                        .expect("cds requested risk metrics")
                    },
                    |result| validate_metrics(result, &[MetricId::BucketedCs01], &["CORP-HAZARD"]),
                );
            },
        );
    }

    group.finish();
}

// Combined Risk Calculations

fn bench_combined_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_risk_metrics");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Bond: parallel DV01 vs bucketed DV01 vs both

    group.bench_function("bond_parallel_only", |b| {
        let ir_market = create_ir_market();
        let bond = create_bond(10);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&ir_market),
                    black_box(as_of),
                    black_box(&[MetricId::Dv01]),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::Dv01], &["USD-OIS"]),
        );
    });

    group.bench_function("bond_bucketed_only", |b| {
        let ir_market = create_ir_market();
        let bond = create_bond(10);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&ir_market),
                    black_box(as_of),
                    black_box(&[MetricId::BucketedDv01]),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::BucketedDv01], &["USD-OIS"]),
        );
    });

    group.bench_function("bond_both", |b| {
        let ir_market = create_ir_market();
        let bond = create_bond(10);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&ir_market),
                    black_box(as_of),
                    black_box(&[MetricId::Dv01, MetricId::BucketedDv01]),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond requested risk metrics")
            },
            |result| {
                validate_metrics(
                    result,
                    &[MetricId::Dv01, MetricId::BucketedDv01],
                    &["USD-OIS"],
                )
            },
        );
    });

    // CDS: parallel CS01 vs bucketed CS01 vs both

    group.bench_function("cds_parallel_only", |b| {
        let credit_market = create_credit_market();
        let cds = create_cds(10);
        support::bench(
            b,
            || {
                cds.price_with_metrics(
                    black_box(&credit_market),
                    black_box(as_of),
                    black_box(&[MetricId::Cs01]),
                    credit_support::pricing_options(),
                )
                .expect("cds requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::Cs01], &["CORP-HAZARD"]),
        );
    });

    group.bench_function("cds_bucketed_only", |b| {
        let credit_market = create_credit_market();
        let cds = create_cds(10);
        support::bench(
            b,
            || {
                cds.price_with_metrics(
                    black_box(&credit_market),
                    black_box(as_of),
                    black_box(&[MetricId::BucketedCs01]),
                    credit_support::pricing_options(),
                )
                .expect("cds requested risk metrics")
            },
            |result| validate_metrics(result, &[MetricId::BucketedCs01], &["CORP-HAZARD"]),
        );
    });

    group.bench_function("cds_both", |b| {
        let credit_market = create_credit_market();
        let cds = create_cds(10);
        support::bench(
            b,
            || {
                cds.price_with_metrics(
                    black_box(&credit_market),
                    black_box(as_of),
                    black_box(&[MetricId::Cs01, MetricId::BucketedCs01]),
                    credit_support::pricing_options(),
                )
                .expect("cds requested risk metrics")
            },
            |result| {
                validate_metrics(
                    result,
                    &[MetricId::Cs01, MetricId::BucketedCs01],
                    &["CORP-HAZARD"],
                )
            },
        );
    });

    group.finish();
}

// Criterion Configuration

criterion_group!(
    benches,
    bench_bond_bucketed_dv01,
    bench_swap_bucketed_dv01,
    bench_bond_bucketed_dv01_by_tenor,
    bench_cds_bucketed_cs01,
    bench_cds_bucketed_cs01_by_tenor,
    bench_combined_metrics,
);
criterion_main!(benches);
