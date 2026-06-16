//! Resolution engine for hierarchy-targeted operations.
//!
//! Resolves hierarchy paths + tag filters to sets of `CurveId`s, with
//! configurable inheritance modes (most-specific-wins vs. cumulative).

use super::{HierarchyNode, MarketDataHierarchy};
use crate::collections::{HashMap, HashSet};
use crate::types::CurveId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedCurveMatch {
    pub curve_id: CurveId,
    pub matched_depth: usize,
}

/// Controls how shocks at multiple hierarchy levels combine for a single curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMode {
    /// The deepest (most specific) matching node's shock wins.
    /// Parent-level shocks are ignored if a more specific one exists.
    #[default]
    MostSpecificWins,

    /// Shocks accumulate walking down the tree.
    /// A curve gets the sum of all shocks from root to its leaf node.
    Cumulative,
}

/// A predicate for filtering nodes by their tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TagPredicate {
    /// Tag value must exactly equal the given value.
    Equals {
        /// Tag key to match.
        key: String,
        /// Expected tag value.
        value: String,
    },
    /// Tag value must be one of the given values.
    In {
        /// Tag key to match.
        key: String,
        /// Accepted tag values.
        values: Vec<String>,
    },
    /// Tag key must exist (any value).
    Exists {
        /// Tag key that must be present.
        key: String,
    },
}

impl TagPredicate {
    /// Check if a node's tags satisfy this predicate.
    pub fn matches(&self, tags: &HashMap<String, String>) -> bool {
        match self {
            TagPredicate::Equals { key, value } => tags.get(key).is_some_and(|v| v == value),
            TagPredicate::In { key, values } => tags.get(key).is_some_and(|v| values.contains(v)),
            TagPredicate::Exists { key } => tags.contains_key(key),
        }
    }
}

/// A filter combining multiple tag predicates (AND semantics).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TagFilter {
    /// All predicates must match (AND semantics).
    pub predicates: Vec<TagPredicate>,
}

impl TagFilter {
    /// Check if a node's tags satisfy all predicates.
    pub fn matches(&self, tags: &HashMap<String, String>) -> bool {
        self.predicates.iter().all(|p| p.matches(tags))
    }

    /// An empty filter matches everything.
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

/// A target specifying a hierarchy path with optional tag filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HierarchyTarget {
    /// Path through the hierarchy (e.g., `["Credit", "US", "IG"]`).
    pub path: Vec<String>,
    /// Optional tag filter applied to nodes in the subtree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_filter: Option<TagFilter>,
}

impl MarketDataHierarchy {
    /// Resolve a hierarchy target to the set of `CurveId`s it covers.
    ///
    /// Walks to the node at `target.path`, then collects all `CurveId`s in that
    /// subtree. If a `tag_filter` is provided, curves in the full subtree of any
    /// matching node are included.
    ///
    /// The `mode` controls how matches at multiple tree depths combine:
    ///
    /// - [`ResolutionMode::MostSpecificWins`]: for each `CurveId`, only the
    ///   matches from the deepest (most specific) matching node are returned.
    ///   Parent-level matches are suppressed when a deeper match exists.
    /// - [`ResolutionMode::Cumulative`]: all matching `CurveId`s from all
    ///   matching nodes are returned, including duplicates across depths.
    pub fn resolve(&self, target: &HierarchyTarget, mode: ResolutionMode) -> Vec<CurveId> {
        self.resolve_matches(target, mode)
            .into_iter()
            .map(|matched| matched.curve_id)
            .collect()
    }

    pub(crate) fn resolve_matches(
        &self,
        target: &HierarchyTarget,
        mode: ResolutionMode,
    ) -> Vec<ResolvedCurveMatch> {
        let Some(node) = self.get_node(&target.path) else {
            return Vec::new();
        };

        let mut matches = Vec::new();
        let start_depth = target.path.len();
        match &target.tag_filter {
            None => collect_all_matches(node, start_depth, &mut matches),
            Some(filter) => collect_filtered_matches(node, filter, start_depth, &mut matches),
        }

        match mode {
            ResolutionMode::Cumulative => matches,
            ResolutionMode::MostSpecificWins => {
                let mut max_depth: HashMap<CurveId, usize> = HashMap::default();
                for matched in &matches {
                    max_depth
                        .entry(matched.curve_id.clone())
                        .and_modify(|best| *best = (*best).max(matched.matched_depth))
                        .or_insert(matched.matched_depth);
                }

                let mut emitted = HashSet::default();
                matches
                    .into_iter()
                    .filter(|matched| {
                        max_depth
                            .get(&matched.curve_id)
                            .is_some_and(|best| *best == matched.matched_depth)
                            && emitted.insert(matched.curve_id.clone())
                    })
                    .collect()
            }
        }
    }

    /// Find all curves matching a tag filter across the entire hierarchy.
    ///
    /// Tag predicates are matched against the **node's** tags, not the curve's.
    pub fn query_by_tags(&self, filter: &TagFilter) -> Vec<CurveId> {
        let mut matches = Vec::new();
        for root in self.roots.values() {
            collect_filtered_matches(root, filter, 1, &mut matches);
        }
        let mut seen = HashSet::default();
        matches
            .into_iter()
            .filter_map(|matched| {
                if seen.insert(matched.curve_id.clone()) {
                    Some(matched.curve_id)
                } else {
                    None
                }
            })
            .collect()
    }
}

fn collect_subtree_matches(
    node: &HierarchyNode,
    matched_depth: usize,
    matches: &mut Vec<ResolvedCurveMatch>,
) {
    for curve_id in node.curve_ids() {
        matches.push(ResolvedCurveMatch {
            curve_id: curve_id.clone(),
            matched_depth,
        });
    }
    for child in node.children().values() {
        collect_subtree_matches(child, matched_depth, matches);
    }
}

fn collect_all_matches(node: &HierarchyNode, depth: usize, matches: &mut Vec<ResolvedCurveMatch>) {
    collect_subtree_matches(node, depth, matches);
}

fn collect_filtered_matches(
    node: &HierarchyNode,
    filter: &TagFilter,
    depth: usize,
    matches: &mut Vec<ResolvedCurveMatch>,
) {
    if filter.matches(node.tags()) {
        collect_subtree_matches(node, depth, matches);
    }
    for child in node.children().values() {
        collect_filtered_matches(child, filter, depth + 1, matches);
    }
}
