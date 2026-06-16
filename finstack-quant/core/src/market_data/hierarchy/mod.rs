//! Market data hierarchy for organizational grouping of curves.
//!
//! Provides a tree structure where each node can hold tags (key-value metadata),
//! child nodes, and leaf references (`CurveId`s) pointing into `MarketContext`'s
//! flat storage. The hierarchy enables:
//!
//! - **Scenario targeting**: Define shocks at any tree level; resolve to per-curve operations.
//! - **Factor model mapping**: Factor models reference hierarchy nodes for scope.
//! - **Completeness tracking**: Compare declared vs. present curves.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
//!
//! let hierarchy = MarketDataHierarchy::builder()
//!     .add_node("Rates/USD/OIS")
//!         .curve_ids(&["USD-OIS"])
//!     .add_node("Credit/US/IG/Financials")
//!         .tag("sector", "Financials")
//!         .curve_ids(&["JPM-5Y", "GS-5Y"])
//!     .build()
//!     .expect("valid hierarchy");
//! ```

mod builder;
mod completeness;
mod resolution;

pub use builder::HierarchyBuilder;
pub use completeness::{CompletenessReport, SubtreeCoverage};
pub use resolution::{HierarchyTarget, ResolutionMode, TagFilter, TagPredicate};

use crate::collections::HashMap;
use crate::types::CurveId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub(crate) fn parse_path(path: &str) -> crate::Result<Vec<String>> {
    if path.is_empty() {
        return Err(crate::Error::Validation(
            "Hierarchy paths must not be empty".to_string(),
        ));
    }

    let segments: Vec<String> = path.split('/').map(String::from).collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(crate::Error::Validation(format!(
            "Hierarchy path '{path}' must not contain empty segments"
        )));
    }

    Ok(segments)
}

/// A single node in the market data hierarchy tree.
///
/// Nodes form a tree: each has a name, optional key-value tags for cross-cutting
/// queries, ordered children, and leaf `CurveId` references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyNode {
    /// Node name. Populated from the parent map key during deserialization.
    /// Skipped during serde to match the spec's JSON format where the name
    /// is the map key (e.g., `{ "Rates": { "children": { "USD": ... } } }`).
    #[serde(skip)]
    name: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    tags: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    children: IndexMap<String, HierarchyNode>,
    #[serde(default, rename = "curves", skip_serializing_if = "Vec::is_empty")]
    curve_ids: Vec<CurveId>,
}

impl HierarchyNode {
    /// Create a new node with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tags: HashMap::default(),
            children: IndexMap::new(),
            curve_ids: Vec::new(),
        }
    }

    /// Node display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tags attached to this node (key-value metadata).
    pub fn tags(&self) -> &HashMap<String, String> {
        &self.tags
    }

    /// Ordered child nodes.
    pub fn children(&self) -> &IndexMap<String, HierarchyNode> {
        &self.children
    }

    /// Leaf `CurveId` references at this node.
    pub fn curve_ids(&self) -> &[CurveId] {
        &self.curve_ids
    }

    /// Collect all `CurveId`s in this subtree (this node + all descendants).
    pub fn all_curve_ids(&self) -> Vec<CurveId> {
        let mut ids = self.curve_ids.clone();
        for child in self.children.values() {
            ids.extend(child.all_curve_ids());
        }
        ids
    }

    /// Set a tag on this node.
    pub fn set_tag(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.tags.insert(key.into(), value.into());
    }

    /// Add a child node. Returns a mutable reference to the child.
    ///
    /// If a child with the same name already exists, the existing node is
    /// returned unchanged and `child` is discarded.
    pub fn add_child(&mut self, child: HierarchyNode) -> &mut HierarchyNode {
        let name = child.name.clone();
        self.children.entry(name).or_insert(child)
    }

    /// Get or create a child node by name.
    pub fn get_or_create_child(&mut self, name: &str) -> &mut HierarchyNode {
        self.children
            .entry(name.to_string())
            .or_insert_with(|| HierarchyNode::new(name))
    }

    /// Add a curve ID to this node's leaf set.
    pub fn add_curve_id(&mut self, id: impl Into<CurveId>) {
        self.curve_ids.push(id.into());
    }
}

/// The top-level market data hierarchy containing root nodes.
///
/// Each root represents a major asset class or category (e.g., "Rates", "Credit",
/// "FX", "Equity", "Volatility"). The hierarchy is fully serializable and can be
/// loaded from JSON configuration files.
#[derive(Debug, Clone, Serialize, Default)]
pub struct MarketDataHierarchy {
    pub(crate) roots: IndexMap<String, HierarchyNode>,
}

/// Custom deserialize that populates each node's `name` field from its map key.
impl<'de> Deserialize<'de> for MarketDataHierarchy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            roots: IndexMap<String, HierarchyNode>,
        }
        let mut raw = Raw::deserialize(deserializer)?;
        // Recursively set name from map key
        fn fixup_names(children: &mut IndexMap<String, HierarchyNode>) {
            for (key, node) in children.iter_mut() {
                node.name = key.clone();
                fixup_names(&mut node.children);
            }
        }
        fixup_names(&mut raw.roots);
        let hierarchy = MarketDataHierarchy { roots: raw.roots };
        hierarchy.validate().map_err(serde::de::Error::custom)?;
        Ok(hierarchy)
    }
}

impl MarketDataHierarchy {
    /// Create an empty hierarchy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start building a hierarchy with the fluent builder API.
    #[must_use]
    pub fn builder() -> HierarchyBuilder {
        HierarchyBuilder::new()
    }

    /// Root nodes of the hierarchy.
    pub fn roots(&self) -> &IndexMap<String, HierarchyNode> {
        &self.roots
    }

    /// Mutable access to root nodes.
    pub fn roots_mut(&mut self) -> &mut IndexMap<String, HierarchyNode> {
        &mut self.roots
    }

    /// Look up a node by path. Returns `None` if the path doesn't exist.
    pub fn get_node(&self, path: &[String]) -> Option<&HierarchyNode> {
        let mut path_iter = path.iter();
        let root_name = path_iter.next()?;
        let mut current = self.roots.get(root_name.as_str())?;
        for segment in path_iter {
            current = current.children.get(segment.as_str())?;
        }
        Some(current)
    }

    /// Mutable lookup by path.
    pub fn get_node_mut(&mut self, path: &[String]) -> Option<&mut HierarchyNode> {
        let mut path_iter = path.iter();
        let root_name = path_iter.next()?;
        let mut current = self.roots.get_mut(root_name.as_str())?;
        for segment in path_iter {
            current = current.children.get_mut(segment.as_str())?;
        }
        Some(current)
    }

    /// Collect all `CurveId`s across the entire hierarchy.
    pub fn all_curve_ids(&self) -> Vec<CurveId> {
        let mut ids = Vec::new();
        for root in self.roots.values() {
            ids.extend(root.all_curve_ids());
        }
        ids
    }

    /// Insert a curve at a `/`-separated path, creating intermediate nodes as needed.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` is malformed or if `curve_id` already exists
    /// somewhere else in the hierarchy.
    pub fn insert_curve(&mut self, path: &str, curve_id: impl Into<CurveId>) -> crate::Result<()> {
        let segments = parse_path(path)?;
        let curve_id = curve_id.into();

        if let Some(existing_path) = self.path_for_curve(&curve_id) {
            return Err(crate::Error::Validation(format!(
                "CurveId '{}' already exists in hierarchy at '{}'",
                curve_id.as_str(),
                existing_path.join("/")
            )));
        }

        let root_name = &segments[0];

        let root = self
            .roots
            .entry(root_name.to_string())
            .or_insert_with(|| HierarchyNode::new(root_name));

        let mut current = root;
        for segment in &segments[1..] {
            current = current.get_or_create_child(segment);
        }

        current.curve_ids.push(curve_id);
        Ok(())
    }

    /// Remove a curve from wherever it sits in the tree. Returns `true` if found.
    pub fn remove_curve(&mut self, curve_id: &CurveId) -> bool {
        fn remove_from_node(node: &mut HierarchyNode, target: &CurveId) -> bool {
            if let Some(pos) = node.curve_ids.iter().position(|id| id == target) {
                node.curve_ids.remove(pos);
                return true;
            }
            for child in node.children.values_mut() {
                if remove_from_node(child, target) {
                    return true;
                }
            }
            false
        }

        for root in self.roots.values_mut() {
            if remove_from_node(root, curve_id) {
                return true;
            }
        }
        false
    }

    /// Find the path from root to a specific curve. Returns `None` if not found.
    pub fn path_for_curve(&self, curve_id: &CurveId) -> Option<Vec<String>> {
        fn find_in_node(node: &HierarchyNode, target: &CurveId, path: &mut Vec<String>) -> bool {
            path.push(node.name().to_string());
            if node.curve_ids.iter().any(|id| id == target) {
                return true;
            }
            for child in node.children.values() {
                if find_in_node(child, target, path) {
                    return true;
                }
            }
            path.pop();
            false
        }

        for root in self.roots.values() {
            let mut path = Vec::new();
            if find_in_node(root, curve_id, &mut path) {
                return Some(path);
            }
        }
        None
    }

    /// Set a tag on a node at the given `/`-separated path.
    pub fn set_tag(&mut self, path: &str, key: &str, value: &str) -> crate::Result<()> {
        let segments = parse_path(path)?;
        let node = self.get_node_mut(&segments).ok_or_else(|| {
            crate::Error::Validation(format!(
                "Hierarchy path '{}' does not exist",
                segments.join("/")
            ))
        })?;
        node.set_tag(key, value);
        Ok(())
    }

    /// Validate structural hierarchy invariants.
    ///
    /// This is primarily useful after performing low-level manual mutations via
    /// `get_node_mut`, `get_or_create_child`, or `add_curve_id`.
    pub fn validate(&self) -> crate::Result<()> {
        fn visit(
            node: &HierarchyNode,
            expected_name: &str,
            path: &mut Vec<String>,
            seen: &mut HashMap<CurveId, String>,
        ) -> crate::Result<()> {
            if expected_name.is_empty() || node.name().is_empty() {
                return Err(crate::Error::Validation(
                    "Hierarchy node names must not be empty".to_string(),
                ));
            }
            if node.name() != expected_name {
                return Err(crate::Error::Validation(format!(
                    "Hierarchy node name '{}' does not match its map key '{}'",
                    node.name(),
                    expected_name
                )));
            }

            path.push(node.name().to_string());
            let current_path = path.join("/");

            for curve_id in node.curve_ids() {
                if let Some(existing_path) = seen.insert(curve_id.clone(), current_path.clone()) {
                    return Err(crate::Error::Validation(format!(
                        "CurveId '{}' appears in multiple hierarchy locations: {}, {}",
                        curve_id.as_str(),
                        existing_path,
                        current_path
                    )));
                }
            }

            for (child_name, child) in node.children() {
                visit(child, child_name, path, seen)?;
            }

            path.pop();
            Ok(())
        }

        let mut seen = HashMap::default();
        let mut path = Vec::new();
        for (root_name, root) in &self.roots {
            visit(root, root_name, &mut path, &mut seen)?;
        }
        Ok(())
    }
}
