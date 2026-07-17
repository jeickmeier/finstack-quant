//! Configuration and metadata filters for dependency-scoped factor matching.
//!
use super::credit::{CreditHierarchicalConfig, CreditHierarchicalMatcher};
use super::filter::DependencyFilter;
use super::matchers::{
    CascadeMatcher, FactorMatcher, FactorNode, HierarchicalMatcher, MappingRule,
    MappingTableMatcher,
};
use crate::primitives::factor_types::FactorId;
use serde::{Deserialize, Serialize};

/// Declarative configuration for a dependency-scoped hierarchical matcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalConfig {
    /// Dependency filter applied before any tree traversal.
    #[serde(default)]
    pub dependency_filter: DependencyFilter,
    /// Root of the attribute classification tree.
    pub root: FactorNode,
}

/// Declarative matcher configuration that can be serialized and rebuilt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub enum MatchingConfig {
    /// Flat mapping table with first-match-wins semantics.
    MappingTable(Vec<MappingRule>),
    /// Ordered matcher cascade.
    Cascade(Vec<MatchingConfig>),
    /// Hierarchical attribute-based tree matcher.
    Hierarchical(HierarchicalConfig),
    /// Calibrated credit hierarchy matcher.
    ///
    /// Emits `(credit::generic, β_pc)` plus
    /// `(credit::level{idx}::{dim_path}::{val_path}, β_level{idx})` entries
    /// using calibrated betas from
    /// [`crate::credit::hierarchy::CreditFactorModel`].
    CreditHierarchical(CreditHierarchicalConfig),
}

impl MatchingConfig {
    /// Builds a concrete matcher from this declarative configuration.
    #[must_use]
    pub fn build_matcher(&self) -> Box<dyn FactorMatcher> {
        match self {
            Self::MappingTable(rules) => Box::new(MappingTableMatcher::new(rules.clone())),
            Self::Cascade(configs) => Box::new(CascadeMatcher::new(
                configs.iter().map(Self::build_matcher).collect(),
            )),
            Self::Hierarchical(config) => Box::new(HierarchicalMatcher::new_scoped(
                config.dependency_filter.clone(),
                config.root.clone(),
            )),
            Self::CreditHierarchical(config) => {
                Box::new(CreditHierarchicalMatcher::new(config.clone()))
            }
        }
    }

    /// Returns every factor identifier this matcher config can emit.
    ///
    /// Used by [`FactorModelConfig`](crate::FactorModelConfig)
    /// validation to verify that every factor
    /// the matcher can name is also defined in `factors`.
    #[must_use]
    pub fn enumerate_factor_ids(&self) -> Vec<FactorId> {
        use std::collections::BTreeSet;
        let mut ids: BTreeSet<FactorId> = BTreeSet::new();
        self.collect_factor_ids(&mut ids);
        ids.into_iter().collect()
    }

    fn collect_factor_ids(&self, ids: &mut std::collections::BTreeSet<FactorId>) {
        match self {
            Self::MappingTable(rules) => {
                for rule in rules {
                    ids.insert(rule.factor_id.clone());
                }
            }
            Self::Cascade(configs) => {
                for cfg in configs {
                    cfg.collect_factor_ids(ids);
                }
            }
            Self::Hierarchical(config) => {
                collect_node_factor_ids(&config.root, ids);
            }
            Self::CreditHierarchical(config) => {
                for fid in config.enumerate_factor_ids() {
                    ids.insert(fid);
                }
            }
        }
    }
}

fn collect_node_factor_ids(node: &FactorNode, ids: &mut std::collections::BTreeSet<FactorId>) {
    if let Some(fid) = &node.factor_id {
        ids.insert(fid.clone());
    }
    for child in &node.children {
        collect_node_factor_ids(child, ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::{AttributeFilter, DependencyFilter};
    use crate::primitives::dependency::{DependencyType, MarketDependency};
    use crate::primitives::factor_types::FactorId;
    use finstack_quant_core::types::{Attributes, CurveId};

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

    #[test]
    fn test_matching_config_mapping_table() {
        let config = MatchingConfig::MappingTable(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Credit),
                curve_type: None,
                id: None,
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("Credit"),
        }]);

        let matcher = config.build_matcher();
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("X"),
        };
        assert_eq!(
            deepest_match(matcher.as_ref(), &dep, &Attributes::default()),
            Some(FactorId::new("Credit"))
        );
    }

    #[test]
    fn test_matching_config_cascade() {
        let config = MatchingConfig::Cascade(vec![
            MatchingConfig::MappingTable(vec![MappingRule {
                dependency_filter: DependencyFilter {
                    dependency_type: Some(DependencyType::Spot),
                    curve_type: None,
                    id: Some("SPECIAL".into()),
                },
                attribute_filter: AttributeFilter::default(),
                factor_id: FactorId::new("Special"),
            }]),
            MatchingConfig::MappingTable(vec![MappingRule {
                dependency_filter: DependencyFilter::default(),
                attribute_filter: AttributeFilter::default(),
                factor_id: FactorId::new("Fallback"),
            }]),
        ]);

        let matcher = config.build_matcher();

        let dep1 = MarketDependency::Spot {
            id: "SPECIAL".into(),
        };
        assert_eq!(
            deepest_match(matcher.as_ref(), &dep1, &Attributes::default()),
            Some(FactorId::new("Special"))
        );

        let dep2 = MarketDependency::Spot { id: "OTHER".into() };
        assert_eq!(
            deepest_match(matcher.as_ref(), &dep2, &Attributes::default()),
            Some(FactorId::new("Fallback"))
        );
    }

    #[test]
    fn test_matching_config_serde_roundtrip() {
        let config = MatchingConfig::Cascade(vec![
            MatchingConfig::MappingTable(vec![]),
            MatchingConfig::Hierarchical(HierarchicalConfig {
                dependency_filter: DependencyFilter {
                    dependency_type: Some(DependencyType::Credit),
                    curve_type: None,
                    id: None,
                },
                root: FactorNode {
                    factor_id: Some(FactorId::new("Root")),
                    filter: AttributeFilter::default(),
                    children: vec![],
                },
            }),
        ]);
        let json_result = serde_json::to_string(&config);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let roundtrip_result: Result<MatchingConfig, _> = serde_json::from_str(&json);
        assert!(roundtrip_result.is_ok());
        let Ok(roundtrip) = roundtrip_result else {
            return;
        };

        assert_eq!(config, roundtrip);
    }
}
