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
//! Credit hierarchy factors shock stored CDS spread quotes and require a lossless
//! calibration recipe on every matched hazard curve. Intensity-only curves cannot
//! supply spread-factor risk. All monetary exposures use portfolio base currency.
//!
//! # References
//!
//! - Factor-model portfolio construction: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//!
//! - Euler-style capital allocation background: `docs/REFERENCES.md#tasche-2008-capital-allocation`
//!
//! - Parametric VaR conventions: `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
//!

use super::assignment::{assign_position_factors, FactorAssignmentReport};
use super::dependencies::flatten as flatten_dependencies;
use super::whatif::{StressPnl, StressResult, WhatIfEngine};
use crate::error::{Error, Result};
use crate::sensitivity::{
    exact_factor_market_keys, DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine,
    SensitivityMatrix,
};
use crate::{MarketFactorKey, Portfolio};
use finstack_quant_calibration::api::schema::HazardCurveParams;
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_models::factor::matching::ISSUER_ID_META_KEY;
use finstack_quant_models::factor::risk::{
    apply_residual_contributions, ParametricDecomposer, PositionResidualContribution,
    ResidualContributionSource, RiskDecomposition,
};
use finstack_quant_models::factor::{
    BumpSizeConfig, CurveType, FactorCovarianceMatrix, FactorDefinition, FactorModelConfig,
    FactorType, MarketDependency, MatchingConfig, PricingMode, RiskMeasure, UnmatchedPolicy,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::recalibration::QuoteBump;
use std::collections::{BTreeMap, HashMap};

/// Builder for the top-level portfolio factor-model orchestrator.
///
/// Use this type to inject a declarative factor-model configuration and, in
/// tests, override the sensitivity engine.
pub struct FactorModelBuilder {
    config: Option<FactorModelConfig>,
    #[cfg(test)]
    custom_sensitivity_engine: Option<Box<dyn FactorSensitivityEngine>>,
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
    /// and the [`FactorModelConfig::validate`] error when matching rules
    /// reference undeclared factor IDs, the risk measure is invalid, or the
    /// covariance axes do not align with the configured factors.
    pub fn build(self) -> Result<FactorModel> {
        let config = self
            .config
            .ok_or_else(|| Error::invalid_input("FactorModelConfig is required"))?;
        config.validate()?;

        let matcher = config.matching.build_matcher();
        let bump_config = config.bump_size.clone().unwrap_or_default();
        let sensitivity_engine = {
            #[cfg(test)]
            let engine = match self.custom_sensitivity_engine {
                Some(engine) => engine,
                None => default_sensitivity_engine(config.pricing_mode, &bump_config)?,
            };
            #[cfg(not(test))]
            let engine = default_sensitivity_engine(config.pricing_mode, &bump_config)?;
            engine
        };
        Ok(FactorModel {
            credit_idiosyncratic_variance: credit_idiosyncratic_variance(&config.matching),
            factors: config.factors,
            covariance: config.covariance,
            matcher,
            sensitivity_engine,
            decomposer: ParametricDecomposer,
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

fn default_sensitivity_engine(
    pricing_mode: PricingMode,
    bump_config: &BumpSizeConfig,
) -> Result<Box<dyn FactorSensitivityEngine>> {
    Ok(match pricing_mode {
        PricingMode::FullRepricing => Box::new(FullRepricingEngine::new(bump_config.clone(), 5)?),
        _ => Box::new(DeltaBasedEngine::new(bump_config.clone())),
    })
}

/// Portfolio-level factor-model orchestrator.
///
/// A `FactorModel` owns the factor definitions, covariance matrix, and the
/// pluggable engines required to move from instrument dependencies to
/// portfolio-level risk decomposition.
pub struct FactorModel {
    credit_idiosyncratic_variance: BTreeMap<finstack_quant_core::types::IssuerId, f64>,
    factors: Vec<FactorDefinition>,
    covariance: FactorCovarianceMatrix,
    matcher: Box<dyn finstack_quant_models::factor::FactorMatcher>,
    sensitivity_engine: Box<dyn FactorSensitivityEngine>,
    decomposer: ParametricDecomposer,
    risk_measure: RiskMeasure,
    unmatched_policy: UnmatchedPolicy,
    bump_config: BumpSizeConfig,
}

impl FactorModel {
    /// Start building a factor model configuration.
    ///
    /// This is the preferred entry point, consistent with other builders
    /// in the workspace.
    #[must_use]
    pub fn builder() -> FactorModelBuilder {
        FactorModelBuilder::new()
    }

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
                let message = format!(
                    "No factor matched dependency {:?} for position '{}'",
                    first_unmatched.dependency, first_unmatched.position_id
                );
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
    /// Each engine cell is a central difference of base-currency PVs. Native
    /// instrument values are converted with the portfolio spot FX helper on
    /// the bumped market at `as_of`. The row weight is
    /// [`crate::position::Position::scale_factor`].
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio to analyze. `base_currency` is the reporting
    ///   currency for every converted PV.
    /// * `market` - Market context used by the sensitivity engine, including
    ///   the FX matrix required for any cross-currency position.
    /// * `as_of` - Valuation date for sensitivity generation and spot FX.
    ///
    /// # Returns
    ///
    /// Weighted sensitivity matrix with one row per position and one column per
    /// configured factor, in `portfolio.base_currency`.
    ///
    /// # Errors
    ///
    /// Propagates assignment, sensitivity-engine, or FX-conversion failures.
    pub fn compute_sensitivities(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix> {
        let mut credit_exposures = CreditExposureMatrix::new(market, portfolio.base_currency);
        self.compute_sensitivities_with_credit_exposures(
            portfolio,
            market,
            as_of,
            &mut credit_exposures,
        )
    }

    fn compute_sensitivities_with_credit_exposures(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        credit_exposures: &mut CreditExposureMatrix<'_>,
    ) -> Result<SensitivityMatrix> {
        let assignment_report = self.assign_factors(portfolio)?;
        let positions: Vec<(String, &dyn Instrument, f64)> = portfolio
            .positions
            .iter()
            .map(|position| {
                (
                    position.position_id.to_string(),
                    position.instrument.as_ref() as &dyn Instrument,
                    position.scale_factor(),
                )
            })
            .collect();

        let mut sensitivities = self.sensitivity_engine.compute_sensitivities(
            &positions,
            &self.factors,
            market,
            as_of,
            portfolio.base_currency,
        )?;
        self.overlay_assignment_driven_credit_sensitivities(
            portfolio,
            as_of,
            &assignment_report,
            &mut sensitivities,
            credit_exposures,
        )?;
        Ok(sensitivities)
    }

    fn overlay_assignment_driven_credit_sensitivities(
        &self,
        portfolio: &Portfolio,
        as_of: Date,
        assignment_report: &FactorAssignmentReport,
        sensitivities: &mut SensitivityMatrix,
        credit_exposures: &mut CreditExposureMatrix<'_>,
    ) -> Result<()> {
        for (position_idx, (position, assignment)) in portfolio
            .positions
            .iter()
            .zip(&assignment_report.assignments)
            .enumerate()
        {
            for (dependency, factor_id, beta) in &assignment.mappings {
                let Some(curve_id) =
                    credit_curve_id(dependency, credit_exposures.bump_contexts.base)?
                else {
                    continue;
                };
                let Some(factor_idx) = self
                    .factors
                    .iter()
                    .position(|factor| factor.id == *factor_id)
                else {
                    // The matcher emitted a factor id that is not
                    // declared in `factors` (typically a runtime issuer
                    // whose tags name a bucket outside the calibrated
                    // universe). Dropping it loses real credit exposure,
                    // so the unmatched policy decides: Strict fails,
                    // Warn surfaces the drop, Residual continues.
                    match self.unmatched_policy {
                        UnmatchedPolicy::Strict => {
                            return Err(Error::invalid_input(format!(
                                "Credit factor '{}' matched for position '{}' is not \
                                     declared in the factor model; its exposure would be \
                                     silently dropped",
                                factor_id, position.position_id
                            )));
                        }
                        UnmatchedPolicy::Warn => {
                            tracing::warn!(
                                position_id = %position.position_id,
                                factor_id = %factor_id,
                                "dropping credit exposure to a factor id not declared \
                                     in the factor model"
                            );
                        }
                        _ => {}
                    }
                    continue;
                };
                if !uses_assignment_driven_credit_shock(&self.factors[factor_idx]) {
                    continue;
                }
                let bump_size = self.bump_config.bump_size_for_factor(
                    &self.factors[factor_idx].id,
                    &self.factors[factor_idx].factor_type,
                );
                let delta = credit_exposures.exposure(
                    position_idx,
                    position.instrument.as_ref(),
                    position.scale_factor(),
                    as_of,
                    &curve_id,
                    bump_size,
                )?;
                // Under the credit hierarchy model Δs_i = β_pc·ΔG +
                // Σ_k β_k·ΔL_k + Δε_i, so exposure to a factor is the
                // issuer CS01 scaled by that factor's calibrated loading —
                // the same convention credit attribution applies
                // (`attribution::credit_factor`). Dropping the beta here
                // would overstate risk for defensive names (β < 1) and
                // understate it for levered ones (β > 1).
                let current = sensitivities.delta(position_idx, factor_idx);
                sensitivities.set_delta(position_idx, factor_idx, current + *beta * delta);
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
        self.analyze_with_sensitivities(portfolio, market, as_of)
            .map(|(decomposition, _)| decomposition)
    }

    /// Run one sensitivity pass and return it with the resulting decomposition.
    ///
    /// This is the canonical entry point for workflows that need both the
    /// baseline sensitivity matrix and its risk decomposition, such as
    /// position and factor what-if analysis. Credit bump contexts and
    /// exposures are shared across decomposition and residual-risk assembly
    /// within this call.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio whose weighted position-factor sensitivities
    ///   and risk decomposition are computed.
    /// * `market` - Market snapshot used for factor shocks and instrument
    ///   repricing.
    /// * `as_of` - Valuation date applied consistently to sensitivity and
    ///   residual-risk calculations.
    ///
    /// # Returns
    ///
    /// The risk decomposition together with the exact sensitivity matrix used
    /// to produce it.
    ///
    /// # Errors
    ///
    /// Propagates factor assignment, sensitivity generation, decomposition,
    /// credit-curve lookup, and residual-risk calculation failures.
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
    /// - `docs/REFERENCES.md#tasche-2008-capital-allocation`
    pub fn analyze_with_sensitivities(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(RiskDecomposition, SensitivityMatrix)> {
        let mut credit_exposures = CreditExposureMatrix::new(market, portfolio.base_currency);
        let sensitivities = self.compute_sensitivities_with_credit_exposures(
            portfolio,
            market,
            as_of,
            &mut credit_exposures,
        )?;
        let mut decomposition =
            self.decomposer
                .decompose(&sensitivities, &self.covariance, &self.risk_measure)?;
        self.add_credit_residual_risk_with_credit_exposures(
            &mut decomposition,
            portfolio,
            as_of,
            &mut credit_exposures,
        )?;
        Ok((decomposition, sensitivities))
    }

    pub(crate) fn add_credit_residual_risk(
        &self,
        decomposition: &mut RiskDecomposition,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<()> {
        let mut credit_exposures = CreditExposureMatrix::new(market, portfolio.base_currency);
        self.add_credit_residual_risk_with_credit_exposures(
            decomposition,
            portfolio,
            as_of,
            &mut credit_exposures,
        )
    }

    fn add_credit_residual_risk_with_credit_exposures(
        &self,
        decomposition: &mut RiskDecomposition,
        portfolio: &Portfolio,
        as_of: Date,
        credit_exposures: &mut CreditExposureMatrix<'_>,
    ) -> Result<()> {
        if self.credit_idiosyncratic_variance.is_empty() {
            return Ok(());
        }
        // Positions sharing an issuer load the *same* idiosyncratic shock,
        // so the issuer's idio variance is (Σ_p e_p)² · σ²_i on the netted
        // exposure — not Σ_p e_p² · σ²_i per position. First pass: collect
        // per-position exposures and accumulate the (net, gross) exposure per
        // issuer in deterministic order.
        let mut per_position: Vec<(
            crate::types::PositionId,
            finstack_quant_core::types::IssuerId,
            f64,
            f64,
        )> = Vec::new();
        let mut issuer_exposures: BTreeMap<finstack_quant_core::types::IssuerId, (f64, f64)> =
            BTreeMap::new();
        for (position_idx, position) in portfolio.positions.iter().enumerate() {
            let Some(issuer_id_str) = position
                .instrument
                .attributes()
                .get_meta(ISSUER_ID_META_KEY)
            else {
                continue;
            };
            let issuer_id = finstack_quant_core::types::IssuerId::new(issuer_id_str);
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
                if let Some(curve_id) =
                    credit_curve_id(dependency, credit_exposures.bump_contexts.base)?
                {
                    exposure += credit_exposures.exposure(
                        position_idx,
                        position.instrument.as_ref(),
                        position.scale_factor(),
                        as_of,
                        &curve_id,
                        self.bump_config.credit_bp,
                    )?;
                }
            }
            let entry = issuer_exposures
                .entry(issuer_id.clone())
                .or_insert((0.0, 0.0));
            entry.0 += exposure;
            entry.1 += exposure.abs();
            per_position.push((
                position.position_id.clone(),
                issuer_id,
                exposure,
                idio_variance,
            ));
        }
        // Second pass: total issuer variance net² · σ² allocated back to
        // positions pro-rata e_p / Σ e_p, which reduces to net · e_p · σ²
        // (Euler-consistent; a hedge leg receives a negative allocation). A
        // flat book (net ≈ 0 relative to gross) carries zero idio risk: every
        // position keeps its row, allocated 0.
        let mut residual_contributions = Vec::with_capacity(per_position.len());
        for (position_id, issuer_id, exposure, idio_variance) in per_position {
            let (net, gross) = issuer_exposures
                .get(&issuer_id)
                .copied()
                .unwrap_or((0.0, 0.0));
            let residual_variance = if net.abs() <= 1e-12 * gross {
                0.0
            } else {
                net * exposure * idio_variance
            };
            residual_contributions.push(PositionResidualContribution {
                position_id: position_id.as_str().to_owned(),
                residual_variance,
                source: ResidualContributionSource::FromCreditModel {
                    issuer_id: issuer_id.as_str().to_owned(),
                },
            });
        }
        Ok(apply_residual_contributions(
            decomposition,
            residual_contributions,
        )?)
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

    /// Shock configured factors and reprice the portfolio without decomposing
    /// stressed risk.
    ///
    /// Use this when only shocked-minus-base P&L is required. Call
    /// [`Self::factor_stress`] when the stressed risk decomposition is also
    /// needed; that path reuses this P&L evaluation and then analyzes the
    /// shocked market once.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio whose position P&L is evaluated.
    /// * `market` - Baseline market snapshot to shock.
    /// * `as_of` - Valuation date used for both endpoints.
    /// * `stresses` - Factor IDs and shock magnitudes in each factor's
    ///   configured market-mapping convention.
    ///
    /// # Returns
    ///
    /// Total and per-position stressed-minus-base P&L in the portfolio base
    /// currency.
    ///
    /// # Errors
    ///
    /// Propagates unknown-factor, market-bump, valuation, and
    /// currency-validation failures.
    pub fn factor_stress_pnl(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        stresses: &[(finstack_quant_models::factor::FactorId, f64)],
    ) -> Result<StressPnl> {
        super::whatif::factor_stress_pnl(self, portfolio, market, as_of, stresses)
            .map(|(pnl, _)| pnl)
    }

    /// Shock configured factors, reprice the portfolio, and decompose risk
    /// under the stressed market.
    ///
    /// This direct workflow does not compute an unused baseline sensitivity
    /// matrix. Call [`Self::what_if`] only when a position remove/resize
    /// scenario also needs the supplied baseline decomposition and
    /// sensitivities.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio whose position P&L and stressed risk are
    ///   evaluated.
    /// * `market` - Baseline market snapshot to shock.
    /// * `as_of` - Valuation date used for both endpoints and stressed
    ///   sensitivities.
    /// * `stresses` - Factor IDs and shock magnitudes in each factor's
    ///   configured market-mapping convention.
    ///
    /// # Returns
    ///
    /// Total and per-position stressed-minus-base P&L plus the risk
    /// decomposition under the stressed market.
    ///
    /// # Errors
    ///
    /// Propagates unknown-factor, market-bump, valuation, currency-validation,
    /// sensitivity, and decomposition failures.
    pub fn factor_stress(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        stresses: &[(finstack_quant_models::factor::FactorId, f64)],
    ) -> Result<StressResult> {
        super::whatif::factor_stress(self, portfolio, market, as_of, stresses)
    }

    pub(crate) fn covariance(&self) -> &FactorCovarianceMatrix {
        &self.covariance
    }

    pub(crate) fn decomposer(&self) -> &ParametricDecomposer {
        &self.decomposer
    }

    pub(crate) fn risk_measure(&self) -> &RiskMeasure {
        &self.risk_measure
    }

    /// Build one stressed market and its exact selective-repricing manifest.
    ///
    /// Assignment-driven credit curve discovery is performed once per stressed
    /// factor and reused for both the market shock and dependency keys.
    pub(crate) fn stressed_market_with_factor_keys(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        stresses: &[(finstack_quant_models::factor::FactorId, f64)],
    ) -> Result<(MarketContext, Option<Vec<MarketFactorKey>>)> {
        use crate::sensitivity::mapping_to_market_bumps;
        use finstack_quant_models::factor::FactorBumpUnit;

        let stress_by_id: HashMap<_, _> = stresses.iter().map(|(id, shift)| (id, *shift)).collect();
        for (factor_id, _) in stresses {
            if !self.factors.iter().any(|factor| factor.id == *factor_id) {
                return Err(Error::invalid_input(format!(
                    "Unknown factor '{factor_id}'"
                )));
            }
        }

        let mut stressed = market.clone();
        let mut exact_keys = (portfolio.dependency_index().indexed_position_count()
            == portfolio.positions.len())
        .then(Vec::new);
        for factor in &self.factors {
            let Some(shift) = stress_by_id.get(&factor.id).copied() else {
                continue;
            };
            let resolved_curve_ids = if uses_assignment_driven_credit_shock(factor) {
                let curve_betas =
                    self.credit_curves_matched_to_factor(portfolio, &stressed, &factor.id)?;
                stressed = shift_credit_curves(&stressed, &curve_betas, shift)?;
                Some(
                    curve_betas
                        .into_iter()
                        .map(|(curve_id, _)| curve_id)
                        .collect::<Vec<_>>(),
                )
            } else {
                stressed = stressed.bump(mapping_to_market_bumps(
                    &factor.market_mapping,
                    shift,
                    FactorBumpUnit::canonical_for(&factor.factor_type),
                    as_of,
                )?)?;
                None
            };

            if exact_keys.is_some() {
                if let Some(factor_keys) =
                    exact_factor_market_keys(factor, market, resolved_curve_ids.as_deref())
                {
                    if let Some(keys) = exact_keys.as_mut() {
                        for key in factor_keys {
                            if !keys.contains(&key) {
                                keys.push(key);
                            }
                        }
                    }
                } else {
                    exact_keys = None;
                }
            }
        }

        Ok((stressed, exact_keys))
    }

    /// Credit curves matched to `factor_id`, each with the issuer's calibrated
    /// loading on that factor. The beta scales the curve shift under a factor
    /// shock (`Δs_i = β_i · ΔF`); a curve reached through several positions of
    /// the same issuer carries one beta, and the first match wins.
    fn credit_curves_matched_to_factor(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        factor_id: &finstack_quant_models::factor::FactorId,
    ) -> Result<Vec<(finstack_quant_core::types::CurveId, f64)>> {
        let mut curve_betas: BTreeMap<finstack_quant_core::types::CurveId, f64> = BTreeMap::new();
        for position in &portfolio.positions {
            let dependencies = flatten_dependencies(&position.instrument.market_dependencies()?);
            for dependency in &dependencies {
                let Some(curve_id) = credit_curve_id(dependency, market)? else {
                    continue;
                };
                let Some(entries) = self
                    .matcher
                    .match_factor_with_betas(dependency, position.instrument.attributes())
                    .map_err(|e| Error::invalid_input(e.to_string()))?
                else {
                    continue;
                };
                if let Some(entry) = entries.iter().find(|entry| entry.factor_id == *factor_id) {
                    curve_betas.entry(curve_id).or_insert(entry.beta);
                }
            }
        }
        Ok(curve_betas.into_iter().collect())
    }
}

fn credit_idiosyncratic_variance(
    matching: &MatchingConfig,
) -> BTreeMap<finstack_quant_core::types::IssuerId, f64> {
    let mut out = BTreeMap::new();
    collect_credit_idiosyncratic_variance(matching, &mut out);
    out
}

fn collect_credit_idiosyncratic_variance(
    matching: &MatchingConfig,
    out: &mut BTreeMap<finstack_quant_core::types::IssuerId, f64>,
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
        _ => {}
    }
}

fn uses_assignment_driven_credit_shock(factor: &FactorDefinition) -> bool {
    matches!(factor.factor_type, FactorType::Credit)
        && matches!(
            factor.market_mapping,
            finstack_quant_models::factor::MarketMapping::CurveParallel { ref curve_ids, .. }
                if curve_ids.is_empty()
        )
}

fn credit_curve_id(
    dependency: &MarketDependency,
    market: &MarketContext,
) -> Result<Option<finstack_quant_core::types::CurveId>> {
    match dependency {
        MarketDependency::CreditCurve { id }
        | MarketDependency::Curve {
            id,
            curve_type: CurveType::Hazard,
        } => Ok(Some(id.clone())),
        MarketDependency::CreditIndex { id } => Ok(Some(
            market.get_credit_index(id)?.index_credit_curve.id().clone(),
        )),
        _ => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CreditBumpKey {
    curve_id: finstack_quant_core::types::CurveId,
    bump_bits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CreditExposureKey {
    position_index: usize,
    curve_id: finstack_quant_core::types::CurveId,
    bump_bits: u64,
}

/// Request-local cache of credit bump markets and scaled position exposures.
///
/// The cache belongs to one evaluation call because every stored value depends
/// on that call's market snapshot and valuation date. It is shared by the
/// assignment overlay and idiosyncratic-residual passes in [`FactorModel::analyze`]
/// but never retained on the model. Its direct high-precision raw up/down PVs
/// are derivative inputs, not ordinary `PortfolioValuation` results, so this
/// financially distinct kernel intentionally does not enter the Money-valued
/// portfolio executor.
struct CreditExposureMatrix<'a> {
    base_currency: Currency,
    bump_contexts: CreditBumpContexts<'a>,
    exposures: HashMap<CreditExposureKey, f64>,
}

impl<'a> CreditExposureMatrix<'a> {
    fn new(base: &'a MarketContext, base_currency: Currency) -> Self {
        Self {
            base_currency,
            bump_contexts: CreditBumpContexts::new(base),
            exposures: HashMap::default(),
        }
    }

    fn exposure(
        &mut self,
        position_index: usize,
        instrument: &dyn Instrument,
        quantity: f64,
        as_of: Date,
        curve_id: &finstack_quant_core::types::CurveId,
        bump_size: f64,
    ) -> Result<f64> {
        if bump_size.abs() < f64::EPSILON {
            return Err(Error::invalid_input(
                "credit factor bump size must be non-zero for sensitivity computation",
            ));
        }
        let key = CreditExposureKey {
            position_index,
            curve_id: curve_id.clone(),
            bump_bits: bump_size.to_bits(),
        };
        if let Some(exposure) = self.exposures.get(&key) {
            return Ok(*exposure);
        }

        let (up, down) = self.bump_contexts.get(curve_id, bump_size)?;
        let pv_up = crate::sensitivity::raw_pv_in_base(instrument, up, as_of, self.base_currency)?;
        let pv_down =
            crate::sensitivity::raw_pv_in_base(instrument, down, as_of, self.base_currency)?;
        let exposure = (pv_up - pv_down) / (2.0 * bump_size) * quantity;
        self.exposures.insert(key, exposure);
        Ok(exposure)
    }
}

struct CreditBumpContexts<'a> {
    base: &'a MarketContext,
    contexts: HashMap<CreditBumpKey, (MarketContext, MarketContext)>,
}

impl<'a> CreditBumpContexts<'a> {
    fn new(base: &'a MarketContext) -> Self {
        Self {
            base,
            contexts: HashMap::new(),
        }
    }

    fn get(
        &mut self,
        curve_id: &finstack_quant_core::types::CurveId,
        bump_size: f64,
    ) -> Result<(&MarketContext, &MarketContext)> {
        let key = CreditBumpKey {
            curve_id: curve_id.clone(),
            bump_bits: bump_size.to_bits(),
        };
        if !self.contexts.contains_key(&key) {
            let up_curve = bump_credit_spreads(self.base, curve_id, bump_size)?;
            let down_curve = bump_credit_spreads(self.base, curve_id, -bump_size)?;
            self.contexts.insert(
                key.clone(),
                (
                    self.base.clone().insert(up_curve),
                    self.base.clone().insert(down_curve),
                ),
            );
        }
        self.contexts
            .get(&key)
            .map(|(up, down)| (up, down))
            .ok_or_else(|| Error::invalid_input("credit bump context was not retained"))
    }
}

/// Recalibrate the issuer curve after shocking its stored CDS spread quotes.
fn bump_credit_spreads(
    market: &MarketContext,
    curve_id: &finstack_quant_core::types::CurveId,
    bump_bp: f64,
) -> Result<HazardCurve> {
    let curve = market.get_hazard(curve_id.as_str())?;
    let recipe = curve.hazard_calibration().ok_or_else(|| {
        Error::invalid_input(format!(
            "Credit spread factor requires a lossless calibration recipe for '{curve_id}'"
        ))
    })?;
    let params: HazardCurveParams =
        serde_json::from_value(recipe.hazard_params.clone()).map_err(|err| {
            Error::invalid_input(format!(
                "Invalid credit replay parameters for '{curve_id}': {err}"
            ))
        })?;
    Ok(bump_hazard_spreads(
        &curve,
        market,
        &QuoteBump::ParallelBp(bump_bp),
        Some(&params.discount_curve_id),
        None,
        None,
    )?)
}

/// Shift each matched issuer's CDS spreads by `beta × delta_bp` and recalibrate.
///
/// Under the hierarchy model `Δs_i = β_i · ΔF`, a factor shock of
/// `delta_bp` moves issuer `i`'s spread by its calibrated loading times the
/// shock — the same convention the sensitivity overlay and credit
/// attribution apply.
fn shift_credit_curves(
    market: &MarketContext,
    curve_betas: &[(finstack_quant_core::types::CurveId, f64)],
    delta_bp: f64,
) -> Result<MarketContext> {
    let mut out = market.clone();
    if delta_bp == 0.0 {
        return Ok(out);
    }
    for (curve_id, beta) in curve_betas {
        let scaled = beta * delta_bp;
        if scaled == 0.0 {
            continue;
        }
        let bumped = bump_credit_spreads(&out, curve_id, scaled)?;
        out = out.insert(bumped);
    }
    Ok(out)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::sensitivity::{FactorSensitivityEngine, SensitivityMatrix};
    use crate::types::{PositionId, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{Attributes, CurveId};
    use finstack_quant_models::factor::matching::{DependencyFilter, MappingRule};
    use finstack_quant_models::factor::{
        BumpSizeConfig, CurveType, DependencyType, FactorCovarianceMatrix, FactorDefinition,
        FactorId, FactorModelConfig, FactorType, MarketMapping, PricingMode, RiskMeasure,
        UnmatchedPolicy,
    };
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::MarketDependencies;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
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
                attribute_filter: finstack_quant_models::factor::AttributeFilter::default(),
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
    fn test_builder_rejects_matching_factor_ids_not_declared_by_config() {
        let mut config = simple_config();
        config.matching = MatchingConfig::MappingTable(vec![MappingRule {
            dependency_filter: DependencyFilter::default(),
            attribute_filter: finstack_quant_models::factor::AttributeFilter::default(),
            factor_id: FactorId::new("MissingFactor"),
        }]);

        let Err(error) = FactorModelBuilder::new().config(config).build() else {
            panic!("builder must validate matching factor IDs");
        };
        assert!(error.to_string().contains("MissingFactor"), "{error}");
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
            .base_currency(Currency::USD)
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
    fn test_analyze_uses_custom_sensitivity_engine() {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };

        let sensitivity_calls = Arc::new(AtomicUsize::new(0));
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
            .with_custom_sensitivity_engine(CountingSensitivityEngine {
                calls: Arc::clone(&sensitivity_calls),
            })
            .build();
        assert!(model_result.is_ok());
        let Ok(model) = model_result else {
            return;
        };

        let portfolio = Portfolio::builder("portfolio")
            .base_currency(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .build()
            .expect("test should succeed");
        let analysis_result = model.analyze_with_sensitivities(
            &portfolio,
            &MarketContext::new(),
            date!(2024 - 01 - 01),
        );
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
            .base_currency(Currency::USD)
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
        let model = FactorModelBuilder::new()
            .config(simple_config())
            .with_custom_sensitivity_engine(WeightEchoEngine)
            .build()
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

    fn canonical_credit_bond(curve_id: CurveId) -> finstack_quant_valuations::instruments::Bond {
        use finstack_quant_models::factor::matching::ISSUER_ID_META_KEY;
        let mut bond = finstack_quant_valuations::instruments::Bond::fixed(
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
            .build()
            .expect("discount curve");
        use finstack_quant_calibration::api::{
            engine, market_datum::MarketDatum, prior_market::PriorMarketObject, schema::*,
        };
        use finstack_quant_calibration::quotes::{
            cds::CdsQuote,
            ids::{Pillar, QuoteId},
            market_quote::MarketQuote,
        };
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
                        interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                        par_interp:
                            finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                        doc_clause: Some("cr14".into()),
                        cds_valuation_convention: None,
                    }),
                }],
            },
            quotes.into_iter().map(MarketDatum::from).collect(),
            vec![PriorMarketObject::DiscountCurve(discount)],
        );
        let result = engine::execute(&request).expect("calibrate issuer spreads");
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
        let market = credit_market(as_of, id.clone()).insert_fx(FxMatrix::new(fx));
        let bumped = bump_credit_spreads(&market, &id, 1.0).unwrap();
        let base = market.get_hazard(id.as_str()).unwrap();
        assert!(bumped.hazard_calibration().is_some());
        for ((_, old), (_, new)) in base.par_spread_points().zip(bumped.par_spread_points()) {
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
            .build()
            .unwrap();
        let error =
            bump_credit_spreads(&market.insert(raw), &CurveId::new("RAW"), 1.0).unwrap_err();
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
        use finstack_quant_models::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_quant_models::factor::matching::CreditHierarchicalConfig;
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
                    require_issuer_id: false,
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
            .base_currency(Currency::USD)
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
        use finstack_quant_models::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_quant_models::factor::matching::{
            CreditHierarchicalConfig, ISSUER_ID_META_KEY,
        };
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
            FactorModelBuilder::new()
                .config(FactorModelConfig {
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
                        MatchingConfig::MappingTable(vec![
                            finstack_quant_models::factor::matching::MappingRule {
                                dependency_filter: Default::default(),
                                attribute_filter: Default::default(),
                                factor_id: FactorId::new("rates::usd"),
                            },
                        ]),
                    ]),
                    pricing_mode: PricingMode::DeltaBased,
                    risk_measure: RiskMeasure::Variance,
                    bump_size: None,
                    unmatched_policy: Some(policy),
                })
                .build()
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

        // Residual/Warn continue (Warn surfaces a tracing warning).
        for policy in [UnmatchedPolicy::Residual, UnmatchedPolicy::Warn] {
            build(policy)
                .compute_sensitivities(&portfolio, &market, as_of)
                .expect("non-strict policies must continue");
        }
    }

    #[test]
    fn credit_factor_stress_scales_curve_shift_by_calibrated_beta() {
        use finstack_quant_models::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_quant_models::factor::matching::CreditHierarchicalConfig;
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
                    require_issuer_id: false,
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
        use finstack_quant_models::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_quant_models::factor::matching::CreditHierarchicalConfig;
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
        FactorModelBuilder::new()
            .config(FactorModelConfig {
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
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .build()
            .unwrap()
    }

    /// Portfolio holding `canonical_credit_bond` positions with the given
    /// `(position_id, quantity)` pairs, all on the same issuer curve.
    fn credit_bond_portfolio(
        as_of: Date,
        curve_id: &CurveId,
        holdings: &[(&str, f64)],
    ) -> Portfolio {
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
        let flat =
            credit_bond_portfolio(as_of, &curve_id, &[("pos-long", 1.0), ("pos-short", -1.0)]);
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
}
