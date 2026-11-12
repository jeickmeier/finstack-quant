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

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_models::correlation::CopulaSpec;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CdsTranche, CdsTranchePricer,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/credit.rs"]
mod credit_support;
#[path = "support/record.rs"]
mod record;
mod support;
use serde_json::{Value, json};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn validate_price(value: &Money) {
    assert_eq!(value.currency(), Currency::USD);
    assert!(value.amount().is_finite());
}

fn validate_cs01(result: &ValuationResult) {
    validate_price(&result.value);
    assert!(result.measures.values().all(|value| value.is_finite()));
    assert!(result.measures.contains_key(&MetricId::Cs01));
}

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
        500.0, // 500bp running coupon
    );

    let schedule_params = ScheduleParams::quarterly_act360();

    let mut tranche = CdsTranche::new(
        format!("CDX_IG42_{}_{}_{}", attach_pct, detach_pct, tenor_years),
        &tranche_params,
        &schedule_params,
        "USD-OIS",
        "CDX.NA.IG.42",
        PayReceive::Receive,
    )
    .expect("Valid tranche parameters");
    tranche.roll_rule = finstack_quant_cashflows::builder::RollRule::CdsImm;
    tranche.stub = finstack_quant_core::dates::StubKind::ShortBack;
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
        .insert(Arc::clone(index_data.base_correlation_curve()))
        .insert_credit_index("CDX.NA.IG.42", index_data)
        .expect("Credit index dependencies should be registered")
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
    let mut issuer_curves = finstack_quant_core::FxHashMap::default();
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

    let mut market = source_market
        .insert(index_curve)
        .insert(Arc::clone(index_data.base_correlation_curve()));
    for curve in index_data
        .issuer_credit_curves()
        .expect("Issuer curves should exist")
        .values()
    {
        market.insert_mut(Arc::clone(curve));
    }
    market
        .insert_credit_index("CDX.NA.IG.42", index_data)
        .expect("Credit index dependencies should be registered")
}

struct CaptureSpec<'a> {
    case_id: &'a str,
    entry_point: &'static str,
    requested_metrics: &'a [&'a str],
}

fn capture_case(
    spec: CaptureSpec<'_>,
    tranche: &CdsTranche,
    market: &MarketContext,
    as_of: Date,
    copula: &CopulaSpec,
    output: impl FnOnce() -> Value,
) {
    record::capture(spec.case_id, || {
        let index = market
            .get_credit_index(&tranche.credit_index_id)
            .expect("captured credit index");
        let mut positive_weights = Vec::new();
        let mut positive_recoveries = Vec::new();
        let mut positive_pds = Vec::new();
        let issuers: Vec<_> = index
            .issuer_credit_curves()
            .into_iter()
            .flat_map(|curves| curves.iter())
            .map(|(id, curve)| {
                let weight = index.get_issuer_weight(id);
                let recovery = index.get_issuer_recovery(id);
                let pd = (1.0 - curve.sp_on_date(tranche.maturity).expect("issuer survival"))
                    .clamp(0.0, 1.0);
                if weight > 0.0 {
                    positive_weights.push(weight);
                    positive_recoveries.push(recovery);
                    positive_pds.push(pd);
                }
                json!({
                    "id": id,
                    "curve_id": curve.id(),
                    "weight": weight,
                    "recovery": recovery,
                    "weight_bits": format!("{:016x}", weight.to_bits()),
                    "recovery_bits": format!("{:016x}", recovery.to_bits()),
                    "maturity_default_probability": pd,
                })
            })
            .collect();
        let uniform = |values: &[f64], tolerance: f64| {
            values.first().is_some_and(|first| {
                values
                    .iter()
                    .all(|value| (value - first).abs() <= tolerance)
            })
        };
        let has_issuers = index.issuer_credit_curves().is_some();
        let positive_count = if has_issuers {
            positive_weights.len()
        } else {
            usize::from(index.num_constituents())
        };
        let tolerance_homogeneous = has_issuers
            && uniform(&positive_weights, 1e-9)
            && uniform(&positive_recoveries, 1e-9)
            && uniform(&positive_pds, 1e-9);
        let exact_equal_exposure =
            !has_issuers || (uniform(&positive_weights, 0.0) && uniform(&positive_recoveries, 0.0));
        let exact_states = u32::try_from(positive_count)
            .ok()
            .and_then(|count| 1usize.checked_shl(count))
            .filter(|states| *states <= 200_000);
        let branch = if !has_issuers {
            "homogeneous_index_binomial"
        } else if tolerance_homogeneous {
            "issuer_homogeneous_binomial"
        } else if exact_states.is_some() {
            "heterogeneous_exact_subsets"
        } else {
            "heterogeneous_fractional_grid"
        };
        json!({
            "fixture": {
                "instrument": tranche,
                "market": market,
                "as_of": as_of,
                "model": {
                    "registry_model": tranche.default_model(),
                    "copula": copula,
                    "entry_point": spec.entry_point,
                },
                "requested_metrics": spec.requested_metrics,
                "dimensions": {
                    "registered_names": index.num_constituents(),
                    "positive_weight_names": positive_count,
                    "issuers": issuers,
                    "index_recovery": index.recovery_rate(),
                    "implicit_homogeneous_weight": if has_issuers { None } else {
                        Some(1.0 / f64::from(index.num_constituents()))
                    },
                    "exact_equal_contractual_weights_and_recoveries": exact_equal_exposure,
                    "pd_uniform_at_maturity": !has_issuers || uniform(&positive_pds, 1e-9),
                    "homogeneity_tolerance": 1e-9,
                    "branch": branch,
                    "branch_basis": "inferred from B0 source and maturity inputs; not an observed integrand counter",
                    "exact_subset_states_if_selected": if branch == "heterogeneous_exact_subsets" {
                        exact_states
                    } else { None },
                    "grid_step_pool_fraction_if_selected": if branch == "heterogeneous_fractional_grid" {
                        Some(0.001)
                    } else { None },
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

fn flat_control_correlation(correlation: f64) -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots([3.0, 7.0, 10.0, 15.0, 30.0, 100.0].map(|strike| (strike, correlation)))
        .build()
        .expect("flat control correlation")
}

fn replace_issuer_pool(
    mut market: MarketContext,
    curves: Vec<(String, Arc<HazardCurve>)>,
    weights: Vec<(String, f64)>,
    recoveries: Vec<(String, f64)>,
    correlation: f64,
) -> MarketContext {
    let index = market
        .get_credit_index("CDX.NA.IG.42")
        .expect("source index");
    let correlation = Arc::new(flat_control_correlation(correlation));
    let replacement = index
        .to_builder()
        .num_constituents(u16::try_from(curves.len()).expect("control constituent count"))
        .issuer_curves(curves.iter().cloned())
        .issuer_weights(weights)
        .issuer_recovery_rates(recoveries)
        .base_correlation_curve(Arc::clone(&correlation))
        .build()
        .expect("control issuer pool");
    for (_, curve) in curves {
        market.insert_mut(curve);
    }
    market
        .insert(correlation)
        .insert_credit_index("CDX.NA.IG.42", replacement)
        .expect("registered control pool")
}

fn constant_control_hazard(id: &str, hazard: f64, recovery: f64) -> Arc<HazardCurve> {
    Arc::new(
        HazardCurve::builder(id)
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .recovery_rate(recovery)
            .knots([(1.0, hazard)])
            .build()
            .expect("constant control hazard"),
    )
}

fn create_four_name_market() -> MarketContext {
    let pds = [0.01_f64, 0.04, 0.12, 0.25];
    let weights = [0.1, 0.2, 0.3, 0.4];
    let recoveries = [0.39863, 0.59731, 0.19619, 0.49579];
    let ids: Vec<_> = (1..=4).map(|i| format!("ISSUER-{i:03}")).collect();
    let curves = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id.clone(),
                constant_control_hazard(id, -(1.0 - pds[i]).ln(), recoveries[i]),
            )
        })
        .collect();
    replace_issuer_pool(
        create_market_with_issuers(4),
        curves,
        ids.iter().cloned().zip(weights).collect(),
        ids.into_iter().zip(recoveries).collect(),
        0.30,
    )
}

fn create_concentrated_market() -> MarketContext {
    let ids: Vec<_> = (1..=50).map(|i| format!("ISSUER-{i:03}")).collect();
    let curves = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let (hazard, recovery) = if i == 0 { (0.08, 0.20) } else { (0.02, 0.50) };
            (id.clone(), constant_control_hazard(id, hazard, recovery))
        })
        .collect();
    replace_issuer_pool(
        create_market_with_issuers(50),
        curves,
        ids.iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), if i == 0 { 0.40 } else { 0.60 / 49.0 }))
            .collect(),
        ids.into_iter()
            .enumerate()
            .map(|(i, id)| (id, if i == 0 { 0.20 } else { 0.50 }))
            .collect(),
        0.30,
    )
}

fn create_recovery_dispersion_market() -> MarketContext {
    let market = create_market_with_issuers(50);
    let index = market
        .get_credit_index("CDX.NA.IG.42")
        .expect("source index");
    let curves: Vec<_> = index
        .issuer_credit_curves()
        .expect("issuer curves")
        .iter()
        .map(|(id, curve)| (id.clone(), Arc::clone(curve)))
        .collect();
    let ids: Vec<_> = curves.iter().map(|(id, _)| id.clone()).collect();
    replace_issuer_pool(
        market,
        curves,
        ids.iter().cloned().map(|id| (id, 1.0 / 50.0)).collect(),
        ids.into_iter()
            .enumerate()
            .map(|(i, id)| (id, [0.20, 0.40, 0.60][i % 3]))
            .collect(),
        0.25,
    )
}

fn create_high_hazard_equal_market() -> MarketContext {
    let ids: Vec<_> = (1..=125).map(|i| format!("ISSUER-{i:03}")).collect();
    let curves = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let bump = (i as f64 / 125.0) * 0.003;
            let curve = HazardCurve::builder(id.as_str())
                .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
                .recovery_rate(0.40)
                .knots([
                    (1.0, 5.0 * (0.012 + bump).min(0.05)),
                    (3.0, 5.0 * (0.016 + bump).min(0.05)),
                    (5.0, 5.0 * (0.020 + bump).min(0.05)),
                    (7.0, 5.0 * (0.024 + bump).min(0.05)),
                    (10.0, 5.0 * (0.030 + bump).min(0.05)),
                ])
                .build()
                .expect("high-hazard issuer curve");
            (id.clone(), Arc::new(curve))
        })
        .collect();
    replace_issuer_pool(
        create_market_with_issuers(125),
        curves,
        ids.iter().cloned().map(|id| (id, 1.0 / 125.0)).collect(),
        ids.into_iter().map(|id| (id, 0.40)).collect(),
        0.25,
    )
}

fn create_high_hazard_homogeneous_market() -> MarketContext {
    let market = create_market();
    let index = market
        .get_credit_index("CDX.NA.IG.42")
        .expect("source index");
    let hazard = Arc::new(
        HazardCurve::builder("CDX.NA.IG.42")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .recovery_rate(0.40)
            .knots([(1.0, 0.05), (3.0, 0.075), (5.0, 0.10), (10.0, 0.125)])
            .build()
            .expect("high-hazard homogeneous curve"),
    );
    let correlation = Arc::new(flat_control_correlation(0.25));
    let replacement = index
        .to_builder()
        .index_credit_curve(Arc::clone(&hazard))
        .base_correlation_curve(Arc::clone(&correlation))
        .build()
        .expect("high-hazard homogeneous index");
    market
        .insert(hazard)
        .insert(correlation)
        .insert_credit_index("CDX.NA.IG.42", replacement)
        .expect("registered high-hazard homogeneous index")
}

fn create_student_t_equal_market() -> MarketContext {
    create_market_with_issuers(50)
}

fn bench_cds_tranche_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_npv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test different tranche types
    let tranches = vec![
        ("equity_0_3", 0.0, 3.0),
        ("junior_mezz_3_7", 3.0, 7.0),
        ("senior_mezz_7_10", 7.0, 10.0),
        ("senior_10_15", 10.0, 15.0),
        ("thin_6_6_1", 6.0, 6.1),
        ("high_60_100", 60.0, 100.0),
    ];

    for (name, attach, detach) in tranches {
        group.bench_with_input(BenchmarkId::from_parameter(name), name, |b, _| {
            let tranche = create_tranche(attach, detach, 5);
            assert_eq!((tranche.attach_pct, tranche.detach_pct), (attach, detach));
            bench_npv_case(b, &format!("cds_tranche_npv/{name}"), &tranche, as_of);
        });
    }
    group.bench_function("ten_year", |b| {
        let tranche = create_tranche(3.0, 7.0, 10);
        bench_npv_case(b, "cds_tranche_npv/ten_year", &tranche, as_of);
    });
    group.bench_function("seasoned", |b| {
        let mut tranche = create_tranche(7.0, 15.0, 5);
        tranche.realized_loss = 0.02;
        bench_npv_case(b, "cds_tranche_npv/seasoned", &tranche, as_of);
    });
    group.finish();
}

/// Validated, optionally captured Gaussian NPV case on the standard market.
fn bench_npv_case(
    b: &mut criterion::Bencher<'_>,
    case_id: &str,
    tranche: &CdsTranche,
    as_of: Date,
) {
    let market = create_market();
    support::bench(
        b,
        || {
            tranche
                .value(black_box(&market), black_box(as_of))
                .expect("CDS tranche PV")
        },
        |value| {
            validate_price(value);
            capture_case(
                CaptureSpec {
                    case_id,
                    entry_point: "Instrument::value",
                    requested_metrics: &[],
                },
                tranche,
                &market,
                as_of,
                &CopulaSpec::Gaussian,
                || json!({"value": value}),
            );
        },
    );
}

fn bench_cds_tranche_cs01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_cs01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    group.bench_function("cs01", |b| {
        let market = create_market();
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
                validate_cs01(result);
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_cs01/cs01",
                        entry_point: "Instrument::price_with_metrics",
                        requested_metrics: &["cs01"],
                    },
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

fn bench_cds_tranche_correlation_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_correlation_delta");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    group.bench_function("correlation_delta", |b| {
        let market = create_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        let pricer = CdsTranchePricer::new();
        support::bench(
            b,
            || {
                pricer
                    .calculate_correlation_delta(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche correlation delta")
            },
            |value| {
                assert!(value.is_finite());
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_correlation_delta/correlation_delta",
                        entry_point: "CdsTranchePricer::calculate_correlation_delta",
                        requested_metrics: &["correlation_delta"],
                    },
                    &tranche,
                    &market,
                    as_of,
                    pricer.get_copula_spec(),
                    || json!({"scalar": value, "unit": "USD per 0.01 absolute correlation"}),
                );
            },
        );
    });

    group.finish();
}

fn bench_cds_tranche_jump_to_default(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_jump_to_default");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    group.bench_function("jump_to_default", |b| {
        let market = create_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        let pricer = CdsTranchePricer::new();
        support::bench(
            b,
            || {
                pricer
                    .calculate_jump_to_default(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche jump to default")
            },
            |value| {
                assert!(value.is_finite());
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_jump_to_default/jump_to_default",
                        entry_point: "CdsTranchePricer::calculate_jump_to_default",
                        requested_metrics: &["jump_to_default"],
                    },
                    &tranche,
                    &market,
                    as_of,
                    pricer.get_copula_spec(),
                    || json!({"scalar": value, "unit": "USD"}),
                );
            },
        );
    });

    group.finish();
}

fn bench_cds_tranche_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_par_spread");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    group.bench_function("par_spread", |b| {
        let market = create_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        let pricer = CdsTranchePricer::new();
        support::bench(
            b,
            || {
                pricer
                    .calculate_par_spread(&tranche, black_box(&market), black_box(as_of))
                    .expect("CDS tranche par spread")
            },
            |value| {
                assert!(value.is_finite());
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_par_spread/par_spread",
                        entry_point: "CdsTranchePricer::calculate_par_spread",
                        requested_metrics: &["par_spread"],
                    },
                    &tranche,
                    &market,
                    as_of,
                    pricer.get_copula_spec(),
                    || json!({"scalar": value, "unit": "basis_points"}),
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
        let market = create_market();
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
                validate_cs01(cs01);
                assert!(values.iter().all(|value| value.is_finite()));
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_all_metrics/all_metrics",
                        entry_point: "price_with_metrics + calculate_correlation_delta + calculate_jump_to_default + calculate_par_spread",
                        requested_metrics: &["cs01", "correlation_delta", "jump_to_default", "par_spread"],
                    },
                    &tranche, &market, as_of, pricer.get_copula_spec(),
                    || json!({
                        "value": cs01.value,
                        "metrics": {
                            "cs01_result": cs01.measures,
                            "correlation_delta": values[0],
                            "jump_to_default": values[1],
                            "par_spread": values[2],
                        },
                    }),
                );
            },
        );
    });

    group.finish();
}

fn bench_cds_tranche_student_t_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_student_t_npv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    group.bench_function("junior_mezz_3_7_df6", |b| {
        let market = create_market();
        let tranche = create_tranche(3.0, 7.0, 5);
        let pricer = CdsTranchePricer::with_copula(CopulaSpec::student_t(6.0).unwrap()).unwrap();
        support::bench(
            b,
            || {
                pricer
                    .price_tranche(black_box(&tranche), black_box(&market), black_box(as_of))
                    .expect("Student-t CDS tranche PV")
            },
            |value| {
                validate_price(value);
                capture_case(
                    CaptureSpec {
                        case_id: "cds_tranche_student_t_npv/junior_mezz_3_7_df6",
                        entry_point: "CdsTranchePricer::price_tranche",
                        requested_metrics: &[],
                    },
                    &tranche,
                    &market,
                    as_of,
                    pricer.get_copula_spec(),
                    || json!({"value": value}),
                );
            },
        );
    });
    group.finish();
}

fn bench_cds_tranche_heterogeneous(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_heterogeneous");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let pool_sizes = [10, 16, 17, 18, 50, 125];

    for &pool_size in &pool_sizes {
        group.bench_with_input(
            BenchmarkId::new("npv_hetero", pool_size),
            &pool_size,
            |b, _| {
                let market = create_market_with_issuers(pool_size);
                let tranche = create_tranche(3.0, 7.0, 5);
                let index = market
                    .get_credit_index("CDX.NA.IG.42")
                    .expect("credit index fixture");
                assert_eq!(
                    index
                        .issuer_credit_curves()
                        .expect("heterogeneous issuer curves")
                        .len(),
                    pool_size
                );
                support::bench(
                    b,
                    || {
                        tranche
                            .value(black_box(&market), black_box(as_of))
                            .expect("heterogeneous CDS tranche PV")
                    },
                    |value| {
                        validate_price(value);
                        capture_case(
                            CaptureSpec {
                                case_id: &format!(
                                    "cds_tranche_heterogeneous/npv_hetero/{pool_size}"
                                ),
                                entry_point: "Instrument::value",
                                requested_metrics: &[],
                            },
                            &tranche,
                            &market,
                            as_of,
                            &CopulaSpec::Gaussian,
                            || json!({"value": value}),
                        );
                    },
                );
            },
        );
    }
    for pool_size in [18, 125] {
        group.bench_with_input(
            BenchmarkId::new("correlation_delta", pool_size),
            &pool_size,
            |b, &pool_size| {
                let market = create_market_with_issuers(pool_size);
                let tranche = create_tranche(3.0, 7.0, 5);
                let pricer = CdsTranchePricer::new();
                support::bench(
                    b,
                    || pricer.calculate_correlation_delta(
                        black_box(&tranche), black_box(&market), black_box(as_of),
                    ).expect("heterogeneous correlation delta"),
                    |value| {
                        assert!(value.is_finite());
                        capture_case(
                            CaptureSpec { case_id: &format!("cds_tranche_heterogeneous/correlation_delta/{pool_size}"), entry_point: "CdsTranchePricer::calculate_correlation_delta", requested_metrics: &["correlation_delta"] },
                            &tranche, &market, as_of, pricer.get_copula_spec(),
                            || json!({"scalar": value, "unit": "USD per 0.01 absolute correlation"}),
                        );
                    },
                );
            },
        );
    }
    group.finish();
}

struct TrancheControl {
    name: &'static str,
    market: fn() -> MarketContext,
    attach_pct: f64,
    detach_pct: f64,
    coupon_bp: f64,
    copula: CopulaSpec,
    names: usize,
    exact_equal_exposure: bool,
}

fn bench_cds_tranche_p5_controls(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_p5_controls");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let controls = [
        TrancheControl {
            name: "gaussian_heterogeneous_four_3_7",
            market: create_four_name_market,
            attach_pct: 3.0,
            detach_pct: 7.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::Gaussian,
            names: 4,
            exact_equal_exposure: false,
        },
        TrancheControl {
            name: "gaussian_concentrated_50_3_7",
            market: create_concentrated_market,
            attach_pct: 3.0,
            detach_pct: 7.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::Gaussian,
            names: 50,
            exact_equal_exposure: false,
        },
        TrancheControl {
            name: "gaussian_recovery_dispersion_50_3_7",
            market: create_recovery_dispersion_market,
            attach_pct: 3.0,
            detach_pct: 7.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::Gaussian,
            names: 50,
            exact_equal_exposure: false,
        },
        TrancheControl {
            name: "gaussian_equal_exposure_125_40_80",
            market: create_high_hazard_equal_market,
            attach_pct: 40.0,
            detach_pct: 80.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::Gaussian,
            names: 125,
            exact_equal_exposure: true,
        },
        TrancheControl {
            name: "gaussian_homogeneous_125_40_80",
            market: create_high_hazard_homogeneous_market,
            attach_pct: 40.0,
            detach_pct: 80.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::Gaussian,
            names: 125,
            exact_equal_exposure: true,
        },
        TrancheControl {
            name: "gaussian_homogeneous_125_60_100",
            market: create_high_hazard_homogeneous_market,
            attach_pct: 60.0,
            detach_pct: 100.0,
            coupon_bp: 25.0,
            copula: CopulaSpec::Gaussian,
            names: 125,
            exact_equal_exposure: true,
        },
        TrancheControl {
            name: "student_t_equal_exposure_50_3_7_df6",
            market: create_student_t_equal_market,
            attach_pct: 3.0,
            detach_pct: 7.0,
            coupon_bp: 500.0,
            copula: CopulaSpec::student_t(6.0).expect("Student-t control"),
            names: 50,
            exact_equal_exposure: true,
        },
    ];
    for control in controls {
        group.bench_function(control.name, |b| {
            let market = (control.market)();
            let mut tranche = create_tranche(control.attach_pct, control.detach_pct, 5);
            tranche.coupon_bp = control.coupon_bp;
            let index = market
                .get_credit_index(&tranche.credit_index_id)
                .expect("control index");
            assert_eq!(usize::from(index.num_constituents()), control.names);
            if let Some(curves) = index.issuer_credit_curves() {
                let positive: Vec<_> = curves
                    .keys()
                    .filter_map(|id| {
                        let weight = index.get_issuer_weight(id);
                        (weight > 0.0).then_some((weight, index.get_issuer_recovery(id)))
                    })
                    .collect();
                assert_eq!(positive.len(), control.names);
                assert_eq!(
                    positive.iter().all(|entry| *entry == positive[0]),
                    control.exact_equal_exposure,
                );
            }
            let pricer =
                CdsTranchePricer::with_copula(control.copula.clone()).expect("control pricer");
            assert_eq!(pricer.get_copula_spec(), &control.copula);
            support::bench(
                b,
                || {
                    pricer
                        .price_tranche(black_box(&tranche), black_box(&market), black_box(as_of))
                        .expect("tranche control PV")
                },
                |value| {
                    validate_price(value);
                    capture_case(
                        CaptureSpec {
                            case_id: &format!("cds_tranche_p5_controls/{}", control.name),
                            entry_point: "CdsTranchePricer::price_tranche",
                            requested_metrics: &[],
                        },
                        &tranche,
                        &market,
                        as_of,
                        pricer.get_copula_spec(),
                        || json!({"value": value}),
                    );
                },
            );
        });
    }
    group.finish();
}

fn bench_cds_tranche_prepared(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_tranche_prepared");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let tranche = create_tranche(3.0, 7.0, 5);
    let pricer = CdsTranchePricer::new();
    let prepared = pricer.prepare(&tranche, &market, as_of).unwrap();
    let index = market.get_credit_index("CDX.NA.IG.42").unwrap();
    let correlation = index.base_correlation_curve();
    assert_eq!(
        prepared.price(correlation).unwrap(),
        pricer.price_tranche(&tranche, &market, as_of).unwrap()
    );
    group.bench_function("prepare_only_3_7", |b| {
        b.iter(|| {
            pricer
                .prepare(black_box(&tranche), black_box(&market), black_box(as_of))
                .unwrap()
        })
    });
    group.bench_function("reuse_3_7", |b| {
        b.iter(|| prepared.price(black_box(correlation)).unwrap())
    });
    let noncontiguous = create_tranche(4.0, 7.0, 5);
    let prepared_noncontiguous = pricer.prepare(&noncontiguous, &market, as_of).unwrap();
    let trials: Vec<_> = [0.26, 0.27]
        .into_iter()
        .map(|rho| {
            BaseCorrelationCurve::builder("interpolated_trial")
                .knots([(3.0, 0.25), (7.0, rho), (100.0, rho)])
                .build()
                .unwrap()
        })
        .collect();
    for trial in &trials {
        assert_eq!(
            prepared_noncontiguous.price(trial).unwrap(),
            pricer
                .prepare(&noncontiguous, &market, as_of)
                .unwrap()
                .price(trial)
                .unwrap()
        );
    }
    // Alternate the detachment knot so the actual interpolated attachment
    // correlation changes on every trial: this is a cache-invalidation control.
    group.bench_function("reuse_interpolated_4_7", |b| {
        let mut trial = 0usize;
        b.iter(|| {
            trial ^= 1;
            prepared_noncontiguous
                .price(black_box(&trials[trial]))
                .unwrap()
        })
    });
    group.bench_function("prepare_and_price_interpolated_4_7", |b| {
        let mut trial = 0usize;
        b.iter(|| {
            trial ^= 1;
            pricer
                .prepare(black_box(&noncontiguous), black_box(&market), as_of)
                .unwrap()
                .price(black_box(&trials[trial]))
                .unwrap()
        })
    });
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
    bench_cds_tranche_student_t_npv,
    bench_cds_tranche_heterogeneous,
    bench_cds_tranche_p5_controls,
    bench_cds_tranche_prepared
);
criterion_main!(benches);
