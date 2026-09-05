//! Credit factor hierarchy artifact types (serde-first data model).
//!
//! This module defines the canonical calibration artifact for the credit
//! factor hierarchy. The central type is
//! [`CreditFactorModel`](crate::factor::credit::hierarchy::CreditFactorModel), a
//! fully self-contained JSON artifact produced by offline calibration and
//! consumed at runtime by attribution, risk, and vol-forecast pipelines.
//!
//! # Schema version
//!
//! [`CreditFactorModelSchema::CURRENT`](crate::factor::credit::hierarchy::CreditFactorModelSchema::CURRENT)
//! is the exact
//! `"finstack_quant.credit_factor_model/1"` marker stored in the model's
//! `schema` field. Consumers must check this field before trusting any other
//! field.
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_models::factor::credit::hierarchy::{
//!     CreditFactorModel, CreditFactorModelSchema,
//! };
//!
//! // Deserialize from JSON, then validate internal consistency.
//! let json = r#"{
//!   "schema": "finstack_quant.credit_factor_model/1",
//!   "as_of": "2024-03-29",
//!   "calibration_window": { "start": "2022-03-29", "end": "2024-03-29" },
//!   "policy": "globally_off",
//!   "generic_factor": { "name": "CDX IG", "series_id": "cdx.ig.5y" },
//!   "hierarchy": { "levels": ["rating", "region", "sector"] },
//!   "panel_frequency": "monthly",
//!   "use_returns_or_levels": "returns",
//!   "bucket_weighting": "equal",
//!   "config": {
//!     "factors": [],
//!     "covariance": { "n": 0, "factor_ids": [], "data": [] },
//!     "matching": { "mapping_table": [] },
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
//! assert_eq!(model.schema, CreditFactorModelSchema::CURRENT);
//! ```
//!
//! # Design notes
//!
//! - Stable artifact structs use `#[serde(deny_unknown_fields)]` to catch schema
//!   drift early. `CalibrationDiagnostics` is the explicitly open extension
//!   object for additive diagnostic fields.
//! - All keyed maps use `BTreeMap` for deterministic serialization order.
//! - `Vec<IssuerBetaRow>` is kept sorted by `issuer_id` so two calibrations on
//!   the same inputs produce byte-identical JSON.

use crate::factor::credit::calibration::{BucketWeighting, PanelFrequency, PanelSpace};
use crate::factor::{FactorId, FactorModelConfig};
use finstack_quant_core::contract::{
    deserialize_json_value, parse_json_value, ContractDescriptor, ContractError, Diagnostic,
    LoadLimits, LoadPhase, Severity, ValidationReport,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Persistence contract for [`CreditFactorModel`].
pub const CREDIT_FACTOR_MODEL_CONTRACT: ContractDescriptor =
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

// dimension_key helper — lives here so CreditHierarchySpec can use it

/// Canonical lowercase key used to read a [`HierarchyDimension`] from a tag map.
///
/// - `Rating` → `"rating"`
/// - `Region` → `"region"`
/// - `Sector` → `"sector"`
/// - `Custom(name)` → `name` (the caller-chosen string, used verbatim).
///
/// # Arguments
///
/// * `dim` - Hierarchy dimension whose canonical tag-map key is required;
///   custom dimensions preserve their configured name exactly. The returned
///   borrow is valid for as long as `dim` is.
#[must_use]
pub fn dimension_key(dim: &HierarchyDimension) -> &str {
    match dim {
        HierarchyDimension::Rating => "rating",
        HierarchyDimension::Region => "region",
        HierarchyDimension::Sector => "sector",
        HierarchyDimension::Custom(name) => name.as_str(),
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
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// Last date of the window (inclusive).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
}

/// Per-issuer regression behavior override supplied by the user before calibration.
///
/// This is the *input* override; the *resolved* outcome is [`IssuerBetaMode`].
///
/// - `Auto` — let the calibration decide based on `min_history`.
/// - `ForceIssuerBeta` — always run per-issuer regression regardless of history.
/// - `ForceBucketOnly` — never run per-issuer regression for this issuer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

/// A single level in the credit factor hierarchy.
///
/// Built-in variants (`Rating`, `Region`, `Sector`) have canonical tag keys.
/// `Custom(key)` reads `issuer_tags[key]` for arbitrary user-defined dimensions
/// such as `"Currency"` or `"AssetType"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditHierarchySpec {
    /// Ordered hierarchy levels, broadest first.
    pub levels: Vec<HierarchyDimension>,
}

impl CreditHierarchySpec {
    /// Write the dotted bucket path for an issuer at hierarchy level `k` into
    /// `out`, reusing that buffer across calls.
    ///
    /// Reads the tag value for each dimension in `self.levels[0..=k]` from
    /// `tags` and joins them with `"."`. `out` is cleared on entry and also
    /// cleared when the write fails.
    ///
    /// Returns `false` if `k >= self.levels.len()` or if any tag for
    /// dimensions `0..=k` is missing from `tags`.
    ///
    /// # Arguments
    ///
    /// * `tags` - Issuer taxonomy whose values become the dotted path
    ///   segments, looked up by each level's [`dimension_key`].
    /// * `k` - Zero-based hierarchy level; the path includes dimensions
    ///   `0..=k`.
    /// * `out` - Destination buffer. Cleared before writing; leftover
    ///   contents are discarded on both success and failure.
    #[must_use]
    pub fn write_bucket_path(&self, tags: &IssuerTags, k: usize, out: &mut String) -> bool {
        out.clear();
        if k >= self.levels.len() {
            return false;
        }
        for (i, dim) in self.levels.iter().take(k + 1).enumerate() {
            let Some(value) = tags.0.get(dimension_key(dim)) else {
                out.clear();
                return false;
            };
            if i > 0 {
                out.push('.');
            }
            out.push_str(value);
        }
        true
    }

    /// Build the dotted bucket path for an issuer at hierarchy level `k`.
    ///
    /// Returns `None` if `k >= self.levels.len()` or if any tag for
    /// dimensions `0..=k` is missing from `tags` (see [`Self::write_bucket_path`]).
    ///
    /// # Arguments
    ///
    /// * `tags` - Issuer taxonomy whose values become the dotted path
    ///   segments, looked up by each level's [`dimension_key`].
    /// * `k` - Zero-based hierarchy level; the path includes dimensions
    ///   `0..=k`.
    #[must_use]
    pub fn bucket_path(&self, tags: &IssuerTags, k: usize) -> Option<String> {
        let mut out = String::new();
        self.write_bucket_path(tags, k, &mut out).then_some(out)
    }

    /// Build the dotted bucket path for an issuer at every hierarchy level.
    ///
    /// `paths[k]` is the path for level `k` (see [`Self::write_bucket_path`]).
    ///
    /// # Arguments
    ///
    /// * `tags` - Issuer taxonomy whose values become the dotted path
    ///   segments, looked up by each level's [`dimension_key`].
    ///
    /// # Errors
    ///
    /// Returns the [`dimension_key`] of the first level whose tag is missing
    /// from `tags`.
    pub fn bucket_paths(&self, tags: &IssuerTags) -> Result<Vec<String>, String> {
        let mut paths = Vec::with_capacity(self.levels.len());
        let mut path_buf = String::new();
        for k in 0..self.levels.len() {
            if !self.write_bucket_path(tags, k, &mut path_buf) {
                let missing = self.levels[..=k]
                    .iter()
                    .find(|d| !tags.0.contains_key(dimension_key(d)))
                    .map_or_else(|| format!("level_{k}"), |d| dimension_key(d).to_owned());
                return Err(missing);
            }
            paths.push(path_buf.clone());
        }
        Ok(paths)
    }
}

/// Flat key-value taxonomy tags for an issuer.
///
/// Uses `BTreeMap` so that serialization is deterministic and two artifacts
/// built from identical inputs produce byte-identical JSON.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct IssuerTags(pub BTreeMap<String, String>);

/// Factor beta loadings for a single issuer.
///
/// `pc` is the loading on the generic (PC) factor.
/// `levels[i]` is the loading on the bucket factor at hierarchy level `i`.
///
/// For `BucketOnly` issuers every component is `1.0` by convention.
///
/// # The `0.0` level-beta sentinel
///
/// `levels[i] == 0.0` marks a level that was **folded** during calibration
/// (the issuer's bucket was below the size threshold). The matcher and
/// `enumerate_factor_ids` skip such levels. A *fitted* beta of exactly `0.0`
/// is indistinguishable from the sentinel, and that is deliberate: every
/// consumer scales by the beta (exposure `= β·CS01`, stress shift `= β·shock`,
/// attribution `= β·ΔL`), so skipping the level and emitting a zero-beta
/// entry produce identical numbers. The degenerate-regressor guard in
/// calibration additionally maps near-zero-information fits to the unit-beta
/// fallback rather than to `0.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IssuerBetas {
    /// Beta on the generic credit PC factor.
    pub pc: f64,
    /// Betas on each hierarchy-level factor, in spec order.
    pub levels: Vec<f64>,
}

impl IssuerBetas {
    /// Unit loadings (`pc = 1.0`, every level `1.0`): the `BucketOnly`
    /// convention for issuers without a calibrated beta row.
    ///
    /// # Arguments
    ///
    /// * `num_levels` - Hierarchy depth; one unit loading per level.
    #[must_use]
    pub fn unit(num_levels: usize) -> Self {
        Self {
            pc: 1.0,
            levels: vec![1.0; num_levels],
        }
    }
}

/// Source provenance of an issuer's idiosyncratic vol estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FitQuality {
    /// In-sample coefficient of determination (R²).
    pub r_squared: f64,
    /// Residual standard deviation of the through-origin peel residual
    /// `y − β x` in basis points of spread move.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IssuerBetaRow {
    /// Unique issuer identifier (e.g. LEI or internal code).
    pub issuer_id: IssuerId,
    /// Taxonomy tags used to assign the issuer to hierarchy buckets.
    pub tags: IssuerTags,
    /// Resolved regression mode for this issuer.
    pub mode: IssuerBetaMode,
    /// Factor beta loadings (all `1.0` for `BucketOnly` issuers).
    ///
    /// For `IssuerBeta` mode, each level loading is the with-intercept OLS
    /// slope of the issuer residual on the **leave-one-out** bucket mean.
    /// Peel and stored factor histories use the **full-bucket** mean, so
    /// the level identity `S_i = β g + Σ β_k L_k + adder` has no drift term.
    pub betas: IssuerBetas,
    /// Value of the issuer's idiosyncratic adder at `as_of` (carry component).
    pub adder_at_anchor: f64,
    /// Annualized idiosyncratic adder volatility (for vol forecasting).
    pub adder_vol_annualized: f64,
    /// Provenance of `adder_vol_annualized`.
    pub adder_vol_source: AdderVolSource,
    /// PC-regression fit statistics; `None` when `mode == BucketOnly`.
    pub fit_quality: Option<FitQuality>,
    /// Per-level regression fit statistics, aligned with `betas.levels`.
    ///
    /// `Some` where a per-level OLS fit ran (`IssuerBeta` mode, level not
    /// folded, regressor not degenerate); `None` otherwise. Empty for
    /// `BucketOnly` rows.
    pub level_fit_quality: Vec<Option<FitQuality>>,
    /// Option-adjusted spread duration in **years** used for DTS weights.
    ///
    /// Calibration persists the caller-supplied duration. Decompose rebuilds
    /// `DTS = spread_duration × current_spread_bp` when the artifact was
    /// calibrated with [`BucketWeighting::Dts`].
    /// Equal-weighted artifacts still store the supplied duration (or `1.0`
    /// when none was given); it is not used at peel time.
    pub spread_duration: f64,
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

/// Static factor correlation matrix `ρ` for the covariance decomposition
/// `Σ(t) = D(t) · ρ · D(t)` where `D(t)` is the diagonal vol matrix.
///
/// `factor_ids` defines the row/column ordering; `data[i][j]` is
/// `ρ_{factor_ids[i], factor_ids[j]}`. The matrix must be square, symmetric,
/// and have unit diagonal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    ///
    /// # Arguments
    ///
    /// * `factor_ids` - Factor identifiers aligned with the exposure or loading columns.
    /// * `data` - Validated market-data payload to insert or transform.
    pub fn new(
        factor_ids: Vec<FactorId>,
        data: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<Self> {
        let matrix = Self { factor_ids, data };
        matrix.check_structure()?;
        Ok(matrix)
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
        // Check symmetry: data[i][j] must equal data[j][i]. Two-dimensional
        // cross-indexing keeps range loops clearer than the clippy suggestion.
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
/// Feeds `Σ(t) = D(t) · ρ · D(t)` and per-issuer idiosyncratic vol forecasts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct VolState {
    /// EWMA or sample vol model for each systematic factor.
    ///
    /// Keys are factor IDs from [`crate::factor::FactorModelConfig`].
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
    #[serde(with = "finstack_quant_core::wire::dates")]
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
/// [`BucketSizeThresholds`][crate::factor::credit::calibration::BucketSizeThresholds]
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
pub struct CalibrationDiagnostics {
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
    /// [`decompose_levels`][crate::factor::credit::decomposition::decompose_levels]
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
/// - [`crate::factor::FactorModelConfig`] respecting its own factor ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditFactorModel {
    /// Exact namespaced v1 schema marker.
    pub schema: CreditFactorModelSchema,
    /// Calibration anchor date (`as_of`).
    #[serde(with = "finstack_quant_core::wire::date")]
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
    /// Static factor correlation matrix `ρ` for `Σ(t) = D(t)·ρ·D(t)`.
    ///
    /// **Which matrix is authoritative:** vol forecasting rebuilds
    /// `Σ(t, h) = D·ρ·D` from this matrix plus `vol_state`; point-in-time
    /// risk uses `config.covariance` directly. Under
    /// [`CovarianceStrategy::Ridge`][crate::factor::credit::calibration::CovarianceStrategy::Ridge]
    /// the two deliberately differ —
    /// `config.covariance = D·ρ·D + α·I`, so its implied correlations are
    /// shrunk relative to `ρ` by `σᵢσⱼ/√((σᵢ²+α)(σⱼ²+α))`.
    ///
    /// Under
    /// [`CovarianceStrategy::LedoitWolf`][crate::factor::credit::calibration::CovarianceStrategy::LedoitWolf]
    /// the divergence is larger still, and affects both the diagonal and the
    /// off-diagonal: `config.covariance` is the shrinkage estimator's own
    /// `periods_per_year · (δ*·μ·I + (1 − δ*)·S)`, computed once over the
    /// complete-case rows (dates where every factor is observed), and is
    /// authoritative for point-in-time risk. The rebuilt `D·ρ·D` instead
    /// combines this same `ρ` with `vol_state` variances — which are
    /// estimated per-factor over all available observations (not just the
    /// complete-case subset) via whichever
    /// [`VolModelChoice`][crate::factor::credit::calibration::VolModelChoice] was
    /// configured (`Sample` or `Ewma`). Because the diagonals come from two
    /// different estimators over two different observation sets, `D·ρ·D`
    /// deliberately differs from `config.covariance` on **both** the
    /// diagonal and the off-diagonal; treat it as an approximation for
    /// horizon scaling, not as a substitute for `config.covariance`.
    pub static_correlation: FactorCorrelationMatrix,
    /// EWMA or sample vol state at the anchor date.
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
        let value = parse_json_value(bytes, limits)?;
        let schema = match value.get("schema") {
            Some(schema) => Some(deserialize_json_value::<String>(schema.clone(), limits)?),
            None => None,
        };
        CREDIT_FACTOR_MODEL_CONTRACT.parse_schema_strict(schema.as_deref(), "/schema", limits)?;
        let model: Self = deserialize_json_value(value, limits)?;
        model.validate().map_err(|error| {
            let mut report = ValidationReport::default();
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "contract/semantic-invalid",
                    LoadPhase::Semantic,
                    Severity::Error,
                    error.to_string(),
                )
                .with_contract(CREDIT_FACTOR_MODEL_CONTRACT.id),
            );
            ContractError::Report(Box::new(report))
        })?;
        Ok((model, ValidationReport::default()))
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
    ///   `config.factors` (see
    ///   [`crate::factor::FactorModelConfig::validate_matching_factor_ids`]) — such a
    ///   factor would silently contribute zero risk at lookup time.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
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

        // Tag values used by hierarchy dimensions must not contain the '.'
        // bucket-path separator (it would mis-segment bucket paths and
        // factor IDs in calibration, fold-up, and matching).
        for row in &self.issuer_betas {
            for dim in &self.hierarchy.levels {
                let key = dimension_key(dim);
                if let Some(v) = row.tags.0.get(key) {
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

        // BucketOnly rows carry β = 1.0 by convention (0.0 only as the
        // folded-level sentinel). A row claiming BucketOnly with
        // fitted-looking betas is contradictory: consumers branching on the
        // mode and consumers reading the betas would disagree about the
        // issuer's loadings.
        for row in &self.issuer_betas {
            if row.mode != IssuerBetaMode::BucketOnly {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor::{
        FactorCovarianceMatrix, FactorDefinition, FactorModelConfig, FactorType, MarketMapping,
        MatchingConfig, PricingMode,
    };
    use finstack_quant_core::dates::create_date;
    use time::Month;

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
            schema: CreditFactorModelSchema::CURRENT,
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
            panel_frequency: PanelFrequency::Monthly,
            use_returns_or_levels: PanelSpace::Returns,
            bucket_weighting: BucketWeighting::Equal,
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
            level_fit_quality: vec![],
            spread_duration: 1.0,
        }
    }

    #[test]
    fn credit_factor_model_round_trips_json() {
        let model = minimal_model();
        let json = serde_json::to_string(&model).unwrap();
        let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema, CreditFactorModelSchema::CreditFactorModel);
        assert_eq!(back.as_of, model.as_of);
        assert_eq!(back.hierarchy.levels, model.hierarchy.levels);
        assert_eq!(back.issuer_betas.len(), 0);
        // Second serialization must be byte-identical (determinism)
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn strict_loader_fuses_schema_deserialization_and_validation() {
        let model = minimal_model();
        let bytes = serde_json::to_vec(&model).expect("serialize model");
        let (loaded, report) = CreditFactorModel::from_slice_strict(
            &bytes,
            &finstack_quant_core::LoadLimits::default(),
        )
        .expect("valid model");
        assert_eq!(loaded.schema, CreditFactorModelSchema::CreditFactorModel);
        assert!(report.diagnostics.is_empty());

        let base = serde_json::to_value(model).expect("serialize fixture");
        for schema in [
            None,
            Some("finstack_quant.credit_factor_model/0"),
            Some("finstack_quant.credit_factor_model/2"),
            Some("finstack_quant.credit_factor_model/not-a-version"),
        ] {
            let mut value = base.clone();
            match schema {
                Some(schema) => value["schema"] = serde_json::json!(schema),
                None => {
                    value
                        .as_object_mut()
                        .expect("model object")
                        .remove("schema");
                }
            }
            assert!(
                CreditFactorModel::from_slice_strict(
                    &serde_json::to_vec(&value).expect("serialize fixture"),
                    &finstack_quant_core::LoadLimits::default(),
                )
                .is_err(),
                "invalid schema must fail"
            );
        }

        let mut invalid = base;
        invalid["hierarchy"]["levels"] = serde_json::json!(["rating", "rating"]);
        assert!(
            CreditFactorModel::from_slice_strict(
                &serde_json::to_vec(&invalid).expect("serialize fixture"),
                &finstack_quant_core::LoadLimits::default(),
            )
            .is_err(),
            "semantic validation must run"
        );
    }

    // INVARIANTS.md §8 contract: the root artifact is closed
    // (`deny_unknown_fields`), so an unknown root key must FAIL to
    // deserialize; `CalibrationDiagnostics` is an open extension point, so
    // an unknown diagnostics key must deserialize successfully. Adding a
    // root key therefore requires a coordinated v1 contract change.
    #[test]
    fn unknown_root_key_is_rejected_but_diagnostics_extension_is_accepted() {
        let model = minimal_model();
        let json = serde_json::to_string(&model).unwrap();

        // Unknown root key: closed root must reject (forward-compat break).
        let mut root = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        root.as_object_mut()
            .unwrap()
            .insert("future_root_field".to_owned(), serde_json::json!(42));
        let with_root_key = serde_json::to_string(&root).unwrap();
        assert!(
            serde_json::from_str::<CreditFactorModel>(&with_root_key).is_err(),
            "unknown root key must fail: root uses deny_unknown_fields, so \
             additive root fields require a new schema version"
        );

        // Unknown diagnostics key: open extension point must accept.
        let mut diag = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        diag.as_object_mut()
            .unwrap()
            .get_mut("diagnostics")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future_diagnostic".to_owned(), serde_json::json!("extra"));
        let with_diag_key = serde_json::to_string(&diag).unwrap();
        let parsed = serde_json::from_str::<CreditFactorModel>(&with_diag_key);
        assert!(
            parsed.is_ok(),
            "unknown CalibrationDiagnostics key must deserialize: diagnostics \
             is a declared open extension point (no deny_unknown_fields)"
        );
    }

    #[test]
    fn wrong_schema_marker_is_rejected_during_deserialization() {
        let mut value = serde_json::to_value(minimal_model()).expect("serialize model");
        value["schema"] = serde_json::json!("finstack_quant.credit_factor_model/999");
        serde_json::from_value::<CreditFactorModel>(value)
            .expect_err("unsupported schema marker must fail during deserialization");
    }

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
        assert_eq!(back.levels, spec.levels);
    }

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

    #[test]
    fn empty_hierarchy_is_valid() {
        let mut model = minimal_model();
        model.hierarchy = CreditHierarchySpec { levels: vec![] };
        assert!(model.validate().is_ok());
        let json = serde_json::to_string(&model).unwrap();
        let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
        assert!(back.validate().is_ok());
        assert!(back.hierarchy.levels.is_empty());
    }

    // Additional: duplicate hierarchy dimensions are rejected
    #[test]
    fn validate_rejects_duplicate_hierarchy_dimensions() {
        let mut model = minimal_model();
        model.hierarchy = CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating, HierarchyDimension::Rating],
        };
        assert!(model.validate().is_err());
    }

    // Additional: FactorCorrelationMatrix constructors
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

    // Additional: IssuerTags deterministic order
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

    // Additional: FactorHistories length mismatch is rejected by validate
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

    // Additional: Dynamic policy round-trips
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

    // Fix 1 test: FactorCorrelationMatrix rejects duplicate factor IDs
    #[test]
    fn factor_correlation_matrix_rejects_duplicate_factor_ids() {
        let fid_a = FactorId::new("f1");
        let result =
            FactorCorrelationMatrix::new(vec![fid_a.clone(), fid_a], vec![vec![1.0], vec![1.0]]);
        assert!(result.is_err());
    }

    // Fix 2 test: validate() rejects a corrupt static_correlation
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

    // CreditHierarchySpec::bucket_path — unit tests (Fix A)

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
    #[test]
    fn validate_rejects_covariance_factor_id_mismatch() {
        use crate::factor::{FactorDefinition, FactorType, MarketMapping};
        use finstack_quant_core::market_data::bumps::BumpUnits;

        let declared = FactorId::new("credit::generic");
        let uncovered = FactorId::new("credit::level0::Rating::IG");
        let mut model = minimal_model();
        model.config.factors = [&declared, &uncovered]
            .into_iter()
            .map(|id| FactorDefinition {
                id: id.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            })
            .collect();
        // Covariance only covers one of the two declared factors; the other
        // would silently contribute zero risk at lookup time.
        model.config.covariance =
            FactorCovarianceMatrix::new(vec![declared.clone()], vec![400.0]).unwrap();
        model.static_correlation = FactorCorrelationMatrix::identity(vec![declared.clone()]);
        model
            .vol_state
            .factors
            .insert(declared, FactorVolModel::Sample { variance: 400.0 });

        let err = model
            .validate()
            .expect_err("covariance missing a declared factor must be rejected");
        assert!(
            err.to_string().contains("credit::level0::Rating::IG"),
            "error must name the uncovered factor: {err}"
        );
    }

    #[test]
    fn validate_rejects_static_correlation_and_vol_state_mismatch() {
        use crate::factor::{FactorDefinition, FactorType, MarketMapping};
        use finstack_quant_core::market_data::bumps::BumpUnits;

        let declared = FactorId::new("credit::generic");
        let mut model = minimal_model();
        model.config.factors = vec![FactorDefinition {
            id: declared.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];
        model.config.covariance =
            FactorCovarianceMatrix::new(vec![declared.clone()], vec![400.0]).unwrap();

        // Correlation matrix over a disjoint id set.
        model.static_correlation =
            FactorCorrelationMatrix::identity(vec![FactorId::new("unrelated")]);
        model
            .vol_state
            .factors
            .insert(declared.clone(), FactorVolModel::Sample { variance: 400.0 });
        assert!(
            model.validate().is_err(),
            "static_correlation over a different factor-id set must be rejected"
        );

        // Fix the correlation; break vol_state instead.
        model.static_correlation = FactorCorrelationMatrix::identity(vec![declared.clone()]);
        model.vol_state.factors.clear();
        model.vol_state.factors.insert(
            FactorId::new("unrelated"),
            FactorVolModel::Sample { variance: 1.0 },
        );
        assert!(
            model.validate().is_err(),
            "vol_state keyed by a different factor-id set must be rejected"
        );

        // Aligned everywhere: validate() must pass.
        model.vol_state.factors.clear();
        model
            .vol_state
            .factors
            .insert(declared, FactorVolModel::Sample { variance: 400.0 });
        assert!(model.validate().is_ok(), "aligned model must validate");
    }

    #[test]
    fn validate_rejects_builtin_and_custom_dimension_key_collision() {
        let mut model = minimal_model();
        // `Rating` and `Custom("rating")` read the SAME tag key at runtime
        // (`dimension_key` maps both to "rating"), so the two levels are the
        // same information counted twice.
        model.hierarchy = CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Custom("rating".to_owned()),
            ],
        };
        let err = model
            .validate()
            .expect_err("colliding dimension keys must be rejected");
        assert!(
            err.to_string().contains("rating"),
            "error must name the colliding key: {err}"
        );
    }
    /// A `BucketOnly` row's betas are `1.0` by convention (`0.0` only as the
    /// folded-level sentinel). A hand-edited row claiming `BucketOnly` with
    /// fitted-looking betas is contradictory: consumers that branch on the
    /// mode and consumers that read the betas would disagree.
    #[test]
    fn validate_rejects_bucket_only_row_with_non_unit_betas() {
        let mut model = minimal_model();
        let mut row = issuer_row("ACME", IssuerBetaMode::BucketOnly);
        row.betas.pc = 5.0;
        model.issuer_betas = vec![row];
        let err = model
            .validate()
            .expect_err("BucketOnly with non-unit pc beta must be rejected");
        assert!(
            err.to_string().contains("ACME"),
            "error must name the issuer: {err}"
        );

        // Folded levels (0.0) and unit betas are the legitimate shapes.
        let mut ok_row = issuer_row("ACME", IssuerBetaMode::BucketOnly);
        ok_row.betas.levels = vec![1.0, 0.0, 1.0];
        model.issuer_betas = vec![ok_row];
        assert!(model.validate().is_ok(), "unit/folded betas must validate");
    }
}
