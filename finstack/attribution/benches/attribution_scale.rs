//! P&L attribution scaling benchmarks.
//!
//! Measures the per-instrument cost of each public attribution entry point
//! in [`finstack_attribution`] across realistic portfolio sizes
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
//! Note: `simple_pnl_bridge` is the minimal baseline (two reprices, no
//! factor loop). The other methodologies all add factor iteration on top
//! and should be benchmarked against the baseline to quantify that cost.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_parallel, attribute_pnl_taylor,
    attribute_pnl_waterfall, default_waterfall_order, simple_pnl_bridge, AttributionMethod,
    TaylorAttributionConfig,
};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::factor_model::credit_hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_core::factor_model::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::IssuerId;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::PricingOptions;
use finstack_valuations::instruments::{Attributes, Instrument};
use finstack_valuations::metrics::MetricId;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use time::{Date, Month};

// ---------------------------------------------------------------------------
// Shared fixture
// ---------------------------------------------------------------------------

const CURVE_ID: &str = "USD-OIS";
const BASE_RATE: f64 = 0.04;
const SHIFT_BP: f64 = 1.0;
const PORTFOLIO_SIZES: &[usize] = &[10, 100, 1000];

/// Build a flat USD-OIS-style discount curve at the given continuously
/// compounded zero rate.
fn build_flat_curve(as_of: Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0_f64, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(CURVE_ID)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

/// Rolling-maturity fixed-rate USD corporate bond. Using a short-dated
/// (1–10y) vanilla fixed-coupon bond keeps every pricing path warm without
/// pulling in options/volatility machinery.
fn sample_bond(idx: usize) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    // Spread maturities across 1..=10 years so the portfolio isn't a
    // degenerate duplicate of a single instrument.
    let years = 1 + (idx % 10) as i32;
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
    Bond::fixed(
        format!("BENCH-BOND-{idx}"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        CURVE_ID,
    )
    .unwrap()
}

/// Shared inputs for every methodology. Built once per N per benchmark run
/// so we don't re-allocate curves inside `b.iter`.
struct Fixture {
    bonds: Vec<Arc<dyn Instrument>>,
    market_t0: MarketContext,
    market_t1: MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: FinstackConfig,
}

impl Fixture {
    fn new(n: usize) -> Self {
        let as_of_t0 = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = Date::from_calendar_date(2025, Month::January, 16).unwrap();

        let curve_t0 = build_flat_curve(as_of_t0, BASE_RATE);
        let curve_t1 = build_flat_curve(as_of_t1, BASE_RATE + SHIFT_BP / 10_000.0);

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let bonds: Vec<Arc<dyn Instrument>> = (0..n)
            .map(|i| Arc::new(sample_bond(i)) as Arc<dyn Instrument>)
            .collect();

        Self {
            bonds,
            market_t0,
            market_t1,
            as_of_t0,
            as_of_t1,
            config: FinstackConfig::default(),
        }
    }
}

/// Metrics requested for the metrics-based methodology. Limited to the
/// bond-applicable subset so `price_with_metrics` does not fail under
/// strict mode; `attribute_pnl_metrics_based` handles missing metrics
/// gracefully via `measures.get()`.
fn bond_attribution_metrics() -> Vec<MetricId> {
    vec![MetricId::Dv01, MetricId::Theta, MetricId::Convexity]
}

// ---------------------------------------------------------------------------
// Per-methodology inner loops
// ---------------------------------------------------------------------------

fn run_simple_bridge(fx: &Fixture) {
    for bond in &fx.bonds {
        let pnl = simple_pnl_bridge(
            bond,
            black_box(&fx.market_t0),
            black_box(&fx.market_t1),
            black_box(fx.as_of_t0),
            black_box(fx.as_of_t1),
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
            .price_with_metrics(&fx.market_t0, fx.as_of_t0, &metrics, opts.clone())
            .unwrap();
        let val_t1 = bond
            .price_with_metrics(&fx.market_t1, fx.as_of_t1, &metrics, opts.clone())
            .unwrap();
        let attr = attribute_pnl_metrics_based(
            bond,
            black_box(&fx.market_t0),
            black_box(&fx.market_t1),
            &val_t0,
            &val_t1,
            black_box(fx.as_of_t0),
            black_box(fx.as_of_t1),
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_parallel(fx: &Fixture) {
    for bond in &fx.bonds {
        let attr = attribute_pnl_parallel(
            bond,
            black_box(&fx.market_t0),
            black_box(&fx.market_t1),
            black_box(fx.as_of_t0),
            black_box(fx.as_of_t1),
            &fx.config,
            None,
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_waterfall(fx: &Fixture, factor_order: &[finstack_attribution::AttributionFactor]) {
    for bond in &fx.bonds {
        let attr = attribute_pnl_waterfall(
            bond,
            black_box(&fx.market_t0),
            black_box(&fx.market_t1),
            black_box(fx.as_of_t0),
            black_box(fx.as_of_t1),
            &fx.config,
            factor_order.to_vec(),
            false,
            None,
        )
        .unwrap();
        black_box(attr);
    }
}

fn run_taylor(fx: &Fixture, taylor_cfg: &TaylorAttributionConfig) {
    for bond in &fx.bonds {
        let attr = attribute_pnl_taylor(
            bond,
            black_box(&fx.market_t0),
            black_box(&fx.market_t1),
            black_box(fx.as_of_t0),
            black_box(fx.as_of_t1),
            taylor_cfg,
        )
        .unwrap();
        black_box(attr);
    }
}

// ---------------------------------------------------------------------------
// Criterion entry point
// ---------------------------------------------------------------------------

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

        group.bench_with_input(BenchmarkId::new("parallel", n), &fx, |b, fx| {
            b.iter(|| run_parallel(fx));
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

// ---------------------------------------------------------------------------
// PR-12: 200-position portfolio with a credit factor model
// ---------------------------------------------------------------------------

use finstack_attribution::{AttributionEnvelope, AttributionSpec, CreditFactorDetailOptions};
use finstack_core::market_data::context::{
    CurveState, MarketContextState, MARKET_CONTEXT_STATE_VERSION,
};
use finstack_valuations::instruments::json_loader::InstrumentJson;

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
        schema_version: CreditFactorModel::SCHEMA_VERSION.to_owned(),
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
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        CURVE_ID,
    )
    .unwrap();
    bond.attributes = Attributes::new().with_meta("credit::issuer_id", format!("BENCH-BOND-{idx}"));
    bond
}

fn build_market_state(as_of: Date, rate: f64) -> MarketContextState {
    MarketContextState {
        version: MARKET_CONTEXT_STATE_VERSION,
        curves: vec![CurveState::Discount(build_flat_curve(as_of, rate))],
        fx: None,
        surfaces: vec![],
        prices: BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![],
        collateral: BTreeMap::new(),
        fx_delta_vol_surfaces: vec![],
        hierarchy: None,
        vol_cubes: vec![],
    }
}

struct CreditFixture {
    /// Pre-built attribution specs (one per bond).
    specs: Vec<AttributionEnvelope>,
}

impl CreditFixture {
    fn new(n: usize) -> Self {
        let as_of_t0 = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = Date::from_calendar_date(2025, Month::January, 16).unwrap();
        let market_t0 = build_market_state(as_of_t0, BASE_RATE);
        let market_t1 = build_market_state(as_of_t1, BASE_RATE + SHIFT_BP / 10_000.0);
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

criterion_group!(
    benches,
    bench_attribution_scale,
    bench_attribution_with_credit_model
);
criterion_main!(benches);
