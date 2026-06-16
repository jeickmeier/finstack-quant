//! CDS Tranche pricing benchmarks.
//!
//! Measures performance of CDS tranche operations using Gaussian Copula:
//! - Present value (NPV) calculation
//! - CS01 (credit spread sensitivity)
//! - Correlation delta
//! - Jump-to-default
//! - Par spread calculation
//!
//! Tests across different tranches (equity, mezzanine, senior) and pool sizes.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranche, TrancheSide,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn create_tranche(attach_pct: f64, detach_pct: f64, tenor_years: i32) -> CDSTranche {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = base + time::Duration::days((tenor_years * 365) as i64);

    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        attach_pct,
        detach_pct,
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        500.0, // 500bp running coupon
    );

    let schedule_params = ScheduleParams::quarterly_act360();

    CDSTranche::new(
        format!("CDX_IG42_{}_{}_{}", attach_pct, detach_pct, tenor_years),
        &tranche_params,
        &schedule_params,
        "USD-OIS",
        "CDX.NA.IG.42",
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Discount curve
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.95),
            (3.0, 0.87),
            (5.0, 0.78),
            (7.0, 0.70),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Index hazard curve
    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base)
        .recovery_rate(0.40)
        .knots(vec![
            (1.0, 0.010),
            (3.0, 0.014),
            (5.0, 0.018),
            (7.0, 0.022),
            (10.0, 0.028),
        ])
        .par_spreads(vec![
            (1.0, 60.0),
            (3.0, 75.0),
            (5.0, 90.0),
            (7.0, 110.0),
            (10.0, 130.0),
        ])
        .build()
        .unwrap();

    // Base correlation curve
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),  // 0-3% equity
            (7.0, 0.45),  // 0-7% junior mezzanine
            (10.0, 0.60), // 0-10% senior mezzanine
            (15.0, 0.75), // 0-15% senior
            (30.0, 0.85), // 0-30% super senior
        ])
        .build()
        .unwrap();

    // Create credit index data
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data)
}

fn create_market_with_issuers(num_issuers: usize) -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.95),
            (3.0, 0.87),
            (5.0, 0.78),
            (7.0, 0.70),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base)
        .recovery_rate(0.40)
        .knots(vec![
            (1.0, 0.012),
            (3.0, 0.016),
            (5.0, 0.020),
            (7.0, 0.024),
            (10.0, 0.030),
        ])
        .par_spreads(vec![
            (1.0, 65.0),
            (3.0, 80.0),
            (5.0, 95.0),
            (7.0, 115.0),
            (10.0, 135.0),
        ])
        .build()
        .unwrap();

    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),
            (7.0, 0.45),
            (10.0, 0.60),
            (15.0, 0.75),
            (30.0, 0.85),
        ])
        .build()
        .unwrap();

    // Create issuer-specific curves with slight variations
    let mut issuer_curves = finstack_quant_core::HashMap::default();
    for i in 0..num_issuers {
        let id = format!("ISSUER-{:03}", i + 1);
        let bump = (i as f64 / num_issuers as f64) * 0.003; // Small heterogeneity
        let hz = HazardCurve::builder(id.as_str())
            .base_date(base)
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, (0.012 + bump).min(0.05)),
                (3.0, (0.016 + bump).min(0.05)),
                (5.0, (0.020 + bump).min(0.05)),
                (7.0, (0.024 + bump).min(0.05)),
                (10.0, (0.030 + bump).min(0.05)),
            ])
            .build()
            .unwrap();
        issuer_curves.insert(id, Arc::new(hz));
    }

    let index_data = CreditIndexData::builder()
        .num_constituents(num_issuers as u16)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .issuer_curves(issuer_curves)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data)
}

fn bench_cds_tranche_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_npv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test different tranche types
    let tranches = vec![
        ("equity_0_3", 0.0, 3.0),
        ("junior_mezz_3_7", 3.0, 7.0),
        ("senior_mezz_7_10", 7.0, 10.0),
        ("senior_10_15", 10.0, 15.0),
    ];

    for (name, attach, detach) in tranches {
        let tranche = create_tranche(attach, detach, 5);
        group.bench_with_input(BenchmarkId::from_parameter(name), name, |b, _| {
            b.iter(|| tranche.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_cds_tranche_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_cs01");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let registry = standard_registry();

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("cs01", |b| {
        b.iter(|| {
            // Use generic CS01 calculator via MetricContext
            let base_pv = tranche.value(black_box(&market), black_box(as_of)).unwrap();
            let mut context = MetricContext::new(
                Arc::new(tranche.clone()),
                Arc::new(market.clone()),
                as_of,
                base_pv,
                MetricContext::default_config(),
            );
            let results = registry.compute(&[MetricId::Cs01], &mut context).unwrap();
            black_box(*results.get(&MetricId::Cs01).unwrap())
        });
    });

    group.finish();
}

fn bench_cds_tranche_correlation_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_correlation_delta");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("correlation_delta", |b| {
        b.iter(|| tranche.correlation_delta(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

fn bench_cds_tranche_jump_to_default(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_jump_to_default");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("jump_to_default", |b| {
        b.iter(|| tranche.jump_to_default(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

fn bench_cds_tranche_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_par_spread");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("par_spread", |b| {
        b.iter(|| tranche.par_spread(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

fn bench_cds_tranche_all_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_all_metrics");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let registry = standard_registry();

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("all_metrics", |b| {
        b.iter(|| {
            let _npv = tranche.value(black_box(&market), black_box(as_of));

            // CS01 via generic calculator
            let _base_pv = *_npv.as_ref().unwrap();
            let mut context = MetricContext::new(
                Arc::new(tranche.clone()),
                Arc::new(market.clone()),
                as_of,
                _base_pv,
                MetricContext::default_config(),
            );
            let _ = registry.compute(&[MetricId::Cs01], &mut context);

            let _corr_delta = tranche.correlation_delta(black_box(&market), black_box(as_of));
            let _jtd = tranche.jump_to_default(black_box(&market), black_box(as_of));
            let _par = tranche.par_spread(black_box(&market), black_box(as_of));
        });
    });

    group.finish();
}

fn bench_cds_tranche_heterogeneous(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_heterogeneous");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Benchmark different pool sizes for heterogeneous portfolios
    let pool_sizes = vec![10, 25, 50, 125];

    for &pool_size in &pool_sizes {
        let market = create_market_with_issuers(pool_size);
        let tranche = create_tranche(3.0, 7.0, 5);

        group.bench_with_input(
            BenchmarkId::new("npv_hetero", pool_size),
            &pool_size,
            |b, _| {
                b.iter(|| tranche.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cds_tranche_npv,
    bench_cds_tranche_cs01,
    bench_cds_tranche_correlation_delta,
    bench_cds_tranche_jump_to_default,
    bench_cds_tranche_par_spread,
    bench_cds_tranche_all_metrics,
    bench_cds_tranche_heterogeneous
);
criterion_main!(benches);
