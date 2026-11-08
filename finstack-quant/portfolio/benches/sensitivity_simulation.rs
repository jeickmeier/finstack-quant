//! Benchmarks for the factor-model hot paths:
//!
//! * `FullRepricingEngine::compute_pnl_profiles` — parallelized across factors
//!   (previously a serial factor loop).
//! * `decompose_factors` — closed-form factor risk decomposition.
//!
//! All are gated behind `autobenches = false`, so they only run when listed in
//! `Cargo.toml`.

use std::any::Any;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::CurveId;
use finstack_quant_factor::risk::decompose_factors;
use finstack_quant_factor::{
    BumpSizeConfig, FactorCovarianceMatrix, FactorDefinition, FactorId, FactorType, MarketMapping,
    RiskMeasure, SensitivityMatrix, UnmatchedPolicy,
};
use finstack_quant_portfolio::Portfolio;
use finstack_quant_portfolio::factor_model::FactorModel;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine,
};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use finstack_quant_valuations::pricer::InstrumentType;
use std::sync::Arc;
use time::macros::date;

#[path = "../../valuations/tests/performance/context_bump_fixture.rs"]
mod context_bump_fixture;

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
        Ok(market.get_discount(self.curve_id.as_str())?.zero_rate(
            self.tenor_years,
            finstack_quant_core::math::Compounding::Continuous,
        ) * self.scale)
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
    let mut group = c.benchmark_group("full_repricing_pnl_profiles");
    group.sample_size(10);

    let n_factors = 24;
    for &n_positions in &[256_usize, 1024] {
        group.bench_with_input(
            BenchmarkId::new(
                "compute_pnl_profiles",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let market = repricing_market(as_of);
                let engine = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
                    .expect("five points form a valid full-repricing grid");

                let instruments = repricing_instruments(n_positions);
                let positions: Vec<(String, &dyn Instrument, f64)> = instruments
                    .iter()
                    .map(|inst| (inst.id.clone(), inst as &dyn Instrument, 1.0))
                    .collect();
                let factors = repricing_factors(n_factors);

                b.iter(|| {
                    let profiles = engine
                        .compute_pnl_profiles(&positions, &factors, &market, as_of, Currency::USD)
                        .expect("bench: pnl profiles should compute");
                    std::hint::black_box(profiles);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new(
                "dependency_routed_profiles",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let engine = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
                    .expect("five points form a valid full-repricing grid");

                let sparse_market = sparse_repricing_market(as_of, n_factors);
                let sparse_instruments = sparse_repricing_instruments(n_positions, n_factors);
                let sparse_positions: Vec<(String, &dyn Instrument, f64)> = sparse_instruments
                    .iter()
                    .map(|instrument| (instrument.id.clone(), instrument as &dyn Instrument, 1.0))
                    .collect();
                let sparse_factors = sparse_repricing_factors(n_factors);
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
    let mut group = c.benchmark_group("delta_based_sensitivities");
    group.sample_size(10);

    let n_factors = 24;
    for &n_positions in &[256_usize, 1024] {
        group.bench_with_input(
            BenchmarkId::new(
                "compute_sensitivities",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let market = repricing_market(as_of);
                let engine = DeltaBasedEngine::new(BumpSizeConfig::default());

                let instruments = repricing_instruments(n_positions);
                let positions: Vec<(String, &dyn Instrument, f64)> = instruments
                    .iter()
                    .map(|inst| (inst.id.clone(), inst as &dyn Instrument, 1.0))
                    .collect();
                let factors = repricing_factors(n_factors);

                b.iter(|| {
                    let matrix = engine
                        .compute_sensitivities(&positions, &factors, &market, as_of, Currency::USD)
                        .expect("bench: delta sensitivities should compute");
                    std::hint::black_box(matrix);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new(
                "dependency_routed_sensitivities",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let engine = DeltaBasedEngine::new(BumpSizeConfig::default());

                let sparse_market = sparse_repricing_market(as_of, n_factors);
                let sparse_instruments = sparse_repricing_instruments(n_positions, n_factors);
                let sparse_positions: Vec<(String, &dyn Instrument, f64)> = sparse_instruments
                    .iter()
                    .map(|instrument| (instrument.id.clone(), instrument as &dyn Instrument, 1.0))
                    .collect();
                let sparse_factors = sparse_repricing_factors(n_factors);
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
    use finstack_quant_factor::{CurveType, DependencyType, FactorModelConfig};
    use finstack_quant_factor::{DependencyFilter, MappingRule, MatchingConfig};

    let factor_id = FactorId::new("rates_stress");
    let covariance = FactorCovarianceMatrix::new(vec![factor_id.clone()], vec![0.04])
        .expect("bench: factor covariance should build");
    FactorModel::from_config(FactorModelConfig {
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
            attribute_filter: finstack_quant_factor::AttributeFilter::default(),
            factor_id,
        }]),

        risk_measure: RiskMeasure::Variance,
        bump_config: Some(BumpSizeConfig::default()),
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
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
    let mut group = c.benchmark_group("factor_stress");
    group.sample_size(10);

    for &n_positions in &[256_usize, 1024] {
        group.bench_with_input(
            BenchmarkId::new("factor_stress_pnl", format!("{n_positions}p_x_1f")),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let market = repricing_market(as_of);
                let model = factor_stress_model();
                let stresses = vec![(FactorId::new("rates_stress"), 10.0)];
                let portfolio = factor_stress_portfolio(n_positions, as_of);

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
                let as_of = date!(2025 - 01 - 01);
                let market = repricing_market(as_of);
                let model = factor_stress_model();
                let stresses = vec![(FactorId::new("rates_stress"), 10.0)];
                let portfolio = factor_stress_portfolio(n_positions, as_of);

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
    let mut group = c.benchmark_group("factor_model_analyze");
    group.sample_size(10);

    for &n_positions in &[256_usize, 1024] {
        group.bench_with_input(
            BenchmarkId::new("analyze", format!("{n_positions}p_x_1f")),
            &n_positions,
            |b, _| {
                let as_of = date!(2025 - 01 - 01);
                let market = repricing_market(as_of);
                let model = factor_stress_model();
                let portfolio = factor_stress_portfolio(n_positions, as_of);

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
    let mut group = c.benchmark_group("parametric_factor_decomposition");
    group.sample_size(10);

    for &(n_positions, n_factors) in &[(256_usize, 24_usize), (1024, 24), (1024, 64)] {
        group.bench_with_input(
            BenchmarkId::new(
                "decompose_variance",
                format!("{n_positions}p_x_{n_factors}f"),
            ),
            &n_positions,
            |b, _| {
                let measure = RiskMeasure::Variance;
                let position_ids: Vec<String> = (0..n_positions).map(|i| format!("P{i}")).collect();
                let factor_ids: Vec<finstack_quant_factor::FactorId> = (0..n_factors)
                    .map(|i| FactorId::new(format!("F{i}")))
                    .collect();
                let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
                for p in 0..n_positions {
                    for f in 0..n_factors {
                        matrix.set_delta(p, f, 100.0 + (p as f64) * 0.01 - (f as f64) * 0.5);
                    }
                }
                let covariance = diagonal_covariance(n_factors);

                b.iter(|| {
                    let decomposition = decompose_factors(&matrix, &covariance, &measure)
                        .expect("bench: parametric decomposition should succeed");
                    std::hint::black_box(decomposition);
                });
            },
        );
    }
    group.finish();
}

fn bench_context_bump_equity_delta(c: &mut Criterion) {
    c.bench_function(
        "context_bump_consumer/equity_delta_engine/curves_1024",
        |b| {
            let market = context_bump_fixture::market(1024);
            let instrument = context_bump_fixture::option();
            let engine = DeltaBasedEngine::new(BumpSizeConfig {
                rates_bp: 1.0,
                credit_bp: 1.0,
                equity_pct: 1.0,
                fx_pct: 1.0,
                vol_points: 1.0,
                overrides: Default::default(),
            });
            let positions = [("CCM-POS".to_string(), &instrument as &dyn Instrument, 1.0)];
            let factors = [FactorDefinition {
                id: FactorId::new("CCM-EQUITY-PCT"),
                factor_type: FactorType::Equity,
                market_mapping: MarketMapping::EquitySpot {
                    tickers: vec!["SPX-SPOT".into()],
                },
                description: None,
            }];
            let compute = || {
                engine
                    .compute_sensitivities(
                        std::hint::black_box(&positions),
                        std::hint::black_box(&factors),
                        std::hint::black_box(&market),
                        context_bump_fixture::as_of(),
                        Currency::USD,
                    )
                    .expect("generic EquitySpot factor sensitivity")
            };
            let checked = compute();
            assert_eq!(checked.n_positions(), 1);
            assert_eq!(checked.n_factors(), 1);
            assert_eq!(checked.position_ids(), &["CCM-POS".to_string()]);
            let expected = (context_bump_fixture::reference_price(101.0, 0.20)
                - context_bump_fixture::reference_price(99.0, 0.20))
                / 2.0;
            let actual = checked.delta(0, 0).expect("one sensitivity");
            assert!(actual.is_finite());
            assert!((actual - expected).abs() <= 1e-9 * expected.abs().max(1.0));
            drop(checked);
            b.iter(|| std::hint::black_box(compute()));
        },
    );
}

criterion_group!(
    benches,
    bench_full_repricing,
    bench_delta_based_sensitivities,
    bench_factor_stress,
    bench_factor_model_analyze,
    bench_parametric_decomposition,
    bench_context_bump_equity_delta
);
criterion_main!(benches);
