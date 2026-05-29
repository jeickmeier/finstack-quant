//! Vol-forecast and reporting wrapper around a calibrated [`CreditFactorModel`].
//!
//! This module is the PR-6 wiring layer between a calibrated credit factor
//! model artifact ([`CreditFactorModel`] from `finstack-core`) and the
//! existing portfolio-level risk decomposition pipeline (
//! [`crate::factor_model::FactorModel`], `RiskDecomposition`).
//!
//! # Layering note
//!
//! The original PR-6 design placed this file under
//! `finstack/valuations/src/factor_model/credit_vol_forecast.rs`. The
//! `factor_model_at` helper, however, returns a [`crate::factor_model::FactorModel`], which lives
//! in `finstack-portfolio` (and `finstack-portfolio` already depends on
//! `finstack-valuations`, so the reverse dependency is not available). To
//! keep the API surface intact without inverting the dependency graph the
//! whole module landed here in `finstack-portfolio` instead. The covariance
//! and idiosyncratic-vol helpers only use `finstack-core` types and would
//! work just as well from `finstack-valuations`; relocation is a low-cost
//! follow-up if the layering ever changes.
//!
//! # PR-6 scope
//!
//! Only the [`FactorVolModel::Sample`] variant is supported. `OneStep` and
//! `Unconditional` map to the calibrated annualized variance unchanged;
//! `NSteps(n)` multiplies variance by `n` (i.e. square-root-of-time scaling
//! when converted to vol). `VolHorizon::Custom` is intentionally **not**
//! exposed in PR-6 to keep PyO3 / WASM binding generation in PR-10/11 simple.
//!
//! # Reuse
//!
//! - Σ(t, h) = D · ρ_static · D, with D = diag(σ_factor) and ρ_static taken
//!   straight from [`CreditFactorModel::static_correlation`].
//! - Per-issuer idiosyncratic vol is sourced from
//!   [`VolState::idiosyncratic`].
//! - The factor universe is taken straight from
//!   [`CreditFactorModel::config.factors`] in canonical order.

use std::collections::BTreeMap;

use finstack_core::math::special_functions::{norm_pdf, standard_normal_inv_cdf};
use finstack_core::types::IssuerId;
use finstack_factor_model::credit::hierarchy::{
    CreditFactorModel, FactorVolModel, IdiosyncraticVolModel,
};
use finstack_factor_model::matching::CREDIT_GENERIC_FACTOR_ID;
use finstack_factor_model::{FactorCovarianceMatrix, FactorModelConfig, RiskMeasure};

use crate::factor_model::model::{FactorModel, FactorModelBuilder};
use crate::factor_model::types::RiskDecomposition;
use crate::types::PositionId;
use crate::Error as PortfolioError;
use finstack_valuations::Error as ValuationsError;

/// Forecast horizon used to scale a calibrated `Sample` vol estimate.
///
/// PR-6 supports three horizons. The `Custom` variant from the design spec is
/// intentionally **not** exposed yet to keep the PyO3 / WASM bindings (PR-10
/// / PR-11) simple to generate without serializing arbitrary scaling
/// callables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolHorizon {
    /// One-period horizon. Returns the calibrated annualized variance
    /// unchanged (Sample model).
    OneStep,
    /// `n`-period horizon. Variance scales linearly with `n`; vol therefore
    /// scales as `sqrt(n)` after the variance → vol conversion. `n = 0`
    /// returns zero variance.
    NSteps(usize),
    /// Long-run / unconditional horizon. For a [`FactorVolModel::Sample`]
    /// model the long-run variance is the sample variance, so this is
    /// numerically identical to [`Self::OneStep`] in PR-6. The variant is
    /// kept distinct so future GARCH / EWMA wiring can override the
    /// behaviour without breaking existing call sites.
    Unconditional,
}

impl VolHorizon {
    /// Parse a horizon descriptor string into a [`VolHorizon`].
    ///
    /// This is the canonical horizon-string parser shared by the PyO3 and
    /// WASM binding crates so that the accepted vocabulary stays in lockstep.
    ///
    /// Accepted forms (leading/trailing whitespace is trimmed):
    /// - `"one_step"` → [`VolHorizon::OneStep`]
    /// - `"unconditional"` → [`VolHorizon::Unconditional`]
    /// - a JSON object string `'{"n_steps": N}'` → [`VolHorizon::NSteps`]
    ///
    /// # Errors
    ///
    /// Returns a human-readable error message string if `s` is neither a
    /// recognized keyword nor a valid `{"n_steps": N}` JSON object.
    pub fn parse(s: &str) -> Result<VolHorizon, String> {
        match s.trim() {
            "one_step" => Ok(VolHorizon::OneStep),
            "unconditional" => Ok(VolHorizon::Unconditional),
            other => {
                // Try JSON object {"n_steps": N}.
                let v: serde_json::Value = serde_json::from_str(other).map_err(|_| {
                    format!(
                        "invalid horizon {other:?}: expected \"one_step\", \"unconditional\", \
                         or {{\"n_steps\": N}}"
                    )
                })?;
                let n = v
                    .get("n_steps")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| {
                        format!("invalid horizon object {other:?}: expected {{\"n_steps\": N}}")
                    })? as usize;
                Ok(VolHorizon::NSteps(n))
            }
        }
    }

    /// Apply this horizon's scaling rule to an annualized variance under the
    /// `Sample` vol model.
    fn scale_sample_variance(self, variance: f64) -> f64 {
        match self {
            Self::OneStep | Self::Unconditional => variance,
            // `n as f64` is exact for the small `n` we expect here. Casting
            // is intentional and lossless within usize values that fit in
            // f64 mantissa precision (53 bits ≈ 9e15).
            #[allow(clippy::cast_precision_loss)]
            Self::NSteps(n) => variance * (n as f64),
        }
    }
}

/// Vol-forecast view over a calibrated [`CreditFactorModel`].
///
/// The forecaster is a thin borrow over the model — it does no allocation
/// beyond what the requested horizon demands and does not mutate the
/// underlying artifact.
pub struct FactorCovarianceForecast<'a> {
    model: &'a CreditFactorModel,
}

impl<'a> FactorCovarianceForecast<'a> {
    /// Wrap a calibrated credit factor model for vol forecasting.
    #[must_use]
    pub fn new(model: &'a CreditFactorModel) -> Self {
        Self { model }
    }

    /// Build the factor covariance matrix `Σ(t, h) = D · ρ_static · D`.
    ///
    /// `D = diag(σ_factor)` where `σ_factor` is the square root of the
    /// horizon-scaled variance for each factor, in the same order as
    /// `CreditFactorModel::config::factors`.
    ///
    /// # Errors
    ///
    /// Returns [`ValuationsError::Core`] when:
    /// - a factor in `config.factors` has no entry in `vol_state.factors`,
    /// - the static correlation matrix axes do not match `config.factors`,
    /// - any computed σ² is negative (data error in the artifact),
    /// - the resulting matrix fails PSD validation in
    ///   [`FactorCovarianceMatrix::new`].
    pub fn covariance_at(
        &self,
        horizon: VolHorizon,
    ) -> Result<FactorCovarianceMatrix, ValuationsError> {
        let factor_ids: Vec<_> = self
            .model
            .config
            .factors
            .iter()
            .map(|f| f.id.clone())
            .collect();
        let n = factor_ids.len();

        // Validate ρ axes line up with factor universe.
        let rho_ids = &self.model.static_correlation.factor_ids;
        if rho_ids.as_slice() != factor_ids.as_slice() {
            return Err(ValuationsError::Core(finstack_core::Error::Validation(
                format!(
                    "FactorCovarianceForecast: static_correlation factor axes do not match \
                     config.factors (got {} ρ ids, {} config factors)",
                    rho_ids.len(),
                    n
                ),
            )));
        }

        let mut sigma = Vec::with_capacity(n);
        for fid in &factor_ids {
            let vol_model = self.model.vol_state.factors.get(fid).ok_or_else(|| {
                ValuationsError::Core(finstack_core::Error::Validation(format!(
                    "FactorCovarianceForecast: vol_state.factors is missing factor {fid}"
                )))
            })?;
            let variance = match vol_model {
                FactorVolModel::Sample { variance } => horizon.scale_sample_variance(*variance),
            };
            if variance < 0.0 {
                return Err(ValuationsError::Core(finstack_core::Error::Validation(
                    format!(
                        "FactorCovarianceForecast: negative variance {variance} for factor {fid}"
                    ),
                )));
            }
            sigma.push(variance.sqrt());
        }

        // Σ[i][j] = σ_i · ρ[i][j] · σ_j (row-major flat).
        let mut data = vec![0.0_f64; n * n];
        for i in 0..n {
            for j in 0..n {
                let rho_ij = self.model.static_correlation.data[i][j];
                data[i * n + j] = sigma[i] * rho_ij * sigma[j];
            }
        }

        FactorCovarianceMatrix::new(factor_ids, data).map_err(ValuationsError::Core)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested
    /// horizon.
    ///
    /// # Errors
    ///
    /// Returns [`ValuationsError::Core`] when the issuer is not present in
    /// `VolState::idiosyncratic` or the calibrated variance is negative.
    pub fn idiosyncratic_vol(
        &self,
        issuer_id: &IssuerId,
        horizon: VolHorizon,
    ) -> Result<f64, ValuationsError> {
        let model = self
            .model
            .vol_state
            .idiosyncratic
            .get(issuer_id)
            .ok_or_else(|| {
                ValuationsError::Core(finstack_core::Error::Validation(format!(
                    "FactorCovarianceForecast: no idiosyncratic vol model for issuer {}",
                    issuer_id.as_str()
                )))
            })?;
        let variance = match model {
            IdiosyncraticVolModel::Sample { variance } => horizon.scale_sample_variance(*variance),
        };
        if variance < 0.0 {
            return Err(ValuationsError::Core(finstack_core::Error::Validation(
                format!(
                    "FactorCovarianceForecast: negative idiosyncratic variance {variance} for \
                     issuer {}",
                    issuer_id.as_str()
                ),
            )));
        }
        Ok(variance.sqrt())
    }

    /// Build a portfolio-level [`crate::factor_model::FactorModel`] using `Σ(t, h)` at the given
    /// horizon and the requested risk measure, reusing the artifact's
    /// declarative matching / pricing-mode configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ValuationsError::Core`] when [`Self::covariance_at`] fails
    /// or when the portfolio [`FactorModelBuilder`] rejects the assembled
    /// configuration.
    pub fn factor_model_at(
        &self,
        horizon: VolHorizon,
        risk_measure: RiskMeasure,
    ) -> Result<FactorModel, ValuationsError> {
        let config = self.factor_model_config_at(horizon, risk_measure)?;
        FactorModelBuilder::new()
            .config(config)
            .build()
            .map_err(|e: PortfolioError| {
                // Portfolio errors carry their own message; wrap into the
                // valuations Error::Core surface so call sites converge on
                // a single error type.
                ValuationsError::Core(finstack_core::Error::Validation(e.to_string()))
            })
    }

    /// Build the canonical factor-model config using `Σ(t, h)` at the given
    /// horizon and requested risk measure.
    ///
    /// # Errors
    ///
    /// Returns [`ValuationsError::Core`] when [`Self::covariance_at`] fails.
    pub fn factor_model_config_at(
        &self,
        horizon: VolHorizon,
        risk_measure: RiskMeasure,
    ) -> Result<FactorModelConfig, ValuationsError> {
        let covariance = self.covariance_at(horizon)?;
        let mut config = self.model.config.clone();
        config.covariance = covariance;
        config.risk_measure = risk_measure;
        Ok(config)
    }
}

// ---------------------------------------------------------------------------
// CreditVolReport
// ---------------------------------------------------------------------------

/// Aggregated vol report grouped by hierarchy level.
///
/// Produced by [`build_credit_vol_report`] from a portfolio
/// [`RiskDecomposition`] together with the calibrated [`CreditFactorModel`].
/// All values are in the units of [`Self::measure`]. `idiosyncratic_total` is
/// computed by combining per-position residual *variances* (independent), then
/// converted to match `measure`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditVolReport {
    /// Total annualized risk under the chosen risk measure (matches
    /// [`RiskDecomposition::total_risk`]).
    pub total: f64,
    /// Risk measure used to aggregate the underlying decomposition.
    pub measure: RiskMeasure,
    /// Contribution from the generic (PC) factor `credit::generic`.
    pub generic: f64,
    /// Per-hierarchy-level rollup, indexed positionally so callers can pair
    /// each entry with the matching [`CreditFactorModel::hierarchy`] level.
    pub by_level: Vec<LevelVolContribution>,
    /// Portfolio idiosyncratic risk in the units of [`Self::measure`]: the sum
    /// of independent per-position residual *variances* (from
    /// [`RiskDecomposition::position_residual_contributions`]), converted to
    /// `measure` units — identity for Variance, `sqrt` for Volatility, and the
    /// standalone parametric tail figure carrying the loss sign for VaR/ES.
    pub idiosyncratic_total: f64,
    /// Optional per-position breakdown if requested by the caller.
    pub by_position_optional: Option<Vec<PositionVolContribution>>,
}

/// Aggregated risk contribution for a single hierarchy level.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelVolContribution {
    /// Human-readable level name, e.g. `"Rating"` for level 0 of a
    /// `(Rating, Region, Sector)` hierarchy.
    pub level_name: String,
    /// Total contribution of this level across all of its buckets.
    pub total: f64,
    /// Per-bucket contribution keyed by the bucket value path
    /// (e.g. `"IG"` or `"IG.NA.Tech"`). Stored in a [`BTreeMap`] for
    /// deterministic iteration order.
    pub by_bucket: BTreeMap<String, f64>,
}

/// Per-position vol breakdown under [`CreditVolReport`].
#[derive(Debug, Clone, PartialEq)]
pub struct PositionVolContribution {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Total factor-driven (systematic) risk for the position.
    pub factor_total: f64,
    /// Idiosyncratic (issuer-specific) risk for the position in the units of
    /// [`CreditVolReport::measure`]. Computed from the per-position residual
    /// *variance*, then converted to match `measure`: identity for Variance,
    /// `sqrt` for Volatility, and the standalone parametric tail figure
    /// (`-z·σ` for VaR, `-(φ(z)/(1−c))·σ` for ES, carrying the loss sign) for
    /// the tail measures.
    pub idiosyncratic: f64,
    /// Approximate total combining factor and idiosyncratic contributions in
    /// the units of `measure`. For Volatility, this is `factor + idio` rather
    /// than `sqrt(factor² + idio²)` — see PR-7 for refinement when
    /// factor-residual covariance becomes available.
    pub total: f64,
}

/// Build a [`CreditVolReport`] by walking a [`RiskDecomposition`] and
/// grouping factor contributions by their canonical credit factor-ID
/// prefix.
///
/// Recognized prefixes:
/// - `credit::generic` → [`CreditVolReport::generic`].
/// - `credit::level{k}::{dim_path}::{val_path}` → bucket `val_path` under
///   `by_level[k]` (level name resolved from
///   [`CreditFactorModel::hierarchy`]).
///
/// Any factor that does not match either prefix is silently ignored; this
/// keeps the report robust to portfolios that contain non-credit factors
/// (e.g. a global equity factor sharing the same `RiskDecomposition`).
///
/// `by_position = true` populates [`CreditVolReport::by_position_optional`]
/// from [`RiskDecomposition::position_factor_contributions`] and
/// [`RiskDecomposition::position_residual_contributions`].
#[must_use]
pub fn build_credit_vol_report(
    decomposition: &RiskDecomposition,
    model: &CreditFactorModel,
    by_position: bool,
) -> CreditVolReport {
    use finstack_factor_model::credit::hierarchy::HierarchyDimension;

    let n_levels = model.hierarchy.levels.len();
    let mut by_level: Vec<LevelVolContribution> = (0..n_levels)
        .map(|k| {
            let level_name = match &model.hierarchy.levels[k] {
                HierarchyDimension::Rating => "Rating".to_owned(),
                HierarchyDimension::Region => "Region".to_owned(),
                HierarchyDimension::Sector => "Sector".to_owned(),
                HierarchyDimension::Custom(name) => name.clone(),
            };
            LevelVolContribution {
                level_name,
                total: 0.0,
                by_bucket: BTreeMap::new(),
            }
        })
        .collect();

    let mut generic = 0.0_f64;

    for fc in &decomposition.factor_contributions {
        let id = fc.factor_id.as_str();
        if id == CREDIT_GENERIC_FACTOR_ID {
            generic += fc.absolute_risk;
            continue;
        }
        // Pattern: credit::level{k}::{dim_path}::{val_path}
        let Some(rest) = id.strip_prefix("credit::level") else {
            continue;
        };
        // rest = "{k}::{dim_path}::{val_path}"
        let mut parts = rest.splitn(3, "::");
        let Some(k_str) = parts.next() else {
            continue;
        };
        let Some(_dim_path) = parts.next() else {
            continue;
        };
        let Some(val_path) = parts.next() else {
            continue;
        };
        let Ok(k) = k_str.parse::<usize>() else {
            continue;
        };
        if k >= by_level.len() {
            continue;
        }
        by_level[k].total += fc.absolute_risk;
        *by_level[k]
            .by_bucket
            .entry(val_path.to_owned())
            .or_insert(0.0) += fc.absolute_risk;
    }

    // Sum per-position residual variances (independent across positions, so
    // portfolio idiosyncratic variance = sum of individual variances).
    let idiosyncratic_variance_sum: f64 = decomposition
        .position_residual_contributions
        .iter()
        .map(|c| c.residual_variance)
        .sum();
    // Convert to match `measure` (sign/scale consistent with the systematic side).
    let idiosyncratic_total =
        idiosyncratic_in_measure_units(idiosyncratic_variance_sum, decomposition.measure);

    let by_position_optional = if by_position {
        // Aggregate per-position factor totals. We key on the position id's
        // string form to preserve deterministic (lexicographic) iteration
        // order without requiring `PositionId: Ord`.
        let mut factor_by_pos: BTreeMap<String, (PositionId, f64)> = BTreeMap::new();
        for pfc in &decomposition.position_factor_contributions {
            let entry = factor_by_pos
                .entry(pfc.position_id.as_str().to_owned())
                .or_insert_with(|| (pfc.position_id.clone(), 0.0));
            entry.1 += pfc.risk_contribution;
        }
        // Accumulate residual *variances* per position (correct for independent
        // residuals). Conversion to `measure` units happens after the loop.
        let mut idio_by_pos: BTreeMap<String, (PositionId, f64)> = BTreeMap::new();
        for prc in &decomposition.position_residual_contributions {
            let entry = idio_by_pos
                .entry(prc.position_id.as_str().to_owned())
                .or_insert_with(|| (prc.position_id.clone(), 0.0));
            entry.1 += prc.residual_variance;
        }
        // Convert variance sums to match `measure`.
        for (_, variance) in idio_by_pos.values_mut() {
            *variance = idiosyncratic_in_measure_units(*variance, decomposition.measure);
        }

        let mut all_keys: std::collections::BTreeSet<String> =
            factor_by_pos.keys().cloned().collect();
        all_keys.extend(idio_by_pos.keys().cloned());

        let rows: Vec<PositionVolContribution> = all_keys
            .into_iter()
            .map(|key| {
                // `key` came from the union of `factor_by_pos` and
                // `idio_by_pos`, so it must be in at least one of them.
                // Use `or_else` to pick whichever map holds the position id;
                // the fallback branch is unreachable by invariant.
                let (factor_total, pid) = if let Some((p, v)) = factor_by_pos.get(&key) {
                    (*v, p.clone())
                } else if let Some((p, _)) = idio_by_pos.get(&key) {
                    (0.0, p.clone())
                } else {
                    // Unreachable: key is guaranteed to be in at least one map.
                    (0.0, PositionId::new(&key))
                };
                let idiosyncratic = idio_by_pos.get(&key).map(|(_, v)| *v).unwrap_or(0.0);
                PositionVolContribution {
                    position_id: pid,
                    factor_total,
                    idiosyncratic,
                    total: factor_total + idiosyncratic,
                }
            })
            .collect();
        Some(rows)
    } else {
        None
    };

    CreditVolReport {
        total: decomposition.total_risk,
        measure: decomposition.measure,
        generic,
        by_level,
        idiosyncratic_total,
        by_position_optional,
    }
}

/// Convert an idiosyncratic (residual) *variance* into the units of `measure`,
/// matching the sign/scale convention used for the systematic side
/// ([`RiskDecomposition::total_risk`]).
///
/// Residual contributions are independent across positions, so a portfolio (or
/// per-position) idiosyncratic variance maps to:
/// - [`RiskMeasure::Variance`] → the variance unchanged;
/// - [`RiskMeasure::Volatility`] → its square root (a volatility);
/// - [`RiskMeasure::VaR`] → the standalone parametric VaR `-z_c · σ`, where
///   `z_c = Φ⁻¹(c)`;
/// - [`RiskMeasure::ExpectedShortfall`] → `-(φ(z_c) / (1 − c)) · σ`.
///
/// The tail measures carry the loss (negative) sign so the idiosyncratic figure
/// is directly comparable to the systematic total. The `-z·σ` and
/// `-(φ(z)/(1−c))·σ` forms are exactly what the systematic simulation converges
/// to for a normal factor model.
fn idiosyncratic_in_measure_units(variance: f64, measure: RiskMeasure) -> f64 {
    match measure {
        RiskMeasure::Variance => variance,
        RiskMeasure::Volatility => variance.max(0.0).sqrt(),
        RiskMeasure::VaR { confidence } => {
            -standard_normal_inv_cdf(confidence) * variance.max(0.0).sqrt()
        }
        RiskMeasure::ExpectedShortfall { confidence } => {
            let z = standard_normal_inv_cdf(confidence);
            -(norm_pdf(z) / (1.0 - confidence)) * variance.max(0.0).sqrt()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::create_date;
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::types::{CurveId, IssuerId};
    use finstack_factor_model::credit::hierarchy::{
        CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
        FactorCorrelationMatrix, FactorVolModel, GenericFactorSpec, HierarchyDimension,
        IdiosyncraticVolModel, IssuerBetaPolicy, LevelsAtAnchor, VolState,
    };
    use finstack_factor_model::matching::CreditHierarchicalConfig;
    use finstack_factor_model::{
        FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
        MarketMapping, MatchingConfig, PricingMode, RiskMeasure,
    };
    use std::collections::BTreeMap;
    use time::Month;

    use crate::factor_model::types::{
        FactorContribution, PositionResidualContribution, ResidualContributionSource,
        RiskDecomposition,
    };
    use crate::types::PositionId;

    // ----- Helpers ---------------------------------------------------------

    fn rates_factor_id() -> FactorId {
        FactorId::new("Rates")
    }

    fn rates_factor() -> FactorDefinition {
        FactorDefinition {
            id: rates_factor_id(),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }
    }

    fn minimal_config(
        factors: Vec<FactorDefinition>,
        cov: FactorCovarianceMatrix,
    ) -> FactorModelConfig {
        FactorModelConfig {
            factors,
            covariance: cov,
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        }
    }

    fn empty_diagnostics() -> CalibrationDiagnostics {
        CalibrationDiagnostics {
            mode_counts: BTreeMap::new(),
            bucket_sizes_per_level: vec![],
            fold_ups: vec![],
            r_squared_histogram: None,
            tag_taxonomy: BTreeMap::new(),
        }
    }

    /// Two-factor model: `Rates` and `Credit`, each with σ²=0.04, ρ=0.5.
    fn two_factor_model() -> CreditFactorModel {
        let rates = rates_factor_id();
        let credit = FactorId::new("Credit");
        let factors = vec![
            rates_factor(),
            FactorDefinition {
                id: credit.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("CDX-IG")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ];
        // ρ = [[1, 0.5], [0.5, 1]]
        let rho = FactorCorrelationMatrix::new(
            vec![rates.clone(), credit.clone()],
            vec![vec![1.0, 0.5], vec![0.5, 1.0]],
        )
        .unwrap();
        // Bootstrapping config covariance is the identity (it gets overwritten
        // by `factor_model_at`), but it must be PSD and axes-aligned.
        let cov = FactorCovarianceMatrix::new(
            vec![rates.clone(), credit.clone()],
            vec![1.0, 0.0, 0.0, 1.0],
        )
        .unwrap();
        let mut vol_factors = BTreeMap::new();
        vol_factors.insert(rates.clone(), FactorVolModel::Sample { variance: 0.04 });
        vol_factors.insert(credit.clone(), FactorVolModel::Sample { variance: 0.04 });

        CreditFactorModel {
            schema_version: CreditFactorModel::SCHEMA_VERSION.to_owned(),
            as_of: create_date(2024, Month::March, 29).unwrap(),
            calibration_window: DateRange {
                start: create_date(2022, Month::March, 29).unwrap(),
                end: create_date(2024, Month::March, 29).unwrap(),
            },
            policy: IssuerBetaPolicy::GloballyOff,
            generic_factor: GenericFactorSpec {
                name: "CDX IG 5Y".to_owned(),
                series_id: "cdx.ig.5y".to_owned(),
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            config: minimal_config(factors, cov),
            issuer_betas: vec![],
            anchor_state: LevelsAtAnchor {
                pc: 0.0,
                by_level: vec![],
            },
            static_correlation: rho,
            vol_state: VolState {
                factors: vol_factors,
                idiosyncratic: BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: empty_diagnostics(),
        }
    }

    fn is_psd(data: &[f64], n: usize) -> bool {
        // Quick PSD check via Cholesky-style decomposition with tolerance.
        let mut l = vec![0.0_f64; n * n];
        for i in 0..n {
            for j in 0..=i {
                let mut sum = data[i * n + j];
                for k in 0..j {
                    sum -= l[i * n + k] * l[j * n + k];
                }
                if i == j {
                    if sum < -1e-10 {
                        return false;
                    }
                    l[i * n + j] = sum.max(0.0).sqrt();
                } else if l[j * n + j] > 0.0 {
                    l[i * n + j] = sum / l[j * n + j];
                } else if sum.abs() > 1e-10 {
                    return false;
                }
            }
        }
        true
    }

    // ----- Tests -----------------------------------------------------------

    #[test]
    fn vol_horizon_parse_recognizes_keywords_and_nsteps() {
        assert_eq!(VolHorizon::parse("one_step").unwrap(), VolHorizon::OneStep);
        assert_eq!(
            VolHorizon::parse("  unconditional  ").unwrap(),
            VolHorizon::Unconditional
        );
        assert_eq!(
            VolHorizon::parse(r#"{"n_steps": 7}"#).unwrap(),
            VolHorizon::NSteps(7)
        );
    }

    #[test]
    fn vol_horizon_parse_rejects_invalid_input() {
        let err = VolHorizon::parse("nonsense").expect_err("must reject unknown keyword");
        assert!(err.contains("invalid horizon"));
        let err2 =
            VolHorizon::parse(r#"{"steps": 3}"#).expect_err("must reject object missing n_steps");
        assert!(err2.contains("n_steps"));
    }

    #[test]
    fn forecast_covariance_is_psd() {
        let model = two_factor_model();
        let forecast = FactorCovarianceForecast::new(&model);
        let cov = forecast
            .covariance_at(VolHorizon::OneStep)
            .expect("covariance");
        assert!(is_psd(cov.as_slice(), cov.n_factors()));

        // Also check NSteps and Unconditional remain PSD.
        let cov_n = forecast
            .covariance_at(VolHorizon::NSteps(5))
            .expect("covariance n=5");
        assert!(is_psd(cov_n.as_slice(), cov_n.n_factors()));
        let cov_u = forecast
            .covariance_at(VolHorizon::Unconditional)
            .expect("covariance unconditional");
        assert!(is_psd(cov_u.as_slice(), cov_u.n_factors()));
    }

    #[test]
    fn one_step_and_unconditional_sample_vol_are_consistent() {
        let model = two_factor_model();
        let forecast = FactorCovarianceForecast::new(&model);
        let one = forecast.covariance_at(VolHorizon::OneStep).unwrap();
        let uncond = forecast.covariance_at(VolHorizon::Unconditional).unwrap();
        assert_eq!(one.n_factors(), uncond.n_factors());
        for (a, b) in one.as_slice().iter().zip(uncond.as_slice().iter()) {
            assert!((a - b).abs() < 1e-12, "Σ_one={a} vs Σ_uncond={b}");
        }
    }

    #[test]
    fn bucket_only_issuer_vol_uses_cached_scalar_for_all_horizons() {
        // BucketOnly issuers, like any issuer in PR-6, get their idiosyncratic
        // variance straight from `vol_state.idiosyncratic`. Under the Sample
        // model, OneStep and Unconditional must return the cached σ exactly.
        let mut model = two_factor_model();
        let issuer = IssuerId::new("ACME");
        model.vol_state.idiosyncratic.insert(
            issuer.clone(),
            IdiosyncraticVolModel::Sample { variance: 0.09 },
        );

        let forecast = FactorCovarianceForecast::new(&model);
        let one_step = forecast
            .idiosyncratic_vol(&issuer, VolHorizon::OneStep)
            .unwrap();
        let uncond = forecast
            .idiosyncratic_vol(&issuer, VolHorizon::Unconditional)
            .unwrap();
        let three_step = forecast
            .idiosyncratic_vol(&issuer, VolHorizon::NSteps(3))
            .unwrap();

        let cached = 0.09_f64.sqrt();
        assert!((one_step - cached).abs() < 1e-12);
        assert!((uncond - cached).abs() < 1e-12);
        // NSteps scales variance by n, so vol scales by sqrt(n).
        assert!((three_step - (0.09_f64 * 3.0).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn idiosyncratic_vol_unknown_issuer_returns_typed_error() {
        let model = two_factor_model();
        let forecast = FactorCovarianceForecast::new(&model);
        let err = forecast
            .idiosyncratic_vol(&IssuerId::new("MISSING"), VolHorizon::OneStep)
            .expect_err("must fail");
        assert!(err.to_string().contains("MISSING"));
    }

    #[test]
    fn nsteps_scales_variance_linearly() {
        let model = two_factor_model();
        let forecast = FactorCovarianceForecast::new(&model);
        let one = forecast.covariance_at(VolHorizon::OneStep).unwrap();
        let four = forecast.covariance_at(VolHorizon::NSteps(4)).unwrap();
        // Σ(4) = 4 · Σ(1) under Sample model (since σ² scales by n and
        // covariance = σ_i · ρ · σ_j scales by n).
        for (a, b) in one.as_slice().iter().zip(four.as_slice().iter()) {
            assert!((4.0 * a - b).abs() < 1e-12, "expected 4·{a}={b}");
        }
    }

    #[test]
    fn factor_model_at_uses_horizon_covariance() {
        // Use a hierarchical matching config so the rebuilt FactorModel has
        // a valid matcher for the configured factors. The factors stay
        // unchanged but the covariance is now horizon-scaled.
        let mut model = two_factor_model();
        model.config.matching = MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
            dependency_filter: Default::default(),
            hierarchy: model.hierarchy.clone(),
            issuer_betas: vec![],
        });

        let forecast = FactorCovarianceForecast::new(&model);
        let fm = forecast
            .factor_model_at(VolHorizon::OneStep, RiskMeasure::Volatility)
            .expect("model");
        assert_eq!(fm.factors().len(), 2);
    }

    #[test]
    fn credit_vol_report_groups_by_level_prefix() {
        let model = two_factor_model();

        // Hand-built decomposition with one entry per recognized prefix.
        let decomposition = RiskDecomposition {
            total_risk: 100.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![
                FactorContribution {
                    factor_id: FactorId::new("credit::generic"),
                    absolute_risk: 30.0,
                    relative_risk: 0.3,
                    marginal_risk: 0.0,
                },
                FactorContribution {
                    factor_id: FactorId::new("credit::level0::Rating::IG"),
                    absolute_risk: 20.0,
                    relative_risk: 0.2,
                    marginal_risk: 0.0,
                },
                FactorContribution {
                    factor_id: FactorId::new("credit::level0::Rating::HY"),
                    absolute_risk: 15.0,
                    relative_risk: 0.15,
                    marginal_risk: 0.0,
                },
                FactorContribution {
                    factor_id: FactorId::new("credit::level1::Rating.Sector::IG.Tech"),
                    absolute_risk: 25.0,
                    relative_risk: 0.25,
                    marginal_risk: 0.0,
                },
                FactorContribution {
                    factor_id: FactorId::new("equity::beta"),
                    absolute_risk: 10.0,
                    relative_risk: 0.1,
                    marginal_risk: 0.0,
                },
            ],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![PositionResidualContribution {
                position_id: PositionId::new("pos-1"),
                residual_variance: 4.0,
                source: ResidualContributionSource::FromCreditModel {
                    issuer_id: IssuerId::new("ACME"),
                },
            }],
        };

        let report = build_credit_vol_report(&decomposition, &model, true);

        assert!((report.total - 100.0).abs() < 1e-12);
        assert_eq!(report.measure, RiskMeasure::Variance);
        assert!((report.generic - 30.0).abs() < 1e-12);

        // Two levels: Rating, Sector.
        assert_eq!(report.by_level.len(), 2);
        assert_eq!(report.by_level[0].level_name, "Rating");
        assert!((report.by_level[0].total - 35.0).abs() < 1e-12);
        assert!(
            (report.by_level[0]
                .by_bucket
                .get("IG")
                .copied()
                .unwrap_or(0.0)
                - 20.0)
                .abs()
                < 1e-12
        );
        assert!(
            (report.by_level[0]
                .by_bucket
                .get("HY")
                .copied()
                .unwrap_or(0.0)
                - 15.0)
                .abs()
                < 1e-12
        );
        assert_eq!(report.by_level[1].level_name, "Sector");
        assert!((report.by_level[1].total - 25.0).abs() < 1e-12);
        assert!(
            (report.by_level[1]
                .by_bucket
                .get("IG.Tech")
                .copied()
                .unwrap_or(0.0)
                - 25.0)
                .abs()
                < 1e-12
        );

        // Idiosyncratic: variance contribution sums.
        assert!((report.idiosyncratic_total - 4.0).abs() < 1e-12);

        let by_pos = report.by_position_optional.expect("by_position rows");
        assert_eq!(by_pos.len(), 1);
        assert_eq!(by_pos[0].position_id, PositionId::new("pos-1"));
        // measure=Variance: idiosyncratic stays as raw variance (4.0), not sqrt.
        assert!((by_pos[0].idiosyncratic - 4.0).abs() < 1e-12);
        assert!((by_pos[0].factor_total).abs() < 1e-12);
        assert!((by_pos[0].total - 4.0).abs() < 1e-12);
    }

    #[test]
    fn credit_vol_report_skips_unknown_prefixes() {
        let model = two_factor_model();
        let decomposition = RiskDecomposition {
            total_risk: 5.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![FactorContribution {
                factor_id: FactorId::new("rates::usd::5y"),
                absolute_risk: 5.0,
                relative_risk: 1.0,
                marginal_risk: 0.0,
            }],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
        };
        let report = build_credit_vol_report(&decomposition, &model, false);
        assert!(report.generic.abs() < 1e-12);
        assert!(report.by_level.iter().all(|l| l.total.abs() < 1e-12));
        assert!(report.by_position_optional.is_none());
    }

    #[test]
    fn credit_vol_report_idiosyncratic_in_var_units() {
        use finstack_core::math::special_functions::standard_normal_inv_cdf;

        let model = two_factor_model();
        let confidence = 0.99;
        let decomposition = RiskDecomposition {
            total_risk: -50.0,
            measure: RiskMeasure::VaR { confidence },
            factor_contributions: vec![],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![
                PositionResidualContribution {
                    position_id: PositionId::new("pos-1"),
                    residual_variance: 4.0,
                    source: ResidualContributionSource::FromCreditModel {
                        issuer_id: IssuerId::new("ACME"),
                    },
                },
                PositionResidualContribution {
                    position_id: PositionId::new("pos-2"),
                    residual_variance: 5.0,
                    source: ResidualContributionSource::FromCreditModel {
                        issuer_id: IssuerId::new("BETA"),
                    },
                },
            ],
        };

        let report = build_credit_vol_report(&decomposition, &model, true);

        // VaR idiosyncratic must be reported in loss (negative) units consistent
        // with the systematic total: -z_c * sqrt(sum of independent residual
        // variances).
        let z = standard_normal_inv_cdf(confidence);
        let expected_total = -z * (4.0_f64 + 5.0).sqrt();
        assert!(
            (report.idiosyncratic_total - expected_total).abs() < 1e-9,
            "idio VaR units: got {}, expected {}",
            report.idiosyncratic_total,
            expected_total
        );
        assert!(
            report.idiosyncratic_total < 0.0,
            "VaR idiosyncratic must carry the loss sign"
        );

        // Per-position idiosyncratic uses the same convention.
        let by_pos = report.by_position_optional.expect("by_position rows");
        let pos1 = by_pos
            .iter()
            .find(|p| p.position_id == PositionId::new("pos-1"))
            .expect("pos-1 present");
        let expected_pos1 = -z * 4.0_f64.sqrt();
        assert!((pos1.idiosyncratic - expected_pos1).abs() < 1e-9);
    }
}
