//! Calibrated credit CS01 benchmarks (CDS, CDS Index, CDS Tranche).
//!
//! These cases exercise par-spread hazard recalibration via
//! `CachedRecalibrationProvider` and therefore live in
//! `finstack-quant-calibration`, which owns the calibration engine and depends
//! on `finstack-quant-valuations-credit`.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::correlation::CopulaSpec;
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CdsTradeParams, CreditDefaultSwap,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CdsIndex, CdsIndexParams, IndexPricing,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CdsTranche, CdsTrancheParams, CdsTranchePricer,
};
use finstack_quant_valuations::instruments::{CdsDateRegime, Instrument, PayReceive};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use serde_json::{Value, json};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../../valuations/tests/support/credit.rs"]
mod credit_support;
#[path = "../../valuations/benches/support/record.rs"]
mod record;
#[path = "../../valuations/benches/support/mod.rs"]
mod support;

// Single-name CDS CS01

fn create_cds(tenor_years: i32) -> CreditDefaultSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    credit_support::cds_buy_protection(
        format!("CDS-{}Y", tenor_years),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        100.0,
        start,
        maturity,
        "USD-OIS",
        "ACME-HAZARD",
    )
    .unwrap()
}

fn create_cds_market() -> MarketContext {
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let source_market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");
    let hazard = credit_support::calibrated_hazard_curve_with_pillars(
        &source_market,
        base,
        "ACME-HAZARD",
        "ACME",
        "USD-OIS",
        &[
            (365, 100.0),
            (3 * 365, 110.0),
            (5 * 365, 120.0),
            (10 * 365, 150.0),
        ],
    )
    .unwrap();

    source_market
        .try_insert(hazard)
        .expect("insert market curve")
}

fn validate_cds_metrics(result: &ValuationResult, metrics: &[MetricId]) {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite());
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        assert!(result.measures.contains_key(metric), "missing {metric}");
        if metric == &MetricId::BucketedCs01 {
            let prefix = "bucketed_cs01::ACME-HAZARD::";
            assert_eq!(
                result
                    .measures
                    .keys()
                    .filter(|key| key.as_str().starts_with(prefix))
                    .count(),
                4
            );
        }
    }
}

fn bench_cds_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_cs01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_cds_market();
                let cds = create_cds(*tenor);
                support::bench(
                    b,
                    || {
                        cds.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::Cs01]),
                            credit_support::pricing_options(),
                        )
                        .expect("CDS CS01")
                    },
                    |result| validate_cds_metrics(result, &[MetricId::Cs01]),
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_cs01_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_cs01_metrics");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let cases = [
        ("parallel", MetricId::Cs01),
        ("bucketed", MetricId::BucketedCs01),
    ];

    for (name, metric) in cases {
        let metrics = [metric];
        group.bench_function(name, |b| {
            let market = create_cds_market();
            let cds = create_cds(5);
            support::bench(
                b,
                || {
                    cds.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(metrics.as_slice()),
                        credit_support::pricing_options(),
                    )
                    .expect("CDS requested spread risk")
                },
                |result| validate_cds_metrics(result, &metrics),
            );
        });
    }
    group.finish();
}

// CDS Index CS01

fn create_cds_index_single_curve(tenor_years: i32) -> CdsIndex {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();
    let preset = CdsIndexParams::cdx_na_ig(42, 1, 100.0);

    CdsIndex::from_preset(
        &preset,
        CdsTradeParams {
            id: format!("CDX-{}Y", tenor_years).into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            side: PayReceive::Pay,
            include_valuation_date_cashflows: false,
            start,
            end,
            recovery_rate: 0.40,
            discount_curve_id: CurveId::new("USD-OIS"),
            credit_curve_id: CurveId::new("CDX-HAZARD"),
        },
    )
    .expect("valid benchmark parameters")
}

fn create_index_market_single_curve() -> MarketContext {
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let source_market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");
    let hazard = credit_support::calibrated_hazard_curve_with_pillars(
        &source_market,
        base,
        "CDX-HAZARD",
        "CDX",
        "USD-OIS",
        &[
            (365, 120.0),
            (3 * 365, 150.0),
            (5 * 365, 180.0),
            (10 * 365, 210.0),
        ],
    )
    .expect("calibrated index hazard with replay metadata");

    source_market
        .try_insert(hazard)
        .expect("insert market curve")
}

fn bench_cds_index_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_cs01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let index = create_cds_index_single_curve(*tenor);
                let market = create_index_market_single_curve();
                assert_eq!(index.pricing, IndexPricing::SingleCurve);
                let recalibration_provider = CachedRecalibrationProvider::default();
                support::bench(
                    b,
                    || {
                        index
                            .cs01(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&recalibration_provider),
                            )
                            .expect("CDS index risk")
                    },
                    |value| assert!(value.is_finite()),
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_index_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_metrics");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let metrics = vec![MetricId::ParSpread, MetricId::Cs01, MetricId::RiskyPv01];

    for tenor in [3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let index = create_cds_index_single_curve(*tenor);
                let market = create_index_market_single_curve();
                assert_eq!(index.pricing, IndexPricing::SingleCurve);
                support::bench(
                    b,
                    || {
                        index
                            .price_with_metrics(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&metrics),
                                credit_support::pricing_options(),
                            )
                            .expect("CDS index risk")
                    },
                    |result| {
                        assert_eq!(result.value.currency(), Currency::USD);
                        assert!(result.value.amount().is_finite());
                        assert!(result.measures.values().all(|value| value.is_finite()));
                        for metric in &metrics {
                            assert!(result.measures.contains_key(metric), "missing {metric}");
                        }
                    },
                );
            },
        );
    }
    group.finish();
}

// CDS Tranche CS01

fn create_tranche(attach_pct: f64, detach_pct: f64, tenor_years: i32) -> CdsTranche {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = base + time::Duration::days((tenor_years * 365) as i64);

    let tranche_params = CdsTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        attach_pct,
        detach_pct,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        maturity,
        500.0,
    );

    let schedule_params = ScheduleParams::quarterly_act360();
    let mut tranche = CdsTranche::new(
        format!("CDX_IG42_{}_{}_{}", attach_pct, detach_pct, tenor_years),
        &tranche_params,
        &schedule_params,
        CdsDateRegime::Bespoke,
        "USD-OIS",
        "CDX.NA.IG.42",
        PayReceive::Receive,
    )
    .expect("Valid tranche parameters");
    tranche.date_regime = CdsDateRegime::Standard;
    tranche.stub = finstack_quant_core::dates::StubKind::ShortBack;
    tranche
}

fn create_tranche_market() -> MarketContext {
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let source_market = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve");
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

    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve.clone()))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .unwrap();

    source_market
        .try_insert(index_curve)
        .expect("insert market curve")
        .try_insert(Arc::clone(index_data.base_correlation_curve()))
        .expect("insert market curve")
        .insert_credit_index("CDX.NA.IG.42", index_data)
        .expect("Credit index dependencies should be registered")
}

fn validate_tranche_cs01(result: &ValuationResult) {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite());
    assert!(result.measures.values().all(|value| value.is_finite()));
    assert!(result.measures.contains_key(&MetricId::Cs01));
}

fn capture_tranche_case(
    case_id: &str,
    entry_point: &'static str,
    requested_metrics: &[&str],
    tranche: &CdsTranche,
    market: &MarketContext,
    as_of: Date,
    copula: &CopulaSpec,
    output: impl FnOnce() -> Value,
) {
    record::capture(case_id, || {
        let index = market
            .get_credit_index(&tranche.credit_index_id)
            .expect("captured credit index");
        json!({
            "fixture": {
                "instrument": tranche,
                "market": market,
                "as_of": as_of,
                "model": {
                    "registry_model": tranche.default_model(),
                    "copula": copula,
                    "entry_point": entry_point,
                },
                "requested_metrics": requested_metrics,
                "dimensions": {
                    "registered_names": index.num_constituents(),
                    "index_recovery": index.recovery_rate(),
                    "attach_pct": tranche.attach_pct,
                    "detach_pct": tranche.detach_pct,
                    "maturity": tranche.maturity,
                    "notional": tranche.notional,
                },
            },
            "output": output(),
        })
    });
}

fn bench_cds_tranche_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_cs01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    group.bench_function("cs01", |b| {
        let market = create_tranche_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        support::bench(
            b,
            || {
                tranche
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        &[MetricId::Cs01],
                        credit_support::pricing_options(),
                    )
                    .expect("CDS tranche CS01")
            },
            |result| {
                validate_tranche_cs01(result);
                capture_tranche_case(
                    "cds_tranche_cs01/cs01",
                    "Instrument::price_with_metrics",
                    &["cs01"],
                    &tranche,
                    &market,
                    as_of,
                    &CopulaSpec::Gaussian,
                    || json!({"value": result.value, "metrics": result.measures}),
                );
            },
        );
    });
    group.finish();
}

fn bench_cds_tranche_all_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_all_metrics");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    group.bench_function("all_metrics", |b| {
        let market = create_tranche_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        let pricer = CdsTranchePricer::new();
        support::bench(
            b,
            || {
                let cs01 = tranche
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        &[MetricId::Cs01],
                        credit_support::pricing_options(),
                    )
                    .expect("CDS tranche CS01");
                let corr_delta = pricer
                    .calculate_correlation_delta(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche correlation delta");
                let jtd = pricer
                    .calculate_jump_to_default(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche jump to default");
                let par = pricer
                    .calculate_par_spread(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche par spread");
                (cs01, [corr_delta, jtd, par])
            },
            |(cs01, values)| {
                validate_tranche_cs01(cs01);
                assert!(values.iter().all(|value| value.is_finite()));
                capture_tranche_case(
                    "cds_tranche_all_metrics/all_metrics",
                    "price_with_metrics + calculate_correlation_delta + calculate_jump_to_default + calculate_par_spread",
                    &["cs01", "correlation_delta", "jump_to_default", "par_spread"],
                    &tranche,
                    &market,
                    as_of,
                    pricer.get_copula_spec(),
                    || {
                        json!({
                            "value": cs01.value,
                            "metrics": {
                                "cs01_result": cs01.measures,
                                "correlation_delta": values[0],
                                "jump_to_default": values[1],
                                "par_spread": values[2],
                            },
                        })
                    },
                );
            },
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_cds_cs01,
    bench_cds_cs01_metrics,
    bench_cds_index_cs01,
    bench_cds_index_metrics,
    bench_cds_tranche_cs01,
    bench_cds_tranche_all_metrics,
);
criterion_main!(benches);
