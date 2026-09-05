//! Benchmarks for the factor-model hot paths:
//!
//! * `FullRepricingEngine::compute_pnl_profiles` — parallelized across factors
//!   (previously a serial factor loop).
//! * `ParametricDecomposer::decompose` — closed-form factor risk decomposition.
//!
//! All are gated behind `autobenches = false`, so they only run when listed in
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
use finstack_quant_models::factor::risk::ParametricDecomposer;
use finstack_quant_models::factor::{
    BumpSizeConfig, FactorCovarianceMatrix, FactorDefinition, FactorId, FactorType, MarketMapping,
    PricingMode, RiskMeasure, SensitivityMatrix, UnmatchedPolicy,
};
use finstack_quant_portfolio::factor_model::FactorModelBuilder;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine,
};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_portfolio::Portfolio;
use finstack_quant_valuations::instruments::{Attributes, Instrument, MarketDependencies};
use finstack_quant_valuations::pricer::InstrumentType;
use std::sync::Arc;
use time::macros::date;

// Minimal curve-sensitive instrument (PV = curve zero-rate at a fixed tenor).

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
        Ok(Money::new(self.raw_value(market)?, Currency::USD).expect("valid money fixture"))
    }
    fn base_value_raw(&self, market: &MarketContext, _as_of: Date) -> Result<f64> {
        self.raw_value(market)
    }
    fn base_value_raw_with_currency(
        &self,
        market: &MarketContext,
        _as_of: Date,
    ) -> Result<(f64, Currency)> {
        Ok((self.raw_value(market)?, Currency::USD))
    }
    fn market_dependencies(&self) -> Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        dependencies
            .curves
            .discount_curves
            .push(self.curve_id.clone());
        Ok(dependencies)
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

fn sparse_repricing_market(as_of: Date, n_factors: usize) -> MarketContext {
    (0..n_factors).fold(MarketContext::new(), |market, factor_index| {
        let curve = DiscountCurve::builder(format!("USD-CURVE-{factor_index}"))
            .base_date(as_of)
            .interp(InterpStyle::MonotoneConvex)
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.80), (10.0, 0.60)])
            .build()
            .expect("bench: sparse discount curve should build");
        market.insert(curve)
    })
}

fn sparse_repricing_instruments(n_positions: usize, n_factors: usize) -> Vec<CurveZeroInstrument> {
    (0..n_positions)
        .map(|position_index| CurveZeroInstrument {
            id: format!("SPARSE_POS_{position_index}"),
            attributes: Attributes::new(),
            curve_id: CurveId::new(format!("USD-CURVE-{}", position_index % n_factors)),
            tenor_years: 1.0 + (position_index % 9) as f64,
            scale: 10_000.0,
        })
        .collect()
}

fn sparse_repricing_factors(n_factors: usize) -> Vec<FactorDefinition> {
    (0..n_factors)
        .map(|factor_index| FactorDefinition {
            id: FactorId::new(format!("sparse_rates_{factor_index}")),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new(format!("USD-CURVE-{factor_index}"))],
                units: finstack_quant_core::market_data::bumps::BumpUnits::RateBp,
            },
            description: None,
        })
        .collect()
}

fn bench_full_repricing(c: &mut Criterion) {
    let as_of = date!(2025 - 01 - 01);
    let market = repricing_market(as_of);
    let engine = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
        .expect("five points form a valid full-repricing grid");

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
                        .compute_pnl_profiles(&positions, &factors, &market, as_of, Currency::USD)
                        .expect("bench: pnl profiles should compute");
                    std::hint::black_box(profiles);
                });
            },
        );

        let sparse_market = sparse_repricing_market(as_of, n_factors);
        let sparse_instruments = sparse_repricing_instruments(n_positions, n_factors);
        let sparse_positions: Vec<(String, &dyn Instrument, f64)> = sparse_instruments
            .iter()
            .map(|instrument| (instrument.id.clone(), instrument as &dyn Instrument, 1.0))
            .collect();
        let sparse_factors = sparse_repricing_factors(n_factors);
        group.bench_with_input(
            BenchmarkId::new(
                "dependency_routed_profiles",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let profiles = engine
                        .compute_pnl_profiles(
                            &sparse_positions,
                            &sparse_factors,
                            &sparse_market,
                            as_of,
                            Currency::USD,
                        )
                        .expect("bench: dependency-routed pnl profiles should compute");
                    std::hint::black_box(profiles);
                });
            },
        );
    }
    group.finish();
}

fn bench_delta_based_sensitivities(c: &mut Criterion) {
    let as_of = date!(2025 - 01 - 01);
    let market = repricing_market(as_of);
    let engine = DeltaBasedEngine::new(BumpSizeConfig::default());

    let mut group = c.benchmark_group("delta_based_sensitivities");
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
                "compute_sensitivities",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let matrix = engine
                        .compute_sensitivities(&positions, &factors, &market, as_of, Currency::USD)
                        .expect("bench: delta sensitivities should compute");
                    std::hint::black_box(matrix);
                });
            },
        );

        let sparse_market = sparse_repricing_market(as_of, n_factors);
        let sparse_instruments = sparse_repricing_instruments(n_positions, n_factors);
        let sparse_positions: Vec<(String, &dyn Instrument, f64)> = sparse_instruments
            .iter()
            .map(|instrument| (instrument.id.clone(), instrument as &dyn Instrument, 1.0))
            .collect();
        let sparse_factors = sparse_repricing_factors(n_factors);
        group.bench_with_input(
            BenchmarkId::new(
                "dependency_routed_sensitivities",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let matrix = engine
                        .compute_sensitivities(
                            &sparse_positions,
                            &sparse_factors,
                            &sparse_market,
                            as_of,
                            Currency::USD,
                        )
                        .expect("bench: dependency-routed delta sensitivities should compute");
                    std::hint::black_box(matrix);
                });
            },
        );
    }
    group.finish();
}

// Direct factor stress (only the work consumed by the public workflow is timed).

fn factor_stress_model() -> finstack_quant_portfolio::factor_model::FactorModel {
    use finstack_quant_models::factor::matching::{DependencyFilter, MappingRule, MatchingConfig};
    use finstack_quant_models::factor::{CurveType, DependencyType, FactorModelConfig};

    let factor_id = FactorId::new("rates_stress");
    let covariance = FactorCovarianceMatrix::new(vec![factor_id.clone()], vec![0.04])
        .expect("bench: factor covariance should build");
    FactorModelBuilder::new()
        .config(FactorModelConfig {
            factors: vec![FactorDefinition {
                id: factor_id.clone(),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new(CURVE_ID)],
                    units: finstack_quant_core::market_data::bumps::BumpUnits::RateBp,
                },
                description: None,
            }],
            covariance,
            matching: MatchingConfig::MappingTable(vec![MappingRule {
                dependency_filter: DependencyFilter {
                    dependency_type: Some(DependencyType::Discount),
                    curve_type: Some(CurveType::Discount),
                    id: Some(CURVE_ID.to_string()),
                },
                attribute_filter: finstack_quant_models::factor::AttributeFilter::default(),
                factor_id,
            }]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: Some(BumpSizeConfig::default()),
            unmatched_policy: Some(UnmatchedPolicy::Residual),
        })
        .build()
        .expect("bench: factor model should build")
}

fn factor_stress_portfolio(n_positions: usize, as_of: Date) -> Portfolio {
    repricing_instruments(n_positions)
        .into_iter()
        .fold(
            Portfolio::builder("FACTOR_STRESS")
                .base_currency(Currency::USD)
                .as_of(as_of),
            |builder, instrument| {
                let position_id = instrument.id.clone();
                let instrument_id = instrument.id.clone();
                builder.position(
                    Position::new(
                        position_id,
                        DUMMY_ENTITY_ID,
                        instrument_id,
                        Arc::new(instrument),
                        1.0,
                        PositionUnit::Units,
                    )
                    .expect("bench: position should build"),
                )
            },
        )
        .build()
        .expect("bench: portfolio should build")
}

fn bench_factor_stress(c: &mut Criterion) {
    let as_of = date!(2025 - 01 - 01);
    let market = repricing_market(as_of);
    let model = factor_stress_model();
    let stresses = vec![(FactorId::new("rates_stress"), 10.0)];
    let mut group = c.benchmark_group("factor_stress");
    group.sample_size(10);

    for &n_positions in &[256_usize, 1024] {
        let portfolio = factor_stress_portfolio(n_positions, as_of);

        group.bench_with_input(
            BenchmarkId::new("factor_stress_pnl", format!("{n_positions}p_x_1f")),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let result = model
                        .factor_stress_pnl(
                            &portfolio,
                            &market,
                            as_of,
                            std::hint::black_box(&stresses),
                        )
                        .expect("bench: factor stress P&L should succeed");
                    std::hint::black_box(result);
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("factor_stress", format!("{n_positions}p_x_1f")),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let result = model
                        .factor_stress(&portfolio, &market, as_of, std::hint::black_box(&stresses))
                        .expect("bench: factor stress should succeed");
                    std::hint::black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_factor_model_analyze(c: &mut Criterion) {
    let as_of = date!(2025 - 01 - 01);
    let market = repricing_market(as_of);
    let model = factor_stress_model();
    let mut group = c.benchmark_group("factor_model_analyze");
    group.sample_size(10);

    for &n_positions in &[256_usize, 1024] {
        let portfolio = factor_stress_portfolio(n_positions, as_of);

        group.bench_with_input(
            BenchmarkId::new("analyze", format!("{n_positions}p_x_1f")),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let result = model
                        .analyze(&portfolio, &market, as_of)
                        .expect("bench: factor-model analyze should succeed");
                    std::hint::black_box(result);
                });
            },
        );
    }
    group.finish();
}

// Monte Carlo factor decomposition (variance / volatility path).

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

fn bench_parametric_decomposition(c: &mut Criterion) {
    let measure = RiskMeasure::Variance;
    let mut group = c.benchmark_group("parametric_factor_decomposition");
    group.sample_size(10);

    for &(n_positions, n_factors) in &[(256_usize, 24_usize), (1024, 24), (1024, 64)] {
        let position_ids: Vec<String> = (0..n_positions).map(|i| format!("P{i}")).collect();
        let factor_ids: Vec<finstack_quant_models::factor::FactorId> = (0..n_factors)
            .map(|i| FactorId::new(format!("F{i}")))
            .collect();
        let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
        for p in 0..n_positions {
            for f in 0..n_factors {
                matrix.set_delta(p, f, 100.0 + (p as f64) * 0.01 - (f as f64) * 0.5);
            }
        }
        let covariance = diagonal_covariance(n_factors);
        let decomposer = ParametricDecomposer;

        group.bench_with_input(
            BenchmarkId::new(
                "decompose_variance",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let decomposition = decomposer
                        .decompose(&matrix, &covariance, &measure)
                        .expect("bench: parametric decomposition should succeed");
                    std::hint::black_box(decomposition);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_full_repricing,
    bench_delta_based_sensitivities,
    bench_factor_stress,
    bench_factor_model_analyze,
    bench_parametric_decomposition
);
criterion_main!(benches);
