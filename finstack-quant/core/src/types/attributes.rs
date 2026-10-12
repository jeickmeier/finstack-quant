//! User-defined tags and key-value metadata attached to instruments.
//!
//! Tags are set-like labels; metadata is a string-to-string map. Selectors
//! support the documented tag and metadata forms and return false for unknown
//! syntax.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// User-defined tags and key-value metadata for instrument classification.
///
/// Tags are a sorted set of labels. Metadata is a sorted string-to-string map.
/// Both fields are public so callers can insert, remove, and iterate entries
/// directly. [`Attributes::add_tag`] and [`Attributes::set_meta`] are the
/// in-place writers; [`Attributes::with_tag`] and [`Attributes::with_meta`]
/// return the bag for construction chains.
///
/// [`Attributes::matches_selector`] accepts three forms:
///
/// - `"*"` matches every attribute set
/// - `"tag:<name>"` matches when that tag is present
/// - `"meta:<key>=<value>"` matches an exact metadata entry
///
/// Any other selector text returns `false`.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::types::Attributes;
///
/// let attrs = Attributes::new()
///     .with_tag("energy")
///     .with_meta("region", "NA");
/// assert!(attrs.has_tag("energy"));
/// assert_eq!(attrs.get_meta("region"), Some("NA"));
/// assert!(attrs.matches_selector("meta:region=NA"));
/// assert!(!attrs.matches_selector("sector:energy"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// User-defined tags for categorization.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<String>,
    /// Structured metadata associated with the instrument.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, String>,
}

impl Attributes {
    /// Create an empty set of attributes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a tag, leaving the set unchanged when the tag is already present.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag label stored exactly as given, including the empty string.
    ///   Matching is case-sensitive, and inserting a tag that is already present
    ///   does not change the set.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.insert(tag.into());
    }

    /// Add a single tag and return the updated attributes.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag label stored exactly as given, including the empty string.
    ///   Matching is case-sensitive, and a tag that is already present is ignored.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.add_tag(tag);
        self
    }

    /// Insert or replace a metadata entry.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key stored exactly as given. An existing entry under
    ///   this key is replaced.
    /// * `value` - Metadata text stored exactly as given, with no numeric or
    ///   percentage parsing.
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.meta.insert(key.into(), value.into());
    }

    /// Add a metadata entry and return the updated attributes.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key stored exactly as given. An existing entry under
    ///   this key is replaced.
    /// * `value` - Metadata text stored exactly as given, with no numeric or
    ///   percentage parsing.
    #[must_use]
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.set_meta(key, value);
        self
    }

    /// Check whether a tag is present.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag label to look up. Matching is exact and case-sensitive.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Look up a metadata value by key.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key to look up. Matching is exact and case-sensitive.
    ///
    /// # Returns
    ///
    /// The stored text when the key is present, or `None` when it is absent.
    #[must_use]
    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(String::as_str)
    }

    /// Check whether a metadata key is present.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key to look up. Matching is exact and case-sensitive.
    ///   An empty stored value still counts as present.
    #[must_use]
    pub fn contains_meta_key(&self, key: &str) -> bool {
        self.meta.contains_key(key)
    }

    /// Match the attributes against a selector string.
    ///
    /// # Arguments
    ///
    /// * `selector` - `"*"` matches every set, `"tag:<name>"` matches a tag,
    ///   and `"meta:<key>=<value>"` matches an exact metadata entry. Any other
    ///   text, including a missing `=`, returns `false`.
    #[must_use]
    pub fn matches_selector(&self, selector: &str) -> bool {
        if selector == "*" {
            return true;
        }

        if let Some(tag) = selector.strip_prefix("tag:") {
            return self.has_tag(tag);
        }

        if let Some(meta_spec) = selector.strip_prefix("meta:") {
            if let Some((key, value)) = meta_spec.split_once('=') {
                return self.get_meta(key) == Some(value);
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_default_is_empty() {
        let attrs = Attributes::default();
        assert!(attrs.tags.is_empty());
        assert!(attrs.meta.is_empty());
    }

    #[test]
    fn test_attributes_builder_methods() {
        let attrs = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA")
            .with_meta("rating", "CCC");

        assert!(attrs.has_tag("energy"));
        assert!(!attrs.has_tag("financials"));
        assert_eq!(attrs.get_meta("region"), Some("NA"));
        assert_eq!(attrs.get_meta("rating"), Some("CCC"));
        assert_eq!(attrs.get_meta("nonexistent"), None);
    }

    #[test]
    fn test_add_tag_is_idempotent() {
        let mut attrs = Attributes::new();
        attrs.add_tag("a");
        attrs.add_tag("a");
        attrs.add_tag("b");
        assert!(attrs.has_tag("a"));
        assert!(attrs.has_tag("b"));
        assert_eq!(attrs.tags.len(), 2);
    }

    #[test]
    fn test_attributes_serde_roundtrip() {
        let attrs = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA");

        let json = serde_json::to_string(&attrs).expect("attributes serialize");
        let deserialized: Attributes = serde_json::from_str(&json).expect("attributes deserialize");

        assert_eq!(attrs, deserialized);
    }

    #[test]
    fn matches_selector_supports_wildcard_tags_and_meta_equality() {
        let attrs = Attributes::default()
            .with_tag("energy")
            .with_tag("distressed")
            .with_meta("region", "NA")
            .with_meta("rating", "CCC");

        assert!(attrs.matches_selector("*"));
        assert!(attrs.matches_selector("tag:energy"));
        assert!(attrs.matches_selector("tag:distressed"));
        assert!(attrs.matches_selector("meta:region=NA"));
        assert!(attrs.matches_selector("meta:rating=CCC"));

        assert!(!attrs.matches_selector("tag:financials"));
        assert!(!attrs.matches_selector("meta:region=EMEA"));
        assert!(!attrs.matches_selector("meta:missing=NA"));
    }

    #[test]
    fn matches_selector_rejects_malformed_or_unknown_selectors() {
        let attrs = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA");

        assert!(!attrs.matches_selector(""));
        assert!(!attrs.matches_selector("energy"));
        assert!(!attrs.matches_selector("meta:region"));
        assert!(!attrs.matches_selector("sector:energy"));
        assert!(!attrs.matches_selector("tag:"));
        assert!(!attrs.matches_selector("meta:=NA"));
    }
}
