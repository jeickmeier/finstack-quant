//! Dynamic metric registry implementation.

use crate::error::{Error, Result};
use crate::registry::schema::{MetricDefinition, MetricRegistry};
use crate::registry::validation::validate_metric_definition;
use indexmap::{IndexMap, IndexSet};

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
}

/// A registered metric with its reference bindings pinned at registration.
#[derive(Debug, Clone)]
pub struct StoredMetric {
    /// Namespace
    pub namespace: String,

    /// Metric definition
    pub definition: MetricDefinition,

    /// The formula with every reference that bound to a metric written as
    /// that metric's qualified identifier. A later registration does not
    /// change it.
    pub qualified_formula: String,

    /// Qualified identifiers of the metrics the formula binds to, in order
    /// of first reference. Historical references (`lag`, windows) are
    /// included.
    pub dependencies: Vec<String>,

    /// References that bound to no metric: the model nodes the formula
    /// requires.
    pub requirements: Vec<String>,
}

impl Registry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            metrics: IndexMap::new(),
        }
    }

    /// Create a new registry preloaded with built-in metrics (fin.* namespace).
    ///
    /// This is a shortcut for `Registry::new().load_builtins()` that avoids the
    /// two-step pattern in call sites.
    ///
    /// # Example
    ///
    /// ```
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
    /// ```
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
        // Stage the whole catalogue so a rejected document leaves nothing
        // behind.
        let mut staged = self.clone();
        for &json in crate::registry::builtins::builtin_metric_sources() {
            staged.load_from_json_str(json)?;
        }
        *self = staged;
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
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry whose validated definitions are loaded or queried.
    pub fn load_registry(&mut self, registry: MetricRegistry) -> Result<()> {
        let namespace = registry.namespace;
        if namespace.is_empty() {
            return Err(Error::registry(
                "Namespace cannot be empty. Provide a namespace identifier (e.g., 'fin', 'custom').",
            ));
        }
        if !crate::dsl::signatures::is_valid_identifier(&namespace) {
            return Err(Error::registry(format!(
                "Invalid namespace '{namespace}': each dot-separated segment must contain only \
                 letters, digits or underscores, and the first must not start with a digit"
            )));
        }
        if namespace.split('.').next() == Some("cs") {
            return Err(Error::registry(format!(
                "Namespace '{namespace}' is reserved: 'cs' names capital-structure references"
            )));
        }

        // Prepare every definition once before changing the live registry.
        let mut prepared = IndexMap::with_capacity(registry.metrics.len());
        for metric in registry.metrics {
            let qualified_id = metric.qualified_id(&namespace);
            if self.metrics.contains_key(&qualified_id) || prepared.contains_key(&metric.id) {
                return Err(Error::registry(format!(
                    "Duplicate metric ID: '{}'. This metric is already registered in the registry.",
                    qualified_id
                )));
            }
            let ast = validate_metric_definition(&metric, &namespace)?;
            prepared.insert(metric.id.clone(), (metric, ast));
        }

        // Pin every reference against the staged and committed definitions.
        // An unqualified reference binds in this namespace; an exact qualified
        // reference binds in any namespace; anything else is a model input.
        let mut staged = Vec::with_capacity(prepared.len());
        let mut same_period = IndexMap::with_capacity(prepared.len());
        for (id, (definition, ast)) in &prepared {
            let mut dependencies = IndexSet::new();
            let mut requirements = IndexSet::new();
            let qualified_ast = ast.map_node_refs(&mut |name| {
                let local = format!("{namespace}.{name}");
                let bound = if !name.contains('.')
                    && (prepared.contains_key(name) || self.metrics.contains_key(&local))
                {
                    Some(local)
                } else if self.metrics.contains_key(name)
                    || name
                        .strip_prefix(namespace.as_str())
                        .and_then(|rest| rest.strip_prefix('.'))
                        .is_some_and(|rest| prepared.contains_key(rest))
                {
                    Some(name.to_string())
                } else {
                    None
                };
                match bound {
                    Some(qualified) => {
                        dependencies.insert(qualified.clone());
                        qualified
                    }
                    None => {
                        requirements.insert(name.to_string());
                        name.to_string()
                    }
                }
            });
            // Same-period edges inside this document decide the cycle check:
            // a `lag` or `shift` by a positive integer literal reads the past.
            let mut current = IndexSet::new();
            crate::utils::formula::collect_identifiers_from_ast(&qualified_ast, &mut current, true);
            let prefix = format!("{namespace}.");
            same_period.insert(
                id.clone(),
                current
                    .into_iter()
                    .filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned))
                    .filter(|name| prepared.contains_key(name))
                    .collect::<IndexSet<String>>(),
            );
            staged.push((
                id.clone(),
                StoredMetric {
                    namespace: namespace.clone(),
                    definition: definition.clone(),
                    qualified_formula: qualified_ast.to_string(),
                    dependencies: dependencies.into_iter().collect(),
                    requirements: requirements.into_iter().collect(),
                },
            ));
        }
        let order = crate::utils::graph::toposort_ids(&same_period).map_err(|remaining| {
            Error::registry(format!(
                "Circular dependency detected among metrics: {}",
                remaining.join(" -> ")
            ))
        })?;

        let mut staged: IndexMap<String, StoredMetric> = staged.into_iter().collect();
        for id in order {
            let metric = staged.swap_remove(&id).ok_or_else(|| {
                Error::registry(format!("internal error: prepared metric '{id}' is missing"))
            })?;
            self.metrics
                .insert(metric.definition.qualified_id(&namespace), metric);
        }
        Ok(())
    }

    /// Get a metric by fully-qualified ID.
    ///
    /// # Example
    ///
    /// ```
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
            Error::registry_not_found(format!(
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
    /// ```
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

    /// The dependency closure of a metric, in import order.
    ///
    /// Returns the qualified identifiers of every metric the formula binds
    /// to, directly or through another metric, with each dependency before
    /// the metric that consumes it. Qualified cross-namespace references and
    /// historical references are included. The requested metric itself is
    /// not. The bindings are those pinned at registration.
    ///
    /// # Errors
    ///
    /// Returns a registry error if `qualified_id` is unknown.
    ///
    /// # Arguments
    ///
    /// * `qualified_id` - Fully qualified registry metric identifier whose
    ///   dependency closure should be returned.
    pub fn get_dependencies(&self, qualified_id: &str) -> Result<Vec<String>> {
        let mut closure = IndexSet::new();
        let mut visited = IndexSet::new();
        self.collect_closure(qualified_id, &mut closure, &mut visited)?;
        closure.shift_remove(qualified_id);
        Ok(closure.into_iter().collect())
    }

    /// Post-order walk: a metric is recorded after its dependencies. A
    /// historical self- or mutual reference is visited once.
    fn collect_closure(
        &self,
        qualified_id: &str,
        closure: &mut IndexSet<String>,
        visited: &mut IndexSet<String>,
    ) -> Result<()> {
        if !visited.insert(qualified_id.to_string()) {
            return Ok(());
        }
        for dependency in &self.get(qualified_id)?.dependencies {
            self.collect_closure(dependency, closure, visited)?;
        }
        closure.insert(qualified_id.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Load one namespace document of `(id, formula)` metrics.
    fn load(registry: &mut Registry, namespace: &str, metrics: &[(&str, &str)]) -> Result<()> {
        let metrics: Vec<_> = metrics
            .iter()
            .map(|(id, formula)| serde_json::json!({"id": id, "name": id, "formula": formula}))
            .collect();
        registry
            .load_from_json_str(
                &serde_json::json!({"namespace": namespace, "schema_version": 1, "metrics": metrics})
                    .to_string(),
            )
            .map(|_| ())
    }

    /// STATEMENTS-CONTRACTS-006 #2: references are qualified in the AST, so
    /// a metric whose id is also a function name cannot collide with a call.
    #[test]
    fn a_metric_named_like_a_function_binds_without_touching_calls() {
        let mut registry = Registry::new();
        load(
            &mut registry,
            "k",
            &[
                ("mean", "revenue - cogs"),
                ("ratio", "mean / mean(revenue, cogs)"),
            ],
        )
        .expect("load");
        let ratio = registry.get("k.ratio").expect("metric");
        assert_eq!(ratio.qualified_formula, "k.mean / mean(revenue, cogs)");
        assert_eq!(ratio.dependencies, ["k.mean"]);
        assert_eq!(ratio.requirements, ["revenue", "cogs"]);
    }

    /// An unqualified reference binds in its own namespace, an exact
    /// qualified reference in any namespace, and the closure follows both.
    #[test]
    fn closure_follows_local_qualified_and_historical_references() {
        let mut registry = Registry::new();
        load(&mut registry, "k", &[("mean", "revenue - cogs")]).expect("first namespace");
        load(
            &mut registry,
            "other",
            &[
                ("uses", "k.mean * 2 + lag(local, 1)"),
                ("local", "deep + x"),
                ("deep", "y"),
                ("mean", "z"),
            ],
        )
        .expect("second namespace");
        let uses = registry.get("other.uses").expect("metric");
        assert_eq!(uses.qualified_formula, "k.mean * 2 + lag(other.local, 1)");
        assert_eq!(uses.dependencies, ["k.mean", "other.local"]);
        // Dependencies before dependents, in reference order.
        assert_eq!(
            registry.get_dependencies("other.uses").expect("closure"),
            ["k.mean", "other.deep", "other.local"]
        );
        // The unqualified `mean` of another namespace is not `k.mean`.
        assert!(
            registry
                .get("other.mean")
                .expect("metric")
                .dependencies
                .is_empty()
        );
        assert!(registry.get_dependencies("nope.metric").is_err());
    }

    /// A binding is pinned at registration: a metric registered later does
    /// not capture an earlier metric's unmatched reference.
    #[test]
    fn a_later_registration_does_not_rebind_an_earlier_metric() {
        let mut registry = Registry::new();
        load(&mut registry, "a", &[("m", "base * 2")]).expect("first document");
        load(&mut registry, "a", &[("base", "1 + 1"), ("n", "base * 3")]).expect("second document");
        let earlier = registry.get("a.m").expect("metric");
        assert_eq!(earlier.qualified_formula, "base * 2");
        assert!(earlier.dependencies.is_empty());
        assert_eq!(earlier.requirements, ["base"]);
        assert_eq!(
            registry.get("a.n").expect("metric").dependencies,
            ["a.base"]
        );
    }

    #[test]
    fn namespaces_follow_the_identifier_grammar_and_cs_is_reserved() {
        for namespace in ["cs", "cs.debt", "9bad", "a..b", "a b", ".a"] {
            let mut registry = Registry::new();
            assert!(
                load(&mut registry, namespace, &[("m", "x")]).is_err(),
                "{namespace}"
            );
            assert!(registry.is_empty());
        }
        let mut registry = Registry::new();
        load(&mut registry, "fin.adj", &[("m", "x")]).expect("dotted namespace");
        assert!(registry.has("fin.adj.m"));
        assert!(
            load(&mut registry, "ok", &[("a.b", "x")]).is_err(),
            "dotted metric id"
        );
    }

    /// Only a same-period reference can close a cycle: a positive literal
    /// `lag` reads the past.
    #[test]
    fn same_period_cycles_are_rejected_and_corkscrews_are_not() {
        let mut registry = Registry::new();
        load(&mut registry, "ok", &[("bal", "lag(bal, 1) + flow")]).expect("corkscrew");
        load(&mut registry, "ok2", &[("a", "lag(b, 1)"), ("b", "a + 1")]).expect("lagged pair");
        for cyclic in [
            &[("x", "y"), ("y", "x")][..],
            &[("p", "lag(q, 0)"), ("q", "p")][..],
            &[("s", "s + 1")][..],
        ] {
            let mut registry = Registry::new();
            let error = load(&mut registry, "bad", cyclic).expect_err("cycle");
            assert!(error.to_string().contains("Circular dependency"), "{error}");
            assert!(
                registry.is_empty(),
                "a rejected document leaves nothing behind"
            );
        }
    }

    #[test]
    fn the_builtin_catalogue_registers_atomically() {
        let mut registry = Registry::new();
        // A metric that collides with a later built-in document.
        load(&mut registry, "fin", &[("roce", "1")]).expect("custom metric");
        assert!(registry.load_builtins().is_err());
        assert_eq!(registry.len(), 1, "no built-in document was kept");
        let mut registry = Registry::with_builtins().expect("catalogue");
        let loaded = registry.len();
        assert!(registry.load_builtins().is_err());
        assert_eq!(registry.len(), loaded);
    }

    /// Two metrics sharing an `id` inside one document must be rejected.
    ///
    /// Collecting definitions into an `IndexMap` without checking keys can
    /// silently coalesce a pair into a single last-wins entry. For a financial
    /// metric registry, silently discarding one of
    /// two conflicting definitions of the same metric is a data-integrity
    /// hazard.
    #[test]
    fn intra_document_duplicate_metric_ids_are_rejected() {
        let json = r#"{
            "namespace": "custom",
            "schema_version": 1,
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
                r#"{"namespace":"a","schema_version":1,"metrics":[{"id":"m1","name":"M","formula":"revenue * 1"}]}"#,
            )
            .expect("first namespace loads");
        registry
            .load_from_json_str(
                r#"{"namespace":"b","schema_version":1,"metrics":[{"id":"m1","name":"M","formula":"revenue * 2"}]}"#,
            )
            .expect("same id in another namespace is a distinct metric");
        assert_eq!(registry.len(), 2);
    }
}
