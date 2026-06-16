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
    pub fn load_builtins(&mut self) -> Result<()> {
        // Load from embedded JSON files
        for json in crate::registry::builtins::builtin_metric_sources()? {
            self.load_from_json_str(&json)?;
        }
        Ok(())
    }

    /// Load metrics from a JSON file path.
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
    pub fn load_from_json(&mut self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        self.load_from_json_str(&json)?;
        Ok(())
    }

    /// Load metrics from a JSON string.
    ///
    /// Returns the deserialized [`MetricRegistry`]
    /// for further inspection when needed.
    pub fn load_from_json_str(&mut self, json: &str) -> Result<MetricRegistry> {
        let registry: MetricRegistry = serde_json::from_str(json)?;
        self.load_registry(registry.clone())?;
        Ok(registry)
    }

    /// Load a metric registry.
    ///
    /// Validates all metrics and checks for collisions.
    /// Supports inter-metric dependencies - metrics can reference other metrics in the same registry.
    pub fn load_registry(&mut self, registry: MetricRegistry) -> Result<()> {
        let namespace = registry.namespace.clone();

        // Validate namespace
        if namespace.is_empty() {
            return Err(Error::registry(
                "Namespace cannot be empty. Provide a namespace identifier (e.g., 'fin', 'custom')."
            ));
        }

        // Track namespace
        self.namespaces.insert(namespace.clone());

        // Sort metrics by dependency order
        let sorted_metrics = self.sort_metrics_by_dependencies(&registry)?;

        // Load each metric in dependency order
        for metric in sorted_metrics {
            // Validate metric
            validate_metric_definition(&metric, &namespace)?;

            // Check for collisions
            let qualified_id = metric.qualified_id(&namespace);
            if self.metrics.contains_key(&qualified_id) {
                return Err(Error::registry(format!(
                    "Duplicate metric ID: '{}'. This metric is already registered in the registry.",
                    qualified_id
                )));
            }

            // Store metric
            self.metrics.insert(
                qualified_id,
                StoredMetric {
                    namespace: namespace.clone(),
                    definition: metric,
                },
            );
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

        // Build map of metric_id -> MetricDefinition for lookup
        let metric_map: IndexMap<String, MetricDefinition> = registry
            .metrics
            .iter()
            .map(|m| (m.id.to_owned(), m.clone()))
            .collect();

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

    /// Get dependencies for a specific metric (including transitive dependencies).
    ///
    /// Returns the ordered list of metric IDs (qualified) that must be added before this metric.
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
