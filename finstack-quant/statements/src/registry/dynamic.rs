//! Dynamic metric registry implementation.

use crate::error::{Error, Result};
use crate::registry::schema::{MetricDefinition, MetricRegistry};
use crate::registry::validation::validate_metric_definition;
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;

/// Dynamic registry for metric definitions.
///
/// Stores metrics organized by namespace and provides lookup, validation,
/// and compilation services so metric formulas can be reused across models.
///
/// # Thread safety
///
/// `Registry` is **not** internally synchronised. Build the registry once at
/// startup (typically via [`Registry::with_builtins`] plus any caller-loaded
/// JSON), then share it as `Arc<Registry>` for concurrent read access — the
/// underlying `IndexMap` lookups are safe to call from multiple threads as
/// long as no thread is mutating. If you need concurrent mutation, wrap the
/// registry in `RwLock` at the call site rather than baking a lock into the
/// type, since the common case (build-once-share-many) does not need one.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Map of fully-qualified metric ID → metric definition
    metrics: IndexMap<String, StoredMetric>,

    /// Set of all namespaces
    namespaces: HashSet<String>,
}

/// Stored metric.
#[derive(Debug, Clone)]
pub struct StoredMetric {
    /// Namespace
    pub namespace: String,

    /// Metric definition
    pub definition: MetricDefinition,
}

impl Registry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            metrics: IndexMap::new(),
            namespaces: HashSet::new(),
        }
    }

    /// Create a new registry preloaded with built-in metrics (fin.* namespace).
    ///
    /// This is a shortcut for `Registry::new().load_builtins()` that avoids the
    /// two-step pattern in call sites.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_statements::registry::Registry;
    ///
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let registry = Registry::with_builtins()?;
    /// assert!(registry.has("fin.gross_profit"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if an embedded metric document cannot be parsed,
    /// validated, dependency-sorted, or added without colliding with an
    /// already-registered metric. The registry is left unchanged when loading
    /// any individual document fails.
    pub fn with_builtins() -> Result<Self> {
        let mut registry = Self::new();
        registry.load_builtins()?;
        Ok(registry)
    }

    /// Load built-in metrics (fin.* namespace).
    ///
    /// This loads standard financial metrics from embedded JSON files.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_statements::registry::Registry;
    ///
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let mut registry = Registry::new();
    /// registry.load_builtins()?;
    ///
    /// assert!(registry.has("fin.gross_profit"));
    /// assert!(registry.has("fin.gross_margin"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Built-ins are compile-time embedded, so this method performs no
    /// filesystem lookup and works in packaged and WASM builds. All standard
    /// metrics share the `fin` namespace and may reference one another.
    ///
    /// # Errors
    ///
    /// Returns an error if an embedded document is malformed, a metric has an
    /// invalid identifier or formula, its dependency graph is cyclic, or one
    /// of its fully-qualified identifiers already exists. Each rejected
    /// document leaves the registry unchanged, although documents loaded
    /// before it remain available.
    pub fn load_builtins(&mut self) -> Result<()> {
        // Load from embedded JSON files
        for json in crate::registry::builtins::builtin_metric_sources()? {
            self.load_from_json_str(&json)?;
        }
        Ok(())
    }

    /// Load one metric registry document from a UTF-8 JSON file.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_statements::registry::Registry;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// registry.load_from_json("metrics/custom.json")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The file must encode a [`MetricRegistry`]. Its namespace is part of
    /// every stored metric identity, so a `gross_margin` definition in `fin`
    /// and one in `custom` are distinct metrics addressed as
    /// `fin.gross_margin` and `custom.gross_margin`.
    ///
    /// # Errors
    ///
    /// Propagates file-read failures and returns a registry error if the JSON
    /// is malformed, definitions or formulas are invalid, dependencies are
    /// cyclic, or a fully-qualified ID collides with an existing metric. A
    /// failed document is staged and rejected atomically; it cannot leave a
    /// partially loaded namespace behind.
    pub fn load_from_json(&mut self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        self.load_from_json_str(&json)?;
        Ok(())
    }

    /// Load one metric registry document from JSON text.
    ///
    /// Returns the deserialized [`MetricRegistry`]
    /// for further inspection when needed. This is useful at a binding or
    /// service boundary when the caller needs both the parsed metadata and the
    /// registry update from the same canonical input.
    ///
    /// # Errors
    ///
    /// Returns a JSON decode or registry error if the document is malformed,
    /// violates metric-definition rules, has a cyclic dependency graph, or
    /// collides with a registered fully-qualified ID. The registry is not
    /// mutated when validation of the document fails.
    pub fn load_from_json_str(&mut self, json: &str) -> Result<MetricRegistry> {
        let registry: MetricRegistry = serde_json::from_str(json)?;
        self.load_registry(registry.clone())?;
        Ok(registry)
    }

    /// Validate and atomically add a metric registry document.
    ///
    /// Validates all metrics and checks for collisions.
    /// Supports inter-metric dependencies: metrics in the same namespace are
    /// topologically ordered so every dependency is stored before its
    /// dependent. Definitions may also refer to metrics already loaded in the
    /// same namespace. No part of `registry` is committed unless every
    /// definition passes validation.
    ///
    /// # Errors
    ///
    /// Returns a registry error when the namespace is empty, a definition has
    /// an invalid ID, name, or DSL formula, the document contains a dependency
    /// cycle, or a fully-qualified metric ID already exists. On error, `self`
    /// retains its exact prior metrics and namespace set.
    pub fn load_registry(&mut self, registry: MetricRegistry) -> Result<()> {
        let namespace = registry.namespace.clone();

        // Validate namespace
        if namespace.is_empty() {
            return Err(Error::registry(
                "Namespace cannot be empty. Provide a namespace identifier (e.g., 'fin', 'custom')."
            ));
        }

        // Sort metrics by dependency order
        let sorted_metrics = self.sort_metrics_by_dependencies(&registry)?;

        // Validate and stage into a local buffer first, committing to `self`
        // only after the whole document passes. A failure part-way through must
        // not leave the registry half-mutated (a stale namespace, or some
        // metrics from a document that was rejected), which would otherwise
        // produce spurious "Duplicate metric ID" errors on a corrected retry.
        let mut staged: Vec<(String, StoredMetric)> = Vec::with_capacity(sorted_metrics.len());
        for metric in sorted_metrics {
            validate_metric_definition(&metric, &namespace)?;

            let qualified_id = metric.qualified_id(&namespace);
            if self.metrics.contains_key(&qualified_id)
                || staged.iter().any(|(id, _)| id == &qualified_id)
            {
                return Err(Error::registry(format!(
                    "Duplicate metric ID: '{}'. This metric is already registered in the registry.",
                    qualified_id
                )));
            }

            staged.push((
                qualified_id,
                StoredMetric {
                    namespace: namespace.clone(),
                    definition: metric,
                },
            ));
        }

        // Commit atomically.
        self.namespaces.insert(namespace);
        for (qualified_id, stored) in staged {
            self.metrics.insert(qualified_id, stored);
        }

        Ok(())
    }

    /// Get a metric by fully-qualified ID.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_statements::registry::Registry;
    ///
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let mut registry = Registry::new();
    /// registry.load_builtins()?;
    ///
    /// let metric = registry.get("fin.gross_margin")?;
    /// assert_eq!(metric.definition.formula, "gross_profit / revenue");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a registry error if `qualified_id` is absent. The diagnostic
    /// includes a small sample of registered identifiers to help callers
    /// distinguish a namespace mistake from an unloaded metric catalog.
    pub fn get(&self, qualified_id: &str) -> Result<&StoredMetric> {
        self.metrics.get(qualified_id).ok_or_else(|| {
            let available: Vec<_> = self.metrics.keys().take(5).map(|s| s.as_str()).collect();
            Error::registry(format!(
                "Metric not found: '{}'. Available metrics include: {}{}",
                qualified_id,
                available.join(", "),
                if self.metrics.len() > 5 { ", ..." } else { "" }
            ))
        })
    }

    /// Check if a metric exists.
    pub fn has(&self, qualified_id: &str) -> bool {
        self.metrics.contains_key(qualified_id)
    }

    /// List all metrics in a namespace.
    ///
    /// Returns an iterator over (qualified_id, metric).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_statements::registry::Registry;
    ///
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let mut registry = Registry::new();
    /// registry.load_builtins()?;
    ///
    /// let fin_metrics: Vec<_> = registry.namespace("fin").collect();
    /// assert!(fin_metrics.len() > 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn namespace<'a>(
        &'a self,
        namespace: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a StoredMetric)> + 'a {
        self.metrics
            .iter()
            .filter(move |(_id, m)| m.namespace == namespace)
            .map(|(id, m)| (id.as_str(), m))
    }

    /// List all namespaces.
    pub fn namespaces(&self) -> Vec<&str> {
        let mut namespaces: Vec<_> = self.namespaces.iter().map(|s| s.as_str()).collect();
        namespaces.sort();
        namespaces
    }

    /// List all metrics.
    pub fn all_metrics(&self) -> impl Iterator<Item = (&str, &StoredMetric)> {
        self.metrics.iter().map(|(id, m)| (id.as_str(), m))
    }

    /// Get the number of metrics.
    pub fn len(&self) -> usize {
        self.metrics.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }

    /// Sort metrics by dependency order using topological sort.
    ///
    /// This allows metrics to reference other metrics in the same registry.
    fn sort_metrics_by_dependencies(
        &self,
        registry: &MetricRegistry,
    ) -> Result<Vec<MetricDefinition>> {
        let namespace = &registry.namespace;

        // Build map of metric_id -> MetricDefinition for lookup.
        //
        // Collision must be detected here rather than left to `IndexMap`'s
        // last-wins insert: coalescing two same-id definitions into one entry
        // would silently discard a metric *before* `load_registry`'s duplicate
        // check runs, leaving that check able to catch only cross-document
        // collisions.
        let mut metric_map: IndexMap<String, MetricDefinition> =
            IndexMap::with_capacity(registry.metrics.len());
        for m in &registry.metrics {
            if metric_map.contains_key(&m.id) {
                return Err(Error::registry(format!(
                    "Duplicate metric ID: '{}' is defined more than once in namespace '{}'. \
                     Each metric ID must be unique within a document; keeping only one \
                     definition would silently discard the other.",
                    m.qualified_id(namespace),
                    namespace
                )));
            }
            metric_map.insert(m.id.to_owned(), m.clone());
        }

        // Build dependency graph: metric_id -> set of metrics it depends on
        let mut dependencies: IndexMap<String, IndexSet<String>> = IndexMap::new();

        // Collect already-loaded metrics from the same namespace (these are valid dependencies)
        let mut existing_metric_ids: IndexSet<String> = IndexSet::new();
        for (qualified_id, stored) in &self.metrics {
            if stored.namespace == *namespace {
                // Extract just the metric ID (without namespace prefix)
                if let Some(id) = qualified_id.strip_prefix(&format!("{}.", namespace)) {
                    existing_metric_ids.insert(id.to_string());
                }
            }
        }

        // Build the set of all metric IDs that can participate in dependency analysis:
        // - metrics that are already loaded in this namespace
        // - metrics that are being loaded from the current registry
        //
        // This allows us to:
        // - detect true circular dependencies between metrics in the same registry, and
        // - still treat references to previously loaded metrics as valid (external) dependencies.
        let mut all_metric_ids: IndexSet<String> = existing_metric_ids;
        for metric_id in metric_map.keys() {
            all_metric_ids.insert(metric_id.clone());
        }

        for (metric_id, metric) in &metric_map {
            let deps = self.extract_metric_dependencies(&metric.formula, &all_metric_ids)?;
            dependencies.insert(metric_id.to_owned(), deps);
        }

        let order = crate::utils::graph::toposort_ids(&dependencies).map_err(|remaining| {
            Error::registry(format!(
                "Circular dependency detected among metrics: {}",
                remaining.join(" -> ")
            ))
        })?;

        let sorted = order
            .into_iter()
            .map(|metric_id| {
                metric_map.get(&metric_id).cloned().ok_or_else(|| {
                    Error::registry(format!(
                        "internal error: metric '{}' missing from map despite dependency entry",
                        metric_id
                    ))
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(sorted)
    }

    /// Extract dependencies from a metric formula.
    ///
    /// Returns the set of metric IDs (unqualified) that this formula references.
    fn extract_metric_dependencies(
        &self,
        formula: &str,
        all_metric_ids: &IndexSet<String>,
    ) -> Result<IndexSet<String>> {
        crate::utils::formula::extract_identifiers(formula, all_metric_ids)
    }

    /// Get a metric's transitive dependencies in insertion order.
    ///
    /// Returns fully-qualified IDs in an order suitable for constructing a
    /// model: each dependency appears before the metric that consumes it. The
    /// requested metric itself is not included. Only dependencies present in
    /// the metric's namespace are returned; identifiers outside the registered
    /// set are treated as model inputs rather than registry metrics.
    ///
    /// # Errors
    ///
    /// Returns a registry error if `qualified_id` is unknown or if a stored
    /// formula cannot be parsed while its dependencies are being extracted.
    /// Valid registries loaded through [`load_registry`](Self::load_registry)
    /// normally avoid the latter case because formulas are checked at load
    /// time.
    pub fn get_metric_dependencies(&self, qualified_id: &str) -> Result<Vec<String>> {
        // Recursively get transitive dependencies
        let mut all_deps = IndexSet::new();
        let mut visited = IndexSet::new();
        self.collect_transitive_dependencies(qualified_id, &mut all_deps, &mut visited)?;

        // Return in dependency order (dependencies before dependents)
        Ok(all_deps.into_iter().collect())
    }

    /// Recursively collect transitive dependencies.
    fn collect_transitive_dependencies(
        &self,
        qualified_id: &str,
        all_deps: &mut IndexSet<String>,
        visited: &mut IndexSet<String>,
    ) -> Result<()> {
        // Avoid infinite loops
        if visited.contains(qualified_id) {
            return Ok(());
        }
        visited.insert(qualified_id.to_string());

        let metric = self.get(qualified_id)?;
        let namespace = &metric.namespace;

        // Get all metric IDs in this namespace
        let all_metric_ids: IndexSet<String> = self
            .namespace(namespace)
            .map(|(id, _)| {
                id.strip_prefix(&format!("{}.", namespace))
                    .unwrap_or(id)
                    .to_string()
            })
            .collect();

        // Extract direct dependencies
        let deps = self.extract_metric_dependencies(&metric.definition.formula, &all_metric_ids)?;

        // Recursively process each dependency
        for dep_id in deps {
            let dep_qualified = format!("{}.{}", namespace, dep_id);
            if self.has(&dep_qualified) {
                self.collect_transitive_dependencies(&dep_qualified, all_deps, visited)?;
                all_deps.insert(dep_qualified);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two metrics sharing an `id` inside one document must be rejected.
    ///
    /// `sort_metrics_by_dependencies` builds its lookup with
    /// `IndexMap::collect`, which overwrites on a duplicate key. That silently
    /// coalesced the pair into a single last-wins entry *before* the duplicate
    /// check ran, so the guard could only ever catch cross-document
    /// collisions. For a financial metric registry, silently discarding one of
    /// two conflicting definitions of the same metric is a data-integrity
    /// hazard.
    #[test]
    fn intra_document_duplicate_metric_ids_are_rejected() {
        let json = r#"{
            "namespace": "custom",
            "metrics": [
                {"id": "m1", "name": "First", "formula": "revenue * 1"},
                {"id": "m1", "name": "Second", "formula": "revenue * 2"}
            ]
        }"#;

        let mut registry = Registry::new();
        let err = registry
            .load_from_json_str(json)
            .expect_err("duplicate ids within one document must be rejected");
        assert!(
            err.to_string().contains("Duplicate metric ID"),
            "expected a duplicate-id diagnostic, got: {err}"
        );
        assert!(
            registry.is_empty(),
            "a rejected document must not leave metrics behind"
        );
    }

    /// The same id in *different* namespaces stays legal: the namespace is part
    /// of a metric's identity.
    #[test]
    fn same_metric_id_in_different_namespaces_is_allowed() {
        let mut registry = Registry::new();
        registry
            .load_from_json_str(
                r#"{"namespace":"a","metrics":[{"id":"m1","name":"M","formula":"revenue * 1"}]}"#,
            )
            .expect("first namespace loads");
        registry
            .load_from_json_str(
                r#"{"namespace":"b","metrics":[{"id":"m1","name":"M","formula":"revenue * 2"}]}"#,
            )
            .expect("same id in another namespace is a distinct metric");
        assert_eq!(registry.len(), 2);
    }
}
