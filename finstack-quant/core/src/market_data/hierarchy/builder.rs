//! Fluent builder for constructing hierarchies from `/`-separated paths.

use super::{parse_path, HierarchyNode, MarketDataHierarchy};

/// Fluent builder for `MarketDataHierarchy`.
///
/// Uses `/`-separated paths to auto-create intermediate nodes:
///
/// ```rust
/// use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
///
/// let h = MarketDataHierarchy::builder()
///     .add_node("Rates/USD/OIS").curve_ids(&["USD-OIS"])
///     .build()
///     .unwrap();
/// ```
pub struct HierarchyBuilder {
    hierarchy: MarketDataHierarchy,
    current_path: Option<Vec<String>>,
    validation_error: Option<crate::Error>,
}

impl HierarchyBuilder {
    pub(super) fn new() -> Self {
        Self {
            hierarchy: MarketDataHierarchy::new(),
            current_path: None,
            validation_error: None,
        }
    }

    /// Start or switch to a node at the given `/`-separated path.
    /// Creates intermediate nodes as needed.
    ///
    pub fn add_node(mut self, path: &str) -> Self {
        let segments = match parse_path(path) {
            Ok(segments) => segments,
            Err(err) => {
                if self.validation_error.is_none() {
                    self.validation_error = Some(err);
                }
                self.current_path = None;
                return self;
            }
        };

        // Ensure the root exists
        let root_name = segments[0].clone();
        if !self.hierarchy.roots.contains_key(root_name.as_str()) {
            self.hierarchy
                .roots
                .insert(root_name.clone(), HierarchyNode::new(root_name.as_str()));
        }

        // Navigate down creating children as needed — we avoid unwrap by using
        // `get_or_create_child` which always returns a valid mutable reference.
        if let Some(root) = self.hierarchy.roots.get_mut(root_name.as_str()) {
            let mut current = root;
            for segment in &segments[1..] {
                current = current.get_or_create_child(segment);
            }
        }

        self.current_path = Some(segments);
        self
    }

    /// Set a tag on the current node.
    ///
    /// No-op if `add_node` has not been called first.
    pub fn tag(mut self, key: &str, value: &str) -> Self {
        if let Some(path) = self.current_path.clone() {
            if let Some(node) = self.hierarchy.get_node_mut(&path) {
                node.set_tag(key, value);
            }
        }
        self
    }

    /// Add curve IDs to the current node.
    ///
    /// No-op if `add_node` has not been called first.
    pub fn curve_ids(mut self, ids: &[&str]) -> Self {
        if let Some(path) = self.current_path.clone() {
            if let Some(node) = self.hierarchy.get_node_mut(&path) {
                for id in ids {
                    node.add_curve_id(*id);
                }
            }
        }
        self
    }

    /// Finalize and validate the hierarchy.
    ///
    /// Validates:
    /// - No duplicate `CurveId` across different nodes (each curve has exactly one path).
    pub fn build(self) -> crate::Result<MarketDataHierarchy> {
        if let Some(err) = self.validation_error {
            return Err(err);
        }

        self.hierarchy.validate()?;
        Ok(self.hierarchy)
    }
}
