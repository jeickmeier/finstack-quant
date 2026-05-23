use super::whatif::WhatIfEngine;
use crate::error::{Error, Result};
use crate::types::PositionId;
use finstack_factor_model::FactorId;

/// Declarative factor-aware constraint surface for optimization.
///
/// **Status: planned.** The `WhatIfEngine::optimize` method currently returns
/// an error for all inputs. These constraint variants are published so
/// downstream code can build constraint sets ahead of the optimizer landing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FactorConstraint {
    /// Cap the absolute portfolio risk attributed to a single factor.
    MaxFactorRisk {
        /// Constrained factor identifier.
        factor_id: FactorId,
        /// Maximum allowed absolute risk contribution.
        max_risk: f64,
    },
    /// Cap a factor's share of total portfolio risk.
    MaxFactorConcentration {
        /// Constrained factor identifier.
        factor_id: FactorId,
        /// Maximum allowed share in the closed interval `[0, 1]`.
        max_fraction: f64,
    },
    /// Force the portfolio to be neutral to the named factor.
    FactorNeutral {
        /// Factor identifier whose net exposure should be neutralized.
        factor_id: FactorId,
    },
}

/// Placeholder result shape for factor-constrained optimization.
///
/// **Status: planned.** Published so downstream code can type-check against
/// the eventual optimizer return type.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FactorOptimizationResult {
    /// Optimized position quantities keyed by position identifier.
    pub optimized_quantities: Vec<(PositionId, f64)>,
}

impl<'a> WhatIfEngine<'a> {
    /// Stub optimization entry point until the quadratic factor-risk problem is supported.
    ///
    /// # Arguments
    ///
    /// * `_constraints` - Factor constraints the future optimizer would enforce.
    ///
    /// # Returns
    ///
    /// Currently always returns an error because covariance-based factor-risk
    /// optimization is not yet implemented.
    ///
    /// # Errors
    ///
    /// Always returns [`crate::Error::OptimizationError`] with an explicit
    /// unsupported-operation message.
    pub fn optimize(&self, _constraints: &[FactorConstraint]) -> Result<FactorOptimizationResult> {
        Err(Error::optimization_error(
            "Factor-constrained optimization is not supported yet because the current LP optimizer cannot represent covariance-based factor-risk constraints",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor_model::{FactorModel, FactorModelBuilder};
    use crate::position::{Position, PositionUnit};
    use crate::sensitivity::{FactorSensitivityEngine, SensitivityMatrix};
    use crate::test_utils::build_test_market_at;
    use crate::types::DUMMY_ENTITY_ID;
    use crate::Portfolio;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
    use finstack_core::types::{Attributes, CurveId};
    use finstack_factor_model::matching::{DependencyFilter, MappingRule, MatchingConfig};
    use finstack_factor_model::{
        CurveType, DependencyType, FactorCovarianceMatrix, FactorDefinition, FactorId,
        FactorModelConfig, FactorType, MarketMapping, PricingMode, RiskMeasure, UnmatchedPolicy,
    };
    use finstack_valuations::instruments::Instrument;
    use finstack_valuations::instruments::MarketDependencies;
    use finstack_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_factor_constraint_variants_construct() {
        let c1 = FactorConstraint::MaxFactorRisk {
            factor_id: FactorId::new("Rates"),
            max_risk: 100.0,
        };
        let c2 = FactorConstraint::FactorNeutral {
            factor_id: FactorId::new("Credit"),
        };

        assert!(matches!(c1, FactorConstraint::MaxFactorRisk { .. }));
        assert!(matches!(c2, FactorConstraint::FactorNeutral { .. }));
    }

    #[test]
    fn test_optimize_returns_explicit_unsupported_error() {
        let setup = build_test_model();
        assert!(setup.is_some());
        let Some((model, portfolio, market)) = setup else {
            return;
        };
        let base_result = model.analyze(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(base_result.is_ok());
        let Ok(base) = base_result else {
            return;
        };
        let sensitivities_result =
            model.compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(sensitivities_result.is_ok());
        let Ok(sensitivities) = sensitivities_result else {
            return;
        };

        let result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .optimize(&[FactorConstraint::FactorNeutral {
                factor_id: FactorId::new("Rates"),
            }]);

        assert!(result.is_err());
    }

    fn build_test_model() -> Option<(FactorModel, Portfolio, MarketContext)> {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return None;
        };

        let model_result = FactorModelBuilder::new()
            .config(FactorModelConfig {
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
                    attribute_filter: finstack_factor_model::AttributeFilter::default(),
                    factor_id: FactorId::new("Rates"),
                }]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(FixedSensitivityEngine)
            .build();
        assert!(model_result.is_ok());
        let Ok(model) = model_result else {
            return None;
        };

        let position_result = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "inst-1",
            Arc::new(MockInstrument::new("inst-1", "USD-OIS", 100.0)),
            2.0,
            PositionUnit::Units,
        );
        assert!(position_result.is_ok());
        let Ok(position) = position_result else {
            return None;
        };

        let portfolio_result = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build();
        assert!(portfolio_result.is_ok());
        let Ok(portfolio) = portfolio_result else {
            return None;
        };

        Some((
            model,
            portfolio,
            build_test_market_at(date!(2024 - 01 - 01)),
        ))
    }

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        discount_curve: CurveId,
        scale: f64,
    }

    impl MockInstrument {
        fn new(id: &str, discount_curve: &str, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::default(),
                discount_curve: CurveId::new(discount_curve),
                scale,
            }
        }
    }

    finstack_valuations::impl_empty_cashflow_provider!(
        MockInstrument,
        finstack_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for MockInstrument {
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
            market: &MarketContext,
            _as_of: finstack_core::dates::Date,
        ) -> finstack_core::Result<Money> {
            let pv = market.get_discount(self.discount_curve.as_str())?.zero(1.0) * self.scale;
            Ok(Money::new(pv, Currency::USD))
        }

        fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
            let mut dependencies = MarketDependencies::new();
            dependencies
                .curves
                .discount_curves
                .push(self.discount_curve.clone());
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
            _as_of: finstack_core::dates::Date,
        ) -> finstack_core::Result<SensitivityMatrix> {
            let mut matrix = SensitivityMatrix::zeros(
                vec!["pos-1".into()],
                factors.iter().map(|factor| factor.id.clone()).collect(),
            );
            if !factors.is_empty() {
                matrix.set_delta(0, 0, 10.0);
            }
            Ok(matrix)
        }
    }
}
