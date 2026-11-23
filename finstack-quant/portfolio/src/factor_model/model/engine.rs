//! [`FactorModel`] risk decomposition, stress testing, and credit idiosyncratic variance.

use super::super::assignment::{FactorAssignmentReport, assign_position_factors};
use super::super::dependencies::factor_dependencies;
use crate::error::{Error, Result};
use crate::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, exact_factor_market_keys, portfolio_rows,
    uses_assignment_driven_credit_shock,
};
use crate::{Portfolio, dependencies::MarketFactorKey};
use finstack_quant_calibration::api::schema::HazardCurveParams;
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_factor::ISSUER_ID_META_KEY;
use finstack_quant_factor::SensitivityMatrix;
use finstack_quant_factor::risk::{
    PositionResidualContribution, ResidualContributionSource, RiskDecomposition,
    apply_residual_contributions, decompose_factors,
};
use finstack_quant_factor::{
    BumpSizeConfig, FactorCovarianceMatrix, FactorDefinition, FactorModelConfig, MarketDependency,
    MatchingConfig, RiskMeasure, UnmatchedPolicy,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::recalibration::QuoteBump;
use std::collections::{BTreeMap, HashMap};
/// Portfolio-level factor-model orchestrator.
///
/// A `FactorModel` owns the factor definitions, covariance matrix, and the
/// pluggable engines required to move from instrument dependencies to
/// portfolio-level risk decomposition.
pub struct FactorModel {
    pub(super) credit_idiosyncratic_variance: BTreeMap<finstack_quant_core::types::IssuerId, f64>,
    pub(super) factors: Vec<FactorDefinition>,
    pub(crate) covariance: FactorCovarianceMatrix,
    pub(super) matcher: Box<dyn finstack_quant_factor::FactorMatcher>,
    pub(super) sensitivity_engine: Box<dyn FactorSensitivityEngine>,
    pub(crate) risk_measure: RiskMeasure,
    pub(super) unmatched_policy: UnmatchedPolicy,
    pub(super) bump_config: BumpSizeConfig,
}

impl FactorModel {
    /// Build a factor model directly from a declarative configuration.
    ///
    /// Python and WASM handles use the same validated constructor.
    ///
    /// # Arguments
    ///
    /// * `config` - Factor definitions, covariance matrix (axes in factor
    ///   order), dependency matching rules, bump sizes and risk
    ///   measure. Validated before the model is assembled.
    ///
    /// # Returns
    ///
    /// A configured [`FactorModel`] ready to assign factors, compute
    /// sensitivities, and decompose risk.
    ///
    /// # Errors
    ///
    /// Returns the [`FactorModelConfig::validate`] error when matching rules
    /// reference undeclared factor IDs, the risk measure is invalid, or the
    /// covariance axes do not align with the configured factors, and a
    /// sensitivity-engine construction error for invalid bump settings.
    pub fn from_config(config: FactorModelConfig) -> Result<Self> {
        config.validate()?;

        let matcher = config.matching.build_matcher();
        let bump_config = config.bump_config.clone().unwrap_or_default();
        let sensitivity_engine = Box::new(DeltaBasedEngine::new(bump_config.clone()))
            as Box<dyn FactorSensitivityEngine>;
        Ok(FactorModel {
            credit_idiosyncratic_variance: credit_idiosyncratic_variance(&config.matching),
            factors: config.factors,
            covariance: config.covariance,
            matcher,
            sensitivity_engine,
            risk_measure: config.risk_measure,
            unmatched_policy: config.unmatched_policy.unwrap_or_default(),
            bump_config,
        })
    }

    /// Replace the sensitivity engine for module tests.
    #[cfg(test)]
    pub(crate) fn with_custom_sensitivity_engine(
        mut self,
        sensitivity_engine: impl FactorSensitivityEngine + 'static,
    ) -> Self {
        self.sensitivity_engine = Box::new(sensitivity_engine);
        self
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
    /// * `market` - Request market used to resolve credit-index aggregates into
    ///   their hazard, issuer-hazard, and base-correlation curve dependencies.
    ///
    /// # Returns
    ///
    /// Assignment report including both successful matches and unmatched
    /// dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a position cannot report dependencies, a referenced
    /// credit index is absent from `market`, or the unmatched policy is strict
    /// and at least one dependency cannot be mapped.
    pub fn assign_factors(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
    ) -> Result<FactorAssignmentReport> {
        let mut assignments = Vec::with_capacity(portfolio.positions.len());
        let mut unmatched = Vec::new();

        for position in &portfolio.positions {
            let dependencies =
                factor_dependencies(&position.instrument.market_dependencies()?, market)?;
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

    pub(super) fn compute_sensitivities_with_credit_exposures(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        credit_exposures: &mut CreditExposureMatrix<'_>,
    ) -> Result<SensitivityMatrix> {
        let assignment_report = self.assign_factors(portfolio, market)?;
        // Same rows as `sensitivity::compute_factor_sensitivities`; this path
        // additionally overlays the assignment-driven credit-hierarchy exposure
        // that the free function rejects.
        let mut sensitivities = self.sensitivity_engine.compute_sensitivities(
            &portfolio_rows(portfolio),
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

    pub(super) fn overlay_assignment_driven_credit_sensitivities(
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
                let Some(curve_id) = credit_curve_id(dependency) else {
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
                    // Warn surfaces the drop.
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
                let current = sensitivities.delta(position_idx, factor_idx)?;
                let weighted_delta = current + *beta * delta;
                if !weighted_delta.is_finite() {
                    return Err(Error::invalid_input(format!(
                        "non-finite assignment-weighted credit sensitivity for position '{}' on factor '{}' (beta = {beta}, exposure = {delta})",
                        position.position_id, factor_id
                    )));
                }
                sensitivities.set_delta(position_idx, factor_idx, weighted_delta);
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
    pub(crate) fn analyze_with_sensitivities(
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
            decompose_factors(&sensitivities, &self.covariance, &self.risk_measure)?;
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

    pub(super) fn add_credit_residual_risk_with_credit_exposures(
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
            let dependencies = factor_dependencies(
                &position.instrument.market_dependencies()?,
                credit_exposures.bump_contexts.base,
            )?;
            let mut exposure = 0.0;
            for dependency in &dependencies {
                if let Some(curve_id) = credit_curve_id(dependency) {
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

    /// Build one stressed market and its exact selective-repricing manifest.
    ///
    /// Assignment-driven credit curve discovery is performed once per stressed
    /// factor and reused for both the market shock and dependency keys.
    pub(crate) fn stressed_market_with_factor_keys(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        stresses: &[(finstack_quant_factor::FactorId, f64)],
    ) -> Result<(MarketContext, Option<Vec<MarketFactorKey>>)> {
        use crate::sensitivity::mapping_to_market_bumps;

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
                    &stressed,
                    &factor.market_mapping,
                    shift,
                    factor.factor_type.bump_units(),
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
    pub(super) fn credit_curves_matched_to_factor(
        &self,
        portfolio: &Portfolio,
        market: &MarketContext,
        factor_id: &finstack_quant_factor::FactorId,
    ) -> Result<Vec<(finstack_quant_core::types::CurveId, f64)>> {
        let mut curve_betas: BTreeMap<finstack_quant_core::types::CurveId, f64> = BTreeMap::new();
        for position in &portfolio.positions {
            let dependencies =
                factor_dependencies(&position.instrument.market_dependencies()?, market)?;
            for dependency in &dependencies {
                let Some(curve_id) = credit_curve_id(dependency) else {
                    continue;
                };
                let Some(entries) = self
                    .matcher
                    .match_factor(dependency, position.instrument.attributes())
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

pub(super) fn credit_idiosyncratic_variance(
    matching: &MatchingConfig,
) -> BTreeMap<finstack_quant_core::types::IssuerId, f64> {
    let mut out = BTreeMap::new();
    for config in matching.credit_configs() {
        for row in &config.issuer_betas {
            out.insert(
                row.issuer_id.clone(),
                row.adder_vol_annualized * row.adder_vol_annualized,
            );
        }
    }
    out
}

pub(super) fn credit_curve_id(
    dependency: &MarketDependency,
) -> Option<finstack_quant_core::types::CurveId> {
    match dependency {
        MarketDependency::CreditCurve { id } => Some(id.clone()),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CreditBumpKey {
    pub(super) curve_id: finstack_quant_core::types::CurveId,
    pub(super) bump_bits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CreditExposureKey {
    pub(super) position_index: usize,
    pub(super) curve_id: finstack_quant_core::types::CurveId,
    pub(super) bump_bits: u64,
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
pub(super) struct CreditExposureMatrix<'a> {
    pub(super) base_currency: Currency,
    pub(super) bump_contexts: CreditBumpContexts<'a>,
    pub(super) exposures: HashMap<CreditExposureKey, f64>,
}

impl<'a> CreditExposureMatrix<'a> {
    pub(super) fn new(base: &'a MarketContext, base_currency: Currency) -> Self {
        Self {
            base_currency,
            bump_contexts: CreditBumpContexts::new(base),
            exposures: HashMap::default(),
        }
    }

    pub(super) fn exposure(
        &mut self,
        position_index: usize,
        instrument: &dyn Instrument,
        quantity: f64,
        as_of: Date,
        curve_id: &finstack_quant_core::types::CurveId,
        bump_size: f64,
    ) -> Result<f64> {
        if !quantity.is_finite() {
            return Err(Error::invalid_input(format!(
                "credit sensitivity quantity must be finite for position {position_index}, got {quantity}"
            )));
        }
        if !bump_size.is_finite() || bump_size.abs() < f64::EPSILON {
            return Err(Error::invalid_input(
                "credit factor bump size must be finite and non-zero for sensitivity computation",
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
        let pv_up = crate::fx::raw_pv_in_base(instrument, up, as_of, self.base_currency)?;
        let pv_down = crate::fx::raw_pv_in_base(instrument, down, as_of, self.base_currency)?;
        if !pv_up.is_finite() || !pv_down.is_finite() {
            return Err(Error::invalid_input(format!(
                "non-finite bumped credit PV for position {position_index} on curve '{curve_id}' (up = {pv_up}, down = {pv_down})"
            )));
        }
        let exposure = (pv_up - pv_down) / (2.0 * bump_size) * quantity;
        if !exposure.is_finite() {
            return Err(Error::invalid_input(format!(
                "non-finite weighted credit sensitivity for position {position_index} on curve '{curve_id}' ({exposure}); check the position quantity and bumped PV difference"
            )));
        }
        self.exposures.insert(key, exposure);
        Ok(exposure)
    }
}

pub(super) struct CreditBumpContexts<'a> {
    pub(super) base: &'a MarketContext,
    pub(super) contexts: HashMap<CreditBumpKey, (MarketContext, MarketContext)>,
}

impl<'a> CreditBumpContexts<'a> {
    pub(super) fn new(base: &'a MarketContext) -> Self {
        Self {
            base,
            contexts: HashMap::new(),
        }
    }

    pub(super) fn get(
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
                    self.base.clone().try_insert(up_curve)?,
                    self.base.clone().try_insert(down_curve)?,
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
pub(super) fn bump_credit_spreads(
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
    let params: HazardCurveParams = serde_json::from_value(recipe.hazard_params().clone())
        .map_err(|err| {
            Error::invalid_input(format!(
                "Invalid credit replay parameters for '{curve_id}': {err}"
            ))
        })?;
    Ok(bump_hazard_spreads(
        &curve,
        market,
        &QuoteBump::ParallelBp(bump_bp),
        &params.discount_curve_id,
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
pub(super) fn shift_credit_curves(
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
        out.try_insert_mut(bumped)?;
    }
    Ok(out)
}
