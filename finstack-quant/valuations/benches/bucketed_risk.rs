//! Bucketed risk benchmarks for DV01 and CS01.
//!
//! Measures performance of bucketed risk calculations with varying bucket counts:
//! - Bucketed DV01 (interest rate risk) for bonds and swaps
//! - Bucketed CS01 (credit spread risk) for CDS
//! - Performance scaling with bucket counts: 5, 11, 21 buckets
//!
//! Each bucket requires a separate curve bump and repricing, so performance
//! should scale linearly with the number of buckets.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, HazardCurve, Seniority,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
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

// ============================================================================
// Test Instrument Creation
// ============================================================================

fn create_bond(tenor_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    Bond::fixed(
        format!("BOND-{}Y", tenor_years),
        Money::new(1_000_000.0, Currency::USD),
        0.05, // 5% coupon
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

fn create_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    test_utils::usd_irs_swap(
        InstrumentId::new(format!("IRS-{}Y", tenor_years)),
        Money::new(10_000_000.0, Currency::USD),
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

    test_utils::cds_buy_protection(
        format!("CDS-{}Y", tenor_years),
        Money::new(10_000_000.0, Currency::USD),
        200.0, // 200 bps spread
        start,
        maturity,
        "USD-OIS",
        "CORP-HAZARD",
    )
    .unwrap()
}

// ============================================================================
// Market Data Creation
// ============================================================================

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

    // Hazard rate curve (200 bps flat)
    let hazard = HazardCurve::builder("CORP-HAZARD")
        .issuer("CORP")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .recovery_rate(0.40)
        .base_date(base)
        .knots([
            (0.0, 0.020),
            (0.25, 0.020),
            (0.5, 0.020),
            (1.0, 0.020),
            (2.0, 0.020),
            (3.0, 0.020),
            (5.0, 0.020),
            (7.0, 0.020),
            (10.0, 0.020),
            (15.0, 0.020),
            (20.0, 0.020),
            (30.0, 0.020),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

// ============================================================================
// Bucketed DV01 Benchmarks
// ============================================================================

fn bench_bond_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_bucketed_dv01");
    let market = create_ir_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test with 10Y bond across different bucket counts
    let bond = create_bond(10);

    // The standard buckets are: [0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20, 30] = 11 buckets
    // We can't easily control bucket count via the metric system, but we can
    // benchmark the standard bucketed DV01 calculation
    group.bench_function("10Y_bond_11_buckets", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&[MetricId::BucketedDv01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    // For comparison: parallel DV01 (single bump)
    group.bench_function("10Y_bond_parallel", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&[MetricId::Dv01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.finish();
}

fn bench_swap_bucketed_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_bucketed_dv01");
    let market = create_ir_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test with different tenor swaps
    for tenor in [5, 10, 30].iter() {
        let swap = create_swap(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_bucketed", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::BucketedDv01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );

        // Parallel DV01 for comparison
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_parallel", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Dv01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_bond_bucketed_dv01_by_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_bucketed_dv01_by_tenor");
    let market = create_ir_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Show how bucketed DV01 performance varies with instrument maturity
    for tenor in [2, 5, 10, 20, 30].iter() {
        let bond = create_bond(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::BucketedDv01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Bucketed CS01 Benchmarks
// ============================================================================

fn bench_cds_bucketed_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_bucketed_cs01");
    let market = create_credit_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test with 10Y CDS
    let cds = create_cds(10);

    // Standard bucketed CS01 (11 buckets by default)
    group.bench_function("10Y_cds_11_buckets", |b| {
        b.iter(|| {
            cds.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&[MetricId::BucketedCs01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    // Parallel CS01 for comparison
    group.bench_function("10Y_cds_parallel", |b| {
        b.iter(|| {
            cds.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&[MetricId::Cs01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.finish();
}

fn bench_cds_bucketed_cs01_by_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_bucketed_cs01_by_tenor");
    let market = create_credit_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Show how bucketed CS01 performance varies with CDS maturity
    for tenor in [2, 5, 10, 20, 30].iter() {
        let cds = create_cds(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    cds.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::BucketedCs01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Combined Risk Calculations
// ============================================================================

fn bench_combined_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_risk_metrics");
    let ir_market = create_ir_market();
    let credit_market = create_credit_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Bond: parallel DV01 vs bucketed DV01 vs both
    let bond = create_bond(10);

    group.bench_function("bond_parallel_only", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&ir_market),
                black_box(as_of),
                black_box(&[MetricId::Dv01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.bench_function("bond_bucketed_only", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&ir_market),
                black_box(as_of),
                black_box(&[MetricId::BucketedDv01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.bench_function("bond_both", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&ir_market),
                black_box(as_of),
                black_box(&[MetricId::Dv01, MetricId::BucketedDv01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    // CDS: parallel CS01 vs bucketed CS01 vs both
    let cds = create_cds(10);

    group.bench_function("cds_parallel_only", |b| {
        b.iter(|| {
            cds.price_with_metrics(
                black_box(&credit_market),
                black_box(as_of),
                black_box(&[MetricId::Cs01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.bench_function("cds_bucketed_only", |b| {
        b.iter(|| {
            cds.price_with_metrics(
                black_box(&credit_market),
                black_box(as_of),
                black_box(&[MetricId::BucketedCs01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.bench_function("cds_both", |b| {
        b.iter(|| {
            cds.price_with_metrics(
                black_box(&credit_market),
                black_box(as_of),
                black_box(&[MetricId::Cs01, MetricId::BucketedCs01]),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

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
