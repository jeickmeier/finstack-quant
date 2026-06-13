//! Factor-model orchestration for portfolio-level risk decomposition.
//!
//! This file contains the top-level builder and runtime model used to connect:
//!
//! - declarative factor definitions and covariance inputs
//! - dependency-to-factor matching
//! - sensitivity generation
//! - downstream decomposition engines
//!
//! The public API is intentionally split between a configuration-time builder
//! ([`FactorModelBuilder`]) and an execution-time model ([`FactorModel`]).
//!
//! # References
//!
//! - Factor-model portfolio construction:
//!   `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - Euler-style capital allocation background:
//!   `docs/REFERENCES.md#tasche-2008-capital-allocation`
//! - Parametric VaR conventions:
//!   `docs/REFERENCES.md#jpmorgan1996RiskMetrics`

use super::assignment::{assign_position_factors, FactorAssignmentReport};
use super::whatif::WhatIfEngine;
use super::{
    ParametricDecomposer, PositionResidualContribution, ResidualContributionSource, RiskDecomposer,
    RiskDecomposition,
};
use crate::error::{Error, Result};
use crate::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine, SensitivityMatrix,
};
use crate::Portfolio;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_factor_model::matching::ISSUER_ID_META_KEY;
use finstack_factor_model::{
    BumpSizeConfig, CurveType, FactorCovarianceMatrix, FactorDefinition, FactorModelConfig,
    FactorModelError, FactorType, MarketDependency, MatchingConfig, PricingMode, RiskMeasure,
    UnmatchedPolicy,
};
use finstack_valuations::calibration::bumps::{bump_hazard_shift, BumpRequest};
use finstack_valuations::instruments::dependencies_flatten::decompose as flatten_dependencies;
use finstack_valuations::instruments::Instrument;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Builder for the top-level portfolio factor-model orchestrator.
///
/// Use this type to inject a declarative factor-model configuration and, in
/// tests, override the sensitivity engine or decomposition engine.
pub struct FactorModelBuilder {
    config: Option<FactorModelConfig>,
    #[cfg(test)]
    custom_sensitivity_engine: Option<Box<dyn FactorSensitivityEngine>>,
    #[cfg(test)]
    custom_decomposer: Option<Box<dyn RiskDecomposer>>,
}

impl FactorModelBuilder {
    /// Create an empty builder.
    ///
    /// # Returns
    ///
    /// Builder with no configuration or overrides installed yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: None,
            #[cfg(test)]
            custom_sensitivity_engine: None,
            #[cfg(test)]
            custom_decomposer: None,
        }
    }

    /// Supply the declarative factor-model configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Factor definitions, covariance matrix, matching rules, and
    ///   risk-measure configuration.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    #[must_use]
    pub fn config(mut self, config: FactorModelConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Override the sensitivity engine selected from the pricing mode (test-only).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_custom_sensitivity_engine(
        mut self,
        sensitivity_engine: impl FactorSensitivityEngine + 'static,
    ) -> Self {
        self.custom_sensitivity_engine = Some(Box::new(sensitivity_engine));
        self
    }

    /// Override the risk decomposer used by the model (test-only).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_custom_decomposer(
        mut self,
        decomposer: impl RiskDecomposer + 'static,
    ) -> Self {
        self.custom_decomposer = Some(Box::new(decomposer));
        self
    }

    /// Build the configured factor model.
    ///
    /// # Returns
    ///
    /// A fully configured [`FactorModel`] ready to assign factors, compute
    /// sensitivities, and decompose risk.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidInput`] when the configuration is missing,
    /// the risk measure is invalid, or the covariance axes do not align with
    /// the configured factors.
    pub fn build(self) -> Result<FactorModel> {
        let config = self
            .config
            .ok_or_else(|| Error::invalid_input("FactorModelConfig is required"))?;
        config.risk_measure.validate()?;
        let factor_ids: Vec<_> = config
            .factors
            .iter()
            .map(|factor| factor.id.clone())
            .collect();
        if factor_ids.as_slice() != config.covariance.factor_ids() {
            return Err(Error::invalid_input(
                "FactorModelConfig covariance axes must match factors in the same order",
            ));
        }

        let matcher = build_matcher(&config.matching);
        let bump_config = config.bump_size.clone().unwrap_or_default();
        let sensitivity_engine = {
            #[cfg(test)]
            let engine = self
                .custom_sensitivity_engine
                .unwrap_or_else(|| default_sensitivity_engine(config.pricing_mode, &bump_config));
            #[cfg(not(test))]
            let engine = default_sensitivity_engine(config.pricing_mode, &bump_config);
            engine
        };
        let decomposer: Box<dyn RiskDecomposer> = {
            #[cfg(test)]
            let d = self
                .custom_decomposer
                .unwrap_or_else(|| Box::new(ParametricDecomposer));
            #[cfg(not(test))]
            let d = Box::new(ParametricDecomposer);
            d
        };

        Ok(FactorModel {
            credit_idiosyncratic_variance: credit_idiosyncratic_variance(&config.matching),
            factors: config.factors,
            covariance: config.covariance,
            matcher,
            sensitivity_engine,
            decomposer,
            risk_measure: config.risk_measure,
            unmatched_policy: config.unmatched_policy.unwrap_or_default(),
            bump_config,
        })
    }
}

impl Default for FactorModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn build_matcher(config: &MatchingConfig) -> Box<dyn finstack_factor_model::FactorMatcher> {
    config.build_matcher()
}

fn default_sensitivity_engine(
    pricing_mode: PricingMode,
    bump_config: &BumpSizeConfig,
) -> Box<dyn FactorSensitivityEngine> {
    match pricing_mode {
        PricingMode::DeltaBased => Box::new(DeltaBasedEngine::new(bump_config.clone())),
        PricingMode::FullRepricing => Box::new(FullRepricingEngine::new(bump_config.clone(), 5)),
    }
}

/// Portfolio-level factor-model orchestrator.
///
/// A `FactorModel` owns the factor definitions, covariance matrix, and the
/// pluggable engines required to move from instrument dependencies to
/// portfolio-level risk decomposition.
pub struct FactorModel {
    credit_idiosyncratic_variance: BTreeMap<finstack_core::types::IssuerId, f64>,
    factors: Vec<FactorDefinition>,
    covariance: FactorCovarianceMatrix,
    matcher: Box<dyn finstack_factor_model::FactorMatcher>,
    sensitivity_engine: Box<dyn FactorSensitivityEngine>,
    decomposer: Box<dyn RiskDecomposer>,
    risk_measure: RiskMeasure,
    unmatched_policy: UnmatchedPolicy,
    bump_config: BumpSizeConfig,
}

impl FactorModel {
    /// Borrow the factor definitions configured on the model.
    ///
    /// # Returns
    ///
    /// Factor definitions in covariance order.
    #[must_use]
    pub fn factors(&self) -> &[FactorDefinition] {
        &self.factors
    }

    /// Match each position dependency in `portfolio` to configured factors.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio whose instrument dependencies should be mapped
    ///   into the configured factor space.
    ///
    /// # Returns
    ///
    /// Assignment report including both successful matches and unmatched
    /// dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a position cannot report dependencies or when the
    /// unmatched policy is strict and at least one dependency cannot be mapped.
    pub fn assign_factors(&self, portfolio: &Portfolio) -> Result<FactorAssignmentReport> {
        let mut assignments = Vec::with_capacity(portfolio.positions.len());
        let mut unmatched = Vec::new();

        for position in &portfolio.positions {
            let dependencies = flatten_dependencies(&position.instrument.market_dependencies()?);
            let (assignment, position_unmatched) = assign_position_factors(
                &position.position_id,
                &dependencies,
                position.instrument.attributes(),
                self.matcher.as_ref(),
            )?;

            if self.unmatched_policy == UnmatchedPolicy::Strict && !position_unmatched.is_empty() {
                let first_unmatched = &position_unmatched[0];
                let message = FactorModelError::UnmatchedDependency {
                    position_id: first_unmatched.position_id.to_string(),
                    dependency: first_unmatched.dependency.clone(),
                }
                .to_string();
                return Err(Error::invalid_input(message));
            }

            if self.unmatched_policy == UnmatchedPolicy::Warn {
                for unmatched_entry in &position_unmatched {
                    tracing::warn!(
                        position_id = %unmatched_entry.position_id,
                        dependency = ?unmatched_entry.dependency,
                        "Unmatched dependency during factor assignment"
                    );
                }
            }

            assignments.push(assignment);
            unmatched.extend(position_unmatched);
        }

        Ok(FactorAssignmentReport {
            assignments,
            unmatched,
        })
    }

    /// Compute the weighted position-factor sensitivity matrix for `portfolio`.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio to analyze.
    /// * `market` - Market context used by the sensitivity engine.
    /// * `as_of` - Valuation date for sensitivity generation.
    ///
    /// # Returns
    ///
    /// Weighted sensitivity matrix with one row per position and one column per
    /// configured factor.
    ///
    /// # Errors
    ///
    /// Propagates assignment or sensitivity-engine failures.
    pub fn compute_sensitivities(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix> {
        let _assignment_report = self.assign_factors(portfolio)?;
        let positions: Vec<(String, &dyn Instrument, f64)> = portfolio
            .positions
            .iter()
            .map(|position| {
                (
                    position.position_id.to_string(),
                    position.instrument.as_ref() as &dyn Instrument,
                    position.quantity,
                )
            })
            .collect();

        let mut sensitivities = self.sensitivity_engine.compute_sensitivities(
            &positions,
            &self.factors,
            market,
            as_of,
        )?;
        self.overlay_assignment_driven_credit_sensitivities(
            portfolio,
            market,
            as_of,
            &mut sensitivities,
        )?;
        Ok(sensitivities)
    }

    fn overlay_assignment_driven_credit_sensitivities(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        sensitivities: &mut SensitivityMatrix,
    ) -> Result<()> {
        for (position_idx, position) in portfolio.positions.iter().enumerate() {
            let dependencies = flatten_dependencies(&position.instrument.market_dependencies()?);
            for dependency in &dependencies {
                let Some(curve_id) = credit_curve_id(dependency) else {
                    continue;
                };
                let Some(entries) = self
                    .matcher
                    .match_factor_with_betas(dependency, position.instrument.attributes())
                    .map_err(|e| Error::invalid_input(e.to_string()))?
                else {
                    continue;
                };
                for entry in entries {
                    let Some(factor_idx) = self
                        .factors
                        .iter()
                        .position(|factor| factor.id == entry.factor_id)
                    else {
                        continue;
                    };
                    if !uses_assignment_driven_credit_shock(&self.factors[factor_idx]) {
                        continue;
                    }
                    let bump_size = self.bump_config.bump_size_for_factor(
                        &self.factors[factor_idx].id,
                        &self.factors[factor_idx].factor_type,
                    );
                    let delta = credit_curve_parallel_delta(
                        position.instrument.as_ref(),
                        position.quantity,
                        market,
                        as_of,
                        curve_id,
                        bump_size,
                    )?;
                    let current = sensitivities.delta(position_idx, factor_idx);
                    sensitivities.set_delta(position_idx, factor_idx, current + delta);
                }
            }
        }
        Ok(())
    }

    /// Run the full sensitivity-plus-decomposition pipeline.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio to analyze.
    /// * `market` - Market context used for sensitivity generation.
    /// * `as_of` - Valuation date for the analysis.
    ///
    /// # Returns
    ///
    /// Portfolio-level risk decomposition in the configured risk-measure units.
    ///
    /// # Errors
    ///
    /// Propagates assignment, sensitivity, and decomposition failures.
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
    /// - `docs/REFERENCES.md#tasche-2008-capital-allocation`
    pub fn analyze(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<RiskDecomposition> {
        let sensitivities = self.compute_sensitivities(portfolio, market, as_of)?;
        let mut decomposition =
            self.decomposer
                .decompose(&sensitivities, &self.covariance, &self.risk_measure)?;
        self.add_credit_residual_risk(&mut decomposition, portfolio, market, as_of)?;
        Ok(decomposition)
    }

    fn add_credit_residual_risk(
        &self,
        decomposition: &mut RiskDecomposition,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<()> {
        if self.credit_idiosyncratic_variance.is_empty() {
            return Ok(());
        }
        let mut residual_contributions = Vec::new();
        for position in &portfolio.positions {
            let Some(issuer_id_str) = position
                .instrument
                .attributes()
                .get_meta(ISSUER_ID_META_KEY)
            else {
                continue;
            };
            let issuer_id = finstack_core::types::IssuerId::new(issuer_id_str);
            let Some(idio_variance) = self.credit_idiosyncratic_variance.get(&issuer_id).copied()
            else {
                continue;
            };
            if idio_variance <= 0.0 {
                continue;
            }
            let dependencies = flatten_dependencies(&position.instrument.market_dependencies()?);
            let mut exposure = 0.0;
            for dependency in &dependencies {
                if let Some(curve_id) = credit_curve_id(dependency) {
                    exposure += credit_curve_parallel_delta(
                        position.instrument.as_ref(),
                        position.quantity,
                        market,
                        as_of,
                        curve_id,
                        self.bump_config.credit_bp,
                    )?;
                }
            }
            let residual_variance = exposure * exposure * idio_variance;
            if residual_variance > 0.0 {
                residual_contributions.push(PositionResidualContribution {
                    position_id: position.position_id.clone(),
                    residual_variance,
                    source: ResidualContributionSource::FromCreditModel { issuer_id },
                });
            }
        }
        apply_residual_contributions(decomposition, residual_contributions);
        Ok(())
    }

    /// Create a what-if engine anchored to a base decomposition and sensitivity matrix.
    ///
    /// # Arguments
    ///
    /// * `base` - Previously computed baseline risk decomposition.
    /// * `sensitivities` - Baseline sensitivity matrix.
    /// * `portfolio` - Portfolio associated with the baseline analysis.
    /// * `market` - Baseline market context.
    /// * `as_of` - Valuation date associated with the baseline analysis.
    ///
    /// # Returns
    ///
    /// What-if engine that can evaluate factor changes relative to the supplied
    /// baseline.
    #[must_use]
    pub fn what_if<'a>(
        &'a self,
        base: &'a RiskDecomposition,
        sensitivities: &'a SensitivityMatrix,
        portfolio: &'a Portfolio,
        market: &'a MarketContext,
        as_of: Date,
    ) -> WhatIfEngine<'a> {
        WhatIfEngine::new(self, base, sensitivities, portfolio, market, as_of)
    }

    pub(crate) fn covariance(&self) -> &FactorCovarianceMatrix {
        &self.covariance
    }

    pub(crate) fn decomposer(&self) -> &dyn RiskDecomposer {
        self.decomposer.as_ref()
    }

    pub(crate) fn risk_measure(&self) -> &RiskMeasure {
        &self.risk_measure
    }

    pub(crate) fn stressed_market(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        stresses: &[(finstack_factor_model::FactorId, f64)],
    ) -> Result<MarketContext> {
        use crate::sensitivity::mapping_to_market_bumps;
        use finstack_factor_model::FactorBumpUnit;

        let stress_by_id: HashMap<_, _> = stresses.iter().map(|(id, shift)| (id, *shift)).collect();
        for (factor_id, _) in stresses {
            if !self.factors.iter().any(|factor| factor.id == *factor_id) {
                return Err(Error::invalid_input(format!(
                    "Unknown factor '{factor_id}'"
                )));
            }
        }

        let mut stressed = market.clone();
        for factor in &self.factors {
            let Some(shift) = stress_by_id.get(&factor.id).copied() else {
                continue;
            };
            if uses_assignment_driven_credit_shock(factor) {
                let curve_ids = self.credit_curves_matched_to_factor(portfolio, &factor.id)?;
                stressed = shift_credit_curves(&stressed, &curve_ids, shift)?;
            } else {
                stressed = stressed.bump(mapping_to_market_bumps(
                    &factor.market_mapping,
                    shift,
                    FactorBumpUnit::canonical_for(&factor.factor_type),
                    as_of,
                )?)?;
            }
        }

        Ok(stressed)
    }

    fn credit_curves_matched_to_factor(
        &self,
        portfolio: &Portfolio,
        factor_id: &finstack_factor_model::FactorId,
    ) -> Result<Vec<finstack_core::types::CurveId>> {
        let mut curve_ids = BTreeSet::new();
        for position in &portfolio.positions {
            let dependencies = flatten_dependencies(&position.instrument.market_dependencies()?);
            for dependency in &dependencies {
                let Some(curve_id) = credit_curve_id(dependency) else {
                    continue;
                };
                let Some(entries) = self
                    .matcher
                    .match_factor_with_betas(dependency, position.instrument.attributes())
                    .map_err(|e| Error::invalid_input(e.to_string()))?
                else {
                    continue;
                };
                if entries.iter().any(|entry| entry.factor_id == *factor_id) {
                    curve_ids.insert(curve_id.clone());
                }
            }
        }
        Ok(curve_ids.into_iter().collect())
    }
}

fn credit_idiosyncratic_variance(
    matching: &MatchingConfig,
) -> BTreeMap<finstack_core::types::IssuerId, f64> {
    let mut out = BTreeMap::new();
    collect_credit_idiosyncratic_variance(matching, &mut out);
    out
}

fn collect_credit_idiosyncratic_variance(
    matching: &MatchingConfig,
    out: &mut BTreeMap<finstack_core::types::IssuerId, f64>,
) {
    match matching {
        MatchingConfig::CreditHierarchical(config) => {
            for row in &config.issuer_betas {
                out.insert(
                    row.issuer_id.clone(),
                    row.adder_vol_annualized * row.adder_vol_annualized,
                );
            }
        }
        MatchingConfig::Cascade(configs) => {
            for config in configs {
                collect_credit_idiosyncratic_variance(config, out);
            }
        }
        MatchingConfig::MappingTable(_) | MatchingConfig::Hierarchical(_) => {}
    }
}

fn apply_residual_contributions(
    decomposition: &mut RiskDecomposition,
    residual_contributions: Vec<PositionResidualContribution>,
) {
    let residual_variance: f64 = residual_contributions
        .iter()
        .map(|contribution| contribution.residual_variance)
        .sum();
    if residual_variance <= 0.0 {
        return;
    }
    let systematic_variance =
        variance_from_measure(decomposition.measure, decomposition.total_risk);
    let combined_variance = systematic_variance + residual_variance;
    let (combined_total, combined_component_scale) =
        risk_total_and_component_scale(decomposition.measure, combined_variance);
    let (_, systematic_component_scale) =
        risk_total_and_component_scale(decomposition.measure, systematic_variance);
    let factor_rescale = if systematic_component_scale.abs() > 0.0 {
        combined_component_scale / systematic_component_scale
    } else {
        0.0
    };

    for contribution in &mut decomposition.factor_contributions {
        contribution.absolute_risk *= factor_rescale;
        contribution.marginal_risk *= factor_rescale;
        contribution.relative_risk = if combined_total.abs() > 0.0 {
            contribution.absolute_risk / combined_total
        } else {
            0.0
        };
    }
    for contribution in &mut decomposition.position_factor_contributions {
        contribution.risk_contribution *= factor_rescale;
    }

    decomposition.total_risk = combined_total;
    decomposition.residual_risk = residual_variance * combined_component_scale;
    decomposition
        .position_residual_contributions
        .extend(residual_contributions);

    // Invariant: the rescale above is chosen so the decomposition stays
    // Euler-additive after the residual term is folded in, i.e. the factor
    // contributions plus the residual still exhaust `total_risk`. This holds
    // for every risk measure because `factor_rescale` is exactly the ratio of
    // combined-to-systematic component scales.
    debug_assert!(
        {
            let factor_sum: f64 = decomposition
                .factor_contributions
                .iter()
                .map(|c| c.absolute_risk)
                .sum();
            (factor_sum + decomposition.residual_risk - decomposition.total_risk).abs()
                <= 1e-6 * decomposition.total_risk.abs().max(1.0)
        },
        "residual overlay broke Euler additivity of the risk decomposition"
    );
}

fn variance_from_measure(measure: RiskMeasure, total_risk: f64) -> f64 {
    match measure {
        RiskMeasure::Variance => total_risk.max(0.0),
        RiskMeasure::Volatility => total_risk * total_risk,
        RiskMeasure::VaR { confidence } => {
            let z = super::math::normal_quantile(confidence);
            if z > 0.0 {
                (total_risk / -z).powi(2)
            } else {
                0.0
            }
        }
        RiskMeasure::ExpectedShortfall { confidence } => {
            let z = super::math::normal_quantile(confidence);
            let es_multiplier = super::math::normal_pdf(z) / (1.0 - confidence);
            if es_multiplier > 0.0 {
                (total_risk / -es_multiplier).powi(2)
            } else {
                0.0
            }
        }
    }
}

fn risk_total_and_component_scale(measure: RiskMeasure, variance: f64) -> (f64, f64) {
    let variance = variance.max(0.0);
    let sigma = variance.sqrt();
    match measure {
        RiskMeasure::Variance => (variance, 1.0),
        RiskMeasure::Volatility => {
            if sigma > 0.0 {
                (sigma, sigma.recip())
            } else {
                (0.0, 0.0)
            }
        }
        RiskMeasure::VaR { confidence } => {
            let z = super::math::normal_quantile(confidence);
            if sigma > 0.0 {
                (-sigma * z, -z * sigma.recip())
            } else {
                (0.0, 0.0)
            }
        }
        RiskMeasure::ExpectedShortfall { confidence } => {
            let z = super::math::normal_quantile(confidence);
            let es_multiplier = super::math::normal_pdf(z) / (1.0 - confidence);
            if sigma > 0.0 {
                (-sigma * es_multiplier, -es_multiplier * sigma.recip())
            } else {
                (0.0, 0.0)
            }
        }
    }
}

fn uses_assignment_driven_credit_shock(factor: &FactorDefinition) -> bool {
    matches!(factor.factor_type, FactorType::Credit)
        && matches!(
            factor.market_mapping,
            finstack_factor_model::MarketMapping::CurveParallel { ref curve_ids, .. }
                if curve_ids.is_empty()
        )
}

fn credit_curve_id(dependency: &MarketDependency) -> Option<&finstack_core::types::CurveId> {
    match dependency {
        MarketDependency::CreditCurve { id }
        | MarketDependency::Curve {
            id,
            curve_type: CurveType::Hazard,
        } => Some(id),
        _ => None,
    }
}

fn credit_curve_parallel_delta(
    instrument: &dyn Instrument,
    quantity: f64,
    market: &MarketContext,
    as_of: Date,
    curve_id: &finstack_core::types::CurveId,
    bump_size: f64,
) -> Result<f64> {
    if bump_size.abs() < f64::EPSILON {
        return Err(Error::invalid_input(
            "credit factor bump size must be non-zero for sensitivity computation",
        ));
    }
    let bump = |value| -> finstack_core::Result<MarketContext> {
        let curve = market.get_hazard(curve_id.as_str())?;
        let bumped = bump_hazard_shift(curve.as_ref(), &BumpRequest::Parallel(value))?;
        Ok(market.clone().insert(bumped))
    };
    let up = bump(bump_size)?;
    let down = bump(-bump_size)?;
    let pv_up = instrument.value_raw(&up, as_of)?;
    let pv_down = instrument.value_raw(&down, as_of)?;
    Ok((pv_up - pv_down) / (2.0 * bump_size) * quantity)
}

fn shift_credit_curves(
    market: &MarketContext,
    curve_ids: &[finstack_core::types::CurveId],
    delta_bp: f64,
) -> Result<MarketContext> {
    let mut out = market.clone();
    if delta_bp == 0.0 {
        return Ok(out);
    }
    for curve_id in curve_ids {
        let curve = out.get_hazard(curve_id.as_str())?;
        let bumped = bump_hazard_shift(curve.as_ref(), &BumpRequest::Parallel(delta_bp))?;
        out = out.insert(bumped);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::sensitivity::{FactorSensitivityEngine, SensitivityMatrix};
    use crate::types::{PositionId, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
    use finstack_core::types::{Attributes, CurveId};
    use finstack_factor_model::matching::{DependencyFilter, MappingRule};
    use finstack_factor_model::{
        BumpSizeConfig, CurveType, DependencyType, FactorCovarianceMatrix, FactorDefinition,
        FactorId, FactorModelConfig, FactorType, MarketMapping, PricingMode, RiskMeasure,
        UnmatchedPolicy,
    };
    use finstack_valuations::instruments::Instrument;
    use finstack_valuations::instruments::MarketDependencies;
    use finstack_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use time::macros::date;

    fn simple_config() -> FactorModelConfig {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
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
                attribute_filter: finstack_factor_model::AttributeFilter::default(),
                factor_id: FactorId::new("Rates"),
            }]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: Some(BumpSizeConfig::default()),
            unmatched_policy: Some(UnmatchedPolicy::Residual),
        }
    }

    fn unreachable_config() -> FactorModelConfig {
        FactorModelConfig {
            factors: Vec::new(),
            covariance: FactorCovarianceMatrix::new(Vec::new(), Vec::new())
                .expect("empty covariance matrix is valid"),
            matching: MatchingConfig::MappingTable(Vec::new()),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        }
    }

    #[test]
    fn test_builder_from_config_exposes_factors() {
        let build_result = FactorModelBuilder::new().config(simple_config()).build();
        assert!(build_result.is_ok());
        let Ok(model) = build_result else {
            return;
        };

        assert_eq!(model.factors().len(), 1);
        assert_eq!(model.factors()[0].id, FactorId::new("Rates"));
    }

    #[test]
    fn test_builder_missing_config_fails() {
        let result = FactorModelBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_covariance_axes_not_aligned_to_factors() {
        let covariance_result = FactorCovarianceMatrix::new(
            vec![FactorId::new("Credit"), FactorId::new("Rates")],
            vec![0.09, 0.01, 0.01, 0.04],
        );
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };

        let result = FactorModelBuilder::new()
            .config(FactorModelConfig {
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
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_assign_factors_collects_matches_and_unmatched() {
        let build_result = FactorModelBuilder::new().config(simple_config()).build();
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
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build()
            .expect("test should succeed");

        let report_result = model.assign_factors(&portfolio);
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
    fn test_analyze_uses_custom_sensitivity_engine_and_decomposer() {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };

        let expected = RiskDecomposition {
            total_risk: 2.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
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
                matching: MatchingConfig::MappingTable(vec![]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(FixedSensitivityEngine)
            .with_custom_decomposer(FixedDecomposer(expected.clone()))
            .build();
        assert!(model_result.is_ok());
        let Ok(model) = model_result else {
            return;
        };

        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .build()
            .expect("test should succeed");
        let analysis_result =
            model.analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
        assert!(analysis_result.is_ok());
        let Ok(actual) = analysis_result else {
            return;
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_analyze_fails_when_strict_policy_has_unmatched_dependencies() {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
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
                matching: MatchingConfig::MappingTable(vec![]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Strict),
            })
            .with_custom_sensitivity_engine(FixedSensitivityEngine)
            .with_custom_decomposer(FixedDecomposer(RiskDecomposition {
                total_risk: 0.0,
                measure: RiskMeasure::Variance,
                factor_contributions: vec![],
                residual_risk: 0.0,
                position_factor_contributions: vec![],
                position_residual_contributions: vec![],
            }))
            .build();
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
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build()
            .expect("test should succeed");

        let analysis_result =
            model.analyze(&portfolio, &MarketContext::new(), date!(2024 - 01 - 01));
        assert!(analysis_result.is_err());
    }

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        discount_curve: CurveId,
        spots: Vec<String>,
    }

    impl MockInstrument {
        fn new(id: &str, discount_curve: &str, spots: Vec<String>) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::default(),
                discount_curve: CurveId::new(discount_curve),
                spots,
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
            _market: &MarketContext,
            _as_of: finstack_core::dates::Date,
        ) -> finstack_core::Result<Money> {
            Ok(Money::new(100.0, Currency::USD))
        }

        fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
            let mut dependencies = MarketDependencies::new();
            dependencies
                .curves
                .discount_curves
                .push(self.discount_curve.clone());
            dependencies.spot_ids.extend(self.spots.iter().cloned());
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
            Ok(SensitivityMatrix::zeros(
                Vec::new(),
                factors.iter().map(|factor| factor.id.clone()).collect(),
            ))
        }
    }

    struct FixedDecomposer(RiskDecomposition);

    impl crate::factor_model::RiskDecomposer for FixedDecomposer {
        fn decompose(
            &self,
            _sensitivities: &SensitivityMatrix,
            _covariance: &FactorCovarianceMatrix,
            _measure: &RiskMeasure,
        ) -> finstack_core::Result<RiskDecomposition> {
            Ok(self.0.clone())
        }
    }

    /// Returns a sensitivity engine that places known deltas for a single
    /// position so the downstream `ParametricDecomposer` can be verified.
    struct KnownDeltaEngine {
        deltas: Vec<f64>,
    }

    impl FactorSensitivityEngine for KnownDeltaEngine {
        fn compute_sensitivities(
            &self,
            positions: &[(String, &dyn Instrument, f64)],
            factors: &[FactorDefinition],
            _market: &MarketContext,
            _as_of: finstack_core::dates::Date,
        ) -> finstack_core::Result<SensitivityMatrix> {
            let position_ids: Vec<String> = positions.iter().map(|(id, _, _)| id.clone()).collect();
            let factor_ids: Vec<_> = factors.iter().map(|f| f.id.clone()).collect();
            let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);
            for (j, &delta) in self.deltas.iter().enumerate() {
                matrix.set_delta(0, j, delta);
            }
            Ok(matrix)
        }
    }

    #[test]
    fn test_analyze_end_to_end_single_factor_with_real_decomposer() {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
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
                matching: MatchingConfig::MappingTable(vec![]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(KnownDeltaEngine { deltas: vec![10.0] })
            .build();
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
            .base_ccy(Currency::USD)
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

        let model_result = FactorModelBuilder::new()
            .config(FactorModelConfig {
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
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(KnownDeltaEngine {
                deltas: vec![10.0, 5.0],
            })
            .build();
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
            .base_ccy(Currency::USD)
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

    fn canonical_credit_bond(curve_id: CurveId) -> finstack_valuations::instruments::Bond {
        use finstack_factor_model::matching::ISSUER_ID_META_KEY;
        let mut bond = finstack_valuations::instruments::Bond::fixed(
            "BOND-ISSUER-B",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            date!(2024 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("canonical bond should build");
        bond.credit_curve_id = Some(curve_id);
        bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
        bond
    }

    fn credit_market(as_of: Date, curve_id: CurveId) -> MarketContext {
        use finstack_core::dates::DayCount;
        use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.05_f64).exp()),
                (5.0, (-0.25_f64).exp()),
                (10.0, (-0.50_f64).exp()),
            ])
            .build()
            .expect("discount curve");
        let hazard = HazardCurve::builder(curve_id)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(1.0, 0.01), (5.0, 0.01), (10.0, 0.01)])
            .build()
            .expect("hazard curve");
        MarketContext::new().insert(discount).insert(hazard)
    }

    #[test]
    fn credit_hierarchy_sensitivities_use_fixed_bp_membership_not_beta_scaled_mapping() {
        use finstack_factor_model::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_factor_model::matching::CreditHierarchicalConfig;
        use std::collections::BTreeMap;

        let as_of = date!(2024 - 01 - 01);
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let market = credit_market(as_of, curve_id.clone());
        let mut tags = BTreeMap::new();
        tags.insert("rating".to_string(), "B".to_string());
        let issuer_row = IssuerBetaRow {
            issuer_id: finstack_core::types::IssuerId::new("ISSUER-B"),
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
        let model = FactorModelBuilder::new()
            .config(FactorModelConfig {
                factors,
                covariance,
                matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                    dependency_filter: Default::default(),
                    hierarchy: CreditHierarchySpec {
                        levels: vec![HierarchyDimension::Rating],
                    },
                    issuer_betas: vec![issuer_row],
                }),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .build()
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
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .unwrap();

        let sensitivities = model
            .compute_sensitivities(&portfolio, &market, as_of)
            .expect("sensitivities");

        let generic = sensitivities.delta(0, 0);
        let rating = sensitivities.delta(0, 1);
        assert!(
            generic.abs() > 1e-8,
            "canonical bond should have credit sensitivity"
        );
        assert!(
            (generic - rating).abs() < 1e-10,
            "fixed-bp hierarchy factors should use the same direct issuer CS01, not beta scaling"
        );
    }

    #[test]
    fn credit_hierarchy_analysis_adds_idiosyncratic_residual_variance() {
        use finstack_factor_model::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_factor_model::matching::CreditHierarchicalConfig;
        use std::collections::BTreeMap;

        let as_of = date!(2024 - 01 - 01);
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let market = credit_market(as_of, curve_id.clone());
        let mut tags = BTreeMap::new();
        tags.insert("rating".to_string(), "B".to_string());
        let issuer_row = IssuerBetaRow {
            issuer_id: finstack_core::types::IssuerId::new("ISSUER-B"),
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
        let model = FactorModelBuilder::new()
            .config(FactorModelConfig {
                factors,
                covariance,
                matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                    dependency_filter: Default::default(),
                    hierarchy: CreditHierarchySpec {
                        levels: vec![HierarchyDimension::Rating],
                    },
                    issuer_betas: vec![issuer_row],
                }),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .build()
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
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .unwrap();

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
}
