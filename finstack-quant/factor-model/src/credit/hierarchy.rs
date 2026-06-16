//! Credit factor hierarchy artifact types (serde-first data model).
//!
//! This module defines the canonical calibration artifact for the credit
//! factor hierarchy. The central type is
//! [`CreditFactorModel`](crate::credit::hierarchy::CreditFactorModel), a
//! fully self-contained JSON artifact produced by offline calibration and
//! consumed at runtime by attribution, risk, and vol-forecast pipelines.
//!
//! # Schema version
//!
//! [`CreditFactorModel::SCHEMA_VERSION`](crate::credit::hierarchy::CreditFactorModel::SCHEMA_VERSION)
//! is `"finstack_quant.credit_factor_model/1"`.
//! Consumers must check this field before trusting any other field.
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_factor_model::credit::hierarchy::CreditFactorModel;
//!
//! // Deserialize from JSON — call `model.validate()` to check schema_version and consistency
//! let json = r#"{
//!   "schema_version": "finstack_quant.credit_factor_model/1",
//!   "as_of": "2024-03-29",
//!   "calibration_window": { "start": "2022-03-29", "end": "2024-03-29" },
//!   "policy": "globally_off",
//!   "generic_factor": { "name": "CDX IG", "series_id": "cdx.ig.5y" },
//!   "hierarchy": { "levels": ["rating", "region", "sector"] },
//!   "config": {
//!     "factors": [],
//!     "covariance": { "n": 0, "factor_ids": [], "data": [] },
//!     "matching": { "MappingTable": [] },
//!     "pricing_mode": "delta_based"
//!   },
//!   "issuer_betas": [],
//!   "anchor_state": { "pc": 0.0, "by_level": [] },
//!   "static_correlation": { "factor_ids": [], "data": [] },
//!   "vol_state": { "factors": {}, "idiosyncratic": {} },
//!   "factor_histories": null,
//!   "diagnostics": {
//!     "mode_counts": {},
//!     "bucket_sizes_per_level": [],
//!     "fold_ups": [],
//!     "r_squared_histogram": null,
//!     "tag_taxonomy": {}
//!   }
//! }"#;
//!
//! let model: CreditFactorModel = serde_json::from_str(json).expect("valid artifact");
//! assert_eq!(model.schema_version, CreditFactorModel::SCHEMA_VERSION);
//! ```
//!
//! # Design notes
//!
//! - Stable artifact structs use `#[serde(deny_unknown_fields)]` to catch schema
//!   drift early. Sub-types where future extension is anticipated (e.g.
//!   `FactorVolModel`, `CalibrationDiagnostics`) omit `deny_unknown_fields` to
//!   allow additive forward-compatible extension without breaking older writers.
//! - All keyed maps use `BTreeMap` for deterministic serialization order.
//! - `Vec<IssuerBetaRow>` is kept sorted by `issuer_id` so two calibrations on
//!   the same inputs produce byte-identical JSON.

use crate::{FactorId, FactorModelConfig};
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// dimension_key helper — lives here so CreditHierarchySpec can use it
// ---------------------------------------------------------------------------

/// Canonical lowercase key used to read a [`HierarchyDimension`] from a tag map.
///
/// - `Rating` → `"rating"`
/// - `Region` → `"region"`
/// - `Sector` → `"sector"`
/// - `Custom(name)` → `name` (the caller-chosen string, used verbatim).
#[must_use]
pub fn dimension_key(dim: &HierarchyDimension) -> String {
    match dim {
        HierarchyDimension::Rating => "rating".to_owned(),
        HierarchyDimension::Region => "region".to_owned(),
        HierarchyDimension::Sector => "sector".to_owned(),
        HierarchyDimension::Custom(name) => name.clone(),
    }
}

// ---------------------------------------------------------------------------
// Date range (no DateRange exists yet in finstack-quant-core)
// ---------------------------------------------------------------------------

/// A closed calendar-date interval `[start, end]`.
///
/// Used to record the history window consumed by calibration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateRange {
    /// First date of the window (inclusive).
    pub start: Date,
    /// Last date of the window (inclusive).
    pub end: Date,
}

// ---------------------------------------------------------------------------
// Policy types
// ---------------------------------------------------------------------------

/// Per-issuer regression behavior override supplied by the user before calibration.
///
/// This is the *input* override; the *resolved* outcome is [`IssuerBetaMode`].
///
/// - `Auto` — let the calibration decide based on `min_history`.
/// - `ForceIssuerBeta` — always run per-issuer regression regardless of history.
/// - `ForceBucketOnly` — never run per-issuer regression for this issuer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerBetaOverride {
    /// Let calibration decide based on available history.
    Auto,
    /// Force per-issuer OLS regression even with limited history.
    ForceIssuerBeta,
    /// Suppress per-issuer regression; use bucket average only.
    ForceBucketOnly,
}

/// Resolved regression mode stored in the calibrated artifact.
///
/// A `BucketOnly` issuer's betas are all 1.0 and carry no fit statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerBetaMode {
    /// Per-issuer OLS beta was estimated.
    IssuerBeta,
    /// Issuer uses the bucket-average beta (all β = 1.0).
    BucketOnly,
}

/// Calibration policy governing which issuers receive a per-issuer regression.
///
/// - `Dynamic` — apply a minimum-history threshold and honour per-issuer overrides.
/// - `GloballyOff` — every issuer is treated as `BucketOnly`; no per-issuer
///   regression is run.  Useful for simpler factor models or data-sparse periods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerBetaPolicy {
    /// Regression is gated on a minimum history threshold with per-issuer overrides.
    Dynamic {
        /// Minimum number of monthly return observations needed to attempt OLS.
        ///
        /// Default is 24 months.
        min_history: usize,
        /// Per-issuer overrides that can force or suppress per-issuer regression.
        ///
        /// Keys without an entry default to [`IssuerBetaOverride::Auto`].
        overrides: BTreeMap<IssuerId, IssuerBetaOverride>,
    },
    /// Every issuer treated as `BucketOnly`; no per-issuer regression is run.
    GloballyOff,
}

// ---------------------------------------------------------------------------
// Hierarchy specification
// ---------------------------------------------------------------------------

/// A single level in the credit factor hierarchy.
///
/// Built-in variants (`Rating`, `Region`, `Sector`) have canonical tag keys.
/// `Custom(key)` reads `issuer_tags[key]` for arbitrary user-defined dimensions
/// such as `"Currency"` or `"AssetType"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HierarchyDimension {
    /// Credit rating bucket (e.g. `"IG"`, `"HY"`, `"NR"`).
    Rating,
    /// Geographic region (e.g. `"EU"`, `"NA"`, `"APAC"`).
    Region,
    /// Industry sector (e.g. `"FIN"`, `"ENERGY"`, `"TECH"`).
    Sector,
    /// User-defined dimension reading `issuer_tags[key]`.
    Custom(String),
}

/// Ordered list of hierarchy dimensions, broadest → narrowest.
///
/// The ordering is significant: factor IDs and beta vectors are indexed
/// positionally from level 0 (broadest) to `levels.len()-1` (narrowest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditHierarchySpec {
    /// Ordered hierarchy levels, broadest first.
    pub levels: Vec<HierarchyDimension>,
}

impl CreditHierarchySpec {
    /// Build the dotted bucket path for an issuer at hierarchy level `k`.
    ///
    /// Reads the tag value for each dimension in `self.levels[0..=k]` from
    /// `tags`, then joins them with `"."`.
    ///
    /// - For `k = 0` returns `Some("<tag_for_dim_0>")`.
    /// - For `k = 1` returns `Some("<tag_for_dim_0>.<tag_for_dim_1>")`.
    /// - For `k = self.levels.len() - 1` returns the full dotted path.
    ///
    /// Returns `None` if `k >= self.levels.len()` or if any tag for
    /// dimensions `0..=k` is missing from `tags`.
    #[must_use]
    pub fn bucket_path(&self, tags: &IssuerTags, k: usize) -> Option<String> {
        if k >= self.levels.len() {
            return None;
        }
        let mut parts = Vec::with_capacity(k + 1);
        for dim in self.levels.iter().take(k + 1) {
            let key = dimension_key(dim);
            let value = tags.0.get(&key)?;
            parts.push(value.clone());
        }
        Some(parts.join("."))
    }
}

// ---------------------------------------------------------------------------
// Issuer tags and betas
// ---------------------------------------------------------------------------

/// Flat key-value taxonomy tags for an issuer.
///
/// Uses `BTreeMap` so that serialization is deterministic and two artifacts
/// built from identical inputs produce byte-identical JSON.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IssuerTags(pub BTreeMap<String, String>);

/// Factor beta loadings for a single issuer.
///
/// `pc` is the loading on the generic (PC) factor.
/// `levels[i]` is the loading on the bucket factor at hierarchy level `i`.
///
/// For `BucketOnly` issuers every component is `1.0` by convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuerBetas {
    /// Beta on the generic credit PC factor.
    pub pc: f64,
    /// Betas on each hierarchy-level factor, in spec order.
    pub levels: Vec<f64>,
}

/// Source provenance of an issuer's idiosyncratic vol estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdderVolSource {
    /// Estimated from the issuer's own residual history.
    FromHistory,
    /// Proxied from the peer-bucket distribution.
    BucketPeerProxy {
        /// Dotted bucket path used as proxy (e.g. `"IG.EU.FIN"`).
        peer_bucket: String,
    },
    /// Supplied directly by the caller at calibration time.
    CallerSupplied,
    /// Hard-coded fallback default.
    Default,
}

/// Regression quality statistics for a single issuer.
///
/// Only present for `IssuerBeta` mode; `None` for `BucketOnly`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitQuality {
    /// In-sample coefficient of determination (R²).
    pub r_squared: f64,
    /// Residual standard deviation in spread-return units.
    pub residual_std: f64,
    /// Number of monthly observations used in the regression.
    pub n_obs: usize,
}

/// Per-issuer beta row in the calibrated artifact.
///
/// Rows are stored sorted by `issuer_id` for wire stability: two calibrations
/// on identical inputs serialize to byte-identical JSON regardless of
/// iteration order inside the calibration loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuerBetaRow {
    /// Unique issuer identifier (e.g. LEI or internal code).
    pub issuer_id: IssuerId,
    /// Taxonomy tags used to assign the issuer to hierarchy buckets.
    pub tags: IssuerTags,
    /// Resolved regression mode for this issuer.
    pub mode: IssuerBetaMode,
    /// Factor beta loadings (all `1.0` for `BucketOnly` issuers).
    pub betas: IssuerBetas,
    /// Value of the issuer's idiosyncratic adder at `as_of` (carry component).
    pub adder_at_anchor: f64,
    /// Annualized idiosyncratic adder volatility (for vol forecasting).
    pub adder_vol_annualized: f64,
    /// Provenance of `adder_vol_annualized`.
    pub adder_vol_source: AdderVolSource,
    /// Regression fit statistics; `None` when `mode == BucketOnly`.
    pub fit_quality: Option<FitQuality>,
}

// ---------------------------------------------------------------------------
// Anchor state
// ---------------------------------------------------------------------------

/// Factor level values for a single hierarchy level at the calibration anchor date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[serde(deny_unknown_fields)]
pub struct LevelsAtAnchor {
    /// Value of the generic PC factor at `as_of`.
    pub pc: f64,
    /// Per-level anchor values in hierarchy spec order.
    pub by_level: Vec<LevelAnchor>,
}

// ---------------------------------------------------------------------------
// Correlation matrix
// ---------------------------------------------------------------------------

/// Static factor correlation matrix `ρ` for the covariance decomposition
/// `Σ(t) = D(t) · ρ · D(t)` where `D(t)` is the diagonal vol matrix.
///
/// `factor_ids` defines the row/column ordering; `data[i][j]` is
/// `ρ_{factor_ids[i], factor_ids[j]}`. The matrix must be square, symmetric,
/// and have unit diagonal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorCorrelationMatrix {
    /// Factor IDs in row/column order.
    pub factor_ids: Vec<FactorId>,
    /// Row-major correlation data. `data[i]` is row `i`.
    pub data: Vec<Vec<f64>>,
}

impl FactorCorrelationMatrix {
    /// Construct and validate a correlation matrix.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `data.len() != factor_ids.len()`
    /// - Any row has length `!= factor_ids.len()`
    /// - Any diagonal entry deviates from `1.0` by more than `1e-9`
    /// - The matrix is not symmetric within `1e-9`
    /// - the `factor_ids` list contains duplicates.
    pub fn new(
        factor_ids: Vec<FactorId>,
        data: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<Self> {
        let n = factor_ids.len();
        let mut seen = std::collections::BTreeSet::new();
        for fid in &factor_ids {
            if !seen.insert(fid) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: duplicate factor_id {fid:?}"
                )));
            }
        }
        if data.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCorrelationMatrix: expected {n} rows, got {}",
                data.len()
            )));
        }
        for (i, row) in data.iter().enumerate() {
            if row.len() != n {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: row {i} has length {}, expected {n}",
                    row.len()
                )));
            }
            let diag = row[i];
            if (diag - 1.0).abs() > 1e-9 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: diagonal entry [{i}][{i}] = {diag}, expected 1.0"
                )));
            }
        }
        // Check symmetry: data[i][j] must equal data[j][i].
        // We need two-dimensional cross-indexing here, so range loops are
        // the clearest choice. Clippy's needless_range_loop suggestion would
        // iterate over one dimension but still require indexing the other.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            for j in (i + 1)..n {
                let lo = data[i][j];
                let hi = data[j][i];
                if (lo - hi).abs() > 1e-9 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FactorCorrelationMatrix: not symmetric at [{i}][{j}]: {lo} vs {hi}"
                    )));
                }
            }
        }
        Ok(Self { factor_ids, data })
    }

    /// Construct an identity correlation matrix for the given factor IDs.
    #[must_use]
    pub fn identity(factor_ids: Vec<FactorId>) -> Self {
        let n = factor_ids.len();
        let data = (0..n)
            .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        Self { factor_ids, data }
    }

    /// Check the structural validity of `&self` (shape, diagonal, symmetry, no duplicate IDs).
    ///
    /// Called by [`CreditFactorModel::validate`] to catch matrices that were
    /// constructed via direct field assignment rather than through [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `data.len() != factor_ids.len()`
    /// - Any row has length `!= factor_ids.len()`
    /// - Any diagonal entry deviates from `1.0` by more than `1e-9`
    /// - The matrix is not symmetric within `1e-9`
    /// - `factor_ids` contains duplicates
    pub fn check_structure(&self) -> finstack_quant_core::Result<()> {
        let n = self.factor_ids.len();
        let mut seen = std::collections::BTreeSet::new();
        for fid in &self.factor_ids {
            if !seen.insert(fid) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: duplicate factor_id {fid:?}"
                )));
            }
        }
        if self.data.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCorrelationMatrix: expected {n} rows, got {}",
                self.data.len()
            )));
        }
        for (i, row) in self.data.iter().enumerate() {
            if row.len() != n {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: row {i} has length {}, expected {n}",
                    row.len()
                )));
            }
            let diag = row[i];
            if (diag - 1.0).abs() > 1e-9 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: diagonal entry [{i}][{i}] = {diag}, expected 1.0"
                )));
            }
        }
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            for j in (i + 1)..n {
                let lo = self.data[i][j];
                let hi = self.data[j][i];
                if (lo - hi).abs() > 1e-9 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FactorCorrelationMatrix: not symmetric at [{i}][{j}]: {lo} vs {hi}"
                    )));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Vol state
// ---------------------------------------------------------------------------

/// Volatility model for a single factor.
///
/// The `Sample` variant stores a single variance estimate.
/// The enum intentionally omits `#[serde(deny_unknown_fields)]` to allow
/// additive extension without breaking readers of older writers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorVolModel {
    /// Simple sample-variance estimate.
    Sample {
        /// Annualized variance estimate for this factor.
        variance: f64,
    },
}

/// Volatility model for an issuer's idiosyncratic adder.
///
/// Mirrors [`FactorVolModel`] in structure; kept separate so per-issuer and
/// per-factor models can diverge independently in later PRs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdiosyncraticVolModel {
    /// Simple sample-variance estimate for the idiosyncratic adder.
    Sample {
        /// Annualized variance of the issuer's idiosyncratic adder.
        variance: f64,
    },
}

/// Complete vol state for all factors and all issuers at the calibration date.
///
/// Feeds `Σ(t) = D(t) · ρ · D(t)` and per-issuer idiosyncratic vol forecasts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolState {
    /// GARCH/EWMA/sample vol model for each systematic factor.
    ///
    /// Keys are factor IDs from [`crate::FactorModelConfig`].
    /// `BTreeMap` for deterministic serialization order.
    pub factors: BTreeMap<FactorId, FactorVolModel>,
    /// Idiosyncratic vol model for each issuer.
    ///
    /// `BTreeMap` for deterministic serialization order.
    pub idiosyncratic: BTreeMap<IssuerId, IdiosyncraticVolModel>,
}

// ---------------------------------------------------------------------------
// Factor histories
// ---------------------------------------------------------------------------

/// Embedded time-series of factor returns.
///
/// Recommended default: embed in the artifact (~100 KB for typical configs).
/// External path is supported for very large calibrations.
///
/// `BTreeMap<FactorId, Vec<f64>>` for deterministic serialization. All value
/// vectors must have the same length as `dates`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorHistories {
    /// Ordered sequence of observation dates (aligned with value vectors).
    pub dates: Vec<Date>,
    /// Factor return series keyed by factor ID.
    ///
    /// Each vector must have `dates.len()` entries.
    pub values: BTreeMap<FactorId, Vec<f64>>,
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Record of a single fold-up event during calibration.
///
/// When a bucket lacks sufficient coverage, its issuers are promoted to a
/// coarser level. Each such event is logged here for auditability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldUpRecord {
    /// Issuer that was folded up.
    pub issuer_id: IssuerId,
    /// Hierarchy level at which the fold-up occurred.
    pub level_index: usize,
    /// Bucket path before the fold-up (e.g. `"IG.EU.FIN"`).
    pub original_bucket: String,
    /// Bucket path after the fold-up (e.g. `"IG.EU"`).
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
pub struct CalibrationDiagnostics {
    /// Count of resolved [`IssuerBetaMode`] values.
    ///
    /// Keys are `"issuer_beta"` and `"bucket_only"`.
    pub mode_counts: BTreeMap<String, usize>,
    /// One entry per hierarchy level: `BTreeMap<bucket_path, IssuerBeta_count>`.
    /// Counts only issuers calibrated in `IssuerBeta` mode, since `BucketOnly`
    /// issuers do not affect fold-up thresholds.
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

// ---------------------------------------------------------------------------
// Generic factor spec
// ---------------------------------------------------------------------------

/// Reference to the generic (PC) time series used as the first factor.
///
/// Values are not stored here; they live in
/// [`FactorHistories`] under the key `"credit::generic"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenericFactorSpec {
    /// Human-readable name for the generic factor (e.g. `"CDX IG 5Y"`).
    pub name: String,
    /// Caller's time-series identifier, used to look up the input data.
    pub series_id: String,
}

// ---------------------------------------------------------------------------
// Top-level artifact
// ---------------------------------------------------------------------------

/// Fully self-contained credit factor model artifact.
///
/// Produced by offline monthly calibration and loaded at startup by attribution,
/// risk, and vol-forecast consumers. JSON serialization is deterministic:
/// `serde_json::to_string` followed by `serde_json::from_str` must produce a
/// byte-identical round-trip.
///
/// # Schema version
///
/// Always check [`schema_version`][Self::schema_version] against
/// [`SCHEMA_VERSION`][Self::SCHEMA_VERSION] before trusting content.
///
/// # Determinism
///
/// Two `CreditFactorModel` values constructed from identical inputs serialize
/// to byte-identical JSON. This relies on:
/// - [`issuer_betas`][Self::issuer_betas] sorted by `issuer_id`.
/// - All maps using `BTreeMap`.
/// - [`crate::FactorModelConfig`] respecting its own factor ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditFactorModel {
    /// Schema version string; must equal [`SCHEMA_VERSION`][Self::SCHEMA_VERSION].
    pub schema_version: String,
    /// Calibration anchor date (`as_of`).
    pub as_of: Date,
    /// History window consumed by calibration.
    pub calibration_window: DateRange,
    /// Beta regression policy used during calibration.
    pub policy: IssuerBetaPolicy,
    /// Reference to the generic PC factor series.
    pub generic_factor: GenericFactorSpec,
    /// Ordered hierarchy specification (broadest → narrowest).
    pub hierarchy: CreditHierarchySpec,
    /// Existing factor-model config (factors, covariance, matching).
    pub config: FactorModelConfig,
    /// Per-issuer beta rows, sorted by `issuer_id` for wire stability.
    pub issuer_betas: Vec<IssuerBetaRow>,
    /// Factor level values at the calibration anchor date.
    pub anchor_state: LevelsAtAnchor,
    /// Static factor correlation matrix `ρ` for `Σ(t) = D(t)·ρ·D(t)`.
    ///
    /// **Which matrix is authoritative:** vol forecasting rebuilds
    /// `Σ(t, h) = D·ρ·D` from this matrix plus `vol_state`; point-in-time
    /// risk uses `config.covariance` directly. Under
    /// [`crate::CovarianceStrategy::Ridge`] the two deliberately differ —
    /// `config.covariance = D·ρ·D + α·I`, so its implied correlations are
    /// shrunk relative to `ρ` by `σᵢσⱼ/√((σᵢ²+α)(σⱼ²+α))`.
    pub static_correlation: FactorCorrelationMatrix,
    /// GARCH/EWMA/sample vol state at the anchor date.
    pub vol_state: VolState,
    /// Embedded factor histories (recommended for self-contained artifacts).
    ///
    /// `None` indicates an externally-referenced history store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor_histories: Option<FactorHistories>,
    /// Structured calibration diagnostics for programmatic coverage checks.
    pub diagnostics: CalibrationDiagnostics,
}

impl CreditFactorModel {
    /// Canonical schema version for this artifact format.
    pub const SCHEMA_VERSION: &'static str = "finstack_quant.credit_factor_model/1";

    /// Validate the artifact's schema version and internal consistency.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `schema_version` does not equal [`Self::SCHEMA_VERSION`].
    /// - `issuer_betas` contains duplicate `issuer_id` values.
    /// - `hierarchy.levels` contains duplicate dimension names.
    /// - any issuer tag value used by a hierarchy dimension contains `'.'`
    ///   (reserved as the bucket-path separator; a dotted value corrupts
    ///   factor identity).
    /// - `factor_histories` has vectors of inconsistent length.
    /// - `static_correlation` fails structural checks (shape, diagonal, symmetry, duplicate IDs).
    /// - the matching config can emit a factor ID not declared in
    ///   `config.factors` (see
    ///   [`crate::FactorModelConfig::validate_matching_factor_ids`]) — such a
    ///   factor would silently contribute zero risk at lookup time.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Schema version
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CreditFactorModel: expected schema_version {:?}, got {:?}",
                Self::SCHEMA_VERSION,
                self.schema_version
            )));
        }

        // Duplicate issuers
        let mut seen_issuers: BTreeSet<&IssuerId> = BTreeSet::new();
        for row in &self.issuer_betas {
            if !seen_issuers.insert(&row.issuer_id) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CreditFactorModel: duplicate issuer_id {:?}",
                    row.issuer_id.as_str()
                )));
            }
        }

        // Duplicate hierarchy dimension names
        let mut seen_dims: BTreeSet<String> = BTreeSet::new();
        for dim in &self.hierarchy.levels {
            let key = match dim {
                HierarchyDimension::Rating => "rating".to_owned(),
                HierarchyDimension::Region => "region".to_owned(),
                HierarchyDimension::Sector => "sector".to_owned(),
                HierarchyDimension::Custom(s) => format!("custom:{s}"),
            };
            if !seen_dims.insert(key.clone()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CreditFactorModel: duplicate hierarchy dimension {key:?}"
                )));
            }
        }

        // Tag values used by hierarchy dimensions must not contain the '.'
        // bucket-path separator (it would mis-segment bucket paths and
        // factor IDs in calibration, fold-up, and matching).
        for row in &self.issuer_betas {
            for dim in &self.hierarchy.levels {
                let key = dimension_key(dim);
                if let Some(v) = row.tags.0.get(&key) {
                    if v.contains('.') {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "CreditFactorModel: issuer {:?} tag {key:?} = {v:?} contains '.', \
                             which is reserved as the bucket-path separator",
                            row.issuer_id.as_str()
                        )));
                    }
                }
            }
        }

        // Static correlation structural re-check (fields are pub, so bypass of new() is possible)
        self.static_correlation.check_structure()?;

        // Every factor ID the matcher can emit must exist in config.factors;
        // an undeclared factor would silently contribute zero risk at
        // covariance-lookup time.
        self.config.validate_matching_factor_ids()?;

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FactorCovarianceMatrix, FactorDefinition, FactorModelConfig, FactorType, MarketMapping,
        MatchingConfig, PricingMode,
    };
    use finstack_quant_core::dates::create_date;
    use time::Month;

    // ------------------------------------------------------------------
    // Test helpers
    // ------------------------------------------------------------------

    fn empty_factor_model_config() -> FactorModelConfig {
        FactorModelConfig {
            factors: vec![],
            covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: Default::default(),
            bump_size: None,
            unmatched_policy: None,
        }
    }

    fn minimal_model() -> CreditFactorModel {
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
                levels: vec![
                    HierarchyDimension::Rating,
                    HierarchyDimension::Region,
                    HierarchyDimension::Sector,
                ],
            },
            config: empty_factor_model_config(),
            issuer_betas: vec![],
            anchor_state: LevelsAtAnchor {
                pc: 0.0,
                by_level: vec![],
            },
            static_correlation: FactorCorrelationMatrix::identity(vec![]),
            vol_state: VolState {
                factors: BTreeMap::new(),
                idiosyncratic: BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: CalibrationDiagnostics {
                mode_counts: BTreeMap::new(),
                bucket_sizes_per_level: vec![],
                fold_ups: vec![],
                r_squared_histogram: None,
                tag_taxonomy: BTreeMap::new(),
            },
        }
    }

    fn issuer_row(id: &str, mode: IssuerBetaMode) -> IssuerBetaRow {
        IssuerBetaRow {
            issuer_id: IssuerId::new(id),
            tags: IssuerTags(BTreeMap::new()),
            mode,
            betas: IssuerBetas {
                pc: 1.0,
                levels: vec![1.0, 1.0, 1.0],
            },
            adder_at_anchor: 0.0,
            adder_vol_annualized: 0.01,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
        }
    }

    // ------------------------------------------------------------------
    // PR-plan test 1: round-trip JSON
    // ------------------------------------------------------------------
    #[test]
    fn credit_factor_model_round_trips_json() {
        let model = minimal_model();
        let json = serde_json::to_string(&model).unwrap();
        let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
        // Verify key fields survive the round-trip
        assert_eq!(back.schema_version, CreditFactorModel::SCHEMA_VERSION);
        assert_eq!(back.as_of, model.as_of);
        assert_eq!(back.hierarchy.levels, model.hierarchy.levels);
        assert_eq!(back.issuer_betas.len(), 0);
        // Second serialization must be byte-identical (determinism)
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }

    // ------------------------------------------------------------------
    // PR-plan test 2: reject duplicate issuers
    // ------------------------------------------------------------------
    #[test]
    fn credit_factor_model_rejects_duplicate_issuers() {
        let mut model = minimal_model();
        model
            .issuer_betas
            .push(issuer_row("ISSUER-A", IssuerBetaMode::BucketOnly));
        model
            .issuer_betas
            .push(issuer_row("ISSUER-A", IssuerBetaMode::BucketOnly));
        assert!(model.validate().is_err());
    }

    // ------------------------------------------------------------------
    // PR-plan test 3: custom dimensions serialize deterministically
    // ------------------------------------------------------------------
    #[test]
    fn credit_hierarchy_custom_dimensions_serialize_deterministically() {
        let spec = CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Custom("Currency".to_owned()),
                HierarchyDimension::Custom("AssetType".to_owned()),
            ],
        };
        let json1 = serde_json::to_string(&spec).unwrap();
        let back: CreditHierarchySpec = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json1, json2);
        // Verify the round-tripped spec matches the original
        assert_eq!(back.levels, spec.levels);
    }

    // ------------------------------------------------------------------
    // PR-plan test 4: factor IDs are stable for same hierarchy
    // ------------------------------------------------------------------
    #[test]
    fn credit_factor_ids_are_stable_for_same_hierarchy() {
        // Two models with the same hierarchy spec and same factor IDs in config
        // should produce the same JSON for the config block.
        let make_model = || {
            let factor_id = FactorId::new("credit::generic");
            let factor_def = FactorDefinition {
                id: factor_id.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![finstack_quant_core::types::CurveId::new("CDX.IG")],
                    units: finstack_quant_core::market_data::bumps::BumpUnits::RateBp,
                },
                description: None,
            };
            let covariance = FactorCovarianceMatrix::new(vec![factor_id], vec![0.0001]).unwrap();
            let config = FactorModelConfig {
                factors: vec![factor_def],
                covariance,
                matching: MatchingConfig::MappingTable(vec![]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: Default::default(),
                bump_size: None,
                unmatched_policy: None,
            };
            let mut model = minimal_model();
            model.config = config;
            model
        };

        let json_a = serde_json::to_string(&make_model()).unwrap();
        let json_b = serde_json::to_string(&make_model()).unwrap();
        assert_eq!(json_a, json_b);
    }

    // ------------------------------------------------------------------
    // PR-plan test 5: empty hierarchy is valid
    // ------------------------------------------------------------------
    #[test]
    fn empty_hierarchy_is_valid() {
        let mut model = minimal_model();
        model.hierarchy = CreditHierarchySpec { levels: vec![] };
        assert!(model.validate().is_ok());
        // Round-trip
        let json = serde_json::to_string(&model).unwrap();
        let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
        assert!(back.validate().is_ok());
        assert!(back.hierarchy.levels.is_empty());
    }

    // ------------------------------------------------------------------
    // Additional: schema version mismatch is rejected
    // ------------------------------------------------------------------
    #[test]
    fn validate_rejects_wrong_schema_version() {
        let mut model = minimal_model();
        model.schema_version = "finstack_quant.credit_factor_model/0".to_owned();
        assert!(model.validate().is_err());
    }

    // ------------------------------------------------------------------
    // Additional: duplicate hierarchy dimensions are rejected
    // ------------------------------------------------------------------
    #[test]
    fn validate_rejects_duplicate_hierarchy_dimensions() {
        let mut model = minimal_model();
        model.hierarchy = CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating, HierarchyDimension::Rating],
        };
        assert!(model.validate().is_err());
    }

    // ------------------------------------------------------------------
    // Additional: FactorCorrelationMatrix constructors
    // ------------------------------------------------------------------
    #[test]
    fn factor_correlation_matrix_identity_roundtrips() {
        let fids = vec![FactorId::new("f1"), FactorId::new("f2")];
        let m = FactorCorrelationMatrix::identity(fids.clone());
        assert_eq!(m.data[0][0], 1.0);
        assert_eq!(m.data[0][1], 0.0);
        assert_eq!(m.data[1][0], 0.0);
        assert_eq!(m.data[1][1], 1.0);

        let json = serde_json::to_string(&m).unwrap();
        let back: FactorCorrelationMatrix = serde_json::from_str(&json).unwrap();
        assert_eq!(back.factor_ids, fids);
        assert_eq!(back.data, m.data);
    }

    #[test]
    fn factor_correlation_matrix_rejects_non_unit_diagonal() {
        let fids = vec![FactorId::new("f1")];
        let result = FactorCorrelationMatrix::new(fids, vec![vec![0.9]]);
        assert!(result.is_err());
    }

    #[test]
    fn factor_correlation_matrix_rejects_asymmetric() {
        let fids = vec![FactorId::new("f1"), FactorId::new("f2")];
        let result = FactorCorrelationMatrix::new(fids, vec![vec![1.0, 0.5], vec![0.6, 1.0]]);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Additional: IssuerTags deterministic order
    // ------------------------------------------------------------------
    #[test]
    fn issuer_tags_serialize_in_btree_order() {
        // BTreeMap guarantees alphabetical key order, so "rating" < "region" < "sector"
        let mut tags = IssuerTags(BTreeMap::new());
        tags.0.insert("sector".to_owned(), "FIN".to_owned());
        tags.0.insert("rating".to_owned(), "IG".to_owned());
        tags.0.insert("region".to_owned(), "EU".to_owned());

        let json = serde_json::to_string(&tags).unwrap();
        // Keys must appear in alphabetical order in the serialized JSON
        let rating_pos = json.find("rating").unwrap();
        let region_pos = json.find("region").unwrap();
        let sector_pos = json.find("sector").unwrap();
        assert!(rating_pos < region_pos);
        assert!(region_pos < sector_pos);
    }

    // ------------------------------------------------------------------
    // Additional: FactorHistories length mismatch is rejected by validate
    // ------------------------------------------------------------------
    #[test]
    fn validate_rejects_mismatched_factor_history_lengths() {
        let mut model = minimal_model();
        let mut values = BTreeMap::new();
        values.insert(FactorId::new("credit::generic"), vec![1.0, 2.0, 3.0]);
        model.factor_histories = Some(FactorHistories {
            dates: vec![
                create_date(2024, Month::January, 1).unwrap(),
                create_date(2024, Month::February, 1).unwrap(),
            ],
            values,
        });
        assert!(model.validate().is_err());
    }

    // ------------------------------------------------------------------
    // Additional: Dynamic policy round-trips
    // ------------------------------------------------------------------
    #[test]
    fn dynamic_policy_round_trips_json() {
        let mut overrides = BTreeMap::new();
        overrides.insert(
            IssuerId::new("ISSUER-X"),
            IssuerBetaOverride::ForceIssuerBeta,
        );
        overrides.insert(
            IssuerId::new("ISSUER-Y"),
            IssuerBetaOverride::ForceBucketOnly,
        );
        let policy = IssuerBetaPolicy::Dynamic {
            min_history: 24,
            overrides,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: IssuerBetaPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, back);
    }

    // ------------------------------------------------------------------
    // Fix 1 test: FactorCorrelationMatrix rejects duplicate factor IDs
    // ------------------------------------------------------------------
    #[test]
    fn factor_correlation_matrix_rejects_duplicate_factor_ids() {
        let fid_a = FactorId::new("f1");
        let result =
            FactorCorrelationMatrix::new(vec![fid_a.clone(), fid_a], vec![vec![1.0], vec![1.0]]);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Fix 2 test: validate() rejects a corrupt static_correlation
    // ------------------------------------------------------------------
    #[test]
    fn validate_rejects_corrupt_static_correlation() {
        let mut model = minimal_model();
        // Bypass new() by assigning directly to the public field.
        // This matrix has a non-unit diagonal — structurally invalid.
        model.static_correlation = FactorCorrelationMatrix {
            factor_ids: vec![FactorId::new("f1")],
            data: vec![vec![0.5]], // diagonal != 1.0
        };
        assert!(model.validate().is_err());
    }

    // ------------------------------------------------------------------
    // CreditHierarchySpec::bucket_path — unit tests (Fix A)
    // ------------------------------------------------------------------

    fn tags_rrs(rating: &str, region: &str, sector: &str) -> IssuerTags {
        let mut m = BTreeMap::new();
        m.insert("rating".to_owned(), rating.to_owned());
        m.insert("region".to_owned(), region.to_owned());
        m.insert("sector".to_owned(), sector.to_owned());
        IssuerTags(m)
    }

    fn spec_rating_region_sector() -> CreditHierarchySpec {
        CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Region,
                HierarchyDimension::Sector,
            ],
        }
    }

    #[test]
    fn bucket_path_full_tags_all_levels() {
        let spec = spec_rating_region_sector();
        let tags = tags_rrs("IG", "EU", "FIN");
        // Level 0: just rating value
        assert_eq!(spec.bucket_path(&tags, 0), Some("IG".to_owned()));
        // Level 1: rating.region
        assert_eq!(spec.bucket_path(&tags, 1), Some("IG.EU".to_owned()));
        // Level 2: full path
        assert_eq!(spec.bucket_path(&tags, 2), Some("IG.EU.FIN".to_owned()));
    }

    #[test]
    fn bucket_path_missing_tag_at_level_1_returns_none() {
        let spec = spec_rating_region_sector();
        // Tags has rating and sector, but no region.
        let mut m = BTreeMap::new();
        m.insert("rating".to_owned(), "IG".to_owned());
        m.insert("sector".to_owned(), "FIN".to_owned());
        let tags = IssuerTags(m);
        // Level 0 still works (only needs rating).
        assert_eq!(spec.bucket_path(&tags, 0), Some("IG".to_owned()));
        // Level 1 requires region — returns None.
        assert_eq!(spec.bucket_path(&tags, 1), None);
        // Level 2 also requires region — returns None.
        assert_eq!(spec.bucket_path(&tags, 2), None);
    }

    #[test]
    fn bucket_path_custom_dimension_uses_verbatim_key() {
        let spec = CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Custom("Currency".to_owned()),
            ],
        };
        let mut m = BTreeMap::new();
        m.insert("rating".to_owned(), "HY".to_owned());
        m.insert("Currency".to_owned(), "USD".to_owned()); // exact key used verbatim
        let tags = IssuerTags(m);
        assert_eq!(spec.bucket_path(&tags, 0), Some("HY".to_owned()));
        assert_eq!(spec.bucket_path(&tags, 1), Some("HY.USD".to_owned()));
    }

    #[test]
    fn bucket_path_k_beyond_levels_returns_none() {
        let spec = spec_rating_region_sector();
        let tags = tags_rrs("IG", "EU", "FIN");
        // k == levels.len() is out of bounds.
        assert_eq!(spec.bucket_path(&tags, 3), None);
        assert_eq!(spec.bucket_path(&tags, 99), None);
    }

    #[test]
    fn bucket_path_empty_hierarchy_returns_none() {
        let spec = CreditHierarchySpec { levels: vec![] };
        let tags = tags_rrs("IG", "EU", "FIN");
        // Any k is out of bounds for an empty hierarchy.
        assert_eq!(spec.bucket_path(&tags, 0), None);
    }
}
