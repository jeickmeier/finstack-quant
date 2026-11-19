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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995603443277304),
            (0.25, 0.991164266563056),
            (0.375, 0.986683213533127),
            (0.5, 0.982161032358301),
            (0.625, 0.977613750592859),
            (0.75, 0.973057113658814),
            (0.875, 0.968491449282078),
            (1.0, 0.96391708450028),
            (1.125, 0.959334345625215),
            (1.25, 0.954743558205581),
            (1.375, 0.950145046990009),
            (1.5, 0.945539135890396),
            (1.625, 0.940926147945548),
            (1.75, 0.936306405285127),
            (1.875, 0.931680229093923),
            (2.0, 0.927047939576442),
            (2.125, 0.922409855921826),
            (2.25, 0.917766296269099),
            (2.375, 0.91311757767276),
            (2.5, 0.908464016068706),
            (2.625, 0.90380592624051),
            (2.75, 0.899143621786045),
            (2.875, 0.894477415084467),
            (3.0, 0.889807617263556),
            (3.125, 0.885137995730962),
            (3.25, 0.880472243595892),
            (3.375, 0.875810558590072),
            (3.5, 0.871153137115433),
            (3.625, 0.866500174232784),
            (3.75, 0.861851863650715),
            (3.875, 0.857208397714698),
            (4.0, 0.852569967396423),
            (4.125, 0.847936762283342),
            (4.25, 0.843308970568431),
            (4.375, 0.83868677904018),
            (4.5, 0.834070373072787),
            (4.625, 0.829459936616585),
            (4.75, 0.824855652188679),
            (4.875, 0.820257700863806),
            (5.0, 0.815666262265412),
            (5.125, 0.811084682850305),
            (5.25, 0.806516236151136),
            (5.375, 0.801960990558474),
            (5.5, 0.797419013460093),
            (5.625, 0.792890371242445),
            (5.75, 0.788375129292187),
            (5.875, 0.783873351997788),
            (6.0, 0.779385102751198),
            (6.125, 0.774910443949589),
            (6.25, 0.770449436997162),
            (6.375, 0.76600214230702),
            (6.5, 0.761568619303111),
            (6.625, 0.757148926422234),
            (6.75, 0.752743121116107),
            (6.875, 0.74835125985351),
            (7.0, 0.743973398122479),
            (7.1875, 0.737432973620303),
            (7.375, 0.730924350336037),
            (7.5625, 0.72444770320041),
            (7.75, 0.71800320223969),
            (7.9375, 0.711591012595335),
            (8.125, 0.705211294544325),
            (8.3125, 0.698864203520175),
            (8.5, 0.69254989013459),
            (8.6875, 0.686268500199782),
            (8.875, 0.680020174751424),
            (9.0625, 0.673805050072225),
            (9.25, 0.667623257716134),
            (9.4375, 0.661474924533137),
            (9.625, 0.65536017269466),
            (9.8125, 0.649279119719555),
            (10.0, 0.64323187850066),
            (10.3125, 0.633240940299762),
            (10.625, 0.623368658822283),
            (10.9375, 0.613614332055767),
            (11.25, 0.6039772483811),
            (11.5625, 0.594456686888306),
            (11.875, 0.585051917690078),
            (12.1875, 0.575762202233005),
            (12.5, 0.56658679360638),
            (12.8125, 0.557524936848565),
            (13.125, 0.548575869250803),
            (13.4375, 0.539738820658446),
            (13.75, 0.531013013769514),
            (14.0625, 0.522397664430525),
            (14.375, 0.513891981929555),
            (14.6875, 0.505495169286444),
            (15.0, 0.497206423540117),
            (15.3125, 0.489029711690409),
            (15.625, 0.480968680164261),
            (15.9375, 0.473022046684402),
            (16.25, 0.465188536839901),
            (16.5625, 0.457466884153669),
            (16.875, 0.449855830147316),
            (17.1875, 0.442354124403371),
            (17.5, 0.434960524624916),
            (17.8125, 0.427673796692653),
            (18.125, 0.420492714719432),
            (18.4375, 0.413416061102268),
            (18.75, 0.406442626571897),
            (19.0625, 0.399571210239862),
            (19.375, 0.392800619643206),
            (19.6875, 0.386129670786761),
            (20.0, 0.37955718818309),
            (20.625, 0.366717287160628),
            (21.25, 0.354284062920776),
            (21.875, 0.342245637132131),
            (22.5, 0.330590443680698),
            (23.125, 0.31930722161358),
            (23.75, 0.308385008205549),
            (24.375, 0.297813132147495),
            (25.0, 0.28758120685574),
            (25.625, 0.277679123901158),
            (26.25, 0.268097046557022),
            (26.875, 0.258825403464479),
            (27.5, 0.249854882414506),
            (28.125, 0.241176424245205),
            (28.75, 0.232781216853253),
            (29.375, 0.224660689318307),
            (30.0, 0.216806506139151),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Replayable par-spread quotes are required for quote-space CS01.
    let source_market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");
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

    source_market
        .try_insert(hazard)
        .expect("insert market curve")
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
