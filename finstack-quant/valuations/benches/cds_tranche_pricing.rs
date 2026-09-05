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
use finstack_quant_valuations::metrics::MetricId;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/credit.rs"]
mod credit_support;
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
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        maturity,
        500.0, // 500bp running coupon
    );

    let schedule_params = ScheduleParams::quarterly_act360();

    let mut tranche = CDSTranche::new(
        format!("CDX_IG42_{}_{}_{}", attach_pct, detach_pct, tenor_years),
        &tranche_params,
        &schedule_params,
        "USD-OIS",
        "CDX.NA.IG.42",
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");
    tranche.standard_imm_dates = true;
    tranche
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

    let source_market = MarketContext::new().insert(discount_curve);
    let index_curve = credit_support::calibrated_hazard_curve_with_pillars(
        &source_market,
        base,
        "CDX.NA.IG.42",
        "CDX.NA.IG.42",
        "USD-OIS",
        &[
            (365, 60.0),
            (3 * 365, 75.0),
            (5 * 365, 90.0),
            (7 * 365, 110.0),
            (10 * 365, 130.0),
        ],
    )
    .unwrap();

    // Base correlation curve
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),  // 0-3% equity
            (7.0, 0.25),  // 0-7% junior mezzanine
            (10.0, 0.25), // 0-10% senior mezzanine
            (15.0, 0.25), // 0-15% senior
            (30.0, 0.25), // 0-30% super senior
        ])
        .build()
        .unwrap();

    // Create credit index data
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve.clone()))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .unwrap();

    source_market
        .insert(index_curve)
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

    let source_market = MarketContext::new().insert(discount_curve);
    let index_curve = credit_support::calibrated_hazard_curve_with_pillars(
        &source_market,
        base,
        "CDX.NA.IG.42",
        "CDX.NA.IG.42",
        "USD-OIS",
        &[
            (365, 65.0),
            (3 * 365, 80.0),
            (5 * 365, 95.0),
            (7 * 365, 115.0),
            (10 * 365, 135.0),
        ],
    )
    .unwrap();

    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),
            (7.0, 0.25),
            (10.0, 0.25),
            (15.0, 0.25),
            (30.0, 0.25),
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
        .index_credit_curve(Arc::new(index_curve.clone()))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .issuer_curves(issuer_curves)
        .build()
        .unwrap();

    source_market
        .insert(index_curve)
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
    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("cs01", |b| {
        b.iter(|| {
            let result = tranche
                .price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    &[MetricId::Cs01],
                    credit_support::pricing_options(),
                )
                .unwrap();
            black_box(result)
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

    let tranche = create_tranche(3.0, 7.0, 5);

    group.bench_function("all_metrics", |b| {
        b.iter(|| {
            let _cs01 = tranche
                .price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    &[MetricId::Cs01],
                    credit_support::pricing_options(),
                )
                .unwrap();

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

    let pool_sizes = vec![50];

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
