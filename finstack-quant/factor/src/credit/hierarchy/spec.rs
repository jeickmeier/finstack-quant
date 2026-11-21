//! Hierarchy dimension specification, issuer tags, policies, and beta rows.

use finstack_quant_core::types::IssuerId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(doc)]
use crate::credit::calibration::BucketWeighting;

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

impl IssuerBetaPolicy {
    /// Variant label of this policy: `"dynamic"` or `"globally_off"`.
    ///
    /// The same string tags the policy in its JSON form, so hosts can report
    /// the policy without reading the per-issuer overrides it may carry.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Dynamic { .. } => "dynamic",
            Self::GloballyOff => "globally_off",
        }
    }
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
    pub(crate) fn write_bucket_path(&self, tags: &IssuerTags, k: usize, out: &mut String) -> bool {
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
    /// dimensions `0..=k` is missing from `tags`.
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
    /// `paths[k]` is the dotted prefix across dimensions `0..=k`.
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
    pub(crate) fn bucket_paths(&self, tags: &IssuerTags) -> Result<Vec<String>, String> {
        let mut paths = Vec::with_capacity(self.levels.len());
        let mut path_buf = String::new();
        for (k, dim) in self.levels.iter().enumerate() {
            let key = dimension_key(dim);
            let Some(value) = tags.0.get(key) else {
                return Err(key.to_owned());
            };
            if k > 0 {
                path_buf.push('.');
            }
            path_buf.push_str(value);
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
    pub(crate) fn unit(num_levels: usize) -> Self {
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
