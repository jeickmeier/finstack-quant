//! Core [`MarketDataHierarchy`] tree types and path validation.

use super::HierarchyBuilder;

use crate::contract::{LoadLimits, deserialize_json_value, parse_json_value};
use crate::market_data::bumps::MarketDataTarget;
use crate::{FxHashMap, FxHashSet};
use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Validate a single hierarchy path segment, preserving non-blank surrounding whitespace.
pub(crate) fn validate_segment(segment: &str) -> crate::Result<()> {
    if segment.trim().is_empty() {
        return Err(crate::Error::Validation(
            "Hierarchy node names must not be empty or whitespace-only".to_string(),
        ));
    }
    if segment.contains('/') {
        return Err(crate::Error::Validation(format!(
            "Hierarchy node name '{segment}' must not contain '/'"
        )));
    }
    Ok(())
}

/// Validate a node tag key.
pub(crate) fn validate_tag_key(key: &str) -> crate::Result<()> {
    if key.trim().is_empty() {
        return Err(crate::Error::Validation(
            "Hierarchy tag keys must not be empty or whitespace-only".to_string(),
        ));
    }
    Ok(())
}

/// Parse a `/`-separated hierarchy path into its non-blank segments.
pub(crate) fn parse_path(path: &str) -> crate::Result<Vec<String>> {
    if path.is_empty() {
        return Err(crate::Error::Validation(
            "Hierarchy paths must not be empty".to_string(),
        ));
    }

    let segments: Vec<String> = path.split('/').map(String::from).collect();
    for segment in &segments {
        validate_segment(segment).map_err(|_| {
            crate::Error::Validation(format!(
                "Hierarchy path '{path}' must not contain empty or whitespace-only segments"
            ))
        })?;
    }

    Ok(segments)
}

/// Parse a strict UTF-8 contract JSON document with duplicate-key and container-depth checks.
pub(crate) fn parse_contract_json<T: serde::de::DeserializeOwned>(json: &str) -> crate::Result<T> {
    let limits = LoadLimits::default();
    let value = parse_json_value(json.as_bytes(), &limits)
        .map_err(|err| crate::Error::Validation(err.to_string()))?;
    deserialize_json_value(value, &limits).map_err(|err| crate::Error::Validation(err.to_string()))
}

fn deserialize_unique_tags<'de, D>(deserializer: D) -> Result<IndexMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UniqueTagsVisitor;

    impl<'de> Visitor<'de> for UniqueTagsVisitor {
        type Value = IndexMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a map of unique tag keys to string values")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut tags = IndexMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                validate_tag_key(&key).map_err(serde::de::Error::custom)?;
                if tags.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "Duplicate hierarchy tag key '{key}'"
                    )));
                }
                tags.insert(key, value);
            }
            Ok(tags)
        }
    }

    deserializer.deserialize_map(UniqueTagsVisitor)
}

/// Named child or root entry preserving authored sequence order at a hierarchy level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HierarchyNodeEntry {
    /// Exact node segment name at this hierarchy level.
    pub name: String,
    /// Node metadata tags, child entries, and typed market-data associations.
    pub node: HierarchyNode,
}

/// A single node within [`MarketDataHierarchy`].
///
/// Nodes can contain tags (key-value pairs for filtering), ordered child entries,
/// and typed [`MarketDataTarget`] associations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HierarchyNode {
    /// Metadata tags for tag-filter selection.
    #[serde(
        default,
        skip_serializing_if = "IndexMap::is_empty",
        deserialize_with = "deserialize_unique_tags"
    )]
    pub(super) tags: IndexMap<String, String>,
    /// Ordered child entries at this level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) children: Vec<HierarchyNodeEntry>,
    /// Typed market-data targets stored directly at this node in authored order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) targets: Vec<MarketDataTarget>,
}

impl HierarchyNode {
    /// Create an empty node.
    fn new() -> Self {
        Self::default()
    }

    /// Tags attached to this node (key-value metadata).
    #[must_use]
    pub fn tags(&self) -> &IndexMap<String, String> {
        &self.tags
    }

    /// Ordered child entries directly under this node.
    #[must_use]
    pub fn children(&self) -> &[HierarchyNodeEntry] {
        &self.children
    }

    /// Look up a direct child node by its exact segment name.
    ///
    /// # Arguments
    ///
    /// * `name` - Exact child segment name at this node.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&HierarchyNode> {
        self.children
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.node)
    }

    /// Typed market-data targets associated directly with this node.
    #[must_use]
    pub fn targets(&self) -> &[MarketDataTarget] {
        &self.targets
    }

    /// Mutable lookup of a direct child node by exact segment name.
    fn child_mut(&mut self, name: &str) -> Option<&mut HierarchyNode> {
        self.children
            .iter_mut()
            .find(|entry| entry.name == name)
            .map(|entry| &mut entry.node)
    }

    /// Set a tag on this node.
    pub(crate) fn set_tag(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.tags.insert(key.into(), value.into());
    }

    /// Get or create a child node by exact name, preserving insertion order.
    pub(crate) fn get_or_create_child(&mut self, name: &str) -> &mut HierarchyNode {
        if let Some(pos) = self.children.iter().position(|entry| entry.name == name) {
            return &mut self.children[pos].node;
        }
        self.children.push(HierarchyNodeEntry {
            name: name.to_string(),
            node: HierarchyNode::new(),
        });
        let last = self.children.len() - 1;
        &mut self.children[last].node
    }

    /// Append a typed market-data target to this node.
    pub(crate) fn add_target(&mut self, target: MarketDataTarget) {
        self.targets.push(target);
    }
}

/// Top-level market-data hierarchy containing ordered root entries.
///
/// Each root represents a major organizational branch (such as `"Rates"`,
/// `"Credit"`, `"FX"`, `"Equity"`, or `"Volatility"`). Roots and child levels
/// use explicit name/node entry arrays so numeric-looking names retain their
/// authored sequence across Rust, Python, and JavaScript.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarketDataHierarchy {
    /// Top-level ordered `(name, node)` entries.
    pub(crate) roots: Vec<HierarchyNodeEntry>,
}

/// Deserialize through the raw entry-array shape, then validate the whole tree.
impl<'de> Deserialize<'de> for MarketDataHierarchy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            roots: Vec<HierarchyNodeEntry>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let hierarchy = MarketDataHierarchy { roots: raw.roots };
        hierarchy.validate().map_err(serde::de::Error::custom)?;
        Ok(hierarchy)
    }
}

impl MarketDataHierarchy {
    /// Start building a hierarchy with the fluent builder API.
    #[must_use]
    pub fn builder() -> HierarchyBuilder {
        HierarchyBuilder::new()
    }

    /// Ordered root entries at the top level of the hierarchy.
    #[must_use]
    pub fn roots(&self) -> &[HierarchyNodeEntry] {
        &self.roots
    }

    /// Look up a node by exact segment path. Returns `None` if any segment is missing.
    ///
    /// # Arguments
    ///
    /// * `path` - Ordered path segments from root to target node.
    #[must_use]
    pub fn get_node(&self, path: &[String]) -> Option<&HierarchyNode> {
        let mut path_iter = path.iter();
        let root_name = path_iter.next()?;
        let mut current = self
            .roots
            .iter()
            .find(|entry| entry.name == *root_name)
            .map(|entry| &entry.node)?;
        for segment in path_iter {
            current = current.child(segment)?;
        }
        Some(current)
    }

    /// Mutable lookup or creation helper for root entries.
    pub(crate) fn get_or_create_root_mut(&mut self, name: &str) -> &mut HierarchyNode {
        if let Some(pos) = self.roots.iter().position(|entry| entry.name == name) {
            return &mut self.roots[pos].node;
        }
        self.roots.push(HierarchyNodeEntry {
            name: name.to_string(),
            node: HierarchyNode::new(),
        });
        let last = self.roots.len() - 1;
        &mut self.roots[last].node
    }

    /// Mutable lookup by exact segment path.
    pub(crate) fn get_node_mut(&mut self, path: &[String]) -> Option<&mut HierarchyNode> {
        let mut path_iter = path.iter();
        let root_name = path_iter.next()?;
        let mut current = self
            .roots
            .iter_mut()
            .find(|entry| entry.name == *root_name)
            .map(|entry| &mut entry.node)?;
        for segment in path_iter {
            current = current.child_mut(segment)?;
        }
        Some(current)
    }

    /// Deserialize a hierarchy from UTF-8 JSON with duplicate-key, nesting-depth,
    /// and structural validation.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical JSON representation of a [`MarketDataHierarchy`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if the JSON contains duplicate object
    /// or tag keys, exceeds contract resource/depth limits, has unknown fields,
    /// or violates hierarchy structural invariants.
    pub fn from_json(json: &str) -> crate::Result<Self> {
        parse_contract_json(json)
    }

    /// Serialize the hierarchy to pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if JSON serialization fails.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self).map_err(|err| {
            crate::Error::Validation(format!("Failed to serialize hierarchy: {err}"))
        })
    }

    /// Validate structural hierarchy invariants; run by deserialization and by
    /// [`HierarchyBuilder::build`].
    ///
    /// A valid hierarchy has non-blank node names without `'/'`, unique sibling
    /// names at every level, non-blank tag keys, valid [`MarketDataTarget`]
    /// identifiers, and at most one occurrence of each exact typed
    /// [`MarketDataTarget`] across the entire tree.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid segments, duplicate root or sibling names,
    /// blank tag keys, invalid targets, or duplicate exact [`MarketDataTarget`]
    /// entries.
    pub fn validate(&self) -> crate::Result<()> {
        fn visit(
            entry: &HierarchyNodeEntry,
            path: &mut Vec<String>,
            seen_targets: &mut FxHashMap<MarketDataTarget, String>,
        ) -> crate::Result<()> {
            validate_segment(&entry.name)?;
            path.push(entry.name.clone());
            let current_path = path.join("/");

            for key in entry.node.tags().keys() {
                validate_tag_key(key)?;
            }

            for target in entry.node.targets() {
                target.validate()?;
                if let Some(existing_path) =
                    seen_targets.insert(target.clone(), current_path.clone())
                {
                    return Err(crate::Error::Validation(format!(
                        "MarketDataTarget '{}:{}' appears in multiple hierarchy locations: {}, {}",
                        target.kind_name(),
                        target.id().as_str(),
                        existing_path,
                        current_path
                    )));
                }
            }

            let mut sibling_names = FxHashSet::default();
            for child in entry.node.children() {
                validate_segment(&child.name)?;
                if !sibling_names.insert(child.name.as_str()) {
                    return Err(crate::Error::Validation(format!(
                        "Duplicate child hierarchy node '{}' under '{}'",
                        child.name, current_path
                    )));
                }
            }

            for child in entry.node.children() {
                visit(child, path, seen_targets)?;
            }

            path.pop();
            Ok(())
        }

        let mut root_names = FxHashSet::default();
        for root in &self.roots {
            validate_segment(&root.name)?;
            if !root_names.insert(root.name.as_str()) {
                return Err(crate::Error::Validation(format!(
                    "Duplicate root hierarchy node '{}'",
                    root.name
                )));
            }
        }

        let mut seen_targets = FxHashMap::default();
        let mut path = Vec::new();
        for root in &self.roots {
            visit(root, &mut path, &mut seen_targets)?;
        }
        Ok(())
    }
}
