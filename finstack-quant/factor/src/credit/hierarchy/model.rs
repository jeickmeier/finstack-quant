//! Canonical calibrated credit factor model artifact, diagnostics, and validation.

use super::correlation::FactorCorrelationMatrix;
use super::spec::{
    CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaPolicy, IssuerBetaRow,
    dimension_key,
};
use crate::credit::calibration::{BucketWeighting, PanelFrequency, PanelSpace};
use crate::{FactorId, FactorModelConfig};
use finstack_quant_core::contract::{
    ContractDescriptor, ContractError, LoadLimits, ValidationReport,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Persistence contract for [`CreditFactorModel`].
const CREDIT_FACTOR_MODEL_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.credit_factor_model");

/// Sole supported credit-factor-model contract marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum CreditFactorModelSchema {
    /// Canonical v1 credit factor model.
    #[serde(rename = "finstack_quant.credit_factor_model/1")]
    CreditFactorModel,
}

impl CreditFactorModelSchema {
    /// The exact marker required by every persisted credit factor model.
    pub const CURRENT: Self = Self::CreditFactorModel;

    /// Return the exact namespaced persistence marker.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "finstack_quant.credit_factor_model/1"
    }
}

// Date range (no DateRange exists yet in finstack-quant-core)

/// A closed calendar-date interval `[start, end]`.
///
/// Used to record the history window consumed by calibration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DateRange {
    /// First date of the window (inclusive).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// Last date of the window (inclusive).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
}

/// Factor level values for a single hierarchy level at the calibration anchor date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelAnchor {
    /// Zero-based index of this level in [`CreditHierarchySpec::levels`].
    pub level_index: usize,
    /// Dimension identifier for this level.
    pub dimension: HierarchyDimension,
    /// Factor level values keyed by dotted bucket path (e.g. `"IG.EU.FIN"`).
    ///
    /// `BTreeMap` for deterministic serialization order.
    pub values: BTreeMap<String, f64>,
}

/// Snapshot of all factor levels at the calibration anchor date.
///
/// Used as the carry term in attribution: `L(t) = L_anchor + ΔL(t)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelsAtAnchor {
    /// Value of the generic PC factor at `as_of`.
    pub pc: f64,
    /// Per-level anchor values in hierarchy spec order.
    pub by_level: Vec<LevelAnchor>,
}

/// Volatility model for a single factor.
///
/// The `Sample` variant stores a single variance estimate; `Ewma` additionally
/// persists the smoothing parameter used at calibration time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum FactorVolModel {
    /// Simple sample-variance estimate.
    Sample {
        /// Annualized variance estimate for this factor.
        variance: f64,
    },
    /// RiskMetrics exponentially weighted moving-average estimate.
    ///
    /// Persists both the smoothing parameter and the calibrated annualized
    /// variance. `lambda` is retained as calibration provenance: no consumer
    /// reads it back today (the martingale forecast in
    /// `FactorCovarianceForecast` matches on the `Ewma { variance, .. }`
    /// shape and ignores λ, since horizon scaling doesn't depend on it), but
    /// keeping it on the wire preserves the option for a future
    /// term-structure-aware forecaster to recompute or extend the estimate
    /// without re-reading the calibration config.
    ///
    /// # References
    ///
    /// - Longerstaey, J., & Spencer, M. (1996). *RiskMetrics — Technical
    ///   Document* (4th ed.). J.P. Morgan/Reuters. §5.2. `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
    Ewma {
        /// Smoothing parameter λ ∈ (0, 1) used at calibration time.
        lambda: f64,
        /// Annualized one-step-ahead variance forecast.
        variance: f64,
    },
}

/// Volatility model for an issuer's idiosyncratic adder.
///
/// Mirrors [`FactorVolModel`] in structure; kept separate so per-issuer and
/// per-factor models can diverge independently in later PRs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IdiosyncraticVolModel {
    /// Simple sample-variance estimate for the idiosyncratic adder.
    Sample {
        /// Annualized variance of the issuer's idiosyncratic adder.
        variance: f64,
    },
    /// RiskMetrics exponentially weighted moving-average estimate.
    ///
    /// Persists both the smoothing parameter and the calibrated annualized
    /// variance. `lambda` is retained as calibration provenance: no consumer
    /// reads it back today (the martingale forecast in
    /// `FactorCovarianceForecast` matches on the `Ewma { variance, .. }`
    /// shape and ignores λ, since horizon scaling doesn't depend on it), but
    /// keeping it on the wire preserves the option for a future
    /// term-structure-aware forecaster to recompute or extend the estimate
    /// without re-reading the calibration config.
    ///
    /// # References
    ///
    /// - Longerstaey, J., & Spencer, M. (1996). *RiskMetrics — Technical
    ///   Document* (4th ed.). J.P. Morgan/Reuters. §5.2. `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
    Ewma {
        /// Smoothing parameter λ ∈ (0, 1) used at calibration time.
        lambda: f64,
        /// Annualized one-step-ahead variance forecast for the idiosyncratic
        /// adder.
        variance: f64,
    },
}

/// Complete vol state for all factors and all issuers at the calibration date.
///
/// Records unregularized per-factor variance estimates and drives per-issuer
/// idiosyncratic volatility forecasts. Systematic covariance forecasts scale
/// the calibrated covariance, preserving the selected covariance estimator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct VolState {
    /// EWMA or sample vol model for each systematic factor.
    ///
    /// Keys are factor IDs from [`crate::FactorModelConfig`].
    /// `BTreeMap` for deterministic serialization order.
    pub factors: BTreeMap<FactorId, FactorVolModel>,
    /// Idiosyncratic vol model for each issuer.
    ///
    /// `BTreeMap` for deterministic serialization order.
    pub idiosyncratic: BTreeMap<IssuerId, IdiosyncraticVolModel>,
}

/// Embedded time-series of factor **moves in bp**.
///
/// These are the official series for rebuilding vol/correlation and for
/// historical-simulation factor P&L. Every date is a real observation
/// (no `None → 0.0` holes). Under [`PanelSpace::Returns`]
/// the stored values are already period moves; under
/// [`PanelSpace::Levels`]
/// they are peeled levels and must be first-differenced before vol or P&L.
///
/// `BTreeMap<FactorId, Vec<f64>>` for deterministic serialization. All value
/// vectors must have the same length as `dates`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FactorHistories {
    /// Ordered sequence of observation dates (aligned with value vectors).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Factor return series keyed by factor ID.
    ///
    /// Each vector must have `dates.len()` entries.
    pub values: BTreeMap<FactorId, Vec<f64>>,
}

/// Record of a single fold-up event during calibration.
///
/// Fold-up means **omit the sparse child factor** and set `β_k = 0` at that
/// level. The issuer already sits in the parent bucket, so its residual
/// continues to contribute to the parent mean. This is not a re-tagging of
/// the issuer into a different leaf.
///
/// # Where the folded risk lands
///
/// No factor is re-assigned: the variation the sparse bucket's factor would
/// have carried stays in each member's residual and flows into the
/// per-issuer **adder** (idiosyncratic) vol. Members of the same folded
/// bucket therefore share a common component that Σ represents as
/// *uncorrelated* idiosyncratic risk — holding several names from one folded
/// bucket overstates diversification for that component. Lower the level's
/// [`BucketSizeThresholds`][crate::credit::calibration::BucketSizeThresholds]
/// entry if that correlation materially matters for a thin bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FoldUpRecord {
    /// Issuer that was folded up.
    pub issuer_id: IssuerId,
    /// Hierarchy level at which the fold-up occurred.
    pub level_index: usize,
    /// Bucket path before the fold-up (e.g. `"IG.EU.FIN"`).
    pub original_bucket: String,
    /// Deepest surviving ancestor bucket path (e.g. `"IG.EU"`), recorded for
    /// diagnostics only. The member's risk is **not** re-assigned to this
    /// bucket's factor — the parent level was already peeled at `k − 1`; the
    /// sparse bucket's own variation moves into the issuer adder instead.
    pub folded_to: String,
    /// Human-readable reason for the fold-up (e.g. `"fewer than 5 issuers"`).
    pub reason: String,
}

/// Structured diagnostics attached to every calibrated artifact.
///
/// Consumers can programmatically check coverage (e.g. "≥ 95 % of buckets
/// had ≥ 5 issuers") without parsing free-form log messages.
///
/// This struct omits `#[serde(deny_unknown_fields)]` to allow additive
/// diagnostic fields in future calibration versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditCalibrationDiagnostics {
    /// Count of resolved [`IssuerBetaMode`] values.
    ///
    /// Keys are `"issuer_beta"` and `"bucket_only"`.
    pub mode_counts: BTreeMap<String, usize>,
    /// One entry per hierarchy level: `BTreeMap<bucket_path, member_count>`.
    /// Counts every issuer assigned to the bucket regardless of
    /// [`IssuerBetaMode`]; the same full-membership count gates fold-up.
    pub bucket_sizes_per_level: Vec<BTreeMap<String, usize>>,
    /// Log of all fold-up events triggered by insufficient bucket coverage.
    ///
    /// **Load-bearing, not purely diagnostic:**
    /// [`decompose_levels`][crate::credit::decomposition::decompose_levels]
    /// reads these records to reconstruct which `(issuer, level)` pairs were
    /// folded during calibration. Stripping or editing them changes
    /// decomposition results.
    pub fold_ups: Vec<FoldUpRecord>,
    /// Optional histogram of per-issuer R² values (bucketed as string ranges).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r_squared_histogram: Option<BTreeMap<String, usize>>,
    /// Canonical tag taxonomy observed during calibration.
    ///
    /// Keys are dimension names (e.g. `"rating"`, `"region"`, `"sector"`);
    /// values are the set of distinct observed tag values.
    pub tag_taxonomy: BTreeMap<String, BTreeSet<String>>,
}

/// Reference to the generic (PC) time series used as the first factor.
///
/// Values are not stored here; they live in
/// [`FactorHistories`] under the key `"credit::generic"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GenericFactorSpec {
    /// Human-readable name for the generic factor (e.g. `"CDX IG 5Y"`).
    pub name: String,
    /// Caller's time-series identifier, used to look up the input data.
    pub series_id: String,
}

/// Fully self-contained credit factor model artifact.
///
/// Produced by offline monthly calibration and loaded at startup by attribution,
/// risk, and vol-forecast consumers. JSON serialization is deterministic:
/// `serde_json::to_string` followed by `serde_json::from_str` must produce a
/// byte-identical round-trip.
///
/// # Schema version
///
/// [`schema`][Self::schema] is deserialized as an exact v1 marker.
///
/// # Determinism
///
/// Two `CreditFactorModel` values constructed from identical inputs serialize
/// to byte-identical JSON. This relies on:
/// - [`issuer_betas`][Self::issuer_betas] sorted by `issuer_id`.
/// - All maps using `BTreeMap`.
/// - [`crate::FactorModelConfig`] respecting its own factor ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditFactorModel {
    /// Exact namespaced v1 schema marker.
    pub schema: CreditFactorModelSchema,
    /// Calibration anchor date (`as_of`).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// History window consumed by calibration.
    pub calibration_window: DateRange,
    /// Beta regression policy used during calibration.
    pub policy: IssuerBetaPolicy,
    /// Reference to the generic PC factor series.
    pub generic_factor: GenericFactorSpec,
    /// Ordered hierarchy specification (broadest → narrowest).
    pub hierarchy: CreditHierarchySpec,
    /// Regular observation frequency used to annualize variance (`252`/`12`/`4`).
    pub panel_frequency: PanelFrequency,
    /// Whether calibration peeled a return panel or a raw level panel.
    pub use_returns_or_levels: PanelSpace,
    /// Bucket-mean weighting used at calibration and required at decompose.
    ///
    /// [`BucketWeighting::Equal`] artifacts must not be DTS-weighted at
    /// decompose; [`BucketWeighting::Dts`] artifacts rebuild weights from
    /// persisted [`IssuerBetaRow::spread_duration`] × current spread (bp).
    pub bucket_weighting: BucketWeighting,
    /// Existing factor-model config (factors, covariance, matching).
    pub config: FactorModelConfig,
    /// Per-issuer beta rows, sorted by `issuer_id` for wire stability.
    pub issuer_betas: Vec<IssuerBetaRow>,
    /// Factor level values at the calibration anchor date.
    pub anchor_state: LevelsAtAnchor,
    /// Correlation estimate retained from covariance calibration.
    ///
    /// `config.covariance` is authoritative for both point-in-time risk and
    /// horizon forecasts. Under ridge this correlation precedes the diagonal
    /// ridge addition; under Ledoit-Wolf it is derived from the shrunk
    /// covariance. Combining it with the unregularized `vol_state.factors`
    /// does not generally reconstruct the selected covariance estimator.
    pub static_correlation: FactorCorrelationMatrix,
    /// EWMA or sample vol state at the anchor date.
    pub vol_state: VolState,
    /// Embedded factor histories (recommended for self-contained artifacts).
    ///
    /// `None` indicates an externally-referenced history store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor_histories: Option<FactorHistories>,
    /// Structured calibration diagnostics for programmatic coverage checks.
    pub diagnostics: CreditCalibrationDiagnostics,
}

impl CreditFactorModel {
    /// Names of the hierarchy levels, broadest first.
    ///
    /// Each name is the level's [`dimension_key`] (`"rating"`, `"region"`,
    /// `"sector"` or a custom dimension's own key), so it matches the serde
    /// form of [`HierarchyDimension`], the issuer-tag keys and the level
    /// dimensions in every serialized artifact.
    #[must_use]
    pub fn level_names(&self) -> Vec<String> {
        self.hierarchy
            .levels
            .iter()
            .map(|level| dimension_key(level).to_owned())
            .collect()
    }

    /// Identifiers of the calibrated issuers, in issuer-beta row order.
    #[must_use]
    pub fn issuer_ids(&self) -> Vec<String> {
        self.issuer_betas
            .iter()
            .map(|row| row.issuer_id.as_str().to_owned())
            .collect()
    }

    /// Identifiers of the factors, in covariance order.
    #[must_use]
    pub fn factor_ids(&self) -> Vec<String> {
        self.config
            .factors
            .iter()
            .map(|factor| factor.id.to_string())
            .collect()
    }

    /// Load and validate a persisted credit-factor-model artifact.
    ///
    /// This entry point fuses bounded JSON deserialization, explicit schema
    /// enforcement, and [`Self::validate`] so callers cannot accidentally use
    /// an unchecked artifact.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a credit factor model.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// missing, malformed, or unsupported schema markers, invalid artifact
    /// shape, or failed internal-consistency validation.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> Result<(Self, ValidationReport), ContractError> {
        let model: Self = CREDIT_FACTOR_MODEL_CONTRACT.decode(bytes, limits)?;
        model
            .validate()
            .map_err(|error| CREDIT_FACTOR_MODEL_CONTRACT.semantic_error(error, limits))?;
        Ok((model, ValidationReport::default()))
    }

    /// Deserialize and validate a credit-factor-model artifact from JSON.
    ///
    /// Runs the serde shape checks (including the exact `schema` marker and
    /// `deny_unknown_fields`) and then [`Self::validate`]. Use
    /// [`Self::from_slice_strict`] instead when loading an untrusted
    /// persisted artifact that needs resource limits and a structured
    /// diagnostics report.
    ///
    /// # Arguments
    ///
    /// * `json` - Complete JSON encoding of a credit factor model, as produced
    ///   by [`Self::to_json`] or the calibrator.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the JSON is
    /// malformed, does not match the artifact shape, or fails
    /// [`Self::validate`].
    pub fn from_json(json: &str) -> finstack_quant_core::Result<Self> {
        let model: Self = serde_json::from_str(json).map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "invalid CreditFactorModel JSON: {error}"
            ))
        })?;
        model.validate()?;
        Ok(model)
    }

    /// Serialize the artifact to compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if serialization
    /// fails.
    pub fn to_json(&self) -> finstack_quant_core::Result<String> {
        serde_json::to_string(self).map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "cannot serialize CreditFactorModel: {error}"
            ))
        })
    }

    /// Validate the artifact's internal consistency.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `issuer_betas` contains duplicate `issuer_id` values.
    /// - `hierarchy.levels` contains duplicate dimension names.
    /// - any issuer tag value used by a hierarchy dimension contains `'.'`
    ///   (reserved as the bucket-path separator; a dotted value corrupts
    ///   factor identity).
    /// - `factor_histories` has vectors of inconsistent length.
    /// - `static_correlation` fails structural checks (shape, diagonal, symmetry, duplicate IDs).
    /// - the matching config can emit a factor ID not declared in
    ///   `config.factors` (see [`crate::FactorModelConfig::validate`]) — such
    ///   a factor would silently contribute zero risk at lookup time.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Duplicate hierarchy dimension keys. Dedup on `dimension_key` — the
        // exact key used to read tags at runtime — so `Rating` and
        // `Custom("rating")` collide here just as they do at lookup time
        // (both read `tags["rating"]`, i.e. the same information twice).
        let mut seen_dims: BTreeSet<&str> = BTreeSet::new();
        for dim in &self.hierarchy.levels {
            let key = dimension_key(dim);
            if !seen_dims.insert(key) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CreditFactorModel: duplicate hierarchy dimension key {key:?}"
                )));
            }
        }

        // Single pass over `issuer_betas` checking:
        // 1. Unique `issuer_id` entries.
        // 2. Tag values used by hierarchy dimensions omit `'.'` (reserved as
        //    the bucket-path separator).
        // 3. `BucketOnly` rows keep conventional loadings (`1.0`, or `0.0` for
        //    folded levels).
        let mut seen_issuers: BTreeSet<&IssuerId> = BTreeSet::new();
        for row in &self.issuer_betas {
            if !seen_issuers.insert(&row.issuer_id) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CreditFactorModel: duplicate issuer_id {:?}",
                    row.issuer_id.as_str()
                )));
            }
            for dim in &self.hierarchy.levels {
                let key = dimension_key(dim);
                if let Some(v) = row.tags.0.get(key)
                    && v.contains('.')
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CreditFactorModel: issuer {:?} tag {key:?} = {v:?} contains '.', \
                             which is reserved as the bucket-path separator",
                        row.issuer_id.as_str()
                    )));
                }
            }
            if row.mode == IssuerBetaMode::BucketOnly {
                let pc_ok = (row.betas.pc - 1.0).abs() < 1e-12;
                let levels_ok = row
                    .betas
                    .levels
                    .iter()
                    .all(|b| b.abs() < 1e-12 || (b - 1.0).abs() < 1e-12);
                if !pc_ok || !levels_ok {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CreditFactorModel: BucketOnly issuer {:?} has non-conventional \
                         betas (pc = {}, levels = {:?}); BucketOnly betas must be 1.0 \
                         (or 0.0 for folded levels)",
                        row.issuer_id.as_str(),
                        row.betas.pc,
                        row.betas.levels
                    )));
                }
            }
        }

        // Static correlation structural re-check (fields are pub, so bypass of new() is possible)
        self.static_correlation.check_structure()?;

        // Every factor ID the matcher can emit must exist in config.factors;
        // an undeclared factor would silently contribute zero risk at
        // covariance-lookup time.
        self.config.validate()?;

        // The static correlation and vol-state factor universes must agree
        // with `config.factors`; otherwise unknown IDs silently zero risk.
        let declared: BTreeSet<&FactorId> = self.config.factors.iter().map(|f| &f.id).collect();
        let check_ids = |label: &str, ids: BTreeSet<&FactorId>| {
            if ids != declared {
                let missing: Vec<&str> =
                    declared.difference(&ids).map(|fid| fid.as_str()).collect();
                let extra: Vec<&str> = ids.difference(&declared).map(|fid| fid.as_str()).collect();
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CreditFactorModel: {label} factor ids do not match config.factors \
                     (missing: {missing:?}, undeclared: {extra:?})"
                )));
            }
            Ok(())
        };
        check_ids(
            "static_correlation",
            self.static_correlation.factor_ids.iter().collect(),
        )?;
        check_ids("vol_state.factors", self.vol_state.factors.keys().collect())?;

        // Factor histories length consistency
        if let Some(hist) = &self.factor_histories {
            let expected = hist.dates.len();
            for (fid, vals) in &hist.values {
                if vals.len() != expected {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CreditFactorModel: factor_histories[{fid}] has {} entries, expected {expected}",
                        vals.len()
                    )));
                }
            }
        }

        Ok(())
    }
}
