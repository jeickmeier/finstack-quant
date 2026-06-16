//! `FactorMatcher` trait and its built-in implementations.
//!
//! Three matchers are provided:
//! - [`MappingTableMatcher`]: flat rule-table lookup, first match wins.
//! - [`HierarchicalMatcher`]: tree traversal, deepest match wins.
//! - [`CascadeMatcher`]: ordered fallback chain over other matchers.
//!
//! A fourth matcher, [`CreditHierarchicalMatcher`], is only constructed
//! through [`MatchingConfig::CreditHierarchical`] and lives in
//! [`super::credit`].

use super::filter::{AttributeFilter, DependencyFilter};
use crate::primitives::dependency::MarketDependency;
use crate::primitives::factor_types::FactorId;
use finstack_quant_core::types::Attributes;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// One factor match decorated with a beta loading.
///
/// For most matchers this collapses to `(factor_id, 1.0)` — one entry per
/// dependency, beta = 1. The credit hierarchy matcher (
/// [`super::credit::CreditHierarchicalMatcher`]) emits multiple entries with
/// calibrated betas read from a [`crate::credit::hierarchy::IssuerBetaRow`].
#[derive(Debug, Clone, PartialEq)]
pub struct FactorMatchEntry {
    /// Matched factor identifier.
    pub factor_id: FactorId,
    /// Beta loading on the matched factor.
    pub beta: f64,
}

/// Error returned by [`FactorMatcher::match_factor_with_betas`] when the
/// matcher can determine the dependency is in scope but cannot produce
/// a deterministic answer (e.g. a required issuer tag is missing).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FactorMatchError {
    /// A required issuer tag is missing for hierarchy bucketing.
    #[error(
        "credit-hierarchical matcher: issuer tag for dimension '{dimension}' is required but missing"
    )]
    MissingRequiredTag {
        /// Hierarchy dimension key that was not found.
        dimension: String,
    },
    /// A calibrated issuer row is structurally inconsistent with the
    /// hierarchy (e.g. `betas.levels` length disagrees with the number of
    /// hierarchy levels). Silently substituting β = 1.0 would misstate risk.
    #[error(
        "credit-hierarchical matcher: issuer '{issuer_id}' has {actual} level betas, \
         expected {expected}; the calibrated config is inconsistent"
    )]
    BetaShapeMismatch {
        /// Issuer whose beta row is malformed.
        issuer_id: String,
        /// Number of betas found on the row.
        actual: usize,
        /// Number of hierarchy levels expected.
        expected: usize,
    },
}

/// Matches a market dependency and instrument attributes to factor identifiers.
pub trait FactorMatcher: Send + Sync {
    /// Returns the matched `(factor_id, beta)` entries for a dependency.
    ///
    /// Most matchers produce a single entry with `beta = 1.0`; the credit
    /// hierarchy matcher emits multiple entries per dependency.
    ///
    /// # Errors
    ///
    /// Returns [`FactorMatchError`] when the matcher recognised the dependency
    /// but the inputs (typically issuer tags) violate the matcher's contract.
    fn match_factor_with_betas(
        &self,
        dependency: &MarketDependency,
        attributes: &Attributes,
    ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError>;
}

/// Helper: lift a single matched factor id into the canonical
/// `Vec<FactorMatchEntry>` shape with `beta = 1.0`.
#[inline]
fn one_entry(factor_id: FactorId) -> Vec<FactorMatchEntry> {
    vec![FactorMatchEntry {
        factor_id,
        beta: 1.0,
    }]
}

#[cfg(test)]
fn deepest_match(
    matcher: &dyn FactorMatcher,
    dependency: &MarketDependency,
    attributes: &Attributes,
) -> Option<FactorId> {
    matcher
        .match_factor_with_betas(dependency, attributes)
        .ok()
        .flatten()
        .and_then(|entries| entries.last().map(|entry| entry.factor_id.clone()))
}

// ---------------------------------------------------------------------------
// MappingTableMatcher
// ---------------------------------------------------------------------------

/// A single matching rule from dependency and attribute filters to a factor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingRule {
    /// Dependency-side filter.
    pub dependency_filter: DependencyFilter,
    /// Instrument metadata filter.
    pub attribute_filter: AttributeFilter,
    /// Factor assigned when both filters match.
    pub factor_id: FactorId,
}

/// Flat lookup-table matcher where the first matching rule wins.
#[derive(Debug, Clone, Default)]
pub struct MappingTableMatcher {
    rules: Vec<MappingRule>,
}

impl MappingTableMatcher {
    /// Creates a matcher from an ordered set of rules.
    #[must_use]
    pub fn new(rules: Vec<MappingRule>) -> Self {
        Self { rules }
    }
}

impl FactorMatcher for MappingTableMatcher {
    fn match_factor_with_betas(
        &self,
        dependency: &MarketDependency,
        attributes: &Attributes,
    ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError> {
        Ok(self
            .rules
            .iter()
            .find(|rule| {
                rule.dependency_filter.matches(dependency)
                    && rule.attribute_filter.matches(attributes)
            })
            .map(|rule| one_entry(rule.factor_id.clone())))
    }
}

// ---------------------------------------------------------------------------
// HierarchicalMatcher
// ---------------------------------------------------------------------------

/// A node in a hierarchical factor classification tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorNode {
    /// Factor assigned at this node when it is a valid classification level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor_id: Option<FactorId>,
    /// Filter that must match for this node to participate in traversal.
    pub filter: AttributeFilter,
    /// Child nodes representing more specific classifications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FactorNode>,
}

/// Tree-based matcher where the deepest matching factor assignment wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchicalMatcher {
    dependency_filter: DependencyFilter,
    root: FactorNode,
}

impl HierarchicalMatcher {
    /// Creates a matcher from the provided root node.
    #[must_use]
    pub fn new(root: FactorNode) -> Self {
        Self {
            dependency_filter: DependencyFilter::default(),
            root,
        }
    }

    /// Creates a matcher scoped to dependencies satisfying the provided filter.
    #[must_use]
    pub fn new_scoped(dependency_filter: DependencyFilter, root: FactorNode) -> Self {
        Self {
            dependency_filter,
            root,
        }
    }

    fn find_best_match(
        node: &FactorNode,
        attrs: &Attributes,
        depth: usize,
    ) -> Option<(usize, FactorId)> {
        if !node.filter.matches(attrs) {
            return None;
        }

        let mut best = node.factor_id.clone().map(|factor_id| (depth, factor_id));

        for child in &node.children {
            if let Some(candidate) = Self::find_best_match(child, attrs, depth + 1) {
                let should_replace = match &best {
                    Some((best_depth, _)) => candidate.0 > *best_depth,
                    None => true,
                };
                if should_replace {
                    best = Some(candidate);
                }
            }
        }

        best
    }
}

impl FactorMatcher for HierarchicalMatcher {
    fn match_factor_with_betas(
        &self,
        dependency: &MarketDependency,
        attributes: &Attributes,
    ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError> {
        if !self.dependency_filter.matches(dependency) {
            return Ok(None);
        }
        Ok(Self::find_best_match(&self.root, attributes, 0)
            .map(|(_, factor_id)| one_entry(factor_id)))
    }
}

// ---------------------------------------------------------------------------
// CascadeMatcher
// ---------------------------------------------------------------------------

/// Ordered matcher chain that returns the first successful factor match.
pub struct CascadeMatcher {
    matchers: Vec<Box<dyn FactorMatcher>>,
}

impl CascadeMatcher {
    /// Creates a cascade from matchers tried in priority order.
    #[must_use]
    pub fn new(matchers: Vec<Box<dyn FactorMatcher>>) -> Self {
        Self { matchers }
    }
}

impl FactorMatcher for CascadeMatcher {
    fn match_factor_with_betas(
        &self,
        dependency: &MarketDependency,
        attributes: &Attributes,
    ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError> {
        for matcher in &self.matchers {
            match matcher.match_factor_with_betas(dependency, attributes)? {
                Some(entries) if !entries.is_empty() => return Ok(Some(entries)),
                _ => continue,
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests_mapping_table {
    use super::*;
    use crate::primitives::dependency::{CurveType, DependencyType, MarketDependency};
    use finstack_quant_core::types::CurveId;

    #[test]
    fn test_empty_table_matches_nothing() {
        let matcher = MappingTableMatcher::new(vec![]);
        let dep = MarketDependency::Spot { id: "AAPL".into() };
        let attrs = Attributes::default();
        assert_eq!(deepest_match(&matcher, &dep, &attrs), None);
    }

    #[test]
    fn test_first_match_wins() {
        let matcher = MappingTableMatcher::new(vec![
            MappingRule {
                dependency_filter: DependencyFilter {
                    dependency_type: Some(DependencyType::Credit),
                    curve_type: None,
                    id: None,
                },
                attribute_filter: AttributeFilter {
                    tags: vec!["energy".into()],
                    meta: vec![("rating".into(), "CCC".into())],
                },
                factor_id: FactorId::new("NA-Energy-CCC"),
            },
            MappingRule {
                dependency_filter: DependencyFilter {
                    dependency_type: Some(DependencyType::Credit),
                    curve_type: None,
                    id: None,
                },
                attribute_filter: AttributeFilter::default(),
                factor_id: FactorId::new("Generic-Credit"),
            },
        ]);

        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };

        let attrs1 = Attributes::default()
            .with_tag("energy")
            .with_meta("rating", "CCC");
        assert_eq!(
            deepest_match(&matcher, &dep, &attrs1),
            Some(FactorId::new("NA-Energy-CCC"))
        );

        let attrs2 = Attributes::default().with_tag("financials");
        assert_eq!(
            deepest_match(&matcher, &dep, &attrs2),
            Some(FactorId::new("Generic-Credit"))
        );
    }

    #[test]
    fn test_no_match_returns_none() {
        let matcher = MappingTableMatcher::new(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("Credit"),
        }]);

        let dep = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        let attrs = Attributes::default();
        assert_eq!(deepest_match(&matcher, &dep, &attrs), None);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let rules = vec![MappingRule {
            dependency_filter: DependencyFilter::default(),
            attribute_filter: AttributeFilter {
                tags: vec!["energy".into()],
                meta: vec![],
            },
            factor_id: FactorId::new("Energy"),
        }];
        let json_result = serde_json::to_string(&rules);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let roundtrip_result: Result<Vec<MappingRule>, _> = serde_json::from_str(&json);
        assert!(roundtrip_result.is_ok());
        let Ok(roundtrip) = roundtrip_result else {
            return;
        };

        assert_eq!(rules, roundtrip);
    }
}

#[cfg(test)]
mod tests_hierarchical {
    use super::*;
    use crate::primitives::dependency::MarketDependency;
    use finstack_quant_core::types::CurveId;

    fn credit_tree() -> FactorNode {
        FactorNode {
            factor_id: Some(FactorId::new("Generic-Credit")),
            filter: AttributeFilter::default(),
            children: vec![
                FactorNode {
                    factor_id: Some(FactorId::new("NA-Credit")),
                    filter: AttributeFilter {
                        tags: vec![],
                        meta: vec![("region".into(), "NA".into())],
                    },
                    children: vec![FactorNode {
                        factor_id: None,
                        filter: AttributeFilter {
                            tags: vec!["energy".into()],
                            meta: vec![],
                        },
                        children: vec![
                            FactorNode {
                                factor_id: Some(FactorId::new("NA-Energy-CCC")),
                                filter: AttributeFilter {
                                    tags: vec![],
                                    meta: vec![("rating".into(), "CCC".into())],
                                },
                                children: vec![],
                            },
                            FactorNode {
                                factor_id: Some(FactorId::new("NA-Energy-IG")),
                                filter: AttributeFilter {
                                    tags: vec![],
                                    meta: vec![("rating".into(), "IG".into())],
                                },
                                children: vec![],
                            },
                        ],
                    }],
                },
                FactorNode {
                    factor_id: Some(FactorId::new("EU-Credit")),
                    filter: AttributeFilter {
                        tags: vec![],
                        meta: vec![("region".into(), "EU".into())],
                    },
                    children: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_deepest_match_wins() {
        let matcher = HierarchicalMatcher::new(credit_tree());
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };

        let attrs = Attributes::default()
            .with_meta("region", "NA")
            .with_tag("energy")
            .with_meta("rating", "CCC");

        assert_eq!(
            deepest_match(&matcher, &dep, &attrs),
            Some(FactorId::new("NA-Energy-CCC"))
        );
    }

    #[test]
    fn test_rolls_up_to_parent() {
        let matcher = HierarchicalMatcher::new(credit_tree());
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };

        let attrs = Attributes::default()
            .with_meta("region", "NA")
            .with_tag("financials");

        assert_eq!(
            deepest_match(&matcher, &dep, &attrs),
            Some(FactorId::new("NA-Credit"))
        );
    }

    #[test]
    fn test_root_fallback() {
        let matcher = HierarchicalMatcher::new(credit_tree());
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };

        let attrs = Attributes::default().with_meta("region", "APAC");

        assert_eq!(
            deepest_match(&matcher, &dep, &attrs),
            Some(FactorId::new("Generic-Credit"))
        );
    }

    #[test]
    fn test_eu_branch() {
        let matcher = HierarchicalMatcher::new(credit_tree());
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };

        let attrs = Attributes::default().with_meta("region", "EU");

        assert_eq!(
            deepest_match(&matcher, &dep, &attrs),
            Some(FactorId::new("EU-Credit"))
        );
    }

    #[test]
    fn test_energy_without_rating_rolls_up_to_na() {
        let matcher = HierarchicalMatcher::new(credit_tree());
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };

        let attrs = Attributes::default()
            .with_meta("region", "NA")
            .with_tag("energy");

        assert_eq!(
            deepest_match(&matcher, &dep, &attrs),
            Some(FactorId::new("NA-Credit"))
        );
    }

    #[test]
    fn test_factor_node_serde_roundtrip() {
        let tree = credit_tree();
        let json_result = serde_json::to_string(&tree);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let roundtrip_result: Result<FactorNode, _> = serde_json::from_str(&json);
        assert!(roundtrip_result.is_ok());
        let Ok(roundtrip) = roundtrip_result else {
            return;
        };

        assert_eq!(roundtrip.children.len(), 2);
    }

    #[test]
    fn test_scoped_hierarchical_matcher_filters_dependency_class() {
        let matcher = HierarchicalMatcher::new_scoped(
            DependencyFilter {
                dependency_type: Some(crate::DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            credit_tree(),
        );
        let credit_dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };
        let spot_dep = MarketDependency::Spot { id: "AAPL".into() };
        let attrs = Attributes::default().with_meta("region", "EU");

        assert_eq!(
            deepest_match(&matcher, &credit_dep, &attrs),
            Some(FactorId::new("EU-Credit"))
        );
        assert_eq!(deepest_match(&matcher, &spot_dep, &attrs), None);
    }
}

#[cfg(test)]
mod tests_cascade {
    use super::*;
    use crate::primitives::dependency::{DependencyType, MarketDependency};
    use finstack_quant_core::types::CurveId;

    #[test]
    fn test_cascade_tries_in_order() {
        let exact = MappingTableMatcher::new(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: Some("ACME-HAZARD".into()),
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("ACME-Specific"),
        }]);

        let fallback = MappingTableMatcher::new(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("Generic-Credit"),
        }]);

        let cascade = CascadeMatcher::new(vec![Box::new(exact), Box::new(fallback)]);
        let attrs = Attributes::default();

        let dep1 = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };
        assert_eq!(
            deepest_match(&cascade, &dep1, &attrs),
            Some(FactorId::new("ACME-Specific"))
        );

        let dep2 = MarketDependency::CreditCurve {
            id: CurveId::new("OTHER-HAZARD"),
        };
        assert_eq!(
            deepest_match(&cascade, &dep2, &attrs),
            Some(FactorId::new("Generic-Credit"))
        );

        let dep3 = MarketDependency::Spot { id: "AAPL".into() };
        assert_eq!(deepest_match(&cascade, &dep3, &attrs), None);
    }

    #[test]
    fn test_empty_cascade() {
        let cascade = CascadeMatcher::new(vec![]);
        let dep = MarketDependency::Spot { id: "AAPL".into() };
        assert_eq!(deepest_match(&cascade, &dep, &Attributes::default()), None);
    }
}
