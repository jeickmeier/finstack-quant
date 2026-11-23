use super::engine::*;
use crate::Portfolio;
use crate::position::{Position, PositionUnit};
use crate::sensitivity::{DeltaBasedEngine, FactorSensitivityEngine};
use crate::types::{DUMMY_ENTITY_ID, PositionId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId};
use finstack_quant_factor::SensitivityMatrix;
use finstack_quant_factor::{
    BumpSizeConfig, CurveType, DependencyType, FactorCovarianceMatrix, FactorDefinition, FactorId,
    FactorModelConfig, FactorType, MarketMapping, MatchingConfig, RiskMeasure, UnmatchedPolicy,
};
use finstack_quant_factor::{DependencyFilter, MappingRule};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::MarketDependencies;
use finstack_quant_valuations::pricer::InstrumentType;
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use time::macros::date;

fn simple_config() -> FactorModelConfig {
    let covariance_result = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return unreachable_config();
    };

    FactorModelConfig {
        factors: vec![FactorDefinition {
            id: FactorId::new("Rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }],
        covariance,
        matching: MatchingConfig::MappingTable(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Discount),
                curve_type: Some(CurveType::Discount),
                id: None,
            },
            attribute_filter: finstack_quant_factor::AttributeFilter::default(),
            factor_id: FactorId::new("Rates"),
        }]),

        risk_measure: RiskMeasure::Variance,
        bump_config: Some(BumpSizeConfig::default()),
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    }
}

fn unreachable_config() -> FactorModelConfig {
    FactorModelConfig {
        factors: Vec::new(),
        covariance: FactorCovarianceMatrix::new(Vec::new(), Vec::new())
            .expect("empty covariance matrix is valid"),
        matching: MatchingConfig::MappingTable(Vec::new()),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: None,
    }
}

#[test]
fn test_from_config_from_config_exposes_factors() {
    let build_result = FactorModel::from_config(simple_config());
    assert!(build_result.is_ok());
    let Ok(model) = build_result else {
        return;
    };

    assert_eq!(model.factors().len(), 1);
    assert_eq!(model.factors()[0].id, FactorId::new("Rates"));
}

#[test]
fn test_from_config_rejects_covariance_axes_not_aligned_to_factors() {
    let covariance_result = FactorCovarianceMatrix::new(
        vec![FactorId::new("Credit"), FactorId::new("Rates")],
        vec![0.09, 0.01, 0.01, 0.04],
    );
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return;
    };

    let result = FactorModel::from_config(FactorModelConfig {
        factors: vec![
            FactorDefinition {
                id: FactorId::new("Rates"),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
            FactorDefinition {
                id: FactorId::new("Credit"),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("ACME-HAZARD")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ],
        covariance,
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    });

    assert!(result.is_err());
}

#[test]
fn test_from_config_rejects_matching_factor_ids_not_declared_by_config() {
    let mut config = simple_config();
    config.matching = MatchingConfig::MappingTable(vec![MappingRule {
        dependency_filter: DependencyFilter::default(),
        attribute_filter: finstack_quant_factor::AttributeFilter::default(),
        factor_id: FactorId::new("MissingFactor"),
    }]);

    let Err(error) = FactorModel::from_config(config) else {
        panic!("builder must validate matching factor IDs");
    };
    assert!(error.to_string().contains("MissingFactor"), "{error}");
}

#[test]
fn test_from_config_matches_builder_and_validates() {
    let model = FactorModel::from_config(simple_config()).expect("valid config");
    assert_eq!(model.factors().len(), simple_config().factors.len());

    let mut config = simple_config();
    config.matching = MatchingConfig::MappingTable(vec![MappingRule {
        dependency_filter: DependencyFilter::default(),
        attribute_filter: finstack_quant_factor::AttributeFilter::default(),
        factor_id: FactorId::new("MissingFactor"),
    }]);
    let Err(error) = FactorModel::from_config(config) else {
        panic!("from_config must validate matching factor IDs");
    };
    assert!(error.to_string().contains("MissingFactor"), "{error}");
}

#[test]
fn test_assign_factors_collects_matches_and_unmatched() {
    let build_result = FactorModel::from_config(simple_config());
    assert!(build_result.is_ok());
    let Ok(model) = build_result else {
        return;
    };

    let position_result = Position::new(
        "pos-1",
        DUMMY_ENTITY_ID,
        "inst-1",
        Arc::new(MockInstrument::new(
            "inst-1",
            "USD-OIS",
            vec!["AAPL".into()],
        )),
        2.0,
        PositionUnit::Units,
    );
    assert!(position_result.is_ok());
    let Ok(position) = position_result else {
        return;
    };

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .position(position)
        .build()
        .expect("test should succeed");

    let report_result = model.assign_factors(&portfolio, &MarketContext::new());
    assert!(report_result.is_ok());
    let Ok(report) = report_result else {
        return;
    };

    assert_eq!(report.assignments.len(), 1);
    assert_eq!(report.assignments[0].position_id, PositionId::new("pos-1"));
    assert_eq!(report.assignments[0].mappings.len(), 1);
    assert_eq!(report.assignments[0].mappings[0].1, FactorId::new("Rates"));
    assert_eq!(report.unmatched.len(), 1);
    assert_eq!(report.unmatched[0].position_id, PositionId::new("pos-1"));
}

#[test]
fn test_analyze_uses_custom_sensitivity_engine() {
    let covariance_result = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return;
    };

    let sensitivity_calls = Arc::new(AtomicUsize::new(0));
    let model_result = FactorModel::from_config(FactorModelConfig {
        factors: vec![FactorDefinition {
            id: FactorId::new("Rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }],
        covariance,
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .map(|model| {
        model.with_custom_sensitivity_engine(CountingSensitivityEngine {
            calls: Arc::clone(&sensitivity_calls),
        })
    });
    assert!(model_result.is_ok());
    let Ok(model) = model_result else {
        return;
    };

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .build()
        .expect("test should succeed");
    let analysis_result =
        model.analyze_with_sensitivities(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
    assert!(analysis_result.is_ok());
    let Ok((actual, sensitivities)) = analysis_result else {
        return;
    };

    assert_eq!(actual.total_risk, 0.0);
    assert_eq!(actual.measure, RiskMeasure::Variance);
    assert_eq!(sensitivities.n_factors(), 1);
    assert_eq!(
        sensitivity_calls.load(Ordering::SeqCst),
        1,
        "combined analysis must run the sensitivity engine exactly once"
    );
}

#[test]
fn test_analyze_fails_when_strict_policy_has_unmatched_dependencies() {
    let covariance_result = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return;
    };

    let model_result = FactorModel::from_config(FactorModelConfig {
        factors: vec![FactorDefinition {
            id: FactorId::new("Rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }],
        covariance,
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Strict),
    })
    .map(|model| model.with_custom_sensitivity_engine(FixedSensitivityEngine));
    assert!(model_result.is_ok());
    let Ok(model) = model_result else {
        return;
    };

    let position_result = Position::new(
        "pos-1",
        DUMMY_ENTITY_ID,
        "inst-1",
        Arc::new(MockInstrument::new("inst-1", "USD-OIS", vec![])),
        1.0,
        PositionUnit::Units,
    );
    assert!(position_result.is_ok());
    let Ok(position) = position_result else {
        return;
    };

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .position(position)
        .build()
        .expect("test should succeed");

    let analysis_result = model.analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
    assert!(analysis_result.is_err());
}

#[derive(Clone)]
struct MockInstrument {
    id: String,
    attributes: Attributes,
    discount_curve: CurveId,
    spots: Vec<String>,
    raw_value_calls: Option<Arc<AtomicUsize>>,
}

impl MockInstrument {
    fn new(id: &str, discount_curve: &str, spots: Vec<String>) -> Self {
        Self {
            id: id.to_string(),
            attributes: Attributes::default(),
            discount_curve: CurveId::new(discount_curve),
            spots,
            raw_value_calls: None,
        }
    }

    fn with_raw_value_calls(mut self, calls: Arc<AtomicUsize>) -> Self {
        self.raw_value_calls = Some(calls);
        self
    }
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    MockInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for MockInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
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

    fn base_value(
        &self,
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(Money::from((100_i64, Currency::USD)))
    }

    fn base_value_raw(
        &self,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(calls) = &self.raw_value_calls {
            calls.fetch_add(1, Ordering::SeqCst);
        }
        Ok(self.base_value(market, as_of)?.amount())
    }

    fn base_value_raw_with_currency(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, Currency)> {
        Ok((self.base_value_raw(market, as_of)?, Currency::USD))
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        dependencies
            .curves
            .discount_curves
            .push(self.discount_curve.clone());
        dependencies
            .market_scalar_ids
            .extend(self.spots.iter().cloned());
        Ok(dependencies)
    }
}

struct FixedSensitivityEngine;

impl FactorSensitivityEngine for FixedSensitivityEngine {
    fn compute_sensitivities(
        &self,
        _positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
        _base_currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<SensitivityMatrix> {
        Ok(SensitivityMatrix::zeros(
            Vec::new(),
            factors.iter().map(|factor| factor.id.clone()).collect(),
        ))
    }
}

struct CountingSensitivityEngine {
    calls: Arc<AtomicUsize>,
}

impl FactorSensitivityEngine for CountingSensitivityEngine {
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
        _base_currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<SensitivityMatrix> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(SensitivityMatrix::zeros(
            positions.iter().map(|(id, _, _)| id.clone()).collect(),
            factors.iter().map(|factor| factor.id.clone()).collect(),
        ))
    }
}

/// Returns a sensitivity engine that places known deltas for a single
/// position so the downstream `decompose_factors` can be verified.
struct KnownDeltaEngine {
    deltas: Vec<f64>,
}

impl FactorSensitivityEngine for KnownDeltaEngine {
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
        _base_currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<SensitivityMatrix> {
        let position_ids: Vec<String> = positions.iter().map(|(id, _, _)| id.clone()).collect();
        let factor_ids: Vec<_> = factors.iter().map(|f| f.id.clone()).collect();
        let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
        for (j, &delta) in self.deltas.iter().enumerate() {
            matrix.set_delta(0, j, delta);
        }
        Ok(matrix)
    }
}

struct WeightEchoEngine;

impl FactorSensitivityEngine for WeightEchoEngine {
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
        _base_currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<SensitivityMatrix> {
        let position_ids: Vec<String> = positions.iter().map(|(id, _, _)| id.clone()).collect();
        let factor_ids: Vec<_> = factors.iter().map(|f| f.id.clone()).collect();
        let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
        for (i, (_, _, weight)) in positions.iter().enumerate() {
            matrix.set_delta(i, 0, *weight);
        }
        Ok(matrix)
    }
}

#[test]
fn test_b1_percentage_position_uses_scale_factor_for_sensitivities() {
    let model = FactorModel::from_config(simple_config())
        .map(|model| model.with_custom_sensitivity_engine(WeightEchoEngine))
        .expect("model should build");

    let position = Position::new(
        "pos-100pct",
        DUMMY_ENTITY_ID,
        "inst-1",
        Arc::new(MockInstrument::new("inst-1", "USD-OIS", vec![])),
        100.0,
        PositionUnit::Percentage,
    )
    .expect("percentage position should build");

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .position(position)
        .build()
        .expect("portfolio should build");

    let decomp = model
        .analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01))
        .expect("analysis should succeed");

    // B-1: 100% should be economically equivalent to a scale factor of 1,
    // not a raw engine weight of 100. With Σ=0.04 and S=1, variance is 0.04.
    assert!(
        (decomp.total_risk - 0.04).abs() < 1e-12,
        "percentage position risk should use scale factor, got {}",
        decomp.total_risk
    );
}

#[test]
fn test_analyze_end_to_end_single_factor_with_real_decomposer() {
    let covariance_result = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return;
    };

    let model_result = FactorModel::from_config(FactorModelConfig {
        factors: vec![FactorDefinition {
            id: FactorId::new("Rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }],
        covariance,
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .map(|model| model.with_custom_sensitivity_engine(KnownDeltaEngine { deltas: vec![10.0] }));
    assert!(model_result.is_ok());
    let Ok(model) = model_result else {
        return;
    };

    let position_result = Position::new(
        "pos-1",
        DUMMY_ENTITY_ID,
        "inst-1",
        Arc::new(MockInstrument::new("inst-1", "USD-OIS", vec![])),
        1.0,
        PositionUnit::Units,
    );
    assert!(position_result.is_ok());
    let Ok(position) = position_result else {
        return;
    };

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .position(position)
        .build()
        .expect("test should succeed");

    let result = model.analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
    assert!(result.is_ok());
    let Ok(decomp) = result else {
        return;
    };

    // S=[10], Σ=[[0.04]] → variance = 10² × 0.04 = 4.0
    let expected_variance = 4.0;
    assert!(
        (decomp.total_risk - expected_variance).abs() < 1e-12,
        "total_risk {} != expected {}",
        decomp.total_risk,
        expected_variance,
    );
    assert_eq!(decomp.measure, RiskMeasure::Variance);
    assert_eq!(decomp.factor_contributions.len(), 1);
    assert!(
        (decomp.factor_contributions[0].absolute_risk - expected_variance).abs() < 1e-12,
        "factor absolute_risk {} != expected {}",
        decomp.factor_contributions[0].absolute_risk,
        expected_variance,
    );
}

#[test]
fn test_analyze_end_to_end_two_factors_with_real_decomposer() {
    let covariance_result = FactorCovarianceMatrix::new(
        vec![FactorId::new("Rates"), FactorId::new("Credit")],
        vec![0.04, 0.03, 0.03, 0.09],
    );
    assert!(covariance_result.is_ok());
    let Ok(covariance) = covariance_result else {
        return;
    };

    let model_result = FactorModel::from_config(FactorModelConfig {
        factors: vec![
            FactorDefinition {
                id: FactorId::new("Rates"),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
            FactorDefinition {
                id: FactorId::new("Credit"),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("ACME-HAZARD")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ],
        covariance,
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .map(|model| {
        model.with_custom_sensitivity_engine(KnownDeltaEngine {
            deltas: vec![10.0, 5.0],
        })
    });
    assert!(model_result.is_ok());
    let Ok(model) = model_result else {
        return;
    };

    let position_result = Position::new(
        "pos-1",
        DUMMY_ENTITY_ID,
        "inst-1",
        Arc::new(MockInstrument::new("inst-1", "USD-OIS", vec![])),
        1.0,
        PositionUnit::Units,
    );
    assert!(position_result.is_ok());
    let Ok(position) = position_result else {
        return;
    };

    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .position(position)
        .build()
        .expect("test should succeed");

    let result = model.analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
    assert!(result.is_ok());
    let Ok(decomp) = result else {
        return;
    };

    // S=[10,5], Σ=[[0.04,0.03],[0.03,0.09]]
    // Σ*S^T = [0.04*10+0.03*5, 0.03*10+0.09*5] = [0.55, 0.75]
    // Variance = S * Σ * S^T = 10*0.55 + 5*0.75 = 9.25
    let expected_variance = 9.25;
    assert!(
        (decomp.total_risk - expected_variance).abs() < 1e-12,
        "total_risk {} != expected {}",
        decomp.total_risk,
        expected_variance,
    );
    assert_eq!(decomp.factor_contributions.len(), 2);

    // Euler contributions: c_k = S_k * (Σ * S^T)_k = S_k * sum_j Σ_kj * S_j
    let rates_contrib = 10.0 * 0.55; // 5.5
    let credit_contrib = 5.0 * 0.75; // 3.75
    assert!(
        (decomp.factor_contributions[0].absolute_risk - rates_contrib).abs() < 1e-12,
        "Rates absolute_risk {} != expected {}",
        decomp.factor_contributions[0].absolute_risk,
        rates_contrib,
    );
    assert!(
        (decomp.factor_contributions[1].absolute_risk - credit_contrib).abs() < 1e-12,
        "Credit absolute_risk {} != expected {}",
        decomp.factor_contributions[1].absolute_risk,
        credit_contrib,
    );
}

fn canonical_credit_bond(
    curve_id: CurveId,
) -> finstack_quant_valuations::instruments::fixed_income::bond::Bond {
    use finstack_quant_factor::ISSUER_ID_META_KEY;
    let mut bond = finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
        "BOND-ISSUER-B",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("canonical bond should build");
    bond.credit_curve_id = Some(curve_id);
    bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
    bond
}

pub(crate) fn credit_market(as_of: Date, curve_id: CurveId) -> MarketContext {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, Seniority};
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.25_f64).exp()),
            (10.0, (-0.50_f64).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    use finstack_quant_calibration::api::{
        engine, market_datum::MarketDatum, prior_market::PriorMarketObject, schema::*,
    };
    use finstack_quant_calibration::quotes::{CdsQuote, MarketQuote, Pillar, QuoteId};
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    let quotes: Vec<MarketQuote> = [1, 5, 10]
        .into_iter()
        .map(|years| {
            MarketQuote::Cds(CdsQuote::CdsParSpread {
                id: QuoteId::new(format!("CDS-{years}")),
                entity: "ISSUER-B".to_string(),
                pillar: Pillar::Date(
                    Date::from_calendar_date(as_of.year() + years, time::Month::March, 20)
                        .expect("maturity"),
                ),
                spread_bp: 100.0,
                recovery_rate: 0.4,
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::Cr14,
                },
            })
        })
        .collect();
    let quote_ids = quotes
        .iter()
        .map(|quote| QuoteId::new(quote.id()))
        .collect();
    let request = CalibrationEnvelope::new(
        CalibrationPlan {
            id: "credit-factor-test".into(),
            description: None,
            quote_sets: indexmap::IndexMap::from([("credit".into(), quote_ids)]),
            settings: Default::default(),
            steps: vec![CalibrationStep {
                id: "hazard".into(),
                quote_set: "credit".into(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id,
                    entity: "ISSUER-B".into(),
                    seniority: Seniority::Senior,
                    currency: Currency::USD,
                    base_date: as_of,
                    discount_curve_id: "USD-OIS".into(),
                    recovery_rate: 0.4,
                    notional: 1.0,
                    method: Default::default(),
                    doc_clause: Some("cr14".into()),
                    cds_valuation_convention: None,
                }),
            }],
        },
        quotes.into_iter().map(MarketDatum::from).collect(),
        vec![PriorMarketObject::DiscountCurve(discount)],
    );
    let result = engine::calibrate(&request).expect("calibrate issuer spreads");
    MarketContext::try_from(result.result.final_market).expect("calibrated market")
}

#[test]
fn credit_exposures_use_spread_quotes_and_reporting_currency() {
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    let as_of = date!(2024 - 01 - 01);
    let id = CurveId::new("ISSUER-B-HAZ");
    let fx = Arc::new(SimpleFxProvider::new());
    fx.set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
        .unwrap();
    let market = credit_market(as_of, id.clone()).insert_fx(FxMatrix::new(fx, None));
    let bumped = bump_credit_spreads(&market, &id, 1.0).unwrap();
    let base = market.get_hazard(id.as_str()).unwrap();
    let base_recipe = base.hazard_calibration().unwrap();
    let bumped_recipe = bumped.hazard_calibration().unwrap();
    for (old_input, new_input) in base_recipe
        .spread_risk_inputs()
        .iter()
        .zip(bumped_recipe.spread_risk_inputs().iter())
    {
        let old = old_input
            .quote
            .get("spread_bp")
            .and_then(|v| v.as_f64())
            .unwrap();
        let new = new_input
            .quote
            .get("spread_bp")
            .and_then(|v| v.as_f64())
            .unwrap();
        assert!(
            (new - old - 1.0).abs() < 1e-8,
            "factor shock must move CDS spreads by one bp"
        );
    }
    let bond = canonical_credit_bond(id.clone());
    let usd = CreditExposureMatrix::new(&market, Currency::USD)
        .exposure(0, &bond, 1.0, as_of, &id, 1.0)
        .unwrap();
    let eur = CreditExposureMatrix::new(&market, Currency::EUR)
        .exposure(0, &bond, 1.0, as_of, &id, 1.0)
        .unwrap();
    assert!(usd.abs() > 1.0);
    assert!((usd - 1.1 * eur).abs() < 1e-7);
    let raw = finstack_quant_core::market_data::term_structures::HazardCurve::builder("RAW")
        .base_date(as_of)
        .knots([(1.0, 0.01), (5.0, 0.01)])
        .recovery_rate(0.4)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .unwrap();
    let error = bump_credit_spreads(
        &market.try_insert(raw).expect("insert market curve"),
        &CurveId::new("RAW"),
        1.0,
    )
    .unwrap_err();
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
fn credit_bump_contexts_are_reused_by_curve_and_bump() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let mut contexts = CreditBumpContexts::new(&market);

    {
        let _ = contexts
            .get(&curve_id, 1.0)
            .expect("first credit bump context");
    }
    {
        let _ = contexts
            .get(&curve_id, 1.0)
            .expect("reused credit bump context");
    }
    assert_eq!(contexts.contexts.len(), 1);

    {
        let _ = contexts
            .get(&curve_id, 5.0)
            .expect("different bump context");
    }
    assert_eq!(contexts.contexts.len(), 2);
}

#[test]
fn credit_exposure_matrix_reuses_endpoint_pvs_for_identical_requests() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let instrument = MockInstrument::new("credit-count", "USD-OIS", vec![])
        .with_raw_value_calls(Arc::clone(&calls));
    let mut exposures = CreditExposureMatrix::new(&market, Currency::USD);

    let first = exposures
        .exposure(3, &instrument, 1.0, as_of, &curve_id, 1.0)
        .expect("first exposure");
    let repeated = exposures
        .exposure(3, &instrument, 1.0, as_of, &curve_id, 1.0)
        .expect("reused exposure");
    assert_eq!(first, repeated);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(exposures.bump_contexts.contexts.len(), 1);
    assert_eq!(exposures.exposures.len(), 1);

    exposures
        .exposure(3, &instrument, 1.0, as_of, &curve_id, 5.0)
        .expect("different bump exposure");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(exposures.bump_contexts.contexts.len(), 2);
    assert_eq!(exposures.exposures.len(), 2);
}

#[test]
fn credit_hierarchy_sensitivities_scale_cs01_by_calibrated_betas() {
    use finstack_quant_factor::CreditHierarchicalConfig;
    use finstack_quant_factor::credit::hierarchy::{
        AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
        IssuerBetas, IssuerTags,
    };
    use std::collections::BTreeMap;

    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let mut tags = BTreeMap::new();
    tags.insert("rating".to_string(), "B".to_string());
    let issuer_row = IssuerBetaRow {
        issuer_id: finstack_quant_core::types::IssuerId::new("ISSUER-B"),
        tags: IssuerTags(tags),
        mode: IssuerBetaMode::IssuerBeta,
        betas: IssuerBetas {
            pc: 5.0,
            levels: vec![7.0],
        },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.0,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
        level_fit_quality: vec![],
        spread_duration: 1.0,
    };
    let factors = vec![
        FactorDefinition {
            id: FactorId::new("credit::generic"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("credit::level0::Rating::B"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
    ];
    let covariance = FactorCovarianceMatrix::new(
        factors.iter().map(|f| f.id.clone()).collect(),
        vec![1.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    let model = FactorModel::from_config(FactorModelConfig {
        factors,
        covariance,
        matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
            dependency_filter: Default::default(),
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            issuer_betas: vec![issuer_row],
            require_issuer_id: false,
        }),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .unwrap();
    let position = Position::new(
        "pos-credit",
        DUMMY_ENTITY_ID,
        "inst-credit",
        Arc::new(canonical_credit_bond(curve_id)),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(position)
        .build()
        .unwrap();

    let sensitivities = model
        .compute_sensitivities(&portfolio, &market, as_of)
        .expect("sensitivities");

    let generic = sensitivities.delta(0, 0).expect("in range");
    let rating = sensitivities.delta(0, 1).expect("in range");
    assert!(
        generic.abs() > 1e-8,
        "canonical bond should have credit sensitivity"
    );
    // The factor model is Δs_i = β_pc·ΔG + Σ_k β_k·ΔL_k + Δε_i, so a unit
    // factor move produces a spread move of β on the issuer curve and the
    // exposure row must be CS01 · (β_pc, β_0, …). With pc = 5 and
    // level-0 β = 7 the two columns must sit in a 7:5 ratio — matching
    // the loading convention used by credit attribution.
    assert!(
        (rating - (7.0 / 5.0) * generic).abs() < 1e-10 * generic.abs().max(1.0),
        "hierarchy factor exposure must be CS01 scaled by the calibrated \
         beta: rating = {rating}, generic = {generic}"
    );
}

#[test]
fn strict_policy_rejects_dropped_credit_factor_ids() {
    use finstack_quant_core::types::Attributes;
    use finstack_quant_factor::credit::hierarchy::{
        AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
        IssuerBetas, IssuerTags,
    };
    use finstack_quant_factor::{CreditHierarchicalConfig, ISSUER_ID_META_KEY};
    use std::collections::BTreeMap;

    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("NEWCO-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let mut tags = BTreeMap::new();
    tags.insert("rating".to_string(), "B".to_string());
    // Calibrated universe knows only ISSUER-B / rating B.
    let issuer_row = IssuerBetaRow {
        issuer_id: finstack_quant_core::types::IssuerId::new("ISSUER-B"),
        tags: IssuerTags(tags),
        mode: IssuerBetaMode::BucketOnly,
        betas: IssuerBetas {
            pc: 1.0,
            levels: vec![1.0],
        },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.0,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
        level_fit_quality: vec![],
        spread_duration: 1.0,
    };
    let factors = vec![
        FactorDefinition {
            id: FactorId::new("credit::generic"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("credit::level0::Rating::B"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("rates::usd"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
    ];
    let covariance = FactorCovarianceMatrix::new(
        factors.iter().map(|f| f.id.clone()).collect(),
        vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    let build = |policy: UnmatchedPolicy| {
        FactorModel::from_config(FactorModelConfig {
            factors: factors.clone(),
            covariance: covariance.clone(),
            // Cascade: credit deps hit the hierarchy; everything else
            // (the discount curve) falls through to a catch-all rates
            // rule so the only unmatched surface is the credit drop.
            matching: MatchingConfig::Cascade(vec![
                MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                    dependency_filter: Default::default(),
                    hierarchy: CreditHierarchySpec {
                        levels: vec![HierarchyDimension::Rating],
                    },
                    issuer_betas: vec![issuer_row.clone()],
                    require_issuer_id: false,
                }),
                MatchingConfig::MappingTable(vec![finstack_quant_factor::MappingRule {
                    dependency_filter: Default::default(),
                    attribute_filter: Default::default(),
                    factor_id: FactorId::new("rates::usd"),
                }]),
            ]),

            risk_measure: RiskMeasure::Variance,
            bump_config: None,
            unmatched_policy: Some(policy),
        })
        .unwrap()
    };

    // Runtime issuer NEWCO (not calibrated) tagged rating HY: the matcher
    // emits credit::level0::Rating::HY, which is not declared in factors.
    let mut bond = canonical_credit_bond(curve_id);
    bond.attributes = Attributes::new()
        .with_meta(ISSUER_ID_META_KEY, "NEWCO")
        .with_meta("credit::rating", "HY");
    let position = Position::new(
        "pos-newco",
        DUMMY_ENTITY_ID,
        "inst-newco",
        Arc::new(bond),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(position)
        .build()
        .unwrap();

    let err = build(UnmatchedPolicy::Strict)
        .compute_sensitivities(&portfolio, &market, as_of)
        .expect_err("Strict must reject a matched-but-undeclared credit factor id");
    assert!(
        err.to_string().contains("credit::level0::Rating::HY"),
        "error must name the dropped factor id: {err}"
    );

    // Warn explicitly permits unmatched risk and surfaces a tracing warning.
    build(UnmatchedPolicy::Warn)
        .compute_sensitivities(&portfolio, &market, as_of)
        .expect("warn policy must continue");
}

#[test]
fn credit_factor_stress_scales_curve_shift_by_calibrated_beta() {
    use finstack_quant_factor::CreditHierarchicalConfig;
    use finstack_quant_factor::credit::hierarchy::{
        AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
        IssuerBetas, IssuerTags,
    };
    use std::collections::BTreeMap;

    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let mut tags = BTreeMap::new();
    tags.insert("rating".to_string(), "B".to_string());
    let issuer_row = IssuerBetaRow {
        issuer_id: finstack_quant_core::types::IssuerId::new("ISSUER-B"),
        tags: IssuerTags(tags),
        mode: IssuerBetaMode::IssuerBeta,
        betas: IssuerBetas {
            pc: 5.0,
            levels: vec![7.0],
        },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.0,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
        level_fit_quality: vec![],
        spread_duration: 1.0,
    };
    let factors = vec![
        FactorDefinition {
            id: FactorId::new("credit::generic"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("credit::level0::Rating::B"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
    ];
    let covariance = FactorCovarianceMatrix::new(
        factors.iter().map(|f| f.id.clone()).collect(),
        vec![1.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    let model = FactorModel::from_config(FactorModelConfig {
        factors,
        covariance,
        matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
            dependency_filter: Default::default(),
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            issuer_betas: vec![issuer_row],
            require_issuer_id: false,
        }),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .unwrap();
    let position = Position::new(
        "pos-credit",
        DUMMY_ENTITY_ID,
        "inst-credit",
        Arc::new(canonical_credit_bond(curve_id.clone())),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(position)
        .build()
        .unwrap();

    // A +2bp shock to the generic factor moves the issuer spread by
    // β_pc × 2bp = 10bp under the model Δs_i = β_pc·ΔG + …, so the
    // stressed market must equal a manual 10bp parallel hazard shift.
    let (stressed, _) = model
        .stressed_market_with_factor_keys(
            &portfolio,
            &market,
            as_of,
            &[(FactorId::new("credit::generic"), 2.0)],
        )
        .expect("stressed market");
    let expected =
        shift_credit_curves(&market, &[(curve_id, 1.0)], 5.0 * 2.0).expect("manual shift");

    let bond = &portfolio.positions[0].instrument;
    let stressed_value = bond.value_raw(&stressed, as_of).expect("stressed value");
    let expected_value = bond.value_raw(&expected, as_of).expect("expected value");
    let base_value = bond.value_raw(&market, as_of).expect("base value");
    assert!(
        (stressed_value - base_value).abs() > 1e-8,
        "shock must move the bond value"
    );
    assert!(
        (stressed_value - expected_value).abs() < 1e-8,
        "factor stress must shift the issuer curve by beta × shock \
         (stressed = {stressed_value}, expected = {expected_value})"
    );
}

/// Credit-hierarchical model with a single issuer (`ISSUER-B`, unit
/// betas, `adder_vol_annualized = 3.0`) used by the idiosyncratic
/// residual-variance tests.
fn credit_hierarchy_model() -> FactorModel {
    use finstack_quant_factor::CreditHierarchicalConfig;
    use finstack_quant_factor::credit::hierarchy::{
        AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
        IssuerBetas, IssuerTags,
    };
    use std::collections::BTreeMap;

    let mut tags = BTreeMap::new();
    tags.insert("rating".to_string(), "B".to_string());
    let issuer_row = IssuerBetaRow {
        issuer_id: finstack_quant_core::types::IssuerId::new("ISSUER-B"),
        tags: IssuerTags(tags),
        mode: IssuerBetaMode::IssuerBeta,
        betas: IssuerBetas {
            pc: 1.0,
            levels: vec![1.0],
        },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 3.0,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
        level_fit_quality: vec![],
        spread_duration: 1.0,
    };
    let factors = vec![
        FactorDefinition {
            id: FactorId::new("credit::generic"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("credit::level0::Rating::B"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
    ];
    let covariance = FactorCovarianceMatrix::new(
        factors.iter().map(|f| f.id.clone()).collect(),
        vec![1.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    FactorModel::from_config(FactorModelConfig {
        factors,
        covariance,
        matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
            dependency_filter: Default::default(),
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            issuer_betas: vec![issuer_row],
            require_issuer_id: false,
        }),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: Some(UnmatchedPolicy::Warn),
    })
    .unwrap()
}

/// Portfolio holding `canonical_credit_bond` positions with the given
/// `(position_id, quantity)` pairs, all on the same issuer curve.
fn credit_bond_portfolio(as_of: Date, curve_id: &CurveId, holdings: &[(&str, f64)]) -> Portfolio {
    let mut builder = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of);
    for (position_id, quantity) in holdings {
        builder = builder.position(
            Position::new(
                *position_id,
                DUMMY_ENTITY_ID,
                format!("inst-{position_id}"),
                Arc::new(canonical_credit_bond(curve_id.clone())),
                *quantity,
                PositionUnit::Units,
            )
            .unwrap(),
        );
    }
    builder.build().unwrap()
}

#[test]
fn credit_hierarchy_sensitivities_reject_finite_quantity_and_beta_overflow() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let model = credit_hierarchy_model();
    let unit_portfolio = credit_bond_portfolio(as_of, &curve_id, &[("credit", 1.0)]);
    let unit_matrix = model
        .compute_sensitivities(&unit_portfolio, &market, as_of)
        .expect("ordinary quote-space credit sensitivities remain finite");
    assert!(unit_matrix.delta(0, 0).expect("in range").is_finite());
    assert!(unit_matrix.delta(0, 0).expect("in range").abs() > 1.0);

    let huge_portfolio = credit_bond_portfolio(as_of, &curve_id, &[("credit", 1e308)]);
    let error = model
        .compute_sensitivities(&huge_portfolio, &market, as_of)
        .expect_err("finite endpoints and finite quantity must not return infinite risk");
    assert!(
        error
            .to_string()
            .contains("non-finite weighted credit sensitivity")
    );

    let mut assignments = model
        .assign_factors(&unit_portfolio, &market)
        .expect("unit credit assignment");
    for (_, _, beta) in &mut assignments.assignments[0].mappings {
        *beta = 1e308;
    }
    let mut matrix = SensitivityMatrix::zeros(
        vec!["credit".into()],
        model
            .factors
            .iter()
            .map(|factor| factor.id.clone())
            .collect(),
    );
    let mut exposures = CreditExposureMatrix::new(&market, Currency::USD);
    let error = model
        .overlay_assignment_driven_credit_sensitivities(
            &unit_portfolio,
            as_of,
            &assignments,
            &mut matrix,
            &mut exposures,
        )
        .expect_err("finite cached exposure times finite beta must not overflow the matrix");
    assert!(
        error
            .to_string()
            .contains("non-finite assignment-weighted credit sensitivity")
    );
}

#[test]
fn credit_hierarchy_analysis_adds_idiosyncratic_residual_variance() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let model = credit_hierarchy_model();
    let portfolio = credit_bond_portfolio(as_of, &curve_id, &[("pos-credit", 1.0)]);

    let decomposition = model.analyze(&portfolio, &market, as_of).expect("analysis");

    let systematic: f64 = decomposition
        .factor_contributions
        .iter()
        .map(|contribution| contribution.absolute_risk)
        .sum();
    assert_eq!(decomposition.position_residual_contributions.len(), 1);
    assert!(decomposition.residual_risk > 0.0);
    assert!(
        (systematic + decomposition.residual_risk - decomposition.total_risk).abs() < 1e-8,
        "systematic plus idiosyncratic residual variance should exhaust total variance"
    );
}

// B3 regression: the idiosyncratic diagonal is per ISSUER, not per
// position. Positions on the same issuer load the same residual shock,
// so their exposures must be netted before squaring.

#[test]
fn credit_idiosyncratic_variance_nets_positions_sharing_an_issuer() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let model = credit_hierarchy_model();

    let single = credit_bond_portfolio(as_of, &curve_id, &[("pos-1", 2.0)]);
    let split = credit_bond_portfolio(as_of, &curve_id, &[("pos-a", 1.0), ("pos-b", 1.0)]);

    let single_decomposition = model.analyze(&single, &market, as_of).expect("single");
    let split_decomposition = model.analyze(&split, &market, as_of).expect("split");

    assert!(single_decomposition.residual_risk > 0.0);
    assert_eq!(split_decomposition.position_residual_contributions.len(), 2);
    assert!(
        (single_decomposition.residual_risk - split_decomposition.residual_risk).abs()
            <= 1e-8 * single_decomposition.residual_risk,
        "splitting one holding into two rows must not change the issuer idio variance: \
         single = {}, split = {}",
        single_decomposition.residual_risk,
        split_decomposition.residual_risk
    );
}

#[test]
fn credit_idiosyncratic_variance_is_zero_for_flat_issuer_book() {
    let as_of = date!(2024 - 01 - 01);
    let curve_id = CurveId::new("ISSUER-B-HAZ");
    let market = credit_market(as_of, curve_id.clone());
    let model = credit_hierarchy_model();

    // +1 / -1 on the same issuer: the net spread exposure is zero, so
    // the issuer-level idiosyncratic shock cannot move the book.
    let flat = credit_bond_portfolio(as_of, &curve_id, &[("pos-long", 1.0), ("pos-short", -1.0)]);
    let decomposition = model.analyze(&flat, &market, as_of).expect("flat book");

    assert!(
        decomposition.residual_risk.abs() < 1e-9,
        "flat single-name book must carry zero idio risk, got {}",
        decomposition.residual_risk
    );
    assert_eq!(decomposition.position_residual_contributions.len(), 2);
    for contribution in &decomposition.position_residual_contributions {
        assert!(
            contribution.residual_variance.abs() < 1e-9,
            "flat-book rows must be present with zero residual variance, got {} for {}",
            contribution.residual_variance,
            contribution.position_id
        );
    }
}

#[test]
fn credit_index_factor_routing_matches_full_hazard_and_correlation_repricing() {
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    let (tranche, market, as_of) = crate::dependencies::tests::credit_index_fixture();
    let factors: Vec<_> = [
        ("discount", "USD-OIS", FactorType::Rates),
        ("hazard", "INDEX-HZ", FactorType::Credit),
        (
            "correlation",
            "INDEX-BC",
            FactorType::Custom("correlation".into()),
        ),
    ]
    .into_iter()
    .map(|(id, curve_id, factor_type)| FactorDefinition {
        id: FactorId::new(id),
        factor_type,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new(curve_id)],
            units: BumpUnits::RateBp,
        },
        description: None,
    })
    .collect();
    let model = FactorModel::from_config(FactorModelConfig {
        covariance: FactorCovarianceMatrix::new(
            factors.iter().map(|factor| factor.id.clone()).collect(),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        )
        .expect("covariance"),
        matching: MatchingConfig::MappingTable(
            [
                ("discount", "USD-OIS"),
                ("hazard", "INDEX-HZ"),
                ("correlation", "INDEX-BC"),
            ]
            .into_iter()
            .map(|(factor, curve)| MappingRule {
                dependency_filter: DependencyFilter {
                    id: Some(curve.into()),
                    ..Default::default()
                },
                attribute_filter: Default::default(),
                factor_id: FactorId::new(factor),
            })
            .collect(),
        ),
        factors: factors.clone(),

        risk_measure: RiskMeasure::Variance,
        bump_config: None,
        unmatched_policy: None,
    })
    .expect("factor model");
    let portfolio = Portfolio::builder("tranche-book")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(
            Position::new(
                "tranche",
                DUMMY_ENTITY_ID,
                "tranche",
                Arc::new(tranche.clone()),
                1.0,
                PositionUnit::Units,
            )
            .expect("position"),
        )
        .build()
        .expect("portfolio");
    let assignments = model
        .assign_factors(&portfolio, &market)
        .expect("assignment");
    assert!(assignments.unmatched.is_empty());
    assert_eq!(assignments.assignments[0].mappings.len(), 3);
    let matrix = model
        .compute_sensitivities(&portfolio, &market, as_of)
        .expect("sensitivities");
    let engine = DeltaBasedEngine::new(BumpSizeConfig::default());
    let positions = [("tranche".into(), &tranche as &dyn Instrument, 1.0)];
    let full_matrix = engine
        .compute_sensitivities(&positions, &factors, &market, as_of, Currency::USD)
        .expect("full matrix");
    for (factor_idx, bump_fn) in [
        (
            1,
            (|spec| MarketBump::HazardCurve {
                id: "INDEX-HZ".into(),
                spec,
            }) as fn(BumpSpec) -> MarketBump,
        ),
        (
            2,
            (|spec: BumpSpec| MarketBump::BaseCorrelation {
                id: "INDEX-BC".into(),
                spec: BumpSpec {
                    units: BumpUnits::Fraction,
                    value: spec.value / 10_000.0,
                    ..spec
                },
            }) as fn(BumpSpec) -> MarketBump,
        ),
    ] {
        let up = market
            .bump(vec![bump_fn(BumpSpec::parallel_bp(1.0))])
            .expect("up market");
        let down = market
            .bump(vec![bump_fn(BumpSpec::parallel_bp(-1.0))])
            .expect("down market");
        let manual = (tranche.value_raw(&up, as_of).expect("up PV")
            - tranche.value_raw(&down, as_of).expect("down PV"))
            / 2.0;
        assert!(manual.abs() > 1.0);
        assert!((matrix.delta(0, factor_idx).expect("in range") - manual).abs() < 1e-8);
        assert!((full_matrix.delta(0, factor_idx).expect("in range") - manual).abs() < 1e-8);
        let pnl = model
            .factor_stress_pnl(
                &portfolio,
                &market,
                as_of,
                &[(factors[factor_idx].id.clone(), 1.0)],
            )
            .expect("stress");
        let expected = tranche.value_raw(&up, as_of).expect("up PV")
            - tranche.value_raw(&market, as_of).expect("base PV");
        assert!((pnl.total_pnl - expected).abs() < 1e-8);
    }
}

#[test]
fn default_unmatched_policy_rejects_unmapped_risk() {
    let portfolio = Portfolio::builder("unmapped-book")
        .base_currency(Currency::USD)
        .as_of(date!(2025 - 01 - 01))
        .position(
            Position::new(
                "unmapped",
                DUMMY_ENTITY_ID,
                "unmapped",
                Arc::new(MockInstrument::new("unmapped", "USD-OTHER", vec![])),
                1.0,
                PositionUnit::Units,
            )
            .expect("position"),
        )
        .build()
        .expect("portfolio");
    let mut config = simple_config();
    config.unmatched_policy = None;
    config.matching = MatchingConfig::MappingTable(vec![]);
    let model = FactorModel::from_config(config).expect("strict model");
    let error = model
        .analyze(&portfolio, &MarketContext::new(), date!(2025 - 01 - 01))
        .expect_err("unmapped risk must not be returned as zero risk");
    assert!(error.to_string().contains("No factor matched"));
}
