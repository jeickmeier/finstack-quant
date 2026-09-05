//! Calibrated credit-hierarchy matcher.
//!
//! Maps a [`MarketDependency::CreditCurve`] (or `Curve` with hazard role) plus
//! issuer tags into the canonical list of credit factors:
//!
//! - `credit::generic` (the PC factor) with the issuer's calibrated `pc` beta;
//! - `credit::level{idx}::{dim_path}::{val_path}` for each hierarchy level the
//!   issuer is tagged for, with its calibrated `levels[idx]` beta.
//!
//! Unknown issuers (no row in `issuer_betas`) are treated as `BucketOnly`:
//! every emitted factor carries beta = 1.0. An unknown issuer with **no**
//! hierarchy tags at all maps to the PC factor only (the documented
//! index-proxy fallback); an unknown issuer with a *partial* tag set is a
//! contract violation and returns
//! [`FactorMatchError::MissingRequiredTag`] — silently truncating the
//! hierarchy would under-map specific credit risk. Known issuers must carry a
//! complete tag set and a beta vector matching the hierarchy depth
//! ([`FactorMatchError::BetaShapeMismatch`] otherwise). Entries with the
//! folded-level sentinel β = 0.0 are not emitted.
//!
//! The matcher delegates the dependency-side gating to the existing
//! [`DependencyFilter`]; it does not duplicate the tree-walking semantics of
//! [`super::HierarchicalMatcher`]. Factor identities are computed
//! deterministically from the calibrated [`CreditHierarchySpec`] and issuer
//! tags rather than enumerated as nodes in a tree.

use super::filter::DependencyFilter;
use super::matchers::{FactorMatchEntry, FactorMatchError, FactorMatcher};
use crate::factor::credit::hierarchy::{
    dimension_key, CreditHierarchySpec, HierarchyDimension, IssuerBetaRow, IssuerTags,
};
use crate::factor::primitives::dependency::MarketDependency;
use crate::factor::primitives::factor_types::FactorId;
use finstack_quant_core::types::{Attributes, IssuerId};
use serde::{Deserialize, Serialize};

/// Reserved key in [`Attributes::meta`] used to thread the issuer identifier
/// from the position into the matcher.
///
/// Set this key on the instrument's [`Attributes`] before calling the matcher.
/// If the key is absent the issuer is treated as unknown (`BucketOnly`).
pub const ISSUER_ID_META_KEY: &str = "credit::issuer_id";

/// Canonical factor ID for the generic credit (PC) factor.
pub const CREDIT_GENERIC_FACTOR_ID: &str = "credit::generic";

/// Declarative configuration for a calibrated credit-hierarchy matcher.
///
/// The matcher emits PC + per-level credit factors with calibrated betas
/// looked up from `issuer_betas`. `issuer_betas` must be sorted by
/// `issuer_id` (binary search is used). `hierarchy` defines the level
/// ordering and dimension keys used to build factor IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditHierarchicalConfig {
    /// Dependency filter; defaults to "any credit-curve dependency".
    #[serde(default)]
    pub dependency_filter: DependencyFilter,
    /// Hierarchy specification (level ordering and dimension keys).
    pub hierarchy: CreditHierarchySpec,
    /// Issuer beta rows, sorted by `issuer_id`.
    #[serde(default)]
    pub issuer_betas: Vec<IssuerBetaRow>,
    /// Require the [`ISSUER_ID_META_KEY`] meta key on every credit dependency.
    ///
    /// When `true`, a credit dependency whose attributes omit the issuer id
    /// is rejected with [`FactorMatchError::MissingRequiredTag`] instead of
    /// being silently downgraded to the PC-only proxy — an absent key is
    /// usually a data-plumbing failure, and the proxy fallback drops both
    /// hierarchy exposure and idiosyncratic risk. Calibrated artifacts set
    /// this to `true`; hand-built configs default to `false` (`serde`
    /// default) so index-proxy workflows without issuer identities keep
    /// working.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_issuer_id: bool,
}

/// `skip_serializing_if` helper keeping pre-existing artifacts byte-stable.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

impl CreditHierarchicalConfig {
    /// Returns the deterministic list of factor IDs this config can emit.
    ///
    /// The list is the union of `credit::generic` and every
    /// `credit::level{idx}::{dim_path}::{val_path}` that appears for any
    /// known issuer in `issuer_betas`. The list is deduplicated and sorted
    /// for stable output.
    ///
    /// # Limitations
    ///
    /// This method only enumerates factor IDs for issuers known to the calibrated
    /// `issuer_betas`. If a runtime issuer with full tags is treated as `BucketOnly`,
    /// its bucket factor IDs are not checked here.
    #[must_use]
    pub fn enumerate_factor_ids(&self) -> Vec<FactorId> {
        use std::collections::BTreeSet;
        let mut ids: BTreeSet<FactorId> = BTreeSet::new();
        ids.insert(FactorId::new(CREDIT_GENERIC_FACTOR_ID));
        let dim_paths: Vec<String> = (0..self.hierarchy.levels.len())
            .map(|k| dimension_path(&self.hierarchy, k))
            .collect();
        let mut path_buf = String::new();
        let mut seen: BTreeSet<(usize, String)> = BTreeSet::new();
        for row in &self.issuer_betas {
            for level_idx in 0..self.hierarchy.levels.len() {
                // β = 0.0 is the calibration sentinel for "level folded /
                // skipped"; folded buckets have no factor definition or
                // covariance row, so they are not enumerated (and the matcher
                // emits no entry for them either).
                if row.betas.levels.get(level_idx).copied().unwrap_or(0.0) == 0.0 {
                    continue;
                }
                if !self
                    .hierarchy
                    .write_bucket_path(&row.tags, level_idx, &mut path_buf)
                {
                    continue;
                }
                if !seen.insert((level_idx, path_buf.clone())) {
                    continue;
                }
                if let Some(dim_path) = dim_paths.get(level_idx) {
                    ids.insert(format_bucket_factor_id(level_idx, dim_path, &path_buf));
                }
            }
        }
        ids.into_iter().collect()
    }
}

/// Per-issuer factor IDs precomputed from calibrated tags and betas.
#[derive(Debug, Clone)]
struct PreparedIssuer {
    /// One slot per hierarchy level. `None` means the level is folded (β = 0)
    /// or the calibrated tags were incomplete.
    level_ids: Vec<Option<FactorId>>,
}

/// Calibrated credit-hierarchy matcher.
///
/// See the module-level docs for the semantics.
#[derive(Debug, Clone)]
pub struct CreditHierarchicalMatcher {
    config: CreditHierarchicalConfig,
    generic_id: FactorId,
    prepared: Vec<PreparedIssuer>,
    dim_paths: Vec<String>,
}

impl CreditHierarchicalMatcher {
    /// Creates a matcher from a calibrated config.
    ///
    /// `issuer_betas` is re-sorted by `issuer_id` defensively: row lookup uses
    /// binary search, and an unsorted vector (e.g. from a hand-assembled
    /// config) would otherwise silently miss calibrated rows and substitute
    /// β = 1.0 for every factor.
    ///
    /// # Arguments
    ///
    /// * `config` - Calibrated hierarchy, issuer beta rows, and dependency
    ///   filter. Rows are sorted in place by issuer id before the matcher
    ///   caches per-row factor identifiers.
    #[must_use]
    pub fn new(mut config: CreditHierarchicalConfig) -> Self {
        config
            .issuer_betas
            .sort_by(|a, b| a.issuer_id.as_str().cmp(b.issuer_id.as_str()));
        let dim_paths: Vec<String> = (0..config.hierarchy.levels.len())
            .map(|k| dimension_path(&config.hierarchy, k))
            .collect();
        let prepared = config
            .issuer_betas
            .iter()
            .map(|row| prepare_issuer(&config.hierarchy, row))
            .collect();
        Self {
            config,
            generic_id: FactorId::new(CREDIT_GENERIC_FACTOR_ID),
            prepared,
            dim_paths,
        }
    }

    fn lookup_row(&self, issuer_id: &IssuerId) -> Option<(usize, &IssuerBetaRow)> {
        self.config
            .issuer_betas
            .binary_search_by(|row| row.issuer_id.as_str().cmp(issuer_id.as_str()))
            .ok()
            .and_then(|idx| self.config.issuer_betas.get(idx).map(|row| (idx, row)))
    }
}

fn prepare_issuer(spec: &CreditHierarchySpec, row: &IssuerBetaRow) -> PreparedIssuer {
    let mut level_ids = Vec::with_capacity(spec.levels.len());
    for level_idx in 0..spec.levels.len() {
        let beta = row.betas.levels.get(level_idx).copied().unwrap_or(0.0);
        if beta == 0.0 {
            level_ids.push(None);
            continue;
        }
        level_ids.push(bucket_factor_id(spec, &row.tags, level_idx));
    }
    PreparedIssuer { level_ids }
}

impl FactorMatcher for CreditHierarchicalMatcher {
    fn match_factor_with_betas(
        &self,
        dependency: &MarketDependency,
        attributes: &Attributes,
    ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError> {
        if !self.config.dependency_filter.matches(dependency) {
            return Ok(None);
        }
        if !is_credit_dependency(dependency) {
            return Ok(None);
        }

        let issuer_id_str = attributes.get_meta(ISSUER_ID_META_KEY);
        if self.config.require_issuer_id && issuer_id_str.is_none() {
            // A credit dependency with no issuer identity is a data-plumbing
            // gap when the config demands one (calibrated artifacts do):
            // silently proxying to PC-only would drop hierarchy exposure and
            // idiosyncratic risk without any signal.
            return Err(FactorMatchError::MissingRequiredTag {
                dimension: ISSUER_ID_META_KEY.to_owned(),
            });
        }

        // Look up calibrated betas if available; otherwise fall back to 1.0.
        let row = issuer_id_str
            .map(IssuerId::new)
            .as_ref()
            .and_then(|id| self.lookup_row(id));

        // A calibrated row whose beta vector disagrees with the hierarchy
        // depth is an inconsistent config; substituting β = 1.0 silently
        // would misstate risk.
        if let Some((_, r)) = row {
            if r.betas.levels.len() != self.config.hierarchy.levels.len() {
                return Err(FactorMatchError::BetaShapeMismatch {
                    issuer_id: r.issuer_id.as_str().to_owned(),
                    actual: r.betas.levels.len(),
                    expected: self.config.hierarchy.levels.len(),
                });
            }
        }

        // Emit PC factor first.
        let mut entries = Vec::with_capacity(1 + self.config.hierarchy.levels.len());
        let pc_beta = row.map_or(1.0, |(_, r)| r.betas.pc);
        entries.push(FactorMatchEntry {
            factor_id: self.generic_id.clone(),
            beta: pc_beta,
        });

        if let Some((idx, r)) = row {
            for dim in &self.config.hierarchy.levels {
                if !r.tags.0.contains_key(dimension_key(dim)) {
                    return Err(FactorMatchError::MissingRequiredTag {
                        dimension: dimension_key(dim).to_owned(),
                    });
                }
            }
            let Some(prepared) = self.prepared.get(idx) else {
                return Err(FactorMatchError::BetaShapeMismatch {
                    issuer_id: r.issuer_id.as_str().to_owned(),
                    actual: r.betas.levels.len(),
                    expected: self.config.hierarchy.levels.len(),
                });
            };
            for (level_idx, cached) in prepared.level_ids.iter().enumerate() {
                let beta = r.betas.levels.get(level_idx).copied().unwrap_or(0.0);
                if beta == 0.0 {
                    continue;
                }
                let Some(factor_id) = cached.clone() else {
                    let dimension = self
                        .config
                        .hierarchy
                        .levels
                        .get(level_idx)
                        .map(dimension_key)
                        .unwrap_or("")
                        .to_owned();
                    return Err(FactorMatchError::MissingRequiredTag { dimension });
                };
                entries.push(FactorMatchEntry { factor_id, beta });
            }
            return Ok(Some(entries));
        }

        // Unknown issuers: tags come from instrument attributes. A complete
        // absence of hierarchy tags is the documented PC-only proxy fallback;
        // a *partial* tag set is treated as a contract violation just like a
        // known issuer's missing tag — silently truncating the hierarchy
        // would under-map specific credit risk without any signal a strict
        // unmatched policy could observe.
        let tags = tags_from_attributes(&self.config.hierarchy, attributes)?;
        let any_hierarchy_tag_present = self
            .config
            .hierarchy
            .levels
            .iter()
            .any(|dim| tags.0.contains_key(dimension_key(dim)));

        let mut val_path = String::new();
        for (level_idx, dim) in self.config.hierarchy.levels.iter().enumerate() {
            if !tags.0.contains_key(dimension_key(dim)) {
                if any_hierarchy_tag_present {
                    return Err(FactorMatchError::MissingRequiredTag {
                        dimension: dimension_key(dim).to_owned(),
                    });
                }
                break;
            }

            if !self
                .config
                .hierarchy
                .write_bucket_path(&tags, level_idx, &mut val_path)
            {
                return Err(FactorMatchError::MissingRequiredTag {
                    dimension: dimension_key(dim).to_owned(),
                });
            }
            let dim_path = self.dim_paths.get(level_idx).map_or("", String::as_str);
            entries.push(FactorMatchEntry {
                factor_id: format_bucket_factor_id(level_idx, dim_path, &val_path),
                beta: 1.0,
            });
        }

        Ok(Some(entries))
    }
}

/// Dotted dimension-name path through the first `level_idx + 1` levels of
/// the hierarchy spec, e.g. `"Rating.Region"` for level index 1.
fn dimension_path(spec: &CreditHierarchySpec, level_idx: usize) -> String {
    let mut out = String::new();
    write_dimension_path(spec, level_idx, &mut out);
    out
}

fn write_dimension_path(spec: &CreditHierarchySpec, level_idx: usize, out: &mut String) {
    out.clear();
    for (i, dim) in spec.levels.iter().take(level_idx + 1).enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(match dim {
            HierarchyDimension::Rating => "Rating",
            HierarchyDimension::Region => "Region",
            HierarchyDimension::Sector => "Sector",
            HierarchyDimension::Custom(name) => name.as_str(),
        });
    }
}

fn format_bucket_factor_id(level_idx: usize, dim_path: &str, val_path: &str) -> FactorId {
    FactorId::new(format!("credit::level{level_idx}::{dim_path}::{val_path}"))
}

/// Builds the canonical factor ID `credit::level{idx}::{dim_path}::{val_path}`
/// for the given hierarchy level. Returns `None` if any required tag is missing.
///
/// # Arguments
///
/// * `spec` - Credit hierarchy that determines the ordered dimensions and
///   canonical dimension names in the factor identifier.
/// * `tags` - Issuer tag map containing a value for every dimension through
///   `level_idx`; a missing value returns `None`.
/// * `level_idx` - Zero-based hierarchy level to encode; an index outside the
///   specification returns `None`.
#[must_use]
pub fn bucket_factor_id(
    spec: &CreditHierarchySpec,
    tags: &IssuerTags,
    level_idx: usize,
) -> Option<FactorId> {
    if level_idx >= spec.levels.len() {
        return None;
    }
    let mut dim_path = String::new();
    write_dimension_path(spec, level_idx, &mut dim_path);
    let mut val_path = String::new();
    if !spec.write_bucket_path(tags, level_idx, &mut val_path) {
        return None;
    }
    Some(format_bucket_factor_id(level_idx, &dim_path, &val_path))
}

/// Whether a [`MarketDependency`] is a credit/hazard one. The matcher only
/// emits factors for credit-side dependencies regardless of how the user
/// configured `dependency_filter`.
fn is_credit_dependency(dep: &MarketDependency) -> bool {
    use crate::factor::primitives::dependency::CurveType;
    match dep {
        MarketDependency::CreditCurve { .. } | MarketDependency::CreditIndex { .. } => true,
        MarketDependency::Curve { curve_type, .. } => *curve_type == CurveType::Hazard,
        _ => false,
    }
}

/// Meta key under which a runtime credit-hierarchy tag is read from
/// [`Attributes::meta`]: `credit::<dimension_key>` (e.g. `credit::rating`).
///
/// Namespaced like [`ISSUER_ID_META_KEY`] so that generic instrument metadata
/// using bare keys such as `"rating"` (also consumed by `AttributeFilter`)
/// is never silently reinterpreted as a credit-hierarchy tag.
fn credit_tag_meta_key(dim: &HierarchyDimension) -> String {
    format!("credit::{}", dimension_key(dim))
}

/// Build an [`IssuerTags`] view from `attributes.meta` using the namespaced
/// [`credit_tag_meta_key`] convention.
///
/// Used as a fallback for unknown issuers (no calibrated row). Tag values
/// containing `'.'` are rejected: the dot is the bucket-path separator, and a
/// dotted runtime value would mis-segment bucket paths and factor IDs
/// (calibration enforces the same rule for calibrated issuers).
fn tags_from_attributes(
    spec: &CreditHierarchySpec,
    attrs: &Attributes,
) -> Result<IssuerTags, FactorMatchError> {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    for dim in &spec.levels {
        let key = dimension_key(dim);
        if let Some(v) = attrs.get_meta(&credit_tag_meta_key(dim)) {
            if v.contains('.') {
                return Err(FactorMatchError::InvalidTagValue {
                    dimension: key.to_owned(),
                    value: v.to_owned(),
                });
            }
            map.insert(key.to_owned(), v.to_owned());
        }
    }
    Ok(IssuerTags(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor::credit::hierarchy::{
        AdderVolSource, IssuerBetaMode, IssuerBetaRow, IssuerBetas, IssuerTags,
    };
    use crate::factor::primitives::dependency::{DependencyType, MarketDependency};
    use finstack_quant_core::types::{Attributes, CurveId, IssuerId};
    use std::collections::BTreeMap;

    fn three_level_spec() -> CreditHierarchySpec {
        CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Region,
                HierarchyDimension::Sector,
            ],
        }
    }

    fn issuer_row(
        id: &str,
        pc: f64,
        levels: Vec<f64>,
        tags: BTreeMap<String, String>,
    ) -> IssuerBetaRow {
        IssuerBetaRow {
            issuer_id: IssuerId::new(id),
            tags: IssuerTags(tags),
            mode: IssuerBetaMode::IssuerBeta,
            betas: IssuerBetas { pc, levels },
            adder_at_anchor: 0.0,
            adder_vol_annualized: 0.01,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
            level_fit_quality: vec![],
            spread_duration: 1.0,
        }
    }

    fn three_level_tags() -> BTreeMap<String, String> {
        let mut tags = BTreeMap::new();
        tags.insert("rating".to_owned(), "IG".to_owned());
        tags.insert("region".to_owned(), "EU".to_owned());
        tags.insert("sector".to_owned(), "FIN".to_owned());
        tags
    }

    fn matcher_with_one_issuer() -> CreditHierarchicalMatcher {
        let row = issuer_row("ISSUER-A", 0.9, vec![0.85, 0.8, 0.75], three_level_tags());
        CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
            require_issuer_id: false,
        })
    }

    /// Runtime tags for unknown issuers must be read from the namespaced
    /// `credit::<dimension>` meta keys, not bare keys: bare `"rating"` /
    /// `"region"` / `"sector"` are generic instrument metadata (also consumed
    /// by `AttributeFilter`) and silently reinterpreting them as
    /// credit-hierarchy tags mis-buckets unrelated instruments.
    #[test]
    fn runtime_tags_use_namespaced_meta_keys() {
        let matcher = matcher_with_one_issuer();
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("NEWCO-HAZARD"),
        };

        // Namespaced keys: full hierarchy is emitted with unit betas.
        let attrs = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "NEWCO")
            .with_meta("credit::rating", "HY")
            .with_meta("credit::region", "NA")
            .with_meta("credit::sector", "TECH");
        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect("must succeed")
            .expect("must match");
        assert_eq!(entries.len(), 4, "PC + 3 levels from namespaced tags");
        assert_eq!(
            entries[1].factor_id,
            FactorId::new("credit::level0::Rating::HY")
        );

        // Bare keys are ignored: PC-only proxy fallback.
        let bare = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "NEWCO")
            .with_meta("rating", "HY")
            .with_meta("region", "NA")
            .with_meta("sector", "TECH");
        let entries = matcher
            .match_factor_with_betas(&dep, &bare)
            .expect("must succeed")
            .expect("must match");
        assert_eq!(
            entries.len(),
            1,
            "bare meta keys must not be read as credit tags"
        );
    }

    /// A runtime tag value containing '.' would mis-segment dotted bucket
    /// paths and factor IDs (`HY.EU` vs `HY`, `EU`); calibration rejects such
    /// values for calibrated issuers, and the matcher must reject them for
    /// runtime issuers instead of silently corrupting factor identity.
    #[test]
    fn runtime_tags_with_dotted_values_are_rejected() {
        let matcher = matcher_with_one_issuer();
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("NEWCO-HAZARD"),
        };
        let attrs = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "NEWCO")
            .with_meta("credit::rating", "A.BBB")
            .with_meta("credit::region", "NA")
            .with_meta("credit::sector", "TECH");
        let err = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect_err("dotted runtime tag value must be rejected");
        assert!(
            err.to_string().contains("A.BBB"),
            "error must show the offending value: {err}"
        );
    }

    /// With `require_issuer_id` set (the calibrated-artifact default), a
    /// credit dependency whose attributes omit `credit::issuer_id` is a
    /// data-plumbing failure, not a modelling choice: the matcher must fail
    /// instead of silently downgrading the position to the PC-only proxy.
    #[test]
    fn require_issuer_id_rejects_missing_issuer_meta() {
        let row = issuer_row("ISSUER-A", 0.9, vec![0.85, 0.8, 0.75], three_level_tags());
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
            require_issuer_id: true,
        });
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-A-HAZARD"),
        };

        let err = matcher
            .match_factor_with_betas(&dep, &Attributes::default())
            .expect_err("missing issuer id meta must be rejected when required");
        assert!(
            err.to_string().contains(ISSUER_ID_META_KEY),
            "error must name the required meta key: {err}"
        );

        // With the issuer id present the matcher works normally.
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-A");
        assert!(matcher.match_factor_with_betas(&dep, &attrs).is_ok());
    }

    #[test]
    fn credit_hierarchical_matcher_returns_generic_and_bucket_factors() {
        let matcher = matcher_with_one_issuer();
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-A-HAZARD"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-A");

        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect("must succeed")
            .expect("must match");

        assert_eq!(entries.len(), 4, "PC + 3 levels");
        assert_eq!(
            entries[0].factor_id,
            FactorId::new("credit::generic"),
            "PC factor must be first"
        );
        assert!((entries[0].beta - 0.9).abs() < 1e-12);

        assert_eq!(
            entries[1].factor_id,
            FactorId::new("credit::level0::Rating::IG")
        );
        assert!((entries[1].beta - 0.85).abs() < 1e-12);

        assert_eq!(
            entries[2].factor_id,
            FactorId::new("credit::level1::Rating.Region::IG.EU")
        );
        assert!((entries[2].beta - 0.8).abs() < 1e-12);

        assert_eq!(
            entries[3].factor_id,
            FactorId::new("credit::level2::Rating.Region.Sector::IG.EU.FIN")
        );
        assert!((entries[3].beta - 0.75).abs() < 1e-12);
    }

    #[test]
    fn credit_hierarchical_matcher_errors_on_missing_required_tag() {
        let mut tags = three_level_tags();
        tags.remove("region"); // Known issuer, but tagged for only level 0.
        let row = issuer_row("ISSUER-MISSING", 1.0, vec![1.0, 1.0, 1.0], tags);
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
            require_issuer_id: false,
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-MISSING-HAZARD"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-MISSING");

        let err = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect_err("missing region tag must be reported as error");
        match err {
            FactorMatchError::MissingRequiredTag { dimension } => {
                assert_eq!(dimension, "region");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    // unsorted issuer_betas must not break lookup
    #[test]
    fn matcher_resorts_unsorted_issuer_betas_before_binary_search() {
        // Rows deliberately supplied in reverse order; binary search over the
        // raw vector would miss "ISSUER-A" and silently fall back to β = 1.0.
        let row_a = issuer_row("ISSUER-A", 0.9, vec![0.85, 0.8, 0.75], three_level_tags());
        let row_z = issuer_row("ISSUER-Z", 1.1, vec![1.0, 1.0, 1.0], three_level_tags());
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row_z, row_a],
            require_issuer_id: false,
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-A-HAZARD"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-A");
        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect("must succeed")
            .expect("must match");
        assert!(
            (entries[0].beta - 0.9).abs() < 1e-12,
            "calibrated pc beta must be found despite unsorted input; got {}",
            entries[0].beta
        );
    }

    // short beta vector is a typed error, not β = 1.0
    #[test]
    fn matcher_errors_on_beta_shape_mismatch() {
        // Two betas for a three-level hierarchy.
        let row = issuer_row("ISSUER-SHORT", 0.9, vec![0.85, 0.8], three_level_tags());
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
            require_issuer_id: false,
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-SHORT-HAZARD"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-SHORT");
        let err = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect_err("short beta vector must error");
        match err {
            FactorMatchError::BetaShapeMismatch {
                issuer_id,
                actual,
                expected,
            } => {
                assert_eq!(issuer_id, "ISSUER-SHORT");
                assert_eq!(actual, 2);
                assert_eq!(expected, 3);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    // unknown issuer with partial tags is an error;
    // with no tags at all it stays the PC-only proxy fallback
    #[test]
    fn unknown_issuer_with_partial_tags_errors_instead_of_truncating() {
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![],
            require_issuer_id: false,
        });
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("UNKNOWN-HAZARD"),
        };

        // Partial tags (rating only) → error naming the first missing dim.
        let attrs_partial = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "UNKNOWN")
            .with_meta("credit::rating", "IG");
        let err = matcher
            .match_factor_with_betas(&dep, &attrs_partial)
            .expect_err("partial tags must error, not silently truncate");
        match err {
            FactorMatchError::MissingRequiredTag { dimension } => {
                assert_eq!(dimension, "region");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }

        // No hierarchy tags at all → documented PC-only fallback.
        let attrs_none = Attributes::default().with_meta(ISSUER_ID_META_KEY, "UNKNOWN");
        let entries = matcher
            .match_factor_with_betas(&dep, &attrs_none)
            .expect("must succeed")
            .expect("must match");
        assert_eq!(entries.len(), 1, "PC-only proxy");
        assert_eq!(
            entries[0].factor_id,
            FactorId::new(CREDIT_GENERIC_FACTOR_ID)
        );
    }

    // Folded-level sentinel: β = 0.0 levels emit no entry and are not
    // enumerated
    #[test]
    fn zero_beta_levels_are_skipped_in_matching_and_enumeration() {
        // Level 1 folded (β = 0.0).
        let row = issuer_row("ISSUER-F", 0.9, vec![0.85, 0.0, 0.75], three_level_tags());
        let config = CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
            require_issuer_id: false,
        };

        let ids = config.enumerate_factor_ids();
        assert!(
            !ids.iter()
                .any(|id| id.as_str() == "credit::level1::Rating.Region::IG.EU"),
            "folded level-1 bucket must not be enumerated: {ids:?}"
        );

        let matcher = CreditHierarchicalMatcher::new(config);
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ISSUER-F-HAZARD"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-F");
        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect("must succeed")
            .expect("must match");
        assert_eq!(entries.len(), 3, "PC + levels 0 and 2 only");
        assert!(
            entries
                .iter()
                .all(|e| e.factor_id.as_str() != "credit::level1::Rating.Region::IG.EU"),
            "folded level must not be emitted: {entries:?}"
        );
    }

    #[test]
    fn credit_hierarchical_matcher_treats_unknown_issuer_as_bucket_only_when_tags_exist() {
        // Configure with NO known issuers; all matches must come from instrument tags.
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![],
            require_issuer_id: false,
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("UNKNOWN-HAZARD"),
        };
        let attrs = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "UNKNOWN-ISSUER")
            .with_meta("credit::rating", "IG")
            .with_meta("credit::region", "EU")
            .with_meta("credit::sector", "FIN");

        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .expect("must succeed")
            .expect("must match (bucket-only)");

        assert_eq!(entries.len(), 4);
        for entry in &entries {
            assert!(
                (entry.beta - 1.0).abs() < 1e-12,
                "BucketOnly betas must all be 1.0"
            );
        }
        assert_eq!(entries[0].factor_id, FactorId::new("credit::generic"));
        assert_eq!(
            entries[3].factor_id,
            FactorId::new("credit::level2::Rating.Region.Sector::IG.EU.FIN")
        );
    }

    // Non-credit dependency falls through to None
    #[test]
    fn non_credit_dependency_falls_through() {
        let matcher = matcher_with_one_issuer();
        let dep = MarketDependency::Spot { id: "AAPL".into() };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-A");
        let result = matcher.match_factor_with_betas(&dep, &attrs).unwrap();
        assert!(result.is_none());
    }

    // Custom dimension keys read from `Custom(name)`
    #[test]
    fn custom_hierarchy_dimension_uses_caller_supplied_key() {
        let spec = CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Custom("Currency".into()),
            ],
        };
        let mut tags = BTreeMap::new();
        tags.insert("rating".to_owned(), "IG".to_owned());
        tags.insert("Currency".to_owned(), "USD".to_owned());

        let row = issuer_row("ISS-X", 1.0, vec![1.0, 1.0], tags);
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: spec,
            issuer_betas: vec![row],
            require_issuer_id: false,
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISS-X");

        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .unwrap()
            .unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[2].factor_id,
            FactorId::new("credit::level1::Rating.Currency::IG.USD")
        );
    }

    // enumerate_factor_ids covers every bucket present in calibrated rows
    #[test]
    fn enumerate_factor_ids_returns_pc_and_all_buckets() {
        let matcher = matcher_with_one_issuer();
        let ids = matcher.config.enumerate_factor_ids();
        assert!(ids.contains(&FactorId::new("credit::generic")));
        assert!(ids.contains(&FactorId::new("credit::level0::Rating::IG")));
        assert!(ids.contains(&FactorId::new("credit::level1::Rating.Region::IG.EU")));
        assert!(ids.contains(&FactorId::new(
            "credit::level2::Rating.Region.Sector::IG.EU.FIN"
        )));
    }

    // Issuer betas are looked up via binary search; sort order matters.
    #[test]
    fn binary_search_finds_issuer_in_sorted_vec() {
        let mut rows = Vec::new();
        for tag in ["AAA", "BBB", "CCC", "DDD"] {
            rows.push(issuer_row(
                tag,
                1.5,
                vec![1.0, 1.0, 1.0],
                three_level_tags(),
            ));
        }
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: rows,
            require_issuer_id: false,
        });
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "CCC");
        let entries = matcher
            .match_factor_with_betas(&dep, &attrs)
            .unwrap()
            .unwrap();
        assert!((entries[0].beta - 1.5).abs() < 1e-12);
    }

    // Serde round-trip on the config
    #[test]
    fn credit_hierarchical_config_serde_roundtrip() {
        let config = CreditHierarchicalConfig {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            hierarchy: three_level_spec(),
            issuer_betas: vec![issuer_row(
                "ISSUER-A",
                0.9,
                vec![0.85, 0.8, 0.75],
                three_level_tags(),
            )],
            require_issuer_id: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: CreditHierarchicalConfig = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}
