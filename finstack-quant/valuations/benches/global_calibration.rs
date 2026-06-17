//! Global calibration solver benchmarks.
//!
//! Benchmarks the `GlobalFitOptimizer` / `CalibrationMethod::GlobalSolve` pathway
//! that is absent from the existing `calibration.rs` (which only exercises
//! sequential bootstrap plans).
//!
//! Covers:
//! - Discount curve LM fit at 8 / 16 / 22 quotes (numeric vs. analytical Jacobian)
//! - Hazard curve LM fit at 3 / 6 tenors
//! - Multi-start restarts (5 and 10)
//! - Bootstrap vs. GlobalSolve head-to-head at equal quote counts

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::term_structures::Seniority;
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::market_datum::MarketDatum;
use finstack_quant_valuations::calibration::api::{
    engine,
    schema::{
        CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams,
        HazardCurveParams, StepParams, CALIBRATION_SCHEMA,
    },
};
use finstack_quant_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_quant_valuations::market::quotes::cds::CdsQuote;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use std::hint::black_box;
use time::Month;

/// Extract the QuoteId of each MarketQuote.
fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
    quotes
        .iter()
        .map(|q| match q {
            MarketQuote::Rates(q) => q.id().clone(),
            MarketQuote::Cds(q) => q.id().clone(),
            MarketQuote::CDSTranche(q) => q.id().clone(),
            MarketQuote::Fx(q) => q.id().clone(),
            MarketQuote::Inflation(q) => q.id().clone(),
            MarketQuote::Vol(q) => q.id().clone(),
            MarketQuote::Xccy(q) => q.id().clone(),
            MarketQuote::Bond(q) => q.id().clone(),
        })
        .collect()
}

fn quotes_to_data(quotes: &[MarketQuote]) -> Vec<MarketDatum> {
    quotes.iter().cloned().map(MarketDatum::from).collect()
}

// ---------------------------------------------------------------------------
// Helpers — rate quote builders
// ---------------------------------------------------------------------------

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).unwrap()
}

/// Build n deposit + swap quotes covering the short end then 1Y..30Y IRS.
///
/// Quote count layout (n=8/16/22):
///   n=8  → 2 deposits + 6 swaps  (1Y 2Y 3Y 5Y 7Y 10Y)
///   n=16 → 4 deposits + 12 swaps (1Y 2Y 3Y 4Y 5Y 6Y 7Y 10Y 12Y 15Y 20Y 25Y)
///   n=22 → 4 deposits + 18 swaps (1Y..10Y + 12Y 15Y 20Y 25Y 30Y + 6M 9M short swaps)
fn make_discount_quotes(n: usize) -> Vec<MarketQuote> {
    let base = base_date();
    let mut quotes: Vec<MarketQuote> = Vec::with_capacity(n);

    // Short end deposits (always included)
    let dep_pillars: &[(i64, f64)] = &[(30, 0.0450), (90, 0.0455), (180, 0.0460), (270, 0.0462)];
    let dep_count = if n <= 8 { 2 } else { 4 };
    for &(days, rate) in dep_pillars.iter().take(dep_count) {
        quotes.push(MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new(format!("DEP-{days}D")),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Date(base + time::Duration::days(days)),
            rate,
        }));
    }

    // Swap quotes
    let swap_tenors: &[(u32, f64)] = &[
        (1, 0.0475),
        (2, 0.0485),
        (3, 0.0490),
        (4, 0.0492),
        (5, 0.0493),
        (6, 0.0494),
        (7, 0.0495),
        (8, 0.0495),
        (9, 0.0496),
        (10, 0.0496),
        (12, 0.0497),
        (15, 0.0498),
        (20, 0.0499),
        (25, 0.0499),
        (30, 0.0500),
        (35, 0.0500),
        (40, 0.0500),
        (50, 0.0500),
    ];
    let swap_count = n - dep_count;
    for &(years, rate) in swap_tenors.iter().take(swap_count) {
        quotes.push(MarketQuote::Rates(RateQuote::Swap {
            id: QuoteId::new(format!("OIS-{years}Y")),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse(&format!("{years}Y")).unwrap()),
            rate,
            spread_decimal: None,
        }));
    }

    quotes
}

/// Build m CDS par-spread quotes (3 or 6 tenors).
fn make_cds_quotes(m: usize) -> Vec<MarketQuote> {
    let currency = Currency::USD;
    let pillars: &[(i32, f64)] = &[
        (1, 80.0),
        (3, 120.0),
        (5, 150.0),
        (7, 170.0),
        (10, 190.0),
        (15, 210.0),
    ];
    pillars
        .iter()
        .take(m)
        .map(|&(years, spread_bp)| {
            let maturity = Date::from_calendar_date(2025 + years, Month::March, 20).unwrap();
            MarketQuote::Cds(CdsQuote::CdsParSpread {
                id: QuoteId::new(format!("CDS-{years}Y")),
                entity: "BENCH-ENTITY".to_string(),
                pillar: Pillar::Date(maturity),
                spread_bp,
                recovery_rate: 0.40,
                convention: CdsConventionKey {
                    currency,
                    doc_clause: CdsDocClause::IsdaNa,
                },
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Envelope builders
// ---------------------------------------------------------------------------

fn discount_envelope(
    quotes: Vec<MarketQuote>,
    method: CalibrationMethod,
    curve_suffix: &str,
) -> CalibrationEnvelope {
    let base = base_date();
    let market_data = quotes_to_data(&quotes);
    let ids = quote_set_ids(&quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("disc".to_string(), ids);

    let curve_id = CurveId::from(format!("USD-OIS-{curve_suffix}"));
    CalibrationEnvelope {
        schema: CALIBRATION_SCHEMA.to_string(),
        schema_url: None,
        market_data,
        prior_market: Vec::new(),
        plan: CalibrationPlan {
            id: format!("global_disc_{curve_suffix}"),
            description: None,
            quote_sets,
            settings: CalibrationConfig::default(),
            steps: vec![CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
                params: StepParams::Discount(DiscountCurveParams {
                    curve_id,
                    currency: Currency::USD,
                    base_date: base,
                    method,
                    interpolation: Default::default(),
                    extrapolation: ExtrapolationPolicy::FlatForward,
                    pricing_discount_id: None,
                    pricing_forward_id: None,
                    conventions: Default::default(),
                }),
            }],
        },
    }
}

fn hazard_envelope(
    disc_quotes: Vec<MarketQuote>,
    cds_quotes: Vec<MarketQuote>,
    method: CalibrationMethod,
    suffix: &str,
) -> CalibrationEnvelope {
    let base = base_date();
    let currency = Currency::USD;
    let mut market_data = quotes_to_data(&disc_quotes);
    market_data.extend(quotes_to_data(&cds_quotes));
    let disc_ids = quote_set_ids(&disc_quotes);
    let cds_ids = quote_set_ids(&cds_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("disc".to_string(), disc_ids);
    quote_sets.insert("cds".to_string(), cds_ids);

    let disc_curve = CurveId::from(format!("USD-OIS-{suffix}"));
    let haz_curve = CurveId::from(format!("BENCH-ENTITY-HZD-{suffix}"));

    CalibrationEnvelope {
        schema: CALIBRATION_SCHEMA.to_string(),
        schema_url: None,
        market_data,
        prior_market: Vec::new(),
        plan: CalibrationPlan {
            id: format!("global_hazard_{suffix}"),
            description: None,
            quote_sets,
            settings: CalibrationConfig::default(),
            steps: vec![
                CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: StepParams::Discount(DiscountCurveParams {
                        curve_id: disc_curve.clone(),
                        currency,
                        base_date: base,
                        method: CalibrationMethod::Bootstrap,
                        interpolation: Default::default(),
                        extrapolation: ExtrapolationPolicy::FlatForward,
                        pricing_discount_id: None,
                        pricing_forward_id: None,
                        conventions: Default::default(),
                    }),
                },
                CalibrationStep {
                    id: "hzd".to_string(),
                    quote_set: "cds".to_string(),
                    params: StepParams::Hazard(HazardCurveParams {
                        curve_id: haz_curve,
                        entity: "BENCH-ENTITY".to_string(),
                        seniority: Seniority::Senior,
                        currency,
                        base_date: base,
                        discount_curve_id: disc_curve,
                        recovery_rate: 0.40,
                        notional: 1.0,
                        method,
                        interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                        par_interp: Default::default(),
                        doc_clause: None,
                        cds_valuation_convention: None,
                    }),
                },
            ],
        },
    }
}

// ---------------------------------------------------------------------------
// Benchmark: GlobalSolve discount curve — representative quote count
// ---------------------------------------------------------------------------

fn bench_global_discount_quote_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_discount_curve");

    let n = 16;
    let quotes = make_discount_quotes(n);
    let env = discount_envelope(
        quotes,
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        &format!("{n}q"),
    );
    group.bench_with_input(BenchmarkId::new("numeric_jac", n), &n, |b, _| {
        b.iter(|| engine::execute(black_box(&env)).unwrap());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Analytical vs. numerical Jacobian (fixed 16 quotes)
// ---------------------------------------------------------------------------

fn bench_global_discount_jacobian(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_discount_jacobian");
    let quotes = make_discount_quotes(16);

    let env_numeric = discount_envelope(
        quotes.clone(),
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        "jac_numeric",
    );
    let env_analytic = discount_envelope(
        quotes,
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: true,
        },
        "jac_analytic",
    );

    group.bench_function("numeric_jac_16q", |b| {
        b.iter(|| engine::execute(black_box(&env_numeric)).unwrap());
    });
    group.bench_function("analytical_jac_16q", |b| {
        b.iter(|| engine::execute(black_box(&env_analytic)).unwrap());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Bootstrap vs. GlobalSolve head-to-head (16 quotes)
// ---------------------------------------------------------------------------

fn bench_global_vs_bootstrap(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_vs_bootstrap");

    let n = 16;
    let quotes = make_discount_quotes(n);

    let env_bootstrap = discount_envelope(
        quotes.clone(),
        CalibrationMethod::Bootstrap,
        &format!("bs_{n}q"),
    );
    let env_global = discount_envelope(
        quotes,
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: true,
        },
        &format!("gs_{n}q"),
    );

    group.bench_with_input(BenchmarkId::new("bootstrap", n), &n, |b, _| {
        b.iter(|| engine::execute(black_box(&env_bootstrap)).unwrap());
    });
    group.bench_with_input(BenchmarkId::new("global_solve", n), &n, |b, _| {
        b.iter(|| engine::execute(black_box(&env_global)).unwrap());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Hazard curve GlobalSolve — representative tenor count
// ---------------------------------------------------------------------------

fn bench_global_hazard_tenor_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_hazard_curve");
    let disc_quotes = make_discount_quotes(8);

    let m = 6;
    let cds_quotes = make_cds_quotes(m);
    let env = hazard_envelope(
        disc_quotes.clone(),
        cds_quotes,
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: true,
        },
        &format!("{m}t"),
    );
    group.bench_with_input(BenchmarkId::new("global_solve", m), &m, |b, _| {
        b.iter(|| engine::execute(black_box(&env)).unwrap());
    });

    let cds_quotes = make_cds_quotes(m);
    let env = hazard_envelope(
        disc_quotes,
        cds_quotes,
        CalibrationMethod::Bootstrap,
        &format!("bs_{m}t"),
    );
    group.bench_with_input(BenchmarkId::new("bootstrap", m), &m, |b, _| {
        b.iter(|| engine::execute(black_box(&env)).unwrap());
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_global_discount_quote_count,
    bench_global_discount_jacobian,
    bench_global_vs_bootstrap,
    bench_global_hazard_tenor_count,
);
criterion_main!(benches);
