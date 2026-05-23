//! Deterministic calibrator that produces a [`CreditFactorModel`] artifact from
//! sparse issuer-spread history (PR-4 MVP).
//!
//! # Algorithm overview
//!
//! The calibration is a sequential "peel-the-onion" identical in structure to
//! [`crate::credit_decomposition::decompose_levels`] but operating
//! on a *time series* of issuer spreads rather than a single snapshot:
//!
//! 1. Classify each issuer as `IssuerBeta` or `BucketOnly` based on the
//!    [`IssuerBetaPolicy`] and per-issuer [`IssuerBetaOverride`].
//! 2. Optionally convert the spread panel to a return panel (default).
//! 3. Inventory hierarchy buckets and fold up under-populated buckets.
//! 4. Regress each issuer's residual on the generic factor (PC peel).
//! 5. Peel hierarchy levels one at a time: cross-sectional bucket means become
//!    bucket factor returns, and `IssuerBeta` issuers fit a per-level β.
//! 6. After the last level, the remaining residual is the issuer adder.
//! 7. Anchor every factor's level value at `as_of` using the same peeling logic
//!    on a single observation in level space.
//! 8. Estimate per-factor variance via the sample variance.
//! 9. Assemble correlation and covariance per [`CovarianceStrategy`]:
//!    `Diagonal` → identity ρ, Σ = diag(σ²); `Ridge` → sample ρ (PSD-repaired
//!    if needed), Σ = D·ρ·D + α·I; `FullSampleRepaired` → sample ρ repaired
//!    to PSD, Σ = D·ρ_repaired·D.
//! 10. Assemble [`FactorModelConfig`] with `MatchingConfig::CreditHierarchical`.
//! 11. Build [`CalibrationDiagnostics`] from the bookkeeping above.
//! 12. Return the assembled [`CreditFactorModel`] after a final
//!     [`CreditFactorModel::validate`] check.
//!
//! # Determinism
//!
//! Every keyed map is a [`BTreeMap`] and every iteration order is stable. Two
//! calibrations with the same inputs serialize to byte-identical JSON.
//!
//! # Reuse with PR-3
//!
//! The anchoring step (step 7) implements the same math as
//! [`decompose_levels`][crate::credit_decomposition::decompose_levels]
//! but is called via a private helper because we don't yet have a fully
//! assembled [`CreditFactorModel`] at that point in the pipeline.
//!
//! # Determinism note on OLS
//!
//! The OLS slope `β = Cov(y, x) / Var(x)` is delegated to
//! `finstack_analytics::beta`, which implements the same math
//! and is deterministic for the same input slice.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use finstack_core::dates::Date;
use finstack_core::factor_model::credit_hierarchy::{
    dimension_key, AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec,
    DateRange, FactorCorrelationMatrix, FactorHistories, FactorVolModel, FitQuality, FoldUpRecord,
    GenericFactorSpec, IdiosyncraticVolModel, IssuerBetaMode, IssuerBetaOverride, IssuerBetaPolicy,
    IssuerBetaRow, IssuerBetas, IssuerTags, LevelAnchor, LevelsAtAnchor, VolState,
};

use finstack_analytics::correlation::nearest_correlation::{
    nearest_correlation_matrix, NearestCorrelationOpts,
};
use finstack_analytics::correlation::validate_correlation_matrix;
use finstack_core::factor_model::matching::{
    bucket_factor_id, CreditHierarchicalConfig, CREDIT_GENERIC_FACTOR_ID,
};
use finstack_core::factor_model::{
    FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
    MarketMapping, MatchingConfig, PricingMode,
};
use finstack_core::market_data::bumps::BumpUnits;
use finstack_core::types::IssuerId;

use finstack_core::{Error, Result};

// ---------------------------------------------------------------------------
// Public configuration types
// ---------------------------------------------------------------------------

/// Whether the calibrator works in price-difference (return) or raw-level space.
///
/// `Returns` (the default) matches the spec's reference math: `r_i(t) =
/// S_i(t) - S_i(t-1)` and the generic factor is differenced the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelSpace {
    /// Difference consecutive observations into a return panel before peeling.
    #[default]
    Returns,
    /// Use the raw level panel as-is.
    Levels,
}

/// Volatility model selector for the per-factor variance forecast.
///
/// PR-4 supports `Sample` only. The other variants are accepted at the type
/// level so that downstream code does not break when those PRs land, but the
/// calibrator returns a clean error if any non-`Sample` variant is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolModelChoice {
    /// Plain sample variance (PR-4).
    Sample,
    /// GARCH(1,1) — deferred to PR-5a.
    Garch,
    /// EGARCH — deferred to PR-5a.
    Egarch,
    /// EWMA with smoothing parameter `lambda` — deferred to PR-5a.
    Ewma {
        /// Smoothing parameter.
        lambda: f64,
    },
}

/// Strategy for assembling the factor covariance matrix.
///
/// PR-4 supports `Diagonal` only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CovarianceStrategy {
    /// Diagonal Σ = diag(σ²) under identity correlation (PR-4 default).
    Diagonal,
    /// Sample correlation (PSD-repaired if needed) plus diagonal ridge:
    /// Σ = D·ρ·D + α·I. Requires `alpha >= 0`. See design spec §4.1.
    Ridge {
        /// Ridge regularisation parameter; must be `>= 0`.
        alpha: f64,
    },
    /// Full sample covariance with PSD repair via nearest-correlation projection:
    /// Σ = D·ρ_repaired·D. See design spec §4.1.
    FullSampleRepaired,
}

/// OLS β shrinkage rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaShrinkage {
    /// No shrinkage; use the OLS estimate directly.
    None,
    /// Convex shrinkage toward 1.0: `β ← (1 - α) · β_fit + α · 1.0`.
    TowardOne {
        /// Shrinkage weight in `[0, 1]`.
        alpha: f64,
    },
}

/// Per-level minimum-bucket-size thresholds used to gate fold-up of sparse
/// hierarchy buckets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketSizeThresholds {
    /// Threshold per hierarchy level. Levels beyond `per_level.len()` use the
    /// default of 5.
    pub per_level: Vec<usize>,
}

impl BucketSizeThresholds {
    fn threshold_for_level(&self, k: usize) -> usize {
        self.per_level.get(k).copied().unwrap_or(5)
    }

    /// Default thresholds for `n` hierarchy levels (5 per level).
    #[must_use]
    pub fn default_for_levels(n: usize) -> Self {
        Self {
            per_level: vec![5; n],
        }
    }
}

/// Configuration for the calibrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditCalibrationConfig {
    /// Issuer-beta classification policy.
    pub policy: IssuerBetaPolicy,
    /// Hierarchy specification (broadest → narrowest).
    pub hierarchy: CreditHierarchySpec,
    /// Per-level minimum-bucket-size thresholds.
    pub min_bucket_size_per_level: BucketSizeThresholds,
    /// Vol-model choice for the per-factor variance forecast (PR-4: `Sample` only).
    pub vol_model: VolModelChoice,
    /// Covariance assembly strategy (PR-4: `Diagonal` only).
    pub covariance_strategy: CovarianceStrategy,
    /// Optional shrinkage applied to OLS β estimates.
    pub beta_shrinkage: BetaShrinkage,
    /// Whether to differentiate the panel before peeling.
    pub use_returns_or_levels: PanelSpace,
    /// Annualization factor for sample variance (default 12.0 ≈ monthly data).
    pub annualization_factor: f64,
}

impl Default for CreditCalibrationConfig {
    fn default() -> Self {
        Self {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec { levels: vec![] },
            min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![] },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            annualization_factor: 12.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Input panels
// ---------------------------------------------------------------------------

/// Sparse issuer-spread history aligned to a sorted date grid.
///
/// `dates` is the sorted observation grid. `spreads[issuer]` has length
/// `dates.len()`; entries are `Some(spread)` when the issuer was observed at
/// that date and `None` otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryPanel {
    /// Observation dates (sorted ascending).
    pub dates: Vec<Date>,
    /// Per-issuer spread series aligned with [`dates`][Self::dates].
    pub spreads: BTreeMap<IssuerId, Vec<Option<f64>>>,
}

/// Point-in-time issuer tags at the calibration `as_of`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IssuerTagPanel {
    /// Tag map keyed by issuer.
    pub tags: BTreeMap<IssuerId, IssuerTags>,
}

/// Generic (PC) factor reference and aligned values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenericFactorSeries {
    /// Reference (name + series_id) embedded into the artifact.
    pub spec: GenericFactorSpec,
    /// Generic factor values aligned with [`HistoryPanel::dates`].
    pub values: Vec<f64>,
}

/// All inputs the calibrator needs for a single calibration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditCalibrationInputs {
    /// Sparse issuer-spread history.
    pub history_panel: HistoryPanel,
    /// Per-issuer hierarchy tags (point-in-time).
    pub issuer_tags: IssuerTagPanel,
    /// Generic factor series + spec.
    pub generic_factor: GenericFactorSeries,
    /// Calibration anchor date (must appear in `history_panel.dates`).
    pub as_of: Date,
    /// Issuer spreads at `as_of` (level space).
    pub asof_spreads: BTreeMap<IssuerId, f64>,
    /// Optional caller-supplied idiosyncratic vol overrides.
    ///
    /// Empty for PR-4 — the peer-proxy fallback chain is deferred to PR-5a.
    /// PR-4 ignores any entries here so behaviour stays deterministic.
    pub idiosyncratic_overrides: BTreeMap<IssuerId, f64>,
}

// ---------------------------------------------------------------------------
// Calibrator
// ---------------------------------------------------------------------------

/// Deterministic calibrator that produces a [`CreditFactorModel`].
///
/// Construct with [`CreditCalibrator::new`], then run [`Self::calibrate`].
#[derive(Debug, Clone)]
pub struct CreditCalibrator {
    config: CreditCalibrationConfig,
}

impl CreditCalibrator {
    /// Wrap a configuration into a calibrator.
    #[must_use]
    pub fn new(config: CreditCalibrationConfig) -> Self {
        Self { config }
    }

    /// Run the full calibration pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] when:
    /// - any unsupported [`VolModelChoice`] or [`CovarianceStrategy`] is requested,
    /// - the inputs are structurally malformed (length mismatches, missing
    ///   `as_of` in the date grid, missing tags),
    /// - the assembled [`CreditFactorModel::validate`] check fails.
    pub fn calibrate(&self, inputs: CreditCalibrationInputs) -> Result<CreditFactorModel> {
        // -- 0. Reject unsupported PR-5a/b features early. ------------------
        match self.config.vol_model {
            VolModelChoice::Sample => {}
            VolModelChoice::Garch | VolModelChoice::Egarch | VolModelChoice::Ewma { .. } => {
                return Err(validation_err(
                    "PR-4 calibrator supports VolModelChoice::Sample only; \
                     GARCH/EGARCH/EWMA are deferred to PR-5a",
                ));
            }
        }
        // All CovarianceStrategy variants are now supported (PR-5b).

        validate_calibration_config(&self.config)?;
        validate_calibration_inputs(&inputs)?;

        // -- Structural validation of inputs. -------------------------------
        let dates = &inputs.history_panel.dates;
        if dates.is_empty() {
            return Err(validation_err(
                "CreditCalibrator: history_panel.dates is empty",
            ));
        }
        if inputs.generic_factor.values.len() != dates.len() {
            return Err(validation_err(format!(
                "CreditCalibrator: generic_factor.values length {} != dates length {}",
                inputs.generic_factor.values.len(),
                dates.len()
            )));
        }
        for (issuer, series) in &inputs.history_panel.spreads {
            if series.len() != dates.len() {
                return Err(validation_err(format!(
                    "CreditCalibrator: spread series for issuer {:?} has length {}, expected {}",
                    issuer.as_str(),
                    series.len(),
                    dates.len()
                )));
            }
        }

        let asof_idx = dates
            .iter()
            .position(|d| *d == inputs.as_of)
            .ok_or_else(|| {
                validation_err(format!(
                    "CreditCalibrator: as_of {:?} not present in history_panel.dates",
                    inputs.as_of
                ))
            })?;

        // -- 1. Mode classification. ----------------------------------------
        let mut modes: BTreeMap<IssuerId, IssuerBetaMode> = BTreeMap::new();
        for issuer in inputs.history_panel.spreads.keys() {
            let mode = classify_mode(&self.config.policy, issuer, &inputs.history_panel.spreads);
            modes.insert(issuer.clone(), mode);
        }

        // -- 2. Returns or levels. ------------------------------------------
        let panel = build_working_panel(
            &self.config.use_returns_or_levels,
            dates,
            &inputs.history_panel.spreads,
            &inputs.generic_factor.values,
        );

        // -- 3. Bucket inventory + fold-up. ---------------------------------
        let inventory =
            build_bucket_inventory(&self.config.hierarchy, &inputs.issuer_tags.tags, &modes)?;
        let (folded, fold_ups) = apply_fold_up(&inventory, &self.config.min_bucket_size_per_level);
        let bucket_sizes_per_level = inventory.bucket_sizes_per_level.clone();
        let tag_taxonomy = inventory.tag_taxonomy.clone();

        // -- 4 + 5. PC peel + per-level peel. -------------------------------
        let peel_outcome = run_peel(
            &self.config,
            &panel,
            &modes,
            &inventory.bucket_paths,
            &folded,
        );

        // -- 6. Adder series → idiosyncratic vol. ---------------------------
        // First compute from-history vols for IssuerBeta issuers only.
        let from_history_vols = issuer_beta_adder_vols(
            &peel_outcome.adder_series,
            &modes,
            self.config.annualization_factor,
        );
        // Build per-level peer proxy index: level_k → bucket_path → [vols].
        let peer_proxy_index = build_peer_proxy_index(
            &from_history_vols,
            &inventory.bucket_paths,
            self.config.hierarchy.levels.len(),
        );

        // -- 7. Anchor levels at as_of. -------------------------------------
        let generic_at_asof = inputs.generic_factor.values[asof_idx];
        let anchor = anchor_levels(
            &self.config.hierarchy,
            &inputs.asof_spreads,
            &inputs.issuer_tags.tags,
            generic_at_asof,
            &peel_outcome.betas,
            &folded,
        )?;

        // -- 8. Per-factor variance forecast (Sample). ----------------------
        let factor_variances = factor_variances(
            &peel_outcome.factor_returns,
            self.config.annualization_factor,
        );

        // -- Build issuer beta rows. ----------------------------------------
        let mut issuer_betas: Vec<IssuerBetaRow> = Vec::new();
        for issuer_id in inputs.history_panel.spreads.keys() {
            // Every issuer in `spreads` was classified in step 1 above, so this
            // lookup is by-construction `Some(_)`. Fall back to BucketOnly to
            // avoid `.expect()` (clippy::expect_used is `#[deny]` in this crate).
            let mode = modes
                .get(issuer_id)
                .copied()
                .unwrap_or(IssuerBetaMode::BucketOnly);
            let tags = inputs
                .issuer_tags
                .tags
                .get(issuer_id)
                .cloned()
                .unwrap_or_default();
            let betas = peel_outcome
                .betas
                .get(issuer_id)
                .cloned()
                .unwrap_or_else(|| unit_betas(self.config.hierarchy.levels.len()));
            let adder_at_anchor = anchor.adder.get(issuer_id).copied().unwrap_or(0.0);
            let (adder_vol, adder_vol_source) = assign_adder_vol(
                issuer_id,
                mode,
                &from_history_vols,
                &peer_proxy_index,
                &inventory.bucket_paths,
                &inputs.idiosyncratic_overrides,
                self.config.hierarchy.levels.len(),
            );
            let fit_quality = peel_outcome.fit_quality.get(issuer_id).cloned();
            issuer_betas.push(IssuerBetaRow {
                issuer_id: issuer_id.clone(),
                tags,
                mode,
                betas,
                adder_at_anchor,
                adder_vol_annualized: adder_vol,
                adder_vol_source,
                fit_quality,
            });
        }
        // BTreeMap iteration is already sorted by issuer_id, but be defensive.
        issuer_betas.sort_by(|a, b| a.issuer_id.as_str().cmp(b.issuer_id.as_str()));

        // -- 9. Correlation matrix and covariance assembly. -----------------
        let factor_id_order =
            build_factor_id_order(&self.config.hierarchy, &peel_outcome.factor_returns);

        // -- 10. Assemble FactorModelConfig. --------------------------------
        let (static_correlation, config) = assemble_factor_model_config(
            &factor_id_order,
            &factor_variances,
            &peel_outcome.factor_returns,
            &self.config.hierarchy,
            &issuer_betas,
            self.config.covariance_strategy,
        )?;

        // -- 11. Diagnostics. -----------------------------------------------
        let diagnostics = build_diagnostics(
            &modes,
            bucket_sizes_per_level,
            fold_ups,
            &peel_outcome.fit_quality,
            tag_taxonomy,
        );

        // -- 12. Bundle artifact + final validate(). ------------------------
        let calibration_window = DateRange {
            start: *dates
                .first()
                .ok_or_else(|| validation_err("dates non-empty checked above"))?,
            end: *dates
                .last()
                .ok_or_else(|| validation_err("dates non-empty checked above"))?,
        };

        let factor_histories = Some(build_factor_histories(
            dates,
            &self.config.use_returns_or_levels,
            &peel_outcome.factor_returns,
        ));

        let vol_state = build_vol_state(&factor_variances, &issuer_betas);

        let model = CreditFactorModel {
            schema_version: CreditFactorModel::SCHEMA_VERSION.to_owned(),
            as_of: inputs.as_of,
            calibration_window,
            policy: self.config.policy.clone(),
            generic_factor: inputs.generic_factor.spec.clone(),
            hierarchy: self.config.hierarchy.clone(),
            config,
            issuer_betas,
            anchor_state: anchor.levels,
            static_correlation,
            vol_state,
            factor_histories,
            diagnostics,
        };

        model.validate()?;
        Ok(model)
    }
}

// ---------------------------------------------------------------------------
// Helpers — small, single-responsibility, no I/O.
// ---------------------------------------------------------------------------

fn validation_err(msg: impl Into<String>) -> Error {
    Error::Validation(msg.into())
}

fn validate_finite(label: impl std::fmt::Display, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(validation_err(format!(
            "{label} must be finite, got {value}"
        )))
    }
}

fn validate_non_negative_finite(label: impl std::fmt::Display, value: f64) -> Result<()> {
    validate_finite(&label, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(validation_err(format!(
            "{label} must be non-negative, got {value}"
        )))
    }
}

fn validate_calibration_config(config: &CreditCalibrationConfig) -> Result<()> {
    validate_finite(
        "CreditCalibrator: annualization_factor",
        config.annualization_factor,
    )?;
    if config.annualization_factor <= 0.0 {
        return Err(validation_err(format!(
            "CreditCalibrator: annualization_factor must be > 0.0, got {}",
            config.annualization_factor
        )));
    }

    if let BetaShrinkage::TowardOne { alpha } = config.beta_shrinkage {
        validate_finite("CreditCalibrator: beta_shrinkage alpha", alpha)?;
        if !(0.0..=1.0).contains(&alpha) {
            return Err(validation_err(format!(
                "CreditCalibrator: beta_shrinkage alpha must be in [0, 1], got {alpha}"
            )));
        }
    }

    if let CovarianceStrategy::Ridge { alpha } = config.covariance_strategy {
        validate_non_negative_finite("CreditCalibrator: ridge alpha", alpha)?;
    }

    Ok(())
}

fn validate_calibration_inputs(inputs: &CreditCalibrationInputs) -> Result<()> {
    for (idx, value) in inputs.generic_factor.values.iter().copied().enumerate() {
        validate_finite(
            format!("CreditCalibrator: generic_factor.values[{idx}]"),
            value,
        )?;
    }

    for (issuer, series) in &inputs.history_panel.spreads {
        for (idx, value) in series.iter().copied().enumerate() {
            if let Some(spread) = value {
                validate_finite(
                    format!(
                        "CreditCalibrator: spread series for issuer {:?} at index {idx}",
                        issuer.as_str()
                    ),
                    spread,
                )?;
            }
        }
    }

    for (issuer, spread) in &inputs.asof_spreads {
        validate_finite(
            format!(
                "CreditCalibrator: asof_spreads for issuer {:?}",
                issuer.as_str()
            ),
            *spread,
        )?;
    }

    for (issuer, vol) in &inputs.idiosyncratic_overrides {
        validate_non_negative_finite(
            format!(
                "CreditCalibrator: idiosyncratic override for issuer {:?}",
                issuer.as_str()
            ),
            *vol,
        )?;
    }

    Ok(())
}

/// `IssuerBetas` with all loadings = 1.0 (BucketOnly default).
fn unit_betas(num_levels: usize) -> IssuerBetas {
    IssuerBetas {
        pc: 1.0,
        levels: vec![1.0; num_levels],
    }
}

/// Step 1: classify an issuer as `IssuerBeta` or `BucketOnly`.
fn classify_mode(
    policy: &IssuerBetaPolicy,
    issuer: &IssuerId,
    spreads: &BTreeMap<IssuerId, Vec<Option<f64>>>,
) -> IssuerBetaMode {
    match policy {
        IssuerBetaPolicy::GloballyOff => IssuerBetaMode::BucketOnly,
        IssuerBetaPolicy::Dynamic {
            min_history,
            overrides,
        } => match overrides.get(issuer) {
            Some(IssuerBetaOverride::ForceIssuerBeta) => IssuerBetaMode::IssuerBeta,
            Some(IssuerBetaOverride::ForceBucketOnly) => IssuerBetaMode::BucketOnly,
            Some(IssuerBetaOverride::Auto) | None => {
                let count = spreads
                    .get(issuer)
                    .map(|s| s.iter().filter(|v| v.is_some()).count())
                    .unwrap_or(0);
                if count >= *min_history {
                    IssuerBetaMode::IssuerBeta
                } else {
                    IssuerBetaMode::BucketOnly
                }
            }
        },
    }
}

/// Working panel after step 2 (returns or levels).
struct WorkingPanel {
    /// Generic factor series in the chosen space, length = dates.len() - 1 (Returns)
    /// or dates.len() (Levels).
    generic: Vec<f64>,
    /// Per-issuer aligned values (`None` for missing observations / missing pair).
    issuers: BTreeMap<IssuerId, Vec<Option<f64>>>,
}

fn build_working_panel(
    space: &PanelSpace,
    dates: &[Date],
    spreads: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    generic: &[f64],
) -> WorkingPanel {
    match space {
        PanelSpace::Levels => WorkingPanel {
            generic: generic.to_vec(),
            issuers: spreads.clone(),
        },
        PanelSpace::Returns => {
            let n = dates.len();
            let mut g = Vec::with_capacity(n.saturating_sub(1));
            for t in 1..n {
                g.push(generic[t] - generic[t - 1]);
            }
            let mut issuers: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
            for (issuer, series) in spreads {
                let mut diffs = Vec::with_capacity(n.saturating_sub(1));
                for t in 1..n {
                    let d = match (series[t - 1], series[t]) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    };
                    diffs.push(d);
                }
                issuers.insert(issuer.clone(), diffs);
            }
            WorkingPanel {
                generic: g,
                issuers,
            }
        }
    }
}

/// Inventory built in step 3, before any fold-up.
struct BucketInventory {
    /// `bucket_paths[issuer][k]` = bucket path at level k (or error).
    bucket_paths: BTreeMap<IssuerId, Vec<String>>,
    /// `bucket_sizes_per_level[k][bucket]` = count of IssuerBeta issuers in that bucket.
    /// (BucketOnly issuers do not count toward the threshold.)
    bucket_sizes_per_level: Vec<BTreeMap<String, usize>>,
    /// Membership keyed by (level_index, bucket_path) → set of IssuerBeta issuer IDs.
    /// Used by fold-up to decide whether to mark members as folded.
    bucket_members_issuer_beta: Vec<BTreeMap<String, BTreeSet<IssuerId>>>,
    /// Observed values per dimension (for diagnostics).
    tag_taxonomy: BTreeMap<String, BTreeSet<String>>,
}

fn build_bucket_inventory(
    hierarchy: &CreditHierarchySpec,
    tags: &BTreeMap<IssuerId, IssuerTags>,
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
) -> Result<BucketInventory> {
    let num_levels = hierarchy.levels.len();
    let mut bucket_paths: BTreeMap<IssuerId, Vec<String>> = BTreeMap::new();
    let mut bucket_sizes_per_level: Vec<BTreeMap<String, usize>> =
        vec![BTreeMap::new(); num_levels];
    let mut bucket_members_issuer_beta: Vec<BTreeMap<String, BTreeSet<IssuerId>>> =
        vec![BTreeMap::new(); num_levels];
    let mut tag_taxonomy: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Initialize tag taxonomy with the canonical dimension keys so that
    // dimensions present in the hierarchy but unseen still appear (with an
    // empty set) — useful for diagnostics consumers.
    for dim in &hierarchy.levels {
        tag_taxonomy.entry(dimension_key(dim)).or_default();
    }

    for (issuer, mode) in modes {
        let issuer_tags = tags.get(issuer).cloned().unwrap_or_default();
        // Update tag taxonomy (every dimension seen contributes a value).
        for dim in &hierarchy.levels {
            let key = dimension_key(dim);
            if let Some(v) = issuer_tags.0.get(&key) {
                tag_taxonomy.entry(key).or_default().insert(v.clone());
            }
        }
        let mut paths = Vec::with_capacity(num_levels);
        for k in 0..num_levels {
            let path = hierarchy.bucket_path(&issuer_tags, k).ok_or_else(|| {
                let missing = hierarchy.levels[..=k]
                    .iter()
                    .find(|d| !issuer_tags.0.contains_key(&dimension_key(d)))
                    .map(dimension_key)
                    .unwrap_or_else(|| format!("level_{k}"));
                validation_err(format!(
                    "CreditCalibrator: issuer {:?} is missing tag for dimension {:?}",
                    issuer.as_str(),
                    missing
                ))
            })?;
            *bucket_sizes_per_level[k].entry(path.clone()).or_insert(0) +=
                if matches!(mode, IssuerBetaMode::IssuerBeta) {
                    1
                } else {
                    0
                };
            // Track members (only IssuerBeta count toward fold threshold).
            if matches!(mode, IssuerBetaMode::IssuerBeta) {
                bucket_members_issuer_beta[k]
                    .entry(path.clone())
                    .or_default()
                    .insert(issuer.clone());
            }
            paths.push(path);
        }
        bucket_paths.insert(issuer.clone(), paths);
    }

    Ok(BucketInventory {
        bucket_paths,
        bucket_sizes_per_level,
        bucket_members_issuer_beta,
        tag_taxonomy,
    })
}

/// Mark which (issuer, level) pairs are folded up.
///
/// Returns `(folded, fold_up_records)` where `folded[issuer][k] == true` iff
/// the issuer's bucket at level `k` was below threshold.
fn apply_fold_up(
    inventory: &BucketInventory,
    thresholds: &BucketSizeThresholds,
) -> (BTreeMap<IssuerId, Vec<bool>>, Vec<FoldUpRecord>) {
    let num_levels = inventory.bucket_sizes_per_level.len();
    let mut folded: BTreeMap<IssuerId, Vec<bool>> = BTreeMap::new();
    for issuer in inventory.bucket_paths.keys() {
        folded.insert(issuer.clone(), vec![false; num_levels]);
    }
    let mut records: Vec<FoldUpRecord> = Vec::new();

    for k in 0..num_levels {
        let threshold = thresholds.threshold_for_level(k);
        for (bucket, members) in &inventory.bucket_members_issuer_beta[k] {
            // Use the IssuerBeta-only count (matches threshold semantics).
            let count = members.len();
            if count < threshold {
                let folded_to = if k == 0 {
                    "<root>".to_owned()
                } else {
                    // Strip the trailing "." segment — the parent path.
                    bucket
                        .rsplit_once('.')
                        .map(|x| x.0)
                        .unwrap_or("<root>")
                        .to_owned()
                };
                let reason = format!("fewer than {threshold} issuer_beta members ({count})");
                for member in members {
                    if let Some(flags) = folded.get_mut(member) {
                        flags[k] = true;
                    }
                    records.push(FoldUpRecord {
                        issuer_id: member.clone(),
                        level_index: k,
                        original_bucket: bucket.clone(),
                        folded_to: folded_to.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }
    }

    // Sort records for determinism: by (level_index, issuer_id).
    records.sort_by(|a, b| {
        a.level_index
            .cmp(&b.level_index)
            .then_with(|| a.issuer_id.as_str().cmp(b.issuer_id.as_str()))
    });

    (folded, records)
}

/// Outcome of the PC + per-level peel (steps 4 + 5).
struct PeelOutcome {
    /// Calibrated betas per issuer.
    betas: BTreeMap<IssuerId, IssuerBetas>,
    /// Adder return series per issuer (length = working panel length).
    adder_series: BTreeMap<IssuerId, Vec<Option<f64>>>,
    /// Fit quality stats for IssuerBeta issuers.
    fit_quality: BTreeMap<IssuerId, FitQuality>,
    /// Per-factor return series (sparse), keyed by FactorId.
    /// Includes the generic factor (always `Some`) and every surviving bucket
    /// factor. Bucket factors carry `None` at dates where all bucket members
    /// were absent ("empty-bucket" sentinel). Use [`factor_variances`] to
    /// compute variance over only the observed entries, and flatten to
    /// `0.0`-substituted `Vec<f64>` when writing to [`FactorHistories`].
    factor_returns: BTreeMap<FactorId, Vec<Option<f64>>>,
}

fn run_peel(
    config: &CreditCalibrationConfig,
    panel: &WorkingPanel,
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    folded: &BTreeMap<IssuerId, Vec<bool>>,
) -> PeelOutcome {
    let n = panel.generic.len();
    let num_levels = config.hierarchy.levels.len();

    let mut betas: BTreeMap<IssuerId, IssuerBetas> = BTreeMap::new();
    let mut residuals: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut fit_quality: BTreeMap<IssuerId, FitQuality> = BTreeMap::new();
    let mut factor_returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();

    // Generic factor is always fully observed; wrap in Some for type uniformity.
    factor_returns.insert(
        FactorId::new(CREDIT_GENERIC_FACTOR_ID),
        panel.generic.iter().map(|v| Some(*v)).collect(),
    );

    // Step 4 — PC peel.
    for (issuer, series) in &panel.issuers {
        let mode = modes
            .get(issuer)
            .copied()
            .unwrap_or(IssuerBetaMode::BucketOnly);
        let beta_pc = match mode {
            IssuerBetaMode::BucketOnly => 1.0,
            IssuerBetaMode::IssuerBeta => {
                let raw = ols_slope(series, &panel.generic).unwrap_or(1.0);
                let shrunk = apply_shrinkage(&config.beta_shrinkage, raw);
                // Fit-quality stats: R² and residual std on the same valid pairs.
                if let Some(fq) = compute_fit_quality(series, &panel.generic, shrunk) {
                    fit_quality.insert(issuer.clone(), fq);
                }
                shrunk
            }
        };
        let res_pc: Vec<Option<f64>> = series
            .iter()
            .enumerate()
            .map(|(t, v)| v.as_ref().map(|s| s - beta_pc * panel.generic[t]))
            .collect();
        residuals.insert(issuer.clone(), res_pc);
        // Initialize beta row with per-level betas at 0.0; the per-level peel
        // below overwrites entries for non-folded buckets. Folded buckets stay
        // at 0.0 (the contractual sentinel for "skip this level").
        betas.insert(
            issuer.clone(),
            IssuerBetas {
                pc: beta_pc,
                levels: vec![0.0; num_levels],
            },
        );
    }

    // Step 5 — per-level peel.
    // Range-based loop over the hierarchy level index `k`. `k` indexes into
    // multiple parallel structures (`bucket_paths[issuer][k]`, `folded[i][k]`,
    // `betas[issuer].levels[k]`); enumerate-iterating any one of them would
    // not eliminate indexing into the others.
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        // 5a. For each surviving (non-folded) bucket, compute factor return series.
        // Build a map: bucket_path → vector of issuer IDs participating.
        // Folded issuers contribute β=0 at this level and DO NOT participate
        // in computing the bucket factor return; they simply propagate
        // residuals unchanged.
        let mut bucket_members: BTreeMap<String, Vec<&IssuerId>> = BTreeMap::new();
        for issuer in panel.issuers.keys() {
            let folded_at_k = folded
                .get(issuer)
                .map(|f| f.get(k).copied().unwrap_or(false))
                .unwrap_or(false);
            if folded_at_k {
                continue;
            }
            let path = &bucket_paths[issuer][k];
            bucket_members.entry(path.clone()).or_default().push(issuer);
        }

        // Compute bucket factor returns f_<level_k>(g, t) = mean over members.
        // Empty buckets at date t (all members missing) emit `None` so that
        // downstream OLS and variance estimation can skip those dates rather
        // than biasing results with an imputed zero.
        let mut bucket_factor_series: BTreeMap<String, Vec<Option<f64>>> = BTreeMap::new();
        for (bucket, members) in &bucket_members {
            let mut series = Vec::with_capacity(n);
            for t in 0..n {
                let mut sum = 0.0;
                let mut count = 0usize;
                for issuer in members {
                    if let Some(Some(v)) = residuals.get(*issuer).map(|r| r[t]) {
                        sum += v;
                        count += 1;
                    }
                }
                series.push(if count > 0 {
                    Some(sum / (count as f64))
                } else {
                    None
                });
            }
            bucket_factor_series.insert(bucket.clone(), series);
        }

        // 5b. For each member, fit / set its level-k beta and update its residual.
        for (bucket, members) in &bucket_members {
            let factor_series = &bucket_factor_series[bucket];
            for issuer in members {
                let mode = modes
                    .get(*issuer)
                    .copied()
                    .unwrap_or(IssuerBetaMode::BucketOnly);
                let r_series = residuals.get(*issuer).cloned().unwrap_or_default();
                let beta_k = match mode {
                    IssuerBetaMode::BucketOnly => 1.0,
                    IssuerBetaMode::IssuerBeta => {
                        // Fit OLS on the issuer's *current* residual vs the bucket factor.
                        // `factor_series` is already `Vec<Option<f64>>`; dates where the
                        // factor is `None` (empty bucket) are skipped by `ols_slope_owned`.
                        let raw = ols_slope_owned(&r_series, factor_series).unwrap_or(1.0);
                        apply_shrinkage(&config.beta_shrinkage, raw)
                    }
                };
                if let Some(b) = betas.get_mut(*issuer) {
                    b.levels[k] = beta_k;
                }
                // Propagate `None` when the factor is unavailable at date t.
                let new_res: Vec<Option<f64>> = r_series
                    .iter()
                    .enumerate()
                    .map(|(t, v)| match (v, factor_series[t]) {
                        (Some(x), Some(f)) => Some(x - beta_k * f),
                        _ => None,
                    })
                    .collect();
                residuals.insert((*issuer).clone(), new_res);
            }
        }

        // Folded issuers: beta_k stays at 0.0 (already initialized) and
        // residual is unchanged (no subtraction). We've simply skipped them.

        // Record bucket factor return series in the canonical FactorId form.
        // The sparse `Vec<Option<f64>>` is stored directly in `factor_returns`;
        // flattening to `Vec<f64>` for `FactorHistories` happens in
        // `build_factor_histories` (substituting `0.0` for `None`).
        // `factor_variances` computes variance over only the `Some` entries.
        for (bucket, series) in bucket_factor_series {
            // Reconstruct an IssuerTags for path → use a synthetic helper:
            // We need bucket_factor_id. The existing helper requires IssuerTags;
            // we don't have them here, but we can build a minimal tag map by
            // splitting the path on '.'.
            let tags = synth_tags_from_path(&config.hierarchy, &bucket);
            // bucket_factor_id is `Some(_)` whenever every dimension key in
            // `levels[0..=k]` appears in `tags` — which `synth_tags_from_path`
            // guarantees by construction. Fall through with `continue` defensively
            // rather than panic via `.expect()`.
            let Some(factor_id) = bucket_factor_id(&config.hierarchy, &tags, k) else {
                continue;
            };
            factor_returns.insert(factor_id, series);
        }
    }

    PeelOutcome {
        betas,
        adder_series: residuals,
        fit_quality,
        factor_returns,
    }
}

/// Reconstruct an [`IssuerTags`] from a dotted bucket path so that callers
/// can re-use [`bucket_factor_id`].
///
/// The path has `k+1` segments aligned with `hierarchy.levels[0..=k]`.
fn synth_tags_from_path(hierarchy: &CreditHierarchySpec, path: &str) -> IssuerTags {
    let segments: Vec<&str> = path.split('.').collect();
    let mut map = BTreeMap::new();
    for (i, seg) in segments.iter().enumerate() {
        if let Some(dim) = hierarchy.levels.get(i) {
            map.insert(dimension_key(dim), (*seg).to_owned());
        }
    }
    IssuerTags(map)
}

/// Apply shrinkage rule to an OLS β estimate.
fn apply_shrinkage(rule: &BetaShrinkage, beta_fit: f64) -> f64 {
    match rule {
        BetaShrinkage::None => beta_fit,
        BetaShrinkage::TowardOne { alpha } => (1.0 - *alpha) * beta_fit + *alpha * 1.0,
    }
}

/// OLS slope on aligned valid pairs of `(y_i, x_i)` where `y` is a sparse
/// `Option<f64>` series and `x` is dense.
///
/// Collects valid pairs, then delegates to
/// `finstack_analytics::beta`.  Returns `None` if fewer than 3
/// valid pairs are available (mirrors the `n < 3` `NaN` return of `beta`).
fn ols_slope(y: &[Option<f64>], x: &[f64]) -> Option<f64> {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (yi, xi) in y.iter().zip(x.iter()) {
        if let Some(v) = yi {
            xs.push(*xi);
            ys.push(*v);
        }
    }
    let result = finstack_analytics::beta(&ys, &xs);
    if result.beta.is_nan() {
        None
    } else {
        Some(result.beta)
    }
}

/// OLS slope when both series are sparse; align on positions where both are `Some`.
///
/// Delegates to `finstack_analytics::beta` after collecting the
/// valid pairs.  Returns `None` when fewer than 3 joint observations exist.
fn ols_slope_owned(y: &[Option<f64>], x: &[Option<f64>]) -> Option<f64> {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (yi, xi) in y.iter().zip(x.iter()) {
        if let (Some(yv), Some(xv)) = (yi, xi) {
            ys.push(*yv);
            xs.push(*xv);
        }
    }
    let result = finstack_analytics::beta(&ys, &xs);
    if result.beta.is_nan() {
        None
    } else {
        Some(result.beta)
    }
}

/// R², residual std, and n_obs for the PC fit (used as the regression diagnostic).
///
/// `residual_std` is a population std dev (sum of squared residuals divided by `n`).
/// Population variance is acceptable because calibration windows are required to be
/// ≥ 24 observations (`min_history` default = 24), keeping the bias negligible.
fn compute_fit_quality(y: &[Option<f64>], x: &[f64], beta: f64) -> Option<FitQuality> {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (yi, xi) in y.iter().zip(x.iter()) {
        if let Some(v) = yi {
            xs.push(*xi);
            ys.push(*v);
        }
    }
    let n = xs.len();
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let mean_x = xs.iter().sum::<f64>() / nf;
    let mean_y = ys.iter().sum::<f64>() / nf;
    let alpha = mean_y - beta * mean_x;
    let mut tss = 0.0;
    let mut rss = 0.0;
    for i in 0..n {
        let resid = ys[i] - alpha - beta * xs[i];
        rss += resid * resid;
        let dy = ys[i] - mean_y;
        tss += dy * dy;
    }
    let r_squared = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let residual_std = (rss / nf).sqrt();
    Some(FitQuality {
        r_squared,
        residual_std,
        n_obs: n,
    })
}

/// Step 6 (part A): per-issuer adder annualized **vol** (std dev) for
/// `IssuerBeta` issuers only, computed from the residual series after the last level.
///
/// Returns the annualized standard deviation `sqrt(var * annualization_factor)`
/// for each `IssuerBeta` issuer that has at least 2 valid residual observations.
/// `BucketOnly` issuers are excluded — they receive their vol via the cascade in
/// [`assign_adder_vol`].
///
/// Variance is computed as the population variance (sums squared deviations from
/// the sample mean and divides by `n`). Population variance is acceptable here
/// because calibration windows are required to be ≥ 24 observations
/// (`min_history` default = 24), keeping the bias negligible.
fn issuer_beta_adder_vols(
    adder_series: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
    annualization_factor: f64,
) -> BTreeMap<IssuerId, f64> {
    let mut out = BTreeMap::new();
    for (issuer, series) in adder_series {
        if !matches!(modes.get(issuer), Some(IssuerBetaMode::IssuerBeta)) {
            continue;
        }
        let valid: Vec<f64> = series.iter().filter_map(|v| *v).collect();
        let n = valid.len();
        if n < 2 {
            continue;
        }
        let nf = n as f64;
        let mean = valid.iter().sum::<f64>() / nf;
        let var = valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nf;
        let ann_var = var * annualization_factor;
        out.insert(issuer.clone(), ann_var.max(0.0).sqrt());
    }
    out
}

/// Step 6 (part B): build the peer proxy index for the bucket-peer cascade.
///
/// Returns a `Vec` (indexed by hierarchy level `k`) of `BTreeMap` from
/// `bucket_path_at_level_k` to the list of `FromHistory` adder vols of all
/// `IssuerBeta` peers in that bucket.
///
/// Only issuers present in `from_history_vols` (i.e., successful `FromHistory`
/// fits) contribute to the index. `BucketOnly` issuers and `IssuerBeta` issuers
/// with insufficient history are implicitly excluded because they have no entry
/// in `from_history_vols`.
///
/// The returned structure is deterministic: `BTreeMap` key order and `Vec`
/// element order both follow `BTreeMap` iteration (lexicographic).
fn build_peer_proxy_index(
    from_history_vols: &BTreeMap<IssuerId, f64>,
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    num_levels: usize,
) -> Vec<BTreeMap<String, Vec<f64>>> {
    // One BTreeMap<bucket_path, vols> per hierarchy level.
    let mut index: Vec<BTreeMap<String, Vec<f64>>> = vec![BTreeMap::new(); num_levels];

    // Iterate in BTreeMap order (sorted by issuer_id) for determinism.
    for (issuer, vol) in from_history_vols {
        if let Some(paths) = bucket_paths.get(issuer) {
            for (k, path) in paths.iter().enumerate() {
                if k < num_levels {
                    index[k].entry(path.clone()).or_default().push(*vol);
                }
            }
        }
    }

    // Sort each bucket's vol list for fully deterministic mean computation.
    for level_map in &mut index {
        for vols in level_map.values_mut() {
            vols.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    index
}

/// Step 6 (part C): assign an `(adder_vol, AdderVolSource)` pair to a single
/// issuer using the full caller → history → peer-proxy → global → zero cascade.
///
/// # Cascade order
///
/// 1. **Caller override** (present in `idiosyncratic_overrides`): use it with
///    `AdderVolSource::CallerSupplied`. Wins for both `IssuerBeta` and `BucketOnly`.
/// 2. **`IssuerBeta` with successful `FromHistory` fit**: use the history-derived
///    vol with `AdderVolSource::FromHistory`.
/// 3. **Bucket-peer proxy** (deepest level first, then walking up): find the
///    deepest level `k` such that the issuer has a path at level `k` and there
///    is at least one `IssuerBeta` peer with a `FromHistory` vol in that bucket.
///    Use the mean of those peers' vols with
///    `AdderVolSource::BucketPeerProxy { peer_bucket }`.
/// 4. **Global default**: if no peers exist anywhere up the hierarchy, use the
///    mean of *all* `IssuerBeta` `FromHistory` vols across the entire model.
///    `AdderVolSource::Default`.
/// 5. **No data at all**: `(0.0, AdderVolSource::Default)` — applies when
///    `from_history_vols` is completely empty (all-BucketOnly model).
#[allow(clippy::too_many_arguments)]
fn assign_adder_vol(
    issuer_id: &IssuerId,
    mode: IssuerBetaMode,
    from_history_vols: &BTreeMap<IssuerId, f64>,
    peer_proxy_index: &[BTreeMap<String, Vec<f64>>],
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    idiosyncratic_overrides: &BTreeMap<IssuerId, f64>,
    num_levels: usize,
) -> (f64, AdderVolSource) {
    // 1. Caller override wins for any mode.
    if let Some(&override_vol) = idiosyncratic_overrides.get(issuer_id) {
        return (override_vol, AdderVolSource::CallerSupplied);
    }

    // 2. IssuerBeta with successful FromHistory fit.
    if matches!(mode, IssuerBetaMode::IssuerBeta) {
        if let Some(&vol) = from_history_vols.get(issuer_id) {
            return (vol, AdderVolSource::FromHistory);
        }
    }

    // 3. Bucket-peer proxy cascade: walk from deepest level to broadest.
    if let Some(paths) = bucket_paths.get(issuer_id) {
        for k in (0..num_levels).rev() {
            if k < paths.len() && k < peer_proxy_index.len() {
                let bucket = &paths[k];
                if let Some(peer_vols) = peer_proxy_index[k].get(bucket) {
                    if !peer_vols.is_empty() {
                        return (
                            mean_of(peer_vols).unwrap_or(0.0),
                            AdderVolSource::BucketPeerProxy {
                                peer_bucket: bucket.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    // 4. Global mean of all IssuerBeta FromHistory vols.
    if !from_history_vols.is_empty() {
        let global_vols: Vec<f64> = from_history_vols.values().copied().collect();
        return (
            mean_of(&global_vols).unwrap_or(0.0),
            AdderVolSource::Default,
        );
    }

    // 5. No IssuerBeta data anywhere: hardcoded 0.0.
    (0.0, AdderVolSource::Default)
}

/// Compute the arithmetic mean of a slice. Returns `None` for an empty slice.
fn mean_of(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / (values.len() as f64))
}

/// Anchor-step output: anchor levels + per-issuer adder values at as_of.
struct AnchorOutcome {
    levels: LevelsAtAnchor,
    adder: BTreeMap<IssuerId, f64>,
}

/// Step 7: anchor levels at as_of (level space, not return space).
///
/// Implements the same peel-the-onion math as
/// [`crate::credit_decomposition::decompose_levels`] but uses
/// the calibrated betas from step 4-5. We don't have a complete
/// `CreditFactorModel` yet, so this is a self-contained re-implementation.
fn anchor_levels(
    hierarchy: &CreditHierarchySpec,
    asof_spreads: &BTreeMap<IssuerId, f64>,
    tags: &BTreeMap<IssuerId, IssuerTags>,
    generic_at_asof: f64,
    betas: &BTreeMap<IssuerId, IssuerBetas>,
    folded: &BTreeMap<IssuerId, Vec<bool>>,
) -> Result<AnchorOutcome> {
    let num_levels = hierarchy.levels.len();
    // Resolve issuer → tags + bucket_paths.
    let mut bucket_paths: BTreeMap<IssuerId, Vec<String>> = BTreeMap::new();
    for issuer in asof_spreads.keys() {
        let issuer_tags = tags.get(issuer).cloned().unwrap_or_default();
        let mut paths = Vec::with_capacity(num_levels);
        for k in 0..num_levels {
            let p = hierarchy.bucket_path(&issuer_tags, k).ok_or_else(|| {
                let missing = hierarchy.levels[..=k]
                    .iter()
                    .find(|d| !issuer_tags.0.contains_key(&dimension_key(d)))
                    .map(dimension_key)
                    .unwrap_or_else(|| format!("level_{k}"));
                validation_err(format!(
                    "CreditCalibrator anchor: issuer {:?} missing tag {:?}",
                    issuer.as_str(),
                    missing
                ))
            })?;
            paths.push(p);
        }
        bucket_paths.insert(issuer.clone(), paths);
    }

    // PC peel.
    let unit = unit_betas(num_levels);
    let mut residuals: BTreeMap<IssuerId, f64> = BTreeMap::new();
    for (issuer, spread) in asof_spreads {
        let b = betas.get(issuer).unwrap_or(&unit);
        residuals.insert(issuer.clone(), spread - b.pc * generic_at_asof);
    }

    // Per-level peel.
    // Range loop over the hierarchy level index — see the parallel comment
    // in `run_peel`.
    let mut by_level: Vec<LevelAnchor> = Vec::with_capacity(num_levels);
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        // Bucket means over non-folded issuers.
        let mut sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for issuer in asof_spreads.keys() {
            let folded_at_k = folded
                .get(issuer)
                .map(|f| f.get(k).copied().unwrap_or(false))
                .unwrap_or(false);
            if folded_at_k {
                continue;
            }
            let path = &bucket_paths[issuer][k];
            let r = residuals[issuer];
            let entry = sums.entry(path.clone()).or_insert((0.0, 0));
            entry.0 += r;
            entry.1 += 1;
        }
        let mut values: BTreeMap<String, f64> = BTreeMap::new();
        for (bucket, (sum, count)) in sums {
            let mean = if count > 0 { sum / (count as f64) } else { 0.0 };
            values.insert(bucket, mean);
        }

        // Subtract β_i^level_k · L_k(g_i^k) from each issuer's residual.
        for issuer in asof_spreads.keys() {
            let folded_at_k = folded
                .get(issuer)
                .map(|f| f.get(k).copied().unwrap_or(false))
                .unwrap_or(false);
            if folded_at_k {
                continue;
            }
            let b = betas.get(issuer).unwrap_or(&unit);
            let path = &bucket_paths[issuer][k];
            let level_value = values.get(path).copied().unwrap_or(0.0);
            let prev = residuals[issuer];
            residuals.insert(issuer.clone(), prev - b.levels[k] * level_value);
        }

        by_level.push(LevelAnchor {
            level_index: k,
            dimension: hierarchy.levels[k].clone(),
            values,
        });
    }

    Ok(AnchorOutcome {
        levels: LevelsAtAnchor {
            pc: generic_at_asof,
            by_level,
        },
        adder: residuals,
    })
}

/// Step 8: per-factor annualized variance.
///
/// Returns annualized factor variances (already squared) suitable for placing
/// on the diagonal of `Σ`. Values are **not** std devs; callers must not
/// take a square root before inserting into the covariance matrix.
///
/// Each series may contain `None` entries for dates where all bucket members
/// were absent (empty-bucket). Such entries are skipped so that variance is
/// computed only over observed dates. The estimator is the **unbiased sample
/// variance** (sum of squared deviations from the sample mean divided by
/// `n − 1`, Bessel's correction). The PC factor is typically observed on every
/// date, but sparse bucket factors can have a much shorter effective history
/// than the PC factor, so the `n` vs `n − 1` distinction is material there:
/// dividing by `n` would systematically understate bucket-factor variance.
fn factor_variances(
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
    annualization_factor: f64,
) -> BTreeMap<FactorId, f64> {
    let mut out = BTreeMap::new();
    for (fid, series) in factor_returns {
        let valid: Vec<f64> = series.iter().filter_map(|v| *v).collect();
        let n = valid.len();
        if n < 2 {
            out.insert(fid.clone(), 0.0);
            continue;
        }
        let nf = n as f64;
        let mean = valid.iter().sum::<f64>() / nf;
        // Unbiased (Bessel-corrected) sample variance: divide by n − 1.
        let var = valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (nf - 1.0);
        out.insert(fid.clone(), (var * annualization_factor).max(0.0));
    }
    out
}

/// Canonical factor ID order: PC first, then bucket factors sorted lexicographically.
fn build_factor_id_order(
    _hierarchy: &CreditHierarchySpec,
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> Vec<FactorId> {
    let pc = FactorId::new(CREDIT_GENERIC_FACTOR_ID);
    let mut buckets: Vec<FactorId> = factor_returns
        .keys()
        .filter(|f| f.as_str() != CREDIT_GENERIC_FACTOR_ID)
        .cloned()
        .collect();
    buckets.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut order = Vec::with_capacity(1 + buckets.len());
    order.push(pc);
    order.extend(buckets);
    order
}

/// Assemble `(FactorCorrelationMatrix, FactorModelConfig)` for a given strategy.
///
/// Returns the static correlation matrix and the covariance-embedded config.
fn assemble_factor_model_config(
    factor_id_order: &[FactorId],
    factor_variances: &BTreeMap<FactorId, f64>,
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
    hierarchy: &CreditHierarchySpec,
    issuer_betas: &[IssuerBetaRow],
    strategy: CovarianceStrategy,
) -> Result<(FactorCorrelationMatrix, FactorModelConfig)> {
    // Build factor definitions (every factor is Credit / CurveParallel placeholder).
    let mut factors = Vec::with_capacity(factor_id_order.len());
    for fid in factor_id_order {
        // PR-4: empty curve_ids is an honest no-op. PR-8a (waterfall + parallel
        // attribution) will replace this with real curve mappings via the credit
        // hierarchical matcher.
        factors.push(FactorDefinition {
            id: fid.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        });
    }

    let n = factor_id_order.len();

    // Per-factor standard deviations (sqrt of annualized variances).
    let stds: Vec<f64> = factor_id_order
        .iter()
        .map(|fid| {
            let var = factor_variances.get(fid).copied().unwrap_or(0.0).max(0.0);
            var.sqrt()
        })
        .collect();

    let (static_correlation, cov_data) = match strategy {
        CovarianceStrategy::Diagonal => {
            // Identity correlation; Σ = diag(σ²).
            let corr = FactorCorrelationMatrix::identity(factor_id_order.to_vec());
            let mut data = vec![0.0_f64; n * n];
            for (i, fid) in factor_id_order.iter().enumerate() {
                let var = factor_variances.get(fid).copied().unwrap_or(0.0);
                data[i * n + i] = var.max(0.0);
            }
            (corr, data)
        }
        CovarianceStrategy::Ridge { alpha } => {
            if alpha < 0.0 {
                return Err(validation_err(format!(
                    "Ridge: alpha must be >= 0.0; got {alpha}"
                )));
            }
            // Sample correlation ρ (PSD-repaired if needed); Σ = D·ρ·D + α·I.
            let rho_flat = sample_correlation_flat(factor_id_order, factor_returns);
            // Repair ρ to PSD if needed (e.g. when n_factors > n_obs).
            let rho_flat = if validate_correlation_matrix(&rho_flat, n).is_ok() {
                rho_flat
            } else {
                nearest_correlation_matrix(&rho_flat, n, NearestCorrelationOpts::default())
                    .map_err(|e| {
                        validation_err(format!("Ridge: nearest_correlation_matrix failed: {e}"))
                    })?
            };
            let corr_data = flat_to_row_major(&rho_flat, n);
            let corr =
                FactorCorrelationMatrix::new(factor_id_order.to_vec(), corr_data).map_err(|e| {
                    validation_err(format!(
                        "Ridge: repaired correlation is not a valid correlation matrix: {e}"
                    ))
                })?;
            // Σ = D·ρ_repaired·D + α·I (row-major flat).
            let mut data = d_rho_d(&stds, &rho_flat, n);
            for i in 0..n {
                data[i * n + i] += alpha;
            }
            (corr, data)
        }
        CovarianceStrategy::FullSampleRepaired => {
            // Sample correlation ρ; repair if not PSD; Σ = D·ρ_repaired·D.
            let rho_flat = sample_correlation_flat(factor_id_order, factor_returns);
            // Check if already PSD; repair if not.
            let rho_repaired = if validate_correlation_matrix(&rho_flat, n).is_ok() {
                rho_flat
            } else {
                nearest_correlation_matrix(&rho_flat, n, NearestCorrelationOpts::default())
                    .map_err(|e| {
                        validation_err(format!(
                            "FullSampleRepaired: nearest_correlation_matrix failed: {e}"
                        ))
                    })?
            };
            let corr_data = flat_to_row_major(&rho_repaired, n);
            let corr =
                FactorCorrelationMatrix::new(factor_id_order.to_vec(), corr_data).map_err(|e| {
                    validation_err(format!(
                        "FullSampleRepaired: repaired correlation is not valid: {e}"
                    ))
                })?;
            let data = d_rho_d(&stds, &rho_repaired, n);
            (corr, data)
        }
    };

    let covariance = FactorCovarianceMatrix::new(factor_id_order.to_vec(), cov_data)
        .map_err(|e| validation_err(format!("FactorCovarianceMatrix::new failed: {e}")))?;

    let matching = MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
        dependency_filter: Default::default(),
        hierarchy: hierarchy.clone(),
        issuer_betas: issuer_betas.to_vec(),
    });

    let config = FactorModelConfig {
        factors,
        covariance,
        matching,
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: Default::default(),
        bump_size: None,
        unmatched_policy: None,
    };

    Ok((static_correlation, config))
}

/// Compute the flat (row-major) sample correlation matrix over the factor return
/// series. For each pair `(i, j)` only dates where BOTH factors have `Some(_)`
/// are used (pairwise complete observation).
///
/// If a factor has fewer than 2 valid observations, all off-diagonal entries in
/// its row/column are set to 0.0 (i.e. the factor is treated as uncorrelated).
///
/// # Marginal-mean limitation
///
/// The mean subtracted before computing the covariance for each factor is its
/// **marginal** mean — the mean over all dates where that factor individually
/// has a valid observation, not the mean over the pairwise overlap. This is a
/// deliberate simplification that avoids data-dependent conditioning but means
/// the off-diagonal terms are not strictly centred on the joint distribution.
///
/// # PSD guarantee
///
/// Off-diagonal entries are clamped to `[-1, 1]` but the resulting matrix is
/// **not guaranteed to be positive semi-definite** (e.g. when the number of
/// factors exceeds the number of observations). Callers that require a PSD
/// matrix must use [`CovarianceStrategy::Ridge`] or
/// [`CovarianceStrategy::FullSampleRepaired`], both of which apply
/// nearest-correlation repair when the raw sample matrix is not PSD.
fn sample_correlation_flat(
    factor_id_order: &[FactorId],
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> Vec<f64> {
    let n = factor_id_order.len();
    if n == 0 {
        return vec![];
    }

    // Materialise each factor's series (missing factors get an empty series).
    let empty: Vec<Option<f64>> = vec![];
    let series: Vec<&Vec<Option<f64>>> = factor_id_order
        .iter()
        .map(|fid| factor_returns.get(fid).unwrap_or(&empty))
        .collect();

    // Flat row-major result — diagonal = 1.0, off-diagonal filled below.
    let mut rho = vec![0.0_f64; n * n];
    for i in 0..n {
        rho[i * n + i] = 1.0;
    }

    for i in 0..n {
        for j in (i + 1)..n {
            // Pairwise: use only dates where both factors are observed.
            //
            // The covariance, both variances, and the means MUST all be formed
            // over the same pairwise-overlap window. A factor's marginal mean
            // (over its full valid history) generally differs from its mean on
            // the subset of dates where the *other* factor is also observed; on
            // a sparse panel the two windows can be wildly different. Demeaning
            // with the marginal mean while summing over the overlap yields a
            // ratio that is not a Pearson correlation (it can even exceed 1
            // before the clamp). Compute the overlap mean for each factor here
            // so the assembled entry is a proper sample correlation.
            let mut overlap_i: Vec<f64> = Vec::new();
            let mut overlap_j: Vec<f64> = Vec::new();
            for (vi_opt, vj_opt) in series[i].iter().zip(series[j].iter()) {
                if let (Some(vi), Some(vj)) = (*vi_opt, *vj_opt) {
                    overlap_i.push(vi);
                    overlap_j.push(vj);
                }
            }
            let count = overlap_i.len();
            let corr = if count >= 2 {
                let nf = count as f64;
                let mean_i = overlap_i.iter().sum::<f64>() / nf;
                let mean_j = overlap_j.iter().sum::<f64>() / nf;
                let mut cov_ij = 0.0_f64;
                let mut var_i = 0.0_f64;
                let mut var_j = 0.0_f64;
                for (vi, vj) in overlap_i.iter().zip(overlap_j.iter()) {
                    let di = vi - mean_i;
                    let dj = vj - mean_j;
                    cov_ij += di * dj;
                    var_i += di * di;
                    var_j += dj * dj;
                }
                // `cov_ij`, `var_i`, `var_j` share the same denominator
                // (n − 1), so it cancels in the ratio and need not be applied.
                if var_i > 0.0 && var_j > 0.0 {
                    (cov_ij / (var_i * var_j).sqrt()).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            rho[i * n + j] = corr;
            rho[j * n + i] = corr;
        }
    }

    rho
}

/// Convert flat row-major `n×n` matrix into `Vec<Vec<f64>>` (row-per-Vec).
fn flat_to_row_major(flat: &[f64], n: usize) -> Vec<Vec<f64>> {
    (0..n).map(|i| flat[i * n..(i + 1) * n].to_vec()).collect()
}

/// Compute `Σ = D · ρ · D` from standard deviations and flat correlation matrix.
///
/// Returns flat row-major `n×n` covariance matrix.
fn d_rho_d(stds: &[f64], rho_flat: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = stds[i] * rho_flat[i * n + j] * stds[j];
        }
    }
    out
}

fn build_factor_histories(
    dates: &[Date],
    space: &PanelSpace,
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> FactorHistories {
    // Returns: histories align to dates[1..]. Levels: histories align to dates.
    let aligned_dates = match space {
        PanelSpace::Returns => dates.iter().skip(1).copied().collect::<Vec<_>>(),
        PanelSpace::Levels => dates.to_vec(),
    };
    // `FactorHistories.values` is `Vec<f64>` (locked at PR-1). Flatten sparse
    // series here: `None` (empty-bucket date) → `0.0`. Downstream consumers
    // should treat 0.0 entries in level-factor histories as "no observation"
    // when the panel is known to be sparse.
    let values: BTreeMap<FactorId, Vec<f64>> = factor_returns
        .iter()
        .map(|(fid, series)| {
            let dense = series.iter().map(|v| v.unwrap_or(0.0)).collect();
            (fid.clone(), dense)
        })
        .collect();
    FactorHistories {
        dates: aligned_dates,
        values,
    }
}

fn build_vol_state(
    factor_variances: &BTreeMap<FactorId, f64>,
    issuer_betas: &[IssuerBetaRow],
) -> VolState {
    let mut factors = BTreeMap::new();
    for (fid, var) in factor_variances {
        factors.insert(fid.clone(), FactorVolModel::Sample { variance: *var });
    }
    let mut idiosyncratic = BTreeMap::new();
    for row in issuer_betas {
        let var = row.adder_vol_annualized.powi(2);
        idiosyncratic.insert(
            row.issuer_id.clone(),
            IdiosyncraticVolModel::Sample { variance: var },
        );
    }
    VolState {
        factors,
        idiosyncratic,
    }
}

fn build_diagnostics(
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
    bucket_sizes_per_level: Vec<BTreeMap<String, usize>>,
    fold_ups: Vec<FoldUpRecord>,
    fit_quality: &BTreeMap<IssuerId, FitQuality>,
    tag_taxonomy: BTreeMap<String, BTreeSet<String>>,
) -> CalibrationDiagnostics {
    let mut mode_counts: BTreeMap<String, usize> = BTreeMap::new();
    mode_counts.insert("issuer_beta".to_owned(), 0);
    mode_counts.insert("bucket_only".to_owned(), 0);
    for mode in modes.values() {
        let key = match mode {
            IssuerBetaMode::IssuerBeta => "issuer_beta",
            IssuerBetaMode::BucketOnly => "bucket_only",
        };
        *mode_counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    // R² histogram: 5 bins [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]. Values < 0 fall into
    // the lowest bin; values > 1 (rare in OLS but possible if mean-shifted) fall
    // into the highest. Bin keys are stable strings for deterministic JSON.
    let r_squared_histogram = if fit_quality.is_empty() {
        None
    } else {
        let mut hist: BTreeMap<String, usize> = BTreeMap::new();
        for label in [
            "[0.0,0.2)",
            "[0.2,0.4)",
            "[0.4,0.6)",
            "[0.6,0.8)",
            "[0.8,1.0]",
        ] {
            hist.insert(label.to_owned(), 0);
        }
        for fq in fit_quality.values() {
            let r2 = fq.r_squared.clamp(0.0, 1.0);
            let key = if r2 < 0.2 {
                "[0.0,0.2)"
            } else if r2 < 0.4 {
                "[0.2,0.4)"
            } else if r2 < 0.6 {
                "[0.4,0.6)"
            } else if r2 < 0.8 {
                "[0.6,0.8)"
            } else {
                "[0.8,1.0]"
            };
            *hist.entry(key.to_owned()).or_insert(0) += 1;
        }
        Some(hist)
    };

    CalibrationDiagnostics {
        mode_counts,
        bucket_sizes_per_level,
        fold_ups,
        r_squared_histogram,
        tag_taxonomy,
    }
}

#[cfg(test)]
mod calibration_estimator_tests {
    use super::{factor_variances, sample_correlation_flat};
    use finstack_core::factor_model::FactorId;
    use std::collections::BTreeMap;

    /// Audit item #5: on a sparse panel the pairwise-overlap mean differs from
    /// each factor's marginal mean. Demeaning the covariance with the marginal
    /// mean (the previous behavior) is not a Pearson correlation; a perfectly
    /// co-moving pair on the overlap window must come back as ρ = +1.
    #[test]
    fn sample_correlation_uses_pairwise_overlap_mean_on_sparse_panel() {
        // Factor A: observed on dates 0..4. Factor B: observed on dates 2..4.
        // On the overlap (dates 2,3) the two move identically, so the true
        // pairwise correlation is exactly +1. A's marginal mean (over 4 obs)
        // is far from its overlap mean (over 2 obs) — the bug would produce a
        // biased value, possibly even > 1 before the clamp.
        let a = FactorId::new("credit::generic");
        let b = FactorId::new("credit::bucket::A");
        let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
        returns.insert(
            a.clone(),
            vec![Some(-5.0), Some(-4.0), Some(1.0), Some(2.0)],
        );
        returns.insert(b.clone(), vec![None, None, Some(1.0), Some(2.0)]);

        let order = vec![a.clone(), b.clone()];
        let rho = sample_correlation_flat(&order, &returns);

        // 2x2 flat row-major: [aa, ab, ba, bb].
        assert_eq!(rho.len(), 4);
        assert!((rho[0] - 1.0).abs() < 1e-12, "diagonal must be 1.0");
        assert!((rho[3] - 1.0).abs() < 1e-12, "diagonal must be 1.0");
        assert!(
            (rho[1] - 1.0).abs() < 1e-9,
            "perfectly co-moving overlap must give correlation +1, got {}",
            rho[1]
        );
        assert!(
            (rho[1] - rho[2]).abs() < 1e-15,
            "correlation matrix must be symmetric"
        );
    }

    /// A perfectly anti-correlated overlap must give ρ = −1 regardless of the
    /// (different) marginal means.
    #[test]
    fn sample_correlation_handles_anti_correlated_overlap() {
        let a = FactorId::new("credit::generic");
        let b = FactorId::new("credit::bucket::A");
        let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
        returns.insert(a.clone(), vec![Some(10.0), Some(1.0), Some(2.0), Some(3.0)]);
        returns.insert(b.clone(), vec![None, Some(-1.0), Some(-2.0), Some(-3.0)]);

        let order = vec![a, b];
        let rho = sample_correlation_flat(&order, &returns);
        assert!(
            (rho[1] - (-1.0)).abs() < 1e-9,
            "perfectly anti-correlated overlap must give -1, got {}",
            rho[1]
        );
    }

    /// Audit item #12: `factor_variances` must use the unbiased (`n − 1`)
    /// sample-variance estimator, not population (`÷ n`).
    #[test]
    fn factor_variances_use_unbiased_n_minus_one_estimator() {
        // Series {0, 2}: mean 1, Σ(dev²) = 1 + 1 = 2.
        //   population variance = 2/2 = 1.0
        //   unbiased  variance  = 2/1 = 2.0
        let fid = FactorId::new("credit::generic");
        let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
        returns.insert(fid.clone(), vec![Some(0.0), Some(2.0)]);

        let out = factor_variances(&returns, 1.0);
        let var = *out.get(&fid).expect("variance present");
        assert!(
            (var - 2.0).abs() < 1e-12,
            "expected unbiased variance 2.0 (n-1), got {var}"
        );
    }
}
