//! Fluent builder for constructing hierarchies from `/`-separated paths.

use super::{MarketDataHierarchy, parse_path, validate_tag_key};
use crate::market_data::bumps::MarketDataTarget;

/// Fluent builder for [`MarketDataHierarchy`].
///
/// Uses `/`-separated paths to auto-create intermediate nodes in insertion
/// order:
///
/// ```rust
/// use finstack_quant_core::market_data::bumps::MarketDataTarget;
/// use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
/// use finstack_quant_core::types::CurveId;
///
/// let h = MarketDataHierarchy::builder()
///     .add_node("Rates/USD/OIS")
///         .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
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
            hierarchy: MarketDataHierarchy::default(),
            current_path: None,
            validation_error: None,
        }
    }

    fn record_error(&mut self, err: crate::Error) {
        if self.validation_error.is_none() {
            self.validation_error = Some(err);
        }
    }

    /// Start or switch to a node at the given `/`-separated path.
    ///
    /// Creates missing intermediate nodes in insertion order and clears the
    /// active target node if `path` fails segment validation. Any error from an
    /// earlier builder call remains sticky until [`Self::build`].
    ///
    /// # Arguments
    ///
    /// * `path` - Hierarchical market-data path identifying the insertion or lookup location.
    pub fn add_node(mut self, path: &str) -> Self {
        let segments = match parse_path(path) {
            Ok(segments) => segments,
            Err(err) => {
                self.record_error(err);
                self.current_path = None;
                return self;
            }
        };

        let mut current = self.hierarchy.get_or_create_root_mut(&segments[0]);
        for segment in &segments[1..] {
            current = current.get_or_create_child(segment);
        }

        self.current_path = Some(segments);
        self
    }

    /// Set a metadata tag on the active node.
    ///
    /// # Arguments
    ///
    /// * `key` - Non-blank tag key attached to the active node.
    /// * `value` - Tag value string associated with `key`.
    pub fn tag(mut self, key: &str, value: &str) -> Self {
        if let Err(err) = validate_tag_key(key) {
            self.record_error(err);
            return self;
        }
        let Some(path) = self.current_path.as_ref() else {
            self.record_error(crate::Error::Validation(
                "Cannot set hierarchy tag without an active node; call add_node first".to_string(),
            ));
            return self;
        };
        if let Some(node) = self.hierarchy.get_node_mut(path) {
            node.set_tag(key, value);
        } else {
            self.record_error(crate::Error::Validation(format!(
                "Active hierarchy node '{}' does not exist",
                path.join("/")
            )));
        }
        self
    }

    /// Add one typed [`MarketDataTarget`] to the active node.
    ///
    /// # Arguments
    ///
    /// * `target` - Typed market-data target with a non-blank identifier.
    pub fn target(self, target: MarketDataTarget) -> Self {
        self.targets(std::iter::once(target))
    }

    /// Add typed [`MarketDataTarget`] entries to the active node in order.
    ///
    /// # Arguments
    ///
    /// * `targets` - Ordered sequence of typed market-data targets to attach to
    ///   the active node.
    pub fn targets<I>(mut self, targets: I) -> Self
    where
        I: IntoIterator<Item = MarketDataTarget>,
    {
        let Some(path) = self.current_path.as_ref() else {
            self.record_error(crate::Error::Validation(
                "Cannot add hierarchy targets without an active node; call add_node first"
                    .to_string(),
            ));
            return self;
        };
        let Some(node) = self.hierarchy.get_node_mut(path) else {
            self.record_error(crate::Error::Validation(format!(
                "Active hierarchy node '{}' does not exist",
                path.join("/")
            )));
            return self;
        };
        for target in targets {
            if let Err(err) = target.validate() {
                self.record_error(err);
                return self;
            }
            node.add_target(target);
        }
        self
    }

    /// Finalize and validate the hierarchy.
    ///
    /// Paths accumulated through [`Self::add_node`] retain their insertion order
    /// at every root and child level.
    ///
    /// # Errors
    ///
    /// Returns the first deferred builder error (from [`Self::add_node`],
    /// [`Self::tag`], [`Self::target`], or [`Self::targets`]) or a structural
    /// validation error such as a duplicate exact [`MarketDataTarget`] across
    /// multiple hierarchy locations.
    pub fn build(self) -> crate::Result<MarketDataHierarchy> {
        if let Some(err) = self.validation_error {
            return Err(err);
        }

        self.hierarchy.validate()?;
        Ok(self.hierarchy)
    }
}
