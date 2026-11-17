//! Resolution engine for hierarchy-targeted operations.
//!
//! Resolves hierarchy paths and optional tag filters to unique typed
//! [`ResolvedTargetMatch`] candidates in preorder placement order, recording
//! each candidate's deepest matching ancestor node depth and path.

use super::{
    HierarchyNode, MarketDataHierarchy, parse_contract_json, validate_segment, validate_tag_key,
};
use crate::market_data::bumps::MarketDataTarget;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A typed market-data target matched at a specific hierarchy node depth and path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTargetMatch {
    /// Typed market-data target held by the matched subtree node.
    pub target: MarketDataTarget,
    /// Depth of the deepest matching node covering this target, counting roots as depth one.
    pub matched_depth: usize,
    /// Full hierarchy path from root to that deepest matching node.
    pub matched_path: Vec<String>,
}

/// Controls how shocks at multiple hierarchy levels combine for a single market target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMode {
    /// The deepest (most specific) matching hierarchy operation wins per exact target.
    #[default]
    MostSpecificWins,

    /// Matching hierarchy operations accumulate in authored and preorder placement order.
    Cumulative,
}

/// A predicate for filtering nodes by their tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(!try_from))]
#[serde(
    rename_all = "snake_case",
    deny_unknown_fields,
    try_from = "RawTagPredicate"
)]
pub enum TagPredicate {
    /// Tag value must exactly equal the given value.
    Equals {
        /// Non-blank tag key to match.
        key: String,
        /// Expected tag value.
        value: String,
    },
    /// Tag value must be one of the given values. An empty `values` list never matches.
    In {
        /// Non-blank tag key to match.
        key: String,
        /// Accepted tag values.
        values: Vec<String>,
    },
    /// Tag key must exist (any value).
    Exists {
        /// Non-blank tag key that must be present.
        key: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum RawTagPredicate {
    Equals { key: String, value: String },
    In { key: String, values: Vec<String> },
    Exists { key: String },
}

impl TryFrom<RawTagPredicate> for TagPredicate {
    type Error = crate::Error;

    fn try_from(raw: RawTagPredicate) -> crate::Result<Self> {
        let predicate = match raw {
            RawTagPredicate::Equals { key, value } => Self::Equals { key, value },
            RawTagPredicate::In { key, values } => Self::In { key, values },
            RawTagPredicate::Exists { key } => Self::Exists { key },
        };
        predicate.validate()?;
        Ok(predicate)
    }
}

impl TagPredicate {
    /// Validate that the predicate carries a non-blank tag key.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `key` is empty or whitespace-only.
    pub fn validate(&self) -> crate::Result<()> {
        let key = match self {
            TagPredicate::Equals { key, .. }
            | TagPredicate::In { key, .. }
            | TagPredicate::Exists { key } => key,
        };
        validate_tag_key(key)
    }

    /// Check whether a node's tags satisfy this predicate.
    ///
    /// # Arguments
    ///
    /// * `tags` - Tag map attached to the node being inspected.
    #[must_use]
    pub fn matches(&self, tags: &IndexMap<String, String>) -> bool {
        match self {
            TagPredicate::Equals { key, value } => tags.get(key).is_some_and(|v| v == value),
            TagPredicate::In { key, values } => {
                !values.is_empty() && tags.get(key).is_some_and(|v| values.contains(v))
            }
            TagPredicate::Exists { key } => tags.contains_key(key),
        }
    }
}

/// A filter combining multiple tag predicates (AND semantics).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TagFilter {
    /// All predicates must match (AND semantics).
    pub predicates: Vec<TagPredicate>,
}

impl TagFilter {
    /// Validate every predicate in this filter.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if any predicate carries a blank tag key.
    pub fn validate(&self) -> crate::Result<()> {
        for predicate in &self.predicates {
            predicate.validate()?;
        }
        Ok(())
    }

    /// Return `true` when the filter contains no predicates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }

    /// Check whether a node's tags satisfy all predicates.
    ///
    /// # Arguments
    ///
    /// * `tags` - Tag map attached to the node being inspected.
    #[must_use]
    pub fn matches(&self, tags: &IndexMap<String, String>) -> bool {
        self.predicates.iter().all(|p| p.matches(tags))
    }
}

fn tag_filter_is_empty(filter: &Option<TagFilter>) -> bool {
    filter.as_ref().is_none_or(TagFilter::is_empty)
}

/// A target specifying a hierarchy path with an optional tag filter.
#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
#[serde(deny_unknown_fields, try_from = "RawHierarchyTarget")]
pub struct HierarchyTarget {
    /// Non-empty path through the hierarchy (e.g., `["Credit", "US", "IG"]`).
    pub path: Vec<String>,
    /// Optional tag filter applied to nodes in the subtree; an empty filter
    /// canonicalizes to `None`.
    #[serde(default, skip_serializing_if = "tag_filter_is_empty")]
    pub tag_filter: Option<TagFilter>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHierarchyTarget {
    path: Vec<String>,
    #[serde(default)]
    tag_filter: Option<TagFilter>,
}

impl TryFrom<RawHierarchyTarget> for HierarchyTarget {
    type Error = crate::Error;

    fn try_from(raw: RawHierarchyTarget) -> crate::Result<Self> {
        Self::new(raw.path, raw.tag_filter)
    }
}

impl PartialEq for HierarchyTarget {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.effective_tag_filter() == other.effective_tag_filter()
    }
}

impl HierarchyTarget {
    /// Construct and validate a [`HierarchyTarget`], canonicalizing any empty
    /// [`TagFilter`] to `None`.
    ///
    /// # Arguments
    ///
    /// * `path` - Non-empty list of valid segment names (non-blank, no `'/'`).
    /// * `tag_filter` - Optional AND-predicate tag filter; an empty predicate list
    ///   becomes `None`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `path` is empty, any segment is
    /// blank or contains `'/'`, or any predicate carries a blank tag key.
    pub fn new(path: Vec<String>, tag_filter: Option<TagFilter>) -> crate::Result<Self> {
        let canonical = Self {
            path,
            tag_filter: tag_filter.filter(|f| !f.is_empty()),
        };
        canonical.validate()?;
        Ok(canonical)
    }

    /// Construct and validate an unfiltered [`HierarchyTarget`] from path segments.
    ///
    /// # Arguments
    ///
    /// * `path` - Non-empty list of valid segment names (non-blank, no `'/'`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `path` is empty or any segment is
    /// blank or contains `'/'`.
    pub fn unfiltered(path: Vec<String>) -> crate::Result<Self> {
        Self::new(path, None)
    }

    /// Return the effective non-empty tag filter, treating an empty filter identically to `None`.
    #[must_use]
    pub fn effective_tag_filter(&self) -> Option<&TagFilter> {
        self.tag_filter.as_ref().filter(|f| !f.is_empty())
    }

    /// Validate path segments and predicate keys on this target.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `path` is empty, any segment is
    /// blank or contains `'/'`, or any predicate carries a blank tag key.
    pub fn validate(&self) -> crate::Result<()> {
        if self.path.is_empty() {
            return Err(crate::Error::Validation(
                "Hierarchy target path cannot be empty".to_string(),
            ));
        }
        for segment in &self.path {
            validate_segment(segment)?;
        }
        if let Some(filter) = &self.tag_filter {
            filter.validate()?;
        }
        Ok(())
    }

    /// Deserialize a [`HierarchyTarget`] from JSON under strict contract checks.
    ///
    /// # Arguments
    ///
    /// * `json` - UTF-8 JSON representation of a [`HierarchyTarget`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] on malformed JSON, duplicate object
    /// keys, unknown fields, container limit violations, or invalid path/tag syntax.
    pub fn from_json(json: &str) -> crate::Result<Self> {
        parse_contract_json(json)
    }

    /// Serialize this target to canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if serialization fails.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string(self).map_err(|err| {
            crate::Error::Validation(format!("Failed to serialize HierarchyTarget: {err}"))
        })
    }
}

impl MarketDataHierarchy {
    /// Resolve a hierarchy selector to unique matched [`ResolvedTargetMatch`]
    /// entries in stable preorder placement order.
    ///
    /// Within a single selector, an unfiltered path or an ancestor matching a
    /// `tag_filter` includes all descendant nodes (and each descendant that also
    /// matches `tag_filter` refines `matched_depth` and `matched_path` to that
    /// deeper matching node). Each typed target appears at most once across the
    /// result, in preorder placement order.
    ///
    /// # Arguments
    ///
    /// * `target` - Hierarchy path and optional tag predicates selecting a
    ///   non-empty set of typed market targets.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `target` fails validation, the
    /// path does not exist in `self`, or the selector matches zero targets.
    pub fn resolve_matches(
        &self,
        target: &HierarchyTarget,
    ) -> crate::Result<Vec<ResolvedTargetMatch>> {
        target.validate()?;
        let Some(node) = self.get_node(&target.path) else {
            return Err(crate::Error::Validation(format!(
                "Hierarchy target path '{}' does not exist",
                target.path.join("/")
            )));
        };

        let mut matches = Vec::new();
        let mut node_path = target.path.clone();
        let filter = target.effective_tag_filter();

        match filter {
            None => {
                let anchor = Some((target.path.len(), target.path.clone()));
                collect_preorder_matches(node, &mut node_path, anchor.as_ref(), None, &mut matches);
            }
            Some(filter) => {
                collect_preorder_matches(node, &mut node_path, None, Some(filter), &mut matches);
            }
        }

        if matches.is_empty() {
            return Err(crate::Error::Validation(format!(
                "Hierarchy target path '{}' matched zero targets",
                target.path.join("/")
            )));
        }
        Ok(matches)
    }
}

fn collect_preorder_matches(
    node: &HierarchyNode,
    current_path: &mut Vec<String>,
    inherited_match: Option<&(usize, Vec<String>)>,
    filter: Option<&TagFilter>,
    matches: &mut Vec<ResolvedTargetMatch>,
) {
    let matched_here = match filter {
        Some(tag_filter) if tag_filter.matches(node.tags()) => {
            Some((current_path.len(), current_path.clone()))
        }
        None | Some(_) => inherited_match.cloned(),
    };

    if let Some((matched_depth, matched_path)) = &matched_here {
        for target in node.targets() {
            matches.push(ResolvedTargetMatch {
                target: target.clone(),
                matched_depth: *matched_depth,
                matched_path: matched_path.clone(),
            });
        }
    }

    for child in node.children() {
        current_path.push(child.name.clone());
        collect_preorder_matches(
            &child.node,
            current_path,
            matched_here.as_ref(),
            filter,
            matches,
        );
        current_path.pop();
    }
}
