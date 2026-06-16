//! Stochastic revolving credit facility (RCF) Monte Carlo pricing benchmarks.
//!
//! Benchmarks the 3-factor MC engine for revolving credit facilities:
//!  - Factor 1: Utilization (mean-reverting OU)
//!  - Factor 2: Short rate (Hull-White 1F, for floating rates)
//!  - Factor 3: Credit spread/hazard rate (CIR)
//!  - Correlated via Cholesky decomposition
//!
//! The existing `fi_misc_pricing.rs` only covers the deterministic pricing path.
//! This file benchmarks the stochastic MC engine that is the hot path in practice.
//!
//! Scenarios:
//! - Path count scaling (100 / 500 / 1K / 5K paths) — main MC cost
//! - Facility tenor scaling (1Y / 3Y / 5Y) — step count impact
//! - Single-factor (utilization only) vs. 3-factor (+ rate + credit) cost
//! - Correlated vs. independent factors

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
    RevolvingCreditFees, StochasticUtilizationSpec, UtilizationProcess,
};
use finstack_quant_valuations::instruments::Instrument;
use std::hint::black_box;
use time::Month;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_market(base: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.952),
            (3.0, 0.873),
            (5.0, 0.800),
            (7.0, 0.735),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

/// Build a stochastic RCF with given maturity and path count.
/// `mc_config` controls the factor model (1-factor vs. 3-factor).
fn make_rcf(
    as_of: Date,
    maturity: Date,
    num_paths: usize,
    mc_config: Option<McConfig>,
) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RCF-BENCH".into())
        .commitment_amount(Money::new(50_000_000.0, Currency::USD))
        .drawn_amount(Money::new(25_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.55,
                    speed: 1.5,
                    volatility: 0.15,
                },
                num_paths,
                seed: Some(42),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.40)
        .build()
        .unwrap()
}

/// Single-factor McConfig (utilization only, constant zero credit spread).
fn single_factor_mc() -> McConfig {
    McConfig {
        recovery_rate: 0.40,
        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
        interest_rate_process: None,
        correlation_matrix: None,
        util_credit_corr: None,
    }
}

/// Three-factor McConfig: utilization OU + CIR credit + independent.
fn three_factor_mc() -> McConfig {
    McConfig {
        recovery_rate: 0.40,
        credit_spread_process: CreditSpreadProcessSpec::Cir {
            kappa: 1.0,
            theta: 0.015,
            sigma: 0.08,
            initial: 0.012,
        },
        interest_rate_process: None, // fixed-rate facility — no stochastic rate
        correlation_matrix: None,
        util_credit_corr: Some(-0.30),
    }
}

/// Three-factor McConfig with explicit correlation matrix.
fn three_factor_correlated_mc() -> McConfig {
    McConfig {
        recovery_rate: 0.40,
        credit_spread_process: CreditSpreadProcessSpec::Cir {
            kappa: 1.0,
            theta: 0.015,
            sigma: 0.08,
            initial: 0.012,
        },
        interest_rate_process: None,
        correlation_matrix: Some([[1.0, 0.10, -0.30], [0.10, 1.0, -0.10], [-0.30, -0.10, 1.0]]),
        util_credit_corr: None,
    }
}

// ---------------------------------------------------------------------------
// Benchmark: path count scaling (3-factor, 3Y facility)
// ---------------------------------------------------------------------------

fn bench_rcf_mc_path_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("rcf_mc_paths");
    let as_of = base_date();
    let maturity = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let market = create_market(as_of);

    for n_paths in [100usize, 500, 1_000, 5_000] {
        let facility = make_rcf(as_of, maturity, n_paths, Some(three_factor_mc()));

        group.throughput(Throughput::Elements(n_paths as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n_paths), &n_paths, |b, _| {
            b.iter(|| {
                black_box(&facility)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: tenor scaling (1K paths, 3-factor)
// ---------------------------------------------------------------------------

fn bench_rcf_mc_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("rcf_mc_tenor");
    let as_of = base_date();
    let market = create_market(as_of);

    for (label, years) in [("1Y", 1i32), ("3Y", 3), ("5Y", 5)] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let facility = make_rcf(as_of, maturity, 1_000, Some(three_factor_mc()));

        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&facility)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: 1-factor vs. 3-factor vs. 3-factor-correlated (1K paths, 3Y)
// ---------------------------------------------------------------------------

fn bench_rcf_mc_factor_model(c: &mut Criterion) {
    let mut group = c.benchmark_group("rcf_mc_factors");
    let as_of = base_date();
    let maturity = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let market = create_market(as_of);
    const PATHS: usize = 1_000;

    let f1 = make_rcf(as_of, maturity, PATHS, Some(single_factor_mc()));
    let f3 = make_rcf(as_of, maturity, PATHS, Some(three_factor_mc()));
    let f3c = make_rcf(as_of, maturity, PATHS, Some(three_factor_correlated_mc()));

    for (label, facility) in [
        ("1_factor", &f1),
        ("3_factor", &f3),
        ("3_factor_corr", &f3c),
    ] {
        group.bench_function(label, |b| {
            b.iter(|| {
                black_box(facility)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rcf_mc_path_count,
    bench_rcf_mc_tenor,
    bench_rcf_mc_factor_model,
);
criterion_main!(benches);
