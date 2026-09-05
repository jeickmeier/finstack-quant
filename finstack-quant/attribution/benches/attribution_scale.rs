//! P&L attribution scaling benchmarks.
//!
//! Measures the per-instrument cost of each public attribution entry point
//! in [`finstack_quant_attribution`] across realistic portfolio sizes
//! (N ∈ {10, 100, 1000}). All methodologies run against the same pair of
//! market states (`market_t0`, `market_t1`) and as-of dates so the numbers
//! are directly comparable. The shift between `market_t0` and `market_t1`
//! is a 1bp parallel move of the flat USD discount curve — small enough to
//! be realistic, large enough that every methodology has something to
//! decompose.
//!
//! The bench group name is `"attribution"` to match the existing style of
//! `attribution.rs`; individual bench ids are `"<method>/<N>"`.
//!
//! Note: `pnl_bridge` is the minimal baseline (two reprices, no
//! factor loop). The other methodologies all add factor iteration on top
//! and should be benchmarked against the baseline to quantify that cost.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, attribute_return_contribution,
    default_waterfall_order, pnl_bridge, AttributionMethod, AttributionRequest, ExecutionPolicy,
    MarketRestoreFlags, MarketSnapshot, TaylorAttributionConfig,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::IssuerId;
use finstack_quant_models::factor::credit::hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_quant_models::factor::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::metrics::MetricId;
use fixtures::{
    market_state, multi_curve_market, return_contribution_spec, sample_bond_idx, BondMarkets,
    BASE_RATE, USD_OIS,
};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use time::{Date, Month};

const SHIFT_BP: f64 = 1.0;
const PORTFOLIO_SIZES: &[usize] = &[10, 100, 1000];

/// Shared inputs for every methodology. Built once per N per benchmark run
/// so we don't re-allocate curves inside `b.iter`.
struct Fixture {
    bonds: Vec<Arc<dyn Instrument>>,
    markets: BondMarkets,
}

impl Fixture {
    fn new(n: usize) -> Self {
        let markets = BondMarkets::new(SHIFT_BP);
        let bonds: Vec<Arc<dyn Instrument>> = (0..n)
            .map(|i| Arc::new(sample_bond_idx(i)) as Arc<dyn Instrument>)
            .collect();
        Self { bonds, markets }
    }
}

/// Metrics requested for the metrics-based methodology. Limited to the
/// bond-applicable subset so `price_with_metrics` does not fail under
/// strict mode; `attribute_pnl_metrics_based` handles missing metrics
/// gracefully via `measures.get()`.
fn bond_attribution_metrics() -> Vec<MetricId> {
    vec![MetricId::Dv01, MetricId::Theta, MetricId::Convexity]
}

// Per-methodology inner loops

fn run_simple_bridge(fx: &Fixture) {
    for bond in &fx.bonds {
        let pnl = pnl_bridge(
            bond,
            black_box(&fx.markets.market_t0),
            black_box(&fx.markets.market_t1),
            black_box(fx.markets.as_of_t0),
            black_box(fx.markets.as_of_t1),
            Currency::USD,
        )
        .unwrap();
        black_box(pnl);
    }
}

fn run_metrics_based(fx: &Fixture) {
    let metrics = bond_attribution_metrics();
    let opts = PricingOptions::default();
    for bond in &fx.bonds {
        let val_t0 = bond
            .price_with_metrics(
                &fx.markets.market_t0,
                fx.markets.as_of_t0,
                &metrics,
                opts.clone(),
            )
            .unwrap();
        let val_t1 = bond
            .price_with_metrics(
                &fx.markets.market_t1,
                fx.markets.as_of_t1,
                &metrics,
                opts.clone(),
            )
            .unwrap();
        let attr = attribute_pnl_metrics_based(
            bond,
            black_box(&fx.markets.market_t0),
            black_box(&fx.markets.market_t1),
            &val_t0,
            &val_t1,
            black_box(fx.markets.as_of_t0),
            black_box(fx.markets.as_of_t1),
        )
        .unwrap();
        black_box(attr);
    }
}

/// Metrics-based attribution with valuations prepared outside the loop, so
/// the measured cost is the linear decomposition rather than `price_with_metrics`.
fn run_metrics_based_precomputed(
    fx: &Fixture,
    vals: &[(
        finstack_quant_valuations::results::ValuationResult,
        finstack_quant_valuations::results::ValuationResult,
    )],
) {
    for (bond, (val_t0, val_t1)) in fx.bonds.iter().zip(vals) {
        let attr = attribute_pnl_metrics_based(
            bond,
            black_box(&fx.markets.market_t0),
            black_box(&fx.markets.market_t1),
            val_t0,
            val_t1,
            black_box(fx.markets.as_of_t0),
            black_box(fx.markets.as_of_t1),
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_parallel(fx: &Fixture, policy: ExecutionPolicy) {
    for bond in &fx.bonds {
        let attr = attribute_pnl(
            &AttributionMethod::Parallel,
            &AttributionRequest {
                execution_policy: policy,
                ..AttributionRequest::new(
                    bond,
                    black_box(&fx.markets.market_t0),
                    black_box(&fx.markets.market_t1),
                    black_box(fx.markets.as_of_t0),
                    black_box(fx.markets.as_of_t1),
                    &fx.markets.config,
                )
            },
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_waterfall(fx: &Fixture, factor_order: &[finstack_quant_attribution::AttributionFactor]) {
    for bond in &fx.bonds {
        let attr = attribute_pnl(
            &AttributionMethod::Waterfall(factor_order.to_vec()),
            &AttributionRequest {
                strict_validation: false,
                model_params_t0: None,
                ..AttributionRequest::new(
                    bond,
                    black_box(&fx.markets.market_t0),
                    black_box(&fx.markets.market_t1),
                    black_box(fx.markets.as_of_t0),
                    black_box(fx.markets.as_of_t1),
                    &fx.markets.config,
                )
            },
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_taylor(fx: &Fixture, taylor_cfg: &TaylorAttributionConfig) {
    for bond in &fx.bonds {
        let attr = attribute_pnl(
            &AttributionMethod::Taylor(taylor_cfg.clone()),
            &AttributionRequest {
                execution_policy: ExecutionPolicy::Parallel,
                ..AttributionRequest::new(
                    bond,
                    black_box(&fx.markets.market_t0),
                    black_box(&fx.markets.market_t1),
                    black_box(fx.markets.as_of_t0),
                    black_box(fx.markets.as_of_t1),
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .unwrap();
        black_box(attr);
    }
}

// Criterion entry point

fn bench_attribution_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("attribution");
    // Full-fat sampling on a 1000-instrument portfolio with waterfall/parallel
    // attribution is pathologically slow, so we shrink the sample count. The
    // default (100) would take minutes per size; 10 samples is enough to see
    // scaling trends for regression tracking.
    group.sample_size(10);

    let waterfall_order = default_waterfall_order();
    let taylor_cfg = TaylorAttributionConfig::default();

    for &n in PORTFOLIO_SIZES {
        let fx = Fixture::new(n);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("simple_bridge", n), &fx, |b, fx| {
            b.iter(|| run_simple_bridge(fx));
        });

        group.bench_with_input(BenchmarkId::new("metrics_based", n), &fx, |b, fx| {
            b.iter(|| run_metrics_based(fx));
        });

        let metrics = bond_attribution_metrics();
        let opts = PricingOptions::default();
        let precomputed: Vec<_> = fx
            .bonds
            .iter()
            .map(|bond| {
                (
                    bond.price_with_metrics(
                        &fx.markets.market_t0,
                        fx.markets.as_of_t0,
                        &metrics,
                        opts.clone(),
                    )
                    .unwrap(),
                    bond.price_with_metrics(
                        &fx.markets.market_t1,
                        fx.markets.as_of_t1,
                        &metrics,
                        opts.clone(),
                    )
                    .unwrap(),
                )
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::new("metrics_based_precomputed", n),
            &fx,
            |b, fx| {
                b.iter(|| run_metrics_based_precomputed(fx, &precomputed));
            },
        );

        group.bench_with_input(BenchmarkId::new("parallel", n), &fx, |b, fx| {
            b.iter(|| run_parallel(fx, ExecutionPolicy::Parallel));
        });

        group.bench_with_input(BenchmarkId::new("parallel_serial", n), &fx, |b, fx| {
            b.iter(|| run_parallel(fx, ExecutionPolicy::Serial));
        });

        group.bench_with_input(BenchmarkId::new("waterfall", n), &fx, |b, fx| {
            b.iter(|| run_waterfall(fx, &waterfall_order));
        });

        group.bench_with_input(BenchmarkId::new("taylor", n), &fx, |b, fx| {
            b.iter(|| run_taylor(fx, &taylor_cfg));
        });
    }

    group.finish();
}

// PR-12: 200-position portfolio with a credit factor model

use finstack_quant_attribution::{AttributionEnvelope, AttributionSpec, CreditFactorDetailOptions};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

/// Build a minimal `CreditFactorModel` that covers `n` synthetic issuers.
/// Each issuer has a single-level (Rating) bucket tag and a pc beta of 0.7.
fn build_credit_model_for_n(n: usize) -> CreditFactorModel {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let issuer_betas: Vec<IssuerBetaRow> = (0..n)
        .map(|i| {
            let rating = if i % 2 == 0 { "IG" } else { "HY" };
            let mut tags_map = BTreeMap::new();
            tags_map.insert("rating".to_owned(), rating.to_owned());
            IssuerBetaRow {
                issuer_id: IssuerId::new(format!("BENCH-BOND-{i}")),
                tags: IssuerTags(tags_map),
                mode: IssuerBetaMode::IssuerBeta,
                betas: IssuerBetas {
                    pc: 0.7,
                    levels: vec![0.5],
                },
                adder_at_anchor: 0.0,
                adder_vol_annualized: 0.01,
                adder_vol_source: AdderVolSource::Default,
                fit_quality: None,
                level_fit_quality: vec![],
                spread_duration: 1.0,
            }
        })
        .collect();

    let calibration_window = DateRange {
        start: Date::from_calendar_date(2022, Month::January, 1).unwrap(),
        end: as_of,
    };

    let config = FactorModelConfig {
        factors: vec![],
        covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
        matching: MatchingConfig::MappingTable(vec![]),
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: Default::default(),
        bump_size: None,
        unmatched_policy: None,
    };

    CreditFactorModel {
        schema: finstack_quant_models::factor::credit::hierarchy::CreditFactorModelSchema::CURRENT,
        as_of,
        calibration_window,
        policy: IssuerBetaPolicy::GloballyOff,
        generic_factor: GenericFactorSpec {
            name: "CDX IG 5Y".to_owned(),
            series_id: "cdx.ig.5y".to_owned(),
        },
        hierarchy: CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating],
        },
        panel_frequency:
            finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
        use_returns_or_levels:
            finstack_quant_models::factor::credit::calibration::PanelSpace::Returns,
        bucket_weighting:
            finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
        config,
        issuer_betas,
        anchor_state: LevelsAtAnchor {
            pc: 100.0,
            by_level: vec![],
        },
        static_correlation: FactorCorrelationMatrix {
            factor_ids: vec![],
            data: vec![],
        },
        vol_state: VolState {
            factors: BTreeMap::new(),
            idiosyncratic: BTreeMap::new(),
        },
        factor_histories: None,
        diagnostics: CalibrationDiagnostics {
            mode_counts: BTreeMap::new(),
            bucket_sizes_per_level: vec![],
            fold_ups: vec![],
            r_squared_histogram: None,
            tag_taxonomy: BTreeMap::new(),
        },
    }
}

/// Build a bond spec with issuer ID metadata.
fn sample_bond_with_issuer(idx: usize) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let years = 1 + (idx % 10) as i32;
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
    let mut bond = Bond::fixed(
        format!("BENCH-BOND-{idx}"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        USD_OIS,
    )
    .unwrap();
    bond.attributes = Attributes::new().with_meta("credit::issuer_id", format!("BENCH-BOND-{idx}"));
    bond
}

struct CreditFixture {
    /// Pre-built attribution specs (one per bond).
    specs: Vec<AttributionEnvelope>,
}

impl CreditFixture {
    fn new(n: usize) -> Self {
        let as_of_t0 = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = Date::from_calendar_date(2025, Month::January, 16).unwrap();
        let market_t0 = market_state(as_of_t0, BASE_RATE, USD_OIS);
        let market_t1 = market_state(as_of_t1, BASE_RATE + SHIFT_BP / 10_000.0, USD_OIS);
        let credit_model = build_credit_model_for_n(n);
        let model_ref = Box::new(credit_model);

        let specs: Vec<AttributionEnvelope> = (0..n)
            .map(|i| {
                let bond = sample_bond_with_issuer(i);
                let spec = AttributionSpec {
                    instrument: InstrumentJson::Bond(bond),
                    market_t0: market_t0.clone(),
                    market_t1: market_t1.clone(),
                    as_of_t0,
                    as_of_t1,
                    method: AttributionMethod::Parallel,
                    config: None,
                    model_params_t0: None,
                    credit_factor_model: Some(model_ref.clone()),
                    credit_factor_detail_options: CreditFactorDetailOptions::default(),
                    full_cross_attribution: false,
                };
                AttributionEnvelope::new(spec)
            })
            .collect();

        Self { specs }
    }
}

/// Run attribution for all specs in the credit fixture.
fn run_attribution_with_credit_model(fx: &CreditFixture) {
    for envelope in &fx.specs {
        let result = envelope.execute().unwrap();
        black_box(result);
    }
}

fn bench_attribution_with_credit_model(c: &mut Criterion) {
    const CREDIT_N: usize = 200;
    let mut group = c.benchmark_group("attribution_credit");
    group.sample_size(10);
    group.throughput(Throughput::Elements(CREDIT_N as u64));

    let fx = CreditFixture::new(CREDIT_N);
    group.bench_function("parallel_with_credit_model/200", |b| {
        b.iter(|| run_attribution_with_credit_model(&fx));
    });

    group.finish();
}

fn bench_return_contribution_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("return_contribution");
    group.sample_size(20);
    for &n in &[100_usize, 1_000, 10_000] {
        let spec = return_contribution_spec(n, false);
        let brinson = return_contribution_spec(n, true);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("gross", n), &spec, |b, spec| {
            b.iter(|| black_box(attribute_return_contribution(spec).unwrap()));
        });
        group.bench_with_input(BenchmarkId::new("brinson", n), &brinson, |b, spec| {
            b.iter(|| black_box(attribute_return_contribution(spec).unwrap()));
        });
    }
    group.finish();
}

fn bench_snapshot_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_extract_restore");
    group.sample_size(20);
    for &n in &[1_usize, 10, 50] {
        let (t0, t1, _) = multi_curve_market(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("rates", n), &(t0, t1), |b, (t0, t1)| {
            b.iter(|| {
                let snap = MarketSnapshot::extract(black_box(t0), MarketRestoreFlags::RATES);
                let restored =
                    MarketSnapshot::restore_market(black_box(t1), &snap, MarketRestoreFlags::RATES);
                black_box(restored);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_attribution_scale,
    bench_attribution_with_credit_model,
    bench_return_contribution_scale,
    bench_snapshot_scale
);
criterion_main!(benches);
