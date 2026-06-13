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
use crate::credit::hierarchy::{
    dimension_key, CreditHierarchySpec, HierarchyDimension, IssuerBetaRow, IssuerTags,
};
use crate::primitives::dependency::MarketDependency;
use crate::primitives::factor_types::FactorId;
use finstack_core::types::{Attributes, IssuerId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public constants
// ---------------------------------------------------------------------------

/// Reserved key in [`Attributes::meta`] used to thread the issuer identifier
/// from the position into the matcher.
///
/// Set this key on the instrument's [`Attributes`] before calling the matcher.
/// If the key is absent the issuer is treated as unknown (`BucketOnly`).
pub const ISSUER_ID_META_KEY: &str = "credit::issuer_id";

/// Canonical factor ID for the generic credit (PC) factor.
pub const CREDIT_GENERIC_FACTOR_ID: &str = "credit::generic";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Declarative configuration for a calibrated credit-hierarchy matcher.
///
/// The matcher emits PC + per-level credit factors with calibrated betas
/// looked up from `issuer_betas`. `issuer_betas` must be sorted by
/// `issuer_id` (binary search is used). `hierarchy` defines the level
/// ordering and dimension keys used to build factor IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        for row in &self.issuer_betas {
            for level_idx in 0..self.hierarchy.levels.len() {
                // β = 0.0 is the calibration sentinel for "level folded /
                // skipped"; folded buckets have no factor definition or
                // covariance row, so they are not enumerated (and the matcher
                // emits no entry for them either).
                if row.betas.levels.get(level_idx).copied().unwrap_or(0.0) == 0.0 {
                    continue;
                }
                if let Some(id) = bucket_factor_id(&self.hierarchy, &row.tags, level_idx) {
                    ids.insert(id);
                }
            }
        }
        ids.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// Matcher
// ---------------------------------------------------------------------------

/// Calibrated credit-hierarchy matcher.
///
/// See the module-level docs for the semantics.
#[derive(Debug, Clone)]
pub struct CreditHierarchicalMatcher {
    config: CreditHierarchicalConfig,
}

impl CreditHierarchicalMatcher {
    /// Creates a matcher from a calibrated config.
    ///
    /// `issuer_betas` is re-sorted by `issuer_id` defensively: row lookup uses
    /// binary search, and an unsorted vector (e.g. from a hand-assembled
    /// config) would otherwise silently miss calibrated rows and substitute
    /// β = 1.0 for every factor.
    #[must_use]
    pub fn new(mut config: CreditHierarchicalConfig) -> Self {
        config
            .issuer_betas
            .sort_by(|a, b| a.issuer_id.as_str().cmp(b.issuer_id.as_str()));
        Self { config }
    }

    fn lookup_row(&self, issuer_id: &IssuerId) -> Option<&IssuerBetaRow> {
        self.config
            .issuer_betas
            .binary_search_by(|row| row.issuer_id.as_str().cmp(issuer_id.as_str()))
            .ok()
            .map(|idx| &self.config.issuer_betas[idx])
    }
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

        // Look up calibrated betas if available; otherwise fall back to 1.0.
        let row = issuer_id_str
            .map(IssuerId::new)
            .as_ref()
            .and_then(|id| self.lookup_row(id));

        // Source of issuer tags: the calibrated row's tags take precedence,
        // because they reflect the canonical taxonomy. If no row is found,
        // we read tags directly from `attributes.meta` using the same key
        // convention.
        let tags_owned: IssuerTags;
        let tags = match row {
            Some(r) => &r.tags,
            None => {
                tags_owned = tags_from_attributes(&self.config.hierarchy, attributes);
                &tags_owned
            }
        };

        // A calibrated row whose beta vector disagrees with the hierarchy
        // depth is an inconsistent config; substituting β = 1.0 silently
        // would misstate risk.
        if let Some(r) = row {
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
        let pc_beta = row.map_or(1.0, |r| r.betas.pc);
        entries.push(FactorMatchEntry {
            factor_id: FactorId::new(CREDIT_GENERIC_FACTOR_ID),
            beta: pc_beta,
        });

        // Unknown issuers: tags come from instrument attributes. A complete
        // absence of hierarchy tags is the documented PC-only proxy fallback;
        // a *partial* tag set is treated as a contract violation just like a
        // known issuer's missing tag — silently truncating the hierarchy
        // would under-map specific credit risk without any signal a strict
        // unmatched policy could observe.
        let any_hierarchy_tag_present = self
            .config
            .hierarchy
            .levels
            .iter()
            .any(|dim| tags.0.contains_key(&dimension_key(dim)));

        // Emit one entry per hierarchy level the issuer is tagged for.
        for (level_idx, dim) in self.config.hierarchy.levels.iter().enumerate() {
            let tag_present = tags.0.contains_key(&dimension_key(dim));
            if !tag_present {
                if row.is_some() || any_hierarchy_tag_present {
                    return Err(FactorMatchError::MissingRequiredTag {
                        dimension: dimension_key(dim),
                    });
                }
                break;
            }

            // Build the bucket factor ID for this level.
            let Some(factor_id) = bucket_factor_id(&self.config.hierarchy, tags, level_idx) else {
                // bucket_factor_id only fails if a deeper-level tag is missing,
                // but we already checked the current level; treat as missing.
                return Err(FactorMatchError::MissingRequiredTag {
                    dimension: dimension_key(dim),
                });
            };

            let beta = row
                .and_then(|r| r.betas.levels.get(level_idx).copied())
                .unwrap_or(1.0);
            // β = 0.0 is the calibration sentinel for "level folded"; the
            // bucket factor does not exist in the model, so emit no entry
            // (the exposure would be identically zero anyway).
            if beta == 0.0 {
                continue;
            }
            entries.push(FactorMatchEntry { factor_id, beta });
        }

        Ok(Some(entries))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Dotted dimension-name path through the first `level_idx + 1` levels of
/// the hierarchy spec, e.g. `"Rating.Region"` for level index 1.
fn dimension_path(spec: &CreditHierarchySpec, level_idx: usize) -> String {
    spec.levels
        .iter()
        .take(level_idx + 1)
        .map(|dim| match dim {
            HierarchyDimension::Rating => "Rating".to_owned(),
            HierarchyDimension::Region => "Region".to_owned(),
            HierarchyDimension::Sector => "Sector".to_owned(),
            HierarchyDimension::Custom(name) => name.clone(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Builds the canonical factor ID `credit::level{idx}::{dim_path}::{val_path}`
/// for the given hierarchy level. Returns `None` if any required tag is missing.
#[must_use]
pub fn bucket_factor_id(
    spec: &CreditHierarchySpec,
    tags: &IssuerTags,
    level_idx: usize,
) -> Option<FactorId> {
    if level_idx >= spec.levels.len() {
        return None;
    }
    let dim_path = dimension_path(spec, level_idx);
    let val_path = spec.bucket_path(tags, level_idx)?;
    Some(FactorId::new(format!(
        "credit::level{level_idx}::{dim_path}::{val_path}"
    )))
}

/// Whether a [`MarketDependency`] is a credit/hazard one. The matcher only
/// emits factors for credit-side dependencies regardless of how the user
/// configured `dependency_filter`.
fn is_credit_dependency(dep: &MarketDependency) -> bool {
    use crate::primitives::dependency::CurveType;
    match dep {
        MarketDependency::CreditCurve { .. } => true,
        MarketDependency::Curve { curve_type, .. } => *curve_type == CurveType::Hazard,
        _ => false,
    }
}

/// Build an [`IssuerTags`] view from `attributes.meta` using the canonical
/// dimension keys defined by `spec`.
///
/// Used as a fallback for unknown issuers (no calibrated row): the matcher
/// reads tags from instrument metadata using the same key convention as
/// [`dimension_key`].
fn tags_from_attributes(spec: &CreditHierarchySpec, attrs: &Attributes) -> IssuerTags {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    for dim in &spec.levels {
        let key = dimension_key(dim);
        if let Some(v) = attrs.get_meta(&key) {
            map.insert(key, v.to_owned());
        }
    }
    IssuerTags(map)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credit::hierarchy::{
        AdderVolSource, IssuerBetaMode, IssuerBetaRow, IssuerBetas, IssuerTags,
    };
    use crate::primitives::dependency::{DependencyType, MarketDependency};
    use finstack_core::types::{Attributes, CurveId, IssuerId};
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
        })
    }

    // --------------------------------------------------------------
    // PR-2 test: known issuer → PC + bucket factors in canonical order
    // --------------------------------------------------------------
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

    // --------------------------------------------------------------
    // PR-2 test: known issuer with missing tag is a typed error
    // --------------------------------------------------------------
    #[test]
    fn credit_hierarchical_matcher_errors_on_missing_required_tag() {
        let mut tags = three_level_tags();
        tags.remove("region"); // Known issuer, but tagged for only level 0.
        let row = issuer_row("ISSUER-MISSING", 1.0, vec![1.0, 1.0, 1.0], tags);
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
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

    // --------------------------------------------------------------
    // unsorted issuer_betas must not break lookup
    // --------------------------------------------------------------
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

    // --------------------------------------------------------------
    // short beta vector is a typed error, not β = 1.0
    // --------------------------------------------------------------
    #[test]
    fn matcher_errors_on_beta_shape_mismatch() {
        // Two betas for a three-level hierarchy.
        let row = issuer_row("ISSUER-SHORT", 0.9, vec![0.85, 0.8], three_level_tags());
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
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

    // --------------------------------------------------------------
    // unknown issuer with partial tags is an error;
    // with no tags at all it stays the PC-only proxy fallback
    // --------------------------------------------------------------
    #[test]
    fn unknown_issuer_with_partial_tags_errors_instead_of_truncating() {
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![],
        });
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("UNKNOWN-HAZARD"),
        };

        // Partial tags (rating only) → error naming the first missing dim.
        let attrs_partial = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "UNKNOWN")
            .with_meta("rating", "IG");
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

    // --------------------------------------------------------------
    // Folded-level sentinel: β = 0.0 levels emit no entry and are not
    // enumerated
    // --------------------------------------------------------------
    #[test]
    fn zero_beta_levels_are_skipped_in_matching_and_enumeration() {
        // Level 1 folded (β = 0.0).
        let row = issuer_row("ISSUER-F", 0.9, vec![0.85, 0.0, 0.75], three_level_tags());
        let config = CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![row],
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

    // --------------------------------------------------------------
    // PR-2 test: unknown issuer with full tags → BucketOnly (β = 1)
    // --------------------------------------------------------------
    #[test]
    fn credit_hierarchical_matcher_treats_unknown_issuer_as_bucket_only_when_tags_exist() {
        // Configure with NO known issuers; all matches must come from instrument tags.
        let matcher = CreditHierarchicalMatcher::new(CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: three_level_spec(),
            issuer_betas: vec![],
        });

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("UNKNOWN-HAZARD"),
        };
        let attrs = Attributes::default()
            .with_meta(ISSUER_ID_META_KEY, "UNKNOWN-ISSUER")
            .with_meta("rating", "IG")
            .with_meta("region", "EU")
            .with_meta("sector", "FIN");

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

    // --------------------------------------------------------------
    // Non-credit dependency falls through to None
    // --------------------------------------------------------------
    #[test]
    fn non_credit_dependency_falls_through() {
        let matcher = matcher_with_one_issuer();
        let dep = MarketDependency::Spot { id: "AAPL".into() };
        let attrs = Attributes::default().with_meta(ISSUER_ID_META_KEY, "ISSUER-A");
        let result = matcher.match_factor_with_betas(&dep, &attrs).unwrap();
        assert!(result.is_none());
    }

    // --------------------------------------------------------------
    // Custom dimension keys read from `Custom(name)`
    // --------------------------------------------------------------
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

    // --------------------------------------------------------------
    // enumerate_factor_ids covers every bucket present in calibrated rows
    // --------------------------------------------------------------
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

    // --------------------------------------------------------------
    // Issuer betas are looked up via binary search; sort order matters.
    // --------------------------------------------------------------
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

    // --------------------------------------------------------------
    // Serde round-trip on the config
    // --------------------------------------------------------------
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
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: CreditHierarchicalConfig = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}
