//! Benchmarks for the two hot paths touched by the recent performance work:
//!
//! * `FullRepricingEngine::compute_pnl_profiles` — parallelized across factors
//!   (previously a serial factor loop).
//! * `SimulationDecomposer::decompose` — its variance decomposition now reads
//!   the scenario-major P&L/shock buffers in contiguous passes instead of one
//!   column-strided pass per factor.
//!
//! Both are gated behind `autobenches = false`, so they only run when listed in
//! `Cargo.toml`.

use std::any::Any;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_quant_factor_model::{
    BumpSizeConfig, FactorCovarianceMatrix, FactorDefinition, FactorId, FactorType, MarketMapping,
    RiskMeasure,
};
use finstack_quant_portfolio::factor_model::{RiskDecomposer, SimulationDecomposer};
use finstack_quant_portfolio::sensitivity::FullRepricingEngine;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;
use time::macros::date;

// ---------------------------------------------------------------------------
// Minimal curve-sensitive instrument (PV = curve zero-rate at a fixed tenor).
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CurveZeroInstrument {
    id: String,
    attributes: Attributes,
    curve_id: CurveId,
    tenor_years: f64,
    scale: f64,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    CurveZeroInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl CurveZeroInstrument {
    fn raw_value(&self, market: &MarketContext) -> Result<f64> {
        Ok(market
            .get_discount(self.curve_id.as_str())?
            .zero(self.tenor_years)
            * self.scale)
    }
}

impl Instrument for CurveZeroInstrument {
    fn id(&self) -> &str {
        &self.id
    }
    fn key(&self) -> InstrumentType {
        InstrumentType::Bond
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }
    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(Money::new(self.raw_value(market)?, Currency::USD))
    }
    fn value_raw(&self, market: &MarketContext, _as_of: Date) -> Result<f64> {
        self.raw_value(market)
    }
}

const CURVE_ID: &str = "USD-OIS";

fn repricing_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder(CURVE_ID)
        .base_date(as_of)
        .interp(InterpStyle::MonotoneConvex)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .expect("bench: discount curve should build");
    MarketContext::new().insert(curve)
}

fn repricing_instruments(n_positions: usize) -> Vec<CurveZeroInstrument> {
    (0..n_positions)
        .map(|i| CurveZeroInstrument {
            id: format!("POS_{i}"),
            attributes: Attributes::new(),
            curve_id: CurveId::new(CURVE_ID),
            // Spread tenors across the curve so positions are not identical.
            tenor_years: 1.0 + (i % 9) as f64,
            scale: 10_000.0,
        })
        .collect()
}

fn repricing_factors(n_factors: usize) -> Vec<FactorDefinition> {
    (0..n_factors)
        .map(|i| FactorDefinition {
            id: FactorId::new(format!("rates_{i}")),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new(CURVE_ID)],
                units: finstack_quant_core::market_data::bumps::BumpUnits::RateBp,
            },
            description: None,
        })
        .collect()
}

fn bench_full_repricing(c: &mut Criterion) {
    let as_of = date!(2025 - 01 - 01);
    let market = repricing_market(as_of);
    let engine = FullRepricingEngine::new(BumpSizeConfig::default(), 5); // 5-point grid

    let mut group = c.benchmark_group("full_repricing_pnl_profiles");
    group.sample_size(10);

    let n_factors = 24;
    for &n_positions in &[256_usize, 1024] {
        let instruments = repricing_instruments(n_positions);
        let positions: Vec<(String, &dyn Instrument, f64)> = instruments
            .iter()
            .map(|inst| (inst.id.clone(), inst as &dyn Instrument, 1.0))
            .collect();
        let factors = repricing_factors(n_factors);

        group.bench_with_input(
            BenchmarkId::new(
                "compute_pnl_profiles",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let profiles = engine
                        .compute_pnl_profiles(&positions, &factors, &market, as_of)
                        .expect("bench: pnl profiles should compute");
                    std::hint::black_box(profiles);
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Monte Carlo factor decomposition (variance / volatility path).
// ---------------------------------------------------------------------------

fn diagonal_covariance(n_factors: usize) -> FactorCovarianceMatrix {
    let factor_ids: Vec<FactorId> = (0..n_factors)
        .map(|i| FactorId::new(format!("F{i}")))
        .collect();
    // Diagonal => trivially PSD; Cholesky succeeds. Off-diagonals stay zero so
    // the benchmark isolates the per-scenario decomposition cost, which scales
    // with n_scenarios * n_factors regardless of covariance content.
    let mut data = vec![0.0_f64; n_factors * n_factors];
    for i in 0..n_factors {
        data[i * n_factors + i] = 0.04; // 20% vol per factor
    }
    FactorCovarianceMatrix::new(factor_ids, data).expect("bench: covariance should build")
}

fn decomposition_sensitivities(n_factors: usize) -> SensitivityMatrix {
    // A few positions, each with a deterministic delta per factor.
    let position_ids: Vec<String> = (0..8).map(|i| format!("P{i}")).collect();
    let factor_ids: Vec<FactorId> = (0..n_factors)
        .map(|i| FactorId::new(format!("F{i}")))
        .collect();
    let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
    for p in 0..8 {
        for f in 0..n_factors {
            matrix.set_delta(p, f, 100.0 + (p as f64) - (f as f64) * 0.5);
        }
    }
    matrix
}

fn bench_mc_decomposition(c: &mut Criterion) {
    let n_scenarios = 16_384;
    let measure = RiskMeasure::Volatility;

    let mut group = c.benchmark_group("mc_factor_decomposition");
    group.sample_size(10);

    for &n_factors in &[32_usize, 64] {
        let sensitivities = decomposition_sensitivities(n_factors);
        let covariance = diagonal_covariance(n_factors);
        let decomposer = SimulationDecomposer::new(n_scenarios, 42);

        group.bench_with_input(
            BenchmarkId::new(
                "decompose_volatility",
                format!("{n_factors}f_x_{n_scenarios}sc"),
            ),
            &n_factors,
            |b, _| {
                b.iter(|| {
                    let decomposition = decomposer
                        .decompose(&sensitivities, &covariance, &measure)
                        .expect("bench: decomposition should succeed");
                    std::hint::black_box(decomposition);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_full_repricing, bench_mc_decomposition);
criterion_main!(benches);
